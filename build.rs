use std::path::PathBuf;

// https://doc.rust-lang.org/cargo/reference/build-scripts.html
fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // rust
    let rust_dir: PathBuf = ["vendor", "tree-sitter-rust", "src"].iter().collect();

    println!("cargo:rerun-if-changed=vendor/tree-sitter-rust/src/parser.c");
    cc::Build::new()
        .include(&rust_dir)
        .warnings(false)
        .file(rust_dir.join("parser.c"))
        .compile("tree-sitter-rust");

    println!("cargo:rerun-if-changed=vendor/tree-sitter-rust/src/scanner.c");
    cc::Build::new()
        .include(&rust_dir)
        .warnings(false)
        .file(rust_dir.join("scanner.c"))
        .compile("tree_sitter_rust_scanner");

}
