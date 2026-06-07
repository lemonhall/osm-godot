//! Scene writer module — constructs Godot .tscn scenes from processed OSM elements.
//!
//! This is the core output layer that replaces arnis's `world_editor` module.
//! Instead of placing Minecraft blocks, we construct procedural 3D meshes
//! and write them as Godot scene files.
//!
//! ## Submodules
//! - `geometry`     — Procedural mesh primitives (boxes, walls, roofs, cylinders)
//! - `mesh_builder` — Bridge between OSM element geometry and MeshData
//! - `chunk_grid`   — Spatial partitioning into chunk-sized units
//! - `tscn_writer`  — Godot .tscn text format generation
//! - `tres_writer`  — Godot .tres material resource generation
//! - `project_writer` — Godot project.godot file generation

pub mod chunk_grid;
pub mod geometry;
pub mod mesh_builder;
pub mod project_writer;
pub mod tres_writer;
pub mod tscn_writer;

use crate::coordinate_system::cartesian::XZBBox;
use crate::ground::Ground;
use chunk_grid::ChunkGrid;
use geometry::MeshData;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tres_writer::MaterialType;

/// Main orchestrator for scene generation.
/// Collects meshes during element processing and writes them all on save.
pub struct SceneWriter {
    pub chunk_grid: ChunkGrid,
    pub ground: Arc<Ground>,
    pub output_dir: PathBuf,
    pub godot_scale: f32,
    material_ids: HashMap<MaterialType, u32>,
}

impl SceneWriter {
    /// Create a new SceneWriter targeting the given output directory.
    pub fn new(
        xzbbox: &XZBBox,
        ground: Arc<Ground>,
        output_dir: PathBuf,
        chunk_size: i32,
        godot_scale: f32,
    ) -> Self {
        let chunk_grid = ChunkGrid::new(xzbbox, chunk_size);

        // Pre-assign material ext_resource IDs (1..N)
        let material_ids: HashMap<MaterialType, u32> = tres_writer::ALL_MATERIALS
            .iter()
            .enumerate()
            .map(|(i, m)| (*m, (i + 1) as u32))
            .collect();

        SceneWriter {
            chunk_grid,
            ground,
            output_dir,
            godot_scale,
            material_ids,
        }
    }

    /// Get ground elevation in Godot meters at arnis block coordinates.
    pub fn ground_y_at(&self, x: i32, z: i32) -> f32 {
        use crate::coordinate_system::cartesian::XZPoint;
        let level = self.ground.level(XZPoint::new(x, z));
        level as f32 * self.godot_scale
    }

    /// Add a mesh element to the appropriate chunk.
    pub fn add_mesh(
        &mut self,
        name: String,
        mesh_data: MeshData,
        material: MaterialType,
        world_x: i32,
        world_z: i32,
    ) {
        let ground_y = self.ground_y_at(world_x, world_z);
        self.chunk_grid.add_mesh_element(
            name,
            mesh_data,
            material,
            world_x,
            world_z,
            self.godot_scale,
            ground_y,
        );
    }

    /// Add an instanced mesh (for trees, lamps, etc.) at the given world position.
    pub fn add_instance(
        &mut self,
        name: String,
        mesh_data: MeshData,
        material: MaterialType,
        world_x: i32,
        world_z: i32,
        y_rotation: f32,
    ) {
        let ground_y = self.ground_y_at(world_x, world_z);
        self.chunk_grid.add_instance(
            name,
            mesh_data,
            material,
            world_x,
            world_z,
            self.godot_scale,
            ground_y,
            y_rotation,
        );
    }

    /// Finalize: write all chunks, materials, and project files to disk.
    pub fn save_all(&self) -> std::io::Result<()> {
        use std::fs;

        let scenes_dir = self.output_dir.join("scenes");
        let materials_dir = self.output_dir.join("materials");
        let scripts_dir = self.output_dir.join("scripts");
        let mesh_data_dir = self.output_dir.join("mesh_data");

        fs::create_dir_all(&scenes_dir)?;
        fs::create_dir_all(&materials_dir)?;
        fs::create_dir_all(&scripts_dir)?;
        fs::create_dir_all(&mesh_data_dir)?;

        // Write materials
        tres_writer::write_all_materials(&materials_dir)?;
        self.write_fps_player_script(&scripts_dir)?;
        self.write_chunk_mesh_loader_script(&scripts_dir)?;

        // Write each chunk scene
        let mut non_empty_count = 0u64;
        for coord in self.chunk_grid.all_coords() {
            let chunk = &self.chunk_grid.chunks[&coord];
            if chunk.elements.is_empty() {
                continue;
            }
            tscn_writer::write_chunk_scene(chunk, &scenes_dir, &mesh_data_dir, &self.material_ids)?;
            non_empty_count += 1;
        }

        // Write master scene
        self.write_master_scene(&scenes_dir)?;

        // Write project files
        project_writer::write_project_file(&self.output_dir, "OSM Godot World")?;
        project_writer::write_default_environment(&self.output_dir)?;
        project_writer::write_metadata(
            &self.output_dir,
            0.0, 0.0, 0.0, 0.0, // Placeholder geo bounds (updated in save_all_with_geo)
            self.godot_scale,
            self.chunk_grid.chunk_size,
        )?;

        println!(
            "  Wrote {} non-empty chunk scenes to {}",
            non_empty_count,
            scenes_dir.display()
        );

        Ok(())
    }

    /// Generate master.tscn — chunk loader with Camera + Light for Run mode.
    fn write_master_scene(&self, scenes_dir: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;

        let path = scenes_dir.join("master.tscn");
        let mut f = std::fs::File::create(&path)?;

        let non_empty: Vec<_> = self
            .chunk_grid
            .all_coords()
            .into_iter()
            .filter(|c| !self.chunk_grid.chunks[c].elements.is_empty())
            .collect();

        // load_steps = chunk PackedScenes + player script + environment + capsule shape.
        let load_steps = non_empty.len() as u32 + 3;
        writeln!(f, "[gd_scene load_steps={load_steps} format=3 uid=\"uid://master000001\"]")?;
        writeln!(f)?;

        writeln!(f, "[ext_resource type=\"Script\" path=\"res://scripts/fps_player.gd\" id=\"player_script\"]")?;

        // Ext resources: each chunk PackedScene
        let mut chunk_eids: HashMap<chunk_grid::ChunkCoord, u32> = HashMap::new();
        for (i, coord) in non_empty.iter().enumerate() {
            let eid = (i + 1) as u32;
            writeln!(f, "[ext_resource type=\"PackedScene\" path=\"res://scenes/Chunk_{}_{}.tscn\" id=\"{eid}\"]", coord.0, coord.1)?;
            chunk_eids.insert(*coord, eid);
        }
        writeln!(f)?;

        // Environment
        writeln!(f, "[sub_resource type=\"Environment\" id=\"1\"]")?;
        writeln!(f, "background_mode = 0")?; // Clear color
        writeln!(f, "background_color = Color(0.45, 0.55, 0.70, 1)")?;
        writeln!(f, "ambient_light_color = Color(0.4, 0.4, 0.45, 1)")?;
        writeln!(f, "ambient_light_energy = 0.6")?;
        writeln!(f, "ambient_source = 3")?; // Color + Sky
        writeln!(f)?;

        writeln!(f, "[sub_resource type=\"CapsuleShape3D\" id=\"2\"]")?;
        writeln!(f, "radius = 0.35")?;
        writeln!(f, "height = 1.8")?;
        writeln!(f)?;

        // Root
        writeln!(f, "[node name=\"World\" type=\"Node3D\"]")?;
        writeln!(f)?;

        // WorldEnvironment
        writeln!(f, "[node name=\"WorldEnvironment\" type=\"WorldEnvironment\" parent=\".\"]")?;
        writeln!(f, "environment = SubResource(\"1\")")?;
        writeln!(f)?;

        // DirectionalLight (sun)
        writeln!(f, "[node name=\"Sun\" type=\"DirectionalLight3D\" parent=\".\"]")?;
        writeln!(f, "transform = Transform3D(0.707, 0.408, -0.577, 0, 0.816, 0.577, 0.707, -0.408, 0.577, 0, 0, 0)")?;
        writeln!(f, "shadow_enabled = true")?;
        writeln!(f)?;

        let world_cx = (self.chunk_grid.xzbbox.min_x() + self.chunk_grid.xzbbox.max_x()) as f32 * 0.5 * self.godot_scale;
        let world_cz = -(self.chunk_grid.xzbbox.min_z() + self.chunk_grid.xzbbox.max_z()) as f32 * 0.5 * self.godot_scale;
        let span_x = (self.chunk_grid.xzbbox.max_x() - self.chunk_grid.xzbbox.min_x()).abs() as f32 * self.godot_scale;
        let span_z = (self.chunk_grid.xzbbox.max_z() - self.chunk_grid.xzbbox.min_z()).abs() as f32 * self.godot_scale;
        let span = span_x.max(span_z).max(1.0);

        // Player starts above the south side of the generated bounds, facing into the city.
        let player_y = (span * 0.08).clamp(10.0, 45.0);
        let player_z = world_cz + (span * 0.35).clamp(20.0, 120.0);
        writeln!(f, "[node name=\"Player\" type=\"CharacterBody3D\" parent=\".\"]")?;
        writeln!(f, "transform = Transform3D(1, 0, 0, 0, 1, 0, 0, 0, 1, {world_cx:.4}, {player_y:.4}, {player_z:.4})")?;
        writeln!(f, "script = ExtResource(\"player_script\")")?;
        writeln!(f)?;

        writeln!(f, "[node name=\"CollisionShape3D\" type=\"CollisionShape3D\" parent=\"Player\"]")?;
        writeln!(f, "shape = SubResource(\"2\")")?;
        writeln!(f)?;

        writeln!(f, "[node name=\"Camera3D\" type=\"Camera3D\" parent=\"Player\"]")?;
        writeln!(f, "transform = Transform3D(1, 0, 0, 0, 0.9781, -0.2079, 0, 0.2079, 0.9781, 0, 1.6, 0)")?;
        writeln!(f, "current = true")?;
        writeln!(f, "far = 10000.0")?;
        writeln!(f)?;

        // Chunk instances
        let chunks_group = "Chunks";
        writeln!(f, "[node name=\"{chunks_group}\" type=\"Node3D\" parent=\".\"]")?;
        writeln!(f)?;

        for coord in &non_empty {
            let eid = chunk_eids[coord];
            let cname = format!("Chunk_{}_{}", coord.0, coord.1);
            let chunk = &self.chunk_grid.chunks[coord];
            let (min_x, min_z, _, _) = chunk.world_bounds;
            let gx = min_x as f32 * self.godot_scale;
            let gz = -(min_z as f32) * self.godot_scale;
            writeln!(f, "[node name=\"{cname}\" parent=\"{chunks_group}\" instance=ExtResource(\"{eid}\")]")?;
            writeln!(f, "transform = Transform3D(1, 0, 0, 0, 1, 0, 0, 0, 1, {gx}, 0, {gz})")?;
            writeln!(f)?;
        }

        Ok(())
    }

    fn write_fps_player_script(&self, scripts_dir: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;

        let path = scripts_dir.join("fps_player.gd");
        let mut f = std::fs::File::create(&path)?;

        writeln!(f, "extends CharacterBody3D")?;
        writeln!(f)?;
        writeln!(f, "@export var move_speed := 14.0")?;
        writeln!(f, "@export var sprint_multiplier := 2.0")?;
        writeln!(f, "@export var mouse_sensitivity := 0.0025")?;
        writeln!(f)?;
        writeln!(f, "@onready var camera: Camera3D = $Camera3D")?;
        writeln!(f)?;
        writeln!(f, "func _ready() -> void:")?;
        writeln!(f, "\tInput.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)")?;
        writeln!(f)?;
        writeln!(f, "func _unhandled_input(event: InputEvent) -> void:")?;
        writeln!(f, "\tif event.is_action_pressed(\"mouse_capture_toggle\"):")?;
        writeln!(f, "\t\tif Input.get_mouse_mode() == Input.MOUSE_MODE_CAPTURED:")?;
        writeln!(f, "\t\t\tInput.set_mouse_mode(Input.MOUSE_MODE_VISIBLE)")?;
        writeln!(f, "\t\telse:")?;
        writeln!(f, "\t\t\tInput.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)")?;
        writeln!(f, "\tif event is InputEventMouseMotion and Input.get_mouse_mode() == Input.MOUSE_MODE_CAPTURED:")?;
        writeln!(f, "\t\trotate_y(-event.relative.x * mouse_sensitivity)")?;
        writeln!(f, "\t\tcamera.rotate_x(-event.relative.y * mouse_sensitivity)")?;
        writeln!(f, "\t\tcamera.rotation.x = clamp(camera.rotation.x, deg_to_rad(-85.0), deg_to_rad(85.0))")?;
        writeln!(f)?;
        writeln!(f, "func _physics_process(_delta: float) -> void:")?;
        writeln!(f, "\tvar input_dir := Input.get_vector(\"move_left\", \"move_right\", \"move_forward\", \"move_backward\")")?;
        writeln!(f, "\tvar direction := (transform.basis * Vector3(input_dir.x, 0.0, input_dir.y)).normalized()")?;
        writeln!(f, "\tvar vertical := 0.0")?;
        writeln!(f, "\tif Input.is_action_pressed(\"jump\"):")?;
        writeln!(f, "\t\tvertical += 1.0")?;
        writeln!(f, "\tif Input.is_action_pressed(\"descend\"):")?;
        writeln!(f, "\t\tvertical -= 1.0")?;
        writeln!(f, "\tvar speed := move_speed")?;
        writeln!(f, "\tif Input.is_action_pressed(\"sprint\"):")?;
        writeln!(f, "\t\tspeed *= sprint_multiplier")?;
        writeln!(f, "\tvelocity = Vector3(direction.x * speed, vertical * speed, direction.z * speed)")?;
        writeln!(f, "\tmove_and_slide()")?;

        Ok(())
    }

    fn write_chunk_mesh_loader_script(&self, scripts_dir: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;

        let path = scripts_dir.join("chunk_mesh_loader.gd");
        let mut f = std::fs::File::create(&path)?;

        writeln!(f, "@tool")?;
        writeln!(f, "extends Node3D")?;
        writeln!(f)?;
        writeln!(f, "@export var mesh_data_path := \"\"")?;
        writeln!(f)?;
        writeln!(f, "func _ready() -> void:")?;
        writeln!(f, "\t_load_meshes()")?;
        writeln!(f)?;
        writeln!(f, "func _load_meshes() -> void:")?;
        writeln!(f, "\t_clear_generated_children()")?;
        writeln!(f, "\tif mesh_data_path.is_empty():")?;
        writeln!(f, "\t\treturn")?;
        writeln!(f, "\tvar file := FileAccess.open(mesh_data_path, FileAccess.READ)")?;
        writeln!(f, "\tif file == null:")?;
        writeln!(f, "\t\tpush_error(\"Failed to open mesh data: \" + mesh_data_path)")?;
        writeln!(f, "\t\treturn")?;
        writeln!(f, "\tvar parsed = JSON.parse_string(file.get_as_text())")?;
        writeln!(f, "\tif typeof(parsed) != TYPE_DICTIONARY:")?;
        writeln!(f, "\t\tpush_error(\"Invalid mesh data: \" + mesh_data_path)")?;
        writeln!(f, "\t\treturn")?;
        writeln!(f, "\tfor element in parsed.get(\"elements\", []):")?;
        writeln!(f, "\t\t_add_mesh_instance(element)")?;
        writeln!(f)?;
        writeln!(f, "func _clear_generated_children() -> void:")?;
        writeln!(f, "\tfor child in get_children():")?;
        writeln!(f, "\t\tif child.get_meta(\"osm_generated\", false):")?;
        writeln!(f, "\t\t\tremove_child(child)")?;
        writeln!(f, "\t\t\tchild.queue_free()")?;
        writeln!(f)?;
        writeln!(f, "func _add_mesh_instance(element: Dictionary) -> void:")?;
        writeln!(f, "\tvar mesh := ArrayMesh.new()")?;
        writeln!(f, "\tvar arrays := []")?;
        writeln!(f, "\tarrays.resize(Mesh.ARRAY_MAX)")?;
        writeln!(f, "\tarrays[Mesh.ARRAY_VERTEX] = _to_vec3_array(element.get(\"vertices\", []))")?;
        writeln!(f, "\tarrays[Mesh.ARRAY_NORMAL] = _to_vec3_array(element.get(\"normals\", []))")?;
        writeln!(f, "\tarrays[Mesh.ARRAY_TEX_UV] = _to_vec2_array(element.get(\"uvs\", []))")?;
        writeln!(f, "\tarrays[Mesh.ARRAY_INDEX] = _to_int_array(element.get(\"indices\", []))")?;
        writeln!(f, "\tif arrays[Mesh.ARRAY_VERTEX].is_empty() or arrays[Mesh.ARRAY_INDEX].is_empty():")?;
        writeln!(f, "\t\treturn")?;
        writeln!(f, "\tmesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)")?;
        writeln!(f, "\tvar instance := MeshInstance3D.new()")?;
        writeln!(f, "\tinstance.name = str(element.get(\"name\", \"Mesh\"))")?;
        writeln!(f, "\tinstance.mesh = mesh")?;
        writeln!(f, "\tinstance.transform = _to_transform(element.get(\"transform\", []))")?;
        writeln!(f, "\tvar material = load(\"res://materials/\" + str(element.get(\"material\", \"terrain_grass\")) + \".tres\")")?;
        writeln!(f, "\tif material != null:")?;
        writeln!(f, "\t\tinstance.set_surface_override_material(0, material)")?;
        writeln!(f, "\tinstance.set_meta(\"osm_generated\", true)")?;
        writeln!(f, "\tadd_child(instance)")?;
        writeln!(f, "\tinstance.owner = get_tree().edited_scene_root if Engine.is_editor_hint() else owner")?;
        writeln!(f)?;
        writeln!(f, "func _to_vec3_array(values: Array) -> PackedVector3Array:")?;
        writeln!(f, "\tvar out := PackedVector3Array()")?;
        writeln!(f, "\tfor i in range(0, values.size() - 2, 3):")?;
        writeln!(f, "\t\tout.append(Vector3(float(values[i]), float(values[i + 1]), float(values[i + 2])))")?;
        writeln!(f, "\treturn out")?;
        writeln!(f)?;
        writeln!(f, "func _to_vec2_array(values: Array) -> PackedVector2Array:")?;
        writeln!(f, "\tvar out := PackedVector2Array()")?;
        writeln!(f, "\tfor i in range(0, values.size() - 1, 2):")?;
        writeln!(f, "\t\tout.append(Vector2(float(values[i]), float(values[i + 1])))")?;
        writeln!(f, "\treturn out")?;
        writeln!(f)?;
        writeln!(f, "func _to_int_array(values: Array) -> PackedInt32Array:")?;
        writeln!(f, "\tvar out := PackedInt32Array()")?;
        writeln!(f, "\tfor value in values:")?;
        writeln!(f, "\t\tout.append(int(value))")?;
        writeln!(f, "\treturn out")?;
        writeln!(f)?;
        writeln!(f, "func _to_transform(values: Array) -> Transform3D:")?;
        writeln!(f, "\tif values.size() < 12:")?;
        writeln!(f, "\t\treturn Transform3D.IDENTITY")?;
        writeln!(f, "\treturn Transform3D(Basis(")?;
        writeln!(f, "\t\tVector3(float(values[0]), float(values[1]), float(values[2])),")?;
        writeln!(f, "\t\tVector3(float(values[3]), float(values[4]), float(values[5])),")?;
        writeln!(f, "\t\tVector3(float(values[6]), float(values[7]), float(values[8]))")?;
        writeln!(f, "\t), Vector3(float(values[9]), float(values[10]), float(values[11])))")?;

        Ok(())
    }

    /// Number of chunks.
    pub fn chunk_count(&self) -> usize {
        self.chunk_grid.chunk_count()
    }

    /// Number of elements placed across all chunks.
    pub fn element_count(&self) -> usize {
        self.chunk_grid
            .chunks
            .values()
            .map(|c| c.elements.len())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinate_system::cartesian::XZBBox;
    use crate::ground::Ground;
    use crate::scene_writer::geometry::MeshData;

    fn unit_mesh() -> MeshData {
        let mut mesh = MeshData::new();
        mesh.vertices.extend_from_slice(&[0.0, 0.0, 0.0, 2.0, 2.0, 2.0]);
        mesh
    }

    #[test]
    fn master_scene_has_current_player_camera_for_run_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let master = std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        assert!(master.contains("[node name=\"Camera3D\" type=\"Camera3D\" parent=\"Player\"]"));
        assert!(master.contains("current = true"));
        assert!(!master.contains("[node name=\"Camera3D\" type=\"Camera3D\" parent=\".\"]"));
    }

    #[test]
    fn master_scene_attaches_direct_children_to_scene_root() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let master = std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        assert!(master.contains("[node name=\"Player\" type=\"CharacterBody3D\" parent=\".\"]"));
        assert!(master.contains("[node name=\"Chunks\" type=\"Node3D\" parent=\".\"]"));
        assert!(!master.contains("parent=\"World\""));
    }

    #[test]
    fn master_scene_instances_chunk_scenes_on_node_declaration() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let mut scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.add_mesh(
            "BuildingWall_1".to_string(),
            unit_mesh(),
            MaterialType::BuildingWall,
            10,
            10,
        );
        scene.save_all().unwrap();

        let master = std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        assert!(master.contains("[node name=\"Chunk_0_0\" parent=\"Chunks\" instance=ExtResource("));
        assert!(!master.contains("\ninstance = ExtResource("));
    }

    #[test]
    fn master_scene_has_fps_player_with_current_camera() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let master = std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        assert!(master.contains("[ext_resource type=\"Script\" path=\"res://scripts/fps_player.gd\" id=\"player_script\"]"));
        assert!(master.contains("[node name=\"Player\" type=\"CharacterBody3D\" parent=\".\"]"));
        assert!(master.contains("script = ExtResource(\"player_script\")"));
        assert!(master.contains("[node name=\"Camera3D\" type=\"Camera3D\" parent=\"Player\"]"));
        assert!(master.contains("current = true"));
        assert!(tmp.path().join("scripts").join("fps_player.gd").exists());
    }
}
