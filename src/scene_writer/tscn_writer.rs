//! Godot .tscn generation.
//!
//! Uses Godot built-in primitives (BoxMesh, CylinderMesh, PlaneMesh) —
//! no ArrayMesh binary data, fully compatible with Godot 4.x.

use crate::scene_writer::chunk_grid::{Chunk, SceneElement};
use crate::scene_writer::tres_writer::MaterialType;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

pub fn write_chunk_scene(
    chunk: &Chunk,
    scenes_dir: &Path,
    mesh_data_dir: &Path,
    _material_ids: &HashMap<MaterialType, u32>,
) -> io::Result<()> {
    let filename = format!("Chunk_{}_{}.tscn", chunk.coord.0, chunk.coord.1);
    let path = scenes_dir.join(&filename);
    let mut f = fs::File::create(&path)?;

    fs::create_dir_all(mesh_data_dir)?;
    write_chunk_mesh_data(chunk, mesh_data_dir)?;

    let chunk_uid = chunk_uid(chunk.coord.0, chunk.coord.1);
    writeln!(
        f,
        "[gd_scene load_steps=2 format=3 uid=\"uid://{chunk_uid}\"]"
    )?;
    writeln!(f)?;
    writeln!(
        f,
        "[ext_resource type=\"Script\" path=\"res://scripts/chunk_mesh_loader.gd\" id=\"1\"]"
    )?;
    writeln!(f)?;

    // Root node
    let root_name = format!("Chunk_{}_{}", chunk.coord.0, chunk.coord.1);
    writeln!(f, "[node name=\"{root_name}\" type=\"Node3D\"]")?;
    writeln!(f, "script = ExtResource(\"1\")")?;
    writeln!(
        f,
        "mesh_data_path = \"res://mesh_data/{filename_base}.json\"",
        filename_base = root_name
    )?;

    Ok(())
}

pub fn write_roads_scene<'a, I>(
    chunks: I,
    scenes_dir: &Path,
    mesh_data_dir: &Path,
    godot_scale: f32,
) -> io::Result<()>
where
    I: IntoIterator<Item = &'a Chunk>,
{
    fs::create_dir_all(scenes_dir)?;
    fs::create_dir_all(mesh_data_dir)?;
    write_roads_mesh_data(chunks, mesh_data_dir, godot_scale)?;

    let path = scenes_dir.join("roads.tscn");
    let mut f = fs::File::create(&path)?;

    writeln!(
        f,
        "[gd_scene load_steps=2 format=3 uid=\"uid://roads00000001\"]"
    )?;
    writeln!(f)?;
    writeln!(
        f,
        "[ext_resource type=\"Script\" path=\"res://scripts/chunk_mesh_loader.gd\" id=\"1\"]"
    )?;
    writeln!(f)?;
    writeln!(f, "[node name=\"Roads\" type=\"Node3D\"]")?;
    writeln!(f, "script = ExtResource(\"1\")")?;
    writeln!(f, "mesh_data_path = \"res://mesh_data/roads.json\"")?;

    Ok(())
}

fn write_chunk_mesh_data(chunk: &Chunk, mesh_data_dir: &Path) -> io::Result<()> {
    let mut elements = Vec::new();

    for elem in &chunk.elements {
        match elem {
            SceneElement::Mesh {
                name,
                mesh_data,
                material_type,
                transform,
                metadata,
            } => {
                elements.push(mesh_json(
                    name,
                    mesh_data,
                    *material_type,
                    *transform,
                    metadata,
                ));
            }
            SceneElement::Instance {
                name,
                mesh_data,
                material_type,
                positions,
            } => {
                for (pi, (pos, rot)) in positions.iter().enumerate() {
                    elements.push(mesh_json(
                        &format!("{name}_{pi}"),
                        mesh_data,
                        *material_type,
                        transform_from_pos_rot(*pos, *rot),
                        &Default::default(),
                    ));
                }
            }
        }
    }

    let payload = serde_json::json!({ "elements": elements });
    let path = mesh_data_dir.join(format!("Chunk_{}_{}.json", chunk.coord.0, chunk.coord.1));
    fs::write(path, serde_json::to_string(&payload)?)
}

fn write_roads_mesh_data<'a, I>(chunks: I, mesh_data_dir: &Path, godot_scale: f32) -> io::Result<()>
where
    I: IntoIterator<Item = &'a Chunk>,
{
    let mut elements = Vec::new();

    for chunk in chunks {
        let (min_x, min_z, _, _) = chunk.world_bounds;
        let chunk_x = min_x as f32 * godot_scale;
        let chunk_z = -(min_z as f32) * godot_scale;

        for elem in &chunk.elements {
            let SceneElement::Mesh {
                name,
                mesh_data,
                material_type,
                transform,
                metadata,
            } = elem
            else {
                continue;
            };
            if !is_road_material(*material_type) {
                continue;
            }
            let mut raised_transform = *transform;
            raised_transform[9] += chunk_x;
            raised_transform[10] += 0.08;
            raised_transform[11] += chunk_z;
            elements.push(mesh_json(
                name,
                mesh_data,
                *material_type,
                raised_transform,
                metadata,
            ));
        }
    }

    let payload = serde_json::json!({ "elements": elements });
    fs::write(
        mesh_data_dir.join("roads.json"),
        serde_json::to_string(&payload)?,
    )
}

fn is_road_material(material_type: MaterialType) -> bool {
    matches!(
        material_type,
        MaterialType::RoadAsphalt | MaterialType::RoadSidewalk
    )
}

fn mesh_json(
    name: &str,
    mesh_data: &crate::scene_writer::geometry::MeshData,
    material_type: MaterialType,
    transform: [f32; 12],
    metadata: &crate::scene_writer::chunk_grid::ElementMetadata,
) -> serde_json::Value {
    serde_json::json!({
        "name": safe_name(name),
        "material": material_type.file_stem(),
        "transform": transform,
        "metadata": metadata,
        "vertices": mesh_data.vertices,
        "normals": mesh_data.normals,
        "uvs": mesh_data.uvs,
        "indices": mesh_data.indices,
    })
}

fn transform_from_pos_rot(pos: (f32, f32, f32), y_rot: f32) -> [f32; 12] {
    let (s, c) = y_rot.sin_cos();
    [c, 0.0, -s, 0.0, 1.0, 0.0, s, 0.0, c, pos.0, pos.1, pos.2]
}

pub fn translation_transform(x: f32, y: f32, z: f32) -> [f32; 12] {
    [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, x, y, z]
}

fn chunk_uid(cx: i32, cz: i32) -> String {
    let h = (cx as u64)
        .wrapping_mul(0x517cc1b7)
        .wrapping_add(cz as u64)
        .wrapping_mul(0x9e3779b9);
    format!("c{h:013x}")
}

fn safe_name(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_writer::chunk_grid::{Chunk, ChunkCoord, SceneElement};
    use crate::scene_writer::geometry::MeshData;

    #[test]
    fn chunk_scene_uses_runtime_array_mesh_loader() {
        let tmp = tempfile::tempdir().unwrap();
        let mesh_data_dir = tmp.path().join("mesh_data");
        std::fs::create_dir_all(&mesh_data_dir).unwrap();
        let mut material_ids = HashMap::new();
        material_ids.insert(MaterialType::BuildingWall, 1);

        let mut mesh = MeshData::new();
        mesh.vertices
            .extend_from_slice(&[0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, -2.0]);
        mesh.normals
            .extend_from_slice(&[0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0]);
        mesh.uvs.extend_from_slice(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
        mesh.indices.extend_from_slice(&[0, 1, 2]);
        let chunk = Chunk {
            coord: ChunkCoord(0, 0),
            world_bounds: (0, 0, 255, 255),
            elements: vec![SceneElement::Mesh {
                name: "BuildingWall_1".to_string(),
                mesh_data: mesh,
                material_type: MaterialType::BuildingWall,
                transform: translation_transform(1.0, 0.0, -1.0),
                metadata: Default::default(),
            }],
        };

        write_chunk_scene(&chunk, tmp.path(), &mesh_data_dir, &material_ids).unwrap();

        let scene = std::fs::read_to_string(tmp.path().join("Chunk_0_0.tscn")).unwrap();
        assert!(scene.contains("path=\"res://scripts/chunk_mesh_loader.gd\""));
        assert!(scene.contains("mesh_data_path = \"res://mesh_data/Chunk_0_0.json\""));
        assert!(!scene.contains("type=\"BoxMesh\""));
        assert!(!scene.contains("type=\"PlaneMesh\""));

        let mesh_data = std::fs::read_to_string(mesh_data_dir.join("Chunk_0_0.json")).unwrap();
        assert!(mesh_data.contains("\"vertices\""));
        assert!(mesh_data.contains("2.0"));
        assert!(mesh_data.contains("\"indices\""));
        assert!(mesh_data.contains("\"material\":\"building_wall\""));
    }

    #[test]
    fn chunk_mesh_data_includes_roads_for_world_streaming() {
        let tmp = tempfile::tempdir().unwrap();
        let mesh_data_dir = tmp.path().join("mesh_data");
        std::fs::create_dir_all(&mesh_data_dir).unwrap();
        let material_ids = HashMap::new();

        let mut road_mesh = MeshData::new();
        road_mesh
            .vertices
            .extend_from_slice(&[0.0, 0.0, 0.0, 4.0, 0.0, -4.0]);
        road_mesh.indices.extend_from_slice(&[0, 1, 0]);
        let mut building_mesh = MeshData::new();
        building_mesh
            .vertices
            .extend_from_slice(&[0.0, 0.0, 0.0, 2.0, 2.0, 2.0]);
        building_mesh.indices.extend_from_slice(&[0, 1, 0]);
        let chunk = Chunk {
            coord: ChunkCoord(0, 0),
            world_bounds: (0, 0, 255, 255),
            elements: vec![
                SceneElement::Mesh {
                    name: "Highway_1".to_string(),
                    mesh_data: road_mesh,
                    material_type: MaterialType::RoadAsphalt,
                    transform: translation_transform(10.0, 0.0, -10.0),
                    metadata: Default::default(),
                },
                SceneElement::Mesh {
                    name: "BuildingWall_1".to_string(),
                    mesh_data: building_mesh,
                    material_type: MaterialType::BuildingWall,
                    transform: translation_transform(1.0, 0.0, -1.0),
                    metadata: Default::default(),
                },
            ],
        };

        write_chunk_scene(&chunk, tmp.path(), &mesh_data_dir, &material_ids).unwrap();

        let mesh_data = std::fs::read_to_string(mesh_data_dir.join("Chunk_0_0.json")).unwrap();
        assert!(mesh_data.contains("\"name\":\"Highway_1\""));
        assert!(mesh_data.contains("\"material\":\"road_asphalt\""));
        assert!(mesh_data.contains("\"name\":\"BuildingWall_1\""));
    }

    #[test]
    fn roads_scene_contains_raised_road_meshes() {
        let tmp = tempfile::tempdir().unwrap();
        let mesh_data_dir = tmp.path().join("mesh_data");
        std::fs::create_dir_all(&mesh_data_dir).unwrap();

        let mut road_mesh = MeshData::new();
        road_mesh
            .vertices
            .extend_from_slice(&[0.0, 0.0, 0.0, 4.0, 0.0, -4.0]);
        road_mesh.indices.extend_from_slice(&[0, 1, 0]);
        let chunk = Chunk {
            coord: ChunkCoord(0, 0),
            world_bounds: (0, 0, 255, 255),
            elements: vec![SceneElement::Mesh {
                name: "Highway_1".to_string(),
                mesh_data: road_mesh,
                material_type: MaterialType::RoadAsphalt,
                transform: translation_transform(10.0, 0.0, -10.0),
                metadata: Default::default(),
            }],
        };

        write_roads_scene([&chunk], tmp.path(), &mesh_data_dir, 0.5).unwrap();

        let roads_scene = std::fs::read_to_string(tmp.path().join("roads.tscn")).unwrap();
        let roads_data = std::fs::read_to_string(mesh_data_dir.join("roads.json")).unwrap();
        assert!(roads_scene.contains("[node name=\"Roads\" type=\"Node3D\"]"));
        assert!(roads_scene.contains("mesh_data_path = \"res://mesh_data/roads.json\""));
        assert!(roads_data.contains("\"name\":\"Highway_1\""));
        assert!(roads_data.contains("\"material\":\"road_asphalt\""));
        let parsed: serde_json::Value = serde_json::from_str(&roads_data).unwrap();
        let transform_y = parsed["elements"][0]["transform"][10].as_f64().unwrap();
        assert!((transform_y - 0.08).abs() < 0.0001);
    }

    #[test]
    fn roads_scene_writes_world_transforms_instead_of_chunk_local_transforms() {
        let tmp = tempfile::tempdir().unwrap();
        let mesh_data_dir = tmp.path().join("mesh_data");
        std::fs::create_dir_all(&mesh_data_dir).unwrap();

        let mut road_mesh = MeshData::new();
        road_mesh
            .vertices
            .extend_from_slice(&[0.0, 0.0, 0.0, 4.0, 0.0, -4.0]);
        road_mesh.indices.extend_from_slice(&[0, 1, 0]);
        let chunk = Chunk {
            coord: ChunkCoord(2, 3),
            world_bounds: (512, 768, 767, 1023),
            elements: vec![SceneElement::Mesh {
                name: "Highway_1".to_string(),
                mesh_data: road_mesh,
                material_type: MaterialType::RoadAsphalt,
                transform: translation_transform(10.0, 0.0, -20.0),
                metadata: Default::default(),
            }],
        };

        write_roads_scene([&chunk], tmp.path(), &mesh_data_dir, 0.5).unwrap();

        let roads_data = std::fs::read_to_string(mesh_data_dir.join("roads.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&roads_data).unwrap();
        let transform = parsed["elements"][0]["transform"].as_array().unwrap();
        let x = transform[9].as_f64().unwrap();
        let y = transform[10].as_f64().unwrap();
        let z = transform[11].as_f64().unwrap();

        assert!((x - 266.0).abs() < 0.0001);
        assert!((y - 0.08).abs() < 0.0001);
        assert!((z + 404.0).abs() < 0.0001);
    }

    #[test]
    fn mesh_json_preserves_navigation_metadata_for_buildings_and_roads() {
        let tmp = tempfile::tempdir().unwrap();
        let mesh_data_dir = tmp.path().join("mesh_data");
        std::fs::create_dir_all(&mesh_data_dir).unwrap();

        let mut mesh = MeshData::new();
        mesh.vertices.extend_from_slice(&[0.0, 0.0, 0.0]);
        mesh.indices.extend_from_slice(&[0, 0, 0]);

        let mut chunk = Chunk {
            coord: ChunkCoord(0, 0),
            world_bounds: (0, 0, 255, 255),
            elements: vec![SceneElement::Mesh {
                name: "BuildingWall_100".to_string(),
                mesh_data: mesh.clone(),
                material_type: MaterialType::BuildingWall,
                transform: translation_transform(1.0, 0.0, 2.0),
                metadata: [
                    ("osm_id".to_string(), "100".to_string()),
                    ("osm_kind".to_string(), "building".to_string()),
                    ("name".to_string(), "Bund Test Building".to_string()),
                    ("addr:housenumber".to_string(), "18".to_string()),
                ]
                .into_iter()
                .collect(),
            }],
        };

        write_chunk_scene(&chunk, tmp.path(), &mesh_data_dir, &HashMap::new()).unwrap();
        let chunk_data = std::fs::read_to_string(mesh_data_dir.join("Chunk_0_0.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&chunk_data).unwrap();
        let metadata = &parsed["elements"][0]["metadata"];
        assert_eq!(metadata["osm_id"], "100");
        assert_eq!(metadata["osm_kind"], "building");
        assert_eq!(metadata["name"], "Bund Test Building");
        assert_eq!(metadata["addr:housenumber"], "18");

        chunk.elements.push(SceneElement::Mesh {
            name: "Highway_200".to_string(),
            mesh_data: mesh,
            material_type: MaterialType::RoadAsphalt,
            transform: translation_transform(3.0, 0.0, 4.0),
            metadata: [
                ("osm_id".to_string(), "200".to_string()),
                ("osm_kind".to_string(), "road".to_string()),
                ("name".to_string(), "Zhongshan East 1st Road".to_string()),
                ("highway".to_string(), "primary".to_string()),
            ]
            .into_iter()
            .collect(),
        });
        write_roads_scene([&chunk], tmp.path(), &mesh_data_dir, 0.5).unwrap();
        let roads_data = std::fs::read_to_string(mesh_data_dir.join("roads.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&roads_data).unwrap();
        let metadata = &parsed["elements"][0]["metadata"];
        assert_eq!(metadata["osm_id"], "200");
        assert_eq!(metadata["osm_kind"], "road");
        assert_eq!(metadata["name"], "Zhongshan East 1st Road");
        assert_eq!(metadata["highway"], "primary");
    }

    #[test]
    fn world_streaming_chunk_mesh_data_keeps_roads_with_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let mesh_data_dir = tmp.path().join("mesh_data");
        std::fs::create_dir_all(&mesh_data_dir).unwrap();

        let mut road_mesh = MeshData::new();
        road_mesh
            .vertices
            .extend_from_slice(&[0.0, 0.0, 0.0, 4.0, 0.0, -4.0]);
        road_mesh.indices.extend_from_slice(&[0, 1, 0]);
        let chunk = Chunk {
            coord: ChunkCoord(0, 0),
            world_bounds: (0, 0, 255, 255),
            elements: vec![SceneElement::Mesh {
                name: "Highway_200".to_string(),
                mesh_data: road_mesh,
                material_type: MaterialType::RoadAsphalt,
                transform: translation_transform(10.0, 0.0, -10.0),
                metadata: [
                    ("osm_id".to_string(), "200".to_string()),
                    ("osm_kind".to_string(), "road".to_string()),
                    ("name".to_string(), "Chunked Road".to_string()),
                    ("highway".to_string(), "primary".to_string()),
                ]
                .into_iter()
                .collect(),
            }],
        };

        write_chunk_scene(&chunk, tmp.path(), &mesh_data_dir, &HashMap::new()).unwrap();

        let chunk_data = std::fs::read_to_string(mesh_data_dir.join("Chunk_0_0.json")).unwrap();
        assert!(chunk_data.contains("\"name\":\"Highway_200\""));
        assert!(chunk_data.contains("\"material\":\"road_asphalt\""));
        let parsed: serde_json::Value = serde_json::from_str(&chunk_data).unwrap();
        let metadata = &parsed["elements"][0]["metadata"];
        assert_eq!(metadata["osm_kind"], "road");
        assert_eq!(metadata["name"], "Chunked Road");
        assert_eq!(metadata["highway"], "primary");
    }
}
