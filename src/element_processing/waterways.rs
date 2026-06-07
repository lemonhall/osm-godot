//! Waterway processor — generates river/stream surface strips.

use crate::osm_parser::ProcessedWay;
use crate::scene_writer::geometry;
use crate::scene_writer::tres_writer::MaterialType;
use crate::scene_writer::SceneWriter;

/// Generate a river/stream surface from a waterway way.
pub fn generate_waterway(
    scene: &mut SceneWriter,
    way: &ProcessedWay,
    godot_scale: f32,
) {
    if way.nodes.len() < 2 {
        return;
    }

    let width = match way.tags.get("waterway").map(String::as_str) {
        Some("river") => 8.0,
        Some("canal") => 6.0,
        Some("stream") => 2.0,
        _ => 4.0,
    };

    let centerline: Vec<(f32, f32)> = way
        .nodes
        .iter()
        .map(|n| (n.x as f32, n.z as f32))
        .collect();

    let mid_idx = centerline.len() / 2;
    let (cx, cz) = centerline[mid_idx];

    let centerline_local: Vec<(f32, f32)> = centerline
        .iter()
        .map(|&(x, z)| (
            (x - cx) * godot_scale,
            -(z - cz) * godot_scale,
        ))
        .collect();

    let mesh = geometry::make_road_surface(&centerline_local, width);

    let name = format!("Waterway_{}", way.id);
    scene.add_mesh(
        name,
        mesh,
        MaterialType::Water,
        cx.round() as i32,
        cz.round() as i32,
    );
}
