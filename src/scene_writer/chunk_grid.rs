//! Spatial partitioning: divides the world XZ bounding box into chunk-sized units.
//! Each chunk holds a collection of SceneElements ready for export.

use crate::coordinate_system::cartesian::XZBBox;
use crate::scene_writer::geometry::MeshData;
use crate::scene_writer::tres_writer::MaterialType;
use std::collections::BTreeMap;
use std::collections::HashMap;

/// Default chunk size in arnis block units. At godot_scale=0.5, 256 blocks = 128m.
pub const DEFAULT_CHUNK_SIZE: i32 = 256;

/// Identifies a chunk in the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkCoord(pub i32, pub i32);

/// A single chunk, containing all scene elements within its bounds.
pub struct Chunk {
    pub coord: ChunkCoord,
    /// World bounds in arnis block coordinates: (min_x, min_z, max_x, max_z).
    pub world_bounds: (i32, i32, i32, i32),
    /// All elements residing in this chunk.
    pub elements: Vec<SceneElement>,
}

pub type ElementMetadata = BTreeMap<String, String>;

/// A rendered element within a chunk.
pub enum SceneElement {
    /// A single mesh instance placed at a specific transform.
    Mesh {
        name: String,
        mesh_data: MeshData,
        material_type: MaterialType,
        /// Column-major 3×4 transform matrix (12 floats).
        transform: [f32; 12],
        metadata: ElementMetadata,
    },
    /// Instanced mesh: multiple placements sharing the same mesh data.
    /// Godot 4 uses MultiMeshInstance3D for efficient instancing.
    Instance {
        name: String,
        mesh_data: MeshData,
        material_type: MaterialType,
        /// (translation, y_rotation) for each instance.
        positions: Vec<((f32, f32, f32), f32)>,
    },
}

/// Manages spatial partitioning of element placements into chunks.
pub struct ChunkGrid {
    /// The full world bounding box (arnis coordinates).
    pub xzbbox: XZBBox,
    /// Chunk size in arnis block units.
    pub chunk_size: i32,
    /// All chunks, keyed by coordinate.
    pub chunks: HashMap<ChunkCoord, Chunk>,
}

impl ChunkGrid {
    /// Create a new chunk grid covering the given world bbox.
    pub fn new(xzbbox: &XZBBox, chunk_size: i32) -> Self {
        ChunkGrid {
            xzbbox: xzbbox.clone(),
            chunk_size,
            chunks: HashMap::new(),
        }
    }

    /// Determine which chunk a world coordinate belongs to.
    pub fn chunk_for(&self, x: i32, z: i32) -> Option<ChunkCoord> {
        if x < self.xzbbox.min_x()
            || x > self.xzbbox.max_x()
            || z < self.xzbbox.min_z()
            || z > self.xzbbox.max_z()
        {
            return None;
        }
        let cx = x.div_euclid(self.chunk_size);
        let cz = z.div_euclid(self.chunk_size);
        Some(ChunkCoord(cx, cz))
    }

    /// Returns bounding box center in Godot coords (meters).
    pub fn bbox_center_godot(&self, godot_scale: f32) -> (f32, f32) {
        let cx = (self.xzbbox.min_x() + self.xzbbox.max_x()) as f32 * 0.5 * godot_scale;
        let cz = -(self.xzbbox.min_z() + self.xzbbox.max_z()) as f32 * 0.5 * godot_scale;
        (cx, cz)
    }

    /// List all chunk coords.
    pub fn all_coords(&self) -> Vec<ChunkCoord> {
        self.chunks.keys().copied().collect()
    }

    /// Chunk bounds in Godot world coordinates (meters).
    pub fn chunk_bounds_godot(&self, coord: ChunkCoord, godot_scale: f32) -> (f32, f32, f32, f32) {
        let (min_x, min_z, max_x, max_z) = self.chunks[&coord].world_bounds;
        (
            min_x as f32 * godot_scale,
            -(max_z as f32) * godot_scale, // Z flips for Godot
            max_x as f32 * godot_scale,
            -(min_z as f32) * godot_scale,
        )
    }

    /// Number of chunks.
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    fn ensure_chunk(&mut self, coord: ChunkCoord) -> &mut Chunk {
        self.chunks.entry(coord).or_insert_with(|| {
            let world_min_x = coord.0 * self.chunk_size;
            let world_min_z = coord.1 * self.chunk_size;
            let world_max_x = world_min_x + self.chunk_size - 1;
            let world_max_z = world_min_z + self.chunk_size - 1;
            Chunk {
                coord,
                world_bounds: (world_min_x, world_min_z, world_max_x, world_max_z),
                elements: Vec::new(),
            }
        })
    }

    /// Add a mesh element to the appropriate chunk.
    pub fn add_mesh_element(
        &mut self,
        name: String,
        mesh_data: MeshData,
        material_type: MaterialType,
        world_x: i32,
        world_z: i32,
        godot_scale: f32,
        ground_y: f32,
        metadata: ElementMetadata,
    ) {
        if let Some(coord) = self.chunk_for(world_x, world_z) {
            let (min_x, min_z, _, _) = self.ensure_chunk(coord).world_bounds;
            let gx = (world_x - min_x) as f32 * godot_scale;
            let gz = -((world_z - min_z) as f32) * godot_scale;

            let transform =
                crate::scene_writer::tscn_writer::translation_transform(gx, ground_y, gz);

            self.ensure_chunk(coord).elements.push(SceneElement::Mesh {
                name,
                mesh_data,
                material_type,
                transform,
                metadata,
            });
        }
    }

    /// Add an instance to the appropriate chunk.
    pub fn add_instance(
        &mut self,
        name: String,
        mesh_data: MeshData,
        material_type: MaterialType,
        world_x: i32,
        world_z: i32,
        godot_scale: f32,
        ground_y: f32,
        y_rotation: f32,
    ) {
        if let Some(coord) = self.chunk_for(world_x, world_z) {
            let (min_x, min_z, _, _) = self.ensure_chunk(coord).world_bounds;
            let gx = (world_x - min_x) as f32 * godot_scale;
            let gz = -((world_z - min_z) as f32) * godot_scale;

            let chunk = self.ensure_chunk(coord);
            // Check if we already have an Instance for this name/material combo
            let existing = chunk.elements.iter_mut().find_map(|e| match e {
                SceneElement::Instance {
                    name: n,
                    material_type: mt,
                    ..
                } if n == &name && mt == &material_type => Some(e),
                _ => None,
            });

            if let Some(SceneElement::Instance { positions, .. }) = existing {
                positions.push(((gx, ground_y, gz), y_rotation));
            } else {
                chunk.elements.push(SceneElement::Instance {
                    name,
                    mesh_data,
                    material_type,
                    positions: vec![((gx, ground_y, gz), y_rotation)],
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinate_system::cartesian::XZBBox;

    fn unit_mesh() -> MeshData {
        let mut mesh = MeshData::new();
        mesh.vertices.extend_from_slice(&[0.0, 0.0, 0.0]);
        mesh
    }

    #[test]
    fn mesh_transform_is_local_to_its_chunk() {
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let mut grid = ChunkGrid::new(&bbox, 256);

        grid.add_mesh_element(
            "Test".to_string(),
            unit_mesh(),
            MaterialType::BuildingWall,
            300,
            300,
            0.5,
            7.0,
            ElementMetadata::new(),
        );

        let chunk = &grid.chunks[&ChunkCoord(1, 1)];
        let SceneElement::Mesh { transform, .. } = &chunk.elements[0] else {
            panic!("expected mesh element");
        };

        assert_eq!(transform[9], 22.0);
        assert_eq!(transform[10], 7.0);
        assert_eq!(transform[11], -22.0);
    }

    #[test]
    fn large_bbox_grid_stays_sparse_until_elements_are_added() {
        let bbox = XZBBox::rect_from_xz_lengths(120_000.0, 130_000.0).unwrap();
        let mut grid = ChunkGrid::new(&bbox, 128);

        assert_eq!(grid.chunk_count(), 0);

        grid.add_mesh_element(
            "SparseTest".to_string(),
            unit_mesh(),
            MaterialType::BuildingWall,
            42,
            42,
            0.5,
            0.0,
            ElementMetadata::new(),
        );

        assert_eq!(grid.chunk_count(), 1);
        assert!(grid.chunks.contains_key(&ChunkCoord(0, 0)));
    }
}
