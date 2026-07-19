> ⚠️ Historical spec (2026-05) — retained for reference; current behavior may differ. See `docs/ARCHITECTURE.md` and the source code.

# osm-world Visual Detail Controls Design

## Goal

Implement ideas 4, 5, and 6 from `ideas.md` as a showcase-oriented visual detail upgrade with controls:

- landmark-specific low-poly models;
- deterministic building façade and roof variation;
- tree and vegetation density/detail controls;
- CLI and in-app settings that make the visuals adjustable;
- screenshot-friendly CLI support so visual changes can be validated repeatably.

## Chosen approach

Use a renderer-side instance/material control approach first. This is larger than a mesh-only pass, but it best matches the requested outcome: highly visible showcase visuals that can be dialed down through settings.

The design separates source data from visual presentation:

1. OSM loading classifies buildings, point features, and vegetation sources.
2. Stable visual metadata is derived from tags and feature hashes.
3. Repeatable detail such as vegetation and landmark silhouettes is represented in render-friendly instance/material data where practical.
4. `VisualDetailSettings` controls visibility, density, detail level, façade intensity, and validation/screenshot behavior.

## Architecture

Add a visual detail subsystem centered on a new `VisualDetailSettings` struct. It should be initialized from CLI flags, stored on the app, and exposed in the in-app Settings panel.

The existing CPU mesh path remains the baseline for terrain, roads, water, landuse, and buildings. New visual detail code should avoid disrupting that path. Detail that is naturally repeatable, such as trees and landmark meshes, should move toward instanced render data so settings can adjust visible density and draw distance without rebuilding every vertex buffer. Where immediate live updates are not practical, changing a setting should clearly mark that a scene reload/regeneration is needed.

## Components

### `src/visual_detail.rs`

Create the central data model:

- `VisualPreset`: `Performance`, `Balanced`, `Showcase`, and `Custom` if needed internally.
- `VisualDetailSettings` with fields for:
  - landmark visibility and landmark detail level;
  - façade variation enabled/intensity;
  - roof variation enabled/intensity;
  - vegetation visibility;
  - vegetation density multiplier;
  - synthetic tree cap;
  - vegetation max visible distance;
  - vegetation/landmark silhouette detail level.
- validation/clamp helpers so UI sliders and CLI values cannot create invalid settings.
- preset constructors, with Showcase as the visually rich mode requested by the user.

### `src/world/point_feature.rs`

Extend point feature classification so generic landmarks become specific visual landmark kinds:

- `man_made=tower` → tower/spire;
- `man_made=water_tower` → elevated tank;
- `man_made=chimney` → chimney/stack;
- `historic=monument`, `historic=memorial`, or `memorial=*` → obelisk/monument;
- `natural=peak` → rocky peak marker;
- `tourism=viewpoint` → lookout marker;
- fallback landmark → generic landmark silhouette.

The classifier should stay deterministic and tag-driven. Names and labels should continue using the existing point-label behavior unless a feature already has a better explicit name.

### `src/world/building.rs` and `src/world/color.rs`

Add deterministic façade and roof variation without texture dependencies.

Façade variation should combine:

- building tags such as `building`, `building:material`, `building:levels`, `height`;
- roof tags such as `roof:material`, `roof:colour`, and `roof:shape` when present;
- a stable hash of the feature footprint/tags as a fallback source of variety.

The first implementation should use vertex colors and simple procedural bands/window stripe hints. It should not introduce external texture assets.

### `src/render/*`

Add render support only as far as needed for the first working version:

- instance buffer support for repeated landmark/vegetation detail where practical;
- a compact instanced pipeline if the existing pipeline cannot express the needed per-instance attributes cleanly;
- per-instance attributes for position, scale, kind/detail variant, color/material variant, and max-visible-distance filtering if feasible.

Avoid a broad renderer rewrite. The new path should coexist with current `SceneBuffers` and streaming tile behavior.

### `src/main.rs`

Add CLI flags needed for visual settings and screenshot validation:

- `--visual-preset performance|balanced|showcase`;
- `--landmark-detail off|simple|showcase`;
- `--facade-variation 0.0..=1.0`;
- `--roof-variation 0.0..=1.0`;
- `--vegetation-density 0.0..=3.0`;
- `--synthetic-tree-cap N`;
- `--vegetation-distance METRES`;
- screenshot validation helpers as needed beyond the existing `--screenshot`, `--screenshot-delay`, `--auto-exit`, camera position, spawn, and `--time-of-day` flags.

If the existing screenshot flags are sufficient after adding visual preset/detail flags, do not add redundant screenshot flags. If repeatable validation needs more control, add only narrowly scoped flags such as a deterministic `--validation-scene` or `--screenshot-label` if they prove necessary during implementation.

### `src/ui/settings.rs`

Add a Visual Detail section to the settings panel:

- preset selector;
- landmark detail controls;
- façade and roof intensity sliders;
- vegetation visible toggle;
- vegetation density slider;
- synthetic tree cap control;
- vegetation max distance slider;
- a clear status label when a changed setting requires scene reload/regeneration.

Shader/material intensity changes should apply live where possible. Placement-changing controls may require reload/regeneration in the first version.

## Behavior

### Landmarks

Landmarks should render as recognizable low-poly silhouettes instead of one generic marker. The showcase preset should make silhouettes visibly different, while lower settings should reduce them to simpler forms or hide them.

### Buildings

Buildings should gain deterministic variation that makes neighborhoods less uniform:

- façade color variation by material/type/hash;
- horizontal window stripe hints or color bands;
- roof color/material variation;
- stable output for the same input file and settings.

No texture files are required for the first version.

### Vegetation

Vegetation controls should tune clutter and performance:

- density multiplier affects synthetic vegetation placement/rendering;
- caps prevent large forests or parks from exceeding geometry/render budgets;
- max distance hides distant vegetation detail;
- showcase mode can be visually dense, but users must be able to dial it down.

## Testing

Add unit tests for:

- visual preset defaults and settings clamping;
- CLI parsing for visual detail flags;
- landmark tag classification;
- deterministic building façade/roof variation;
- vegetation density/cap behavior.

Add mesh/render-adjacent tests where current test patterns support them:

- landmark detail emits distinct geometry or instance kinds for at least tower, chimney, monument, peak, and viewpoint;
- vegetation settings reduce generated or rendered tree detail;
- façade/roof variation is stable for identical input and differs across hash/tag variants.

## Verification

Canonical verification should include:

```bash
cargo test
```

If the repository has a broader check target at implementation time, use that instead or in addition.

Visual validation should use deterministic command lines with existing screenshot support plus new visual flags. Example target commands after implementation:

```bash
cargo run --release -- \
  --input <prepared-area.osm> \
  --visual-preset showcase \
  --time-of-day 16.5 \
  --spawn-lat <lat> \
  --spawn-lon <lon> \
  --cam-yaw <degrees> \
  --cam-pitch <degrees> \
  --screenshot artifacts/visual-showcase.png \
  --screenshot-delay 5 \
  --auto-exit 7
```

and a lower-detail comparison:

```bash
cargo run --release -- \
  --input <prepared-area.osm> \
  --visual-preset performance \
  --vegetation-density 0.25 \
  --facade-variation 0.25 \
  --screenshot artifacts/visual-performance.png \
  --screenshot-delay 5 \
  --auto-exit 7
```

The implementation may adjust exact commands to match available prepared-area fixtures and camera coordinates. The important requirement is that the agent can produce repeatable screenshots for before/after inspection without manual camera movement.

## Non-goals

- Do not add external texture assets in the first version.
- Do not replace the whole renderer.
- Do not require a new scene file format.
- Do not implement visual regression image diffing unless it naturally falls out of the screenshot validation work.
