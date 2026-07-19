# Ideas for `osm-world`

Grounded in the current project: a Rust/WGPU 3D OpenStreetMap renderer with tile streaming, LODs, terrain/elevation, buildings, roads, railways, water/landuse, point features, street signs, minimap, day/night lighting, shadows, an Axum area-prep API, and a Next.js/OpenLayers map picker.

Remove completed items from this list.

## Visual and rendering ideas
8. **Screenshot tour mode**
    - Accept a list of camera positions/times of day and render a screenshot sequence.
    - Value: repeatable visual regression captures and showcase videos.

## Interaction and navigation ideas

11. **Route/path preview overlay**
    - Let users enter a start/end point or select two POIs, then draw a simple route overlay using loaded road geometry.
    - First version can be visual-only and operate on the loaded road graph.

12. **Guided camera modes**
    - Add orbit, follow-road, and top-down map camera modes in addition to flycam.
    - Value: easier demos and screenshots for non-gamer users.

13. **Measurement tools**
    - Add click-to-measure distance/elevation difference in the 3D scene or map picker.
    - Value: useful for validating coordinate/elevation accuracy.

## Data and source ideas

17. **Map data diagnostics endpoint**
    - Add an API endpoint that summarizes feature counts, bbox size, source status, cache paths, and estimated renderer cost before launching.
    - Value: helps users understand why a selected area is slow or sparse.

18. **Prepared normalized scene format**
    - Introduce a renderer-focused prepared JSON/binary scene file instead of relying only on `.osm` XML/PBF inputs.
    - Value: faster loading, stable source metadata, and easier multi-source data integration.

## Performance and engineering ideas

20. **Persistent tile mesh cache**
    - Cache generated CPU tile meshes by input hash, tile coord, LOD, and generator version.
    - Value: faster repeated launches of the same area.

21. **Adaptive upload budget**
    - Adjust per-frame GPU upload budget based on frame time so loading speeds up when idle and backs off during frame drops.

23. **Automated visual regression scenes**
    - Maintain a small suite of prepared city scenes plus screenshot camera positions.
    - Compare image outputs or at least file generation success in CI/local verification.
