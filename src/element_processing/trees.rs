//! Tree element processor — places varied low-poly vegetation meshes at OSM tree nodes.

use crate::osm_parser::ProcessedNode;
use crate::scene_writer::geometry;
use crate::scene_writer::geometry::MeshData;
use crate::scene_writer::tres_writer::MaterialType;
use crate::scene_writer::SceneWriter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VegetationProfile {
    Broadleaf,
    Conifer,
    Shrub,
}

/// Generate a tree at a node position.
pub fn generate_tree(scene: &mut SceneWriter, node: &ProcessedNode) {
    let profile = profile_for_id(node.id);
    generate_profile_instance(scene, node.id, profile, node.x, node.z);
}

pub fn generate_profile_instance(
    scene: &mut SceneWriter,
    seed: u64,
    profile: VegetationProfile,
    world_x: i32,
    world_z: i32,
) {
    let rotation = unit_hash(seed ^ 0x8f3d_2a91) * std::f32::consts::TAU;
    let scale = 0.85 + unit_hash(seed ^ 0x51ab_c309) * 0.45;

    match profile {
        VegetationProfile::Broadleaf => {
            let trunk_height = 2.1 * scale;
            let trunk = geometry::make_cylinder(0.22 * scale, trunk_height, 7);
            scene.add_instance(
                format!("VegetationTrunk_{seed}"),
                trunk,
                MaterialType::TreeTrunk,
                world_x,
                world_z,
                rotation,
            );

            let crown = offset_mesh(
                geometry::make_cylinder(1.25 * scale, 1.45 * scale, 10),
                0.0,
                trunk_height,
                0.0,
            );
            scene.add_instance(
                format!("VegetationTree_{seed}"),
                crown,
                MaterialType::TreeLeaves,
                world_x,
                world_z,
                rotation,
            );
        }
        VegetationProfile::Conifer => {
            let trunk_height = 1.25 * scale;
            let trunk = geometry::make_cylinder(0.18 * scale, trunk_height, 6);
            scene.add_instance(
                format!("VegetationTrunk_{seed}"),
                trunk,
                MaterialType::TreeTrunk,
                world_x,
                world_z,
                rotation,
            );

            let crown = offset_mesh(
                geometry::make_cone(1.15 * scale, 3.6 * scale, 9),
                0.0,
                trunk_height * 0.45,
                0.0,
            );
            scene.add_instance(
                format!("VegetationConifer_{seed}"),
                crown,
                MaterialType::TreeLeaves,
                world_x,
                world_z,
                rotation,
            );
        }
        VegetationProfile::Shrub => {
            let shrub = offset_mesh(
                geometry::make_cylinder(0.95 * scale, 0.85 * scale, 8),
                0.0,
                0.08,
                0.0,
            );
            scene.add_instance(
                format!("VegetationShrub_{seed}"),
                shrub,
                MaterialType::TreeLeaves,
                world_x,
                world_z,
                rotation,
            );
        }
    }
}

pub fn profile_for_id(id: u64) -> VegetationProfile {
    match stable_hash(id) % 3 {
        0 => VegetationProfile::Broadleaf,
        1 => VegetationProfile::Conifer,
        _ => VegetationProfile::Shrub,
    }
}

fn offset_mesh(mut mesh: MeshData, x: f32, y: f32, z: f32) -> MeshData {
    for vertex in mesh.vertices.chunks_exact_mut(3) {
        vertex[0] += x;
        vertex[1] += y;
        vertex[2] += z;
    }
    mesh
}

pub(crate) fn stable_hash(value: u64) -> u64 {
    let mut x = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

pub(crate) fn unit_hash(value: u64) -> f32 {
    let bits = (stable_hash(value) >> 40) as u32;
    bits as f32 / 16_777_215.0
}
