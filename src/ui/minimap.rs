use crate::camera::Flycam;

fn player_arrow_direction(camera: &Flycam, rotate_with_camera: bool) -> egui::Vec2 {
    world_direction_on_minimap(camera, rotate_with_camera, horizontal_forward(camera))
}

fn world_direction_on_minimap(
    camera: &Flycam,
    rotate_with_camera: bool,
    world_direction: glam::Vec3,
) -> egui::Vec2 {
    let map_up = minimap_up(camera, rotate_with_camera);
    let view = glam::Mat4::look_to_rh(glam::Vec3::ZERO, glam::Vec3::NEG_Y, map_up);
    let screen_direction = view.transform_vector3(world_direction);
    // `look_to_rh(..., NEG_Y, map_up)` produces a view-space +X axis that is
    // opposite egui's screen-right direction for this top-down overlay.
    egui::vec2(-screen_direction.x, screen_direction.y).normalized()
}

fn horizontal_forward(camera: &Flycam) -> glam::Vec3 {
    let forward = camera.forward();
    let horizontal = glam::vec3(forward.x, 0.0, forward.z).normalize_or_zero();
    if horizontal.length_squared() > 0.0 {
        horizontal
    } else {
        glam::Vec3::NEG_Z
    }
}

fn minimap_up(camera: &Flycam, rotate_with_camera: bool) -> glam::Vec3 {
    if rotate_with_camera {
        -horizontal_forward(camera)
    } else {
        glam::Vec3::Z
    }
}

pub struct MinimapState {
    pub visible: bool,
    pub zoom: f32,
    pub rotate_with_camera: bool,
    pub show_tile_debug: bool,
    pub texture_id: Option<egui::TextureId>,
}

impl Default for MinimapState {
    fn default() -> Self {
        Self {
            visible: true,
            zoom: 500.0,
            rotate_with_camera: false,
            show_tile_debug: false,
            texture_id: None,
        }
    }
}

fn compass_heading_degrees(camera: &Flycam) -> f32 {
    let forward = horizontal_forward(camera);
    let degrees = forward.x.atan2(-forward.z).to_degrees().rem_euclid(360.0);
    if degrees >= 359.999 { 0.0 } else { degrees }
}

fn compass_heading_label(degrees: f32) -> &'static str {
    const LABELS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    let index = ((degrees + 22.5) / 45.0).floor() as usize % LABELS.len();
    LABELS[index]
}

fn draw_compass(
    painter: &egui::Painter,
    rect: egui::Rect,
    camera: &Flycam,
    rotate_with_camera: bool,
) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.5 - 17.0;
    let cardinals = [
        ("N", glam::Vec3::NEG_Z, egui::Color32::from_rgb(255, 80, 64)),
        ("E", glam::Vec3::X, egui::Color32::WHITE),
        ("S", glam::Vec3::Z, egui::Color32::LIGHT_GRAY),
        ("W", glam::Vec3::NEG_X, egui::Color32::WHITE),
    ];

    painter.circle_stroke(
        center,
        radius,
        egui::Stroke::new(1.0_f32, egui::Color32::from_white_alpha(150)),
    );

    for (label, world_direction, color) in cardinals {
        let direction = world_direction_on_minimap(camera, rotate_with_camera, world_direction);
        let pos = center + direction * radius;
        painter.text(
            pos,
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(13.0),
            color,
        );
    }

    let heading = compass_heading_degrees(camera);
    let heading_text = format!(
        "{} {:03.0}°",
        compass_heading_label(heading),
        heading.round()
    );
    let label_rect = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(7.0, 7.0),
        egui::vec2(72.0, 20.0),
    );
    painter.rect_filled(label_rect, 3.0, egui::Color32::from_black_alpha(155));
    painter.text(
        label_rect.center(),
        egui::Align2::CENTER_CENTER,
        heading_text,
        egui::FontId::monospace(12.0),
        egui::Color32::WHITE,
    );
}

fn tile_debug_legend_entries() -> [(crate::stream::TileDebugState, egui::Color32); 7] {
    use crate::stream::TileDebugState;
    [
        (
            TileDebugState::Queued,
            egui::Color32::from_rgb(120, 170, 255),
        ),
        (
            TileDebugState::Generating,
            egui::Color32::from_rgb(255, 214, 92),
        ),
        (
            TileDebugState::Uploaded,
            egui::Color32::from_rgb(122, 220, 255),
        ),
        (
            TileDebugState::Visible,
            egui::Color32::from_rgb(101, 240, 162),
        ),
        (
            TileDebugState::Culled,
            egui::Color32::from_rgb(150, 150, 165),
        ),
        (
            TileDebugState::Evicted,
            egui::Color32::from_rgb(190, 120, 255),
        ),
        (TileDebugState::Failed, egui::Color32::from_rgb(255, 86, 86)),
    ]
}

fn tile_debug_color(state: crate::stream::TileDebugState) -> egui::Color32 {
    tile_debug_legend_entries()
        .into_iter()
        .find_map(|(candidate, color)| (candidate == state).then_some(color))
        .unwrap_or(egui::Color32::WHITE)
}

fn world_point_to_minimap(
    rect: egui::Rect,
    camera: &Flycam,
    rotate_with_camera: bool,
    zoom: f32,
    point: glam::Vec2,
) -> egui::Pos2 {
    let delta = glam::vec3(
        point.x - camera.position.x,
        0.0,
        point.y - camera.position.z,
    );
    let view = glam::Mat4::look_to_rh(
        glam::Vec3::ZERO,
        glam::Vec3::NEG_Y,
        minimap_up(camera, rotate_with_camera),
    );
    let screen = view.transform_vector3(delta);
    let scale = rect.width().min(rect.height()) / (zoom.max(1.0) * 2.0);
    rect.center() + egui::vec2(-screen.x * scale, screen.y * scale)
}

fn draw_tile_debug_overlay(
    painter: &egui::Painter,
    rect: egui::Rect,
    camera: &Flycam,
    state: &MinimapState,
    tile_debug_entries: &[crate::stream::TileDebugEntry],
    tile_size: f32,
) {
    let clip_rect = rect.expand(24.0);
    for entry in tile_debug_entries {
        let tile_rect = entry.coord.rect(tile_size.max(1.0));
        let points = [
            world_point_to_minimap(
                rect,
                camera,
                state.rotate_with_camera,
                state.zoom,
                glam::vec2(tile_rect.min_x, tile_rect.min_z),
            ),
            world_point_to_minimap(
                rect,
                camera,
                state.rotate_with_camera,
                state.zoom,
                glam::vec2(tile_rect.max_x, tile_rect.min_z),
            ),
            world_point_to_minimap(
                rect,
                camera,
                state.rotate_with_camera,
                state.zoom,
                glam::vec2(tile_rect.max_x, tile_rect.max_z),
            ),
            world_point_to_minimap(
                rect,
                camera,
                state.rotate_with_camera,
                state.zoom,
                glam::vec2(tile_rect.min_x, tile_rect.max_z),
            ),
        ];
        if !points.iter().any(|point| clip_rect.contains(*point)) {
            continue;
        }
        let color = tile_debug_color(entry.state);
        painter.add(egui::Shape::convex_polygon(
            points.to_vec(),
            color.linear_multiply(0.18),
            egui::Stroke::new(1.0_f32, color),
        ));
    }

    let mut y = rect.top() + 31.0;
    for (debug_state, color) in tile_debug_legend_entries() {
        let swatch =
            egui::Rect::from_min_size(egui::pos2(rect.left() + 7.0, y + 3.0), egui::vec2(7.0, 7.0));
        painter.rect_filled(swatch, 1.0, color);
        painter.text(
            egui::pos2(rect.left() + 18.0, y),
            egui::Align2::LEFT_TOP,
            debug_state.label(),
            egui::FontId::monospace(9.5),
            egui::Color32::from_white_alpha(220),
        );
        y += 13.0;
    }
}

fn handle_minimap_click(state: &mut MinimapState, clicked: bool, ctrl_down: bool) -> bool {
    if clicked && ctrl_down {
        state.rotate_with_camera = !state.rotate_with_camera;
        true
    } else {
        false
    }
}

pub fn draw(
    ctx: &egui::Context,
    camera: &Flycam,
    state: &mut MinimapState,
    tile_debug_entries: &[crate::stream::TileDebugEntry],
    tile_size: f32,
) {
    if !state.visible {
        return;
    }

    let minimap_size = 256.0_f32;
    let padding = 8.0;

    egui::Area::new(egui::Id::new("minimap"))
        .anchor(egui::Align2::RIGHT_BOTTOM, [-padding, -padding])
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(egui::Color32::from_black_alpha(180))
                .corner_radius(4.0)
                .inner_margin(2.0)
                .show(ui, |ui| {
                    let (rect, response) = ui
                        .allocate_exact_size(egui::Vec2::splat(minimap_size), egui::Sense::click());

                    if let Some(tex_id) = state.texture_id {
                        let sized = egui::load::SizedTexture {
                            id: tex_id,
                            size: egui::Vec2::splat(minimap_size),
                        };
                        let uv =
                            egui::Rect::from_min_max(egui::pos2(0.0, 1.0), egui::pos2(1.0, 0.0));
                        ui.put(
                            rect,
                            egui::Image::from_texture(sized)
                                .uv(uv)
                                .fit_to_exact_size(egui::Vec2::splat(minimap_size)),
                        );
                    }

                    // Player arrow
                    let center = rect.center();
                    let arrow_size = 8.0;
                    let arrow_direction = player_arrow_direction(camera, state.rotate_with_camera);
                    let wing_direction = egui::Vec2::new(-arrow_direction.y, arrow_direction.x);
                    let tip = center + arrow_direction * arrow_size;
                    let tail = center - arrow_direction * arrow_size * 0.55;
                    let left = tail + wing_direction * arrow_size * 0.45;
                    let right = tail - wing_direction * arrow_size * 0.45;

                    let painter = ui.painter_at(rect);
                    if state.show_tile_debug {
                        draw_tile_debug_overlay(
                            &painter,
                            rect,
                            camera,
                            state,
                            tile_debug_entries,
                            tile_size,
                        );
                    }
                    draw_compass(&painter, rect, camera, state.rotate_with_camera);
                    painter.add(egui::Shape::convex_polygon(
                        vec![tip, left, right],
                        egui::Color32::WHITE,
                        egui::Stroke::new(1.0_f32, egui::Color32::BLACK),
                    ));

                    // Do not request focus here: if egui owns keyboard focus after
                    // Ctrl+click, WASD camera movement is temporarily swallowed.
                    handle_minimap_click(state, response.clicked(), ui.input(|i| i.modifiers.ctrl));

                    // Scroll to zoom
                    if response.hover_pos().is_some_and(|p| rect.contains(p)) {
                        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                        if scroll != 0.0 {
                            state.zoom = (state.zoom * (1.0 - scroll * 0.001)).clamp(200.0, 2000.0);
                        }
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compass_heading_uses_north_zero_clockwise_degrees() {
        let mut camera = Flycam::new(1.0);
        camera.pitch = 0.0;

        camera.yaw = -std::f32::consts::FRAC_PI_2;
        assert!((compass_heading_degrees(&camera) - 0.0).abs() < 0.001);
        assert_eq!(compass_heading_label(compass_heading_degrees(&camera)), "N");

        camera.yaw = 0.0;
        assert!((compass_heading_degrees(&camera) - 90.0).abs() < 0.001);
        assert_eq!(compass_heading_label(compass_heading_degrees(&camera)), "E");

        camera.yaw = std::f32::consts::FRAC_PI_2;
        assert!((compass_heading_degrees(&camera) - 180.0).abs() < 0.001);
        assert_eq!(compass_heading_label(compass_heading_degrees(&camera)), "S");

        camera.yaw = std::f32::consts::PI;
        assert!((compass_heading_degrees(&camera) - 270.0).abs() < 0.001);
        assert_eq!(compass_heading_label(compass_heading_degrees(&camera)), "W");
    }

    #[test]
    fn fixed_map_places_east_right_and_west_left() {
        let mut camera = Flycam::new(1.0);
        camera.yaw = -std::f32::consts::FRAC_PI_2;
        camera.pitch = 0.0;

        let east = world_direction_on_minimap(&camera, false, glam::Vec3::X);
        let west = world_direction_on_minimap(&camera, false, glam::Vec3::NEG_X);

        assert!(east.x > 0.99, "east should be right, got {east:?}");
        assert!(west.x < -0.99, "west should be left, got {west:?}");
        assert!(east.y.abs() < 0.001);
        assert!(west.y.abs() < 0.001);
    }

    #[test]
    fn tile_debug_overlay_starts_hidden() {
        let state = MinimapState::default();

        assert!(!state.show_tile_debug);
    }

    #[test]
    fn ctrl_click_toggles_minimap_rotation() {
        let mut state = MinimapState::default();

        assert!(handle_minimap_click(&mut state, true, true));
        assert!(state.rotate_with_camera);

        assert!(handle_minimap_click(&mut state, true, true));
        assert!(!state.rotate_with_camera);
    }

    #[test]
    fn plain_click_does_not_toggle_minimap_rotation() {
        let mut state = MinimapState::default();

        assert!(!handle_minimap_click(&mut state, true, false));
        assert!(!state.rotate_with_camera);
    }

    #[test]
    fn arrow_points_north_when_camera_faces_north_on_fixed_map() {
        let mut camera = Flycam::new(1.0);
        camera.yaw = -std::f32::consts::FRAC_PI_2;
        camera.pitch = 0.0;

        let direction = player_arrow_direction(&camera, false);

        assert!(direction.x.abs() < 0.001);
        assert!(direction.y < -0.99);
    }

    #[test]
    fn arrow_points_up_when_map_rotates_with_camera() {
        let mut camera = Flycam::new(1.0);
        camera.yaw = 1.0;
        camera.pitch = 0.0;

        let direction = player_arrow_direction(&camera, true);

        assert!(direction.x.abs() < 0.001);
        assert!(direction.y < -0.99);
    }

    #[test]
    fn tile_debug_legend_covers_all_streaming_states() {
        let states: Vec<_> = tile_debug_legend_entries()
            .into_iter()
            .map(|(state, _)| state)
            .collect();

        assert_eq!(
            states,
            vec![
                crate::stream::TileDebugState::Queued,
                crate::stream::TileDebugState::Generating,
                crate::stream::TileDebugState::Uploaded,
                crate::stream::TileDebugState::Visible,
                crate::stream::TileDebugState::Culled,
                crate::stream::TileDebugState::Evicted,
                crate::stream::TileDebugState::Failed,
            ]
        );
    }

    #[test]
    fn fixed_minimap_projects_world_east_to_screen_right() {
        let mut camera = Flycam::new(1.0);
        camera.position = glam::vec3(0.0, 10.0, 0.0);
        camera.yaw = -std::f32::consts::FRAC_PI_2;
        camera.pitch = 0.0;
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(256.0, 256.0));

        let center = world_point_to_minimap(rect, &camera, false, 500.0, glam::vec2(0.0, 0.0));
        let east = world_point_to_minimap(rect, &camera, false, 500.0, glam::vec2(100.0, 0.0));

        assert!(east.x > center.x);
        assert!((east.y - center.y).abs() < 0.001);
    }
}
