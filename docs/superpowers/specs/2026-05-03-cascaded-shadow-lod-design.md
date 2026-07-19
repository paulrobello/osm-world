> ⚠️ Historical spec (2026-05) — retained for reference; current behavior may differ. See `docs/ARCHITECTURE.md` and the source code.

# Cascaded Shadow LOD Design

## Goal

Replace the current single city-scale directional shadow map with a dynamic shadow LOD system that works with the day/night cycle and avoids city-scale artifacts.

## Problem

A single 2048² shadow map covering roughly 10 km cannot provide stable, correctly shaped shadows for buildings and streets. It creates a tradeoff that cannot be solved by small patches:

- small coverage gives crisp shadows but visible square edges;
- large coverage avoids edges but makes each texel several metres wide;
- camera-relative projection causes swimming unless snapped;
- receiver geometry can create broad detached shadow smears;
- shadow-map sampling must account for framebuffer-to-texture Y orientation or shadows appear mirrored/offset.

The caster-only and texel-snapping fixes reduce artifacts, but the architecture still needs LOD.

## Selected Approach

Use a compact cascaded shadow map system with four dynamic cascades:

1. **Contact/near cascade**
   - Covers nearby city geometry where contact and street-level shadow quality matters most.
   - Target radius: about 350 m from the camera.

2. **Near cascade**
   - Covers nearby blocks where building shadows should remain crisp.
   - Target radius: about 900 m from the camera.

3. **Mid cascade**
   - Covers medium-distance city geometry.
   - Target radius: about 2200 m from the camera.

4. **Far cascade**
   - Covers wider city context without returning to a single whole-city shadow map.
   - Target radius: about 5200 m from the camera.

Fragments beyond the far cascade fade to fully lit direct light instead of sampling a low-quality whole-city shadow.

## Dynamic Day/Night Behavior

The sky still uses `sun_direction(day_cycle.time_of_day)` and draws the moon opposite the sun. Shadow cascades and direct lighting use `dominant_light_direction(day_cycle.time_of_day)`: the sun while it is above the horizon, then the moon after sunset. This keeps shadows fully dynamic across the day/night cycle instead of leaving night shadows tied to the below-ground sun.

Each cascade snaps its light-space origin to its own shadow texel size. This keeps shadows stable under sub-texel camera movement while still allowing the cascade to update when the camera crosses a shadow texel boundary.

## Data Model

Add a light uniform containing:

- two light view-projection matrices;
- cascade radii in world units;
- fade parameters;
- pass/debug parameters, including active cascade and cascade-debug tint toggle.

The scene uniform carries both `sun_direction` for sky/fog appearance and `light_direction`/`light_intensity` for direct city lighting and shadow bias.

The same uniform is used by:

- the shadow pass vertex shader, selecting which cascade matrix to render with;
- the city fragment shader, selecting and sampling the appropriate cascade.

## GPU Resources

Use a single depth texture with four array layers, one layer per cascade. This keeps binding simple and avoids atlas coordinate math.

Shadow bind group layout:

- binding 0: depth texture array;
- binding 1: comparison sampler;
- binding 2: cascade uniform buffer.

Shadow pass:

- render one layer per cascade matrix;
- render only the dedicated shadow caster index buffer.

## Shader Sampling

The city shader chooses a cascade by camera distance to the fragment:

- choose cascade weights from camera distance to the fragment;
- blend each cascade into the next over a small transition band;
- fade the final cascade to fully lit near the far radius.

Out-of-cascade samples return fully lit. A settings/CLI debug mode tints geometry by selected cascade so the otherwise smoothly blended LOD bands are visible.

A fullscreen contact-shadow composite pass samples the resolved scene color plus scene depth, reconstructs nearby world positions, and ray-marches a short distance toward the dominant light direction. This adds screen-space contact darkening near buildings without putting receiver geometry back into the shadow caster pass.

## Caster Policy

Keep the dedicated shadow caster index buffer and include all building triangles in it. Receiver surfaces remain excluded because they caused the previous large-scale self-shadowing artifacts. With cascades in place, building walls are needed so shadows contact and line up with their casters instead of appearing as detached roof projections.

This is still not a final physically perfect shadow model. Contact details can be addressed later with a tighter near cascade or a separate contact-shadow technique.

## Verification

Automated tests:

- cascade selection and fade behavior;
- light projection stability under sub-texel camera movement;
- shadow caster filtering.

Manual screenshot checks:

- known city camera pose at 14:00;
- near/mid shadows visible but not map-spanning;
- no obvious camera sliding in repeated captures;
- debug cascade screenshot showing near/mid/far bands;
- night screenshot confirming moon-direction shadows after sunset;
- night sky screenshot confirming stars are isolated points rather than large cell-wide twinkling groups.

## Out of Scope

- screen-space ambient occlusion beyond the lightweight contact-shadow pass;
- obstacle placement/editor UX, which belongs to `par-particle-life`, not this repo.
