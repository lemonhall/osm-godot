//! Writes Godot .tres (text resource) files for materials.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Material type identifiers used by element processors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialType {
    BuildingWall,
    BuildingWallBrick,
    BuildingWallConcrete,
    BuildingWallCommercial,
    BuildingWallGlass,
    BuildingWallGreenhouse,
    BuildingWallStone,
    BuildingRoof,
    BuildingRoofDark,
    BuildingRoofTile,
    BuildingRoofMetal,
    BuildingWindow,
    BuildingDoor,
    BuildingTrim,
    RooftopEquipment,
    RoadAsphalt,
    RoadSidewalk,
    TerrainGrass,
    TerrainDirt,
    TerrainBuiltUp,
    Water,
    TreeLeaves,
    TreeTrunk,
    RailwayGravel,
}

impl MaterialType {
    /// File stem (without .tres extension).
    pub fn file_stem(&self) -> &'static str {
        match self {
            MaterialType::BuildingWall => "building_wall",
            MaterialType::BuildingWallBrick => "building_wall_brick",
            MaterialType::BuildingWallConcrete => "building_wall_concrete",
            MaterialType::BuildingWallCommercial => "building_wall_commercial",
            MaterialType::BuildingWallGlass => "building_wall_glass",
            MaterialType::BuildingWallGreenhouse => "building_wall_greenhouse",
            MaterialType::BuildingWallStone => "building_wall_stone",
            MaterialType::BuildingRoof => "building_roof",
            MaterialType::BuildingRoofDark => "building_roof_dark",
            MaterialType::BuildingRoofTile => "building_roof_tile",
            MaterialType::BuildingRoofMetal => "building_roof_metal",
            MaterialType::BuildingWindow => "building_window",
            MaterialType::BuildingDoor => "building_door",
            MaterialType::BuildingTrim => "building_trim",
            MaterialType::RooftopEquipment => "rooftop_equipment",
            MaterialType::RoadAsphalt => "road_asphalt",
            MaterialType::RoadSidewalk => "road_sidewalk",
            MaterialType::TerrainGrass => "terrain_grass",
            MaterialType::TerrainDirt => "terrain_dirt",
            MaterialType::TerrainBuiltUp => "terrain_built_up",
            MaterialType::Water => "water",
            MaterialType::TreeLeaves => "tree_leaves",
            MaterialType::TreeTrunk => "tree_trunk",
            MaterialType::RailwayGravel => "railway_gravel",
        }
    }

    /// Albedo color as (r, g, b, a) in 0.0–1.0 range.
    pub fn albedo(&self) -> (f32, f32, f32, f32) {
        match self {
            MaterialType::BuildingWall => (0.62, 0.58, 0.50, 1.0),
            MaterialType::BuildingWallBrick => (0.58, 0.31, 0.24, 1.0),
            MaterialType::BuildingWallConcrete => (0.54, 0.55, 0.52, 1.0),
            MaterialType::BuildingWallCommercial => (0.42, 0.46, 0.47, 1.0),
            MaterialType::BuildingWallGlass => (0.18, 0.31, 0.38, 1.0),
            MaterialType::BuildingWallGreenhouse => (0.62, 0.86, 0.78, 1.0),
            MaterialType::BuildingWallStone => (0.39, 0.38, 0.35, 1.0),
            MaterialType::BuildingRoof => (0.42, 0.12, 0.08, 1.0),
            MaterialType::BuildingRoofDark => (0.11, 0.12, 0.13, 1.0),
            MaterialType::BuildingRoofTile => (0.52, 0.16, 0.10, 1.0),
            MaterialType::BuildingRoofMetal => (0.36, 0.39, 0.40, 1.0),
            MaterialType::BuildingWindow => (0.12, 0.22, 0.30, 1.0),
            MaterialType::BuildingDoor => (0.28, 0.16, 0.08, 1.0),
            MaterialType::BuildingTrim => (0.78, 0.74, 0.65, 1.0),
            MaterialType::RooftopEquipment => (0.30, 0.32, 0.33, 1.0),
            MaterialType::RoadAsphalt => (0.09, 0.10, 0.11, 1.0),
            MaterialType::RoadSidewalk => (0.78, 0.74, 0.66, 1.0),
            MaterialType::TerrainGrass => (0.34, 0.68, 0.23, 1.0),
            MaterialType::TerrainDirt => (0.58, 0.43, 0.24, 1.0),
            MaterialType::TerrainBuiltUp => (0.66, 0.61, 0.52, 1.0),
            MaterialType::Water => (0.10, 0.30, 0.60, 0.7),
            MaterialType::TreeLeaves => (0.15, 0.45, 0.10, 1.0),
            MaterialType::TreeTrunk => (0.35, 0.22, 0.12, 1.0),
            MaterialType::RailwayGravel => (0.40, 0.38, 0.35, 1.0),
        }
    }

    /// Roughness value.
    pub fn roughness(&self) -> f32 {
        match self {
            MaterialType::BuildingWall
            | MaterialType::BuildingWallBrick
            | MaterialType::BuildingWallConcrete
            | MaterialType::BuildingWallCommercial
            | MaterialType::BuildingWallStone => 0.85,
            MaterialType::BuildingWallGlass
            | MaterialType::BuildingWallGreenhouse
            | MaterialType::BuildingWindow => 0.18,
            MaterialType::BuildingRoof
            | MaterialType::BuildingRoofDark
            | MaterialType::BuildingRoofTile
            | MaterialType::BuildingRoofMetal => 0.70,
            MaterialType::BuildingDoor => 0.75,
            MaterialType::BuildingTrim => 0.80,
            MaterialType::RooftopEquipment => 0.65,
            MaterialType::RoadAsphalt => 0.90,
            MaterialType::RoadSidewalk => 0.80,
            MaterialType::TerrainGrass => 0.95,
            MaterialType::TerrainDirt => 0.90,
            MaterialType::TerrainBuiltUp => 0.85,
            MaterialType::Water => 0.10,
            MaterialType::TreeLeaves => 0.90,
            MaterialType::TreeTrunk => 0.80,
            MaterialType::RailwayGravel => 0.85,
        }
    }

    /// Metallic value.
    pub fn metallic(&self) -> f32 {
        match self {
            MaterialType::Water => 0.1,
            MaterialType::BuildingWallGlass
            | MaterialType::BuildingWallGreenhouse
            | MaterialType::BuildingWindow => 0.0,
            _ => 0.0,
        }
    }
}

/// All material types defined.
pub const ALL_MATERIALS: &[MaterialType] = &[
    MaterialType::BuildingWall,
    MaterialType::BuildingWallBrick,
    MaterialType::BuildingWallConcrete,
    MaterialType::BuildingWallCommercial,
    MaterialType::BuildingWallGlass,
    MaterialType::BuildingWallGreenhouse,
    MaterialType::BuildingWallStone,
    MaterialType::BuildingRoof,
    MaterialType::BuildingRoofDark,
    MaterialType::BuildingRoofTile,
    MaterialType::BuildingRoofMetal,
    MaterialType::BuildingWindow,
    MaterialType::BuildingDoor,
    MaterialType::BuildingTrim,
    MaterialType::RooftopEquipment,
    MaterialType::RoadAsphalt,
    MaterialType::RoadSidewalk,
    MaterialType::TerrainGrass,
    MaterialType::TerrainDirt,
    MaterialType::TerrainBuiltUp,
    MaterialType::Water,
    MaterialType::TreeLeaves,
    MaterialType::TreeTrunk,
    MaterialType::RailwayGravel,
];

/// Write a single .tres material file.
pub fn write_material(mat_type: MaterialType, materials_dir: &Path) -> io::Result<()> {
    let (r, g, b, a) = mat_type.albedo();
    let roughness = mat_type.roughness();
    let metallic = mat_type.metallic();
    let filename = format!("{}.tres", mat_type.file_stem());
    let path = materials_dir.join(&filename);

    let mut f = fs::File::create(&path)?;

    writeln!(
        f,
        "[gd_resource type=\"StandardMaterial3D\" load_steps=0 format=3 uid=\"uid://{}\"]",
        material_uid(mat_type)
    )?;
    writeln!(f)?;
    writeln!(f, "[resource]")?;
    writeln!(f, "albedo_color = Color({}, {}, {}, {})", r, g, b, a)?;
    writeln!(f, "diffuse_mode = 3")?;
    writeln!(f, "specular_mode = 1")?;
    writeln!(f, "roughness = {}", roughness)?;
    writeln!(f, "metallic = {}", metallic)?;

    if matches!(
        mat_type,
        MaterialType::BuildingWindow | MaterialType::BuildingWallGlass | MaterialType::BuildingWallGreenhouse
    ) {
        writeln!(f, "emission_enabled = true")?;
        writeln!(f, "emission = Color({}, {}, {}, 1)", r, g, b)?;
        writeln!(f, "emission_energy_multiplier = 0.18")?;
    }

    // Water gets transparency (Godot 4.x StandardMaterial3D)
    if a < 1.0 {
        writeln!(f, "transparency = 1")?;
    }

    Ok(())
}

/// Write all material files to the materials directory.
pub fn write_all_materials(materials_dir: &Path) -> io::Result<Vec<(MaterialType, u32)>> {
    fs::create_dir_all(materials_dir)?;

    let mut id_map = Vec::new();
    for (i, mat) in ALL_MATERIALS.iter().enumerate() {
        write_material(*mat, materials_dir)?;
        id_map.push((*mat, (i + 1) as u32)); // ext_resource IDs start at 1
    }

    Ok(id_map)
}

/// Generate a stable uid string for a material.
fn material_uid(mat: MaterialType) -> String {
    // Simple stable UID — Godot 4 format
    let hash = mat
        .file_stem()
        .bytes()
        .fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
    format!("c{hash:013x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn building_materials_are_not_whitebox_defaults() {
        let wall = MaterialType::BuildingWall.albedo();
        let roof = MaterialType::BuildingRoof.albedo();

        assert!(wall.0 < 0.75 && wall.1 < 0.75 && wall.2 < 0.75);
        assert_ne!(wall, roof);
    }

    #[test]
    fn terrain_and_roads_use_bright_readable_colors() {
        let grass = MaterialType::TerrainGrass.albedo();
        let sidewalk = MaterialType::RoadSidewalk.albedo();

        assert!(grass.1 >= 0.65, "grass should read as saturated green: {grass:?}");
        assert!(
            sidewalk.0 >= 0.72 && sidewalk.1 >= 0.68,
            "sidewalk should contrast against asphalt and buildings: {sidewalk:?}"
        );
    }

    #[test]
    fn generated_materials_use_toon_shading() {
        let tmp = tempfile::tempdir().unwrap();

        write_material(MaterialType::TerrainGrass, tmp.path()).unwrap();

        let material = std::fs::read_to_string(tmp.path().join("terrain_grass.tres")).unwrap();
        assert!(material.contains("diffuse_mode = 3"));
        assert!(material.contains("specular_mode = 1"));
    }
}
