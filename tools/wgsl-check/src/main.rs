//! Validates the sky's shader as the renderer will compile it: `cargo run --manifest-path tools/wgsl-check/Cargo.toml`.

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../big_nukes/src/sky.rs");
    let source = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let open = "const SHADER: &str = r#\"";
    let start = source.find(open).expect("sky.rs holds the shader") + open.len();
    let end = start + source[start..].find("\"#;").expect("the shader ends");
    let wgsl = &source[start..end];
    let module = naga::front::wgsl::parse_str(wgsl).unwrap_or_else(|e| panic!("{}", e.emit_to_string(wgsl)));
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
        .validate(&module)
        .unwrap_or_else(|e| panic!("{e:?}"));
    for entry in ["vs", "fs"] {
        assert!(module.entry_points.iter().any(|e| e.name == entry), "{entry}");
    }
    println!("the sky's WGSL validates");
}
