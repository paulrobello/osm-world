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

Use a compact cascaded shadow map system with two dynamic cascades:

1. **Near cascade**
   - Covers nearby city geometry where shadow quality matters most.
   - Target radius: about 900 m from the camera.
   - Uses high effective resolution.

2. **Mid cascade**
   - Covers medium-distance city geometry.
   - Target radius: about 2800 m from the camera.
   - Lower effective texel density than the near cascade.

Fragments beyond the mid cascade fade to fully lit direct light instead of sampling a low-quality whole-city shadow.

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

Use a single depth texture with two array layers, one layer per cascade. This keeps binding simple and avoids atlas coordinate math.

Shadow bind group layout:

- binding 0: depth texture array;
- binding 1: comparison sampler;
- binding 2: cascade uniform buffer.

Shadow pass:

- render layer 0 with near cascade matrix;
- render layer 1 with mid cascade matrix;
- render only the dedicated shadow caster index buffer.

## Shader Sampling

The city shader chooses a cascade by camera distance to the fragment:

- distance <= near radius: sample near cascade;
- distance <= mid radius: sample mid cascade;
- near/mid transition: blend between cascades over a small band;
- mid/far transition: fade shadow influence to fully lit.

Out-of-cascade samples return fully lit. A settings/CLI debug mode tints geometry by selected cascade so the otherwise smoothly blended LOD bands are visible: blue for near, orange for mid, purple for far fade.

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

- more than two cascades;
- screen-space ambient occlusion/contact shadows;
- obstacle placement/editor UX, which belongs to `par-particle-life`, not this repo.
