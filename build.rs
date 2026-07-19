//! Build script for the `osm-world` package.
//!
//! Cross-checks the WGSL feature-type constants in `shaders/features.wgsl`
//! against their Rust source of truth in `src/mesh.rs::feature`. A drift
//! between the two would silently desync the shader's per-feature material /
//! overlay / discard branches from the geometry the CPU emits, so we fail the
//! build at compile time rather than letting it reach the GPU. The same
//! invariant is re-checked at test time by `tests/shader_source_test.rs`.
//!
//! No code is generated; this script only asserts. `cargo` therefore does not
//! need to re-run it when `OUT_DIR` changes, only when `src/mesh.rs` or
//! `shaders/features.wgsl` change (declared via `cargo:rerun-if-changed`).

use std::collections::BTreeMap;

const MESH_RS: &str = "src/mesh.rs";
const FEATURES_WGSL: &str = "shaders/features.wgsl";

fn main() {
    println!("cargo:rerun-if-changed={MESH_RS}");
    println!("cargo:rerun-if-changed={FEATURES_WGSL}");

    let mesh = std::fs::read_to_string(MESH_RS).expect("build.rs: could not read src/mesh.rs");
    let features = std::fs::read_to_string(FEATURES_WGSL)
        .expect("build.rs: could not read shaders/features.wgsl");

    let rust_consts = parse_feature_module(&mesh)
        .expect("build.rs: could not parse `pub mod feature { ... }` from src/mesh.rs");
    let wgsl_consts = parse_features_wgsl(&features).expect(
        "build.rs: could not parse `const FEATURE_*: f32 = ...;` from shaders/features.wgsl",
    );

    let mut mismatches: Vec<String> = Vec::new();
    for (name, rust_value) in &rust_consts {
        match wgsl_consts.get(name) {
            Some(wgsl_value) if wgsl_value == rust_value => {
                // OK — values agree.
            }
            Some(wgsl_value) => {
                mismatches.push(format!(
                    "  {name}: rust = {rust_value}, wgsl = {wgsl_value}"
                ));
            }
            None => {
                mismatches.push(format!(
                    "  {name}: present in src/mesh.rs (={rust_value}) but missing from shaders/features.wgsl"
                ));
            }
        }
    }
    for (name, wgsl_value) in &wgsl_consts {
        if !rust_consts.contains_key(name) {
            mismatches.push(format!(
                "  {name}: present in shaders/features.wgsl (={wgsl_value}) but missing from src/mesh.rs"
            ));
        }
    }

    if !mismatches.is_empty() {
        panic!(
            "Feature-type constants drifted between src/mesh.rs and shaders/features.wgsl.\n\
             Fix one side to match the other:\n{}\n\
             Both files document the slot-reservation convention; see also ARC-006 in AUDIT.md.",
            mismatches.join("\n")
        );
    }
}

/// Extract `pub const NAME: f32 = VALUE;` declarations from the
/// `pub mod feature { ... }` block in `src/mesh.rs`.
fn parse_feature_module(src: &str) -> Option<BTreeMap<String, String>> {
    let start = src.find("pub mod feature {")?;
    let end = src[start..].find("}\n")?;
    let block = &src[start..start + end];

    let mut out = BTreeMap::new();
    for line in block.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("pub const ") else {
            continue;
        };
        let Some(rest) = rest.strip_suffix(';') else {
            continue;
        };
        // rest looks like "NAME: f32 = VALUE"
        let Some((name, value)) = rest.split_once(": f32 = ") else {
            continue;
        };
        out.insert(name.trim().to_string(), value.trim().to_string());
    }
    Some(out)
}

/// Extract `const FEATURE_NAME: f32 = VALUE;` declarations from
/// `shaders/features.wgsl`, returning them keyed by `NAME` (FEATURE_ prefix
/// stripped) so they line up with the Rust `feature::NAME` constants.
fn parse_features_wgsl(src: &str) -> Option<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for line in src.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("const FEATURE_") else {
            continue;
        };
        let Some(rest) = rest.strip_suffix(';') else {
            continue;
        };
        // rest looks like "NAME: f32 = VALUE"
        let Some((name, value)) = rest.split_once(": f32 = ") else {
            continue;
        };
        out.insert(name.trim().to_string(), value.trim().to_string());
    }
    Some(out)
}
