//! Writes Godot .tres (text resource) files for materials.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Material type identifiers used by element processors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialType {
    BuildingWall,
    BuildingRoof,
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
            MaterialType::BuildingRoof => "building_roof",
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
            MaterialType::BuildingWall => (0.85, 0.82, 0.75, 1.0),
            MaterialType::BuildingRoof => (0.35, 0.18, 0.10, 1.0),
            MaterialType::RoadAsphalt => (0.15, 0.15, 0.15, 1.0),
            MaterialType::RoadSidewalk => (0.65, 0.65, 0.65, 1.0),
            MaterialType::TerrainGrass => (0.25, 0.55, 0.15, 1.0),
            MaterialType::TerrainDirt => (0.45, 0.35, 0.20, 1.0),
            MaterialType::TerrainBuiltUp => (0.55, 0.50, 0.45, 1.0),
            MaterialType::Water => (0.10, 0.30, 0.60, 0.7),
            MaterialType::TreeLeaves => (0.15, 0.45, 0.10, 1.0),
            MaterialType::TreeTrunk => (0.35, 0.22, 0.12, 1.0),
            MaterialType::RailwayGravel => (0.40, 0.38, 0.35, 1.0),
        }
    }

    /// Roughness value.
    pub fn roughness(&self) -> f32 {
        match self {
            MaterialType::BuildingWall => 0.85,
            MaterialType::BuildingRoof => 0.70,
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
            _ => 0.0,
        }
    }
}

/// All material types defined.
pub const ALL_MATERIALS: &[MaterialType] = &[
    MaterialType::BuildingWall,
    MaterialType::BuildingRoof,
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

    writeln!(f, "[gd_resource type=\"StandardMaterial3D\" load_steps=0 format=3 uid=\"uid://{}\"]", material_uid(mat_type))?;
    writeln!(f)?;
    writeln!(f, "[resource]")?;
    writeln!(f, "albedo_color = Color({}, {}, {}, {})", r, g, b, a)?;
    writeln!(f, "roughness = {}", roughness)?;
    writeln!(f, "metallic = {}", metallic)?;

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
    let hash = mat.file_stem().bytes().fold(0u64, |h, b| {
        h.wrapping_mul(31).wrapping_add(b as u64)
    });
    format!("c{hash:013x}")
}
