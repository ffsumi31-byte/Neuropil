# Neuropil

Chat-driven 3D modelling with sharp, 8-corner cubes. You describe a model in plain English;
a planner written in **Rust** builds it inside the server — **no API key, no cloud AI, no model download**.

```
browser (index.html · studio.js · styles.css)      Rust server (src/)
 chat UI + Three.js viewer           ──/api/ai──▶   nlu.rs      tokens, numbers, colours, fuzzy words, clauses
 draws whatever Rust returns         ◀──────────    brain.rs    intent → plan → execute → self-check → reply
                                      /api/op       builders.rs 16 procedural recipes (tower, house, castle, robot…)
                                      /api/export   geom.rs     cube kernel: extrude, taper, mirror, bounds, OBJ
```

The server is stateless: every request carries the workspace, Rust returns the new one.
The same prompt on the same workspace always gives the same result.

## What it understands

- Build: `build a tower with 8 floors`, `add 3 trees`, `a huge neon robot`, `build a bridge with 5 spans`
- Edit by part or object: `make the roof blue`, `paint the house gold`, `delete the chimney`
- Transform: `make it taller`, `scale it 2x`, `move the roof up 1`, `mirror it`, `duplicate it 3 times`
- Chisel: `extrude the top 3 times and taper into a sharp obelisk`
- Chain steps: `build a house, make the roof red and add 2 trees`
- `undo`, `clear`, `help`

Unknown objects get an honest abstract sculpture seeded from the prompt, and the reply says so.

## Run locally

```
cargo run          # http://localhost:8080
cargo test         # unit tests for the kernel, parser, builders and planner
```

## Deploy (Render)

`render.yaml` already builds with `cargo build --release` and health-checks `/api/health`.
The frontend files are embedded into the binary (`include_str!`), so push `index.html`,
`styles.css` and `studio.js` together with `src/`.

## API

| Route | Body | Returns |
|---|---|---|
| `GET /api/health` | – | engine version, object list |
| `POST /api/ai` | `{prompt, workspace, selected?}` | `{reply, intent, steps, workspace, changed, suggestions, action}` |
| `POST /api/op` | `{op: push\|extrude\|taper\|mirror, target, …, workspace}` | `{workspace, changed, message}` |
| `POST /api/export/obj` | `{workspace}` | Wavefront OBJ text (world-space, no helpers) |
| `POST /api/prompt` | same as `/api/ai` | legacy alias used by `brain_ingest.py` |

## Teach it a new object

1. Add a recipe `fn lamp(p: &Params) -> Vec<Cube>` in `src/builders.rs` and list it in `build()` and `KINDS`.
2. Add its words to `OBJECTS` in `src/nlu.rs`, e.g. `("lamp", &["lamp", "lantern"])`.

That's it — placement, naming, colours, `make the <part> red`, mirror, duplicate and export all work automatically.

## Legacy files

`app.ts` and `brain_semantic.py` target an older graph API (`/api/brain`, `/api/learn`) that no longer exists.
`brain_ingest.py` still works through `/api/prompt`.
