//! Element processing modules — convert OSM elements to Godot 3D meshes.

use crate::scene_writer::chunk_grid::ElementMetadata;
use std::collections::HashMap;

pub mod buildings;
pub mod highways;
pub mod railways;
pub mod trees;
pub mod water_areas;
pub mod waterways;

const METADATA_KEYS: &[&str] = &[
    "name",
    "official_name",
    "alt_name",
    "old_name",
    "building",
    "building:levels",
    "building:height",
    "height",
    "roof:shape",
    "roof:material",
    "roof:colour",
    "highway",
    "amenity",
    "shop",
    "tourism",
];

pub(crate) fn osm_metadata(id: u64, kind: &str, tags: &HashMap<String, String>) -> ElementMetadata {
    let mut metadata = ElementMetadata::new();
    metadata.insert("osm_id".to_string(), id.to_string());
    metadata.insert("osm_kind".to_string(), kind.to_string());

    for key in METADATA_KEYS {
        if let Some(value) = tags.get(*key) {
            metadata.insert((*key).to_string(), value.clone());
        }
    }

    for (key, value) in tags {
        if key.starts_with("addr:") {
            metadata.insert(key.clone(), value.clone());
        }
    }

    metadata
}
