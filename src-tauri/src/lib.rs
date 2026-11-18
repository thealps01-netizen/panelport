//! Panelport: open locally hosted web panels as native windows.

pub mod commands;
pub mod discovery;
pub mod docker;
#[cfg(feature = "e2e")]
mod e2e;
pub mod http;
pub mod i18n;
pub mod icons;
pub mod monitor;
pub mod selftest;
pub mod settings;
pub mod state;
pub mod tray;
pub mod urls;
pub mod windows;

use state::{AppState, Paths};
use std::sync::Arc;
use tauri::{Manager, RunEvent};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

/// Every command the bundled pages may call. Each needs an `allow-*` entry in
/// `capabilities/local-pages.json` and a name in build.rs (a unit test checks both).
macro_rules! handler {
    () => {
        tauri::generate_handler![
            commands::app_info,
            commands::dismiss_notice,
            commands::list_panels,
            commands::save_panels,
            commands::add_panels,
            commands::mark_discovery_offered,
            commands::discover,
            commands::open_panel,
            commands::show_settings,
            commands::show_launcher,
            commands::panel_detail,
            commands::start_docker,
            commands::start_containers,
            commands::check_now,
            commands::get_autostart,
            commands::set_autostart,
            commands::open_log_folder,
            commands::open_releases,
        ]
    };
}

/// Second gate after the ACL: a command runs only when the window's top-level page is a bundled
/// page. The ACL checks the calling frame, so this also stops a bundled page that a panel page
/// managed to load inside a frame.
pub fn top_level_guard<R: tauri::Runtime, F>(
    inner: F,
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static
where
    F: Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static,
{
    move |invoke| {
        let webview = invoke.message.webview();
        let local = webview
            .url()
            .map(|u| urls::is_app_local(&u))
            .unwrap_or(false);
        if !local {
            log::warn!(
                "refused command {:?} from window {}: top-level page is not bundled",
                invoke.message.command(),
                webview.label()
            );
            invoke
                .resolver
                .reject("not allowed: Panelport commands only run on Panelport's own pages");
            return true;
        }
        inner(invoke)
    }
}

fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}");
        default(info);
    }));
}

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--self-test") {
        std::process::exit(selftest::run(args.get(i + 1).map(String::as_str)));
    }
    let autostarted = args.iter().any(|a| a == "--autostart");
    let lang = i18n::detect();
    // Test profile: separate settings, logs and WebView2 data, and no single-instance lock, so a
    // check of the installed build never reads or changes the user's own Panelport data.
    let profile = std::env::var_os("PANELPORT_PROFILE_DIR")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute());

    let log_target = match &profile {
        Some(dir) => TargetKind::Folder {
            path: dir.join("logs"),
            file_name: Some("panelport".into()),
        },
        None => TargetKind::LogDir {
            file_name: Some("panelport".into()),
        },
    };
    let log_plugin = tauri_plugin_log::Builder::new()
        .clear_targets()
        .target(Target::new(log_target))
        .target(Target::new(TargetKind::Stderr))
        .max_file_size(1_000_000)
        .rotation_strategy(RotationStrategy::KeepSome(3))
        .level(log::LevelFilter::Info)
        .build();

    let mut builder = tauri::Builder::default();
    if profile.is_none() {
        // Must be the first plugin: a second launch only focuses the running instance.
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            windows::show_launcher(app, false);
        }));
    }
    let app = builder
        .plugin(log_plugin)
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .invoke_handler(top_level_guard(handler!()))
        .on_window_event(windows::on_window_event)
        .setup(move |app| {
            install_panic_hook();
            let (data, logs) = match &profile {
                Some(dir) => (dir.join("data"), dir.join("logs")),
                None => (app.path().app_local_data_dir()?, app.path().app_log_dir()?),
            };
            let mut paths = Paths::new(data, logs);
            if let Some(dir) = &profile {
                paths.webview = Some(dir.join("webview"));
                log::warn!("test profile in use: {}", dir.display());
            }
            log::info!(
                "Panelport {} starting (lang {:?}, data {}, autostart {})",
                env!("CARGO_PKG_VERSION"),
                lang,
                paths.data.display(),
                autostarted
            );
            let loaded = settings::load(&paths.settings);
            match &loaded.notice {
                settings::LoadNotice::Corrupt { moved_to, error } => {
                    log::error!(
                        "settings unreadable ({error}); moved to {}",
                        moved_to.display()
                    )
                }
                settings::LoadNotice::Upgraded { from, backup } => {
                    log::info!(
                        "settings upgraded from schema {from}; backup {}",
                        backup.display()
                    )
                }
                settings::LoadNotice::TooNew { found } => {
                    log::warn!("settings schema {found} is newer than this version; read-only")
                }
                _ => {}
            }
            let state = Arc::new(AppState::new(paths, lang, loaded));
            app.manage(state);
            let handle = app.handle().clone();
            tray::create(&handle)?;
            monitor::spawn(handle.clone());
            #[cfg(feature = "e2e")]
            e2e::spawn(handle.clone());
            if !autostarted {
                windows::show_launcher(&handle, false);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Panelport failed to start");

    app.run(|app, event| match event {
        // Closing the last window keeps Panelport in the tray; only Quit (exit code set) ends it.
        RunEvent::ExitRequested {
            code: None, api, ..
        } => api.prevent_exit(),
        RunEvent::Exit => {
            if let Some(state) = app.try_state::<Arc<AppState>>() {
                if let Err(e) = state.flush() {
                    log::error!("saving settings on exit failed: {e}");
                }
            }
            log::info!("Panelport exiting");
        }
        _ => {}
    });
}

#[cfg(test)]
mod ipc_security_tests {
    //! Proves that panel (remote) origins cannot reach any command, while bundled pages can.
    use super::*;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{get_ipc_response, mock_builder, INVOKE_KEY};
    use tauri::webview::InvokeRequest;
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    fn test_state() -> Arc<AppState> {
        let dir = std::env::temp_dir().join(format!("panelport-acl-{}", settings::new_id()));
        let paths = Paths::new(dir.join("data"), dir.join("logs"));
        let loaded = settings::load(&paths.settings);
        Arc::new(AppState::new(paths, i18n::Lang::En, loaded))
    }

    fn request(cmd: &str, url: &str) -> InvokeRequest {
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: url.parse().unwrap(),
            body: InvokeBody::Json(
                serde_json::json!({ "id": "pabc", "find": false, "enabled": true }),
            ),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        }
    }

    /// Every command the app registers, plus core and plugin commands a page could try.
    const ALL_COMMANDS: &[&str] = &[
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
        "plugin:opener|open_url",
        "plugin:opener|open_path",
        "plugin:autostart|enable",
        "plugin:notification|notify",
        "plugin:window|close",
        "plugin:webview|create_webview_window",
        "plugin:event|listen",
        "plugin:path|resolve_directory",
    ];

    const PANEL_ORIGINS: &[&str] = &[
        "http://localhost:3552/",
        "http://localhost:3552/environments/0/containers",
        "http://127.0.0.1:8081/",
        "http://[::1]:9000/",
        "https://nas.lan:5001/ui/",
        "https://example.com/",
    ];

    #[test]
    fn panel_origins_get_no_ipc_but_bundled_pages_do() {
        let state = test_state();
        let app = mock_builder()
            .manage(state)
            .invoke_handler(top_level_guard(tauri::generate_handler![
                commands::app_info,
                commands::list_panels,
                commands::check_now
            ]))
            .build(tauri::generate_context!(test = true))
            .expect("mock app");

        let launcher = WebviewWindowBuilder::new(
            &app,
            windows::LAUNCHER,
            WebviewUrl::App("launcher.html".into()),
        )
        .build()
        .unwrap();
        // A panel window while its panel is down shows the bundled status page...
        let panel_down = WebviewWindowBuilder::new(
            &app,
            windows::panel_label("pabc"),
            WebviewUrl::App("down.html?panel=pabc".into()),
        )
        .build()
        .unwrap();
        // ...and the panel itself while it is up.
        let panel = WebviewWindowBuilder::new(
            &app,
            windows::panel_label("pdef"),
            WebviewUrl::External("http://localhost:3552/".parse().unwrap()),
        )
        .build()
        .unwrap();

        // Bundled pages may call commands.
        for (w, page) in [
            (&launcher, "launcher.html"),
            (&panel_down, "down.html?panel=pabc"),
        ] {
            let local = urls::app_url(page).to_string();
            let ok = get_ipc_response(w, request("app_info", &local));
            assert!(
                ok.is_ok(),
                "bundled page in {} was refused: {:?}",
                w.label(),
                ok.err()
            );
            let ok = get_ipc_response(w, request("check_now", &local));
            assert!(
                ok.is_ok(),
                "bundled page in {} was refused: {:?}",
                w.label(),
                ok.err()
            );
        }

        // A bundled page framed inside a panel page (call from a local frame, top-level page
        // remote) is refused by the top-level guard.
        for cmd in ["app_info", "list_panels", "check_now"] {
            let res = get_ipc_response(
                &panel,
                request(cmd, urls::app_url("launcher.html").as_ref()),
            );
            assert!(res.is_err(), "framed bundled page could call {cmd}");
            assert!(res.unwrap_err().to_string().contains("not allowed"));
        }

        // Panel origins, in panel windows and anywhere else, get nothing.
        let mut checked = 0;
        for w in [&panel, &panel_down, &launcher] {
            for origin in PANEL_ORIGINS {
                for cmd in ALL_COMMANDS {
                    let res = get_ipc_response(w, request(cmd, origin));
                    assert!(res.is_err(), "{origin} could call {cmd} in {}", w.label());
                    let msg = res.unwrap_err().to_string();
                    assert!(
                        msg.contains("not allowed") || msg.contains("remote"),
                        "{origin} -> {cmd}: refused for an unexpected reason: {msg}"
                    );
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 3 * PANEL_ORIGINS.len() * ALL_COMMANDS.len());
    }

    #[test]
    fn handler_lists_match_capability() {
        // Every registered command must be granted in the capability, and nothing else.
        let cap: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/local-pages.json")).unwrap();
        let mut granted: Vec<String> = cap["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                p.as_str()
                    .unwrap()
                    .trim_start_matches("allow-")
                    .replace('-', "_")
            })
            .collect();
        granted.sort();
        let mut cmds: Vec<String> = ALL_COMMANDS
            .iter()
            .filter(|c| !c.contains(':'))
            .map(|c| c.to_string())
            .collect();
        cmds.sort();
        assert_eq!(granted, cmds);
        assert!(
            cap.get("remote").is_none(),
            "the capability must not have a remote section"
        );
        let built: Vec<String> = include_str!("../build.rs")
            .split("const COMMANDS: &[&str] = &[")
            .nth(1)
            .unwrap()
            .split(']')
            .next()
            .unwrap()
            .split('"')
            .skip(1)
            .step_by(2)
            .map(String::from)
            .collect();
        let mut built_sorted = built.clone();
        built_sorted.sort();
        assert_eq!(built_sorted, cmds);
    }
}
