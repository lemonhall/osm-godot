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
    material_ids: &HashMap<MaterialType, u32>,
) -> io::Result<()> {
    let filename = format!("Chunk_{}_{}.tscn", chunk.coord.0, chunk.coord.1);
    let path = scenes_dir.join(&filename);
    let mut f = fs::File::create(&path)?;

    let ext_count = material_ids.len();
    let sub_count = chunk.elements.len() as u32;
    let load_steps = ext_count as u32 + sub_count;

    let chunk_uid = chunk_uid(chunk.coord.0, chunk.coord.1);
    writeln!(f, "[gd_scene load_steps={load_steps} format=3 uid=\"uid://{chunk_uid}\"]")?;
    writeln!(f)?;

    let mut mat_ext_ids: HashMap<MaterialType, u32> = HashMap::new();
    for (mt, &eid) in material_ids {
        writeln!(f, "[ext_resource type=\"Material\" path=\"res://materials/{}.tres\" id=\"{eid}\"]", mt.file_stem())?;
        mat_ext_ids.insert(*mt, eid);
    }
    if !material_ids.is_empty() { writeln!(f)?; }

    let mut sub_id = 1u32;
    let mut mesh_entries: Vec<(usize, u32)> = Vec::new();

    for (i, elem) in chunk.elements.iter().enumerate() {
        let (mesh_data, name) = match elem {
            SceneElement::Mesh { name, mesh_data, .. } => (mesh_data, name),
            SceneElement::Instance { name, mesh_data, .. } => (mesh_data, name),
        };
        let res_name = format!("m{:03}", i);
        let (w, h, d) = aabb_dims(mesh_data);

        if name.starts_with("Tree_") {
            let r = (w.max(d) * 0.45).max(0.1).min(2.0);
            writeln!(f, "[sub_resource type=\"CylinderMesh\" id=\"{sub_id}\"]")?;
            writeln!(f, "resource_name = \"{res_name}\"")?;
            writeln!(f, "height = {:.2}", h.max(1.0))?;
            writeln!(f, "top_radius = {:.2}", if h > 4.0 { 0.0 } else { r })?; // cone if tall
            writeln!(f, "bottom_radius = {r:.2}")?;
        } else if name.starts_with("Terrain_") {
            writeln!(f, "[sub_resource type=\"PlaneMesh\" id=\"{sub_id}\"]")?;
            writeln!(f, "resource_name = \"{res_name}\"")?;
            writeln!(f, "size = Vector2({:.2}, {:.2})", w.max(0.5), d.max(0.5))?;
        } else if name.starts_with("Water_") || name.starts_with("Waterway_") {
            writeln!(f, "[sub_resource type=\"PlaneMesh\" id=\"{sub_id}\"]")?;
            writeln!(f, "resource_name = \"{res_name}\"")?;
            writeln!(f, "size = Vector2({:.2}, {:.2})", w.max(0.5), d.max(0.5))?;
        } else if name.starts_with("Highway_") || name.starts_with("Railway_") {
            writeln!(f, "[sub_resource type=\"BoxMesh\" id=\"{sub_id}\"]")?;
            writeln!(f, "resource_name = \"{res_name}\"")?;
            writeln!(f, "size = Vector3({:.2}, 0.15, {:.2})", w.max(0.3), d.max(0.3))?;
        } else {
            // Building wall or roof — BoxMesh with AABB dimensions
            writeln!(f, "[sub_resource type=\"BoxMesh\" id=\"{sub_id}\"]")?;
            writeln!(f, "resource_name = \"{res_name}\"")?;
            writeln!(f, "size = Vector3({:.2}, {:.2}, {:.2})", w.max(0.3), h.max(0.5), d.max(0.3))?;
        }
        writeln!(f)?;
        mesh_entries.push((i, sub_id));
        sub_id += 1;
    }

    // Root node
    let root_name = format!("Chunk_{}_{}", chunk.coord.0, chunk.coord.1);
    writeln!(f, "[node name=\"{root_name}\" type=\"Node3D\"]")?;
    writeln!(f)?;

    for (elem_idx, mid) in &mesh_entries {
        let elem = &chunk.elements[*elem_idx];
        match elem {
            SceneElement::Mesh { name, material_type, transform, .. } => {
                writeln!(f, "[node name=\"{}\" type=\"MeshInstance3D\" parent=\".\"]", safe_name(name))?;
                write_xform(&mut f, *transform)?;
                writeln!(f, "mesh = SubResource(\"{mid}\")")?;
                if let Some(&eid) = mat_ext_ids.get(material_type) {
                    writeln!(f, "surface_material_override/0 = ExtResource(\"{eid}\")")?;
                }
                writeln!(f)?;
            }
            SceneElement::Instance { name, material_type, positions, .. } => {
                for (pi, (pos, rot)) in positions.iter().enumerate() {
                    writeln!(f, "[node name=\"{}_{}\" type=\"MeshInstance3D\" parent=\".\"]", safe_name(name), pi)?;
                    write_xform_pr(&mut f, *pos, *rot)?;
                    writeln!(f, "mesh = SubResource(\"{mid}\")")?;
                    if let Some(&eid) = mat_ext_ids.get(material_type) {
                        writeln!(f, "surface_material_override/0 = ExtResource(\"{eid}\")")?;
                    }
                    writeln!(f)?;
                }
            }
        }
    }

    Ok(())
}

fn aabb_dims(m: &crate::scene_writer::geometry::MeshData) -> (f32, f32, f32) {
    let v = &m.vertices;
    if v.is_empty() { return (1.0, 1.0, 1.0); }
    let (mut mx, mut my, mut mz) = (f32::MAX, f32::MAX, f32::MAX);
    let (mut Mx, mut My, mut Mz) = (f32::MIN, f32::MIN, f32::MIN);
    for i in (0..v.len()).step_by(3) {
        mx = mx.min(v[i]); Mx = Mx.max(v[i]);
        my = my.min(v[i+1]); My = My.max(v[i+1]);
        mz = mz.min(v[i+2]); Mz = Mz.max(v[i+2]);
    }
    (Mx - mx, My - my, Mz - mz)
}

fn write_xform(f: &mut fs::File, m: [f32; 12]) -> io::Result<()> {
    write!(f, "transform = Transform3D(")?;
    for (i, v) in m.iter().enumerate() {
        if i > 0 { write!(f, ", ")?; }
        if v.fract() == 0.0 { write!(f, "{}", *v as i32)?; }
        else { write!(f, "{v:.4}")?; }
    }
    writeln!(f, ")")
}

fn write_xform_pr(f: &mut fs::File, pos: (f32, f32, f32), y_rot: f32) -> io::Result<()> {
    let (s, c) = y_rot.sin_cos();
    write_xform(f, [c, 0.0, -s, 0.0, 1.0, 0.0, s, 0.0, c, pos.0, pos.1, pos.2])
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
    fn chunk_scene_attaches_meshes_to_scene_root() {
        let tmp = tempfile::tempdir().unwrap();
        let mut material_ids = HashMap::new();
        material_ids.insert(MaterialType::BuildingWall, 1);

        let mut mesh = MeshData::new();
        mesh.vertices.extend_from_slice(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
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

        write_chunk_scene(&chunk, tmp.path(), &material_ids).unwrap();

        let scene = std::fs::read_to_string(tmp.path().join("Chunk_0_0.tscn")).unwrap();
        assert!(scene.contains("[node name=\"BuildingWall_1\" type=\"MeshInstance3D\" parent=\".\"]"));
        assert!(!scene.contains("parent=\"Chunk_0_0\""));
    }
}
