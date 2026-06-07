//! Generates Godot .tscn (text scene) files using built-in mesh primitives.
//!
//! Instead of embedding raw ArrayMesh binary data (fragile across Godot versions),
//! we use BoxMesh / CylinderMesh / PlaneMesh which are simple parameterized primitives.

use crate::scene_writer::chunk_grid::{Chunk, SceneElement};
use crate::scene_writer::tres_writer::MaterialType;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Write a single chunk scene.
pub fn write_chunk_scene(
    chunk: &Chunk,
    scenes_dir: &Path,
    material_ids: &HashMap<MaterialType, u32>,
) -> io::Result<()> {
    let filename = format!("Chunk_{}_{}.tscn", chunk.coord.0, chunk.coord.1);
    let path = scenes_dir.join(&filename);
    let mut f = fs::File::create(&path)?;

    // Count load steps
    let ext_count = material_ids.len();
    let sub_count = chunk.elements.len() as u32;
    let load_steps = ext_count as u32 + sub_count;

    let chunk_uid = chunk_uid(chunk.coord.0, chunk.coord.1);
    writeln!(f, "[gd_scene load_steps={load_steps} format=3 uid=\"uid://{chunk_uid}\"]")?;
    writeln!(f)?;

    // External resources (materials)
    let mut mat_ext_ids: HashMap<MaterialType, u32> = HashMap::new();
    for (mat_type, &ext_id) in material_ids {
        let path_str = format!("res://materials/{}.tres", mat_type.file_stem());
        writeln!(f, "[ext_resource type=\"Material\" path=\"{path_str}\" id=\"{ext_id}\"]")?;
        mat_ext_ids.insert(*mat_type, ext_id);
    }
    if !material_ids.is_empty() { writeln!(f)?; }

    // Sub-resources — one mesh per element
    let mut sub_id = 1u32;
    let mut elem_to_mesh: Vec<(usize, u32, &str)> = Vec::new(); // (elem_idx, sub_id, mesh_type)

    for (i, elem) in chunk.elements.iter().enumerate() {
        let res_name = format!("elem_{i}_mesh");
        match elem {
            SceneElement::Mesh { name, mesh_data, .. } => {
                let (mesh_type, dims) = classify_mesh(name, mesh_data);
                write_mesh_sub(&mut f, sub_id, &res_name, mesh_type, dims)?;
                elem_to_mesh.push((i, sub_id, mesh_type));
            }
            SceneElement::Instance { name, mesh_data, .. } => {
                let (mesh_type, dims) = classify_mesh(name, mesh_data);
                write_mesh_sub(&mut f, sub_id, &res_name, mesh_type, dims)?;
                elem_to_mesh.push((i, sub_id, mesh_type));
            }
        }
        sub_id += 1;
    }

    // Root node
    let root_name = format!("Chunk_{}_{}", chunk.coord.0, chunk.coord.1);
    writeln!(f, "[node name=\"{root_name}\" type=\"Node3D\"]")?;
    writeln!(f)?;

    // Child nodes
    for (elem_idx, mesh_sub_id, _mesh_type) in &elem_to_mesh {
        let elem = &chunk.elements[*elem_idx];
        match elem {
            SceneElement::Mesh { name, material_type, transform, .. } => {
                let node_name = sanitize_name(name);
                writeln!(f, "[node name=\"{node_name}\" type=\"MeshInstance3D\" parent=\"{root_name}\"]")?;
                write_transform3d(&mut f, *transform)?;
                writeln!(f, "mesh = SubResource(\"{mesh_sub_id}\")")?;
                if let Some(&ext_id) = mat_ext_ids.get(material_type) {
                    writeln!(f, "surface_material_override/0 = ExtResource(\"{ext_id}\")")?;
                }
                writeln!(f)?;
            }
            SceneElement::Instance { name, material_type, positions, .. } => {
                for (pi, (pos, rot)) in positions.iter().enumerate() {
                    let node_name = format!("{}_{}", sanitize_name(name), pi);
                    writeln!(f, "[node name=\"{node_name}\" type=\"MeshInstance3D\" parent=\"{root_name}\"]")?;
                    write_transform3d_from_pos_rot(&mut f, *pos, *rot)?;
                    writeln!(f, "mesh = SubResource(\"{mesh_sub_id}\")")?;
                    if let Some(&ext_id) = mat_ext_ids.get(material_type) {
                        writeln!(f, "surface_material_override/0 = ExtResource(\"{ext_id}\")")?;
                    }
                    writeln!(f)?;
                }
            }
        }
    }

    Ok(())
}

// ─── Mesh type classification ──────────────────────────────────────────────

type MeshDims = (f32, f32, f32); // (width, height, depth) or (radius, height, _)

#[derive(Copy, Clone)]
enum GodotMesh {
    Box,
    Cylinder,
    Plane,
}

fn classify_mesh(name: &str, mesh_data: &crate::scene_writer::geometry::MeshData) -> (&'static str, MeshDims) {
    let (w, h, d) = aabb_dims(mesh_data);

    if name.starts_with("Tree_") {
        if h > w * 1.5 {
            ("cylinder", (w.max(d) * 0.5, h, 0.0)) // trunk
        } else {
            ("cylinder", (w.max(d) * 0.5, h, 0.0)) // canopy (cone approximated)
        }
    } else if name.starts_with("Terrain_") {
        ("plane", (w, 0.0, d))
    } else if name.starts_with("Water_") || name.starts_with("Waterway_") {
        ("plane", (w, 0.0, d))
    } else if name.starts_with("Highway_") || name.starts_with("Railway_") {
        ("box", (w, 0.1, d)) // thin road
    } else {
        // Buildings → box
        ("box", (w.max(0.5), h.max(2.0), d.max(0.5)))
    }
}

fn aabb_dims(mesh_data: &crate::scene_writer::geometry::MeshData) -> (f32, f32, f32) {
    let verts = &mesh_data.vertices;
    if verts.is_empty() {
        return (1.0, 1.0, 1.0);
    }
    let mut min = (f32::MAX, f32::MAX, f32::MAX);
    let mut max = (f32::MIN, f32::MIN, f32::MIN);
    for i in (0..verts.len()).step_by(3) {
        min.0 = min.0.min(verts[i]);
        min.1 = min.1.min(verts[i + 1]);
        min.2 = min.2.min(verts[i + 2]);
        max.0 = max.0.max(verts[i]);
        max.1 = max.1.max(verts[i + 1]);
        max.2 = max.2.max(verts[i + 2]);
    }
    (max.0 - min.0, max.1 - min.1, max.2 - min.2)
}

fn write_mesh_sub(
    f: &mut fs::File,
    id: u32,
    res_name: &str,
    mesh_type: &str,
    dims: MeshDims,
) -> io::Result<()> {
    match mesh_type {
        "box" => {
            writeln!(f, "[sub_resource type=\"BoxMesh\" id=\"{id}\"]")?;
            writeln!(f, "resource_name = \"{res_name}\"")?;
            writeln!(f, "size = Vector3({:.2}, {:.2}, {:.2})", dims.0, dims.1, dims.2)?;
        }
        "cylinder" => {
            let radius = dims.0;
            let height = dims.1;
            let top_r = if dims.2 > 0.0 { 0.0 } else { radius }; // 0=cone
            writeln!(f, "[sub_resource type=\"CylinderMesh\" id=\"{id}\"]")?;
            writeln!(f, "resource_name = \"{res_name}\"")?;
            writeln!(f, "height = {height:.2}")?;
            writeln!(f, "top_radius = {top_r:.2}")?;
            writeln!(f, "bottom_radius = {radius:.2}")?;
        }
        "plane" => {
            writeln!(f, "[sub_resource type=\"PlaneMesh\" id=\"{id}\"]")?;
            writeln!(f, "resource_name = \"{res_name}\"")?;
            writeln!(f, "size = Vector2({:.2}, {:.2})", dims.0.max(0.1), dims.2.max(0.1))?;
        }
        _ => {
            writeln!(f, "[sub_resource type=\"BoxMesh\" id=\"{id}\"]")?;
            writeln!(f, "resource_name = \"{res_name}\"")?;
            writeln!(f, "size = Vector3({:.2}, {:.2}, {:.2})", 1.0, 1.0, 1.0)?;
        }
    }
    writeln!(f)?;
    Ok(())
}

// ─── Math ───────────────────────────────────────────────────────────────────

fn write_transform3d(f: &mut fs::File, m: [f32; 12]) -> io::Result<()> {
    write!(f, "transform = Transform3D(")?;
    for (i, v) in m.iter().enumerate() {
        if i > 0 { write!(f, ", ")?; }
        if v.fract() == 0.0 { write!(f, "{}", *v as i32)?; }
        else { write!(f, "{v:.4}")?; }
    }
    writeln!(f, ")")
}

fn write_transform3d_from_pos_rot(f: &mut fs::File, pos: (f32, f32, f32), y_rot: f32) -> io::Result<()> {
    let (s, c) = y_rot.sin_cos();
    write_transform3d(f, [c, 0.0, -s, 0.0, 1.0, 0.0, s, 0.0, c, pos.0, pos.1, pos.2])
}

pub fn identity_transform() -> [f32; 12] {
    [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]
}

pub fn translation_transform(x: f32, y: f32, z: f32) -> [f32; 12] {
    [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, x, y, z]
}

fn chunk_uid(cx: i32, cz: i32) -> String {
    let h = (cx as u64).wrapping_mul(0x517cc1b7).wrapping_add(cz as u64).wrapping_mul(0x9e3779b9);
    format!("c{h:013x}")
}

fn sanitize_name(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect()
}
