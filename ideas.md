# Ideas for `osm-world`

Grounded in the current project: a Rust/WGPU 3D OpenStreetMap renderer with tile streaming, LODs, terrain/elevation, buildings, roads, railways, water/landuse, point features, street signs, minimap, day/night lighting, shadows, an Axum area-prep API, and a Next.js/OpenLayers map picker.

Remove completed items from this list.

## High-impact enhancements

1. **City quality preset system**
   - Add presets like `fast-preview`, `balanced`, and `cinematic` for stream radius, upload budget, tile cap, shadow quality, label distances, and LOD thresholds.
   - Expose via CLI and settings UI.
   - Value: easier tuning across laptops, screenshots, and large city runs.

## Visual and rendering ideas

2. **Weather presets**
   - Add foggy morning, golden hour, rainy dusk, and clear noon presets using existing atmosphere, fog, sky, and day-cycle controls.
   - Optional follow-up: simple rain streaks or wet-road specular boost.

3. **Improved water rendering**
   - Add subtle animated normal/wave distortion and stronger sun glints for rivers and lakes.
   - Keep geometry unchanged; start as shader-only work.

7. **Terrain tunnel cuts and bridge abutments**
    - Follow up the current bridge/tunnel visuals with terrain carving around tunnel portals and stronger bridge approach structures.
    - Value: removes the main visual mismatch in layered roads.

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

19. **Streaming debug overlay improvements**
    - Visualize tile states on the minimap: queued, generating, uploaded, visible, culled, evicted, failed.
    - Value: easier tuning and debugging of LOD/streaming behavior.

20. **Persistent tile mesh cache**
    - Cache generated CPU tile meshes by input hash, tile coord, LOD, and generator version.
    - Value: faster repeated launches of the same area.

21. **Adaptive upload budget**
    - Adjust per-frame GPU upload budget based on frame time so loading speeds up when idle and backs off during frame drops.

22. **LOD simplification for buildings**
    - Replace far-distance full building meshes with simplified boxes, merged blocks, or height-only extrusions.
    - Value: reduces far-city GPU cost.

23. **Automated visual regression scenes**
    - Maintain a small suite of prepared city scenes plus screenshot camera positions.
    - Compare image outputs or at least file generation success in CI/local verification.

24. **Graphify-backed architecture docs**
    - Generate a short `docs/architecture.md` from the graphify community structure, linking core modules like `world::loader`, `stream`, `render`, `server`, and `web`.
    - Value: makes onboarding easier as the renderer grows.

## Small polish ideas

25. **Copy command variants**
    - In the web UI, provide copy buttons for debug, release, screenshot, and no-streaming command variants.

26. **Map picker presets**
    - Add quick-select bbox presets for common test areas like Sacramento and Woodland.

27. **Cache cleanup UI**
    - Let users remove old prepared areas from the web picker.

28. **Settings import/export**
    - Save and load renderer settings profiles for lighting, labels, minimap, and performance.

29. **First-run help overlay**
    - Show controls for flycam, minimap rotation, settings, screenshots, and labels the first time the app opens.

30. **Better error messages in the web UI**
    - Show actionable hints for Overpass failure, bbox too large, missing Overture CLI, SRTM download issues, and spawn-outside-bbox problems.
