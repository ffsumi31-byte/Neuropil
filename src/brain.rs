//! brain.rs — the Neuropil AI. No API, no model download: a planner written in Rust.
//!
//! prompt → clauses → intent + entities → plan → execute on the cube workspace →
//! self-check (grounding, bounds, unique names) → plain-English reply.
//! Same prompt + same workspace always gives the same result.

use crate::builders::{self, Params};
use crate::geom::{self, Cube};
use crate::nlu::{self, has_any};
use serde::{Deserialize, Serialize};

const DIRECTIONS: &[&str] = &[
    "left", "right", "up", "down", "front", "back", "top", "bottom", "forward", "backward",
];
const BUILD_VERBS: &[&str] = &[
    "build", "make", "create", "generate", "add", "spawn", "construct", "design", "place", "put", "give",
    "sculpt", "craft", "model",
];
const FRESH: &[&str] = &[
    "a", "an", "new", "another", "some", "build", "create", "generate", "add", "spawn", "construct",
];
const REFS: &[&str] = &[
    "the", "it", "this", "that", "its", "everything", "all", "whole", "these", "them", "those",
];
const SCALE_VERBS: &[&str] = &["scale", "resize", "grow", "shrink", "enlarge", "stretch"];
const SCALE_ADJ: &[&str] = &[
    "bigger", "larger", "smaller", "taller", "shorter", "wider", "thinner", "narrower", "slimmer", "fatter",
    "thicker", "higher",
];
const MAX_CUBES: usize = 600;

// ───────────────────────────── API types ─────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AiRequest {
    pub prompt: String,
    #[serde(default)]
    pub workspace: Vec<Cube>,
    #[serde(default)]
    pub selected: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StepLog {
    pub op: String,
    pub detail: String,
    pub ok: bool,
}

#[derive(Debug, Serialize)]
pub struct AiResponse {
    pub reply: String,
    pub intent: String,
    pub confidence: f32,
    pub steps: Vec<StepLog>,
    pub workspace: Vec<Cube>,
    pub suggestions: Vec<String>,
    pub action: Option<String>,
    pub changed: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpRequest {
    pub op: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub face: Option<String>,
    #[serde(default)]
    pub axis: Option<String>,
    #[serde(default)]
    pub amount: Option<f32>,
    #[serde(default)]
    pub dist: Option<f32>,
    #[serde(default)]
    pub inset: Option<f32>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub factor: Option<f32>,
    #[serde(default)]
    pub workspace: Vec<Cube>,
}

#[derive(Debug, Serialize)]
pub struct OpResponse {
    pub workspace: Vec<Cube>,
    pub changed: Vec<String>,
    pub message: String,
}

// ───────────────────────────── helpers ─────────────────────────────

fn origin_cube() -> Cube {
    let mut c = Cube::new("Origin_Cube", [0.0, 0.5, 0.0], [1.0, 1.0, 1.0], "#5EB58B");
    c.part = "origin".to_string();
    c.role = "Root 1x1x1 sharp origin cube".to_string();
    c
}

fn is_pristine_origin(ws: &[Cube]) -> bool {
    ws.len() == 1 && ws[0].name == "Origin_Cube" && ws[0].group.is_empty()
}

fn group_kind(group: &str) -> String {
    group.split('_').next().unwrap_or("").to_string()
}

fn next_group(ws: &[Cube], kind: &str) -> String {
    let prefix = format!("{}_", kind);
    let mut max_n = 0u32;
    for c in ws.iter() {
        if let Some(rest) = c.group.strip_prefix(prefix.as_str()) {
            if let Ok(n) = rest.parse::<u32>() {
                if n > max_n {
                    max_n = n;
                }
            }
        }
    }
    format!("{}_{}", kind, max_n + 1)
}

fn unique_name(ws: &[Cube], base: &str) -> String {
    if !ws.iter().any(|c| c.name == base) {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let cand = format!("{}_{}", base, n);
        if !ws.iter().any(|c| c.name == cand) {
            return cand;
        }
        n += 1;
    }
}

fn pick<'a>(seed: u64, options: &[&'a str]) -> &'a str {
    options[(seed % options.len() as u64) as usize]
}

struct Ctx {
    ws: Vec<Cube>,
    selected: Option<String>,
    seed: u64,
    log: Vec<StepLog>,
    notes: Vec<String>,
    intents: Vec<String>,
    unknown: Vec<String>,
    talk: Option<String>,
    action: Option<String>,
    ground: Vec<String>,
}

impl Ctx {
    fn step(&mut self, ok: bool, op: &str, detail: String) {
        self.log.push(StepLog { op: op.to_string(), detail, ok });
    }
    fn fail(&mut self, op: &str, detail: &str) {
        self.step(false, op, detail.to_string());
        self.notes.push(format!("I couldn't do that: {}", detail));
    }
}

// ───────────────────────────── intent ─────────────────────────────

#[derive(Debug, PartialEq, Clone, Copy)]
enum Intent {
    Greet,
    Help,
    Thanks,
    Undo,
    Clear,
    Extrude,
    Taper,
    Mirror,
    Duplicate,
    Delete,
    Move,
    Scale,
    Paint,
    Build,
    Unknown,
}

fn classify(t: &[String], ws: &[Cube]) -> Intent {
    if has_any(t, &["undo", "revert"]) {
        return Intent::Undo;
    }
    if has_any(t, &["clear", "reset", "wipe", "restart", "empty"]) || (has_any(t, &["start"]) && has_any(t, &["over", "fresh"])) {
        return Intent::Clear;
    }
    if has_any(t, &["help", "commands", "examples", "capabilities"]) || (has_any(t, &["what"]) && has_any(t, &["can", "do"])) {
        return Intent::Help;
    }
    if t.len() <= 3 && has_any(t, &["hi", "hello", "hey", "yo", "sup", "hola"]) {
        return Intent::Greet;
    }
    if t.len() <= 4 && has_any(t, &["thanks", "thank", "thx", "cool", "nice", "great", "awesome"]) {
        return Intent::Thanks;
    }
    if has_any(t, &["extrude", "extrusion"]) {
        return Intent::Extrude;
    }
    if has_any(t, &["taper", "sharpen", "chisel", "bevel", "wedge", "ridge"]) {
        return Intent::Taper;
    }
    if has_any(t, &["mirror", "symmetric", "symmetry", "reflect"]) {
        return Intent::Mirror;
    }
    if has_any(t, &["duplicate", "copy", "copies", "clone", "array", "repeat"]) {
        return Intent::Duplicate;
    }
    if has_any(t, &["delete", "remove", "erase", "destroy", "discard"]) {
        return Intent::Delete;
    }
    if has_any(t, &["move", "shift", "slide", "raise", "lower", "lift", "nudge", "translate"]) {
        return Intent::Move;
    }
    let fresh = has_any(t, FRESH);
    let refs = has_any(t, REFS);
    let styled = t.iter().any(|w| nlu::color_of(w).is_some()) || nlu::material_of(t).is_some();
    if has_any(t, &["paint", "colour", "color", "recolor", "recolour", "tint", "dye"]) {
        return Intent::Paint;
    }
    if has_any(t, SCALE_VERBS) || (has_any(t, SCALE_ADJ) && !fresh) {
        return Intent::Scale;
    }
    if styled && !ws.is_empty() && refs && !fresh {
        return Intent::Paint;
    }
    if nlu::find_object(t).is_some() {
        return Intent::Build;
    }
    if has_any(t, BUILD_VERBS) && !has_any(t, &["it", "this", "that", "its", "them"]) {
        return Intent::Build;
    }
    Intent::Unknown
}

fn run_clause(ctx: &mut Ctx, t: &[String]) {
    let intent = classify(t, &ctx.ws);
    match intent {
        Intent::Greet => {
            ctx.talk = Some(
                "Hi! I'm the Neuropil engine — a planner written in Rust that runs inside this server. \
                 No API key, no cloud AI. Tell me what to build, e.g. “build a house, then make the roof red”. \
                 Say “help” to see everything I know."
                    .to_string(),
            );
        }
        Intent::Thanks => {
            ctx.talk = Some("Anytime. Want to change something, or build the next thing?".to_string());
        }
        Intent::Help => ctx.talk = Some(help_text()),
        Intent::Undo => {
            ctx.action = Some("undo".to_string());
            ctx.intents.push("undo".to_string());
            ctx.notes.push("Rolled back your last change.".to_string());
        }
        Intent::Clear => do_clear(ctx),
        Intent::Build => do_build(ctx, t),
        Intent::Paint => do_paint(ctx, t),
        Intent::Scale => do_scale(ctx, t),
        Intent::Move => do_move(ctx, t),
        Intent::Delete => do_delete(ctx, t),
        Intent::Mirror => do_mirror(ctx, t),
        Intent::Duplicate => do_duplicate(ctx, t),
        Intent::Taper => do_taper(ctx, t),
        Intent::Extrude => do_extrude(ctx, t),
        Intent::Unknown => ctx.unknown.push(t.join(" ")),
    }
}

fn help_text() -> String {
    format!(
        "I build from sharp 8-corner cubes, fully inside this Rust server.\n\n\
         **Objects I know:** {}.\n\n\
         **Things you can say:**\n\
         • “build a tower with 8 floors” · “add 3 trees” · “a huge red castle”\n\
         • “make the roof blue” · “paint the house gold” · “make it neon”\n\
         • “make it taller” · “scale it 2x” · “move the roof up 1”\n\
         • “mirror it” · “duplicate it 3 times” · “delete the chimney”\n\
         • “extrude the top 3 times and taper into a sharp obelisk”\n\
         • “undo” · “clear”\n\n\
         I can chain steps in one message: “build a house, make the roof red and add 2 trees”.",
        builders::KINDS.join(", ")
    )
}

// ───────────────────────────── targeting ─────────────────────────────

fn part_matches(c: &Cube, t: &[String], skip: &[String]) -> bool {
    let words: Vec<&str> = c.part.split('_').filter(|w| w.len() >= 3).collect();
    for tok in t.iter() {
        if DIRECTIONS.contains(&tok.as_str()) || skip.contains(tok) {
            continue;
        }
        let s = nlu::singular(tok);
        if skip.contains(&s) {
            continue;
        }
        if words.iter().any(|w| *w == s.as_str()) {
            return true;
        }
    }
    false
}

/// Which cubes does this clause talk about? Kinds ("the house") narrow to a build,
/// parts ("the roof") narrow to matching cubes, otherwise selection, then the latest build.
fn resolve_targets(ctx: &Ctx, t: &[String], last_cube_only: bool) -> Vec<usize> {
    let ws = &ctx.ws;
    if ws.is_empty() {
        return Vec::new();
    }
    let all_words = has_any(t, &["all", "everything", "whole", "scene", "every"]);
    let plural_ref = has_any(t, &["them", "these", "those", "both"]);

    let mut kinds: Vec<String> = Vec::new();
    let mut kind_words: Vec<String> = Vec::new();
    let mut plural_kind = false;
    for w in t.iter() {
        if let Some(k) = nlu::kind_of_word(w) {
            let exists = ws.iter().any(|c| group_kind(&c.group) == k);
            if exists && !kinds.iter().any(|x| x == k) {
                kinds.push(k.to_string());
                kind_words.push(w.clone());
                if nlu::singular(w) != *w {
                    plural_kind = true;
                }
            }
        }
    }

    let mut pool: Vec<usize> = Vec::new();
    if kinds.is_empty() {
        pool = (0..ws.len()).collect();
    } else {
        for k in kinds.iter() {
            let mut groups: Vec<String> = Vec::new();
            for c in ws.iter() {
                if group_kind(&c.group) == *k && !groups.contains(&c.group) {
                    groups.push(c.group.clone());
                }
            }
            let chosen: Vec<String> = if all_words || plural_ref || plural_kind {
                groups
            } else {
                groups.last().cloned().into_iter().collect()
            };
            for (i, c) in ws.iter().enumerate() {
                if chosen.contains(&c.group) && group_kind(&c.group) == *k {
                    pool.push(i);
                }
            }
        }
    }

    let mut hits: Vec<usize> = Vec::new();
    for &i in pool.iter() {
        if part_matches(&ws[i], t, &kind_words) {
            hits.push(i);
        }
    }
    if !hits.is_empty() {
        return hits;
    }
    if !kinds.is_empty() {
        return pool;
    }
    if all_words {
        return (0..ws.len()).collect();
    }
    if let Some(sel) = &ctx.selected {
        if let Some(i) = ws.iter().position(|c| c.name == *sel) {
            if last_cube_only || has_any(t, &["this", "selected", "selection", "it", "that"]) {
                return vec![i];
            }
        }
    }
    if last_cube_only {
        return vec![ws.len() - 1];
    }
    let last = &ws[ws.len() - 1];
    if plural_ref {
        let k = group_kind(&last.group);
        return ws.iter().enumerate().filter(|(_, c)| group_kind(&c.group) == k).map(|(i, _)| i).collect();
    }
    if last.group.is_empty() {
        return vec![ws.len() - 1];
    }
    ws.iter().enumerate().filter(|(_, c)| c.group == last.group).map(|(i, _)| i).collect()
}

// ───────────────────────────── building ─────────────────────────────

fn max_x(ws: &[Cube]) -> Option<f32> {
    geom::bounds(ws).map(|b| b.1[0])
}

/// Put a freshly built object into the workspace: names, group, position in the row.
fn place(ctx: &mut Ctx, kind: &str, mut cubes: Vec<Cube>, cursor: &mut Option<f32>) -> Option<String> {
    let (lo, hi) = geom::bounds(&cubes)?;
    let width = hi[0] - lo[0];
    let target_min = match *cursor {
        Some(c) => c,
        None => match max_x(&ctx.ws) {
            Some(mx) => mx + 1.5,
            None => -width / 2.0,
        },
    };
    let shift = target_min - lo[0];
    *cursor = Some(target_min + width + 1.5);
    let group = next_group(&ctx.ws, kind);
    for c in cubes.iter_mut() {
        c.pos[0] += shift;
        c.group = group.clone();
        c.role = format!("{} {}", kind, c.part);
        c.name = unique_name(&ctx.ws, &format!("{}_{}", group, c.part));
        ctx.ws.push(c.clone());
    }
    ctx.ground.push(group.clone());
    Some(group)
}

fn describe_build(kind: &str, count: Option<u32>, instances: u32) -> String {
    if instances > 1 {
        return format!("Built {} {}s", instances, kind);
    }
    match (kind, count) {
        ("tower", Some(n)) => format!("Built a {}-floor tower", n),
        ("pyramid", Some(n)) => format!("Built a {}-layer pyramid", n),
        ("stairs", Some(n)) => format!("Built a {}-step staircase", n),
        ("bridge", Some(n)) => format!("Built a {}-span bridge", n),
        ("house", Some(n)) => format!("Built a {}-storey house", n),
        ("wall", Some(n)) => format!("Built a {}-block wall", n),
        _ => format!("Built a {}", kind),
    }
}

fn do_build(ctx: &mut Ctx, t: &[String]) {
    let hit = match nlu::find_object(t) {
        Some(h) => h,
        None => {
            do_fallback(ctx, t);
            return;
        }
    };
    let kind = hit.kind;
    let color = t.iter().find_map(|w| nlu::color_of(w));
    let robot = has_any(t, &["robot", "android", "cyborg"]);
    let size_kind = matches!(kind, "tower" | "pyramid" | "stairs" | "bridge" | "wall")
        || (kind == "house" && has_any(t, &["floor", "floors", "storey", "storeys", "story", "stories", "level", "levels"]));
    let number = nlu::first_number(t).map(|n| n.round().max(1.0).min(30.0) as u32);
    let (mut count, instances) = match number {
        Some(n) if size_kind && !hit.plural => (Some(n), 1u32),
        Some(n) => (None, n.min(12)),
        None => (None, 1u32),
    };
    if count.is_none() && kind == "tower" && has_any(t, &["tall", "high", "skyscraper"]) {
        count = Some(8);
    }
    let mut p = Params {
        scale: nlu::scale_word(t),
        count,
        color: color.clone(),
        mat: nlu::material_of(t).map(|m| m.to_string()),
        seed: ctx.seed,
    };
    if robot && p.color.is_none() {
        p.color = Some("#94A3B8".to_string());
    }
    if robot && p.mat.is_none() {
        p.mat = Some("metal".to_string());
    }

    if is_pristine_origin(&ctx.ws) {
        ctx.ws.clear();
    }
    let mut cursor: Option<f32> = None;
    let mut total = 0usize;
    let mut last_group = String::new();
    for k in 0..instances {
        if ctx.ws.len() >= MAX_CUBES {
            ctx.fail("build", "the scene is full (600 cubes max). Delete something or clear first.");
            break;
        }
        p.seed = ctx.seed.wrapping_add(k as u64);
        let mut cubes = builders::build(kind, &p);
        if robot {
            for c in cubes.iter_mut() {
                if c.part.starts_with("eye") {
                    c.color = "#22D3EE".to_string();
                    c.mat = "neon".to_string();
                } else if c.part.starts_with("head") || c.part.starts_with("arm") {
                    c.color = "#94A3B8".to_string();
                }
            }
        }
        total += cubes.len();
        if let Some(g) = place(ctx, kind, cubes, &mut cursor) {
            last_group = g;
        }
    }
    if total == 0 {
        return;
    }
    ctx.intents.push("build".to_string());
    ctx.step(true, "build", format!("{} × {} → {} cubes", instances, kind, total));
    let mut note = describe_build(kind, count, instances);
    if let Some(c) = &color {
        note.push_str(&format!(" in {}", c));
    }
    note.push_str(&format!(" ({} cubes", total));
    note.push_str(if last_group.is_empty() { ")." } else { ", sharp 8-corner blocks)." });
    if hit.fuzzy {
        note.push_str(&format!(" I read “{}” as “{}”.", hit.word, kind));
    }
    ctx.notes.push(note);
}

fn do_fallback(ctx: &mut Ctx, t: &[String]) {
    let skip = ["a", "an", "the", "me", "please", "some", "of", "with"];
    let words: Vec<String> = t
        .iter()
        .filter(|w| !nlu::VERBS.contains(&w.as_str()) && !skip.contains(&w.as_str()))
        .cloned()
        .collect();
    let subject = words.join(" ");
    let color = t.iter().find_map(|w| nlu::color_of(w));
    if is_pristine_origin(&ctx.ws) {
        ctx.ws.clear();
    }
    let cubes = builders::sculpture(ctx.seed, color);
    let n = cubes.len();
    let mut cursor: Option<f32> = None;
    place(ctx, "sculpture", cubes, &mut cursor);
    ctx.intents.push("build".to_string());
    ctx.step(true, "sculpt", format!("abstract tower seeded from “{}”", subject));
    ctx.notes.push(format!(
        "I don't know how to build “{}” yet, so I sculpted an abstract {}-block tower from your words instead. \
         What I can build properly: {}.",
        subject,
        n,
        builders::KINDS.join(", ")
    ));
}

// ───────────────────────────── edits ─────────────────────────────

fn do_clear(ctx: &mut Ctx) {
    ctx.ws = vec![origin_cube()];
    ctx.intents.push("clear".to_string());
    ctx.step(true, "clear", "workspace reset to the 1×1×1 origin cube".to_string());
    ctx.notes.push("Cleared the workspace back to a single origin cube.".to_string());
}

fn paint_idx(ctx: &mut Ctx, idx: &[usize], color: Option<&str>, mat: Option<&str>) -> usize {
    for &i in idx.iter() {
        if let Some(c) = color {
            ctx.ws[i].color = c.to_string();
        }
        if let Some(m) = mat {
            ctx.ws[i].mat = m.to_string();
        }
    }
    idx.len()
}

fn do_paint(ctx: &mut Ctx, t: &[String]) {
    let mat = nlu::material_of(t);
    let color_count = t.iter().filter(|w| nlu::color_of(w).is_some()).count();
    if color_count == 0 && mat.is_none() {
        ctx.fail("paint", "I need a colour (red, blue, #ff8800…) or a material (neon, metal, matte).");
        return;
    }
    let mut painted = 0usize;
    if color_count <= 1 {
        let color = t.iter().find_map(|w| nlu::color_of(w));
        let idx = resolve_targets(ctx, t, false);
        painted += paint_idx(ctx, &idx, color.as_deref(), mat);
    } else {
        let mut seg: Vec<String> = Vec::new();
        for w in t.iter() {
            seg.push(w.clone());
            if let Some(col) = nlu::color_of(w) {
                let idx = resolve_targets(ctx, &seg, false);
                painted += paint_idx(ctx, &idx, Some(col.as_str()), mat);
                seg.clear();
            }
        }
    }
    if painted == 0 {
        ctx.fail("paint", "there is nothing in the workspace to paint yet.");
        return;
    }
    ctx.intents.push("paint".to_string());
    ctx.step(true, "paint", format!("{} cubes restyled", painted));
    ctx.notes.push(format!("Restyled {} cubes.", painted));
}

fn do_scale(ctx: &mut Ctx, t: &[String]) {
    let shrink = has_any(t, &["smaller", "shrink", "reduce", "decrease", "thinner", "narrower", "shorter", "slimmer"]);
    let mag = if has_any(t, &["half"]) {
        0.5
    } else if let Some(n) = nlu::first_number(t) {
        if shrink {
            1.0 / n.max(0.1)
        } else {
            n
        }
    } else if shrink {
        0.8
    } else {
        1.25
    };
    let mag = mag.max(0.1).min(6.0);
    let f = if has_any(t, &["taller", "shorter", "higher"]) {
        [1.0, mag, 1.0]
    } else if has_any(t, &["wider", "thinner", "narrower", "slimmer", "fatter", "thicker"]) {
        [mag, 1.0, mag]
    } else {
        [mag, mag, mag]
    };
    let idx = resolve_targets(ctx, t, false);
    if idx.is_empty() {
        ctx.fail("scale", "there is nothing to scale yet.");
        return;
    }
    let n = idx.len() as f32;
    let px = idx.iter().map(|&i| ctx.ws[i].pos[0]).sum::<f32>() / n;
    let pz = idx.iter().map(|&i| ctx.ws[i].pos[2]).sum::<f32>() / n;
    for &i in idx.iter() {
        let c = &mut ctx.ws[i];
        c.pos[0] = px + (c.pos[0] - px) * f[0];
        c.pos[1] *= f[1];
        c.pos[2] = pz + (c.pos[2] - pz) * f[2];
        for k in 0..3 {
            c.size[k] *= f[k];
        }
        if !c.group.is_empty() {
            let g = c.group.clone();
            if !ctx.ground.contains(&g) {
                ctx.ground.push(g);
            }
        }
    }
    ctx.intents.push("scale".to_string());
    ctx.step(true, "scale", format!("{} cubes × [{:.2}, {:.2}, {:.2}]", idx.len(), f[0], f[1], f[2]));
    ctx.notes.push(format!("Scaled {} cubes by [{:.2}, {:.2}, {:.2}].", idx.len(), f[0], f[1], f[2]));
}

fn do_move(ctx: &mut Ctx, t: &[String]) {
    let amount = nlu::number_after(t, &["up", "down", "left", "right", "forward", "backward", "back", "by"])
        .or_else(|| nlu::first_number(t))
        .unwrap_or(1.0);
    let mut d = [0.0f32; 3];
    if has_any(t, &["up", "raise", "lift", "higher"]) {
        d[1] += amount;
    }
    if has_any(t, &["down", "lower"]) {
        d[1] -= amount;
    }
    if has_any(t, &["left"]) {
        d[0] -= amount;
    }
    if has_any(t, &["right"]) {
        d[0] += amount;
    }
    if has_any(t, &["forward", "front"]) {
        d[2] += amount;
    }
    if has_any(t, &["backward", "back"]) {
        d[2] -= amount;
    }
    if d == [0.0, 0.0, 0.0] {
        ctx.fail("move", "tell me a direction: up, down, left, right, forward or back.");
        return;
    }
    let idx = resolve_targets(ctx, t, false);
    if idx.is_empty() {
        ctx.fail("move", "there is nothing to move yet.");
        return;
    }
    for &i in idx.iter() {
        for k in 0..3 {
            ctx.ws[i].pos[k] += d[k];
        }
    }
    ctx.intents.push("move".to_string());
    ctx.step(true, "move", format!("{} cubes by [{:.1}, {:.1}, {:.1}]", idx.len(), d[0], d[1], d[2]));
    ctx.notes.push(format!("Moved {} cubes.", idx.len()));
}

fn do_delete(ctx: &mut Ctx, t: &[String]) {
    if has_any(t, &["all", "everything", "scene"]) {
        do_clear(ctx);
        return;
    }
    let mut idx = resolve_targets(ctx, t, false);
    if idx.is_empty() {
        ctx.fail("delete", "there is nothing to delete.");
        return;
    }
    idx.sort();
    idx.dedup();
    for &i in idx.iter().rev() {
        ctx.ws.remove(i);
    }
    ctx.intents.push("delete".to_string());
    ctx.step(true, "delete", format!("{} cubes removed", idx.len()));
    ctx.notes.push(format!("Removed {} cubes.", idx.len()));
}

/// Copy cubes with fresh names and a fresh group; `shift_x` moves the copy, `plane` mirrors it.
fn add_copy(ctx: &mut Ctx, members: &[Cube], shift_x: f32, plane: Option<f32>) -> usize {
    let first = &members[0];
    let kind = group_kind(&first.group);
    let group = if first.group.is_empty() { String::new() } else { next_group(&ctx.ws, &kind) };
    let mut added = 0;
    for m in members.iter() {
        let mut c = match plane {
            Some(px) => geom::mirror_x(m, px),
            None => m.clone(),
        };
        c.pos[0] += shift_x;
        let base = if group.is_empty() { format!("{}_copy", m.name) } else { format!("{}_{}", group, m.part) };
        c.name = unique_name(&ctx.ws, &base);
        c.group = group.clone();
        ctx.ws.push(c);
        added += 1;
    }
    added
}

fn do_mirror(ctx: &mut Ctx, t: &[String]) {
    let idx = resolve_targets(ctx, t, false);
    if idx.is_empty() {
        ctx.fail("mirror", "there is nothing to mirror yet.");
        return;
    }
    let members: Vec<Cube> = idx.iter().map(|&i| ctx.ws[i].clone()).collect();
    let plane = match geom::bounds(&members) {
        Some((_, hi)) => hi[0] + 0.5,
        None => return,
    };
    let added = add_copy(ctx, &members, 0.0, Some(plane));
    ctx.intents.push("mirror".to_string());
    ctx.step(true, "mirror", format!("{} cubes reflected across x = {:.2}", added, plane));
    ctx.notes.push(format!("Mirrored it — {} cubes now sit on the other side.", added));
}

fn do_duplicate(ctx: &mut Ctx, t: &[String]) {
    let idx = resolve_targets(ctx, t, false);
    if idx.is_empty() {
        ctx.fail("duplicate", "there is nothing to duplicate yet.");
        return;
    }
    let copies = nlu::number_before(t, &["times", "time", "copies", "copy", "x"])
        .or_else(|| nlu::first_number(t))
        .unwrap_or(1.0)
        .round()
        .max(1.0)
        .min(12.0) as usize;
    let members: Vec<Cube> = idx.iter().map(|&i| ctx.ws[i].clone()).collect();
    let step = match geom::bounds(&members) {
        Some((lo, hi)) => hi[0] - lo[0] + 1.0,
        None => return,
    };
    let mut added = 0;
    for k in 1..=copies {
        if ctx.ws.len() + members.len() > MAX_CUBES {
            ctx.fail("duplicate", "the scene is full (600 cubes max).");
            break;
        }
        added += add_copy(ctx, &members, step * k as f32, None);
    }
    ctx.intents.push("duplicate".to_string());
    ctx.step(true, "duplicate", format!("{} copies, {} cubes", copies, added));
    ctx.notes.push(format!("Duplicated it {} time(s) in a row ({} cubes added).", copies, added));
}

fn do_taper(ctx: &mut Ctx, t: &[String]) {
    let mode = if has_any(t, &["wedge", "slant", "slope"]) {
        "wedge-top"
    } else if has_any(t, &["razor", "blade", "edge"]) {
        "front-edge"
    } else if has_any(t, &["ridge", "gable"]) {
        "ridge"
    } else if has_any(t, &["bottom", "base"]) {
        "bot-in"
    } else {
        "top-in"
    };
    let factor = match nlu::first_number(t) {
        Some(n) if n > 0.0 && n < 1.0 => n,
        _ => {
            if has_any(t, &["sharp", "pointy", "needle", "point", "spike", "spiky"]) {
                0.1
            } else {
                0.5
            }
        }
    };
    let idx = resolve_targets(ctx, t, true);
    if idx.is_empty() {
        ctx.fail("taper", "there is nothing to chisel yet.");
        return;
    }
    for &i in idx.iter() {
        let res: Result<(), String> = geom::taper(&mut ctx.ws[i], mode, factor);
        if let Err(e) = res {
            ctx.fail("taper", e.as_str());
            return;
        }
    }
    ctx.intents.push("taper".to_string());
    ctx.step(true, "taper", format!("{} cubes · {} · factor {:.2}", idx.len(), mode, factor));
    ctx.notes.push(format!("Chiselled {} cube(s) with a {} taper.", idx.len(), mode));
}

fn do_extrude(ctx: &mut Ctx, t: &[String]) {
    let faces = ["top", "bottom", "left", "right", "front", "back"];
    let face = t.iter().find(|w| faces.contains(&w.as_str())).cloned().unwrap_or_else(|| "top".to_string());
    let count = nlu::number_before(t, &["times", "time", "x", "steps"]).unwrap_or(1.0).round().max(1.0).min(12.0) as usize;
    let dist = nlu::number_after(t, &["by", "distance"]).unwrap_or(1.0);
    let color = t.iter().find_map(|w| nlu::color_of(w));
    let idx = resolve_targets(ctx, t, true);
    if idx.is_empty() {
        ctx.fail("extrude", "there is no cube to extrude from.");
        return;
    }
    let mut src = ctx.ws[idx[0]].clone();
    let mut made = 0;
    for _ in 0..count {
        if ctx.ws.len() >= MAX_CUBES {
            ctx.fail("extrude", "the scene is full (600 cubes max).");
            break;
        }
        let res: Result<Cube, String> = geom::extrude(&src, face.as_str(), dist, 0.85, color.as_deref());
        match res {
            Ok(mut c) => {
                c.name = unique_name(&ctx.ws, &c.name);
                ctx.ws.push(c.clone());
                src = c;
                made += 1;
            }
            Err(e) => {
                ctx.fail("extrude", e.as_str());
                break;
            }
        }
    }
    if made > 0 {
        ctx.intents.push("extrude".to_string());
        ctx.step(true, "extrude", format!("{} × {} face", made, face));
        ctx.notes.push(format!("Extruded the {} face {} time(s).", face, made));
    }
}

// ───────────────────────────── self-check & reply ─────────────────────────────

/// Critic pass: clamp broken numbers and snap fresh builds to the ground.
fn self_check(ctx: &mut Ctx) {
    let mut fixes = 0;
    for c in ctx.ws.iter_mut() {
        for k in 0..3 {
            if !c.size[k].is_finite() || c.size[k] < 0.05 {
                c.size[k] = 0.05;
                fixes += 1;
            }
            if !c.pos[k].is_finite() {
                c.pos[k] = 0.0;
                fixes += 1;
            }
            if c.pos[k].abs() > 120.0 {
                c.pos[k] = c.pos[k].signum() * 120.0;
                fixes += 1;
            }
        }
    }
    let groups = ctx.ground.clone();
    let mut snapped = 0;
    for g in groups.iter() {
        let members: Vec<Cube> = ctx.ws.iter().filter(|c| c.group == *g).cloned().collect();
        if let Some((lo, _)) = geom::bounds(&members) {
            if lo[1].abs() > 0.001 {
                for c in ctx.ws.iter_mut() {
                    if c.group == *g {
                        c.pos[1] -= lo[1];
                    }
                }
                snapped += 1;
            }
        }
    }
    ctx.step(true, "self-check", format!("{} value fixes · {} build(s) re-grounded · {} cubes", fixes, snapped, ctx.ws.len()));
}

fn suggest(ws: &[Cube], seed: u64) -> Vec<String> {
    let kind = ws.last().map(|c| group_kind(&c.group)).unwrap_or_default();
    let pool: &[&str] = match kind.as_str() {
        "house" => &["Make the roof blue", "Add 3 trees", "Duplicate it 2 times", "Make it neon"],
        "tower" => &["Make it taller", "Make it neon", "Mirror it", "Build a bridge"],
        "castle" => &["Make the towers gold", "Add a tree", "Make it bigger", "Mirror it"],
        "character" => &["Make the torso red", "Duplicate it 3 times", "Make it metal", "Build a sword"],
        "tree" => &["Add 5 trees", "Make it bigger", "Build a house", "Make the leaves gold"],
        _ => &["Build a castle", "Make it taller", "Mirror it", "Build a robot"],
    };
    let start = (seed % pool.len() as u64) as usize;
    (0..3).map(|i| pool[(start + i) % pool.len()].to_string()).collect()
}

fn scene_line(ws: &[Cube]) -> String {
    match geom::bounds(ws) {
        Some((lo, hi)) => format!(
            "Scene: {} cubes · {:.1} × {:.1} × {:.1} units.",
            ws.len(),
            hi[0] - lo[0],
            hi[1] - lo[1],
            hi[2] - lo[2]
        ),
        None => "Scene is empty.".to_string(),
    }
}

pub fn think(req: AiRequest) -> AiResponse {
    let prompt = req.prompt.trim().to_string();
    let before: Vec<String> = req.workspace.iter().map(|c| c.name.clone()).collect();
    let mut ctx = Ctx {
        ws: req.workspace,
        selected: req.selected,
        seed: nlu::hash(&prompt),
        log: Vec::new(),
        notes: Vec::new(),
        intents: Vec::new(),
        unknown: Vec::new(),
        talk: None,
        action: None,
        ground: Vec::new(),
    };

    if prompt.is_empty() {
        let suggestions = suggest(&ctx.ws, 0);
        return AiResponse {
            reply: "Type what you want to build — for example “build a tower with 6 floors”.".to_string(),
            intent: "chat".to_string(),
            confidence: 1.0,
            steps: Vec::new(),
            workspace: ctx.ws,
            suggestions,
            action: None,
            changed: Vec::new(),
        };
    }

    let clauses = nlu::clauses(&prompt);
    for toks in clauses.iter() {
        run_clause(&mut ctx, toks);
    }
    let edited = !ctx.intents.is_empty() && ctx.action.is_none();
    if edited {
        self_check(&mut ctx);
    }

    let mut reply = match &ctx.talk {
        Some(t) => t.clone(),
        None => ctx.notes.join(" "),
    };
    if ctx.talk.is_none() {
        if !ctx.unknown.is_empty() && !ctx.notes.is_empty() {
            reply.push_str(&format!(" I skipped “{}” — I didn't understand it.", ctx.unknown.join("”, “")));
        } else if ctx.notes.is_empty() {
            reply = format!(
                "I didn't understand “{}”. Try “build a house”, “make it red” or “mirror it” — say “help” for the full list.",
                prompt
            );
        }
        if edited {
            reply.push_str(&format!("\n{}", scene_line(&ctx.ws)));
        }
    }

    let intent = if ctx.talk.is_some() && ctx.intents.is_empty() {
        "chat".to_string()
    } else if ctx.intents.is_empty() {
        "unknown".to_string()
    } else {
        ctx.intents.join("+")
    };
    let confidence = if clauses.is_empty() {
        0.0
    } else {
        ((ctx.intents.len() + if ctx.talk.is_some() { 1 } else { 0 }) as f32 / clauses.len() as f32).min(1.0)
    };
    let changed: Vec<String> = ctx.ws.iter().filter(|c| !before.contains(&c.name)).map(|c| c.name.clone()).collect();
    let suggestions = suggest(&ctx.ws, ctx.seed);

    AiResponse {
        reply,
        intent,
        confidence,
        steps: ctx.log,
        workspace: ctx.ws,
        suggestions,
        action: ctx.action,
        changed,
    }
}

/// Single structured edit from the inspector buttons (push / extrude / taper / mirror).
pub fn apply_op(req: OpRequest) -> Result<OpResponse, String> {
    let mut ws = req.workspace;
    let name = req.target.clone().ok_or_else(|| "No cube is selected.".to_string())?;
    let idx = ws
        .iter()
        .position(|c| c.name == name)
        .ok_or_else(|| format!("Cube '{}' was not found in the workspace.", name))?;
    let mut changed: Vec<String> = Vec::new();
    let message: String = match req.op.as_str() {
        "push" => {
            let axis = req.axis.clone().unwrap_or_else(|| "y+".to_string());
            let amount = req.amount.unwrap_or(0.25);
            geom::push_face(&mut ws[idx], &axis, amount)?;
            format!("Pushed {} of {} by {:.2}.", axis, name, amount)
        }
        "extrude" => {
            let face = req.face.clone().unwrap_or_else(|| "top".to_string());
            let src = ws[idx].clone();
            let mut c = geom::extrude(&src, &face, req.dist.unwrap_or(0.8), req.inset.unwrap_or(0.9), None)?;
            c.name = unique_name(&ws, &c.name);
            changed.push(c.name.clone());
            ws.push(c);
            format!("Extruded the {} face of {}.", face, name)
        }
        "taper" => {
            let mode = req.mode.clone().unwrap_or_else(|| "top-in".to_string());
            geom::taper(&mut ws[idx], &mode, req.factor.unwrap_or(0.45))?;
            format!("Applied a {} taper to {}.", mode, name)
        }
        "mirror" => {
            let src = ws[idx].clone();
            let plane = match geom::bounds(&[src.clone()]) {
                Some((_, hi)) => hi[0] + 0.5,
                None => 0.0,
            };
            let mut c = geom::mirror_x(&src, plane);
            c.name = unique_name(&ws, &c.name);
            changed.push(c.name.clone());
            ws.push(c);
            format!("Mirrored {} to the right.", name)
        }
        other => return Err(format!("Unknown operation '{}'. Valid: push, extrude, taper, mirror.", other)),
    };
    Ok(OpResponse { workspace: ws, changed, message })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(prompt: &str, ws: Vec<Cube>) -> AiResponse {
        think(AiRequest { prompt: prompt.to_string(), workspace: ws, selected: None })
    }

    #[test]
    fn building_replaces_the_starter_cube() {
        let r = run("build a house", vec![origin_cube()]);
        assert!(r.workspace.len() >= 6);
        assert!(r.workspace.iter().any(|c| c.part == "roof"));
        assert!(!r.workspace.iter().any(|c| c.name == "Origin_Cube"));
    }

    #[test]
    fn paints_a_part_by_name() {
        let r1 = run("build a house", vec![]);
        let r2 = run("make the roof blue", r1.workspace);
        let roof = r2.workspace.iter().find(|c| c.part == "roof").unwrap();
        assert_eq!(roof.color, "#3B82F6");
        let door = r2.workspace.iter().find(|c| c.part == "door").unwrap();
        assert_ne!(door.color, "#3B82F6");
    }

    #[test]
    fn chains_commands_in_one_message() {
        let r = run("build a house, make the roof red and add 2 trees", vec![]);
        let trees = r.workspace.iter().filter(|c| c.part == "trunk").count();
        assert_eq!(trees, 2);
        assert!(r.intent.contains("build") && r.intent.contains("paint"));
    }

    #[test]
    fn tower_floors_and_grounding() {
        let r = run("build a tower with 6 floors", vec![]);
        let (lo, hi) = geom::bounds(&r.workspace).unwrap();
        assert!(lo[1].abs() < 0.01);
        assert!(hi[1] > 9.0);
    }

    #[test]
    fn extrude_then_taper_from_the_origin_cube() {
        let r = run("extrude top face 3 times and taper into a sharp obelisk", vec![origin_cube()]);
        assert_eq!(r.workspace.len(), 4);
        assert!(r.workspace.last().unwrap().corners[2][0] < 0.2);
    }

    #[test]
    fn unknown_object_gets_an_honest_fallback() {
        let r = run("build a dragon", vec![]);
        assert!(r.reply.contains("don't know"));
        assert!(!r.workspace.is_empty());
    }

    #[test]
    fn same_prompt_same_result() {
        let a = run("build a castle", vec![]);
        let b = run("build a castle", vec![]);
        assert_eq!(a.workspace.len(), b.workspace.len());
        assert_eq!(a.reply, b.reply);
    }

    #[test]
    fn mirror_adds_a_reflected_copy() {
        let r1 = run("build a sword", vec![]);
        let n = r1.workspace.len();
        let r2 = run("mirror it", r1.workspace);
        assert_eq!(r2.workspace.len(), n * 2);
    }
}
