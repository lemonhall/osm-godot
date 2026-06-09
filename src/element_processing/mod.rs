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
    "name:zh",
    "name:zh-Hans",
    "name:zh-Hant",
    "official_name",
    "official_name:zh",
    "official_name:zh-Hans",
    "official_name:zh-Hant",
    "alt_name",
    "alt_name:zh",
    "alt_name:zh-Hans",
    "alt_name:zh-Hant",
    "old_name",
    "brand:zh",
    "operator:zh",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn building_plaque_metadata_preserves_chinese_name_tags() {
        let tags = HashMap::from([
            ("name".to_string(), "Science Building".to_string()),
            ("name:zh".to_string(), "建筑科学院".to_string()),
            ("official_name:zh".to_string(), "中国建筑科学研究院".to_string()),
            ("brand:zh".to_string(), "便利蜂".to_string()),
            ("operator:zh".to_string(), "某某运营公司".to_string()),
            ("building".to_string(), "commercial".to_string()),
        ]);

        let metadata = osm_metadata(42, "building", &tags);

        assert_eq!(metadata.get("name:zh"), Some(&"建筑科学院".to_string()));
        assert_eq!(
            metadata.get("official_name:zh"),
            Some(&"中国建筑科学研究院".to_string())
        );
        assert_eq!(metadata.get("brand:zh"), Some(&"便利蜂".to_string()));
        assert_eq!(metadata.get("operator:zh"), Some(&"某某运营公司".to_string()));
    }
}
