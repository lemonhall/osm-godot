//! Spatial partitioning: divides the world XZ bounding box into chunk-sized units.
//! Each chunk holds a collection of SceneElements ready for export.

use crate::coordinate_system::cartesian::XZBBox;
use crate::scene_writer::geometry::MeshData;
use crate::scene_writer::tres_writer::MaterialType;
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

/// A rendered element within a chunk.
pub enum SceneElement {
    /// A single mesh instance placed at a specific transform.
    Mesh {
        name: String,
        mesh_data: MeshData,
        material_type: MaterialType,
        /// Column-major 3×4 transform matrix (12 floats).
        transform: [f32; 12],
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
        let mut chunks = HashMap::new();

        let min_cx = xzbbox.min_x().div_euclid(chunk_size);
        let max_cx = xzbbox.max_x().div_euclid(chunk_size);
        let min_cz = xzbbox.min_z().div_euclid(chunk_size);
        let max_cz = xzbbox.max_z().div_euclid(chunk_size);

        for cx in min_cx..=max_cx {
            for cz in min_cz..=max_cz {
                let world_min_x = cx * chunk_size;
                let world_min_z = cz * chunk_size;
                let world_max_x = world_min_x + chunk_size - 1;
                let world_max_z = world_min_z + chunk_size - 1;

                chunks.insert(
                    ChunkCoord(cx, cz),
                    Chunk {
                        coord: ChunkCoord(cx, cz),
                        world_bounds: (world_min_x, world_min_z, world_max_x, world_max_z),
                        elements: Vec::new(),
                    },
                );
            }
        }

        ChunkGrid {
            xzbbox: xzbbox.clone(),
            chunk_size,
            chunks,
        }
    }

    /// Determine which chunk a world coordinate belongs to.
    pub fn chunk_for(&self, x: i32, z: i32) -> Option<ChunkCoord> {
        let cx = x.div_euclid(self.chunk_size);
        let cz = z.div_euclid(self.chunk_size);
        let coord = ChunkCoord(cx, cz);
        if self.chunks.contains_key(&coord) {
            Some(coord)
        } else {
            None
        }
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
    ) {
        if let Some(coord) = self.chunk_for(world_x, world_z) {
            let gx = world_x as f32 * godot_scale;
            let gz = -(world_z as f32) * godot_scale;

            let transform = crate::scene_writer::tscn_writer::translation_transform(gx, ground_y, gz);

            if let Some(chunk) = self.chunks.get_mut(&coord) {
                chunk.elements.push(SceneElement::Mesh {
                    name,
                    mesh_data,
                    material_type,
                    transform,
                });
            }
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
            let gx = world_x as f32 * godot_scale;
            let gz = -(world_z as f32) * godot_scale;

            if let Some(chunk) = self.chunks.get_mut(&coord) {
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
}
