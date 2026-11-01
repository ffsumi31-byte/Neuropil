//! builders.rs — procedural generators. Each object is a recipe of sharp cubes
//! (position, size, colour, chiselled corners) built in Rust, with no model or API call.

use crate::geom::{self, Cube};

pub const KINDS: &[&str] = &[
    "tower", "pyramid", "house", "castle", "character", "sword", "chair", "table", "tree", "car",
    "bridge", "stairs", "rocket", "arch", "wall", "cube",
];

#[derive(Debug, Clone)]
pub struct Params {
    pub scale: f32,
    pub count: Option<u32>,
    pub color: Option<String>,
    pub mat: Option<String>,
    pub seed: u64,
}

fn pc(p: &Params, default: &str) -> String {
    p.color.clone().unwrap_or_else(|| default.to_string())
}

/// A box standing on `y` (its bottom face), centred on x / z.
fn bx(part: &str, x: f32, y: f32, z: f32, w: f32, h: f32, d: f32, color: &str) -> Cube {
    Cube::new(part, [x, y + h / 2.0, z], [w, h, d], color)
}

fn shape(mut c: Cube, mode: &str, f: f32) -> Cube {
    let _ = geom::taper(&mut c, mode, f);
    c
}

fn rnd(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*seed >> 40) as f32) / 16777216.0
}

pub fn build(kind: &str, p: &Params) -> Vec<Cube> {
    let mut v: Vec<Cube> = match kind {
        "tower" => tower(p),
        "pyramid" => pyramid(p),
        "house" => house(p),
        "castle" => castle(p),
        "character" => character(p),
        "sword" => sword(p),
        "chair" => chair(p),
        "table" => table(p),
        "tree" => tree(p),
        "car" => car(p),
        "bridge" => bridge(p),
        "stairs" => stairs(p),
        "rocket" => rocket(p),
        "arch" => arch(p),
        "wall" => wall(p),
        "cube" => vec![bx("cube", 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, &pc(p, "#5EB58B"))],
        _ => Vec::new(),
    };
    for c in v.iter_mut() {
        for k in 0..3 {
            c.pos[k] *= p.scale;
            c.size[k] *= p.scale;
        }
        if let Some(m) = &p.mat {
            c.mat = m.clone();
        }
    }
    v
}

fn tower(p: &Params) -> Vec<Cube> {
    let n = p.count.unwrap_or(4).max(1).min(30);
    let main = pc(p, "#12879B");
    let mut v = vec![bx("base", 0.0, 0.0, 0.0, 3.2, 0.5, 3.2, "#1E293B")];
    let mut y = 0.5_f32;
    let mut w = 2.4_f32;
    for i in 0..n {
        let col = if i % 2 == 0 { main.clone() } else { "#5EB58B".to_string() };
        v.push(bx(&format!("segment_{}", i + 1), 0.0, y, 0.0, w, 1.3, w, &col));
        y += 1.3;
        w *= 0.87;
    }
    v.push(shape(bx("spire", 0.0, y, 0.0, w * 1.1, 2.0, w * 1.1, "#E2E8F0"), "top-in", 0.04));
    v
}

fn pyramid(p: &Params) -> Vec<Cube> {
    let n = p.count.unwrap_or(6).max(1).min(30);
    let main = pc(p, "#D4AF37");
    let mut v = Vec::new();
    for i in 0..n {
        let w = (n - i) as f32 * 0.8 + 0.4;
        let col = if i % 2 == 0 { main.clone() } else { "#E8D8B0".to_string() };
        v.push(bx(&format!("layer_{}", i + 1), 0.0, i as f32 * 0.7, 0.0, w, 0.7, w, &col));
    }
    v
}

fn house(p: &Params) -> Vec<Cube> {
    let floors = p.count.unwrap_or(1).max(1).min(8);
    let wall = pc(p, "#E8D8B0");
    let mut v = Vec::new();
    for f in 0..floors {
        let y = f as f32 * 2.2;
        v.push(bx(&format!("wall_{}", f + 1), 0.0, y, 0.0, 4.2, 2.2, 3.4, &wall));
        v.push(bx(&format!("window_{}_l", f + 1), -1.2, y + 1.0, 1.72, 0.8, 0.8, 0.1, "#93C5FD"));
        v.push(bx(&format!("window_{}_r", f + 1), 1.2, y + 1.0, 1.72, 0.8, 0.8, 0.1, "#93C5FD"));
    }
    v.push(bx("door", 0.0, 0.0, 1.72, 0.9, 1.6, 0.1, "#7C4A21"));
    let top = floors as f32 * 2.2;
    v.push(shape(bx("roof", 0.0, top, 0.0, 4.8, 1.7, 3.8, "#B91C1C"), "ridge", 0.02));
    v.push(bx("chimney", 1.3, top, -0.8, 0.5, 1.4, 0.5, "#57534E"));
    v
}

fn castle(p: &Params) -> Vec<Cube> {
    let stone = pc(p, "#94A3B8");
    let mut v = Vec::new();
    for (i, (x, z)) in [(-3.0f32, -3.0f32), (3.0, -3.0), (-3.0, 3.0), (3.0, 3.0)].iter().enumerate() {
        v.push(bx(&format!("tower_{}", i + 1), *x, 0.0, *z, 1.8, 5.0, 1.8, &stone));
        v.push(shape(bx(&format!("tower_{}_cap", i + 1), *x, 5.0, *z, 2.2, 1.6, 2.2, "#B91C1C"), "top-in", 0.05));
    }
    v.push(bx("wall_front", 0.0, 0.0, 3.0, 4.2, 3.0, 0.7, &stone));
    v.push(bx("wall_back", 0.0, 0.0, -3.0, 4.2, 3.0, 0.7, &stone));
    v.push(bx("wall_left", -3.0, 0.0, 0.0, 0.7, 3.0, 4.2, &stone));
    v.push(bx("wall_right", 3.0, 0.0, 0.0, 0.7, 3.0, 4.2, &stone));
    v.push(bx("gate", 0.0, 0.0, 3.4, 1.4, 2.0, 0.1, "#3B2A1A"));
    v.push(bx("keep", 0.0, 0.0, 0.0, 2.6, 6.5, 2.6, "#CBD5E1"));
    v.push(shape(bx("keep_cap", 0.0, 6.5, 0.0, 3.0, 2.0, 3.0, "#12879B"), "top-in", 0.05));
    v
}

fn character(p: &Params) -> Vec<Cube> {
    let shirt = pc(p, "#12879B");
    let skin = "#F1C27D";
    vec![
        bx("leg_l", -0.42, 0.0, 0.0, 0.76, 1.7, 0.8, "#1E293B"),
        bx("leg_r", 0.42, 0.0, 0.0, 0.76, 1.7, 0.8, "#1E293B"),
        bx("torso", 0.0, 1.7, 0.0, 1.6, 1.7, 0.9, &shirt),
        bx("arm_l", -1.2, 1.75, 0.0, 0.7, 1.6, 0.7, skin),
        bx("arm_r", 1.2, 1.75, 0.0, 0.7, 1.6, 0.7, skin),
        bx("head", 0.0, 3.4, 0.0, 1.2, 1.2, 1.2, skin),
        bx("eye_l", -0.3, 3.9, 0.62, 0.22, 0.22, 0.06, "#0B0F19"),
        bx("eye_r", 0.3, 3.9, 0.62, 0.22, 0.22, 0.06, "#0B0F19"),
    ]
}

fn sword(p: &Params) -> Vec<Cube> {
    let blade = pc(p, "#CBD5E1");
    vec![
        bx("pommel", 0.0, 0.0, 0.0, 0.45, 0.35, 0.45, "#D4AF37"),
        bx("handle", 0.0, 0.35, 0.0, 0.3, 1.3, 0.3, "#5B3A1E"),
        bx("guard", 0.0, 1.65, 0.0, 1.7, 0.28, 0.5, "#D4AF37"),
        shape(bx("blade", 0.0, 1.93, 0.0, 0.55, 4.4, 0.14, &blade), "top-in", 0.08),
    ]
}

fn chair(p: &Params) -> Vec<Cube> {
    let wood = pc(p, "#8B5A2B");
    let mut v = vec![
        bx("seat", 0.0, 1.0, 0.0, 1.4, 0.2, 1.4, &wood),
        bx("backrest", 0.0, 1.2, -0.6, 1.4, 1.4, 0.2, &wood),
    ];
    for (i, (x, z)) in [(-0.55f32, -0.55f32), (0.55, -0.55), (-0.55, 0.55), (0.55, 0.55)].iter().enumerate() {
        v.push(bx(&format!("leg_{}", i + 1), *x, 0.0, *z, 0.22, 1.0, 0.22, &wood));
    }
    v
}

fn table(p: &Params) -> Vec<Cube> {
    let wood = pc(p, "#8B5A2B");
    let mut v = vec![bx("tabletop", 0.0, 1.2, 0.0, 3.0, 0.2, 1.8, &wood)];
    for (i, (x, z)) in [(-1.25f32, -0.7f32), (1.25, -0.7), (-1.25, 0.7), (1.25, 0.7)].iter().enumerate() {
        v.push(bx(&format!("leg_{}", i + 1), *x, 0.0, *z, 0.25, 1.2, 0.25, "#6B4423"));
    }
    v
}

fn tree(p: &Params) -> Vec<Cube> {
    let leaf = pc(p, "#3F9B5A");
    vec![
        bx("trunk", 0.0, 0.0, 0.0, 0.7, 1.8, 0.7, "#6B4423"),
        shape(bx("leaves_1", 0.0, 1.5, 0.0, 3.0, 1.4, 3.0, &leaf), "top-in", 0.35),
        shape(bx("leaves_2", 0.0, 2.6, 0.0, 2.3, 1.3, 2.3, &leaf), "top-in", 0.35),
        shape(bx("leaves_3", 0.0, 3.6, 0.0, 1.6, 1.3, 1.6, &leaf), "top-in", 0.05),
    ]
}

fn car(p: &Params) -> Vec<Cube> {
    let body = pc(p, "#DC2626");
    let mut v = vec![
        bx("body", 0.0, 0.45, 0.0, 3.6, 0.8, 1.7, &body),
        shape(bx("cabin", -0.2, 1.25, 0.0, 1.9, 0.75, 1.5, "#93C5FD"), "top-in", 0.78),
        bx("light_l", 1.82, 0.8, -0.55, 0.08, 0.25, 0.35, "#FACC15"),
        bx("light_r", 1.82, 0.8, 0.55, 0.08, 0.25, 0.35, "#FACC15"),
    ];
    for (i, (x, z)) in [(-1.2f32, -0.9f32), (1.2, -0.9), (-1.2, 0.9), (1.2, 0.9)].iter().enumerate() {
        v.push(bx(&format!("wheel_{}", i + 1), *x, 0.0, *z, 0.9, 0.9, 0.4, "#0B0F19"));
    }
    v
}

fn bridge(p: &Params) -> Vec<Cube> {
    let spans = p.count.unwrap_or(3).max(1).min(12);
    let len = spans as f32 * 3.0;
    let stone = pc(p, "#94A3B8");
    let mut v = vec![
        bx("deck", 0.0, 1.6, 0.0, len, 0.3, 2.4, &stone),
        bx("rail_l", 0.0, 1.9, -1.1, len, 0.5, 0.15, "#CBD5E1"),
        bx("rail_r", 0.0, 1.9, 1.1, len, 0.5, 0.15, "#CBD5E1"),
    ];
    for i in 0..=spans {
        let x = -len / 2.0 + i as f32 * 3.0;
        v.push(bx(&format!("pillar_{}", i + 1), x, 0.0, 0.0, 0.7, 1.6, 1.8, "#64748B"));
    }
    v
}

fn stairs(p: &Params) -> Vec<Cube> {
    let n = p.count.unwrap_or(6).max(1).min(30);
    let col = pc(p, "#94A3B8");
    let mut v = Vec::new();
    for i in 0..n {
        let c = if i % 2 == 0 { col.clone() } else { "#CBD5E1".to_string() };
        v.push(bx(&format!("step_{}", i + 1), i as f32 * 0.9, 0.0, 0.0, 0.9, (i + 1) as f32 * 0.5, 2.2, &c));
    }
    v
}

fn rocket(p: &Params) -> Vec<Cube> {
    let body = pc(p, "#F1F5F9");
    vec![
        bx("engine", 0.0, 0.0, 0.0, 0.9, 0.5, 0.9, "#334155"),
        bx("body", 0.0, 0.5, 0.0, 1.4, 4.0, 1.4, &body),
        shape(bx("nose", 0.0, 4.5, 0.0, 1.4, 1.6, 1.4, "#DC2626"), "top-in", 0.05),
        bx("window", 0.0, 3.2, 0.72, 0.6, 0.6, 0.08, "#22D3EE"),
        bx("fin_l", -0.9, 0.5, 0.0, 0.5, 1.5, 0.9, "#DC2626"),
        bx("fin_r", 0.9, 0.5, 0.0, 0.5, 1.5, 0.9, "#DC2626"),
        bx("fin_f", 0.0, 0.5, 0.9, 0.9, 1.5, 0.5, "#DC2626"),
        bx("fin_b", 0.0, 0.5, -0.9, 0.9, 1.5, 0.5, "#DC2626"),
    ]
}

fn arch(p: &Params) -> Vec<Cube> {
    let stone = pc(p, "#94A3B8");
    vec![
        bx("pillar_l", -1.7, 0.0, 0.0, 1.0, 4.0, 1.3, &stone),
        bx("pillar_r", 1.7, 0.0, 0.0, 1.0, 4.0, 1.3, &stone),
        bx("lintel", 0.0, 4.0, 0.0, 4.6, 0.9, 1.3, &stone),
        shape(bx("keystone", 0.0, 4.9, 0.0, 1.2, 0.7, 1.3, "#D4AF37"), "ridge", 0.4),
    ]
}

fn wall(p: &Params) -> Vec<Cube> {
    let n = p.count.unwrap_or(6).max(1).min(40);
    let stone = pc(p, "#94A3B8");
    let mut v = vec![bx("wall", 0.0, 0.0, 0.0, n as f32, 2.0, 0.6, &stone)];
    for i in 0..n {
        if i % 2 == 0 {
            let x = -(n as f32) / 2.0 + 0.5 + i as f32;
            v.push(bx(&format!("merlon_{}", i + 1), x, 2.0, 0.0, 0.7, 0.5, 0.6, &stone));
        }
    }
    v
}

/// Fallback when the object is unknown: a deterministic abstract tower seeded by the prompt.
pub fn sculpture(seed: u64, color: Option<String>) -> Vec<Cube> {
    let mut s = seed;
    let palette = ["#12879B", "#5EB58B", "#E2E8F0", "#D4AF37", "#8B5CF6"];
    let n = 5 + (rnd(&mut s) * 3.0) as usize;
    let mut v = Vec::new();
    let mut y = 0.0f32;
    let mut w = 2.8f32 + rnd(&mut s);
    for i in 0..n {
        let h = 0.8 + rnd(&mut s) * 0.9;
        let ox = (rnd(&mut s) - 0.5) * 0.5;
        let oz = (rnd(&mut s) - 0.5) * 0.5;
        let depth = w * (0.8 + rnd(&mut s) * 0.4);
        let col = match &color {
            Some(c) => c.clone(),
            None => palette[i % palette.len()].to_string(),
        };
        let mut c = bx(&format!("block_{}", i + 1), ox, y, oz, w, h, depth, &col);
        if i + 1 == n {
            let _ = geom::taper(&mut c, "top-in", 0.1);
        }
        v.push(c);
        y += h;
        w *= 0.82;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> Params {
        Params { scale: 1.0, count: None, color: None, mat: None, seed: 7 }
    }

    #[test]
    fn every_kind_builds_something_grounded() {
        for kind in KINDS.iter() {
            let cubes = build(kind, &params());
            assert!(!cubes.is_empty(), "{} built nothing", kind);
            let (lo, _) = geom::bounds(&cubes).unwrap();
            assert!(lo[1].abs() < 0.01, "{} floats at y={}", kind, lo[1]);
        }
    }

    #[test]
    fn tower_floor_count_changes_height() {
        let mut p = params();
        p.count = Some(2);
        let short = geom::bounds(&build("tower", &p)).unwrap().1[1];
        p.count = Some(8);
        let tall = geom::bounds(&build("tower", &p)).unwrap().1[1];
        assert!(tall > short + 5.0);
    }
}
