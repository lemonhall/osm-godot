//! Procedural mesh primitives for Godot scene generation.
//!
//! All meshes use Godot's coordinate conventions:
//!   X = right/east, Y = up, Z = back/south (Godot forward = -Z)
//!
//! And store flat-packed vertex data suitable for writing to .tscn ArrayMesh resources.

use std::f32::consts::PI;

// ─── MeshData ───────────────────────────────────────────────────────────────

/// Raw mesh data: flat-packed vertex attributes + triangle indices.
#[derive(Clone, Debug)]
pub struct MeshData {
    /// Vertex positions: [x0, y0, z0, x1, y1, z1, ...]
    pub vertices: Vec<f32>,
    /// Vertex normals: [nx0, ny0, nz0, nx1, ny1, nz1, ...]
    pub normals: Vec<f32>,
    /// UV coordinates: [u0, v0, u1, v1, ...]
    pub uvs: Vec<f32>,
    /// Triangle indices (0-based into the vertex arrays)
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn with_capacity(n_verts: usize, n_indices: usize) -> Self {
        Self {
            vertices: Vec::with_capacity(n_verts * 3),
            normals: Vec::with_capacity(n_verts * 3),
            uvs: Vec::with_capacity(n_verts * 2),
            indices: Vec::with_capacity(n_indices),
        }
    }

    /// Number of vertices.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len() / 3
    }

    /// Append another mesh, with an optional translation offset.
    pub fn append(&mut self, other: &MeshData, offset: (f32, f32, f32)) {
        let base_idx = self.vertex_count() as u32;
        for i in (0..other.vertices.len()).step_by(3) {
            self.vertices.push(other.vertices[i] + offset.0);
            self.vertices.push(other.vertices[i + 1] + offset.1);
            self.vertices.push(other.vertices[i + 2] + offset.2);
        }
        self.normals.extend_from_slice(&other.normals);
        self.uvs.extend_from_slice(&other.uvs);
        self.indices
            .extend(other.indices.iter().map(|i| i + base_idx));
    }
}

// ─── Building helpers ───────────────────────────────────────────────────────

/// A flat-shaded quad face (two triangles).
fn push_quad(
    m: &mut MeshData,
    a: (f32, f32, f32),
    b: (f32, f32, f32),
    c: (f32, f32, f32),
    d: (f32, f32, f32),
    normal: (f32, f32, f32),
) {
    let base = m.vertex_count() as u32;

    // Vertices
    for &(x, y, z) in &[a, b, c, d] {
        m.vertices.extend_from_slice(&[x, y, z]);
        m.normals.extend_from_slice(&[normal.0, normal.1, normal.2]);
    }
    // UVs (planar unwrap, simple)
    m.uvs.extend_from_slice(&[0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0]);
    // Two triangles: a-b-d, b-c-d
    m.indices
        .extend_from_slice(&[base, base + 1, base + 3, base + 1, base + 2, base + 3]);
}

/// Make a unit box centered at origin: [-w/2..w/2, 0..h, -d/2..d/2]
pub fn make_box(w: f32, h: f32, d: f32) -> MeshData {
    let hw = w * 0.5;
    let hd = d * 0.5;
    let mut m = MeshData::with_capacity(24, 36);

    // Bottom face (Y=0)
    push_quad(
        &mut m,
        (-hw, 0.0, -hd),
        (hw, 0.0, -hd),
        (hw, 0.0, hd),
        (-hw, 0.0, hd),
        (0.0, -1.0, 0.0),
    );
    // Top face (Y=h)
    push_quad(
        &mut m,
        (-hw, h, hd),
        (hw, h, hd),
        (hw, h, -hd),
        (-hw, h, -hd),
        (0.0, 1.0, 0.0),
    );
    // Front face (-Z)
    push_quad(
        &mut m,
        (-hw, 0.0, -hd),
        (hw, 0.0, -hd),
        (hw, h, -hd),
        (-hw, h, -hd),
        (0.0, 0.0, -1.0),
    );
    // Back face (+Z)
    push_quad(
        &mut m,
        (hw, 0.0, hd),
        (-hw, 0.0, hd),
        (-hw, h, hd),
        (hw, h, hd),
        (0.0, 0.0, 1.0),
    );
    // Left face (-X)
    push_quad(
        &mut m,
        (-hw, 0.0, hd),
        (-hw, 0.0, -hd),
        (-hw, h, -hd),
        (-hw, h, hd),
        (-1.0, 0.0, 0.0),
    );
    // Right face (+X)
    push_quad(
        &mut m,
        (hw, 0.0, -hd),
        (hw, 0.0, hd),
        (hw, h, hd),
        (hw, h, -hd),
        (1.0, 0.0, 0.0),
    );

    m
}

/// Extrude a polygon outline into walls. Assumes polygon vertices are in
/// clockwise/ccw order and the polygon is simple (no self-intersection).
///
/// `thickness` is how thick the walls are (inward from the outline).
pub fn make_wall_outline(polygon: &[(f32, f32)], height: f32, thickness: f32) -> MeshData {
    if polygon.len() < 3 {
        return MeshData::new();
    }

    let n = polygon.len();
    // Estimate: each edge → outer + inner faces, plus top rim
    let mut m = MeshData::with_capacity(n * 8, n * 12);

    // Build outer + inner wall edges
    for i in 0..n {
        let j = (i + 1) % n;
        let (x0, z0) = polygon[i];
        let (x1, z1) = polygon[j];

        // Edge direction
        let dx = x1 - x0;
        let dz = z1 - z0;
        let len = (dx * dx + dz * dz).sqrt();
        if len < 0.001 {
            continue;
        }

        // Inward normal (assumes CCW polygon → inward is to the right of edge direction)
        let nx = dz / len;
        let nz = -dx / len;

        // Outer face
        let outer_tl = (x0, 0.0, z0);
        let outer_tr = (x1, 0.0, z1);
        let outer_br = (x1, height, z1);
        let outer_bl = (x0, height, z0);
        push_quad(
            &mut m,
            outer_tl,
            outer_tr,
            outer_br,
            outer_bl,
            (-nx, 0.0, -nz),
        );

        // Inner face (offset inward by thickness)
        let inner_tl = (x0 + nx * thickness, 0.0, z0 + nz * thickness);
        let inner_tr = (x1 + nx * thickness, 0.0, z1 + nz * thickness);
        let inner_br = (x1 + nx * thickness, height, z1 + nz * thickness);
        let inner_bl = (x0 + nx * thickness, height, z0 + nz * thickness);
        push_quad(&mut m, inner_tr, inner_tl, inner_bl, inner_br, (nx, 0.0, nz));
    }

    // Top rim (between outer and inner edge at y=height)
    for i in 0..n {
        let j = (i + 1) % n;
        let (x0, z0) = polygon[i];
        let (x1, z1) = polygon[j];

        let dx = x1 - x0;
        let dz = z1 - z0;
        let len = (dx * dx + dz * dz).sqrt();
        if len < 0.001 {
            continue;
        }
        let nx = dz / len;
        let nz = -dx / len;

        let o0 = (x0, height, z0);
        let o1 = (x1, height, z1);
        let i0 = (x0 + nx * thickness, height, z0 + nz * thickness);
        let i1 = (x1 + nx * thickness, height, z1 + nz * thickness);

        push_quad(&mut m, o0, o1, i1, i0, (0.0, 1.0, 0.0));
    }

    m
}

/// Flat roof as a triangulated polygon at the given height.
pub fn make_roof_flat(polygon: &[(f32, f32)], height: f32) -> MeshData {
    let mut m = MeshData::new();
    if polygon.len() < 3 {
        return m;
    }

    // Simple fan triangulation from polygon[0]
    let (cx, cz) = polygon[0];
    let base = m.vertex_count() as u32;

    for &(x, z) in polygon {
        m.vertices.extend_from_slice(&[x, height, z]);
        m.normals.extend_from_slice(&[0.0, 1.0, 0.0]);
        m.uvs.extend_from_slice(&[x * 0.1, z * 0.1]); // tiled UV
    }

    let n = polygon.len() as u32;
    for i in 1..(n - 1) {
        m.indices
            .extend_from_slice(&[base, base + i, base + i + 1]);
    }

    // Also add bottom face for the roof (visible from below)
    let base2 = m.vertex_count() as u32;
    for &(x, z) in polygon {
        m.vertices.extend_from_slice(&[x, height - 0.1, z]);
        m.normals.extend_from_slice(&[0.0, -1.0, 0.0]);
        m.uvs.extend_from_slice(&[x * 0.1, z * 0.1]);
    }
    // Reverse winding for bottom
    for i in 1..(n - 1) {
        m.indices
            .extend_from_slice(&[base2, base2 + i + 1, base2 + i]);
    }

    m
}

/// Simple gabled roof: ridge at `ridge_y`, eaves at `base_y`.
/// `ridge_dir` is (dx, dz) — the direction of the ridge line.
pub fn make_roof_gabled(
    polygon: &[(f32, f32)],
    base_y: f32,
    ridge_y: f32,
) -> MeshData {
    if polygon.len() < 4 {
        return make_roof_flat(polygon, base_y);
    }

    // Find bounding box to place ridge
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;
    for &(x, z) in polygon {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_z = min_z.min(z);
        max_z = max_z.max(z);
    }

    let width_x = max_x - min_x;
    let width_z = max_z - min_z;

    let mut m = MeshData::new();

    // Determine ridge direction based on which axis is longer
    if width_x > width_z {
        // Ridge runs along Z (peaked in X direction)
        let mid_x = (min_x + max_x) * 0.5;

        // Left slope (min_x → mid_x)
        push_quad(
            &mut m,
            (min_x, base_y, min_z),
            (min_x, base_y, max_z),
            (mid_x, ridge_y, max_z),
            (mid_x, ridge_y, min_z),
            normal_of((min_x, base_y, 0.0), (mid_x, ridge_y, 0.0), (min_x, base_y, 0.0)),
        );
        // Right slope (mid_x → max_x)
        push_quad(
            &mut m,
            (mid_x, ridge_y, min_z),
            (mid_x, ridge_y, max_z),
            (max_x, base_y, max_z),
            (max_x, base_y, min_z),
            normal_of((mid_x, ridge_y, 0.0), (max_x, base_y, 0.0), (mid_x, ridge_y, 0.0)),
        );

        // Front + back triangles (gable ends)
        add_gable_end(&mut m, (min_x, base_y, min_z), (max_x, base_y, min_z), (mid_x, ridge_y, min_z));
        add_gable_end(&mut m, (max_x, base_y, max_z), (min_x, base_y, max_z), (mid_x, ridge_y, max_z));
    } else {
        // Ridge runs along X (peaked in Z direction)
        let mid_z = (min_z + max_z) * 0.5;

        // Front slope (min_z → mid_z)
        push_quad(
            &mut m,
            (min_x, base_y, min_z),
            (max_x, base_y, min_z),
            (max_x, ridge_y, mid_z),
            (min_x, ridge_y, mid_z),
            normal_of((0.0, base_y, min_z), (0.0, ridge_y, mid_z), (0.0, base_y, min_z)),
        );
        // Back slope (mid_z → max_z)
        push_quad(
            &mut m,
            (min_x, ridge_y, mid_z),
            (max_x, ridge_y, mid_z),
            (max_x, base_y, max_z),
            (min_x, base_y, max_z),
            normal_of((0.0, ridge_y, mid_z), (0.0, base_y, max_z), (0.0, ridge_y, mid_z)),
        );

        // Left + right triangles (gable ends)
        add_gable_end(&mut m, (min_x, base_y, min_z), (min_x, base_y, max_z), (min_x, ridge_y, mid_z));
        add_gable_end(&mut m, (max_x, base_y, max_z), (max_x, base_y, min_z), (max_x, ridge_y, mid_z));
    }

    m
}

fn add_gable_end(
    m: &mut MeshData,
    bottom_left: (f32, f32, f32),
    bottom_right: (f32, f32, f32),
    peak: (f32, f32, f32),
) {
    // Normal perpendicular to the triangle
    let n = triangle_normal(bottom_left, bottom_right, peak);
    let n = (-n.0, -n.1, -n.2); // flip so both sides get correct normals

    let base = m.vertex_count() as u32;
    m.vertices.extend_from_slice(&[
        bottom_left.0, bottom_left.1, bottom_left.2,
        bottom_right.0, bottom_right.1, bottom_right.2,
        peak.0, peak.1, peak.2,
    ]);
    for _ in 0..3 {
        m.normals.extend_from_slice(&[n.0, n.1, n.2]);
    }
    m.uvs.extend_from_slice(&[0.0, 0.0, 1.0, 0.0, 0.5, 1.0]);
    m.indices.extend_from_slice(&[base, base + 1, base + 2]);
}

// ─── Road surface ───────────────────────────────────────────────────────────

/// A road surface strip following a centerline polyline.
/// Roads are flat quads at ground level (y=0).
pub fn make_road_surface(centerline: &[(f32, f32)], width: f32) -> MeshData {
    if centerline.len() < 2 {
        return MeshData::new();
    }

    let n = centerline.len();
    let mut m = MeshData::with_capacity(n * 4, (n - 1) * 6);
    let hw = width * 0.5;

    for i in 0..(n - 1) {
        let (x0, z0) = centerline[i];
        let (x1, z1) = centerline[i + 1];

        let dx = x1 - x0;
        let dz = z1 - z0;
        let len = (dx * dx + dz * dz).sqrt();
        if len < 0.001 {
            continue;
        }
        let nx = -dz / len * hw;
        let nz = dx / len * hw;

        let base = m.vertex_count() as u32;

        // Four corners of the road segment at y=0 (ground level, raised slightly)
        let y_road = 0.05;
        m.vertices.extend_from_slice(&[
            x0 - nx, y_road, z0 - nz,
            x0 + nx, y_road, z0 + nz,
            x1 + nx, y_road, z1 + nz,
            x1 - nx, y_road, z1 - nz,
        ]);
        for _ in 0..4 {
            m.normals.extend_from_slice(&[0.0, 1.0, 0.0]);
        }
        m.uvs.extend_from_slice(&[0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0]);
        m.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    m
}

// ─── Tree primitives ────────────────────────────────────────────────────────

/// Cylinder (e.g., tree trunk) at origin, extending +Y.
pub fn make_cylinder(radius: f32, height: f32, segments: u32) -> MeshData {
    let n = segments as usize;
    let mut m = MeshData::with_capacity((n + 1) * 2, n * 6);

    let angle_step = 2.0 * PI / n as f32;

    // Top center
    let top_center = m.vertex_count() as u32;
    m.vertices.extend_from_slice(&[0.0, height, 0.0]);
    m.normals.extend_from_slice(&[0.0, 1.0, 0.0]);
    m.uvs.extend_from_slice(&[0.5, 0.5]);

    // Bottom center
    let bot_center = m.vertex_count() as u32;
    m.vertices.extend_from_slice(&[0.0, 0.0, 0.0]);
    m.normals.extend_from_slice(&[0.0, -1.0, 0.0]);
    m.uvs.extend_from_slice(&[0.5, 0.5]);

    // Ring vertices
    let ring_start = m.vertex_count() as u32;
    for i in 0..n {
        let angle = i as f32 * angle_step;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        m.vertices.extend_from_slice(&[x, height, z]);
        m.normals.extend_from_slice(&[0.0, 1.0, 0.0]);
        m.uvs.extend_from_slice(&[(angle / (2.0 * PI)).fract(), 1.0]);
    }
    let ring_bot_start = m.vertex_count() as u32;
    for i in 0..n {
        let angle = i as f32 * angle_step;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        m.vertices.extend_from_slice(&[x, 0.0, z]);
        m.normals.extend_from_slice(&[0.0, -1.0, 0.0]);
        m.uvs.extend_from_slice(&[(angle / (2.0 * PI)).fract(), 0.0]);
    }

    // Top and bottom caps
    for i in 0..n {
        let j = (i + 1) % n;
        m.indices
            .extend_from_slice(&[top_center, ring_start + i as u32, ring_start + j as u32]);
        m.indices
            .extend_from_slice(&[bot_center, ring_bot_start + j as u32, ring_bot_start + i as u32]);
    }

    // Side faces
    for i in 0..n {
        let j = (i + 1) % n;
        let base = m.vertex_count() as u32;
        let a = ring_start + i as u32;
        let b = ring_start + j as u32;
        let c = ring_bot_start + j as u32;
        let d = ring_bot_start + i as u32;

        m.vertices.extend_from_slice(&[
            m.vertices[a as usize * 3], height, m.vertices[a as usize * 3 + 2],
            m.vertices[b as usize * 3], height, m.vertices[b as usize * 3 + 2],
            m.vertices[c as usize * 3], 0.0, m.vertices[c as usize * 3 + 2],
            m.vertices[d as usize * 3], 0.0, m.vertices[d as usize * 3 + 2],
        ]);

        // Side normal (radial outward)
        let angle_mid = (i as f32 + 0.5) * angle_step;
        let nx = angle_mid.cos();
        let nz = angle_mid.sin();
        for _ in 0..4 {
            m.normals.extend_from_slice(&[nx, 0.0, nz]);
        }
        m.uvs.extend_from_slice(&[0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0]);
        m.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    m
}

/// Cone (e.g., tree canopy) at origin, pointing +Y.
pub fn make_cone(radius: f32, height: f32, segments: u32) -> MeshData {
    let n = segments as usize;
    let mut m = MeshData::with_capacity(n * 2 + 2, n * 3);

    let angle_step = 2.0 * PI / n as f32;

    // Tip
    let tip = m.vertex_count() as u32;
    m.vertices.extend_from_slice(&[0.0, height, 0.0]);
    m.normals.extend_from_slice(&[0.0, 1.0, 0.0]);
    m.uvs.extend_from_slice(&[0.5, 1.0]);

    // Base ring
    let base_start = m.vertex_count() as u32;
    for i in 0..n {
        let angle = i as f32 * angle_step;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        m.vertices.extend_from_slice(&[x, 0.0, z]);
        m.normals.extend_from_slice(&[0.0, -1.0, 0.0]);
        m.uvs.extend_from_slice(&[(angle / (2.0 * PI)).fract(), 0.0]);
    }

    // Base center
    let base_center = m.vertex_count() as u32;
    m.vertices.extend_from_slice(&[0.0, 0.0, 0.0]);
    m.normals.extend_from_slice(&[0.0, -1.0, 0.0]);
    m.uvs.extend_from_slice(&[0.5, 0.0]);

    // Cone sides + base
    for i in 0..n {
        let j = (i + 1) % n;
        // Side triangle
        m.indices
            .extend_from_slice(&[tip, base_start + i as u32, base_start + j as u32]);
        // Base triangle
        m.indices
            .extend_from_slice(&[base_center, base_start + j as u32, base_start + i as u32]);
    }

    m
}

// ─── Math helpers ───────────────────────────────────────────────────────────

fn triangle_normal(
    a: (f32, f32, f32),
    b: (f32, f32, f32),
    c: (f32, f32, f32),
) -> (f32, f32, f32) {
    let u = (b.0 - a.0, b.1 - a.1, b.2 - a.2);
    let v = (c.0 - a.0, c.1 - a.1, c.2 - a.2);
    let nx = u.1 * v.2 - u.2 * v.1;
    let ny = u.2 * v.0 - u.0 * v.2;
    let nz = u.0 * v.1 - u.1 * v.0;
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if len > 0.0 {
        (nx / len, ny / len, nz / len)
    } else {
        (0.0, 1.0, 0.0)
    }
}

fn normal_of(
    a: (f32, f32, f32),
    b: (f32, f32, f32),
    _c: (f32, f32, f32),
) -> (f32, f32, f32) {
    // Simplified: just use Y-up normal for roof slopes
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let dz = b.2 - a.2;
    let len = (dx * dx + dy * dy + dz * dz).sqrt();
    if len > 0.0 {
        // Assume quads span the X or Z axis, compute the cross product
        let n = triangle_normal(a, b, (a.0, b.1, a.2));
        n
    } else {
        (0.0, 1.0, 0.0)
    }
}
