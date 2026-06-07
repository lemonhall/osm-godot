//! Tree element processor — places 3D tree meshes at OSM node positions.

use crate::osm_parser::ProcessedNode;
use crate::scene_writer::geometry;
use crate::scene_writer::tres_writer::MaterialType;
use crate::scene_writer::SceneWriter;

/// Generate a tree at a node position.
pub fn generate_tree(
    scene: &mut SceneWriter,
    node: &ProcessedNode,
) {
    let world_x = node.x;
    let world_z = node.z;

    // Tree parameters (scale with godot_scale, but keep proportional)
    let trunk_radius = 0.3;
    let trunk_height = 2.5;
    let canopy_radius = 1.5;

    // Trunk mesh (local coords at origin)
    let trunk = geometry::make_cylinder(trunk_radius, trunk_height, 8);

    // Canopy mesh (offset upward by trunk height)
    let canopy = geometry::make_cone(canopy_radius, canopy_radius * 3.0, 8);
    let mut canopy_pos = canopy;
    // Offset canopy vertices up (TODO: make a proper append_with_offset)
    let vertex_count = canopy_pos.vertices.len() / 3;
    for i in 0..vertex_count {
        canopy_pos.vertices[i * 3 + 1] += trunk_height; // Y offset
    }

    // Combine trunk + canopy into one mesh
    let mut tree_mesh = trunk;
    tree_mesh.append(&canopy_pos, (0.0, 0.0, 0.0));

    let name = format!("Tree_{}", node.id);
    scene.add_mesh(name, tree_mesh, MaterialType::TreeTrunk, world_x, world_z);
}
