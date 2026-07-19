//! Integration test for the public `par_osm_rust::osm::parse_osm_xml_str`
//! parser. Lives in the library crate (ARC-008) so the binary crate's
//! `src/server/tests.rs` is no longer the only place library behavior is
//! exercised.

use par_osm_rust::osm::parse_osm_xml_str;

#[test]
fn parses_nodes_ways_and_bounds_from_xml_string() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <bounds minlat="38.0" minlon="-121.0" maxlat="38.001" maxlon="-120.999"/>
  <node id="1" lat="38.0" lon="-121.0"/>
  <node id="2" lat="38.001" lon="-121.0"/>
  <node id="3" lat="38.001" lon="-120.999"/>
  <node id="4" lat="38.0" lon="-120.999"/>
  <way id="100">
    <nd ref="1"/>
    <nd ref="2"/>
    <nd ref="3"/>
    <nd ref="4"/>
    <nd ref="1"/>
    <tag k="building" v="yes"/>
  </way>
</osm>"#;

    let data = parse_osm_xml_str(xml).expect("well-formed OSM XML should parse");

    assert_eq!(data.nodes.len(), 4);
    assert_eq!(
        data.nodes.get(&1).map(|node| (node.lat, node.lon)),
        Some((38.0, -121.0))
    );
    assert_eq!(data.ways.len(), 1);
    let way = &data.ways[0];
    assert_eq!(way.node_refs, vec![1, 2, 3, 4, 1]);
    assert_eq!(way.tags.get("building").map(String::as_str), Some("yes"));
    assert_eq!(data.bounds, Some((38.0, -121.0, 38.001, -120.999)));
}

#[test]
fn parse_osm_xml_str_collects_poi_nodes_with_tags() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<osm version="0.6">
  <node id="1" lat="38.0005" lon="-120.9995">
    <tag k="amenity" v="cafe"/>
    <tag k="name" v="Test Cafe"/>
  </node>
</osm>"#;

    let data = parse_osm_xml_str(xml).expect("tagged node should parse");

    assert_eq!(data.poi_nodes.len(), 1);
    let poi = &data.poi_nodes[0];
    assert_eq!(poi.lat, 38.0005);
    assert_eq!(poi.lon, -120.9995);
    assert_eq!(poi.tags.get("name").map(String::as_str), Some("Test Cafe"));
}
