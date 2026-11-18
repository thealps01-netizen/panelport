// Registers the app's commands with Tauri's ACL. With an app manifest, every command needs an
// explicit `allow-<command>` permission; capabilities/local-pages.json grants them to bundled
// pages only (no `remote` section), so panel origins get none.
const COMMANDS: &[&str] = &[
    "app_info",
    "dismiss_notice",
    "list_panels",
    "save_panels",
    "add_panels",
    "mark_discovery_offered",
    "discover",
    "open_panel",
    "show_settings",
    "show_launcher",
    "panel_detail",
    "start_docker",
    "start_containers",
    "check_now",
    "get_autostart",
    "set_autostart",
    "open_log_folder",
    "open_releases",
];

fn main() {
    // Tauri normally embeds its Windows manifest only into the app binary, so test binaries
    // miss the Common Controls v6 dependency and fail to start (STATUS_ENTRYPOINT_NOT_FOUND).
    // Instead, the same manifest is embedded into every binary through linker arguments.
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let mut windows = tauri_build::WindowsAttributes::new();
    if msvc {
        windows = tauri_build::WindowsAttributes::new_without_app_manifest();
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed=windows-app-manifest.xml");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .windows_attributes(windows)
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("tauri build step failed");
}
