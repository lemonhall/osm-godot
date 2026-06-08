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
pub mod navigation;
pub mod project_writer;
pub mod tres_writer;
pub mod tscn_writer;

use crate::coordinate_system::cartesian::XZBBox;
use crate::ground::Ground;
use chunk_grid::{ChunkGrid, ElementMetadata, SceneElement};
use geometry::MeshData;
use navigation::NavigationGraphBuilder;
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
    navigation_graph: NavigationGraphBuilder,
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
            navigation_graph: NavigationGraphBuilder::default(),
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

    pub fn add_navigation_road(
        &mut self,
        osm_id: u64,
        tags: &HashMap<String, String>,
        centerline_arnis: &[(f32, f32)],
    ) {
        self.navigation_graph
            .add_road(osm_id, tags, centerline_arnis, self.godot_scale);
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
        self.write_navigation_controller_script(&scripts_dir)?;

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
        self.write_navigation_graph()?;

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
        let load_steps = 13;
        writeln!(
            f,
            "[gd_scene load_steps={load_steps} format=3 uid=\"uid://master000001\"]"
        )?;
        writeln!(f)?;

        writeln!(f, "[ext_resource type=\"Script\" path=\"res://scripts/fps_player.gd\" id=\"player_script\"]")?;
        writeln!(f, "[ext_resource type=\"Script\" path=\"res://scripts/world_streamer.gd\" id=\"streamer_script\"]")?;
        writeln!(f, "[ext_resource type=\"Script\" path=\"res://scripts/navigation_controller.gd\" id=\"navigation_script\"]")?;
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

        writeln!(
            f,
            "[node name=\"NavigationController\" type=\"Node3D\" parent=\".\"]"
        )?;
        writeln!(f, "script = ExtResource(\"navigation_script\")")?;
        writeln!(f, "player_path = NodePath(\"../Player\")")?;
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

    fn write_navigation_graph(&self) -> std::io::Result<()> {
        std::fs::write(
            self.output_dir.join("navigation_graph.json"),
            serde_json::to_string(&self.navigation_graph.to_json_value())?,
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
        writeln!(f, "var controls_enabled := true")?;
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
        writeln!(f, "\tif not controls_enabled:")?;
        writeln!(f, "\t\treturn")?;
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
        writeln!(f, "\tif not controls_enabled:")?;
        writeln!(f, "\t\tvelocity = Vector3.ZERO")?;
        writeln!(f, "\t\treturn")?;
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
        writeln!(f)?;
        writeln!(f, "func set_controls_enabled(enabled: bool) -> void:")?;
        writeln!(f, "\tcontrols_enabled = enabled")?;
        writeln!(f, "\tlook_enabled = enabled")?;
        writeln!(f, "\tvelocity = Vector3.ZERO")?;
        writeln!(f, "\tif enabled:")?;
        writeln!(f, "\t\tInput.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)")?;

        Ok(())
    }

    fn write_chunk_mesh_loader_script(&self, scripts_dir: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;

        let path = scripts_dir.join("chunk_mesh_loader.gd");
        let mut f = std::fs::File::create(&path)?;

        f.write_all(
            r#"extends Node3D

@export var mesh_data_path := ""
@export var load_budget_usec := 2500

var load_thread: Thread = null
var load_state := "idle"
var pending_elements: Array = []
var pending_element_index := 0
var batches := {}
var batch_keys := []
var batch_index := 0

func _ready() -> void:
	set_meta("chunk_loading_complete", false)
	_begin_threaded_load()

func _exit_tree() -> void:
	if load_thread != null and load_thread.is_started():
		load_thread.wait_to_finish()

func _process(_delta: float) -> void:
	if load_state == "reading":
		_collect_thread_result()
	elif load_state == "batching":
		_process_pending_elements()
	elif load_state == "creating":
		_process_pending_batches()

func _begin_threaded_load() -> void:
	_clear_generated_children()
	pending_elements.clear()
	pending_element_index = 0
	batches.clear()
	batch_keys.clear()
	batch_index = 0
	if mesh_data_path.is_empty():
		_finish_loading()
		return
	load_thread = Thread.new()
	load_state = "reading"
	var err := load_thread.start(Callable(self, "_load_mesh_data_thread"))
	if err != OK:
		push_error("Failed to start chunk load thread: " + str(err))
		_finish_loading()

func _load_mesh_data_thread() -> Dictionary:
	var file := FileAccess.open(mesh_data_path, FileAccess.READ)
	if file == null:
		return {"ok": false, "error": "Failed to open mesh data: " + mesh_data_path}
	var parsed: Variant = JSON.parse_string(file.get_as_text())
	if typeof(parsed) != TYPE_DICTIONARY:
		return {"ok": false, "error": "Invalid mesh data: " + mesh_data_path}
	return {"ok": true, "elements": parsed.get("elements", [])}

func _collect_thread_result() -> void:
	if load_thread == null or load_thread.is_alive():
		return
	var result: Variant = load_thread.wait_to_finish()
	load_thread = null
	if typeof(result) != TYPE_DICTIONARY or not bool(result.get("ok", false)):
		push_error(str(result.get("error", "Failed to load chunk mesh data")))
		_finish_loading()
		return
	pending_elements = result.get("elements", [])
	pending_element_index = 0
	load_state = "batching"

func _process_pending_elements() -> void:
	var start: int = Time.get_ticks_usec()
	while pending_element_index < pending_elements.size():
		_add_element_to_batch(pending_elements[pending_element_index], batches)
		pending_element_index += 1
		if Time.get_ticks_usec() - start >= load_budget_usec:
			return
	pending_elements.clear()
	batch_keys = batches.keys()
	batch_index = 0
	load_state = "creating"

func _process_pending_batches() -> void:
	var start: int = Time.get_ticks_usec()
	while batch_index < batch_keys.size():
		var material_name := str(batch_keys[batch_index])
		_create_batch_instance(material_name, batches[material_name])
		batch_index += 1
		if Time.get_ticks_usec() - start >= load_budget_usec:
			return
	_finish_loading()

func _finish_loading() -> void:
	load_state = "complete"
	set_meta("chunk_loading_complete", true)
	set_process(false)

func _clear_generated_children() -> void:
	for child in get_children():
		if child.get_meta("osm_generated", false):
			remove_child(child)
			child.queue_free()

func _new_batch() -> Dictionary:
	return {
		"vertices": [],
		"normals": [],
		"uvs": [],
		"indices": [],
		"collision": false,
		"road": false,
		"count": 0,
	}

func _add_element_to_batch(element: Dictionary, batches: Dictionary) -> void:
	var material_name := str(element.get("material", "terrain_grass"))
	if not batches.has(material_name):
		batches[material_name] = _new_batch()
	var batch: Dictionary = batches[material_name]
	var transform := _to_transform(element.get("transform", []))
	var vertices: Array = element.get("vertices", [])
	var indices: Array = element.get("indices", [])
	if vertices.is_empty() or indices.is_empty():
		return
	var base_index: int = batch["vertices"].size()
	for i in range(0, vertices.size() - 2, 3):
		var vertex := Vector3(float(vertices[i]), float(vertices[i + 1]), float(vertices[i + 2]))
		batch["vertices"].append(transform * vertex)
	var normals: Array = element.get("normals", [])
	for i in range(0, normals.size() - 2, 3):
		var normal := Vector3(float(normals[i]), float(normals[i + 1]), float(normals[i + 2]))
		batch["normals"].append((transform.basis * normal).normalized())
	var uvs: Array = element.get("uvs", [])
	for i in range(0, uvs.size() - 1, 2):
		batch["uvs"].append(Vector2(float(uvs[i]), float(uvs[i + 1])))
	for value in indices:
		batch["indices"].append(int(value) + base_index)
	batch["collision"] = bool(batch["collision"]) or _should_add_collision(material_name)
	batch["road"] = bool(batch["road"]) or material_name.begins_with("road_")
	batch["count"] = int(batch["count"]) + 1
	_add_metadata_marker(element, transform)

func _create_batch_instance(material_name: String, batch: Dictionary) -> void:
	var mesh := ArrayMesh.new()
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = PackedVector3Array(batch["vertices"])
	arrays[Mesh.ARRAY_NORMAL] = PackedVector3Array(batch["normals"])
	arrays[Mesh.ARRAY_TEX_UV] = PackedVector2Array(batch["uvs"])
	arrays[Mesh.ARRAY_INDEX] = PackedInt32Array(batch["indices"])
	if arrays[Mesh.ARRAY_VERTEX].is_empty() or arrays[Mesh.ARRAY_INDEX].is_empty():
		return
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	var instance := MeshInstance3D.new()
	instance.name = "Batch_" + material_name
	instance.mesh = mesh
	var material = load("res://materials/" + material_name + ".tres")
	if material != null:
		instance.set_surface_override_material(0, material)
	instance.set_meta("osm_generated", true)
	instance.set_meta("batch_material", material_name)
	instance.set_meta("batch_element_count", int(batch["count"]))
	if bool(batch["road"]):
		instance.set_meta("osm_kind", "road")
	add_child(instance)
	instance.owner = get_tree().edited_scene_root if Engine.is_editor_hint() else owner
	if bool(batch["collision"]):
		_add_collision_body(instance.name, mesh, Transform3D.IDENTITY)

func _add_metadata_marker(element: Dictionary, transform: Transform3D) -> void:
	var metadata: Variant = element.get("metadata", {})
	if typeof(metadata) != TYPE_DICTIONARY or metadata.is_empty():
		return
	var marker := Node3D.new()
	marker.name = str(element.get("name", "Meta")) + "_Meta"
	marker.transform = transform
	marker.set_meta("osm_generated", true)
	_apply_metadata(marker, metadata)
	add_child(marker)
	marker.owner = get_tree().edited_scene_root if Engine.is_editor_hint() else owner

func _add_collision_body(source_name: String, mesh: ArrayMesh, source_transform: Transform3D) -> void:
	var body := StaticBody3D.new()
	body.name = source_name + "_Collision"
	body.transform = source_transform
	body.set_meta("osm_generated", true)
	var shape := CollisionShape3D.new()
	shape.shape = mesh.create_trimesh_shape()
	body.add_child(shape)
	add_child(body)
	body.owner = get_tree().edited_scene_root if Engine.is_editor_hint() else owner
	shape.owner = body.owner

func _apply_metadata(node: Node, metadata: Variant) -> void:
	if typeof(metadata) != TYPE_DICTIONARY:
		return
	node.set_meta("osm_metadata", metadata)
	for key in metadata.keys():
		node.set_meta(StringName(_sanitize_meta_key(str(key))), metadata[key])

func _sanitize_meta_key(key: String) -> String:
	var out := key
	out = out.replace(":", "_")
	out = out.replace("-", "_")
	out = out.replace(".", "_")
	out = out.replace(" ", "_")
	return out

func _should_add_collision(material_name: String) -> bool:
	return (
		material_name.begins_with("terrain_")
	)

func _to_transform(values: Array) -> Transform3D:
	if values.size() < 12:
		return Transform3D.IDENTITY
	return Transform3D(Basis(
		Vector3(float(values[0]), float(values[1]), float(values[2])),
		Vector3(float(values[3]), float(values[4]), float(values[5])),
		Vector3(float(values[6]), float(values[7]), float(values[8]))
	), Vector3(float(values[9]), float(values[10]), float(values[11])))
"#
            .as_bytes(),
        )?;

        Ok(())
    }

    fn write_navigation_controller_script(
        &self,
        scripts_dir: &std::path::Path,
    ) -> std::io::Result<()> {
        use std::io::Write;

        let path = scripts_dir.join("navigation_controller.gd");
        let mut f = std::fs::File::create(&path)?;

        f.write_all(
            r#"extends Node3D

@export var player_path: NodePath = NodePath("../Player")
@export var index_path := "res://navigation_index.json"
@export var graph_path := "res://navigation_graph.json"
@export var panel_toggle_key := KEY_N
@export var guidance_update_interval := 0.2
@export var route_ribbon_width := 2.4
@export var arrival_radius := 5.0

var player: Node3D = null
var navigation_entries := []
var graph_nodes := []
var graph_edges := []
var adjacency := {}
var node_positions := {}
var route_waypoints := []
var route_total_distance := 0.0
var current_instruction := ""
var navigation_status := "idle"
var destination_name := ""
var destination_position := Vector3.ZERO
var guidance_update_accumulator := 0.0

var hud_layer: CanvasLayer = null
var panel: PanelContainer = null
var search_box: LineEdit = null
var result_list: ItemList = null
var cancel_navigation_button: Button = null
var start_navigation_button: Button = null
var instruction_label: Label = null
var distance_label: Label = null
var route_overlay: Node3D = null
var route_line: MeshInstance3D = null
var destination_circle: MeshInstance3D = null
var route_ribbon_material: StandardMaterial3D = null
var destination_circle_material: StandardMaterial3D = null
var navigation_start_in_progress := false

func _ready() -> void:
	player = get_node_or_null(player_path) as Node3D
	_load_navigation_data()
	_build_runtime_nodes()
	_build_hud()
	_build_route_overlay()
	set_process(true)

func _input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == panel_toggle_key:
		_toggle_panel()
	if panel != null and panel.visible and event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_ENTER:
		_start_from_ui_selection()

func _process(delta: float) -> void:
	if route_waypoints.is_empty():
		return
	guidance_update_accumulator += delta
	if guidance_update_accumulator < guidance_update_interval:
		return
	guidance_update_accumulator = 0.0
	_update_guidance()

func start_navigation_to_query(query: String) -> bool:
	if player == null:
		player = get_node_or_null(player_path) as Node3D
	var matches := search_destinations(query, 8)
	if matches.is_empty() and query == "外滩":
		matches = search_destinations("外滩源", 8)
	if matches.is_empty():
		navigation_status = "destination_not_found"
		current_instruction = "Destination not found"
		_update_hud()
		return false
	return _start_navigation_to_entry(matches[0])

func search_destinations(query: String, limit := 12) -> Array:
	var results := []
	var needle := query.strip_edges().to_lower()
	if needle.is_empty():
		return results
	for entry in navigation_entries:
		if typeof(entry) != TYPE_DICTIONARY:
			continue
		if _entry_matches(entry, needle):
			results.append(entry)
			if results.size() >= limit:
				break
	return results

func get_route_waypoint_count() -> int:
	return route_waypoints.size()

func get_current_instruction() -> String:
	return current_instruction

func get_navigation_status() -> String:
	return navigation_status

func get_route_total_distance() -> float:
	return route_total_distance

func get_graph_node_count() -> int:
	return graph_nodes.size()

func get_graph_edge_count() -> int:
	return graph_edges.size()

func _load_navigation_data() -> void:
	navigation_entries = _load_json_array(index_path, "entries")
	var graph := _load_json_dict(graph_path)
	graph_nodes = graph.get("nodes", [])
	graph_edges = graph.get("edges", [])

func _load_json_dict(path: String) -> Dictionary:
	var file := FileAccess.open(path, FileAccess.READ)
	if file == null:
		push_error("Failed to open local navigation data: " + path)
		return {}
	var parsed: Variant = JSON.parse_string(file.get_as_text())
	if typeof(parsed) != TYPE_DICTIONARY:
		push_error("Invalid local navigation data: " + path)
		return {}
	return parsed

func _load_json_array(path: String, key: String) -> Array:
	var data := _load_json_dict(path)
	var value: Variant = data.get(key, [])
	if typeof(value) != TYPE_ARRAY:
		return []
	return value

func _build_runtime_nodes() -> void:
	adjacency.clear()
	node_positions.clear()
	for node in graph_nodes:
		if typeof(node) != TYPE_DICTIONARY:
			continue
		var id := str(node.get("id", ""))
		var pos: Array = node.get("position", [])
		if id.is_empty() or pos.size() < 2:
			continue
		node_positions[id] = Vector3(float(pos[0]), 0.2, float(pos[1]))
		adjacency[id] = []
	for edge in graph_edges:
		if typeof(edge) != TYPE_DICTIONARY:
			continue
		var from_id := str(edge.get("from", ""))
		var to_id := str(edge.get("to", ""))
		if from_id.is_empty() or to_id.is_empty() or not adjacency.has(from_id):
			continue
		adjacency[from_id].append({
			"to": to_id,
			"cost": max(0.001, float(edge.get("cost", 1.0))),
			"name": str(edge.get("name", "")),
			"highway": str(edge.get("highway", "")),
		})

func _entry_matches(entry: Dictionary, needle: String) -> bool:
	for key in ["name", "official_name", "alt_name", "old_name", "building", "highway", "amenity", "shop", "tourism", "addr:street"]:
		if str(entry.get(key, "")).to_lower().contains(needle):
			return true
	return str(entry.get("osm_id", "")).contains(needle)

func _start_navigation_to_entry(entry: Dictionary) -> bool:
	if node_positions.is_empty() or adjacency.is_empty():
		navigation_status = "graph_empty"
		current_instruction = "Navigation graph is empty"
		_update_hud()
		return false
	if player == null:
		navigation_status = "player_not_found"
		current_instruction = "Player not found"
		_update_hud()
		return false
	var center: Array = entry.get("center", [])
	if center.size() < 2:
		navigation_status = "destination_missing_center"
		current_instruction = "Destination has no center"
		_update_hud()
		return false
	var start_candidates := _nearest_graph_nodes(player.global_position, 8, 80.0)
	var goal_candidates := _nearest_graph_nodes(Vector3(float(center[0]), 0.0, float(center[1])), 16, 140.0)
	if start_candidates.is_empty() or goal_candidates.is_empty():
		navigation_status = "snap_failed"
		current_instruction = "Could not snap to road"
		_update_hud()
		return false
	var node_path := _find_best_route(start_candidates, goal_candidates)
	if node_path.size() < 2:
		navigation_status = "route_not_found"
		current_instruction = "Route not found"
		_update_hud()
		return false
	route_waypoints.clear()
	for node_id in node_path:
		route_waypoints.append(node_positions[str(node_id)])
	route_total_distance = _route_distance(route_waypoints)
	destination_name = _entry_display_name(entry)
	destination_position = route_waypoints.back() as Vector3
	navigation_status = "routing"
	current_instruction = "沿绿色路线行驶"
	if instruction_label != null:
		instruction_label.visible = true
	if distance_label != null:
		distance_label.visible = true
	guidance_update_accumulator = guidance_update_interval
	_draw_route_ribbon()
	_draw_destination_circle()
	_update_guidance()
	return true

func _nearest_graph_node(pos: Vector3) -> String:
	var best_id := ""
	var best_dist := INF
	for id in node_positions.keys():
		var node_pos: Vector3 = node_positions[id]
		var dx := node_pos.x - pos.x
		var dz := node_pos.z - pos.z
		var dist := dx * dx + dz * dz
		if dist < best_dist:
			best_dist = dist
			best_id = str(id)
	return best_id

func _nearest_graph_nodes(pos: Vector3, limit: int, max_distance: float) -> Array:
	var remaining := []
	for id in node_positions.keys():
		var node_pos: Vector3 = node_positions[id]
		var distance := Vector2(node_pos.x - pos.x, node_pos.z - pos.z).length()
		if distance <= max_distance:
			remaining.append({"id": str(id), "distance": distance})
	if remaining.is_empty():
		var fallback := _nearest_graph_node(pos)
		if not fallback.is_empty():
			remaining.append({"id": fallback, "distance": 0.0})
	var results := []
	while results.size() < limit and not remaining.is_empty():
		var best_index := 0
		var best_distance := float(remaining[0].get("distance", INF))
		for i in range(1, remaining.size()):
			var distance := float(remaining[i].get("distance", INF))
			if distance < best_distance:
				best_distance = distance
				best_index = i
		results.append(remaining[best_index])
		remaining.remove_at(best_index)
	return results

func _find_best_route(start_candidates: Array, goal_candidates: Array) -> Array:
	var best_path := []
	var best_cost := INF
	for start in start_candidates:
		var start_id := str(start.get("id", ""))
		if start_id.is_empty():
			continue
		for goal in goal_candidates:
			var goal_id := str(goal.get("id", ""))
			if goal_id.is_empty():
				continue
			var path := _find_route(start_id, goal_id)
			if path.size() < 2:
				continue
			var cost := _path_cost(path) + float(start.get("distance", 0.0)) + float(goal.get("distance", 0.0))
			if cost < best_cost:
				best_cost = cost
				best_path = path
	return best_path

func _find_route(start_id: String, goal_id: String) -> Array:
	var open_set := [start_id]
	var came_from := {}
	var g_score := {start_id: 0.0}
	var f_score := {start_id: _heuristic(start_id, goal_id)}
	while not open_set.is_empty():
		var current := _lowest_score_node(open_set, f_score)
		if current == goal_id:
			return _reconstruct_path(came_from, current)
		open_set.erase(current)
		for edge in adjacency.get(current, []):
			var next_id := str(edge.get("to", ""))
			if next_id.is_empty() or not node_positions.has(next_id):
				continue
			var tentative := float(g_score.get(current, INF)) + float(edge.get("cost", 1.0))
			if tentative < float(g_score.get(next_id, INF)):
				came_from[next_id] = current
				g_score[next_id] = tentative
				f_score[next_id] = tentative + _heuristic(next_id, goal_id)
				if not open_set.has(next_id):
					open_set.append(next_id)
	return []

func _path_cost(path: Array) -> float:
	var total := 0.0
	for i in range(1, path.size()):
		var a := str(path[i - 1])
		var b := str(path[i])
		if node_positions.has(a) and node_positions.has(b):
			total += node_positions[a].distance_to(node_positions[b])
	return total

func _lowest_score_node(open_set: Array, f_score: Dictionary) -> String:
	var best := str(open_set[0])
	var best_score := float(f_score.get(best, INF))
	for value in open_set:
		var id := str(value)
		var score := float(f_score.get(id, INF))
		if score < best_score:
			best = id
			best_score = score
	return best

func _heuristic(a: String, b: String) -> float:
	if not node_positions.has(a) or not node_positions.has(b):
		return 0.0
	return node_positions[a].distance_to(node_positions[b])

func _reconstruct_path(came_from: Dictionary, current: String) -> Array:
	var total := [current]
	while came_from.has(current):
		current = str(came_from[current])
		total.push_front(current)
	return total

func _route_distance(points: Array) -> float:
	var total := 0.0
	for i in range(1, points.size()):
		total += (points[i - 1] as Vector3).distance_to(points[i] as Vector3)
	return total

func _instruction_for_segment(index: int) -> String:
	if route_waypoints.size() < 2:
		return ""
	return "沿绿色路线行驶"

func _update_guidance() -> void:
	if player == null or route_waypoints.size() < 2:
		return
	if player.global_position.distance_to(destination_position) <= arrival_radius:
		_complete_navigation()
		return
	current_instruction = "沿绿色路线行驶"
	_update_hud()

func _complete_navigation() -> void:
	navigation_status = "arrived"
	route_waypoints.clear()
	route_total_distance = 0.0
	current_instruction = ""
	destination_name = ""
	if route_line != null:
		route_line.visible = false
		route_line.mesh = null
	if destination_circle != null:
		destination_circle.visible = false
	if instruction_label != null:
		instruction_label.visible = false
	if distance_label != null:
		distance_label.visible = false

func _build_hud() -> void:
	hud_layer = CanvasLayer.new()
	hud_layer.name = "NavigationHUD"
	add_child(hud_layer)
	panel = PanelContainer.new()
	panel.name = "NavigationPanel"
	panel.visible = false
	panel.anchor_left = 0.5
	panel.anchor_top = 0.5
	panel.anchor_right = 0.5
	panel.anchor_bottom = 0.5
	panel.offset_left = -220.0
	panel.offset_top = -170.0
	panel.offset_right = 220.0
	panel.offset_bottom = 170.0
	hud_layer.add_child(panel)
	var box := VBoxContainer.new()
	box.custom_minimum_size = Vector2(420, 320)
	panel.add_child(box)
	search_box = LineEdit.new()
	search_box.placeholder_text = "搜索目的地"
	box.add_child(search_box)
	result_list = ItemList.new()
	result_list.custom_minimum_size = Vector2(420, 220)
	box.add_child(result_list)
	var buttons := HBoxContainer.new()
	box.add_child(buttons)
	cancel_navigation_button = Button.new()
	cancel_navigation_button.name = "CancelNavigationButton"
	cancel_navigation_button.text = "取消"
	cancel_navigation_button.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	buttons.add_child(cancel_navigation_button)
	start_navigation_button = Button.new()
	start_navigation_button.name = "StartNavigationButton"
	start_navigation_button.text = "开始导航"
	start_navigation_button.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	buttons.add_child(start_navigation_button)
	instruction_label = Label.new()
	instruction_label.name = "InstructionLabel"
	instruction_label.position = Vector2(82, 24)
	instruction_label.add_theme_font_size_override("font_size", 24)
	instruction_label.text = "导航就绪"
	hud_layer.add_child(instruction_label)
	distance_label = Label.new()
	distance_label.name = "DistanceLabel"
	distance_label.position = Vector2(0, 24)
	hud_layer.add_child(distance_label)
	search_box.text_changed.connect(_on_search_changed)
	cancel_navigation_button.pressed.connect(_cancel_navigation_panel)
	start_navigation_button.pressed.connect(_start_from_ui_selection)

func _build_route_overlay() -> void:
	route_overlay = Node3D.new()
	route_overlay.name = "RouteOverlay"
	add_child(route_overlay)
	route_line = MeshInstance3D.new()
	route_line.name = "RouteRibbon"
	route_overlay.add_child(route_line)
	destination_circle = MeshInstance3D.new()
	destination_circle.name = "DestinationCircle"
	destination_circle.visible = false
	route_overlay.add_child(destination_circle)
	route_ribbon_material = StandardMaterial3D.new()
	route_ribbon_material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	route_ribbon_material.albedo_color = Color(0.0, 1.0, 0.18, 1.0)
	route_ribbon_material.emission_enabled = true
	route_ribbon_material.emission = Color(0.0, 1.0, 0.18, 1.0)
	route_ribbon_material.emission_energy_multiplier = 2.4
	route_ribbon_material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	route_ribbon_material.cull_mode = BaseMaterial3D.CULL_DISABLED
	route_ribbon_material.no_depth_test = true
	destination_circle_material = StandardMaterial3D.new()
	destination_circle_material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	destination_circle_material.albedo_color = Color(0.0, 1.0, 0.18, 0.42)
	destination_circle_material.emission_enabled = true
	destination_circle_material.emission = Color(0.0, 1.0, 0.18, 1.0)
	destination_circle_material.emission_energy_multiplier = 2.8
	destination_circle_material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	destination_circle_material.cull_mode = BaseMaterial3D.CULL_DISABLED
	destination_circle_material.no_depth_test = true

func _toggle_panel() -> void:
	if panel == null:
		return
	_set_panel_visible(not panel.visible)

func _set_panel_visible(visible: bool) -> void:
	if panel == null:
		return
	panel.visible = visible
	if visible:
		navigation_start_in_progress = false
		if start_navigation_button != null:
			start_navigation_button.disabled = false
		_set_player_controls_enabled(false)
		Input.set_mouse_mode(Input.MOUSE_MODE_VISIBLE)
		search_box.grab_focus()
	else:
		_set_player_controls_enabled(true)
		Input.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)

func _set_player_controls_enabled(enabled: bool) -> void:
	if player == null:
		player = get_node_or_null(player_path) as Node3D
	if player != null and player.has_method("set_controls_enabled"):
		player.call("set_controls_enabled", enabled)

func _cancel_navigation_panel() -> void:
	_set_panel_visible(false)

func _on_search_changed(text: String) -> void:
	if result_list == null:
		return
	result_list.clear()
	for entry in search_destinations(text, 12):
		result_list.add_item(_entry_display_name(entry))
		result_list.set_item_metadata(result_list.item_count - 1, entry)

func _start_from_ui_selection() -> void:
	if navigation_start_in_progress:
		return
	navigation_start_in_progress = true
	if start_navigation_button != null:
		start_navigation_button.disabled = true
	var started := false
	if result_list == null or result_list.item_count == 0:
		if search_box != null:
			started = start_navigation_to_query(search_box.text)
		_finish_navigation_start(started)
		return
	var selected := result_list.get_selected_items()
	var index := int(selected[0]) if selected.size() > 0 else 0
	var entry = result_list.get_item_metadata(index)
	if typeof(entry) == TYPE_DICTIONARY:
		started = _start_navigation_to_entry(entry)
	_finish_navigation_start(started)

func _finish_navigation_start(started: bool) -> void:
	navigation_start_in_progress = false
	if started:
		_set_panel_visible(false)
	elif start_navigation_button != null:
		start_navigation_button.disabled = false

func _entry_display_name(entry: Dictionary) -> String:
	for key in ["name", "official_name", "alt_name", "addr:street", "building", "highway"]:
		var value := str(entry.get(key, ""))
		if not value.is_empty():
			return value
	return str(entry.get("osm_kind", "destination")) + " " + str(entry.get("osm_id", ""))

func _draw_route_ribbon() -> void:
	if route_line == null:
		return
	var mesh := ImmediateMesh.new()
	mesh.surface_begin(Mesh.PRIMITIVE_TRIANGLES)
	for i in range(1, route_waypoints.size()):
		var a: Vector3 = route_waypoints[i - 1]
		var b: Vector3 = route_waypoints[i]
		_draw_route_ribbon_segment(mesh, a, b)
	mesh.surface_end()
	mesh.surface_set_material(0, route_ribbon_material)
	route_line.mesh = mesh
	route_line.visible = true

func _draw_destination_circle() -> void:
	if destination_circle == null:
		return
	destination_circle.mesh = _make_destination_circle_mesh(arrival_radius, 0.65)
	destination_circle.material_override = destination_circle_material
	destination_circle.position = Vector3(destination_position.x, 0.58, destination_position.z)
	destination_circle.visible = true

func _make_destination_circle_mesh(radius: float, thickness: float) -> Mesh:
	var mesh := ImmediateMesh.new()
	var segments: int = 72
	var inner_radius: float = max(0.1, radius - thickness)
	mesh.surface_begin(Mesh.PRIMITIVE_TRIANGLES)
	for i in range(segments):
		var a0: float = TAU * float(i) / float(segments)
		var a1: float = TAU * float(i + 1) / float(segments)
		var outer0: Vector3 = Vector3(cos(a0) * radius, 0.0, sin(a0) * radius)
		var inner0: Vector3 = Vector3(cos(a0) * inner_radius, 0.0, sin(a0) * inner_radius)
		var outer1: Vector3 = Vector3(cos(a1) * radius, 0.0, sin(a1) * radius)
		var inner1: Vector3 = Vector3(cos(a1) * inner_radius, 0.0, sin(a1) * inner_radius)
		mesh.surface_add_vertex(outer0)
		mesh.surface_add_vertex(outer1)
		mesh.surface_add_vertex(inner1)
		mesh.surface_add_vertex(outer0)
		mesh.surface_add_vertex(inner1)
		mesh.surface_add_vertex(inner0)
	mesh.surface_end()
	return mesh

func _draw_route_ribbon_segment(mesh: ImmediateMesh, a: Vector3, b: Vector3) -> void:
	var delta := Vector2(b.x - a.x, b.z - a.z)
	if delta.length() < 0.01:
		return
	var normal := Vector2(-delta.y, delta.x).normalized() * (route_ribbon_width * 0.5)
	var y := 0.42
	var a_left := Vector3(a.x + normal.x, y, a.z + normal.y)
	var a_right := Vector3(a.x - normal.x, y, a.z - normal.y)
	var b_left := Vector3(b.x + normal.x, y, b.z + normal.y)
	var b_right := Vector3(b.x - normal.x, y, b.z - normal.y)
	mesh.surface_add_vertex(a_left)
	mesh.surface_add_vertex(b_left)
	mesh.surface_add_vertex(b_right)
	mesh.surface_add_vertex(a_left)
	mesh.surface_add_vertex(b_right)
	mesh.surface_add_vertex(a_right)

func _update_hud() -> void:
	if instruction_label != null:
		instruction_label.text = current_instruction
	if distance_label != null:
		if route_total_distance > 0.0:
			distance_label.text = destination_name + " " + str(int(route_total_distance)) + " m"
		else:
			distance_label.text = navigation_status

"#
            .as_bytes(),
        )?;

        Ok(())
    }

    fn write_world_streamer_script(&self, scripts_dir: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;

        let path = scripts_dir.join("world_streamer.gd");
        let mut f = std::fs::File::create(&path)?;

        let script = format!(
            r#"extends Node3D

@export var manifest_path := "res://world_manifest.json"
@export var player_path: NodePath = NodePath("../Player")
@export var stream_radius := {stream_radius}
@export var unload_radius := {unload_radius}
@export var max_concurrent_chunk_loads := 2

var manifest := {{}}
var chunk_entries := {{}}
var loaded_chunks := {{}}
var pending_chunk_keys := []
var pending_chunk_lookup := {{}}
var loading_chunk_keys := {{}}
var player: Node3D = null
var chunk_size_blocks := 1.0
var godot_scale := 1.0
var last_player_chunk_key := ""

func _ready() -> void:
	_load_manifest()
	player = get_node_or_null(player_path) as Node3D
	_refresh_streaming()

func _physics_process(_delta: float) -> void:
	_refresh_streaming()

func _process(_delta: float) -> void:
	_drain_load_queue()

func _load_manifest() -> void:
	var file := FileAccess.open(manifest_path, FileAccess.READ)
	if file == null:
		push_error("Failed to open world manifest: " + manifest_path)
		return
	var parsed = JSON.parse_string(file.get_as_text())
	if typeof(parsed) != TYPE_DICTIONARY:
		push_error("Invalid world manifest: " + manifest_path)
		return
	manifest = parsed
	chunk_size_blocks = max(1.0, float(manifest.get("chunk_size_blocks", 1)))
	godot_scale = max(0.001, float(manifest.get("godot_scale", 1.0)))
	chunk_entries.clear()
	for entry in manifest.get("chunks", []):
		var coord: Array = entry.get("coord", [])
		if coord.size() < 2:
			continue
		chunk_entries[_chunk_key(int(coord[0]), int(coord[1]))] = entry

func _refresh_streaming() -> void:
	if player == null:
		player = get_node_or_null(player_path) as Node3D
	if player == null or chunk_entries.is_empty():
		return
	var current: Array = _find_player_chunk(player.global_position)
	if current.size() < 2:
		return
	var current_key := _chunk_key(int(current[0]), int(current[1]))
	if current_key == last_player_chunk_key and not loaded_chunks.is_empty():
		return
	last_player_chunk_key = current_key
	var keep: Dictionary = {{}}
	for dx in range(-stream_radius, stream_radius + 1):
		for dz in range(-stream_radius, stream_radius + 1):
			var cx := int(current[0]) + dx
			var cz := int(current[1]) + dz
			var key := _chunk_key(cx, cz)
			if chunk_entries.has(key):
				keep[key] = true
				_request_chunk(key)
	_unload_far_chunks(current, keep)
	_drain_load_queue()

func _request_chunk(key: String) -> void:
	if loaded_chunks.has(key) or pending_chunk_lookup.has(key):
		return
	pending_chunk_lookup[key] = true
	pending_chunk_keys.append(key)

func _drain_load_queue() -> void:
	_prune_finished_loading()
	while loading_chunk_keys.size() < max_concurrent_chunk_loads and not pending_chunk_keys.is_empty():
		var key := str(pending_chunk_keys.pop_front())
		pending_chunk_lookup.erase(key)
		if loaded_chunks.has(key) or not chunk_entries.has(key):
			continue
		_start_chunk_load(key)

func _start_chunk_load(key: String) -> void:
	var entry: Dictionary = chunk_entries[key]
	var packed := load(str(entry.get("scene_path", ""))) as PackedScene
	if packed == null:
		push_error("Failed to load chunk scene: " + str(entry.get("scene_path", "")))
		return
	var instance := packed.instantiate() as Node3D
	var origin: Array = entry.get("origin", [0.0, 0.0])
	instance.position = Vector3(float(origin[0]), 0.0, float(origin[1]))
	instance.set_meta("chunk_key", key)
	instance.set_meta("streamed_chunk", true)
	add_child(instance)
	loaded_chunks[key] = instance
	loading_chunk_keys[key] = true

func _prune_finished_loading() -> void:
	for key in loading_chunk_keys.keys():
		if not loaded_chunks.has(key):
			loading_chunk_keys.erase(key)
			continue
		var node: Node = loaded_chunks[key]
		if node.get_meta("chunk_loading_complete", false):
			loading_chunk_keys.erase(key)

func _unload_far_chunks(current: Array, keep: Dictionary) -> void:
	_prune_pending_queue(keep)
	for key in loaded_chunks.keys():
		if keep.has(key):
			continue
		if loading_chunk_keys.has(key):
			continue
		var coord: Array = _parse_chunk_key(key)
		if coord.size() < 2:
			continue
		var dist: int = max(abs(int(coord[0]) - int(current[0])), abs(int(coord[1]) - int(current[1])))
		if dist > unload_radius:
			var node: Node = loaded_chunks[key]
			loaded_chunks.erase(key)
			node.queue_free()

func _prune_pending_queue(keep: Dictionary) -> void:
	var next_pending := []
	pending_chunk_lookup.clear()
	for key in pending_chunk_keys:
		var key_string := str(key)
		if keep.has(key_string):
			next_pending.append(key_string)
			pending_chunk_lookup[key_string] = true
	pending_chunk_keys = next_pending

func _find_player_chunk(pos: Vector3) -> Array:
	var coord := _coord_from_position(pos)
	var key := _chunk_key(int(coord[0]), int(coord[1]))
	if chunk_entries.has(key):
		return coord
	var search_radius: int = max(stream_radius, unload_radius) + 2
	for dx in range(-search_radius, search_radius + 1):
		for dz in range(-search_radius, search_radius + 1):
			var nearby_key := _chunk_key(int(coord[0]) + dx, int(coord[1]) + dz)
			if chunk_entries.has(nearby_key):
				return chunk_entries[nearby_key].get("coord", [])
	return []

func _coord_from_position(pos: Vector3) -> Array:
	var block_x := pos.x / godot_scale
	var block_z := -pos.z / godot_scale
	return [int(floor(block_x / chunk_size_blocks)), int(floor(block_z / chunk_size_blocks))]

func _chunk_key(cx: int, cz: int) -> String:
	return str(cx) + ":" + str(cz)

func _parse_chunk_key(key: String) -> Array:
	var parts := key.split(":")
	if parts.size() < 2:
		return []
	return [int(parts[0]), int(parts[1])]
"#,
            stream_radius = self.stream_radius.max(0),
            unload_radius = self.stream_radius.max(0) + 1
        );
        f.write_all(script.as_bytes())?;

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
    use crate::element_processing::highways;
    use crate::ground::Ground;
    use crate::osm_parser::{ProcessedNode, ProcessedWay};
    use crate::scene_writer::geometry::MeshData;
    use std::collections::HashMap;

    fn unit_mesh() -> MeshData {
        let mut mesh = MeshData::new();
        mesh.vertices
            .extend_from_slice(&[0.0, 0.0, 0.0, 2.0, 2.0, 2.0]);
        mesh
    }

    fn test_road_way(id: u64) -> ProcessedWay {
        let mut tags = HashMap::new();
        tags.insert("highway".to_string(), "primary".to_string());
        tags.insert("name".to_string(), "Navigation Test Road".to_string());
        ProcessedWay {
            id,
            tags,
            nodes: vec![
                ProcessedNode {
                    id: 1,
                    tags: HashMap::new(),
                    x: 32,
                    z: 32,
                },
                ProcessedNode {
                    id: 2,
                    tags: HashMap::new(),
                    x: 160,
                    z: 32,
                },
                ProcessedNode {
                    id: 3,
                    tags: HashMap::new(),
                    x: 160,
                    z: 160,
                },
            ],
        }
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
        let collision_fn = script
            .split("func _should_add_collision(material_name: String) -> bool:")
            .nth(1)
            .unwrap();
        assert!(!collision_fn.contains("material_name.begins_with(\"road_\")"));
        assert!(!collision_fn.contains("material_name == \"railway_gravel\""));
        assert!(!collision_fn.contains("material_name == \"building_wall\""));
    }

    #[test]
    fn chunk_loader_batches_meshes_by_material_and_keeps_metadata_markers() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let script =
            std::fs::read_to_string(tmp.path().join("scripts").join("chunk_mesh_loader.gd"))
                .unwrap();
        assert!(script.contains("func _add_element_to_batch"));
        assert!(script.contains("func _create_batch_instance"));
        assert!(script.contains("func _add_metadata_marker"));
        assert!(script.contains("batch_element_count"));
        assert!(script.contains("PackedVector3Array(batch[\"vertices\"])"));
        assert!(!script.contains("func _add_mesh_instance"));
    }

    #[test]
    fn chunk_loader_uses_threaded_incremental_loading_budget() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let script =
            std::fs::read_to_string(tmp.path().join("scripts").join("chunk_mesh_loader.gd"))
                .unwrap();
        assert!(script.contains("@export var load_budget_usec"));
        assert!(script.contains("Thread.new()"));
        assert!(script.contains("func _load_mesh_data_thread() -> Dictionary:"));
        assert!(script.contains("func _process(_delta: float) -> void:"));
        assert!(script.contains("Time.get_ticks_usec() - start >= load_budget_usec"));
        assert!(script.contains("set_meta(\"chunk_loading_complete\", true)"));
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
        assert!(script.contains("func _start_chunk_load"));
        assert!(script.contains("func _unload_far_chunks"));
        assert!(script.contains("world_manifest.json"));
        assert!(script.contains("loaded_chunks"));
        assert!(script.contains("last_player_chunk_key"));
        assert!(script.contains("func _coord_from_position(pos: Vector3) -> Array:"));
        assert!(script.contains("chunk_size_blocks"));
        assert!(script.contains("@export var max_concurrent_chunk_loads"));
        assert!(script.contains("var pending_chunk_keys := []"));
        assert!(script.contains("func _request_chunk(key: String) -> void:"));
        assert!(script.contains("func _drain_load_queue() -> void:"));
        assert!(!script.contains("for key in chunk_entries.keys():"));
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

    #[test]
    fn navigation_graph_is_written_from_highway_centerlines() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let mut scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 128, 0.5);

        highways::generate_highway(&mut scene, &test_road_way(300), 0.5);
        scene.save_all().unwrap();

        let graph_path = tmp.path().join("navigation_graph.json");
        assert!(graph_path.exists(), "navigation_graph.json should be generated");
        let graph: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(graph_path).unwrap()).unwrap();
        let nodes = graph["nodes"].as_array().unwrap();
        let edges = graph["edges"].as_array().unwrap();
        assert!(nodes.len() >= 3, "expected centerline nodes, got {nodes:?}");
        assert!(
            edges.len() >= 4,
            "expected bidirectional road edges, got {edges:?}"
        );
        assert!(edges.iter().any(|edge| edge["osm_id"] == "300"
            && edge["highway"] == "primary"
            && edge["name"] == "Navigation Test Road"
            && edge["cost"].as_f64().unwrap_or_default() > 0.0));
    }

    #[test]
    fn master_scene_mounts_navigation_controller() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 128, 0.5);

        scene.save_all().unwrap();

        let master =
            std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        assert!(master.contains(
            "[ext_resource type=\"Script\" path=\"res://scripts/navigation_controller.gd\" id=\"navigation_script\"]"
        ));
        assert!(master.contains(
            "[node name=\"NavigationController\" type=\"Node3D\" parent=\".\"]"
        ));
        assert!(master.contains("script = ExtResource(\"navigation_script\")"));
        assert!(master.contains("player_path = NodePath(\"../Player\")"));
        assert!(tmp
            .path()
            .join("scripts")
            .join("navigation_controller.gd")
            .exists());
    }

    #[test]
    fn navigation_controller_uses_local_graph_and_has_no_network_api() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 128, 0.5);

        scene.save_all().unwrap();

        let script = std::fs::read_to_string(
            tmp.path().join("scripts").join("navigation_controller.gd"),
        )
        .unwrap();
        assert!(script.contains("func start_navigation_to_query(query: String) -> bool:"));
        assert!(script.contains("navigation_index.json"));
        assert!(script.contains("navigation_graph.json"));
        assert!(script.contains("func _find_route"));
        assert!(script.contains("func _nearest_graph_node"));
        assert!(!script.contains("DisplayServer.tts_speak"));
        for forbidden in ["HTTPRequest", "HTTPClient", "WebSocketPeer", "https://", "http://"] {
            assert!(
                !script.contains(forbidden),
                "navigation must stay local and must not contain {forbidden}"
            );
        }
    }

    #[test]
    fn navigation_controller_uses_only_bright_green_route_ribbon() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 128, 0.5);

        scene.save_all().unwrap();

        let script = std::fs::read_to_string(
            tmp.path().join("scripts").join("navigation_controller.gd"),
        )
        .unwrap();
        assert!(script.contains("@export var route_ribbon_width := 2.4"));
        assert!(script.contains("route_ribbon_material.albedo_color = Color(0.0, 1.0, 0.18, 1.0)"));
        assert!(script.contains("route_ribbon_material.emission_enabled = true"));
        assert!(script.contains("route_ribbon_material.no_depth_test = true"));
        assert!(script.contains("func _draw_route_ribbon() -> void:"));
        assert!(script.contains("@export var arrival_radius := 5.0"));
        assert!(script.contains("destination_circle.name = \"DestinationCircle\""));
        assert!(script.contains("func _draw_destination_circle() -> void:"));
        assert!(script.contains("func _make_destination_circle_mesh(radius: float, thickness: float) -> Mesh:"));
        assert!(!script.contains("TurnMarkers"));
        assert!(!script.contains("func _draw_turn_markers() -> void:"));
        assert!(!script.contains("func _maybe_queue_voice_for_maneuver"));
        assert!(!script.contains("DisplayServer.tts_speak"));
    }

    #[test]
    fn navigation_guidance_does_not_use_markers_or_voice_prompts() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 128, 0.5);

        scene.save_all().unwrap();

        let script = std::fs::read_to_string(
            tmp.path().join("scripts").join("navigation_controller.gd"),
        )
        .unwrap();
        assert!(!script.contains("turn_marker_root"));
        assert!(!script.contains("turn_arrow_meshes"));
        assert!(!script.contains("spoken_maneuver_alerts"));

        let update_guidance_start = script.find("func _update_guidance() -> void:").unwrap();
        let build_hud_start = script.find("func _build_hud() -> void:").unwrap();
        let update_guidance = &script[update_guidance_start..build_hud_start];
        assert!(update_guidance.contains("current_instruction = \"沿绿色路线行驶\""));
        assert!(!update_guidance.contains("_maybe_queue_voice_for_maneuver"));
        assert!(!update_guidance.contains("_queue_voice_instruction"));
    }

    #[test]
    fn navigation_arrival_circle_clears_all_guidance() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 128, 0.5);

        scene.save_all().unwrap();

        let script = std::fs::read_to_string(
            tmp.path().join("scripts").join("navigation_controller.gd"),
        )
        .unwrap();
        assert!(script.contains("if player.global_position.distance_to(destination_position) <= arrival_radius:"));
        assert!(script.contains("_complete_navigation()"));
        assert!(script.contains("func _complete_navigation() -> void:"));
        assert!(script.contains("route_waypoints.clear()"));
        assert!(script.contains("route_line.visible = false"));
        assert!(script.contains("destination_circle.visible = false"));
        assert!(script.contains("instruction_label.visible = false"));
        assert!(script.contains("distance_label.visible = false"));
    }

    #[test]
    fn navigation_panel_is_centered_with_cancel_and_start_actions() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 128, 0.5);

        scene.save_all().unwrap();

        let script = std::fs::read_to_string(
            tmp.path().join("scripts").join("navigation_controller.gd"),
        )
        .unwrap();
        assert!(script.contains("panel.anchor_left = 0.5"));
        assert!(script.contains("panel.anchor_top = 0.5"));
        assert!(script.contains("panel.offset_left = -220.0"));
        assert!(script.contains("search_box.placeholder_text = \"搜索目的地\""));
        assert!(script.contains("cancel_navigation_button.name = \"CancelNavigationButton\""));
        assert!(script.contains("cancel_navigation_button.text = \"取消\""));
        assert!(script.contains("start_navigation_button.name = \"StartNavigationButton\""));
        assert!(script.contains("start_navigation_button.text = \"开始导航\""));
        assert!(script.contains("start_navigation_button.disabled = true"));
        assert!(script.contains("navigation_start_in_progress"));
        assert!(script.contains("cancel_navigation_button.pressed.connect(_cancel_navigation_panel)"));
    }

    #[test]
    fn navigation_panel_disables_fps_controls_while_open() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 128, 0.5);

        scene.save_all().unwrap();

        let player_script =
            std::fs::read_to_string(tmp.path().join("scripts").join("fps_player.gd")).unwrap();
        let navigation_script = std::fs::read_to_string(
            tmp.path().join("scripts").join("navigation_controller.gd"),
        )
        .unwrap();
        assert!(player_script.contains("var controls_enabled := true"));
        assert!(player_script.contains("func set_controls_enabled(enabled: bool) -> void:"));
        assert!(player_script.contains("if not controls_enabled:"));
        assert!(navigation_script.contains("_set_player_controls_enabled(false)"));
        assert!(navigation_script.contains("_set_player_controls_enabled(true)"));
        assert!(navigation_script.contains("player.call(\"set_controls_enabled\", enabled)"));
    }

    #[test]
    fn navigation_guidance_is_throttled_and_route_visuals_are_static() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 128, 0.5);

        scene.save_all().unwrap();

        let script = std::fs::read_to_string(
            tmp.path().join("scripts").join("navigation_controller.gd"),
        )
        .unwrap();
        assert!(script.contains("@export var guidance_update_interval := 0.2"));
        assert!(script.contains("guidance_update_accumulator += delta"));
        assert!(script.contains("if guidance_update_accumulator < guidance_update_interval:"));
        assert!(script.contains("_draw_route_ribbon()"));
        assert!(!script.contains("func _update_arrow(index: int) -> void:"));
        let update_arrow_start = script.find("func _update_guidance() -> void:").unwrap();
        let update_hud_start = script.find("func _build_hud() -> void:").unwrap();
        let update_guidance = &script[update_arrow_start..update_hud_start];
        assert!(!update_guidance.contains("CylinderMesh.new()"));
        assert!(!update_guidance.contains("ImmediateMesh.new()"));
    }
}
