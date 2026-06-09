//! Vegetation area processor — turns OSM green polygons into ground patches and plants.

use crate::osm_parser::ProcessedWay;
use crate::scene_writer::geometry;
use crate::scene_writer::tres_writer::MaterialType;
use crate::scene_writer::SceneWriter;

use super::trees::{self, VegetationProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VegetationKind {
    Woodland,
    Park,
    Grass,
    Scrub,
}

pub fn is_vegetation_area(way: &ProcessedWay) -> bool {
    classify_vegetation_area(way).is_some()
}

pub fn generate_vegetation_area(scene: &mut SceneWriter, way: &ProcessedWay, godot_scale: f32) {
    let Some(kind) = classify_vegetation_area(way) else {
        return;
    };
    let footprint = closed_footprint(way);
    if footprint.len() < 4 {
        return;
    }

    let (cx, cz) = centroid(&footprint);
    let footprint_local: Vec<(f32, f32)> = footprint
        .iter()
        .map(|&(x, z)| ((x - cx) * godot_scale, -(z - cz) * godot_scale))
        .collect();
    let ground_mesh = geometry::make_roof_flat(&footprint_local, 0.03);
    let world_x = cx.round() as i32;
    let world_z = cz.round() as i32;
    let mut metadata = super::osm_metadata(way.id, "vegetation", &way.tags);
    metadata.insert("vegetation_kind".to_string(), kind.name().to_string());
    scene.add_mesh_with_metadata(
        format!("VegetationGround_{}", way.id),
        ground_mesh,
        kind.ground_material(),
        world_x,
        world_z,
        metadata,
    );

    for (index, (x, z, profile)) in scatter_points(way.id, kind, &footprint)
        .into_iter()
        .enumerate()
    {
        let seed = way.id.wrapping_mul(10_000).wrapping_add(index as u64);
        trees::generate_profile_instance(scene, seed, profile, x, z);
    }
}

fn classify_vegetation_area(way: &ProcessedWay) -> Option<VegetationKind> {
    if !is_closed_way(way) {
        return None;
    }

    match way.tags.get("landuse").map(String::as_str) {
        Some("forest") => return Some(VegetationKind::Woodland),
        Some("grass") | Some("meadow") | Some("recreation_ground") | Some("village_green") => {
            return Some(VegetationKind::Grass)
        }
        _ => {}
    }

    match way.tags.get("natural").map(String::as_str) {
        Some("wood") => return Some(VegetationKind::Woodland),
        Some("scrub") | Some("heath") => return Some(VegetationKind::Scrub),
        Some("grassland") => return Some(VegetationKind::Grass),
        _ => {}
    }

    match way.tags.get("leisure").map(String::as_str) {
        Some("park") | Some("garden") => Some(VegetationKind::Park),
        _ => None,
    }
}

fn is_closed_way(way: &ProcessedWay) -> bool {
    if way.nodes.len() < 4 {
        return false;
    }
    let first = &way.nodes[0];
    let last = way.nodes.last().unwrap();
    first.x == last.x && first.z == last.z
}

fn closed_footprint(way: &ProcessedWay) -> Vec<(f32, f32)> {
    way.nodes
        .iter()
        .map(|node| (node.x as f32, node.z as f32))
        .collect()
}

fn centroid(poly: &[(f32, f32)]) -> (f32, f32) {
    let n = poly.len().max(1);
    let sum_x: f32 = poly.iter().map(|p| p.0).sum();
    let sum_z: f32 = poly.iter().map(|p| p.1).sum();
    (sum_x / n as f32, sum_z / n as f32)
}

fn scatter_points(
    osm_id: u64,
    kind: VegetationKind,
    polygon: &[(f32, f32)],
) -> Vec<(i32, i32, VegetationProfile)> {
    let (min_x, max_x, min_z, max_z) = bounds(polygon);
    let step = kind.spacing();
    let max_count = kind.max_instances();
    let mut points = Vec::new();
    let mut sample_index = 0u64;
    let mut x = min_x + step * 0.5;

    while x <= max_x && points.len() < max_count {
        let mut z = min_z + step * 0.5;
        while z <= max_z && points.len() < max_count {
            let jitter_x = (trees::unit_hash(osm_id ^ sample_index) - 0.5) * step * 0.45;
            let jitter_z =
                (trees::unit_hash(osm_id ^ sample_index.rotate_left(17)) - 0.5) * step * 0.45;
            let px = x + jitter_x;
            let pz = z + jitter_z;
            if point_in_polygon((px, pz), polygon) {
                points.push((
                    px.round() as i32,
                    pz.round() as i32,
                    kind.profile_for(osm_id, sample_index),
                ));
            }
            sample_index += 1;
            z += step;
        }
        x += step;
    }

    points
}

fn bounds(poly: &[(f32, f32)]) -> (f32, f32, f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for &(x, z) in poly {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_z = min_z.min(z);
        max_z = max_z.max(z);
    }
    (min_x, max_x, min_z, max_z)
}

fn point_in_polygon(point: (f32, f32), polygon: &[(f32, f32)]) -> bool {
    let (px, pz) = point;
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let (xi, zi) = polygon[i];
        let (xj, zj) = polygon[j];
        let crosses = (zi > pz) != (zj > pz);
        if crosses {
            let x_intersect = (xj - xi) * (pz - zi) / ((zj - zi).abs().max(0.0001)) + xi;
            if px < x_intersect {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

impl VegetationKind {
    fn name(self) -> &'static str {
        match self {
            VegetationKind::Woodland => "woodland",
            VegetationKind::Park => "park",
            VegetationKind::Grass => "grass",
            VegetationKind::Scrub => "scrub",
        }
    }

    fn ground_material(self) -> MaterialType {
        match self {
            VegetationKind::Woodland | VegetationKind::Scrub => MaterialType::TreeLeaves,
            VegetationKind::Park | VegetationKind::Grass => MaterialType::TerrainGrass,
        }
    }

    fn spacing(self) -> f32 {
        match self {
            VegetationKind::Woodland => 48.0,
            VegetationKind::Park => 56.0,
            VegetationKind::Scrub => 42.0,
            VegetationKind::Grass => 72.0,
        }
    }

    fn max_instances(self) -> usize {
        match self {
            VegetationKind::Woodland => 18,
            VegetationKind::Park => 14,
            VegetationKind::Scrub => 18,
            VegetationKind::Grass => 8,
        }
    }

    fn profile_for(self, osm_id: u64, sample_index: u64) -> VegetationProfile {
        let h = trees::stable_hash(osm_id.wrapping_mul(97).wrapping_add(sample_index));
        match self {
            VegetationKind::Woodland => match h % 3 {
                0 => VegetationProfile::Broadleaf,
                1 => VegetationProfile::Conifer,
                _ => VegetationProfile::Shrub,
            },
            VegetationKind::Park => {
                if h % 4 == 0 {
                    VegetationProfile::Shrub
                } else {
                    VegetationProfile::Broadleaf
                }
            }
            VegetationKind::Grass => VegetationProfile::Shrub,
            VegetationKind::Scrub => {
                if h % 5 == 0 {
                    VegetationProfile::Broadleaf
                } else {
                    VegetationProfile::Shrub
                }
            }
        }
    }
}
