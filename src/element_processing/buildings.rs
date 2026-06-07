//! Building processor — wall outline + roof as separate meshes.

use crate::osm_parser::ProcessedWay;
use crate::scene_writer::geometry::{self, MeshData};
use crate::scene_writer::mesh_builder;
use crate::scene_writer::tres_writer::MaterialType;
use crate::scene_writer::SceneWriter;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingCategory {
    Residential,
    Commercial,
    Industrial,
    Civic,
    Religious,
    HighRise,
    Garage,
    Greenhouse,
    Historic,
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallDepthStyle {
    None,
    SubtlePilasters,
    ModernPillars,
    InstitutionalBands,
    IndustrialBeams,
    HistoricOrnate,
    ReligiousButtress,
    SkyscraperFins,
    GlassCurtain,
}

#[derive(Debug, Clone, Copy)]
pub struct BuildingStyle {
    pub category: BuildingCategory,
    pub wall_material: MaterialType,
    pub roof_material: MaterialType,
    pub depth_style: WallDepthStyle,
    pub window_spacing: f32,
    pub window_width: f32,
    pub window_height: f32,
    pub door_width: f32,
    pub has_windows: bool,
    pub trim_every_floor: bool,
    pub parapet: bool,
}

pub struct BuildingDetailMeshes {
    pub windows: MeshData,
    pub door: MeshData,
    pub trim: MeshData,
    pub rooftop: MeshData,
}

pub fn classify_building_style(
    tags: &HashMap<String, String>,
    height: f32,
    element_id: u64,
) -> BuildingStyle {
    let building = tags.get("building").map(String::as_str).unwrap_or("yes");
    let amenity = tags.get("amenity").map(String::as_str).unwrap_or("");
    let historic = tags.get("historic").map(String::as_str).unwrap_or("");
    let levels = tags
        .get("building:levels")
        .or_else(|| tags.get("levels"))
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or((height / 3.0).max(1.0));

    let category = if height >= 36.0 || levels >= 12.0 {
        BuildingCategory::HighRise
    } else if matches!(
        amenity,
        "school" | "kindergarten" | "college" | "university" | "hospital"
    ) || matches!(
        building,
        "school"
            | "kindergarten"
            | "college"
            | "university"
            | "hospital"
            | "public"
            | "government"
            | "civic"
    ) {
        BuildingCategory::Civic
    } else if amenity == "place_of_worship"
        || matches!(
            building,
            "church" | "cathedral" | "chapel" | "temple" | "mosque"
        )
    {
        BuildingCategory::Religious
    } else if !historic.is_empty() || matches!(building, "castle" | "ruins" | "fort") {
        BuildingCategory::Historic
    } else if matches!(building, "garage" | "garages" | "carport") {
        BuildingCategory::Garage
    } else if matches!(building, "greenhouse" | "glasshouse") {
        BuildingCategory::Greenhouse
    } else if matches!(building, "industrial" | "factory" | "warehouse" | "hangar") {
        BuildingCategory::Industrial
    } else if matches!(
        building,
        "commercial" | "retail" | "office" | "hotel" | "supermarket"
    ) {
        BuildingCategory::Commercial
    } else if matches!(
        building,
        "house"
            | "detached"
            | "semidetached_house"
            | "terrace"
            | "residential"
            | "apartments"
            | "dormitory"
    ) {
        BuildingCategory::Residential
    } else {
        BuildingCategory::Default
    };

    let variant = (element_id % 3) as f32;
    match category {
        BuildingCategory::Residential => BuildingStyle {
            category,
            wall_material: MaterialType::BuildingWallBrick,
            roof_material: MaterialType::BuildingRoofTile,
            depth_style: WallDepthStyle::SubtlePilasters,
            window_spacing: 3.0 + variant * 0.25,
            window_width: 0.95,
            window_height: 1.15,
            door_width: 1.25,
            has_windows: true,
            trim_every_floor: false,
            parapet: false,
        },
        BuildingCategory::Commercial => BuildingStyle {
            category,
            wall_material: MaterialType::BuildingWallCommercial,
            roof_material: MaterialType::BuildingRoofDark,
            depth_style: WallDepthStyle::ModernPillars,
            window_spacing: 3.0,
            window_width: 1.45,
            window_height: 1.25,
            door_width: 1.6,
            has_windows: true,
            trim_every_floor: true,
            parapet: true,
        },
        BuildingCategory::Industrial => BuildingStyle {
            category,
            wall_material: MaterialType::BuildingWallConcrete,
            roof_material: MaterialType::BuildingRoofMetal,
            depth_style: WallDepthStyle::IndustrialBeams,
            window_spacing: 4.5,
            window_width: 1.8,
            window_height: 0.9,
            door_width: 2.2,
            has_windows: true,
            trim_every_floor: false,
            parapet: true,
        },
        BuildingCategory::Civic => BuildingStyle {
            category,
            wall_material: MaterialType::BuildingWallConcrete,
            roof_material: MaterialType::BuildingRoofDark,
            depth_style: WallDepthStyle::InstitutionalBands,
            window_spacing: 3.5,
            window_width: 1.15,
            window_height: 1.3,
            door_width: 1.7,
            has_windows: true,
            trim_every_floor: true,
            parapet: true,
        },
        BuildingCategory::Religious => BuildingStyle {
            category,
            wall_material: MaterialType::BuildingWallBrick,
            roof_material: MaterialType::BuildingRoofDark,
            depth_style: WallDepthStyle::ReligiousButtress,
            window_spacing: 4.0,
            window_width: 0.75,
            window_height: 1.7,
            door_width: 1.4,
            has_windows: true,
            trim_every_floor: false,
            parapet: false,
        },
        BuildingCategory::HighRise => BuildingStyle {
            category,
            wall_material: MaterialType::BuildingWallGlass,
            roof_material: MaterialType::BuildingRoofDark,
            depth_style: WallDepthStyle::SkyscraperFins,
            window_spacing: 2.6,
            window_width: 1.6,
            window_height: 1.35,
            door_width: 1.8,
            has_windows: true,
            trim_every_floor: true,
            parapet: true,
        },
        BuildingCategory::Garage => BuildingStyle {
            category,
            wall_material: MaterialType::BuildingWallConcrete,
            roof_material: MaterialType::BuildingRoofMetal,
            depth_style: WallDepthStyle::None,
            window_spacing: 5.0,
            window_width: 0.8,
            window_height: 0.55,
            door_width: 2.6,
            has_windows: false,
            trim_every_floor: false,
            parapet: false,
        },
        BuildingCategory::Greenhouse => BuildingStyle {
            category,
            wall_material: MaterialType::BuildingWallGreenhouse,
            roof_material: MaterialType::BuildingRoofMetal,
            depth_style: WallDepthStyle::GlassCurtain,
            window_spacing: 5.0,
            window_width: 1.0,
            window_height: 1.0,
            door_width: 1.15,
            has_windows: false,
            trim_every_floor: false,
            parapet: false,
        },
        BuildingCategory::Historic => BuildingStyle {
            category,
            wall_material: MaterialType::BuildingWallStone,
            roof_material: MaterialType::BuildingRoofDark,
            depth_style: WallDepthStyle::HistoricOrnate,
            window_spacing: 4.2,
            window_width: 0.8,
            window_height: 1.45,
            door_width: 1.35,
            has_windows: true,
            trim_every_floor: false,
            parapet: false,
        },
        BuildingCategory::Default => BuildingStyle {
            category,
            wall_material: MaterialType::BuildingWall,
            roof_material: MaterialType::BuildingRoof,
            depth_style: WallDepthStyle::None,
            window_spacing: 3.5,
            window_width: 1.0,
            window_height: 1.1,
            door_width: 1.2,
            has_windows: true,
            trim_every_floor: false,
            parapet: height > 9.0,
        },
    }
}

pub fn generate_building(scene: &mut SceneWriter, way: &ProcessedWay, godot_scale: f32) {
    if way.nodes.len() < 3 {
        return;
    }

    // Footprint in arnis coords
    let fp: Vec<(f32, f32)> = way.nodes.iter().map(|n| (n.x as f32, n.z as f32)).collect();
    let fp = close_poly(fp);
    if fp.len() < 3 {
        return;
    }

    // Filter tiny buildings: area < 2 arnis_units² ≈ 0.5 m²
    let area = polygon_area(&fp);
    if area < 2.0 {
        return;
    }

    // Height in meters
    let height = mesh_builder::building_height(&way.tags, godot_scale).max(2.5);

    // Centroids
    let (cx, cz) = centroid(&fp);

    // Roof type
    let roof_type = way
        .tags
        .get("roof:shape")
        .or_else(|| way.tags.get("roof:type"))
        .map(|s| s.as_str())
        .unwrap_or("flat");
    let style = classify_building_style(&way.tags, height, way.id);

    // Convert to local Godot coords
    let fp_local: Vec<(f32, f32)> = fp
        .iter()
        .map(|&(x, z)| ((x - cx) * godot_scale, -(z - cz) * godot_scale))
        .collect();

    // ── Wall mesh ──
    let wall = geometry::make_wall_outline(&fp_local, height, 0.3);

    // ── Roof mesh ──
    let roof = match roof_type {
        "gabled" | "gambrel" | "mansard" | "pyramidal" | "hipped" | "dome" | "onion" => {
            geometry::make_roof_gabled(&fp_local, height, height + 3.0)
        }
        _ => geometry::make_roof_flat(&fp_local, height),
    };

    let wx = cx.round() as i32;
    let wz = cz.round() as i32;

    scene.add_mesh(
        format!("BuildingWall_{}", way.id),
        wall,
        style.wall_material,
        wx,
        wz,
    );
    scene.add_mesh(
        format!("BuildingRoof_{}", way.id),
        roof,
        style.roof_material,
        wx,
        wz,
    );

    let details = make_building_detail_meshes(&fp_local, height, roof_type, &style);
    if details.windows.vertex_count() > 0 {
        scene.add_mesh(
            format!("BuildingWindows_{}", way.id),
            details.windows,
            MaterialType::BuildingWindow,
            wx,
            wz,
        );
    }
    if details.door.vertex_count() > 0 {
        scene.add_mesh(
            format!("BuildingDoor_{}", way.id),
            details.door,
            MaterialType::BuildingDoor,
            wx,
            wz,
        );
    }
    if details.trim.vertex_count() > 0 {
        scene.add_mesh(
            format!("BuildingTrim_{}", way.id),
            details.trim,
            MaterialType::BuildingTrim,
            wx,
            wz,
        );
    }
    if details.rooftop.vertex_count() > 0 {
        scene.add_mesh(
            format!("BuildingRooftop_{}", way.id),
            details.rooftop,
            MaterialType::RooftopEquipment,
            wx,
            wz,
        );
    }
}

pub fn make_building_detail_meshes(
    polygon: &[(f32, f32)],
    height: f32,
    roof_type: &str,
    style: &BuildingStyle,
) -> BuildingDetailMeshes {
    let mut windows = MeshData::new();
    let mut door = MeshData::new();
    let mut trim = MeshData::new();
    let mut rooftop = MeshData::new();

    if polygon.len() < 3 || height < 2.5 {
        return BuildingDetailMeshes {
            windows,
            door,
            trim,
            rooftop,
        };
    }

    let floor_count = (height / 3.0).floor().max(1.0) as u32;
    let signed = signed_area(polygon);
    let mut longest_edge: Option<((f32, f32), (f32, f32), f32)> = None;

    for edge in polygon.windows(2) {
        let (x0, z0) = edge[0];
        let (x1, z1) = edge[1];
        let dx = x1 - x0;
        let dz = z1 - z0;
        let len = (dx * dx + dz * dz).sqrt();
        if len < style.window_width + 0.6 {
            continue;
        }

        if longest_edge.map(|(_, _, best)| len > best).unwrap_or(true) {
            longest_edge = Some(((x0, z0), (x1, z1), len));
        }

        let tangent = (dx / len, dz / len);
        let normal = outward_normal(tangent, signed);
        let window_count = (len / style.window_spacing).floor().max(1.0) as u32;
        let step = len / (window_count + 1) as f32;

        for floor in 0..floor_count {
            let y0 = floor as f32 * 3.0 + 1.05;
            let y1 = (y0 + style.window_height).min(height - 0.35);
            if y1 > y0 && style.has_windows {
                for i in 0..window_count {
                    let dist = step * (i + 1) as f32;
                    let cx = x0 + tangent.0 * dist;
                    let cz = z0 + tangent.1 * dist;
                    push_wall_panel(
                        &mut windows,
                        (cx, cz),
                        tangent,
                        normal,
                        style.window_width.min(len * 0.35),
                        y0,
                        y1,
                        0.08,
                    );
                }
            }

            if style.trim_every_floor && floor > 0 {
                let cy = floor as f32 * 3.0;
                push_wall_panel(
                    &mut trim,
                    ((x0 + x1) * 0.5, (z0 + z1) * 0.5),
                    tangent,
                    normal,
                    len * 0.96,
                    cy,
                    (cy + 0.14).min(height),
                    0.10,
                );
            }
        }

        push_depth_features(
            &mut trim,
            style.depth_style,
            (x0, z0),
            tangent,
            normal,
            len,
            height,
            style.window_spacing,
        );
    }

    if let Some(((x0, z0), (x1, z1), len)) = longest_edge {
        let tangent = ((x1 - x0) / len, (z1 - z0) / len);
        let normal = outward_normal(tangent, signed);
        push_wall_panel(
            &mut door,
            ((x0 + x1) * 0.5, (z0 + z1) * 0.5),
            tangent,
            normal,
            style.door_width.min(len * 0.45),
            0.05,
            2.35_f32.min(height - 0.15),
            0.12,
        );
    }

    let roof_flat = matches!(roof_type, "flat" | "roof" | "yes") || roof_type.trim().is_empty();
    if style.parapet && roof_flat {
        let parapet = geometry::make_wall_outline(polygon, 0.55, 0.18);
        rooftop.append(&parapet, (0.0, height, 0.0));

        let (min_x, max_x, min_z, max_z) = bbox(polygon);
        let equipment = geometry::make_box(
            ((max_x - min_x) * 0.14).clamp(0.7, 2.2),
            0.75,
            ((max_z - min_z) * 0.12).clamp(0.7, 1.8),
        );
        rooftop.append(
            &equipment,
            ((min_x + max_x) * 0.5, height + 0.05, (min_z + max_z) * 0.5),
        );
    }

    BuildingDetailMeshes {
        windows,
        door,
        trim,
        rooftop,
    }
}

fn push_depth_features(
    trim: &mut MeshData,
    depth_style: WallDepthStyle,
    start: (f32, f32),
    tangent: (f32, f32),
    normal: (f32, f32),
    len: f32,
    height: f32,
    spacing: f32,
) {
    if depth_style == WallDepthStyle::None || len < 2.0 {
        return;
    }

    let (panel_width, panel_height, feature_spacing, include_end_posts) = match depth_style {
        WallDepthStyle::SubtlePilasters => (0.18, height, spacing * 1.25, false),
        WallDepthStyle::ModernPillars => (0.24, height, spacing * 1.4, true),
        WallDepthStyle::InstitutionalBands => (0.28, height, spacing * 1.6, true),
        WallDepthStyle::IndustrialBeams => (0.42, height, len, true),
        WallDepthStyle::HistoricOrnate => (0.30, height, spacing * 1.35, true),
        WallDepthStyle::ReligiousButtress => (0.48, height * 0.72, spacing * 1.7, true),
        WallDepthStyle::SkyscraperFins => (0.16, height, spacing, true),
        WallDepthStyle::GlassCurtain => (0.20, height, len, true),
        WallDepthStyle::None => return,
    };

    if include_end_posts {
        for dist in [
            0.08_f32.max(panel_width),
            (len - 0.08_f32.max(panel_width)).max(0.0),
        ] {
            let cx = start.0 + tangent.0 * dist;
            let cz = start.1 + tangent.1 * dist;
            push_wall_panel(
                trim,
                (cx, cz),
                tangent,
                normal,
                panel_width,
                0.05,
                height,
                0.14,
            );
        }
    }

    let count = (len / feature_spacing).floor() as u32;
    if count == 0 {
        return;
    }

    let step = len / (count + 1) as f32;
    for i in 0..count {
        if depth_style == WallDepthStyle::ReligiousButtress && i % 2 == 1 {
            continue;
        }
        let dist = step * (i + 1) as f32;
        let cx = start.0 + tangent.0 * dist;
        let cz = start.1 + tangent.1 * dist;
        push_wall_panel(
            trim,
            (cx, cz),
            tangent,
            normal,
            panel_width,
            0.05,
            panel_height.max(1.0),
            0.14,
        );
    }

    if matches!(
        depth_style,
        WallDepthStyle::HistoricOrnate | WallDepthStyle::ReligiousButtress
    ) {
        push_wall_panel(
            trim,
            (
                start.0 + tangent.0 * (len * 0.5),
                start.1 + tangent.1 * (len * 0.5),
            ),
            tangent,
            normal,
            len * 0.92,
            (height - 0.28).max(0.05),
            height,
            0.16,
        );
    }
}

fn push_wall_panel(
    mesh: &mut MeshData,
    center_xz: (f32, f32),
    tangent: (f32, f32),
    normal: (f32, f32),
    width: f32,
    y0: f32,
    y1: f32,
    offset: f32,
) {
    let half = width * 0.5;
    let cx = center_xz.0 + normal.0 * offset;
    let cz = center_xz.1 + normal.1 * offset;
    let lx = cx - tangent.0 * half;
    let lz = cz - tangent.1 * half;
    let rx = cx + tangent.0 * half;
    let rz = cz + tangent.1 * half;

    let base = mesh.vertex_count() as u32;
    mesh.vertices
        .extend_from_slice(&[lx, y0, lz, rx, y0, rz, rx, y1, rz, lx, y1, lz]);
    for _ in 0..4 {
        mesh.normals.extend_from_slice(&[normal.0, 0.0, normal.1]);
    }
    mesh.uvs
        .extend_from_slice(&[0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0]);
    mesh.indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);

    let back = mesh.vertex_count() as u32;
    mesh.vertices
        .extend_from_slice(&[lx, y0, lz, lx, y1, lz, rx, y1, rz, rx, y0, rz]);
    for _ in 0..4 {
        mesh.normals.extend_from_slice(&[-normal.0, 0.0, -normal.1]);
    }
    mesh.uvs
        .extend_from_slice(&[0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0]);
    mesh.indices
        .extend_from_slice(&[back, back + 1, back + 2, back, back + 2, back + 3]);
}

fn outward_normal(tangent: (f32, f32), signed_area: f32) -> (f32, f32) {
    if signed_area >= 0.0 {
        (tangent.1, -tangent.0)
    } else {
        (-tangent.1, tangent.0)
    }
}

fn signed_area(p: &[(f32, f32)]) -> f32 {
    let mut a = 0.0;
    for i in 0..p.len() {
        let j = (i + 1) % p.len();
        a += p[i].0 * p[j].1 - p[j].0 * p[i].1;
    }
    a * 0.5
}

fn bbox(polygon: &[(f32, f32)]) -> (f32, f32, f32, f32) {
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;
    for &(x, z) in polygon {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_z = min_z.min(z);
        max_z = max_z.max(z);
    }
    (min_x, max_x, min_z, max_z)
}

fn close_poly(mut p: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    if p.len() < 2 {
        return p;
    }
    let f = p[0];
    let l = *p.last().unwrap();
    if (f.0 - l.0).abs() > 0.01 || (f.1 - l.1).abs() > 0.01 {
        p.push(f);
    }
    p
}

fn centroid(p: &[(f32, f32)]) -> (f32, f32) {
    let n = p.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mut a = 0.0f64;
    let mut cx = 0.0f64;
    let mut cz = 0.0f64;
    for i in 0..n {
        let j = (i + 1) % n;
        let (x0, z0) = (p[i].0 as f64, p[i].1 as f64);
        let (x1, z1) = (p[j].0 as f64, p[j].1 as f64);
        let cross = x0 * z1 - x1 * z0;
        a += cross;
        cx += (x0 + x1) * cross;
        cz += (z0 + z1) * cross;
    }
    if a.abs() < 1e-6 {
        let sx: f64 = p.iter().map(|p| p.0 as f64).sum();
        let sz: f64 = p.iter().map(|p| p.1 as f64).sum();
        return ((sx / n as f64) as f32, (sz / n as f64) as f32);
    }
    let inv = 1.0 / (3.0 * a);
    ((cx * inv) as f32, (cz * inv) as f32)
}

fn polygon_area(p: &[(f32, f32)]) -> f64 {
    let n = p.len();
    if n < 3 {
        return 0.0;
    }
    let mut a = 0.0f64;
    for i in 0..n {
        let j = (i + 1) % n;
        a += p[i].0 as f64 * p[j].1 as f64 - p[j].0 as f64 * p[i].1 as f64;
    }
    a.abs() * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn tags(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn style_infers_civic_highrise_and_residential_buildings() {
        let school = classify_building_style(
            &tags(&[("building", "school"), ("amenity", "school")]),
            9.0,
            11,
        );
        assert_eq!(school.category, BuildingCategory::Civic);
        assert_eq!(school.wall_material, MaterialType::BuildingWallConcrete);
        assert_eq!(school.roof_material, MaterialType::BuildingRoofDark);

        let highrise = classify_building_style(
            &tags(&[("building", "apartments"), ("building:levels", "18")]),
            54.0,
            12,
        );
        assert_eq!(highrise.category, BuildingCategory::HighRise);
        assert_eq!(highrise.wall_material, MaterialType::BuildingWallGlass);
        assert!(highrise.window_spacing <= 3.0);

        let house = classify_building_style(&tags(&[("building", "house")]), 6.0, 13);
        assert_eq!(house.category, BuildingCategory::Residential);
        assert_eq!(house.wall_material, MaterialType::BuildingWallBrick);
        assert_eq!(house.roof_material, MaterialType::BuildingRoofTile);
    }

    #[test]
    fn facade_details_create_windows_door_bands_and_rooftop_parts() {
        let footprint = vec![
            (-8.0, -5.0),
            (8.0, -5.0),
            (8.0, 5.0),
            (-8.0, 5.0),
            (-8.0, -5.0),
        ];
        let style = classify_building_style(
            &tags(&[("building", "commercial"), ("building:levels", "4")]),
            12.0,
            99,
        );

        let details = make_building_detail_meshes(&footprint, 12.0, "flat", &style);

        assert!(details.windows.vertex_count() >= 64);
        assert!(details.door.vertex_count() >= 4);
        assert!(details.trim.vertex_count() >= 32);
        assert!(details.rooftop.vertex_count() >= 24);
    }

    #[test]
    fn style_infers_specialized_arnis_like_building_types() {
        let garage = classify_building_style(&tags(&[("building", "garage")]), 3.0, 21);
        assert_eq!(garage.category, BuildingCategory::Garage);
        assert_eq!(garage.wall_material, MaterialType::BuildingWallConcrete);
        assert!(!garage.has_windows);
        assert!(garage.door_width >= 2.4);

        let greenhouse = classify_building_style(&tags(&[("building", "greenhouse")]), 3.0, 22);
        assert_eq!(greenhouse.category, BuildingCategory::Greenhouse);
        assert_eq!(
            greenhouse.wall_material,
            MaterialType::BuildingWallGreenhouse
        );
        assert!(!greenhouse.has_windows);

        let historic = classify_building_style(
            &tags(&[("historic", "castle"), ("building", "yes")]),
            12.0,
            23,
        );
        assert_eq!(historic.category, BuildingCategory::Historic);
        assert_eq!(historic.wall_material, MaterialType::BuildingWallStone);
        assert_eq!(historic.depth_style, WallDepthStyle::HistoricOrnate);
    }

    #[test]
    fn facade_depth_styles_add_category_specific_vertical_detail() {
        let footprint = vec![
            (-9.0, -4.0),
            (9.0, -4.0),
            (9.0, 4.0),
            (-9.0, 4.0),
            (-9.0, -4.0),
        ];
        let highrise = classify_building_style(
            &tags(&[("building", "office"), ("building:levels", "16")]),
            48.0,
            24,
        );
        let details = make_building_detail_meshes(&footprint, 48.0, "flat", &highrise);

        assert_eq!(highrise.depth_style, WallDepthStyle::SkyscraperFins);
        assert!(details.trim.vertex_count() >= 256);
    }
}
