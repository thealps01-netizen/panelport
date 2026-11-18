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
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("tauri build step failed");
}
