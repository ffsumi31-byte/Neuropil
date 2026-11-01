//! nlu.rs — tiny natural-language understanding, written from scratch (no external AI).
//! Tokens, numbers, colours, materials, fuzzy object matching and clause splitting.

/// FNV-1a hash — gives every prompt a stable seed so the engine is deterministic.
pub fn hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn tokenize(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.to_lowercase().chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        let decimal_dot = ch == '.'
            && !cur.is_empty()
            && cur.chars().all(|c| c.is_ascii_digit())
            && i + 1 < chars.len()
            && chars[i + 1].is_ascii_digit();
        if ch.is_alphanumeric() || ch == '#' || decimal_dot {
            cur.push(ch);
        } else if !cur.is_empty() {
            out.push(cur.clone());
            cur.clear();
        }
        i += 1;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Words that start a new command. "and" only splits a sentence when a verb follows it.
pub const VERBS: &[&str] = &[
    "build", "make", "create", "generate", "add", "spawn", "construct", "design", "place", "put", "give",
    "sculpt", "craft", "model", "paint", "color", "colour", "recolor", "tint", "delete", "remove", "erase",
    "extrude", "taper", "mirror", "duplicate", "copy", "clone", "move", "shift", "raise", "lower", "scale",
    "resize", "clear", "reset", "undo", "grow", "shrink", "enlarge", "sharpen", "turn",
];

/// Split a prompt into clauses (token lists): by punctuation, "then", and "and <verb>".
pub fn clauses(text: &str) -> Vec<Vec<String>> {
    let chars: Vec<char> = text.chars().collect();
    let mut parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    for (i, &ch) in chars.iter().enumerate() {
        let sentence_dot = ch == '.' && (i + 1 >= chars.len() || chars[i + 1].is_whitespace());
        if ch == ',' || ch == ';' || ch == '!' || ch == '?' || ch == '\n' || sentence_dot {
            parts.push(cur.clone());
            cur.clear();
        } else {
            cur.push(ch);
        }
    }
    parts.push(cur);

    let mut out: Vec<Vec<String>> = Vec::new();
    for part in parts.iter() {
        let toks = tokenize(part);
        let mut clause: Vec<String> = Vec::new();
        for (i, tok) in toks.iter().enumerate() {
            let next_is_verb = i + 1 < toks.len() && VERBS.contains(&toks[i + 1].as_str());
            let splitter = tok == "then" || ((tok == "and" || tok == "also") && next_is_verb);
            if splitter {
                push_clause(&mut out, &mut clause);
            } else {
                clause.push(tok.clone());
            }
        }
        push_clause(&mut out, &mut clause);
    }
    out
}

fn push_clause(out: &mut Vec<Vec<String>>, clause: &mut Vec<String>) {
    while !clause.is_empty() && (clause[0] == "and" || clause[0] == "now" || clause[0] == "please") {
        clause.remove(0);
    }
    while !clause.is_empty() && (clause[clause.len() - 1] == "and" || clause[clause.len() - 1] == "please") {
        clause.pop();
    }
    if !clause.is_empty() {
        out.push(clause.clone());
    }
    clause.clear();
}

pub fn has_any(toks: &[String], words: &[&str]) -> bool {
    toks.iter().any(|t| words.contains(&t.as_str()))
}

pub fn singular(tok: &str) -> String {
    if tok.len() > 4 && tok.ends_with("ies") {
        return format!("{}y", &tok[..tok.len() - 3]);
    }
    if tok.len() > 4 && (tok.ends_with("ses") || tok.ends_with("xes") || tok.ends_with("ches") || tok.ends_with("shes")) {
        return tok[..tok.len() - 2].to_string();
    }
    if tok.len() > 3 && tok.ends_with('s') && !tok.ends_with("ss") {
        return tok[..tok.len() - 1].to_string();
    }
    tok.to_string()
}

pub fn number_of(tok: &str) -> Option<f32> {
    let t = tok.trim_start_matches('x').trim_end_matches('x');
    if let Some(first) = t.chars().next() {
        if first.is_ascii_digit() {
            if let Ok(n) = t.parse::<f32>() {
                return Some(n);
            }
        }
    }
    match tok {
        "one" => Some(1.0),
        "two" | "twice" | "double" | "couple" => Some(2.0),
        "three" | "triple" | "few" => Some(3.0),
        "four" | "several" => Some(4.0),
        "five" => Some(5.0),
        "six" => Some(6.0),
        "seven" => Some(7.0),
        "eight" => Some(8.0),
        "nine" => Some(9.0),
        "ten" => Some(10.0),
        "twelve" | "dozen" => Some(12.0),
        _ => None,
    }
}

pub fn first_number(toks: &[String]) -> Option<f32> {
    toks.iter().find_map(|t| number_of(t))
}

/// A number written right before one of `words` ("3 times", "five floors").
pub fn number_before(toks: &[String], words: &[&str]) -> Option<f32> {
    for j in 1..toks.len() {
        if words.contains(&toks[j].as_str()) {
            if let Some(n) = number_of(&toks[j - 1]) {
                return Some(n);
            }
        }
    }
    None
}

/// A number written right after one of `words` ("by 2", "up 3").
pub fn number_after(toks: &[String], words: &[&str]) -> Option<f32> {
    for j in 0..toks.len().saturating_sub(1) {
        if words.contains(&toks[j].as_str()) {
            if let Some(n) = number_of(&toks[j + 1]) {
                return Some(n);
            }
        }
    }
    None
}

const COLORS: &[(&str, &str)] = &[
    ("red", "#E5484D"), ("crimson", "#B91C1C"), ("orange", "#F97316"), ("yellow", "#FACC15"),
    ("gold", "#D4AF37"), ("green", "#5EB58B"), ("lime", "#84CC16"), ("teal", "#12879B"),
    ("cyan", "#22D3EE"), ("blue", "#3B82F6"), ("navy", "#1E3A8A"), ("purple", "#8B5CF6"),
    ("violet", "#7C3AED"), ("pink", "#EC4899"), ("magenta", "#D946EF"), ("white", "#F1F5F9"),
    ("black", "#0B0F19"), ("gray", "#64748B"), ("grey", "#64748B"), ("silver", "#CBD5E1"),
    ("brown", "#8B5A2B"), ("beige", "#E8D8B0"), ("sand", "#E8D8B0"),
];

pub fn color_of(tok: &str) -> Option<String> {
    if tok.starts_with('#') {
        let h = &tok[1..];
        if h.chars().all(|c| c.is_ascii_hexdigit()) {
            if h.len() == 6 {
                return Some(format!("#{}", h.to_uppercase()));
            }
            if h.len() == 3 {
                let mut s = String::from("#");
                for c in h.chars() {
                    s.push(c.to_ascii_uppercase());
                    s.push(c.to_ascii_uppercase());
                }
                return Some(s);
            }
        }
        return None;
    }
    for (name, hex) in COLORS.iter() {
        if *name == tok {
            return Some(hex.to_string());
        }
    }
    None
}

pub fn material_of(toks: &[String]) -> Option<&'static str> {
    if has_any(toks, &["neon", "glowing", "glow", "emissive"]) {
        Some("neon")
    } else if has_any(toks, &["metal", "metallic", "titanium", "steel", "chrome"]) {
        Some("metal")
    } else if has_any(toks, &["matte"]) {
        Some("matte")
    } else {
        None
    }
}

/// Size adjectives for brand-new builds.
pub fn scale_word(toks: &[String]) -> f32 {
    if has_any(toks, &["tiny", "mini", "miniature"]) {
        0.5
    } else if has_any(toks, &["small", "little"]) {
        0.7
    } else if has_any(toks, &["huge", "giant", "massive", "enormous", "colossal"]) {
        2.2
    } else if has_any(toks, &["big", "large"]) {
        1.5
    } else {
        1.0
    }
}

/// Everything the engine can build, with the words people use for it.
pub const OBJECTS: &[(&str, &[&str])] = &[
    ("tower", &["tower", "obelisk", "spire", "pillar", "column", "skyscraper", "monolith", "lighthouse"]),
    ("pyramid", &["pyramid", "ziggurat"]),
    ("house", &["house", "home", "cabin", "hut", "cottage", "shed", "building"]),
    ("castle", &["castle", "fortress", "fort", "citadel"]),
    ("character", &["character", "person", "human", "man", "avatar", "humanoid", "r15", "rig", "player", "npc", "robot", "guy"]),
    ("sword", &["sword", "katana", "blade", "saber", "sabre", "dagger", "knife"]),
    ("chair", &["chair", "stool", "throne", "seat"]),
    ("table", &["table", "desk", "bench"]),
    ("tree", &["tree", "pine", "oak"]),
    ("car", &["car", "vehicle", "truck", "buggy", "automobile"]),
    ("bridge", &["bridge", "walkway"]),
    ("stairs", &["stairs", "staircase", "stair", "steps", "step"]),
    ("rocket", &["rocket", "missile", "spaceship", "shuttle"]),
    ("arch", &["arch", "gate", "gateway", "portal", "doorway"]),
    ("wall", &["wall", "fence", "barrier"]),
    ("cube", &["cube", "block", "box"]),
];

pub struct ObjHit {
    pub kind: &'static str,
    pub word: String,
    pub fuzzy: bool,
    pub plural: bool,
}

/// Exact / plural match of a single word against the object table.
pub fn kind_of_word(tok: &str) -> Option<&'static str> {
    let s = singular(tok);
    for (kind, words) in OBJECTS.iter() {
        if words.contains(&tok) || words.contains(&s.as_str()) {
            return Some(*kind);
        }
    }
    None
}

pub fn lev(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[b.len()]
}

fn is_known_word(tok: &str) -> bool {
    VERBS.contains(&tok) || color_of(tok).is_some() || kind_of_word(tok).is_some()
}

/// Find the first object word in a clause; falls back to typo-tolerant matching.
pub fn find_object(toks: &[String]) -> Option<ObjHit> {
    for t in toks.iter() {
        if let Some(kind) = kind_of_word(t) {
            return Some(ObjHit { kind, word: t.clone(), fuzzy: false, plural: singular(t) != *t });
        }
    }
    for t in toks.iter() {
        if t.len() < 5 || is_known_word(t) || number_of(t).is_some() {
            continue;
        }
        for (kind, words) in OBJECTS.iter() {
            for w in words.iter() {
                let limit = if w.len() >= 8 { 2 } else { 1 };
                if w.len() >= 5 && lev(t, w) <= limit {
                    return Some(ObjHit { kind: *kind, word: t.clone(), fuzzy: true, plural: false });
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_clauses_on_and_verb() {
        let c = clauses("Build a house, make the roof red and paint the door blue");
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn keeps_and_inside_a_noun_phrase() {
        let c = clauses("a red and blue tower");
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn numbers_and_colors() {
        assert_eq!(number_of("3"), Some(3.0));
        assert_eq!(number_of("x4"), Some(4.0));
        assert_eq!(number_of("nan"), None);
        assert_eq!(color_of("blue").as_deref(), Some("#3B82F6"));
        assert_eq!(color_of("#abc").as_deref(), Some("#AABBCC"));
    }

    #[test]
    fn finds_objects_with_typos_and_plurals() {
        let t = tokenize("add three towers");
        assert_eq!(find_object(&t).unwrap().kind, "tower");
        let t = tokenize("build a pyramd");
        let hit = find_object(&t).unwrap();
        assert_eq!(hit.kind, "pyramid");
        assert!(hit.fuzzy);
    }
}
