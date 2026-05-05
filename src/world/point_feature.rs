use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointFeatureKind {
    Tree,
    Landmark,
    Nature,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointFeatureStyle {
    pub kind: PointFeatureKind,
}

pub fn point_feature_style(tags: &HashMap<String, String>) -> Option<PointFeatureStyle> {
    if tags.get("natural").map(String::as_str) == Some("tree") {
        return Some(PointFeatureStyle {
            kind: PointFeatureKind::Tree,
        });
    }
    if matches!(
        tags.get("natural").map(String::as_str),
        Some("peak" | "rock" | "spring")
    ) {
        return Some(PointFeatureStyle {
            kind: PointFeatureKind::Nature,
        });
    }
    if matches!(
        tags.get("tourism").map(String::as_str),
        Some("attraction" | "viewpoint" | "artwork")
    ) || tags.contains_key("historic")
        || matches!(
            tags.get("man_made").map(String::as_str),
            Some("tower" | "water_tower" | "chimney")
        )
    {
        return Some(PointFeatureStyle {
            kind: PointFeatureKind::Landmark,
        });
    }
    None
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
    fn classifies_natural_tree() {
        let style = point_feature_style(&tags(&[("natural", "tree")])).unwrap();
        assert_eq!(style.kind, PointFeatureKind::Tree);
    }

    #[test]
    fn classifies_natural_peak_as_nature() {
        let style = point_feature_style(&tags(&[("natural", "peak")])).unwrap();
        assert_eq!(style.kind, PointFeatureKind::Nature);
    }

    #[test]
    fn classifies_historic_monument_as_landmark() {
        let style = point_feature_style(&tags(&[("historic", "monument")])).unwrap();
        assert_eq!(style.kind, PointFeatureKind::Landmark);
    }

    #[test]
    fn ignores_unrendered_tags() {
        assert!(point_feature_style(&tags(&[("amenity", "bench")])).is_none());
    }
}
