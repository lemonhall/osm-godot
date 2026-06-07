//! Water area processor — creates water surface planes from OSM water polygons.

use crate::osm_parser::ProcessedWay;
use crate::scene_writer::geometry;
use crate::scene_writer::tres_writer::MaterialType;
use crate::scene_writer::SceneWriter;

/// Generate a water surface from a water-area way.
pub fn generate_water_area(
    scene: &mut SceneWriter,
    way: &ProcessedWay,
    godot_scale: f32,
) {
    if way.nodes.len() < 3 {
        return;
    }

    // Footprint in arnis coords
    let footprint: Vec<(f32, f32)> = way
        .nodes
        .iter()
        .map(|n| (n.x as f32, n.z as f32))
        .collect();

    // Close polygon
    let footprint = close_poly(footprint);
    if footprint.len() < 3 {
        return;
    }

    // Compute centroid
    let (cx, cz) = centroid(&footprint);

    // Convert to local Godot coords
    let footprint_local: Vec<(f32, f32)> = footprint
        .iter()
        .map(|&(x, z)| (
            (x - cx) * godot_scale,
            -(z - cz) * godot_scale,
        ))
        .collect();

    // Build flat water surface at y=0 (SceneWriter will lift to ground level)
    let mut mesh = geometry::make_roof_flat(&footprint_local, 0.0);

    // Also add a bottom face for water (it's visible from below)
    // (make_roof_flat already includes bottom face)

    let world_x = cx.round() as i32;
    let world_z = cz.round() as i32;

    let name = format!("Water_{}", way.id);
    scene.add_mesh(name, mesh, MaterialType::Water, world_x, world_z);
}

fn close_poly(mut poly: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    if poly.len() < 2 {
        return poly;
    }
    let first = poly[0];
    let &(lx, lz) = poly.last().unwrap();
    if (first.0 - lx).abs() > 0.01 || (first.1 - lz).abs() > 0.01 {
        poly.push(first);
    }
    poly
}

fn centroid(poly: &[(f32, f32)]) -> (f32, f32) {
    let n = poly.len();
    let sum_x: f64 = poly.iter().map(|p| p.0 as f64).sum();
    let sum_z: f64 = poly.iter().map(|p| p.1 as f64).sum();
    ((sum_x / n as f64) as f32, (sum_z / n as f64) as f32)
}
