fn main() {
    // Baked into release builds (state.rs, DEFAULT_CATALOG_URL).
    println!("cargo:rerun-if-env-changed=PORTSIDE_DEFAULT_CATALOG_URL");
    tauri_build::build();
}
