use crate::camera::Flycam;
use crate::world::loader::ResolvedPointFeature;
use crate::world::point_feature::{PointFeatureKind, point_feature_label, point_feature_style};

const MAX_VISIBLE_LABELS: usize = 24;
const STREET_SIGN_BOARD_LABEL_Y_OFFSET: f32 = 2.56;
const STREET_SIGN_ON_SIGN_DISTANCE: f32 = 120.0;

#[derive(Clone, Debug)]
pub struct PoiLabelSettings {
    pub visible: bool,
    pub max_distance: f32,
}

impl Default for PoiLabelSettings {
    fn default() -> Self {
        Self {
            visible: true,
            max_distance: 300.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StreetSignLabelSettings {
    pub visible: bool,
    pub max_distance: f32,
}

impl Default for StreetSignLabelSettings {
    fn default() -> Self {
        Self {
            visible: true,
            max_distance: 500.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PoiLabel {
    pub text: String,
    pub position: glam::Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StreetSignLabel {
    pub text: String,
    pub position: glam::Vec3,
    pub tangent: (f32, f32),
}

struct LabelDrawStyle {
    id_prefix: &'static str,
    fill: egui::Color32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StreetSignLabelMode {
    OnSign,
    FloatingBadge,
}

fn street_sign_label_mode(distance: f32) -> StreetSignLabelMode {
    if distance <= STREET_SIGN_ON_SIGN_DISTANCE {
        StreetSignLabelMode::OnSign
    } else {
        StreetSignLabelMode::FloatingBadge
    }
}

fn label_overlay_order() -> egui::Order {
    // Keep world-projected labels below normal egui windows such as Settings.
    egui::Order::Background
}

pub fn labels_from_point_features(point_features: &[ResolvedPointFeature]) -> Vec<PoiLabel> {
    point_features
        .iter()
        .filter_map(|feature| {
            let style = point_feature_style(&feature.tags)?;
            if !matches!(
                style.kind,
                PointFeatureKind::Poi | PointFeatureKind::Landmark
            ) {
                return None;
            }
            let text = point_feature_label(&feature.tags)?;
            let y_offset = match style.kind {
                PointFeatureKind::Landmark => 5.8,
                PointFeatureKind::Poi => 5.4,
                PointFeatureKind::Tree | PointFeatureKind::Nature => return None,
            };
            Some(PoiLabel {
                text,
                position: glam::vec3(
                    feature.point.0,
                    feature.elevation + y_offset,
                    feature.point.1,
                ),
            })
        })
        .collect()
}

pub fn labels_from_street_signs(
    street_signs: &[crate::world::street_sign::ResolvedStreetSign],
) -> Vec<StreetSignLabel> {
    street_signs
        .iter()
        .filter(|sign| !sign.name.trim().is_empty())
        .map(|sign| StreetSignLabel {
            text: sign.name.trim().to_string(),
            position: glam::vec3(
                sign.point.0,
                sign.elevation + STREET_SIGN_BOARD_LABEL_Y_OFFSET,
                sign.point.1,
            ),
            tangent: sign.tangent,
        })
        .collect()
}

pub fn draw(
    ctx: &egui::Context,
    camera: &Flycam,
    labels: &[PoiLabel],
    settings: &PoiLabelSettings,
    viewport_size: egui::Vec2,
) {
    draw_projected_labels(
        ctx,
        camera,
        labels,
        settings.visible,
        settings.max_distance,
        viewport_size,
        LabelDrawStyle {
            id_prefix: "poi_label",
            fill: egui::Color32::from_black_alpha(185),
        },
    );
}

pub fn draw_street_signs(
    ctx: &egui::Context,
    camera: &Flycam,
    labels: &[StreetSignLabel],
    settings: &StreetSignLabelSettings,
    viewport_size: egui::Vec2,
) {
    if !settings.visible || labels.is_empty() {
        return;
    }

    let mut visible: Vec<_> = labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| {
            let distance = label.position.distance(camera.position);
            if distance > settings.max_distance {
                return None;
            }
            let screen_pos = project_world_to_screen(camera, label.position, viewport_size)?;
            Some((distance, index, label, screen_pos))
        })
        .collect();
    visible.sort_by(|a, b| a.0.total_cmp(&b.0));

    let painter = ctx.layer_painter(egui::LayerId::new(
        label_overlay_order(),
        egui::Id::new("street_sign_board_labels"),
    ));
    for (distance, index, label, screen_pos) in visible.into_iter().take(MAX_VISIBLE_LABELS) {
        match street_sign_label_mode(distance) {
            StreetSignLabelMode::OnSign => {
                draw_on_sign_text(&painter, camera, label, viewport_size)
            }
            StreetSignLabelMode::FloatingBadge => draw_floating_label(
                ctx,
                &LabelDrawStyle {
                    id_prefix: "street_sign_label",
                    fill: egui::Color32::from_rgba_unmultiplied(8, 74, 38, 220),
                },
                index,
                &label.text,
                screen_pos,
            ),
        }
    }
}

fn draw_projected_labels(
    ctx: &egui::Context,
    camera: &Flycam,
    labels: &[PoiLabel],
    visible_enabled: bool,
    max_distance: f32,
    viewport_size: egui::Vec2,
    style: LabelDrawStyle,
) {
    if !visible_enabled || labels.is_empty() {
        return;
    }

    let mut visible: Vec<_> = labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| {
            let distance = label.position.distance(camera.position);
            if distance > max_distance {
                return None;
            }
            let screen_pos = project_world_to_screen(camera, label.position, viewport_size)?;
            Some((distance, index, label, screen_pos))
        })
        .collect();
    visible.sort_by(|a, b| a.0.total_cmp(&b.0));

    for (_distance, index, label, screen_pos) in visible.into_iter().take(MAX_VISIBLE_LABELS) {
        draw_floating_label(ctx, &style, index, &label.text, screen_pos);
    }
}

fn draw_floating_label(
    ctx: &egui::Context,
    style: &LabelDrawStyle,
    index: usize,
    text: &str,
    screen_pos: egui::Pos2,
) {
    egui::Area::new(egui::Id::new((style.id_prefix, index)))
        .order(label_overlay_order())
        .interactable(false)
        .fixed_pos(screen_pos + egui::vec2(8.0, -30.0))
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(style.fill)
                .corner_radius(3.0)
                .inner_margin(egui::Margin::symmetric(5, 2))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(text)
                            .color(egui::Color32::WHITE)
                            .small(),
                    );
                });
        });
}

fn draw_on_sign_text(
    painter: &egui::Painter,
    camera: &Flycam,
    label: &StreetSignLabel,
    viewport_size: egui::Vec2,
) {
    for quad in street_sign_text_world_quads(label, camera.position) {
        let Some(points) = project_world_quad_to_screen(camera, quad, viewport_size) else {
            continue;
        };
        painter.add(egui::Shape::convex_polygon(
            points.to_vec(),
            egui::Color32::WHITE,
            egui::Stroke::NONE,
        ));
    }
}

fn street_sign_text_world_quads(
    label: &StreetSignLabel,
    camera_position: glam::Vec3,
) -> Vec<[glam::Vec3; 4]> {
    const BOARD_HALF_WIDTH: f32 = 1.32;
    const BOARD_HALF_HEIGHT: f32 = 0.24;
    const TEXT_FACE_OFFSET: f32 = 0.105;
    const GLYPH_COLUMNS: usize = 5;
    const GLYPH_ROWS: usize = 7;
    const GLYPH_GAP_COLUMNS: usize = 1;
    const CELL_FILL: f32 = 0.82;
    const MIN_READABLE_FACE_DOT: f32 = 0.18;

    let text = label.text.trim().to_uppercase();
    if text.is_empty() {
        return Vec::new();
    }

    let char_count = text.chars().count();
    let total_columns = text
        .chars()
        .enumerate()
        .fold(0usize, |columns, (index, ch)| {
            columns + glyph_width(ch) + usize::from(index + 1 < char_count) * GLYPH_GAP_COLUMNS
        });
    if total_columns == 0 {
        return Vec::new();
    }

    let max_width = BOARD_HALF_WIDTH * 2.0 * 0.86;
    let max_height = BOARD_HALF_HEIGHT * 2.0 * 0.70;
    let cell = (max_width / total_columns as f32).min(max_height / GLYPH_ROWS as f32);
    let text_width = total_columns as f32 * cell;
    let text_height = GLYPH_ROWS as f32 * cell;

    let tangent = normalize_label_tangent(label.tangent);
    let sign_right = glam::vec3(tangent.0, 0.0, tangent.1);
    let normal_candidate = glam::vec3(-tangent.1, 0.0, tangent.0);
    let to_camera = (camera_position - label.position).normalize_or_zero();
    let camera_side = to_camera.dot(normal_candidate);
    if camera_side.abs() < MIN_READABLE_FACE_DOT {
        return Vec::new();
    }
    let (face_normal, right) = if camera_side >= 0.0 {
        (normal_candidate, sign_right)
    } else {
        (-normal_candidate, -sign_right)
    };
    let up = glam::Vec3::Y;
    let origin = label.position + face_normal * TEXT_FACE_OFFSET - right * (text_width * 0.5)
        + up * (text_height * 0.5);

    let mut quads = Vec::new();
    let mut cursor_column = 0usize;
    for ch in text.chars() {
        let rows = glyph_rows(ch);
        let width = glyph_width(ch);
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..width.min(GLYPH_COLUMNS) {
                let mask = 1 << (GLYPH_COLUMNS - 1 - col);
                if bits & mask == 0 {
                    continue;
                }
                let x0 = (cursor_column + col) as f32 * cell;
                let x1 = x0 + cell * CELL_FILL;
                let y0 = -(row as f32) * cell;
                let y1 = y0 - cell * CELL_FILL;
                quads.push([
                    origin + right * x0 + up * y0,
                    origin + right * x1 + up * y0,
                    origin + right * x1 + up * y1,
                    origin + right * x0 + up * y1,
                ]);
            }
        }
        cursor_column += width + GLYPH_GAP_COLUMNS;
    }
    quads
}

fn normalize_label_tangent(tangent: (f32, f32)) -> (f32, f32) {
    let len = (tangent.0 * tangent.0 + tangent.1 * tangent.1).sqrt();
    if len <= 1e-6 {
        (1.0, 0.0)
    } else {
        (tangent.0 / len, tangent.1 / len)
    }
}

fn glyph_width(ch: char) -> usize {
    if ch == ' ' { 3 } else { 5 }
}

fn glyph_rows(ch: char) -> [u8; 7] {
    match ch {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '/' => [
            0b00001, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000,
        ],
        '&' => [
            0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101,
        ],
        ' ' => [0; 7],
        _ => [0b11111, 0b00001, 0b00010, 0b00100, 0b00100, 0, 0b00100],
    }
}

fn project_world_quad_to_screen(
    camera: &Flycam,
    quad: [glam::Vec3; 4],
    viewport_size: egui::Vec2,
) -> Option<[egui::Pos2; 4]> {
    Some([
        project_world_to_screen(camera, quad[0], viewport_size)?,
        project_world_to_screen(camera, quad[1], viewport_size)?,
        project_world_to_screen(camera, quad[2], viewport_size)?,
        project_world_to_screen(camera, quad[3], viewport_size)?,
    ])
}

fn project_world_to_screen(
    camera: &Flycam,
    world_position: glam::Vec3,
    viewport_size: egui::Vec2,
) -> Option<egui::Pos2> {
    if viewport_size.x <= 0.0 || viewport_size.y <= 0.0 {
        return None;
    }
    let clip = (camera.projection_matrix() * camera.view_matrix()) * world_position.extend(1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if ndc.x < -1.0 || ndc.x > 1.0 || ndc.y < -1.0 || ndc.y > 1.0 {
        return None;
    }
    Some(egui::pos2(
        (ndc.x + 1.0) * 0.5 * viewport_size.x,
        (1.0 - ndc.y) * 0.5 * viewport_size.y,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn tags(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn projected_labels_draw_below_settings_windows() {
        assert_eq!(label_overlay_order(), egui::Order::Background);
    }

    #[test]
    fn labels_include_named_pois_and_skip_trees() {
        let features = vec![
            ResolvedPointFeature {
                tags: tags(&[("amenity", "restaurant"), ("name", "Taco Bell")]),
                point: (1.0, 2.0),
                elevation: 3.0,
                rep_lat: 0.0,
                rep_lon: 0.0,
            },
            ResolvedPointFeature {
                tags: tags(&[("natural", "tree")]),
                point: (4.0, 5.0),
                elevation: 6.0,
                rep_lat: 0.0,
                rep_lon: 0.0,
            },
        ];

        let labels = labels_from_point_features(&features);

        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].text, "Taco Bell");
        assert_eq!(labels[0].position, glam::vec3(1.0, 8.4, 2.0));
    }

    #[test]
    fn label_settings_default_to_nearby_labels_only() {
        let settings = PoiLabelSettings::default();

        assert!(settings.visible);
        assert_eq!(settings.max_distance, 300.0);
    }

    #[test]
    fn labels_include_street_sign_names_independent_from_pois() {
        let signs = vec![crate::world::street_sign::ResolvedStreetSign {
            name: "Main Street".to_string(),
            point: (1.0, 2.0),
            elevation: 3.0,
            tangent: (1.0, 0.0),
            rep_lat: 0.0,
            rep_lon: 0.0,
        }];

        let labels = labels_from_street_signs(&signs);

        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].text, "Main Street");
        assert_eq!(labels[0].position, glam::vec3(1.0, 5.56, 2.0));
        assert_eq!(labels[0].tangent, (1.0, 0.0));
    }

    #[test]
    fn street_sign_text_quads_follow_the_sign_plane() {
        let label = StreetSignLabel {
            text: "A".to_string(),
            position: glam::Vec3::ZERO,
            tangent: (0.0, 1.0),
        };

        let quads = street_sign_text_world_quads(&label, glam::vec3(-10.0, 0.0, 0.0));

        assert!(!quads.is_empty());
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_z = f32::MAX;
        let mut max_z = f32::MIN;
        for point in quads.iter().flatten() {
            min_x = min_x.min(point.x);
            max_x = max_x.max(point.x);
            min_z = min_z.min(point.z);
            max_z = max_z.max(point.z);
        }
        assert!(
            max_x - min_x < 0.01,
            "sign-plane glyphs should stay on the face plane, x range was {}",
            max_x - min_x
        );
        assert!(
            max_z - min_z > 0.01,
            "glyphs should advance along the sign tangent, z range was {}",
            max_z - min_z
        );
    }

    #[test]
    fn street_sign_text_flips_horizontally_on_back_face() {
        let label = StreetSignLabel {
            text: "A".to_string(),
            position: glam::Vec3::ZERO,
            tangent: (0.0, 1.0),
        };

        let front = street_sign_text_world_quads(&label, glam::vec3(-10.0, 0.0, 0.0));
        let back = street_sign_text_world_quads(&label, glam::vec3(10.0, 0.0, 0.0));

        assert!(!front.is_empty());
        assert!(!back.is_empty());
        let front_first_z = front[0][0].z;
        let back_first_z = back[0][0].z;
        assert!(
            front_first_z < 0.0,
            "front text should start on the left side of the front face"
        );
        assert!(
            back_first_z > 0.0,
            "back text should flip its horizontal basis so it reads from behind"
        );
    }

    #[test]
    fn street_sign_text_is_hidden_at_grazing_angles() {
        let label = StreetSignLabel {
            text: "A".to_string(),
            position: glam::Vec3::ZERO,
            tangent: (0.0, 1.0),
        };

        let quads = street_sign_text_world_quads(&label, glam::vec3(0.01, 0.0, 10.0));

        assert!(quads.is_empty());
    }

    #[test]
    fn close_street_sign_labels_draw_on_the_sign_before_floating() {
        assert_eq!(
            street_sign_label_mode(STREET_SIGN_ON_SIGN_DISTANCE - 1.0),
            StreetSignLabelMode::OnSign
        );
        assert_eq!(
            street_sign_label_mode(STREET_SIGN_ON_SIGN_DISTANCE + 1.0),
            StreetSignLabelMode::FloatingBadge
        );
    }

    #[test]
    fn street_sign_label_settings_are_independent_from_poi_settings() {
        let poi = PoiLabelSettings::default();
        let street = StreetSignLabelSettings::default();

        assert!(poi.visible);
        assert!(street.visible);
        assert_ne!(poi.max_distance, street.max_distance);
    }

    #[test]
    fn projects_visible_world_points_to_screen() {
        let mut camera = Flycam::new(1.0);
        camera.position = glam::Vec3::ZERO;
        camera.yaw = -std::f32::consts::FRAC_PI_2;
        camera.pitch = 0.0;

        let screen = project_world_to_screen(
            &camera,
            glam::vec3(0.0, 0.0, -10.0),
            egui::vec2(100.0, 100.0),
        )
        .unwrap();

        assert!((screen.x - 50.0).abs() < 0.001);
        assert!((screen.y - 50.0).abs() < 0.001);
        assert!(
            project_world_to_screen(
                &camera,
                glam::vec3(0.0, 0.0, 10.0),
                egui::vec2(100.0, 100.0)
            )
            .is_none()
        );
    }
}
