//! Terrain mesh generation — creates a heightmap-based ground mesh for each chunk.

use crate::coordinate_system::cartesian::XZPoint;
use crate::ground::Ground;
use crate::scene_writer::chunk_grid::{ChunkCoord, ChunkGrid};
use crate::scene_writer::geometry::MeshData;
use crate::scene_writer::tres_writer::MaterialType;
use std::sync::Arc;

/// Generate terrain meshes for all chunks and add them to the chunk grid.
pub fn generate_terrain(
    chunk_grid: &mut ChunkGrid,
    ground: &Arc<Ground>,
    godot_scale: f32,
) {
    let step = 8; // Sample every N blocks (higher = coarser terrain = faster)

    for coord in chunk_grid.all_coords() {
        let terrain_mesh = build_chunk_terrain(chunk_grid, ground, coord, godot_scale, step);
        if terrain_mesh.vertices.is_empty() {
            continue;
        }

        let chunk = &mut chunk_grid.chunks.get_mut(&coord).unwrap();
        let (min_x, min_z, _, _) = chunk.world_bounds;

        // Determine material based on land cover
        let material = if let Some(lc) = ground.land_cover_grid() {
            // Sample center of chunk for land cover
            let cx = min_x + chunk_grid.chunk_size / 2;
            let cz = min_z + chunk_grid.chunk_size / 2;
            let lc_class = ground.cover_class(XZPoint::new(cx, cz));
            land_cover_to_material(lc_class)
        } else {
            MaterialType::TerrainGrass
        };

        use crate::scene_writer::chunk_grid::SceneElement;
        use crate::scene_writer::tscn_writer;

        chunk.elements.push(SceneElement::Mesh {
            name: format!("Terrain_{}_{}", coord.0, coord.1),
            mesh_data: terrain_mesh,
            material_type: material,
            transform: tscn_writer::translation_transform(0.0, 0.0, 0.0),
        });
    }
}

/// Build a heightmap terrain mesh for a single chunk.
fn build_chunk_terrain(
    chunk_grid: &ChunkGrid,
    ground: &Arc<Ground>,
    coord: ChunkCoord,
    godot_scale: f32,
    step: i32,
) -> MeshData {
    let chunk = &chunk_grid.chunks[&coord];
    let (min_x, min_z, max_x, max_z) = chunk.world_bounds;

    let cols = ((max_x - min_x) / step + 1) as usize;
    let rows = ((max_z - min_z) / step + 1) as usize;

    if cols < 2 || rows < 2 {
        return MeshData::new();
    }

    let mut vertices = Vec::with_capacity(cols * rows * 3);
    let mut normals = Vec::with_capacity(cols * rows * 3);
    let mut uvs = Vec::with_capacity(cols * rows * 2);
    let mut indices = Vec::with_capacity((cols - 1) * (rows - 1) * 6);

    // Sample elevation grid
    let mut heights = vec![vec![0.0f32; cols]; rows];
    for (ri, row) in heights.iter_mut().enumerate() {
        for (ci, cell) in row.iter_mut().enumerate() {
            let wx = min_x + ci as i32 * step;
            let wz = min_z + ri as i32 * step;
            let level = ground.level(XZPoint::new(wx, wz));
            *cell = level as f32 * godot_scale;
        }
    }

    for ri in 0..rows {
        for ci in 0..cols {
            let wx = min_x + ci as i32 * step;
            let wz = min_z + ri as i32 * step;
            let x = (wx - min_x) as f32 * godot_scale;
            let z = -((wz - min_z) as f32) * godot_scale;
            let y = heights[ri][ci];
            vertices.push(x);
            vertices.push(y);
            vertices.push(z);

            normals.push(0.0);
            normals.push(1.0);
            normals.push(0.0); // placeholder — compute proper normals below

            uvs.push(ci as f32 / cols.max(1) as f32);
            uvs.push(ri as f32 / rows.max(1) as f32);
        }
    }

    // Build triangle indices
    for ri in 0..(rows - 1) {
        for ci in 0..(cols - 1) {
            let tl = (ri * cols + ci) as u32;
            let tr = (ri * cols + ci + 1) as u32;
            let bl = ((ri + 1) * cols + ci) as u32;
            let br = ((ri + 1) * cols + ci + 1) as u32;

            indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }
    }

    // Compute smooth normals per vertex
    compute_smooth_normals(&mut normals, &vertices, &indices);

    MeshData {
        vertices,
        normals,
        uvs,
        indices,
    }
}

/// Compute per-vertex smooth normals by averaging face normals.
fn compute_smooth_normals(normals: &mut [f32], vertices: &[f32], indices: &[u32]) {
    // Zero out normals first
    for n in normals.iter_mut() {
        *n = 0.0;
    }

    // Accumulate face normals
    for tri in indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let i0 = tri[0] as usize * 3;
        let i1 = tri[1] as usize * 3;
        let i2 = tri[2] as usize * 3;

        let v0 = (vertices[i0], vertices[i0 + 1], vertices[i0 + 2]);
        let v1 = (vertices[i1], vertices[i1 + 1], vertices[i1 + 2]);
        let v2 = (vertices[i2], vertices[i2 + 1], vertices[i2 + 2]);

        let u = (v1.0 - v0.0, v1.1 - v0.1, v1.2 - v0.2);
        let v = (v2.0 - v0.0, v2.1 - v0.1, v2.2 - v0.2);
        let nx = u.1 * v.2 - u.2 * v.1;
        let ny = u.2 * v.0 - u.0 * v.2;
        let nz = u.0 * v.1 - u.1 * v.0;

        for &idx in &[i0, i1, i2] {
            normals[idx] += nx;
            normals[idx + 1] += ny;
            normals[idx + 2] += nz;
        }
    }

    // Normalize all normals
    for n in normals.chunks_mut(3) {
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len > 0.0 {
            n[0] /= len;
            n[1] /= len;
            n[2] /= len;
        } else {
            n[0] = 0.0;
            n[1] = 1.0;
            n[2] = 0.0;
        }
    }
}

/// Map ESA WorldCover land class to a MaterialType.
fn land_cover_to_material(lc_class: u8) -> MaterialType {
    match lc_class {
        10 => MaterialType::TreeLeaves,   // Tree cover → grass (trees are separate objects)
        20 => MaterialType::TerrainGrass, // Shrubland
        30 => MaterialType::TerrainGrass, // Grassland
        40 => MaterialType::TerrainDirt,  // Cropland
        50 => MaterialType::TerrainBuiltUp, // Built-up
        60 => MaterialType::TerrainDirt,  // Bare
        70 => MaterialType::TerrainDirt,  // Snow/ice (rare)
        80 => MaterialType::Water,        // Water
        90 => MaterialType::TerrainGrass, // Wetland
        95 => MaterialType::TerrainGrass, // Mangroves
        100 => MaterialType::TerrainDirt, // Moss/lichen
        _ => MaterialType::TerrainGrass,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinate_system::cartesian::XZBBox;

    #[test]
    fn terrain_mesh_is_local_to_its_chunk() {
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let mut chunk_grid = ChunkGrid::new(&bbox, 256);
        let ground = Arc::new(Ground::new_flat(0));

        generate_terrain(&mut chunk_grid, &ground, 0.5);

        let chunk = &chunk_grid.chunks[&ChunkCoord(1, 1)];
        let terrain = chunk
            .elements
            .iter()
            .find(|element| {
                matches!(
                    element,
                    crate::scene_writer::chunk_grid::SceneElement::Mesh { name, .. }
                        if name == "Terrain_1_1"
                )
            })
            .expect("terrain mesh for chunk 1,1");

        let crate::scene_writer::chunk_grid::SceneElement::Mesh {
            transform,
            mesh_data,
            ..
        } = terrain
        else {
            panic!("expected mesh element");
        };

        assert_eq!(transform[9], 0.0);
        assert_eq!(transform[10], 0.0);
        assert_eq!(transform[11], 0.0);
        assert_eq!(mesh_data.vertices[0], 0.0);
        assert_eq!(mesh_data.vertices[2], 0.0);
    }
}
