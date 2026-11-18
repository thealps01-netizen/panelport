//! Panelport: open locally hosted web panels as native windows.

pub mod chrome;
pub mod commands;
pub mod discovery;
pub mod docker;
#[cfg(feature = "e2e")]
mod e2e;
pub mod http;
pub mod i18n;
pub mod icons;
pub mod monitor;
pub mod passwords;
pub mod previews;
pub mod selftest;
pub mod settings;
pub mod state;
#[cfg(test)]
pub mod testutil;
pub mod tray;
pub mod urls;
pub mod windows;

use state::{AppState, Paths};
use std::sync::Arc;
use tauri::{Manager, RunEvent};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

/// Every command a bundled page may call. Each needs an `allow-*` entry in
/// `capabilities/local-pages.json` or `capabilities/title-bar.json` and a name in build.rs
/// (unit tests check all three lists).
macro_rules! handler {
    () => {
        tauri::generate_handler![
            $crate::commands::app_info,
            $crate::commands::dismiss_notice,
            $crate::commands::list_panels,
            $crate::commands::save_panels,
            $crate::commands::add_panels,
            $crate::commands::mark_discovery_offered,
            $crate::commands::discover,
            $crate::commands::open_panel,
            $crate::commands::show_view,
            $crate::commands::panel_detail,
            $crate::commands::start_docker,
            $crate::commands::start_containers,
            $crate::commands::check_now,
            $crate::commands::get_autostart,
            $crate::commands::set_autostart,
            $crate::commands::open_log_folder,
            $crate::commands::open_releases,
            $crate::commands::get_prefs,
            $crate::commands::set_prefs,
            $crate::commands::panel_preview,
            $crate::commands::clear_previews,
            $crate::commands::clear_saved_passwords,
            $crate::commands::container_logs,
            $crate::commands::docker_status,
            $crate::commands::win_minimize,
            $crate::commands::win_toggle_maximize,
            $crate::commands::win_is_maximized,
            $crate::commands::win_close,
            $crate::commands::win_start_drag,
            $crate::commands::bar_info,
            $crate::commands::bar_reload,
            $crate::commands::bar_back,
        ]
    };
}
pub(crate) use handler;

/// Second gate after the ACL: a command runs only when the webview's top-level page is a bundled
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
                "refused command {:?} from webview {}: top-level page is not bundled",
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
    let context = tauri::generate_context!();
    if let Some(i) = args.iter().position(|a| a == "--self-test") {
        std::process::exit(selftest::run(args.get(i + 1).map(String::as_str), context));
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
            windows::show_launcher(app, windows::View::Panels);
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
                windows::show_launcher(&handle, windows::View::Panels);
            }
            Ok(())
        })
        .build(context)
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
    //! Proves that panel (remote) origins cannot reach any command, that the title-bar webview of
    //! a panel window reaches only its own commands, and that bundled pages work.
    use super::*;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{get_ipc_response, mock_builder, INVOKE_KEY};
    use tauri::webview::{InvokeRequest, WebviewBuilder};
    use tauri::window::WindowBuilder;
    use tauri::{LogicalPosition, LogicalSize, WebviewUrl, WebviewWindowBuilder};

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
            body: InvokeBody::Json(serde_json::json!({ "id": "pabc", "view": "panels" })),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        }
    }

    /// Commands the title-bar webview of a panel window may call.
    const BAR_COMMANDS: &[&str] = &[
        "bar_info",
        "bar_reload",
        "bar_back",
        "win_minimize",
        "win_toggle_maximize",
        "win_is_maximized",
        "win_close",
        "win_start_drag",
    ];

    /// Commands the launcher and the "not reachable" page may call.
    const LOCAL_COMMANDS: &[&str] = &[
        "app_info",
        "dismiss_notice",
        "list_panels",
        "save_panels",
        "add_panels",
        "mark_discovery_offered",
        "discover",
        "open_panel",
        "show_view",
        "panel_detail",
        "start_docker",
        "start_containers",
        "check_now",
        "get_autostart",
        "set_autostart",
        "open_log_folder",
        "open_releases",
        "get_prefs",
        "set_prefs",
        "panel_preview",
        "clear_previews",
        "clear_saved_passwords",
        "container_logs",
        "docker_status",
        "win_minimize",
        "win_toggle_maximize",
        "win_is_maximized",
        "win_close",
        "win_start_drag",
    ];

    /// Core and plugin commands a page could try.
    const OTHER_COMMANDS: &[&str] = &[
        "plugin:opener|open_url",
        "plugin:opener|open_path",
        "plugin:autostart|enable",
        "plugin:notification|notify",
        "plugin:window|close",
        "plugin:window|start_dragging",
        "plugin:webview|create_webview_window",
        "plugin:webview|set_webview_position",
        "plugin:event|listen",
        "plugin:path|resolve_directory",
    ];

    fn all_commands() -> Vec<&'static str> {
        let mut v: Vec<&str> = LOCAL_COMMANDS
            .iter()
            .chain(BAR_COMMANDS)
            .chain(OTHER_COMMANDS)
            .copied()
            .collect();
        v.sort();
        v.dedup();
        v
    }

    const PANEL_ORIGINS: &[&str] = &[
        "http://localhost:3552/",
        "http://localhost:3552/environments/0/containers",
        "http://127.0.0.1:8081/",
        "http://[::1]:9000/",
        "https://nas.lan:5001/ui/",
        "https://example.com/",
    ];

    /// `get_ipc_response` takes anything that is `AsRef<Webview>`; a plain child webview is not.
    struct Wv<'a>(&'a tauri::Webview<tauri::test::MockRuntime>);
    impl AsRef<tauri::Webview<tauri::test::MockRuntime>> for Wv<'_> {
        fn as_ref(&self) -> &tauri::Webview<tauri::test::MockRuntime> {
            self.0
        }
    }

    fn ipc(
        w: &tauri::Webview<tauri::test::MockRuntime>,
        r: InvokeRequest,
    ) -> Result<tauri::ipc::InvokeResponseBody, serde_json::Value> {
        get_ipc_response(&Wv(w), r)
    }

    fn is_refusal(msg: &str) -> bool {
        msg.contains("not allowed") || msg.contains("remote")
    }

    #[test]
    fn panel_origins_get_no_ipc_but_bundled_pages_do() {
        let state = test_state();
        let app = mock_builder()
            .manage(state)
            .invoke_handler(top_level_guard(tauri::generate_handler![
                commands::app_info,
                commands::list_panels,
                commands::check_now,
                commands::bar_info,
                commands::win_is_maximized
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
        let launcher = launcher.as_ref().clone();

        // Two panel windows as the app builds them: a title-bar webview over the panel webview.
        // One shows the panel itself, the other the bundled "not reachable" page.
        let mut built = Vec::new();
        for (id, start) in [
            (
                "pabc",
                WebviewUrl::External("http://localhost:3552/".parse().unwrap()),
            ),
            ("pdef", WebviewUrl::App("down.html?panel=pdef".into())),
        ] {
            let window = WindowBuilder::new(&app, windows::window_label(id))
                .build()
                .unwrap();
            let bar = window
                .add_child(
                    WebviewBuilder::new(
                        windows::bar_label(id),
                        WebviewUrl::App("titlebar.html".into()),
                    ),
                    LogicalPosition::new(0.0, 0.0),
                    LogicalSize::new(800.0, chrome::BAR_H),
                )
                .unwrap();
            let panel = window
                .add_child(
                    WebviewBuilder::new(windows::panel_label(id), start),
                    LogicalPosition::new(0.0, chrome::BAR_H),
                    LogicalSize::new(800.0, 556.0),
                )
                .unwrap();
            assert_eq!(
                window.webviews().len(),
                2,
                "panel window holds two webviews"
            );
            built.push((bar, panel));
        }
        let (bar, panel) = built.remove(0);
        let (_bar_down, panel_down) = built.remove(0);
        let bar_url = urls::app_url("titlebar.html").to_string();
        let down_url = urls::app_url("down.html?panel=pdef").to_string();
        let launcher_url = urls::app_url("launcher.html").to_string();

        // The launcher and the "not reachable" page may call app commands.
        for (w, url) in [(&launcher, &launcher_url), (&panel_down, &down_url)] {
            for cmd in ["app_info", "check_now", "list_panels"] {
                let ok = ipc(w, request(cmd, url));
                assert!(ok.is_ok(), "{cmd} refused in {}: {:?}", w.label(), ok.err());
            }
        }

        // The title bar may call its own commands and nothing else.
        for cmd in ["bar_info", "win_is_maximized"] {
            let ok = ipc(&bar, request(cmd, &bar_url));
            assert!(ok.is_ok(), "{cmd} refused in the title bar: {:?}", ok.err());
        }
        for cmd in all_commands().iter().filter(|c| !BAR_COMMANDS.contains(c)) {
            let res = ipc(&bar, request(cmd, &bar_url));
            assert!(res.is_err(), "the title bar could call {cmd}");
            assert!(is_refusal(&res.unwrap_err().to_string()));
        }

        // A bundled page framed inside a panel page (local frame, remote top-level page) is
        // refused by the top-level guard, title-bar commands included.
        for cmd in [
            "app_info",
            "list_panels",
            "check_now",
            "bar_info",
            "win_close",
        ] {
            let res = ipc(&panel, request(cmd, &launcher_url));
            assert!(res.is_err(), "framed bundled page could call {cmd}");
            assert!(res.unwrap_err().to_string().contains("not allowed"));
        }

        // Panel origins get nothing, in the panel webview of a two-webview window and anywhere
        // else: no app command, no title-bar command, no core or plugin command.
        let cmds = all_commands();
        let mut checked = 0;
        for w in [&panel, &panel_down, &bar, &launcher] {
            for origin in PANEL_ORIGINS {
                for cmd in &cmds {
                    let res = ipc(w, request(cmd, origin));
                    assert!(res.is_err(), "{origin} could call {cmd} in {}", w.label());
                    let msg = res.unwrap_err().to_string();
                    assert!(
                        is_refusal(&msg),
                        "{origin} -> {cmd}: unexpected refusal: {msg}"
                    );
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 4 * PANEL_ORIGINS.len() * cmds.len());
    }

    fn granted(json: &str) -> (serde_json::Value, Vec<String>) {
        let cap: serde_json::Value = serde_json::from_str(json).unwrap();
        let mut list: Vec<String> = cap["permissions"]
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
        list.sort();
        (cap, list)
    }

    fn sorted(list: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = list.iter().map(|c| c.to_string()).collect();
        v.sort();
        v
    }

    #[test]
    fn handler_lists_match_capabilities() {
        let (local, local_cmds) = granted(include_str!("../capabilities/local-pages.json"));
        let (bar, bar_cmds) = granted(include_str!("../capabilities/title-bar.json"));
        assert_eq!(local_cmds, sorted(LOCAL_COMMANDS));
        assert_eq!(bar_cmds, sorted(BAR_COMMANDS));
        for cap in [&local, &bar] {
            assert!(
                cap.get("remote").is_none(),
                "a capability must not have a remote section"
            );
        }
        // Who gets what: the launcher window and panel content webviews get the app commands;
        // only title-bar webviews get the title-bar capability. Panel windows themselves are in
        // neither list, so a webview in them is matched by its own label only.
        assert_eq!(local["windows"], serde_json::json!(["launcher"]));
        assert_eq!(local["webviews"], serde_json::json!(["panel-*"]));
        assert!(bar.get("windows").is_none());
        assert_eq!(bar["webviews"], serde_json::json!(["bar-*"]));

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
        let mut all: Vec<String> = sorted(LOCAL_COMMANDS);
        all.extend(sorted(BAR_COMMANDS));
        all.sort();
        all.dedup();
        assert_eq!(built_sorted, all);
    }
}
