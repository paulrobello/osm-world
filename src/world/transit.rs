use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitKind {
    BusStop,
    TrainStation,
    Platform,
    StopPosition,
}

pub fn transit_kind(tags: &HashMap<String, String>) -> Option<TransitKind> {
    if tags.get("highway").map(String::as_str) == Some("bus_stop") {
        return Some(TransitKind::BusStop);
    }
    if matches!(
        tags.get("railway").map(String::as_str),
        Some("station" | "halt" | "tram_stop" | "subway_entrance")
    ) {
        return Some(TransitKind::TrainStation);
    }
    match tags.get("public_transport").map(String::as_str) {
        Some("platform") => Some(TransitKind::Platform),
        Some("station") => Some(TransitKind::TrainStation),
        Some("stop_position") => Some(TransitKind::StopPosition),
        _ => None,
    }
}

pub fn transit_label(tags: &HashMap<String, String>) -> Option<String> {
    if let Some(name) = tags
        .get("name")
        .map(String::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        return Some(name.to_string());
    }
    Some(
        match transit_kind(tags)? {
            TransitKind::BusStop => "Bus Stop",
            TransitKind::TrainStation => "Train Station",
            TransitKind::Platform => "Transit Platform",
            TransitKind::StopPosition => "Transit Stop",
        }
        .to_string(),
    )
}

pub fn is_transit_route(tags: &HashMap<String, String>) -> bool {
    tags.get("type").map(String::as_str) == Some("route")
        && matches!(
            tags.get("route").map(String::as_str),
            Some("bus" | "trolleybus" | "tram" | "train" | "subway" | "light_rail")
        )
}

pub fn transit_route_label(tags: &HashMap<String, String>) -> String {
    if let Some(name) = tags
        .get("name")
        .map(String::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        return name.to_string();
    }
    match tags.get("route").map(String::as_str) {
        Some("bus" | "trolleybus") => "Bus Route",
        Some("tram") => "Tram Route",
        Some("train") => "Train Route",
        Some("subway") => "Subway Route",
        Some("light_rail") => "Light Rail Route",
        _ => "Transit Route",
    }
    .to_string()
}

pub fn transit_route_color(tags: &HashMap<String, String>) -> [f32; 3] {
    match tags.get("route").map(String::as_str) {
        Some("bus" | "trolleybus") => [0.05, 0.55, 1.0],
        Some("tram" | "light_rail") => [0.10, 0.85, 0.65],
        Some("train" | "subway") => [1.0, 0.62, 0.12],
        _ => [0.20, 0.80, 0.95],
    }
}

pub fn transit_color(kind: TransitKind) -> [f32; 3] {
    match kind {
        TransitKind::BusStop => [0.08, 0.58, 1.0],
        TransitKind::TrainStation => [0.95, 0.62, 0.12],
        TransitKind::Platform => [0.20, 0.85, 0.95],
        TransitKind::StopPosition => [0.55, 0.85, 1.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn classifies_common_public_transit_point_tags() {
        assert_eq!(
            transit_kind(&tags(&[("highway", "bus_stop")])),
            Some(TransitKind::BusStop)
        );
        assert_eq!(
            transit_kind(&tags(&[("railway", "station")])),
            Some(TransitKind::TrainStation)
        );
        assert_eq!(
            transit_kind(&tags(&[("public_transport", "platform")])),
            Some(TransitKind::Platform)
        );
    }

    #[test]
    fn transit_routes_include_bus_tram_and_train_relations() {
        assert!(is_transit_route(&tags(&[
            ("type", "route"),
            ("route", "bus")
        ])));
        assert!(is_transit_route(&tags(&[
            ("type", "route"),
            ("route", "tram")
        ])));
        assert!(is_transit_route(&tags(&[
            ("type", "route"),
            ("route", "train")
        ])));
        assert!(!is_transit_route(&tags(&[
            ("type", "route"),
            ("route", "road")
        ])));
    }

    #[test]
    fn transit_label_uses_name_or_kind_fallback() {
        assert_eq!(
            transit_label(&tags(&[
                ("highway", "bus_stop"),
                ("name", "Downtown Terminal")
            ]))
            .as_deref(),
            Some("Downtown Terminal")
        );
        assert_eq!(
            transit_label(&tags(&[("railway", "station")])).as_deref(),
            Some("Train Station")
        );
    }
}
