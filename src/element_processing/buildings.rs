//! Building element processor — converts OSM building ways into Godot 3D meshes.
//!
//! Process:
//! 1. Extract footprint polygon in arnis XZ coords
//! 2. Parse height from OSM tags
//! 3. Generate wall + roof mesh centered at local origin
//! 4. Compute world position (centroid) and add to SceneWriter

use crate::osm_parser::ProcessedWay;
use crate::scene_writer::geometry;
use crate::scene_writer::mesh_builder;
use crate::scene_writer::tres_writer::MaterialType;
use crate::scene_writer::SceneWriter;

/// Process a building way and add its meshes to the scene.
pub fn generate_building(
    scene: &mut SceneWriter,
    way: &ProcessedWay,
    godot_scale: f32,
) {
    if way.nodes.len() < 3 {
        return;
    }

    // Footprint in arnis block coords
    let footprint_arnis: Vec<(f32, f32)> = way
        .nodes
        .iter()
        .map(|n| (n.x as f32, n.z as f32))
        .collect();

    let footprint_arnis = close_polygon(footprint_arnis);
    if footprint_arnis.len() < 3 {
        return;
    }

    // Parse building height (in meters — we use it directly as Godot units)
    let height = mesh_builder::building_height(&way.tags, godot_scale);

    // Determine roof type
    let roof_type = way
        .tags
        .get("roof:shape")
        .or_else(|| way.tags.get("roof:type"))
        .map(|s| s.as_str())
        .unwrap_or("flat");

    // Compute centroid in arnis coords for world position
    let (center_x, center_z) = centroid(&footprint_arnis);

    // Convert footprint to Godot coords centered at origin
    let footprint_local: Vec<(f32, f32)> = footprint_arnis
        .iter()
        .map(|&(x, z)| {
            (
                (x - center_x) * godot_scale,
                -(z - center_z) * godot_scale, // Flip Z for Godot
            )
        })
        .collect();

    // Build wall mesh (in local Godot space)
    let wall_thickness = 0.3;
    let mut mesh = geometry::make_wall_outline(&footprint_local, height, wall_thickness);

    // Build roof mesh
    let roof = match roof_type {
        "gabled" | "gambrel" | "mansard" | "pyramidal" | "hipped" | "dome" | "onion" => {
            geometry::make_roof_gabled(&footprint_local, height, height + 3.0)
        }
        _ => geometry::make_roof_flat(&footprint_local, height),
    };
    mesh.append(&roof, (0.0, 0.0, 0.0));

    // World position in arnis coords — SceneWriter handles the Godot conversion
    let world_x = center_x.round() as i32;
    let world_z = center_z.round() as i32;

    let name = format!("Building_{}", way.id);
    scene.add_mesh(name, mesh, MaterialType::BuildingWall, world_x, world_z);
}

fn close_polygon(mut poly: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
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
    if n == 0 {
        return (0.0, 0.0);
    }

    let mut area = 0.0f64;
    let mut cx = 0.0f64;
    let mut cz = 0.0f64;

    for i in 0..n {
        let j = (i + 1) % n;
        let (x0, z0) = (poly[i].0 as f64, poly[i].1 as f64);
        let (x1, z1) = (poly[j].0 as f64, poly[j].1 as f64);
        let cross = x0 * z1 - x1 * z0;
        area += cross;
        cx += (x0 + x1) * cross;
        cz += (z0 + z1) * cross;
    }

    if area.abs() < 1e-6 {
        let sum_x: f64 = poly.iter().map(|p| p.0 as f64).sum();
        let sum_z: f64 = poly.iter().map(|p| p.1 as f64).sum();
        return ((sum_x / n as f64) as f32, (sum_z / n as f64) as f32);
    }

    let inv = 1.0 / (3.0 * area);
    ((cx * inv) as f32, (cz * inv) as f32)
}
