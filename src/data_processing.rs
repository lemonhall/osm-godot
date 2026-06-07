//! Main dispatch loop — processes all OSM elements and generates Godot scene meshes.
//!
//! This is the adaptation of arnis's `data_processing.rs`. Instead of dispatching
//! to block-placing element processors, we dispatch to mesh-generating processors.

use crate::coordinate_system::cartesian::XZBBox;
use crate::element_processing::{buildings, highways, railways, trees, water_areas, waterways};
use crate::ground::Ground;
use crate::osm_parser::ProcessedElement;
use crate::scene_writer::SceneWriter;
use colored::Colorize;
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
    let mut water_count = 0u64;

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
    println!("  Water:     {}", water_count);
    println!(
        "  Total scene elements: {}",
        scene.element_count()
    );
    println!("  Chunks:    {}", scene.chunk_count());

    Ok(scene)
}
