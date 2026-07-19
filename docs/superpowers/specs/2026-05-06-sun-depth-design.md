> ⚠️ Historical spec (2026-05) — retained for reference; current behavior may differ. See `docs/ARCHITECTURE.md` and the source code.

# Sun Depth Design

## Goal
Make the analytic sky sun read as a natural, volumetric celestial body instead of a flat white disk.

## Approach
Use a shader-only change in `shaders/sky.wgsl`. Keep the existing full-screen sky pipeline and `sun_direction` uniform. Replace the current hard white disk with layered analytic terms: a warm core, softened limb falloff, subtle procedural surface variation, and a broader corona glow.

## Scope
- Modify only the sky shader and shader source tests.
- Do not add a post-processing bloom pass or new render pipeline.
- Preserve current day/night timing and moon behavior.

## Verification
- Add a regression/source test that the sky shader includes the new sun-depth terms and remains parseable WGSL.
- Run targeted shader tests, then repository verification as practical.
