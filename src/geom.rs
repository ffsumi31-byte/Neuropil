//! geom.rs — the sharp-cube kernel: cube data, extrude / taper / push / mirror,
//! bounds and OBJ export. Everything the 3D model *is* lives here, in Rust.

use serde::{Deserialize, Serialize};
use std::fmt::Write;

pub type V3 = [f32; 3];
pub type Corners = [[f32; 3]; 8];

/// The 8 corners of a unit cube. Order matches the browser renderer.
pub const UNIT_CORNERS: Corners = [
    [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [-0.5, 0.5, -0.5],
    [-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5],
];

/// Quad faces as corner indices (same winding as the browser renderer).
pub const FACES: [(&str, [usize; 4]); 6] = [
    ("back", [0, 3, 2, 1]),
    ("front", [4, 5, 6, 7]),
    ("bottom", [0, 1, 5, 4]),
    ("top", [3, 7, 6, 2]),
    ("left", [0, 4, 7, 3]),
    ("right", [1, 2, 6, 5]),
];

fn default_color() -> String {
    "#5EB58B".to_string()
}
fn default_mat() -> String {
    "matte".to_string()
}
fn unit_corners() -> Corners {
    UNIT_CORNERS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cube {
    pub name: String,
    #[serde(default)]
    pub role: String,
    /// Semantic part name, e.g. "roof" or "leg_l". The AI targets edits by this.
    #[serde(default)]
    pub part: String,
    /// Which build this cube belongs to, e.g. "house_2".
    #[serde(default)]
    pub group: String,
    pub pos: V3,
    pub size: V3,
    /// Euler rotation in degrees (XYZ order).
    #[serde(default)]
    pub rot: V3,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_mat")]
    pub mat: String,
    #[serde(default = "unit_corners")]
    pub corners: Corners,
}

impl Cube {
    pub fn new(part: &str, pos: V3, size: V3, color: &str) -> Cube {
        Cube {
            name: part.to_string(),
            role: String::new(),
            part: part.to_string(),
            group: String::new(),
            pos,
            size,
            rot: [0.0, 0.0, 0.0],
            color: color.to_string(),
            mat: "matte".to_string(),
            corners: UNIT_CORNERS,
        }
    }

    /// World-space position of corner `i` (scale, then rotation, then translation).
    pub fn world_corner(&self, i: usize) -> V3 {
        let c = self.corners[i];
        let local = [c[0] * self.size[0], c[1] * self.size[1], c[2] * self.size[2]];
        let r = rotate(local, self.rot);
        [r[0] + self.pos[0], r[1] + self.pos[1], r[2] + self.pos[2]]
    }
}

/// Rotate a vector by Euler angles in degrees (Z first, then Y, then X — three.js "XYZ").
pub fn rotate(v: V3, rot_deg: V3) -> V3 {
    let (sx, cx) = rot_deg[0].to_radians().sin_cos();
    let (sy, cy) = rot_deg[1].to_radians().sin_cos();
    let (sz, cz) = rot_deg[2].to_radians().sin_cos();
    let a = [v[0] * cz - v[1] * sz, v[0] * sz + v[1] * cz, v[2]];
    let b = [a[0] * cy + a[2] * sy, a[1], -a[0] * sy + a[2] * cy];
    [b[0], b[1] * cx - b[2] * sx, b[1] * sx + b[2] * cx]
}

/// Axis-aligned bounds (min, max) of every cube corner in world space.
pub fn bounds(ws: &[Cube]) -> Option<(V3, V3)> {
    if ws.is_empty() {
        return None;
    }
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for c in ws.iter() {
        for i in 0..8 {
            let w = c.world_corner(i);
            for k in 0..3 {
                if w[k] < lo[k] {
                    lo[k] = w[k];
                }
                if w[k] > hi[k] {
                    hi[k] = w[k];
                }
            }
        }
    }
    Some((lo, hi))
}

fn scale_corners(c: &mut Cube, idx: &[usize], on_x: bool, on_z: bool, f: f32) {
    for &i in idx.iter() {
        if on_x {
            c.corners[i][0] *= f;
        }
        if on_z {
            c.corners[i][2] *= f;
        }
    }
}

/// Chisel a cube by moving its corners. Modes: top-in, point, bot-in, ridge, front-edge, wedge-top.
pub fn taper(c: &mut Cube, mode: &str, factor: f32) -> Result<(), String> {
    let f = if factor.is_finite() { factor.max(0.02).min(1.5) } else { 0.5 };
    match mode {
        "top-in" | "point" => scale_corners(c, &[2, 3, 6, 7], true, true, f),
        "bot-in" => scale_corners(c, &[0, 1, 4, 5], true, true, f),
        "ridge" => scale_corners(c, &[2, 3, 6, 7], true, false, f),
        "front-edge" => scale_corners(c, &[4, 5, 6, 7], true, false, 0.08),
        "wedge-top" => {
            c.corners[3][1] = -0.45;
            c.corners[2][1] = -0.45;
        }
        other => {
            return Err(format!(
                "unknown taper mode '{}' (use top-in, bot-in, ridge, front-edge, wedge-top)",
                other
            ))
        }
    }
    Ok(())
}

/// Grow a new cube out of one face of `src`.
pub fn extrude(src: &Cube, face: &str, dist: f32, inset: f32, color: Option<&str>) -> Result<Cube, String> {
    let p = src.pos;
    let s = src.size;
    let d = dist.max(0.05);
    let (size, pos) = match face {
        "top" => ([s[0] * inset, d, s[2] * inset], [p[0], p[1] + s[1] / 2.0 + d / 2.0, p[2]]),
        "bottom" => ([s[0] * inset, d, s[2] * inset], [p[0], p[1] - s[1] / 2.0 - d / 2.0, p[2]]),
        "front" => ([s[0] * inset, s[1] * inset, d], [p[0], p[1], p[2] + s[2] / 2.0 + d / 2.0]),
        "back" => ([s[0] * inset, s[1] * inset, d], [p[0], p[1], p[2] - s[2] / 2.0 - d / 2.0]),
        "right" => ([d, s[1] * inset, s[2] * inset], [p[0] + s[0] / 2.0 + d / 2.0, p[1], p[2]]),
        "left" => ([d, s[1] * inset, s[2] * inset], [p[0] - s[0] / 2.0 - d / 2.0, p[1], p[2]]),
        other => {
            return Err(format!(
                "unknown face '{}' (use top, bottom, left, right, front, back)",
                other
            ))
        }
    };
    let mut c = Cube::new(face, pos, size, color.unwrap_or(src.color.as_str()));
    c.name = format!("{}_{}", src.name, face);
    c.role = format!("Extruded {} from {}", face, src.name);
    c.mat = src.mat.clone();
    c.group = src.group.clone();
    Ok(c)
}

/// Push or pull one face of a cube along an axis ("x+", "x-", "y+", ...).
pub fn push_face(c: &mut Cube, axis: &str, amount: f32) -> Result<(), String> {
    let (i, sign) = match axis {
        "x+" => (0usize, 1.0f32),
        "x-" => (0, -1.0),
        "y+" => (1, 1.0),
        "y-" => (1, -1.0),
        "z+" => (2, 1.0),
        "z-" => (2, -1.0),
        other => return Err(format!("unknown axis '{}' (use x+, x-, y+, y-, z+, z-)", other)),
    };
    let new_size = (c.size[i] + amount).max(0.1);
    let applied = new_size - c.size[i];
    c.size[i] = new_size;
    c.pos[i] += sign * applied / 2.0;
    Ok(())
}

/// Reflect a cube across the plane x = plane_x. Corners are re-indexed so faces keep facing outward.
pub fn mirror_x(c: &Cube, plane_x: f32) -> Cube {
    let swap = [1usize, 0, 3, 2, 5, 4, 7, 6];
    let mut m = c.clone();
    for i in 0..8 {
        let o = c.corners[i];
        m.corners[swap[i]] = [-o[0], o[1], o[2]];
    }
    m.pos[0] = 2.0 * plane_x - c.pos[0];
    m.rot = [c.rot[0], -c.rot[1], -c.rot[2]];
    m.name = format!("{}_mirror", c.name);
    m
}

/// Wavefront OBJ with every transform baked into world coordinates. No helpers, no gizmos.
pub fn to_obj(ws: &[Cube]) -> String {
    let mut out = String::from("# Neuropil sharp-cube export (generated by the Rust engine)\n");
    let mut base: usize = 0;
    for c in ws.iter() {
        let _ = writeln!(out, "o {}", c.name.replace(' ', "_"));
        for i in 0..8 {
            let w = c.world_corner(i);
            let _ = writeln!(out, "v {:.5} {:.5} {:.5}", w[0], w[1], w[2]);
        }
        for (_, f) in FACES.iter() {
            let _ = writeln!(out, "f {} {} {} {}", base + f[0] + 1, base + f[1] + 1, base + f[2] + 1, base + f[3] + 1);
        }
        base += 8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrude_top_stacks_on_top() {
        let src = Cube::new("origin", [0.0, 0.5, 0.0], [1.0, 1.0, 1.0], "#fff");
        let c = extrude(&src, "top", 1.0, 0.8, None).unwrap();
        assert!((c.pos[1] - 1.5).abs() < 1e-4);
        assert!((c.size[0] - 0.8).abs() < 1e-4);
    }

    #[test]
    fn mirror_lands_on_the_other_side() {
        let src = Cube::new("a", [2.0, 0.5, 0.0], [1.0, 1.0, 1.0], "#fff");
        let m = mirror_x(&src, 0.0);
        assert!((m.pos[0] + 2.0).abs() < 1e-4);
    }

    #[test]
    fn obj_has_8_vertices_per_cube() {
        let ws = vec![Cube::new("a", [0.0, 0.5, 0.0], [1.0, 1.0, 1.0], "#fff")];
        let obj = to_obj(&ws);
        assert_eq!(obj.lines().filter(|l| l.starts_with("v ")).count(), 8);
        assert_eq!(obj.lines().filter(|l| l.starts_with("f ")).count(), 6);
    }

    #[test]
    fn unknown_taper_mode_is_an_error() {
        let mut c = Cube::new("a", [0.0, 0.5, 0.0], [1.0, 1.0, 1.0], "#fff");
        assert!(taper(&mut c, "nonsense", 0.5).is_err());
    }
}
