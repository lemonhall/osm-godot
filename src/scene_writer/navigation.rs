//! Lightweight offline navigation graph generation.

use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct NavigationGraphBuilder {
    nodes: Vec<NavigationNode>,
    edges: Vec<NavigationEdge>,
    node_lookup: HashMap<(i32, i32), String>,
}

#[derive(Clone, Serialize)]
struct NavigationNode {
    id: String,
    position: [f32; 2],
}

#[derive(Clone, Serialize)]
struct NavigationEdge {
    from: String,
    to: String,
    cost: f32,
    osm_id: String,
    highway: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

impl NavigationGraphBuilder {
    pub fn add_road(
        &mut self,
        osm_id: u64,
        tags: &HashMap<String, String>,
        centerline_arnis: &[(f32, f32)],
        godot_scale: f32,
    ) {
        if centerline_arnis.len() < 2 {
            return;
        }

        let highway = tags
            .get("highway")
            .cloned()
            .unwrap_or_else(|| "road".to_string());
        let name = tags
            .get("name")
            .or_else(|| tags.get("official_name"))
            .or_else(|| tags.get("alt_name"))
            .cloned();
        let osm_id = osm_id.to_string();

        let mut last_node_id: Option<String> = None;
        for &(x, z) in centerline_arnis {
            let position = [x * godot_scale, -z * godot_scale];
            let node_id = self.node_id_for_position(position);
            if let Some(from) = last_node_id {
                if from != node_id {
                    let cost = distance_between_nodes(&self.nodes, &from, &node_id);
                    if cost > f32::EPSILON {
                        self.edges.push(NavigationEdge {
                            from: from.clone(),
                            to: node_id.clone(),
                            cost,
                            osm_id: osm_id.clone(),
                            highway: highway.clone(),
                            name: name.clone(),
                        });
                        self.edges.push(NavigationEdge {
                            from: node_id.clone(),
                            to: from,
                            cost,
                            osm_id: osm_id.clone(),
                            highway: highway.clone(),
                            name: name.clone(),
                        });
                    }
                }
            }
            last_node_id = Some(node_id);
        }
    }

    pub fn to_json_value(&self) -> serde_json::Value {
        let mut edges = self.edges.clone();
        edges.extend(self.connector_edges(6.0));
        serde_json::json!({
            "version": 1,
            "coordinate_space": "godot_xz",
            "nodes": self.nodes,
            "edges": edges,
        })
    }

    fn node_id_for_position(&mut self, position: [f32; 2]) -> String {
        let key = quantized_key(position);
        if let Some(id) = self.node_lookup.get(&key) {
            return id.clone();
        }

        let id = format!("n{}", self.nodes.len());
        self.nodes.push(NavigationNode {
            id: id.clone(),
            position,
        });
        self.node_lookup.insert(key, id.clone());
        id
    }

    fn connector_edges(&self, max_distance: f32) -> Vec<NavigationEdge> {
        let mut buckets: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (index, node) in self.nodes.iter().enumerate() {
            let key = (
                (node.position[0] / max_distance).floor() as i32,
                (node.position[1] / max_distance).floor() as i32,
            );
            buckets.entry(key).or_default().push(index);
        }

        let mut existing_pairs: HashSet<(String, String)> = HashSet::new();
        for edge in &self.edges {
            existing_pairs.insert((edge.from.clone(), edge.to.clone()));
        }

        let mut connectors = Vec::new();
        let mut connected_pairs: HashSet<(String, String)> = HashSet::new();
        for (index, node) in self.nodes.iter().enumerate() {
            let key = (
                (node.position[0] / max_distance).floor() as i32,
                (node.position[1] / max_distance).floor() as i32,
            );
            for dx in -1..=1 {
                for dz in -1..=1 {
                    let Some(candidates) = buckets.get(&(key.0 + dx, key.1 + dz)) else {
                        continue;
                    };
                    for &other_index in candidates {
                        if other_index <= index {
                            continue;
                        }
                        let other = &self.nodes[other_index];
                        let distance = distance_between_positions(node.position, other.position);
                        if distance <= f32::EPSILON || distance > max_distance {
                            continue;
                        }
                        let forward = (node.id.clone(), other.id.clone());
                        let backward = (other.id.clone(), node.id.clone());
                        if existing_pairs.contains(&forward)
                            || existing_pairs.contains(&backward)
                            || connected_pairs.contains(&forward)
                            || connected_pairs.contains(&backward)
                        {
                            continue;
                        }
                        connected_pairs.insert(forward.clone());
                        connectors.push(NavigationEdge {
                            from: forward.0.clone(),
                            to: forward.1.clone(),
                            cost: distance,
                            osm_id: "connector".to_string(),
                            highway: "connector".to_string(),
                            name: None,
                        });
                        connectors.push(NavigationEdge {
                            from: backward.0.clone(),
                            to: backward.1.clone(),
                            cost: distance,
                            osm_id: "connector".to_string(),
                            highway: "connector".to_string(),
                            name: None,
                        });
                    }
                }
            }
        }
        connectors
    }
}

fn quantized_key(position: [f32; 2]) -> (i32, i32) {
    // Two Godot meters keeps nearly identical OSM road endpoints connected without
    // merging separate parallel lanes too aggressively at city scale.
    (
        (position[0] / 2.0).round() as i32,
        (position[1] / 2.0).round() as i32,
    )
}

fn distance_between_nodes(nodes: &[NavigationNode], from: &str, to: &str) -> f32 {
    let Some(from_node) = nodes.iter().find(|node| node.id == from) else {
        return 0.0;
    };
    let Some(to_node) = nodes.iter().find(|node| node.id == to) else {
        return 0.0;
    };
    distance_between_positions(from_node.position, to_node.position)
}

fn distance_between_positions(from: [f32; 2], to: [f32; 2]) -> f32 {
    let dx = from[0] - to[0];
    let dz = from[1] - to[1];
    (dx * dx + dz * dz).sqrt()
}
