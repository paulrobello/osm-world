use std::collections::HashMap;

pub fn address_label_text(tags: &HashMap<String, String>) -> Option<String> {
    tag_value(tags, "addr:housenumber")
}

pub fn address_full_text(tags: &HashMap<String, String>) -> Option<String> {
    let house_number = tag_value(tags, "addr:housenumber")?;
    match tag_value(tags, "addr:street") {
        Some(street) => Some(format!("{house_number} {street}")),
        None => Some(house_number),
    }
}

fn tag_value(tags: &HashMap<String, String>, key: &str) -> Option<String> {
    tags.get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
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
    fn address_label_prefers_house_number_for_close_range_labels() {
        let label = address_label_text(&tags(&[
            ("addr:housenumber", " 1420 "),
            ("addr:street", "Main Street"),
        ]));

        assert_eq!(label.as_deref(), Some("1420"));
    }

    #[test]
    fn address_full_text_combines_house_number_and_street_for_inspection() {
        let label = address_full_text(&tags(&[
            ("addr:housenumber", "1420"),
            ("addr:street", "Main Street"),
        ]));

        assert_eq!(label.as_deref(), Some("1420 Main Street"));
    }
}
