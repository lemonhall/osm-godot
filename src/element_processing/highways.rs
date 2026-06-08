//! Highway element processor — converts OSM highway ways into road surface meshes.

use crate::osm_parser::ProcessedWay;
use crate::scene_writer::geometry;
use crate::scene_writer::mesh_builder;
use crate::scene_writer::tres_writer::MaterialType;
use crate::scene_writer::SceneWriter;

/// Generate a road surface from a highway way.
pub fn generate_highway(scene: &mut SceneWriter, way: &ProcessedWay, godot_scale: f32) {
    if way.nodes.len() < 2 {
        return;
    }

    // Parse road width
    let width = mesh_builder::highway_width(&way.tags);

    // Centerline in arnis coords
    let centerline_arnis: Vec<(f32, f32)> =
        way.nodes.iter().map(|n| (n.x as f32, n.z as f32)).collect();
    scene.add_navigation_road(way.id, &way.tags, &centerline_arnis);

    // Compute midpoint for world position reference
    let mid_idx = centerline_arnis.len() / 2;
    let (center_x, center_z) = centerline_arnis[mid_idx];

    // Convert centerline to local Godot coords (centered at midpoint)
    let centerline_local: Vec<(f32, f32)> = centerline_arnis
        .iter()
        .map(|&(x, z)| ((x - center_x) * godot_scale, -(z - center_z) * godot_scale))
        .collect();

    // Generate road surface mesh
    let mesh = geometry::make_road_surface(&centerline_local, width);

    let world_x = center_x.round() as i32;
    let world_z = center_z.round() as i32;

    // Determine material based on highway type
    let material = match way.tags.get("highway").map(String::as_str) {
        Some("footway") | Some("path") | Some("cycleway") | Some("pedestrian") => {
            MaterialType::RoadSidewalk
        }
        _ => MaterialType::RoadAsphalt,
    };

    let mut metadata = super::osm_metadata(way.id, "road", &way.tags);
    metadata.insert("road_width_m".to_string(), format!("{width:.2}"));

    let name = format!("Highway_{}", way.id);
    scene.add_mesh_with_metadata(name, mesh, material, world_x, world_z, metadata);
}
