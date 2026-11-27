fn main() {
    // Listing the app's own commands makes each one need an explicit permission.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["api", "save_file"])),
    )
    .expect("failed to run tauri-build");
}
