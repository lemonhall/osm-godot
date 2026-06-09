//! Main dispatch loop — processes all OSM elements and generates Godot scene meshes.
//!
//! This is the adaptation of arnis's `data_processing.rs`. Instead of dispatching
//! to block-placing element processors, we dispatch to mesh-generating processors.

use crate::coordinate_system::cartesian::XZBBox;
use crate::element_processing::{
    buildings, highways, railways, trees, vegetation, water_areas, waterways,
};
use crate::ground::Ground;
use crate::osm_parser::ProcessedElement;
use crate::scene_writer::SceneWriter;
use colored::Colorize;
use std::collections::HashSet;
use std::sync::Arc;

/// Process all OSM elements and populate the SceneWriter.
pub fn process_elements(
    elements: Vec<ProcessedElement>,
    xzbbox: &XZBBox,
    ground: Arc<Ground>,
    output_dir: std::path::PathBuf,
    godot_scale: f32,
    chunk_size: i32,
) -> Result<SceneWriter, String> {
    println!("{} Processing OSM elements...", "[4/6]".bold());

    let mut scene = SceneWriter::new(xzbbox, ground, output_dir, chunk_size, godot_scale);

    let total = elements.len();
    let mut building_count = 0u64;
    let mut highway_count = 0u64;
    let mut tree_count = 0u64;
    let mut vegetation_count = 0u64;
    let mut water_count = 0u64;
    let mut seen_elements: HashSet<(&'static str, u64)> = HashSet::new();

    for (i, element) in elements.iter().enumerate() {
        // Progress indicator every 1000 elements
        if i % 1000 == 0 && total > 0 {
            println!(
                "  Processing element {}/{} ({}%)...",
                i,
                total,
                (i * 100) / total
            );
        }

        if !seen_elements.insert((element.kind(), element.id())) {
            continue;
        }

        match element {
            ProcessedElement::Way(way) => {
                if way.tags.contains_key("building") || way.tags.contains_key("building:part") {
                    buildings::generate_building(&mut scene, way, godot_scale);
                    building_count += 1;
                } else if way.tags.contains_key("highway") {
                    highways::generate_highway(&mut scene, way, godot_scale);
                    highway_count += 1;
                } else if way.tags.contains_key("water")
                    || way
                        .tags
                        .get("natural")
                        .map(|v| v == "water" || v == "bay")
                        .unwrap_or(false)
                {
                    water_areas::generate_water_area(&mut scene, way, godot_scale);
                    water_count += 1;
                } else if way.tags.contains_key("railway") {
                    railways::generate_railway(&mut scene, way, godot_scale);
                } else if way.tags.contains_key("waterway") {
                    waterways::generate_waterway(&mut scene, way, godot_scale);
                } else if vegetation::is_vegetation_area(way) {
                    vegetation::generate_vegetation_area(&mut scene, way, godot_scale);
                    vegetation_count += 1;
                }
            }
            ProcessedElement::Node(node) => {
                if node.tags.get("natural") == Some(&"tree".to_string()) {
                    trees::generate_tree(&mut scene, node);
                    tree_count += 1;
                }
            }
            ProcessedElement::Relation(_rel) => {
                // v1: Skip relations (buildings from relations handled later)
            }
        }
    }

    println!();
    println!("{}", "Element Summary:".green().bold());
    println!("  Buildings: {}", building_count);
    println!("  Highways:  {}", highway_count);
    println!("  Trees:     {}", tree_count);
    println!("  Vegetation areas: {}", vegetation_count);
    println!("  Water:     {}", water_count);
    println!("  Total scene elements: {}", scene.element_count());
    println!("  Chunks:    {}", scene.chunk_count());

    Ok(scene)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinate_system::cartesian::XZBBox;
    use crate::ground::Ground;
    use crate::osm_parser::{ProcessedNode, ProcessedWay};
    use crate::scene_writer::chunk_grid::SceneElement;
    use crate::scene_writer::tres_writer::MaterialType;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn node(id: u64, x: i32, z: i32, tags: HashMap<String, String>) -> ProcessedNode {
        ProcessedNode { id, tags, x, z }
    }

    fn closed_way(id: u64, tags: HashMap<String, String>) -> ProcessedWay {
        ProcessedWay {
            id,
            tags,
            nodes: vec![
                node(id * 10 + 1, 20, 20, HashMap::new()),
                node(id * 10 + 2, 220, 20, HashMap::new()),
                node(id * 10 + 3, 220, 220, HashMap::new()),
                node(id * 10 + 4, 20, 220, HashMap::new()),
                node(id * 10 + 1, 20, 20, HashMap::new()),
            ],
        }
    }

    fn process_test_elements(elements: Vec<ProcessedElement>) -> SceneWriter {
        let bbox = XZBBox::rect_from_xz_lengths(511.0, 511.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        process_elements(
            elements,
            &bbox,
            ground,
            PathBuf::from("E:\\tmp\\osm-godot-test"),
            0.5,
            128,
        )
        .unwrap()
    }

    fn mesh_names(scene: &SceneWriter) -> Vec<String> {
        scene
            .chunk_grid
            .chunks
            .values()
            .flat_map(|chunk| chunk.elements.iter())
            .map(|element| match element {
                SceneElement::Mesh { name, .. } | SceneElement::Instance { name, .. } => {
                    name.clone()
                }
            })
            .collect()
    }

    fn mesh_materials(scene: &SceneWriter, prefix: &str) -> Vec<MaterialType> {
        scene
            .chunk_grid
            .chunks
            .values()
            .flat_map(|chunk| chunk.elements.iter())
            .filter_map(|element| match element {
                SceneElement::Mesh {
                    name,
                    material_type,
                    ..
                } if name.starts_with(prefix) => Some(*material_type),
                SceneElement::Instance {
                    name,
                    material_type,
                    ..
                } if name.starts_with(prefix) => Some(*material_type),
                _ => None,
            })
            .collect()
    }

    fn vegetation_positions(scene: &SceneWriter) -> Vec<(String, Vec<((f32, f32, f32), f32)>)> {
        let mut result: Vec<_> = scene
            .chunk_grid
            .chunks
            .values()
            .flat_map(|chunk| chunk.elements.iter())
            .filter_map(|element| match element {
                SceneElement::Instance {
                    name, positions, ..
                } if name.starts_with("Vegetation") => Some((name.clone(), positions.clone())),
                _ => None,
            })
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    #[test]
    fn process_elements_skips_duplicate_osm_ids() {
        let mut tags = HashMap::new();
        tags.insert("building".to_string(), "yes".to_string());
        let way = ProcessedWay {
            id: 42,
            tags,
            nodes: vec![
                ProcessedNode {
                    id: 1,
                    tags: HashMap::new(),
                    x: 0,
                    z: 0,
                },
                ProcessedNode {
                    id: 2,
                    tags: HashMap::new(),
                    x: 10,
                    z: 0,
                },
                ProcessedNode {
                    id: 3,
                    tags: HashMap::new(),
                    x: 10,
                    z: 10,
                },
                ProcessedNode {
                    id: 4,
                    tags: HashMap::new(),
                    x: 0,
                    z: 10,
                },
            ],
        };
        let bbox = XZBBox::rect_from_xz_lengths(100.0, 100.0).unwrap();
        let ground = Arc::new(Ground::new_flat(0));
        let tmp = tempfile::tempdir().unwrap();

        let scene = process_elements(
            vec![
                ProcessedElement::Way(way.clone()),
                ProcessedElement::Way(way),
            ],
            &bbox,
            ground,
            tmp.path().to_path_buf(),
            0.5,
            256,
        )
        .unwrap();

        // One building now emits wall, roof, windows, and door meshes. If the
        // duplicate OSM id were processed too, this would be doubled.
        assert_eq!(scene.element_count(), 4);
    }

    #[test]
    fn vegetation_area_classifies_osm_green_polygons() {
        let forest = closed_way(
            101,
            HashMap::from([("landuse".to_string(), "forest".to_string())]),
        );
        let park = closed_way(
            102,
            HashMap::from([("leisure".to_string(), "park".to_string())]),
        );
        let scrub = closed_way(
            103,
            HashMap::from([("natural".to_string(), "scrub".to_string())]),
        );
        let parking = closed_way(
            104,
            HashMap::from([("amenity".to_string(), "parking".to_string())]),
        );

        let scene = process_test_elements(vec![
            ProcessedElement::Way(forest),
            ProcessedElement::Way(park),
            ProcessedElement::Way(scrub),
            ProcessedElement::Way(parking),
        ]);
        let names = mesh_names(&scene);

        assert!(names.iter().any(|name| name == "VegetationGround_101"));
        assert!(names.iter().any(|name| name == "VegetationGround_102"));
        assert!(names.iter().any(|name| name == "VegetationGround_103"));
        assert!(!names.iter().any(|name| name == "VegetationGround_104"));
    }

    #[test]
    fn vegetation_area_writes_ground_patch_mesh() {
        let scene = process_test_elements(vec![ProcessedElement::Way(closed_way(
            201,
            HashMap::from([("leisure".to_string(), "garden".to_string())]),
        ))]);

        let names = mesh_names(&scene);
        let materials = mesh_materials(&scene, "VegetationGround_");

        assert!(names.iter().any(|name| name == "VegetationGround_201"));
        assert!(materials.iter().all(|material| matches!(
            material,
            MaterialType::TerrainGrass | MaterialType::TreeLeaves
        )));
        assert!(!materials.is_empty());
    }

    #[test]
    fn vegetation_scatter_is_deterministic_and_capped() {
        let forest = ProcessedElement::Way(closed_way(
            301,
            HashMap::from([("natural".to_string(), "wood".to_string())]),
        ));

        let first = process_test_elements(vec![forest.clone()]);
        let second = process_test_elements(vec![forest]);
        let first_positions = vegetation_positions(&first);
        let second_positions = vegetation_positions(&second);
        let instance_count: usize = first_positions
            .iter()
            .map(|(_, positions)| positions.len())
            .sum();

        assert_eq!(first_positions, second_positions);
        assert!(
            instance_count >= 3,
            "woodland vegetation should create concrete plant instances"
        );
        assert!(
            instance_count <= 48,
            "vegetation scatter must be capped for FPS safety"
        );
    }

    #[test]
    fn vegetation_tree_generation_has_multiple_profiles() {
        let elements: Vec<_> = (0..12)
            .map(|i| {
                ProcessedElement::Node(node(
                    400 + i,
                    20 + (i as i32 * 12),
                    260,
                    HashMap::from([("natural".to_string(), "tree".to_string())]),
                ))
            })
            .collect();

        let scene = process_test_elements(elements);
        let names = mesh_names(&scene);

        assert!(names.iter().any(|name| name.starts_with("VegetationTree_")));
        assert!(names
            .iter()
            .any(|name| name.starts_with("VegetationConifer_")));
        assert!(names
            .iter()
            .any(|name| name.starts_with("VegetationShrub_")));
        assert!(!names.iter().any(|name| name.starts_with("Tree_")));
    }
}
