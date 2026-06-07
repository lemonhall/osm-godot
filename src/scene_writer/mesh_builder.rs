//! Mesh construction utilities — bridges OSM element geometry to Godot MeshData.
//!
//! Provides helper functions that element processors call to build meshes
//! from OSM data (polygon footprints, centerlines, height tags, etc.).

use crate::coordinate_system::cartesian::XZPoint;
use crate::scene_writer::geometry::{self, MeshData};

/// Scale factor: arnis block units → Godot meters.
const BLOCK_TO_METERS: f32 = 0.5;
const MAX_BUILDING_HEIGHT_METERS: f32 = 700.0;

/// Convert a list of arnis XZPoints to (x, z) tuples in Godot space.
pub fn polygon_godot(points: &[XZPoint], godot_scale: f32) -> Vec<(f32, f32)> {
    points
        .iter()
        .map(|p| (p.x as f32 * godot_scale, -(p.z as f32) * godot_scale))
        .collect()
}

/// Convert arnis XZPoint to Godot (x, z).
pub fn point_godot(p: XZPoint, godot_scale: f32) -> (f32, f32) {
    (p.x as f32 * godot_scale, -(p.z as f32) * godot_scale)
}

/// Parse building height from OSM tags. Returns height in Godot meters.
/// Respects: building:height, height, building:levels (×3m), levels (×3m).
pub fn building_height(tags: &std::collections::HashMap<String, String>, _godot_scale: f32) -> f32 {
    // Try explicit height
    if let Some(h) = tags.get("height") {
        if let Ok(meters) = h.trim_end_matches(" m").parse::<f32>() {
            return clamp_building_height(meters);
        }
    }
    if let Some(h) = tags.get("building:height") {
        if let Ok(meters) = h.trim_end_matches(" m").parse::<f32>() {
            return clamp_building_height(meters);
        }
    }

    // Try levels
    let levels = tags
        .get("building:levels")
        .or_else(|| tags.get("levels"))
        .and_then(|l| l.parse::<f32>().ok())
        .unwrap_or(2.0);

    clamp_building_height(levels * 3.0) // ~3m per level
}

fn clamp_building_height(height: f32) -> f32 {
    if !height.is_finite() {
        return MAX_BUILDING_HEIGHT_METERS;
    }
    height.clamp(1.0, MAX_BUILDING_HEIGHT_METERS)
}

/// Parse road width from highway tags. Returns width in Godot meters.
pub fn highway_width(tags: &std::collections::HashMap<String, String>) -> f32 {
    let base = match tags.get("highway").map(String::as_str) {
        Some("motorway") | Some("trunk") => 12.0,
        Some("primary") => 10.0,
        Some("secondary") => 8.0,
        Some("tertiary") => 6.0,
        Some("residential") | Some("living_street") => 5.0,
        Some("service") | Some("alley") => 3.0,
        Some("footway") | Some("path") | Some("cycleway") => 1.5,
        Some("pedestrian") => 8.0,
        _ => 5.0,
    };

    // Override with explicit width tag if present
    if let Some(w) = tags.get("width") {
        if let Ok(meters) = w.trim_end_matches(" m").parse::<f32>() {
            return meters;
        }
    }

    base * BLOCK_TO_METERS
}

/// Build a building mesh: walls from footprint + roof.
/// `footprint` is in Godot (x, z) coords, `base_y` is ground elevation in Godot meters.
pub fn build_building(
    footprint: &[(f32, f32)],
    _base_y: f32,
    height: f32,
    roof_type: &str,
) -> MeshData {
    let wall_thickness = 0.3; // 30cm walls
    let mut mesh = geometry::make_wall_outline(footprint, height, wall_thickness);

    // Roof
    let roof = match roof_type {
        "gabled" | "gambrel" | "mansard" => {
            geometry::make_roof_gabled(footprint, height, height + 3.0)
        }
        "pyramidal" | "hipped" | "dome" | "onion" => {
            geometry::make_roof_gabled(footprint, height, height + 3.0)
        }
        _ => geometry::make_roof_flat(footprint, height),
    };

    mesh.append(&roof, (0.0, 0.0, 0.0));
    mesh
}

/// Build a tree mesh at the given location.
/// Returns (trunk_mesh, canopy_mesh) with appropriate positions.
pub fn build_tree(
    trunk_radius: f32,
    trunk_height: f32,
    canopy_radius: f32,
) -> (MeshData, MeshData) {
    let trunk = geometry::make_cylinder(trunk_radius, trunk_height, 8);
    let canopy = geometry::make_cone(canopy_radius, canopy_radius * 3.0, 8);
    (trunk, canopy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn tags(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn building_height_clamps_absurd_osm_levels() {
        let height = building_height(&tags(&[("building:levels", "1235678911121415")]), 0.5);

        assert!(height <= 700.0, "height should be bounded, got {height}");
    }
}
