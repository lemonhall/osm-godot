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
use chunk_grid::{ChunkGrid, ElementMetadata, SceneElement};
use geometry::MeshData;
use std::collections::{HashMap, HashSet};
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
    pub stream_radius: i32,
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
            stream_radius: 2,
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
        self.add_mesh_with_metadata(
            name,
            mesh_data,
            material,
            world_x,
            world_z,
            ElementMetadata::new(),
        );
    }

    pub fn add_mesh_with_metadata(
        &mut self,
        name: String,
        mesh_data: MeshData,
        material: MaterialType,
        world_x: i32,
        world_z: i32,
        metadata: ElementMetadata,
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
            metadata,
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
        let assets_dir = self.output_dir.join("assets");

        fs::create_dir_all(&scenes_dir)?;
        fs::create_dir_all(&materials_dir)?;
        fs::create_dir_all(&scripts_dir)?;
        fs::create_dir_all(&mesh_data_dir)?;
        fs::create_dir_all(&assets_dir)?;

        // Write materials
        tres_writer::write_all_materials(&materials_dir)?;
        self.write_cloud_texture_asset(&assets_dir)?;
        self.write_fps_player_script(&scripts_dir)?;
        self.write_chunk_mesh_loader_script(&scripts_dir)?;
        self.write_world_streamer_script(&scripts_dir)?;

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
        self.write_world_manifest()?;
        self.write_navigation_index()?;

        // Write project files
        project_writer::write_project_file(&self.output_dir, "OSM Godot World")?;
        project_writer::write_default_environment(&self.output_dir)?;
        project_writer::write_metadata(
            &self.output_dir,
            0.0,
            0.0,
            0.0,
            0.0, // Placeholder geo bounds (updated in save_all_with_geo)
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

        // load_steps = scripts/textures + scene subresources. Chunks are loaded at runtime
        // from world_manifest.json by world_streamer.gd.
        let load_steps = 12;
        writeln!(
            f,
            "[gd_scene load_steps={load_steps} format=3 uid=\"uid://master000001\"]"
        )?;
        writeln!(f)?;

        writeln!(f, "[ext_resource type=\"Script\" path=\"res://scripts/fps_player.gd\" id=\"player_script\"]")?;
        writeln!(f, "[ext_resource type=\"Script\" path=\"res://scripts/world_streamer.gd\" id=\"streamer_script\"]")?;
        writeln!(f, "[ext_resource type=\"Texture2D\" path=\"res://assets/cloud_billboard.png\" id=\"cloud_texture\"]")?;
        writeln!(f)?;

        let world_cx = (self.chunk_grid.xzbbox.min_x() + self.chunk_grid.xzbbox.max_x()) as f32
            * 0.5
            * self.godot_scale;
        let world_cz = -(self.chunk_grid.xzbbox.min_z() + self.chunk_grid.xzbbox.max_z()) as f32
            * 0.5
            * self.godot_scale;
        let span_x = (self.chunk_grid.xzbbox.max_x() - self.chunk_grid.xzbbox.min_x()).abs() as f32
            * self.godot_scale;
        let span_z = (self.chunk_grid.xzbbox.max_z() - self.chunk_grid.xzbbox.min_z()).abs() as f32
            * self.godot_scale;
        let span = span_x.max(span_z).max(1.0);
        let floor_size_x = span_x.max(16.0) + 64.0;
        let floor_size_z = span_z.max(16.0) + 64.0;

        // Environment
        writeln!(f, "[sub_resource type=\"ProceduralSkyMaterial\" id=\"1\"]")?;
        writeln!(f, "sky_top_color = Color(0.23, 0.58, 0.96, 1)")?;
        writeln!(f, "sky_horizon_color = Color(0.72, 0.88, 1, 1)")?;
        writeln!(f, "ground_bottom_color = Color(0.32, 0.48, 0.28, 1)")?;
        writeln!(f, "ground_horizon_color = Color(0.70, 0.82, 0.62, 1)")?;
        writeln!(f, "sun_angle_max = 8.0")?;
        writeln!(f, "sun_curve = 0.08")?;
        writeln!(f)?;

        writeln!(f, "[sub_resource type=\"Sky\" id=\"2\"]")?;
        writeln!(f, "sky_material = SubResource(\"1\")")?;
        writeln!(f)?;

        writeln!(f, "[sub_resource type=\"Environment\" id=\"3\"]")?;
        writeln!(f, "background_mode = 2")?;
        writeln!(f, "sky = SubResource(\"2\")")?;
        writeln!(f, "ambient_light_color = Color(0.78, 0.82, 0.88, 1)")?;
        writeln!(f, "ambient_light_energy = 1.15")?;
        writeln!(f, "ambient_source = 3")?; // Color + Sky
        writeln!(f)?;

        writeln!(f, "[sub_resource type=\"CapsuleShape3D\" id=\"4\"]")?;
        writeln!(f, "radius = 0.35")?;
        writeln!(f, "height = 1.8")?;
        writeln!(f)?;

        writeln!(f, "[sub_resource type=\"SphereMesh\" id=\"5\"]")?;
        writeln!(f, "radius = 1.0")?;
        writeln!(f, "height = 2.0")?;
        writeln!(f)?;

        writeln!(f, "[sub_resource type=\"StandardMaterial3D\" id=\"6\"]")?;
        writeln!(f, "albedo_color = Color(1, 0.82, 0.20, 1)")?;
        writeln!(f, "emission_enabled = true")?;
        writeln!(f, "emission = Color(1, 0.70, 0.12, 1)")?;
        writeln!(f, "emission_energy_multiplier = 2.5")?;
        writeln!(f)?;

        writeln!(f, "[sub_resource type=\"BoxShape3D\" id=\"7\"]")?;
        writeln!(
            f,
            "size = Vector3({floor_size_x:.4}, 1.0, {floor_size_z:.4})"
        )?;
        writeln!(f)?;

        // Root
        writeln!(f, "[node name=\"World\" type=\"Node3D\"]")?;
        writeln!(f)?;

        // WorldEnvironment
        writeln!(
            f,
            "[node name=\"WorldEnvironment\" type=\"WorldEnvironment\" parent=\".\"]"
        )?;
        writeln!(f, "environment = SubResource(\"3\")")?;
        writeln!(f)?;

        // DirectionalLight (sun)
        writeln!(
            f,
            "[node name=\"Sun\" type=\"DirectionalLight3D\" parent=\".\"]"
        )?;
        writeln!(f, "transform = Transform3D(0.707, 0.408, -0.577, 0, 0.816, 0.577, 0.707, -0.408, 0.577, 0, 0, 0)")?;
        writeln!(f, "light_color = Color(1, 0.92, 0.78, 1)")?;
        writeln!(f, "light_energy = 2.4")?;
        writeln!(f, "shadow_enabled = true")?;
        writeln!(f)?;

        let (player_x, player_y, player_z) = self.player_spawn_godot(world_cx, world_cz, span);

        writeln!(
            f,
            "[node name=\"SunDisk\" type=\"MeshInstance3D\" parent=\".\"]"
        )?;
        writeln!(f, "transform = Transform3D(5, 0, 0, 0, 5, 0, 0, 0, 5, {world_cx:.4}, 140.0000, {sun_z:.4})", sun_z = world_cz - span * 0.35)?;
        writeln!(f, "mesh = SubResource(\"5\")")?;
        writeln!(f, "material_override = SubResource(\"6\")")?;
        writeln!(f)?;

        writeln!(f, "[node name=\"Clouds\" type=\"Node3D\" parent=\".\"]")?;
        writeln!(f)?;
        for (i, (dx, dy, dz, sx, _sy, sz)) in [
            (-0.30, 92.0, -0.15, 14.0, 3.0, 7.0),
            (-0.12, 96.0, -0.20, 10.0, 2.4, 5.0),
            (0.18, 88.0, -0.10, 16.0, 3.2, 6.0),
            (0.34, 102.0, 0.04, 12.0, 2.8, 5.5),
        ]
        .iter()
        .enumerate()
        {
            let cloud_x = world_cx + span * dx;
            let cloud_z = world_cz + span * dz;
            let pixel_size = 0.055 * f32::max(*sx, *sz);
            writeln!(
                f,
                "[node name=\"Cloud_{i}\" type=\"Sprite3D\" parent=\"Clouds\"]"
            )?;
            writeln!(f, "transform = Transform3D(1, 0, 0, 0, 1, 0, 0, 0, 1, {cloud_x:.4}, {dy:.4}, {cloud_z:.4})")?;
            writeln!(f, "billboard = 1")?;
            writeln!(f, "transparent = true")?;
            writeln!(f, "modulate = Color(1, 1, 1, 0.92)")?;
            writeln!(f, "pixel_size = {pixel_size:.4}")?;
            writeln!(f, "texture = ExtResource(\"cloud_texture\")")?;
            writeln!(f)?;
        }

        writeln!(
            f,
            "[node name=\"WorldFloor\" type=\"StaticBody3D\" parent=\".\"]"
        )?;
        writeln!(f, "transform = Transform3D(1, 0, 0, 0, 1, 0, 0, 0, 1, {world_cx:.4}, -0.5000, {world_cz:.4})")?;
        writeln!(f)?;

        writeln!(
            f,
            "[node name=\"CollisionShape3D\" type=\"CollisionShape3D\" parent=\"WorldFloor\"]"
        )?;
        writeln!(f, "shape = SubResource(\"7\")")?;
        writeln!(f)?;

        writeln!(
            f,
            "[node name=\"Player\" type=\"CharacterBody3D\" parent=\".\"]"
        )?;
        writeln!(f, "transform = Transform3D(1, 0, 0, 0, 1, 0, 0, 0, 1, {player_x:.4}, {player_y:.4}, {player_z:.4})")?;
        writeln!(f, "floor_snap_length = 0.35")?;
        writeln!(f, "script = ExtResource(\"player_script\")")?;
        writeln!(f)?;

        writeln!(
            f,
            "[node name=\"CollisionShape3D\" type=\"CollisionShape3D\" parent=\"Player\"]"
        )?;
        writeln!(f, "shape = SubResource(\"4\")")?;
        writeln!(f)?;

        writeln!(
            f,
            "[node name=\"Camera3D\" type=\"Camera3D\" parent=\"Player\"]"
        )?;
        writeln!(
            f,
            "transform = Transform3D(1, 0, 0, 0, 0.9781, -0.2079, 0, 0.2079, 0.9781, 0, 1.6, 0)"
        )?;
        writeln!(f, "current = true")?;
        writeln!(f, "far = 10000.0")?;
        writeln!(f)?;

        writeln!(
            f,
            "[node name=\"WorldStreamer\" type=\"Node3D\" parent=\".\"]"
        )?;
        writeln!(f, "script = ExtResource(\"streamer_script\")")?;
        writeln!(f, "manifest_path = \"res://world_manifest.json\"")?;
        writeln!(f, "player_path = NodePath(\"../Player\")")?;
        writeln!(f, "stream_radius = {}", self.stream_radius.max(0))?;
        writeln!(f, "unload_radius = {}", self.stream_radius.max(0) + 1)?;
        writeln!(f)?;

        Ok(())
    }

    fn write_world_manifest(&self) -> std::io::Result<()> {
        let mut coords = self.chunk_grid.all_coords();
        coords.sort_by_key(|coord| (coord.0, coord.1));

        let chunks: Vec<_> = coords
            .into_iter()
            .filter_map(|coord| {
                let chunk = &self.chunk_grid.chunks[&coord];
                if chunk.elements.is_empty() {
                    return None;
                }
                let (min_x, min_z, max_x, max_z) = chunk.world_bounds;
                let origin_x = min_x as f32 * self.godot_scale;
                let origin_z = -(min_z as f32) * self.godot_scale;
                let bounds = self.chunk_grid.chunk_bounds_godot(coord, self.godot_scale);
                let road_count = chunk
                    .elements
                    .iter()
                    .filter(|element| match element {
                        SceneElement::Mesh { material_type, .. } => {
                            matches!(
                                material_type,
                                MaterialType::RoadAsphalt | MaterialType::RoadSidewalk
                            )
                        }
                        SceneElement::Instance { .. } => false,
                    })
                    .count();
                Some(serde_json::json!({
                    "coord": [coord.0, coord.1],
                    "world_bounds_blocks": [min_x, min_z, max_x, max_z],
                    "bounds_godot": [bounds.0, bounds.1, bounds.2, bounds.3],
                    "origin": [origin_x, origin_z],
                    "scene_path": format!("res://scenes/Chunk_{}_{}.tscn", coord.0, coord.1),
                    "mesh_data_path": format!("res://mesh_data/Chunk_{}_{}.json", coord.0, coord.1),
                    "element_count": chunk.elements.len(),
                    "road_count": road_count,
                }))
            })
            .collect();

        let payload = serde_json::json!({
            "version": 1,
            "chunk_size_blocks": self.chunk_grid.chunk_size,
            "godot_scale": self.godot_scale,
            "stream_radius": self.stream_radius.max(0),
            "chunks": chunks,
        });
        std::fs::write(
            self.output_dir.join("world_manifest.json"),
            serde_json::to_string(&payload)?,
        )
    }

    fn write_navigation_index(&self) -> std::io::Result<()> {
        let mut coords = self.chunk_grid.all_coords();
        coords.sort_by_key(|coord| (coord.0, coord.1));
        let mut seen: HashSet<(String, String)> = HashSet::new();
        let mut entries = Vec::new();

        for coord in coords {
            let chunk = &self.chunk_grid.chunks[&coord];
            let chunk_x = chunk.world_bounds.0 as f32 * self.godot_scale;
            let chunk_z = -(chunk.world_bounds.1 as f32) * self.godot_scale;

            for element in &chunk.elements {
                let SceneElement::Mesh {
                    mesh_data,
                    transform,
                    metadata,
                    ..
                } = element
                else {
                    continue;
                };
                let Some(osm_kind) = metadata.get("osm_kind") else {
                    continue;
                };
                if osm_kind != "road" && osm_kind != "building" {
                    continue;
                }
                let Some(osm_id) = metadata.get("osm_id") else {
                    continue;
                };
                if !seen.insert((osm_kind.clone(), osm_id.clone())) {
                    continue;
                }

                let (min_x, min_z, max_x, max_z) =
                    mesh_bounds_godot(mesh_data, chunk_x, chunk_z, transform);
                let center_x = (min_x + max_x) * 0.5;
                let center_z = (min_z + max_z) * 0.5;
                let mut entry = serde_json::Map::new();
                entry.insert("osm_id".to_string(), serde_json::json!(osm_id));
                entry.insert("osm_kind".to_string(), serde_json::json!(osm_kind));
                entry.insert("chunk".to_string(), serde_json::json!([coord.0, coord.1]));
                entry.insert(
                    "center".to_string(),
                    serde_json::json!([center_x, center_z]),
                );
                entry.insert(
                    "bbox".to_string(),
                    serde_json::json!([min_x, min_z, max_x, max_z]),
                );
                for key in [
                    "name",
                    "official_name",
                    "alt_name",
                    "building",
                    "highway",
                    "amenity",
                    "shop",
                    "tourism",
                    "addr:city",
                    "addr:street",
                    "addr:housenumber",
                ] {
                    if let Some(value) = metadata.get(key) {
                        entry.insert(key.to_string(), serde_json::json!(value));
                    }
                }
                entries.push(serde_json::Value::Object(entry));
            }
        }

        let payload = serde_json::json!({
            "version": 1,
            "entries": entries,
        });
        std::fs::write(
            self.output_dir.join("navigation_index.json"),
            serde_json::to_string(&payload)?,
        )
    }

    fn player_spawn_godot(&self, world_cx: f32, world_cz: f32, span: f32) -> (f32, f32, f32) {
        if let Some(spawn) = self.find_walkable_spawn_godot(world_cx, world_cz) {
            return spawn;
        }

        let player_z = world_cz + (span * 0.35).clamp(20.0, 120.0);
        let player_x_blocks = (world_cx / self.godot_scale).round() as i32;
        let player_z_blocks = (-player_z / self.godot_scale).round() as i32;
        let player_y = self.ground_y_at(player_x_blocks, player_z_blocks) + 1.0;
        (world_cx, player_y, player_z)
    }

    fn find_walkable_spawn_godot(&self, world_cx: f32, world_cz: f32) -> Option<(f32, f32, f32)> {
        let mut best: Option<(u8, f32, f32, f32, f32)> = None;

        for chunk in self.chunk_grid.chunks.values() {
            let chunk_x = chunk.world_bounds.0 as f32 * self.godot_scale;
            let chunk_z = -(chunk.world_bounds.1 as f32) * self.godot_scale;

            for element in &chunk.elements {
                let SceneElement::Mesh {
                    material_type,
                    transform,
                    ..
                } = element
                else {
                    continue;
                };
                let Some(priority) = spawn_material_priority(*material_type) else {
                    continue;
                };

                let x = chunk_x + transform[9];
                let y = transform[10] + 1.0;
                let z = chunk_z + transform[11];
                let distance2 = (x - world_cx).powi(2) + (z - world_cz).powi(2);

                let should_replace = best
                    .map(|(best_priority, best_distance2, _, _, _)| {
                        priority < best_priority
                            || (priority == best_priority && distance2 < best_distance2)
                    })
                    .unwrap_or(true);
                if should_replace {
                    best = Some((priority, distance2, x, y, z));
                }
            }
        }

        best.map(|(_, _, x, y, z)| (x, y, z))
    }

    fn write_cloud_texture_asset(&self, assets_dir: &std::path::Path) -> std::io::Result<()> {
        let path = assets_dir.join("cloud_billboard.png");
        let mut img = image::RgbaImage::from_pixel(512, 192, image::Rgba([0, 0, 0, 0]));

        draw_cloud_ellipse(&mut img, 262.0, 132.0, 164.0, 26.0, [215, 236, 255, 90]);
        draw_cloud_ellipse(&mut img, 154.0, 112.0, 118.0, 42.0, [255, 255, 255, 235]);
        draw_cloud_ellipse(&mut img, 260.0, 102.0, 132.0, 50.0, [255, 255, 255, 235]);
        draw_cloud_ellipse(&mut img, 360.0, 118.0, 98.0, 38.0, [255, 255, 255, 235]);
        draw_cloud_ellipse(&mut img, 210.0, 78.0, 48.0, 48.0, [255, 255, 255, 235]);
        draw_cloud_ellipse(&mut img, 298.0, 70.0, 58.0, 58.0, [255, 255, 255, 235]);
        draw_cloud_ellipse(&mut img, 364.0, 88.0, 44.0, 44.0, [255, 255, 255, 235]);

        img.save(&path)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

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
        writeln!(f, "@export var noclip_speed_multiplier := 2.5")?;
        writeln!(f, "@export var jump_velocity := 5.5")?;
        writeln!(f, "@export var mouse_sensitivity := 0.0025")?;
        writeln!(
            f,
            "var gravity := float(ProjectSettings.get_setting(\"physics/3d/default_gravity\"))"
        )?;
        writeln!(f, "var noclip := false")?;
        writeln!(f, "var look_enabled := true")?;
        writeln!(f)?;
        writeln!(f, "@onready var camera: Camera3D = $Camera3D")?;
        writeln!(
            f,
            "@onready var collision_shape: CollisionShape3D = $CollisionShape3D"
        )?;
        writeln!(f)?;
        writeln!(f, "func _ready() -> void:")?;
        writeln!(f, "\tlook_enabled = true")?;
        writeln!(f, "\tInput.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)")?;
        writeln!(f)?;
        writeln!(f, "func _input(event: InputEvent) -> void:")?;
        writeln!(f, "\tif event.is_action_pressed(\"mouse_capture_toggle\"):")?;
        writeln!(f, "\t\tif look_enabled:")?;
        writeln!(f, "\t\t\tlook_enabled = false")?;
        writeln!(f, "\t\t\tInput.set_mouse_mode(Input.MOUSE_MODE_VISIBLE)")?;
        writeln!(f, "\t\telse:")?;
        writeln!(f, "\t\t\tlook_enabled = true")?;
        writeln!(f, "\t\t\tInput.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)")?;
        writeln!(f, "\tif event is InputEventMouseButton and event.button_index == MOUSE_BUTTON_LEFT and event.pressed:")?;
        writeln!(f, "\t\tlook_enabled = true")?;
        writeln!(f, "\t\tInput.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)")?;
        writeln!(f, "\tif event.is_action_pressed(\"noclip_toggle\") or _is_key_pressed_once(event, KEY_V):")?;
        writeln!(f, "\t\tnoclip = not noclip")?;
        writeln!(f, "\t\tcollision_shape.disabled = noclip")?;
        writeln!(f, "\tif event is InputEventMouseMotion and look_enabled:")?;
        writeln!(f, "\t\trotate_y(-event.relative.x * mouse_sensitivity)")?;
        writeln!(
            f,
            "\t\tcamera.rotate_x(-event.relative.y * mouse_sensitivity)"
        )?;
        writeln!(
            f,
            "\t\tcamera.rotation.x = clamp(camera.rotation.x, deg_to_rad(-85.0), deg_to_rad(85.0))"
        )?;
        writeln!(f)?;
        writeln!(f, "func _physics_process(delta: float) -> void:")?;
        writeln!(f, "\tvar input_dir := _movement_input_vector()")?;
        writeln!(f, "\tvar direction := (transform.basis * Vector3(input_dir.x, 0.0, input_dir.y)).normalized()")?;
        writeln!(f, "\tvar speed := move_speed")?;
        writeln!(
            f,
            "\tif Input.is_action_pressed(\"sprint\") or Input.is_key_pressed(KEY_SHIFT):"
        )?;
        writeln!(f, "\t\tspeed *= sprint_multiplier")?;
        writeln!(f, "\tif noclip:")?;
        writeln!(
            f,
            "\t\t_noclip_move(delta, direction, speed * noclip_speed_multiplier)"
        )?;
        writeln!(f, "\t\treturn")?;
        writeln!(f, "\tvelocity.x = direction.x * speed")?;
        writeln!(f, "\tvelocity.z = direction.z * speed")?;
        writeln!(f, "\tif is_on_floor():")?;
        writeln!(f, "\t\tif Input.is_action_just_pressed(\"jump\"):")?;
        writeln!(f, "\t\t\tvelocity.y = jump_velocity")?;
        writeln!(f, "\t\telse:")?;
        writeln!(f, "\t\t\tvelocity.y = -0.1")?;
        writeln!(f, "\telse:")?;
        writeln!(f, "\t\tvelocity.y -= gravity * delta")?;
        writeln!(f, "\tmove_and_slide()")?;
        writeln!(f)?;
        writeln!(
            f,
            "func _noclip_move(delta: float, direction: Vector3, speed: float) -> void:"
        )?;
        writeln!(f, "\tvar vertical := 0.0")?;
        writeln!(f, "\tif Input.is_action_pressed(\"jump\"):")?;
        writeln!(f, "\t\tvertical += 1.0")?;
        writeln!(f, "\tif Input.is_action_pressed(\"descend\"):")?;
        writeln!(f, "\t\tvertical -= 1.0")?;
        writeln!(
            f,
            "\tglobal_position += (direction + Vector3.UP * vertical).normalized() * speed * delta"
        )?;
        writeln!(f, "\tvelocity = Vector3.ZERO")?;
        writeln!(f)?;
        writeln!(f, "func _movement_input_vector() -> Vector2:")?;
        writeln!(f, "\tvar input_dir := Input.get_vector(\"move_left\", \"move_right\", \"move_forward\", \"move_backward\")")?;
        writeln!(f, "\tvar direct := Vector2.ZERO")?;
        writeln!(
            f,
            "\tif Input.is_key_pressed(KEY_A) or Input.is_key_pressed(KEY_LEFT):"
        )?;
        writeln!(f, "\t\tdirect.x -= 1.0")?;
        writeln!(
            f,
            "\tif Input.is_key_pressed(KEY_D) or Input.is_key_pressed(KEY_RIGHT):"
        )?;
        writeln!(f, "\t\tdirect.x += 1.0")?;
        writeln!(
            f,
            "\tif Input.is_key_pressed(KEY_W) or Input.is_key_pressed(KEY_UP):"
        )?;
        writeln!(f, "\t\tdirect.y -= 1.0")?;
        writeln!(
            f,
            "\tif Input.is_key_pressed(KEY_S) or Input.is_key_pressed(KEY_DOWN):"
        )?;
        writeln!(f, "\t\tdirect.y += 1.0")?;
        writeln!(f, "\tif direct.length_squared() > 0.0:")?;
        writeln!(f, "\t\treturn direct.normalized()")?;
        writeln!(f, "\treturn input_dir")?;
        writeln!(f)?;
        writeln!(
            f,
            "func _is_key_pressed_once(event: InputEvent, key: Key) -> bool:"
        )?;
        writeln!(f, "\tif not (event is InputEventKey):")?;
        writeln!(f, "\t\treturn false")?;
        writeln!(f, "\tvar key_event := event as InputEventKey")?;
        writeln!(f, "\treturn key_event.pressed and not key_event.echo and (key_event.keycode == key or key_event.physical_keycode == key)")?;

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
        writeln!(
            f,
            "\tvar file := FileAccess.open(mesh_data_path, FileAccess.READ)"
        )?;
        writeln!(f, "\tif file == null:")?;
        writeln!(
            f,
            "\t\tpush_error(\"Failed to open mesh data: \" + mesh_data_path)"
        )?;
        writeln!(f, "\t\treturn")?;
        writeln!(
            f,
            "\tvar parsed: Variant = JSON.parse_string(file.get_as_text())"
        )?;
        writeln!(f, "\tif typeof(parsed) != TYPE_DICTIONARY:")?;
        writeln!(
            f,
            "\t\tpush_error(\"Invalid mesh data: \" + mesh_data_path)"
        )?;
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
        writeln!(
            f,
            "\tarrays[Mesh.ARRAY_VERTEX] = _to_vec3_array(element.get(\"vertices\", []))"
        )?;
        writeln!(
            f,
            "\tarrays[Mesh.ARRAY_NORMAL] = _to_vec3_array(element.get(\"normals\", []))"
        )?;
        writeln!(
            f,
            "\tarrays[Mesh.ARRAY_TEX_UV] = _to_vec2_array(element.get(\"uvs\", []))"
        )?;
        writeln!(
            f,
            "\tarrays[Mesh.ARRAY_INDEX] = _to_int_array(element.get(\"indices\", []))"
        )?;
        writeln!(
            f,
            "\tif arrays[Mesh.ARRAY_VERTEX].is_empty() or arrays[Mesh.ARRAY_INDEX].is_empty():"
        )?;
        writeln!(f, "\t\treturn")?;
        writeln!(
            f,
            "\tmesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)"
        )?;
        writeln!(f, "\tvar instance := MeshInstance3D.new()")?;
        writeln!(f, "\tinstance.name = str(element.get(\"name\", \"Mesh\"))")?;
        writeln!(f, "\tinstance.mesh = mesh")?;
        writeln!(
            f,
            "\tinstance.transform = _to_transform(element.get(\"transform\", []))"
        )?;
        writeln!(
            f,
            "\tvar material_name := str(element.get(\"material\", \"terrain_grass\"))"
        )?;
        writeln!(
            f,
            "\tvar material = load(\"res://materials/\" + material_name + \".tres\")"
        )?;
        writeln!(f, "\tif material != null:")?;
        writeln!(f, "\t\tinstance.set_surface_override_material(0, material)")?;
        writeln!(f, "\tinstance.set_meta(\"osm_generated\", true)")?;
        writeln!(
            f,
            "\t_apply_metadata(instance, element.get(\"metadata\", {{}}))"
        )?;
        writeln!(f, "\tadd_child(instance)")?;
        writeln!(
            f,
            "\tinstance.owner = get_tree().edited_scene_root if Engine.is_editor_hint() else owner"
        )?;
        writeln!(f, "\tif _should_add_collision(material_name):")?;
        writeln!(
            f,
            "\t\t_add_collision_body(str(instance.name), mesh, instance.transform)"
        )?;
        writeln!(f)?;
        writeln!(f, "func _add_collision_body(source_name: String, mesh: ArrayMesh, source_transform: Transform3D) -> void:")?;
        writeln!(f, "\tvar body := StaticBody3D.new()")?;
        writeln!(f, "\tbody.name = source_name + \"_Collision\"")?;
        writeln!(f, "\tbody.transform = source_transform")?;
        writeln!(f, "\tbody.set_meta(\"osm_generated\", true)")?;
        writeln!(f, "\tvar shape := CollisionShape3D.new()")?;
        writeln!(f, "\tshape.shape = mesh.create_trimesh_shape()")?;
        writeln!(f, "\tbody.add_child(shape)")?;
        writeln!(f, "\tadd_child(body)")?;
        writeln!(
            f,
            "\tbody.owner = get_tree().edited_scene_root if Engine.is_editor_hint() else owner"
        )?;
        writeln!(f, "\tshape.owner = body.owner")?;
        writeln!(f)?;
        writeln!(
            f,
            "func _apply_metadata(node: Node, metadata: Variant) -> void:"
        )?;
        writeln!(f, "\tif typeof(metadata) != TYPE_DICTIONARY:")?;
        writeln!(f, "\t\treturn")?;
        writeln!(f, "\tnode.set_meta(\"osm_metadata\", metadata)")?;
        writeln!(f, "\tfor key in metadata.keys():")?;
        writeln!(
            f,
            "\t\tnode.set_meta(StringName(_sanitize_meta_key(str(key))), metadata[key])"
        )?;
        writeln!(f)?;
        writeln!(f, "func _sanitize_meta_key(key: String) -> String:")?;
        writeln!(f, "\tvar out := key")?;
        writeln!(f, "\tout = out.replace(\":\", \"_\")")?;
        writeln!(f, "\tout = out.replace(\"-\", \"_\")")?;
        writeln!(f, "\tout = out.replace(\".\", \"_\")")?;
        writeln!(f, "\tout = out.replace(\" \", \"_\")")?;
        writeln!(f, "\treturn out")?;
        writeln!(f)?;
        writeln!(
            f,
            "func _should_add_collision(material_name: String) -> bool:"
        )?;
        writeln!(f, "\treturn (")?;
        writeln!(f, "\t\tmaterial_name.begins_with(\"terrain_\")")?;
        writeln!(f, "\t)")?;
        writeln!(f)?;
        writeln!(
            f,
            "func _to_vec3_array(values: Array) -> PackedVector3Array:"
        )?;
        writeln!(f, "\tvar out := PackedVector3Array()")?;
        writeln!(f, "\tfor i in range(0, values.size() - 2, 3):")?;
        writeln!(
            f,
            "\t\tout.append(Vector3(float(values[i]), float(values[i + 1]), float(values[i + 2])))"
        )?;
        writeln!(f, "\treturn out")?;
        writeln!(f)?;
        writeln!(
            f,
            "func _to_vec2_array(values: Array) -> PackedVector2Array:"
        )?;
        writeln!(f, "\tvar out := PackedVector2Array()")?;
        writeln!(f, "\tfor i in range(0, values.size() - 1, 2):")?;
        writeln!(
            f,
            "\t\tout.append(Vector2(float(values[i]), float(values[i + 1])))"
        )?;
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
        writeln!(
            f,
            "\t\tVector3(float(values[0]), float(values[1]), float(values[2])),"
        )?;
        writeln!(
            f,
            "\t\tVector3(float(values[3]), float(values[4]), float(values[5])),"
        )?;
        writeln!(
            f,
            "\t\tVector3(float(values[6]), float(values[7]), float(values[8]))"
        )?;
        writeln!(
            f,
            "\t), Vector3(float(values[9]), float(values[10]), float(values[11])))"
        )?;

        Ok(())
    }

    fn write_world_streamer_script(&self, scripts_dir: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;

        let path = scripts_dir.join("world_streamer.gd");
        let mut f = std::fs::File::create(&path)?;

        writeln!(f, "extends Node3D")?;
        writeln!(f)?;
        writeln!(
            f,
            "@export var manifest_path := \"res://world_manifest.json\""
        )?;
        writeln!(
            f,
            "@export var player_path: NodePath = NodePath(\"../Player\")"
        )?;
        writeln!(
            f,
            "@export var stream_radius := {}",
            self.stream_radius.max(0)
        )?;
        writeln!(
            f,
            "@export var unload_radius := {}",
            self.stream_radius.max(0) + 1
        )?;
        writeln!(f)?;
        writeln!(f, "var manifest := {{}}")?;
        writeln!(f, "var chunk_entries := {{}}")?;
        writeln!(f, "var loaded_chunks := {{}}")?;
        writeln!(f, "var player: Node3D = null")?;
        writeln!(f)?;
        writeln!(f, "func _ready() -> void:")?;
        writeln!(f, "\t_load_manifest()")?;
        writeln!(f, "\tplayer = get_node_or_null(player_path) as Node3D")?;
        writeln!(f, "\t_refresh_streaming()")?;
        writeln!(f)?;
        writeln!(f, "func _physics_process(_delta: float) -> void:")?;
        writeln!(f, "\t_refresh_streaming()")?;
        writeln!(f)?;
        writeln!(f, "func _load_manifest() -> void:")?;
        writeln!(
            f,
            "\tvar file := FileAccess.open(manifest_path, FileAccess.READ)"
        )?;
        writeln!(f, "\tif file == null:")?;
        writeln!(
            f,
            "\t\tpush_error(\"Failed to open world manifest: \" + manifest_path)"
        )?;
        writeln!(f, "\t\treturn")?;
        writeln!(f, "\tvar parsed = JSON.parse_string(file.get_as_text())")?;
        writeln!(f, "\tif typeof(parsed) != TYPE_DICTIONARY:")?;
        writeln!(
            f,
            "\t\tpush_error(\"Invalid world manifest: \" + manifest_path)"
        )?;
        writeln!(f, "\t\treturn")?;
        writeln!(f, "\tmanifest = parsed")?;
        writeln!(f, "\tchunk_entries.clear()")?;
        writeln!(f, "\tfor entry in manifest.get(\"chunks\", []):")?;
        writeln!(f, "\t\tvar coord: Array = entry.get(\"coord\", [])")?;
        writeln!(f, "\t\tif coord.size() < 2:")?;
        writeln!(f, "\t\t\tcontinue")?;
        writeln!(
            f,
            "\t\tchunk_entries[_chunk_key(int(coord[0]), int(coord[1]))] = entry"
        )?;
        writeln!(f)?;
        writeln!(f, "func _refresh_streaming() -> void:")?;
        writeln!(f, "\tif player == null:")?;
        writeln!(f, "\t\tplayer = get_node_or_null(player_path) as Node3D")?;
        writeln!(f, "\tif player == null or chunk_entries.is_empty():")?;
        writeln!(f, "\t\treturn")?;
        writeln!(
            f,
            "\tvar current: Array = _find_player_chunk(player.global_position)"
        )?;
        writeln!(f, "\tif current.size() < 2:")?;
        writeln!(f, "\t\treturn")?;
        writeln!(f, "\tvar keep: Dictionary = {{}}")?;
        writeln!(f, "\tfor dx in range(-stream_radius, stream_radius + 1):")?;
        writeln!(f, "\t\tfor dz in range(-stream_radius, stream_radius + 1):")?;
        writeln!(f, "\t\t\tvar cx := int(current[0]) + dx")?;
        writeln!(f, "\t\t\tvar cz := int(current[1]) + dz")?;
        writeln!(f, "\t\t\tvar key := _chunk_key(cx, cz)")?;
        writeln!(f, "\t\t\tif chunk_entries.has(key):")?;
        writeln!(f, "\t\t\t\tkeep[key] = true")?;
        writeln!(f, "\t\t\t\t_load_chunk(key)")?;
        writeln!(f, "\t_unload_far_chunks(current, keep)")?;
        writeln!(f)?;
        writeln!(f, "func _load_chunk(key: String) -> void:")?;
        writeln!(f, "\tif loaded_chunks.has(key):")?;
        writeln!(f, "\t\treturn")?;
        writeln!(f, "\tvar entry: Dictionary = chunk_entries[key]")?;
        writeln!(
            f,
            "\tvar packed := load(str(entry.get(\"scene_path\", \"\"))) as PackedScene"
        )?;
        writeln!(f, "\tif packed == null:")?;
        writeln!(f, "\t\tpush_error(\"Failed to load chunk scene: \" + str(entry.get(\"scene_path\", \"\")))")?;
        writeln!(f, "\t\treturn")?;
        writeln!(f, "\tvar instance := packed.instantiate() as Node3D")?;
        writeln!(f, "\tvar origin: Array = entry.get(\"origin\", [0.0, 0.0])")?;
        writeln!(
            f,
            "\tinstance.position = Vector3(float(origin[0]), 0.0, float(origin[1]))"
        )?;
        writeln!(f, "\tinstance.set_meta(\"chunk_key\", key)")?;
        writeln!(f, "\tinstance.set_meta(\"streamed_chunk\", true)")?;
        writeln!(f, "\tadd_child(instance)")?;
        writeln!(f, "\tloaded_chunks[key] = instance")?;
        writeln!(f)?;
        writeln!(
            f,
            "func _unload_far_chunks(current: Array, keep: Dictionary) -> void:"
        )?;
        writeln!(f, "\tfor key in loaded_chunks.keys():")?;
        writeln!(f, "\t\tif keep.has(key):")?;
        writeln!(f, "\t\t\tcontinue")?;
        writeln!(f, "\t\tvar coord: Array = _parse_chunk_key(key)")?;
        writeln!(f, "\t\tif coord.size() < 2:")?;
        writeln!(f, "\t\t\tcontinue")?;
        writeln!(f, "\t\tvar dist: int = max(abs(int(coord[0]) - int(current[0])), abs(int(coord[1]) - int(current[1])))")?;
        writeln!(f, "\t\tif dist > unload_radius:")?;
        writeln!(f, "\t\t\tvar node: Node = loaded_chunks[key]")?;
        writeln!(f, "\t\t\tloaded_chunks.erase(key)")?;
        writeln!(f, "\t\t\tnode.queue_free()")?;
        writeln!(f)?;
        writeln!(f, "func _find_player_chunk(pos: Vector3) -> Array:")?;
        writeln!(f, "\tvar best: Array = []")?;
        writeln!(f, "\tvar best_d2: float = INF")?;
        writeln!(f, "\tfor key in chunk_entries.keys():")?;
        writeln!(f, "\t\tvar entry: Dictionary = chunk_entries[key]")?;
        writeln!(f, "\t\tvar bounds: Array = entry.get(\"bounds_godot\", [])")?;
        writeln!(f, "\t\tif bounds.size() < 4:")?;
        writeln!(f, "\t\t\tcontinue")?;
        writeln!(f, "\t\tvar min_x := float(bounds[0])")?;
        writeln!(f, "\t\tvar min_z := float(bounds[1])")?;
        writeln!(f, "\t\tvar max_x := float(bounds[2])")?;
        writeln!(f, "\t\tvar max_z := float(bounds[3])")?;
        writeln!(
            f,
            "\t\tif pos.x >= min_x and pos.x <= max_x and pos.z >= min_z and pos.z <= max_z:"
        )?;
        writeln!(f, "\t\t\treturn entry.get(\"coord\", [])")?;
        writeln!(f, "\t\tvar cx: float = clamp(pos.x, min_x, max_x)")?;
        writeln!(f, "\t\tvar cz: float = clamp(pos.z, min_z, max_z)")?;
        writeln!(
            f,
            "\t\tvar d2: float = Vector2(pos.x - cx, pos.z - cz).length_squared()"
        )?;
        writeln!(f, "\t\tif d2 < best_d2:")?;
        writeln!(f, "\t\t\tbest_d2 = d2")?;
        writeln!(f, "\t\t\tbest = entry.get(\"coord\", [])")?;
        writeln!(f, "\treturn best")?;
        writeln!(f)?;
        writeln!(f, "func _chunk_key(cx: int, cz: int) -> String:")?;
        writeln!(f, "\treturn str(cx) + \":\" + str(cz)")?;
        writeln!(f)?;
        writeln!(f, "func _parse_chunk_key(key: String) -> Array:")?;
        writeln!(f, "\tvar parts := key.split(\":\")")?;
        writeln!(f, "\tif parts.size() < 2:")?;
        writeln!(f, "\t\treturn []")?;
        writeln!(f, "\treturn [int(parts[0]), int(parts[1])]")?;

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

fn draw_cloud_ellipse(
    img: &mut image::RgbaImage,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    rgba: [u8; 4],
) {
    let min_x = ((cx - rx).floor() as i32).max(0) as u32;
    let max_x = ((cx + rx).ceil() as i32).min(img.width() as i32 - 1) as u32;
    let min_y = ((cy - ry).floor() as i32).max(0) as u32;
    let max_y = ((cy + ry).ceil() as i32).min(img.height() as i32 - 1) as u32;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = (x as f32 + 0.5 - cx) / rx;
            let dy = (y as f32 + 0.5 - cy) / ry;
            if dx * dx + dy * dy <= 1.0 {
                blend_pixel(img.get_pixel_mut(x, y), rgba);
            }
        }
    }
}

fn mesh_bounds_godot(
    mesh_data: &MeshData,
    chunk_x: f32,
    chunk_z: f32,
    transform: &[f32; 12],
) -> (f32, f32, f32, f32) {
    if mesh_data.vertices.len() < 3 {
        let x = chunk_x + transform[9];
        let z = chunk_z + transform[11];
        return (x, z, x, z);
    }

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for vertex in mesh_data.vertices.chunks_exact(3) {
        let x = chunk_x + transform[9] + vertex[0];
        let z = chunk_z + transform[11] + vertex[2];
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_z = min_z.min(z);
        max_z = max_z.max(z);
    }
    (min_x, min_z, max_x, max_z)
}

fn blend_pixel(dst: &mut image::Rgba<u8>, src: [u8; 4]) {
    let src_a = src[3] as f32 / 255.0;
    let dst_a = dst[3] as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= f32::EPSILON {
        return;
    }
    for channel in 0..3 {
        let src_c = src[channel] as f32 / 255.0;
        let dst_c = dst[channel] as f32 / 255.0;
        let out_c = (src_c * src_a + dst_c * dst_a * (1.0 - src_a)) / out_a;
        dst[channel] = (out_c * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    dst[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

fn spawn_material_priority(material_type: MaterialType) -> Option<u8> {
    match material_type {
        MaterialType::RoadSidewalk => Some(0),
        MaterialType::RoadAsphalt => Some(1),
        MaterialType::TerrainGrass | MaterialType::TerrainBuiltUp | MaterialType::TerrainDirt => {
            Some(2)
        }
        _ => None,
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
        mesh.vertices
            .extend_from_slice(&[0.0, 0.0, 0.0, 2.0, 2.0, 2.0]);
        mesh
    }

    #[test]
    fn master_scene_has_current_player_camera_for_run_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let master =
            std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
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

        let master =
            std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        assert!(master.contains("[node name=\"Player\" type=\"CharacterBody3D\" parent=\".\"]"));
        assert!(master.contains("[node name=\"WorldStreamer\" type=\"Node3D\" parent=\".\"]"));
        assert!(!master.contains("parent=\"World\""));
    }

    #[test]
    fn master_scene_writes_chunk_scenes_without_static_instances() {
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

        let master =
            std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        assert!(tmp.path().join("scenes").join("Chunk_0_0.tscn").exists());
        assert!(tmp.path().join("mesh_data").join("Chunk_0_0.json").exists());
        assert!(!master.contains("res://scenes/Chunk_0_0.tscn"));
        assert!(!master.contains("[node name=\"Chunk_0_0\""));
        assert!(!master.contains("\ninstance = ExtResource("));
    }

    #[test]
    fn master_scene_streams_roads_through_chunk_data() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let mut scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.add_mesh(
            "Highway_1".to_string(),
            unit_mesh(),
            MaterialType::RoadAsphalt,
            128,
            128,
        );
        scene.save_all().unwrap();

        let master =
            std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        let chunk_data =
            std::fs::read_to_string(tmp.path().join("mesh_data").join("Chunk_0_0.json")).unwrap();
        assert!(!tmp.path().join("mesh_data").join("roads.json").exists());
        assert!(!master.contains("res://scenes/roads.tscn"));
        assert!(!master.contains("[node name=\"Roads\""));
        assert!(chunk_data.contains("\"material\":\"road_asphalt\""));
        assert!(chunk_data.contains("\"name\":\"Highway_1\""));
    }

    #[test]
    fn master_scene_has_fps_player_with_current_camera() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let master =
            std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        assert!(master.contains("[ext_resource type=\"Script\" path=\"res://scripts/fps_player.gd\" id=\"player_script\"]"));
        assert!(master.contains("[node name=\"Player\" type=\"CharacterBody3D\" parent=\".\"]"));
        assert!(master.contains("script = ExtResource(\"player_script\")"));
        assert!(master.contains("[node name=\"Camera3D\" type=\"Camera3D\" parent=\"Player\"]"));
        assert!(master.contains("current = true"));
        assert!(tmp.path().join("scripts").join("fps_player.gd").exists());
    }

    #[test]
    fn master_scene_has_world_floor_collision_for_stable_walking() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let master =
            std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        assert!(master.contains("[sub_resource type=\"BoxShape3D\" id=\"7\"]"));
        assert!(master.contains("[node name=\"WorldFloor\" type=\"StaticBody3D\" parent=\".\"]"));
        assert!(master.contains(
            "[node name=\"CollisionShape3D\" type=\"CollisionShape3D\" parent=\"WorldFloor\"]"
        ));
        assert!(master.contains("shape = SubResource(\"7\")"));
    }

    #[test]
    fn master_scene_has_bright_stylized_sky_sun_and_clouds() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let master =
            std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        assert!(master.contains("[sub_resource type=\"ProceduralSkyMaterial\""));
        assert!(master.contains("[sub_resource type=\"Sky\""));
        assert!(master.contains("background_mode = 2"));
        assert!(master.contains("[node name=\"SunDisk\" type=\"MeshInstance3D\" parent=\".\"]"));
        assert!(master.contains("[node name=\"Clouds\" type=\"Node3D\" parent=\".\"]"));
        assert!(master.contains("light_energy = 2.4"));
    }

    #[test]
    fn master_scene_uses_cloud_billboards_instead_of_blob_meshes() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let master =
            std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        assert!(tmp
            .path()
            .join("assets")
            .join("cloud_billboard.png")
            .exists());
        assert!(master.contains("[ext_resource type=\"Texture2D\" path=\"res://assets/cloud_billboard.png\" id=\"cloud_texture\"]"));
        assert!(master.contains("[node name=\"Cloud_0\" type=\"Sprite3D\" parent=\"Clouds\"]"));
        assert!(master.contains("texture = ExtResource(\"cloud_texture\")"));
        assert!(
            !master.contains("[node name=\"Cloud_0\" type=\"MeshInstance3D\" parent=\"Clouds\"]")
        );
    }

    #[test]
    fn master_scene_spawns_player_on_flat_ground() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let master =
            std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        let player_line = master
            .lines()
            .skip_while(|line| !line.contains("[node name=\"Player\""))
            .find(|line| line.starts_with("transform = Transform3D"))
            .unwrap();
        assert!(
            player_line.contains(", 1.0000, "),
            "player should spawn with capsule near flat ground: {player_line}"
        );
    }

    #[test]
    fn master_scene_prefers_road_spawn_for_walkable_start() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let mut scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.add_mesh(
            "Road_1".to_string(),
            unit_mesh(),
            MaterialType::RoadAsphalt,
            300,
            150,
        );
        scene.save_all().unwrap();

        let master =
            std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        let player_line = master
            .lines()
            .skip_while(|line| !line.contains("[node name=\"Player\""))
            .find(|line| line.starts_with("transform = Transform3D"))
            .unwrap();
        assert!(
            player_line.contains(", 150.0000, 1.0000, -75.0000)"),
            "player should spawn on the road mesh transform: {player_line}"
        );
    }

    #[test]
    fn fps_player_script_uses_gravity_and_floor_motion() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let script =
            std::fs::read_to_string(tmp.path().join("scripts").join("fps_player.gd")).unwrap();
        assert!(script.contains("default_gravity"));
        assert!(script.contains("is_on_floor()"));
        assert!(script.contains("@export var jump_velocity"));
        assert!(script.contains("velocity.y -= gravity * delta"));
        assert!(!script.contains("vertical * speed"));
    }

    #[test]
    fn fps_player_script_has_noclip_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let script =
            std::fs::read_to_string(tmp.path().join("scripts").join("fps_player.gd")).unwrap();
        assert!(script.contains("var noclip := false"));
        assert!(script.contains("noclip_toggle"));
        assert!(script.contains(
            "func _noclip_move(delta: float, direction: Vector3, speed: float) -> void:"
        ));
        assert!(script.contains("collision_shape.disabled = noclip"));
    }

    #[test]
    fn fps_player_script_handles_mouse_in_input_callback() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let script =
            std::fs::read_to_string(tmp.path().join("scripts").join("fps_player.gd")).unwrap();
        assert!(script.contains("func _input(event: InputEvent) -> void:"));
        assert!(script.contains("event is InputEventMouseButton"));
        assert!(script.contains("MOUSE_BUTTON_LEFT"));
        assert!(script.contains("var look_enabled := true"));
        assert!(script.contains("event is InputEventMouseMotion and look_enabled"));
        assert!(!script.contains("Input.get_mouse_mode() == Input.MOUSE_MODE_CAPTURED"));
        assert!(!script.contains("func _unhandled_input(event: InputEvent) -> void:"));
    }

    #[test]
    fn fps_player_script_has_direct_keyboard_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let script =
            std::fs::read_to_string(tmp.path().join("scripts").join("fps_player.gd")).unwrap();
        assert!(script.contains("func _movement_input_vector() -> Vector2:"));
        assert!(script.contains("Input.is_key_pressed(KEY_W)"));
        assert!(script.contains("Input.is_key_pressed(KEY_UP)"));
        assert!(script.contains("key_event.keycode == key"));
        assert!(script.contains("key_event.physical_keycode == key"));
    }

    #[test]
    fn chunk_loader_creates_collision_for_solid_meshes() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let script =
            std::fs::read_to_string(tmp.path().join("scripts").join("chunk_mesh_loader.gd"))
                .unwrap();
        assert!(script.contains("StaticBody3D.new()"));
        assert!(script.contains("CollisionShape3D.new()"));
        assert!(script.contains("mesh.create_trimesh_shape()"));
        assert!(script.contains("func _apply_metadata(node: Node, metadata: Variant) -> void:"));
        assert!(script.contains("node.set_meta(\"osm_metadata\", metadata)"));
        assert!(script.contains("func _sanitize_meta_key(key: String) -> String:"));
        assert!(script.contains("out = out.replace(\":\", \"_\")"));
        assert!(script.contains("func _should_add_collision(material_name: String) -> bool:"));
        assert!(script.contains("material_name.begins_with(\"terrain_\")"));
        assert!(!script.contains("material_name.begins_with(\"road_\")"));
        assert!(!script.contains("material_name == \"railway_gravel\""));
        assert!(!script.contains("material_name == \"building_wall\""));
    }

    #[test]
    fn world_streaming_manifest_and_master_do_not_static_load_chunks() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(1400.0, 1400.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let mut scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 128, 0.5);

        for i in 0..24 {
            let x = 20 + (i % 6) * 160;
            let z = 20 + (i / 6) * 160;
            scene.add_mesh(
                format!("BuildingWall_{i}"),
                unit_mesh(),
                MaterialType::BuildingWall,
                x,
                z,
            );
        }

        scene.save_all().unwrap();

        let master =
            std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        assert!(!master.contains("res://scenes/Chunk_"));
        assert!(!master.contains("parent=\"Chunks\" instance=ExtResource"));
        assert!(master.contains("world_streamer.gd"));

        let manifest_path = tmp.path().join("world_manifest.json");
        assert!(manifest_path.exists());
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(manifest_path).unwrap()).unwrap();
        let chunks = manifest["chunks"].as_array().unwrap();
        assert!(
            chunks.len() >= 12,
            "expected many manifest chunks, got {chunks:?}"
        );
        assert!(chunks[0]["coord"].is_array());
        assert!(chunks[0]["scene_path"]
            .as_str()
            .unwrap()
            .starts_with("res://scenes/"));
        assert!(chunks[0]["mesh_data_path"]
            .as_str()
            .unwrap()
            .starts_with("res://mesh_data/"));
    }

    #[test]
    fn world_streaming_script_has_radius_load_and_unload_logic() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 128, 0.5);

        scene.save_all().unwrap();

        let script =
            std::fs::read_to_string(tmp.path().join("scripts").join("world_streamer.gd")).unwrap();
        assert!(script.contains("@export var stream_radius"));
        assert!(script.contains("@export var unload_radius"));
        assert!(script.contains("func _load_chunk"));
        assert!(script.contains("func _unload_far_chunks"));
        assert!(script.contains("world_manifest.json"));
        assert!(script.contains("loaded_chunks"));
    }

    #[test]
    fn world_streaming_navigation_index_contains_buildings_and_roads() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let mut scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 128, 0.5);

        scene.add_mesh_with_metadata(
            "BuildingWall_100".to_string(),
            unit_mesh(),
            MaterialType::BuildingWall,
            20,
            20,
            [
                ("osm_id".to_string(), "100".to_string()),
                ("osm_kind".to_string(), "building".to_string()),
                ("name".to_string(), "Streaming Test Building".to_string()),
                ("building".to_string(), "office".to_string()),
            ]
            .into_iter()
            .collect(),
        );
        scene.add_mesh_with_metadata(
            "Highway_200".to_string(),
            unit_mesh(),
            MaterialType::RoadAsphalt,
            40,
            40,
            [
                ("osm_id".to_string(), "200".to_string()),
                ("osm_kind".to_string(), "road".to_string()),
                ("name".to_string(), "Streaming Test Road".to_string()),
                ("highway".to_string(), "primary".to_string()),
            ]
            .into_iter()
            .collect(),
        );

        scene.save_all().unwrap();

        let index_path = tmp.path().join("navigation_index.json");
        assert!(index_path.exists());
        let index: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(index_path).unwrap()).unwrap();
        let entries = index["entries"].as_array().unwrap();
        assert!(entries.iter().any(|e| e["osm_kind"] == "building"
            && e["name"] == "Streaming Test Building"
            && e["chunk"].is_array()
            && e["center"].is_array()
            && e["bbox"].is_array()));
        assert!(entries.iter().any(|e| e["osm_kind"] == "road"
            && e["name"] == "Streaming Test Road"
            && e["highway"] == "primary"
            && e["chunk"].is_array()));
    }
}
