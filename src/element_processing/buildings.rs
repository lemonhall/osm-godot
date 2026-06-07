//! Building processor — wall outline + roof as separate meshes.

use crate::osm_parser::ProcessedWay;
use crate::scene_writer::geometry;
use crate::scene_writer::mesh_builder;
use crate::scene_writer::tres_writer::MaterialType;
use crate::scene_writer::SceneWriter;

pub fn generate_building(
    scene: &mut SceneWriter,
    way: &ProcessedWay,
    godot_scale: f32,
) {
    if way.nodes.len() < 3 { return; }

    // Footprint in arnis coords
    let fp: Vec<(f32, f32)> = way.nodes.iter().map(|n| (n.x as f32, n.z as f32)).collect();
    let fp = close_poly(fp);
    if fp.len() < 3 { return; }

    // Filter tiny buildings: area < 2 arnis_units² ≈ 0.5 m²
    let area = polygon_area(&fp);
    if area < 2.0 { return; }

    // Height in meters
    let height = mesh_builder::building_height(&way.tags, godot_scale).max(2.5);

    // Centroids
    let (cx, cz) = centroid(&fp);

    // Roof type
    let roof_type = way.tags.get("roof:shape")
        .or_else(|| way.tags.get("roof:type"))
        .map(|s| s.as_str()).unwrap_or("flat");

    // Convert to local Godot coords
    let fp_local: Vec<(f32, f32)> = fp.iter()
        .map(|&(x, z)| ((x - cx) * godot_scale, -(z - cz) * godot_scale))
        .collect();

    // ── Wall mesh ──
    let wall = geometry::make_wall_outline(&fp_local, height, 0.3);

    // ── Roof mesh ──
    let roof = match roof_type {
        "gabled"|"gambrel"|"mansard"|"pyramidal"|"hipped"|"dome"|"onion" =>
            geometry::make_roof_gabled(&fp_local, height, height + 3.0),
        _ => geometry::make_roof_flat(&fp_local, height),
    };

    let wx = cx.round() as i32;
    let wz = cz.round() as i32;

    scene.add_mesh(format!("BuildingWall_{}", way.id), wall, MaterialType::BuildingWall, wx, wz);
    scene.add_mesh(format!("BuildingRoof_{}", way.id), roof, MaterialType::BuildingRoof, wx, wz);
}

fn close_poly(mut p: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    if p.len() < 2 { return p; }
    let f = p[0]; let l = *p.last().unwrap();
    if (f.0 - l.0).abs() > 0.01 || (f.1 - l.1).abs() > 0.01 { p.push(f); }
    p
}

fn centroid(p: &[(f32, f32)]) -> (f32, f32) {
    let n = p.len();
    if n == 0 { return (0.0,0.0); }
    let mut a = 0.0f64; let mut cx = 0.0f64; let mut cz = 0.0f64;
    for i in 0..n {
        let j = (i+1)%n;
        let (x0,z0) = (p[i].0 as f64, p[i].1 as f64);
        let (x1,z1) = (p[j].0 as f64, p[j].1 as f64);
        let cross = x0*z1 - x1*z0;
        a += cross; cx += (x0+x1)*cross; cz += (z0+z1)*cross;
    }
    if a.abs() < 1e-6 {
        let sx: f64 = p.iter().map(|p| p.0 as f64).sum();
        let sz: f64 = p.iter().map(|p| p.1 as f64).sum();
        return ((sx/n as f64) as f32, (sz/n as f64) as f32);
    }
    let inv = 1.0/(3.0*a);
    ((cx*inv) as f32, (cz*inv) as f32)
}

fn polygon_area(p: &[(f32, f32)]) -> f64 {
    let n = p.len();
    if n < 3 { return 0.0; }
    let mut a = 0.0f64;
    for i in 0..n {
        let j = (i+1)%n;
        a += p[i].0 as f64 * p[j].1 as f64 - p[j].0 as f64 * p[i].1 as f64;
    }
    a.abs() * 0.5
}
