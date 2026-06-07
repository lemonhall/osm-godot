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
    writeln!(f, "[gd_scene load_steps=2 format=3 uid=\"uid://{chunk_uid}\"]")?;
    writeln!(f)?;
    writeln!(f, "[ext_resource type=\"Script\" path=\"res://scripts/chunk_mesh_loader.gd\" id=\"1\"]")?;
    writeln!(f)?;

    // Root node
    let root_name = format!("Chunk_{}_{}", chunk.coord.0, chunk.coord.1);
    writeln!(f, "[node name=\"{root_name}\" type=\"Node3D\"]")?;
    writeln!(f, "script = ExtResource(\"1\")")?;
    writeln!(f, "mesh_data_path = \"res://mesh_data/{filename_base}.json\"", filename_base = root_name)?;

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
            } => {
                elements.push(mesh_json(
                    name,
                    mesh_data,
                    *material_type,
                    *transform,
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
                    ));
                }
            }
        }
    }

    let payload = serde_json::json!({ "elements": elements });
    let path = mesh_data_dir.join(format!("Chunk_{}_{}.json", chunk.coord.0, chunk.coord.1));
    fs::write(path, serde_json::to_string(&payload)?)
}

fn mesh_json(
    name: &str,
    mesh_data: &crate::scene_writer::geometry::MeshData,
    material_type: MaterialType,
    transform: [f32; 12],
) -> serde_json::Value {
    serde_json::json!({
        "name": safe_name(name),
        "material": material_type.file_stem(),
        "transform": transform,
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
    let h = (cx as u64).wrapping_mul(0x517cc1b7).wrapping_add(cz as u64).wrapping_mul(0x9e3779b9);
    format!("c{h:013x}")
}

fn safe_name(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect()
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
        mesh.vertices.extend_from_slice(&[
            0.0, 0.0, 0.0,
            2.0, 0.0, 0.0,
            0.0, 0.0, -2.0,
        ]);
        mesh.normals.extend_from_slice(&[
            0.0, 1.0, 0.0,
            0.0, 1.0, 0.0,
            0.0, 1.0, 0.0,
        ]);
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
}
