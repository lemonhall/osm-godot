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
use chunk_grid::{ChunkGrid, SceneElement};
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

        fs::create_dir_all(&scenes_dir)?;
        fs::create_dir_all(&materials_dir)?;

        // Write materials
        tres_writer::write_all_materials(&materials_dir)?;

        // Write each chunk scene
        let mut non_empty_count = 0u64;
        for coord in self.chunk_grid.all_coords() {
            let chunk = &self.chunk_grid.chunks[&coord];
            if chunk.elements.is_empty() {
                continue;
            }
            tscn_writer::write_chunk_scene(chunk, &scenes_dir, &self.material_ids)?;
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

        // load_steps = N ext_resources + 1 Environment sub_resource
        let load_steps = non_empty.len() as u32 + 1;
        writeln!(f, "[gd_scene load_steps={load_steps} format=3 uid=\"uid://master000001\"]")?;
        writeln!(f)?;

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

        // Camera3D
        let world_cx = (self.chunk_grid.xzbbox.min_x() + self.chunk_grid.xzbbox.max_x()) as f32 * 0.5 * self.godot_scale;
        let world_cz = -(self.chunk_grid.xzbbox.min_z() + self.chunk_grid.xzbbox.max_z()) as f32 * 0.5 * self.godot_scale;
        let span_x = (self.chunk_grid.xzbbox.max_x() - self.chunk_grid.xzbbox.min_x()).abs() as f32 * self.godot_scale;
        let span_z = (self.chunk_grid.xzbbox.max_z() - self.chunk_grid.xzbbox.min_z()).abs() as f32 * self.godot_scale;
        let span = span_x.max(span_z).max(1.0);
        let cam_y = (span * 0.75).clamp(80.0, 600.0);
        let cam_z = world_cz + (span * 0.85).clamp(60.0, 500.0);
        let camera = look_at_transform(
            (world_cx, cam_y, cam_z),
            (world_cx, 0.0, world_cz),
        );
        writeln!(f, "[node name=\"Camera3D\" type=\"Camera3D\" parent=\".\"]")?;
        write_transform3d(&mut f, camera)?;
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
            writeln!(f, "[node name=\"{cname}\" type=\"Node3D\" parent=\"{chunks_group}\"]")?;
            writeln!(f, "transform = Transform3D(1, 0, 0, 0, 1, 0, 0, 0, 1, {gx}, 0, {gz})")?;
            writeln!(f, "instance = ExtResource(\"{eid}\")")?;
            writeln!(f)?;
        }

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

fn look_at_transform(eye: (f32, f32, f32), target: (f32, f32, f32)) -> [f32; 12] {
    let forward = normalize((target.0 - eye.0, target.1 - eye.1, target.2 - eye.2));
    let up = (0.0, 1.0, 0.0);
    let right = normalize(cross(forward, up));
    let camera_up = cross(right, forward);
    let back = (-forward.0, -forward.1, -forward.2);

    [
        right.0, right.1, right.2,
        camera_up.0, camera_up.1, camera_up.2,
        back.0, back.1, back.2,
        eye.0, eye.1, eye.2,
    ]
}

fn normalize(v: (f32, f32, f32)) -> (f32, f32, f32) {
    let len = (v.0 * v.0 + v.1 * v.1 + v.2 * v.2).sqrt();
    if len <= f32::EPSILON {
        (0.0, 0.0, 1.0)
    } else {
        (v.0 / len, v.1 / len, v.2 / len)
    }
}

fn cross(a: (f32, f32, f32), b: (f32, f32, f32)) -> (f32, f32, f32) {
    (
        a.1 * b.2 - a.2 * b.1,
        a.2 * b.0 - a.0 * b.2,
        a.0 * b.1 - a.1 * b.0,
    )
}

fn write_transform3d(f: &mut std::fs::File, m: [f32; 12]) -> std::io::Result<()> {
    use std::io::Write;

    write!(f, "transform = Transform3D(")?;
    for (i, v) in m.iter().enumerate() {
        if i > 0 {
            write!(f, ", ")?;
        }
        if v.fract() == 0.0 {
            write!(f, "{}", *v as i32)?;
        } else {
            write!(f, "{v:.4}")?;
        }
    }
    writeln!(f, ")")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinate_system::cartesian::XZBBox;
    use crate::ground::Ground;

    #[test]
    fn master_scene_has_current_camera_for_run_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let master = std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        assert!(master.contains("[node name=\"Camera3D\" type=\"Camera3D\" parent=\".\"]"));
        assert!(master.contains("current = true"));
        assert!(!master.contains("Transform3D(1, 0, 0, 0, 1, 0, 0, 0, 1, 127.8, 150.0, -127.8)"));
    }

    #[test]
    fn master_scene_attaches_direct_children_to_scene_root() {
        let tmp = tempfile::tempdir().unwrap();
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let scene = SceneWriter::new(&bbox, ground, tmp.path().to_path_buf(), 256, 0.5);

        scene.save_all().unwrap();

        let master = std::fs::read_to_string(tmp.path().join("scenes").join("master.tscn")).unwrap();
        assert!(master.contains("[node name=\"Camera3D\" type=\"Camera3D\" parent=\".\"]"));
        assert!(master.contains("[node name=\"Chunks\" type=\"Node3D\" parent=\".\"]"));
        assert!(!master.contains("parent=\"World\""));
    }
}
