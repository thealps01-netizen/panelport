//! `panelport --self-test <out.json>`: a headless check used by the release workflow on a clean
//! machine. It opens no window and touches no user settings. Exit code 0 means every required
//! check passed; the JSON file lists each check.

use crate::{discovery, docker, http, settings, urls};
use serde_json::{json, Value};
use std::time::Duration;

fn check(name: &str, ok: bool, detail: impl Into<Value>, required: bool) -> Value {
    json!({ "name": name, "ok": ok, "required": required, "detail": detail.into() })
}

pub fn run(out: Option<&str>, context: tauri::Context<tauri::Wry>) -> i32 {
    let mut checks = Vec::new();

    // Shipped files next to the executable (NSIS installs resources into the app folder).
    let exe = std::env::current_exe().ok();
    let dir = exe
        .as_ref()
        .and_then(|e| e.parent().map(|p| p.to_path_buf()));
    let licenses = dir.as_ref().map(|d| d.join("THIRD_PARTY_LICENSES.txt"));
    let lic_ok = licenses
        .as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .is_some_and(|m| m.len() > 10_000);
    checks.push(check(
        "third_party_licenses_present",
        lic_ok,
        licenses.map(|p| p.display().to_string()),
        cfg!(windows),
    ));

    // Settings write/read/upgrade in a throwaway folder.
    let tmp = std::env::temp_dir().join(format!("panelport-selftest-{}", settings::new_id()));
    let path = tmp.join("settings.json");
    let roundtrip = (|| -> Result<(), String> {
        std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
        std::fs::write(
            &path,
            r#"{"schema":0,"panels":[{"name":"T","url":"http://localhost:1/"}]}"#,
        )
        .map_err(|e| e.to_string())?;
        let l = settings::load(&path);
        if !matches!(l.notice, settings::LoadNotice::Upgraded { .. })
            || l.settings.panels.len() != 1
        {
            return Err(format!("upgrade failed: {:?}", l.notice));
        }
        let again = settings::load(&path);
        (again.settings == l.settings)
            .then_some(())
            .ok_or("reload differs".to_string())
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    checks.push(check(
        "settings_roundtrip",
        roundtrip.is_ok(),
        roundtrip.err().unwrap_or_default(),
        true,
    ));

    // Navigation policy sanity.
    let panel = url::Url::parse("http://localhost:3552/").unwrap();
    let nav_ok = urls::decide(
        &url::Url::parse("http://localhost:3552/x").unwrap(),
        Some(&panel),
    ) == urls::Nav::Allow
        && matches!(
            urls::decide(
                &url::Url::parse("https://example.com/").unwrap(),
                Some(&panel)
            ),
            urls::Nav::External(_)
        )
        && urls::decide(&url::Url::parse("file:///c:/").unwrap(), Some(&panel)) == urls::Nav::Deny;
    checks.push(check("navigation_policy", nav_ok, "", true));

    // A real HTTP health check against a server this process starts.
    let http_ok = (|| -> Result<String, String> {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let port = l.local_addr().map_err(|e| e.to_string())?.port();
        std::thread::spawn(move || {
            if let Some(Ok(mut s)) = l.incoming().next() {
                let mut b = [0u8; 512];
                let _ = s.read(&mut b);
                let _ = s.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 22\r\nConnection: close\r\n\r\n<!doctype html><p>ok\n",
                );
            }
        });
        let c = http::check(
            &url::Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap(),
            Duration::from_secs(5),
            true,
        );
        (c.health == http::Health::Up)
            .then(|| c.detail.clone())
            .ok_or(c.detail)
    })();
    checks.push(check(
        "http_health_check",
        http_ok.is_ok(),
        http_ok.unwrap_or_else(|e| e),
        true,
    ));

    // Informational: what this machine has.
    let cli = docker::find_cli();
    let engine = docker::engine(cli.as_ref());
    checks.push(check(
        "docker",
        true,
        json!({ "cli": cli.map(|p| p.display().to_string()), "engine": engine }),
        false,
    ));
    let ports = discovery::listening_ports();
    checks.push(check(
        "listening_ports_scan",
        true,
        json!(ports.len()),
        false,
    ));

    // A real panel window (two webviews): IPC refused to the panel, password saving on, and the
    // caption buttons answering the system's hit test.
    checks.extend(panel_window_checks(context));

    let ok = checks
        .iter()
        .all(|c| c["ok"].as_bool() == Some(true) || c["required"].as_bool() == Some(false));
    let report = json!({
        "app": "Panelport",
        "version": env!("CARGO_PKG_VERSION"),
        "ok": ok,
        "checks": checks,
    });
    let text = serde_json::to_string_pretty(&report).unwrap_or_default();
    match out {
        Some(p) => {
            if let Err(e) = std::fs::write(p, &text) {
                eprintln!("could not write {p}: {e}");
                return 2;
            }
        }
        None => println!("{text}"),
    }
    if ok {
        0
    } else {
        1
    }
}

/// Opens a real panel window in a hidden, throwaway Tauri app: the same two-webview window,
/// capabilities, IPC guard and password setting as the app uses, with a temporary data folder and
/// a local test page as the panel. Checks that the window holds two webviews, that the panel page
/// (a remote origin) is refused every command it tries, that password saving reads back as on,
/// and (Windows) that the caption buttons answer the system's hit test. Touches no user data.
fn panel_window_checks(context: tauri::Context<tauri::Wry>) -> Vec<Value> {
    use crate::state::{AppState, Paths};
    use std::sync::{Arc, Mutex};

    let dir =
        std::env::temp_dir().join(format!("panelport-selftest-window-{}", settings::new_id()));
    let failed = |why: String| {
        vec![
            check(
                "panel_window_two_webviews",
                false,
                json!({ "error": why.clone() }),
                true,
            ),
            check(
                "panel_webview_no_ipc",
                false,
                json!({ "error": why.clone() }),
                true,
            ),
            check(
                "panel_password_autosave",
                false,
                json!({ "error": why }),
                cfg!(windows),
            ),
        ]
    };
    // The panel: a page that tries a title-bar command and an app command and writes the
    // outcome into its own address (the only channel a page without IPC has).
    let page = "<!doctype html><title>Panelport self-test panel</title><script>\
        const t=(c)=>window.__TAURI_INTERNALS__?window.__TAURI_INTERNALS__.invoke(c).then(()=>'ALLOWED',()=>'denied'):Promise.resolve('no-bridge');\
        Promise.all([t('bar_info'),t('app_info'),t('win_close')]).then(r=>{location.hash='ipc='+r.join(',')});\
        </script>";
    let port = match serve_page(page) {
        Ok(p) => p,
        Err(e) => return failed(format!("test page server: {e}")),
    };
    let url = match url::Url::parse(&format!("http://127.0.0.1:{port}/")) {
        Ok(u) => u,
        Err(e) => return failed(e.to_string()),
    };
    let mut paths = Paths::new(dir.join("data"), dir.join("logs"));
    paths.webview = Some(dir.join("webview"));
    let loaded = settings::load(&paths.settings);
    let state = Arc::new(AppState::new(paths, crate::i18n::Lang::En, loaded));
    let _ = state.update(false, |s| {
        s.panels.push(settings::Panel {
            id: "pselftest".into(),
            name: "Self-test".into(),
            url: url.to_string(),
            health_path: "/".into(),
            icon: settings::IconPref::Monogram,
            geometry: None,
        });
        s.prefs.keep_previews = false;
    });

    let result: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
    let slot = result.clone();
    let app = tauri::Builder::default()
        .manage(state)
        .invoke_handler(crate::top_level_guard(crate::handler!()))
        .setup(move |app| {
            let handle = app.handle().clone();
            let (window, content) = crate::windows::build_panel_window(
                &handle,
                "pselftest",
                "Self-test",
                tauri::WebviewUrl::External(url.clone()),
                url.clone(),
                false,
            )
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            crate::passwords::enable(&content);
            std::thread::spawn(move || {
                // Wait for the panel page to report its IPC attempts.
                let mut ipc = String::from("no report");
                for _ in 0..100 {
                    std::thread::sleep(Duration::from_millis(200));
                    if let Some(frag) = content
                        .url()
                        .ok()
                        .and_then(|u| u.fragment().map(String::from))
                    {
                        if let Some(r) = frag.strip_prefix("ipc=") {
                            ipc = r.to_string();
                            break;
                        }
                    }
                }
                let labels: Vec<String> = window
                    .webviews()
                    .iter()
                    .map(|w| w.label().to_string())
                    .collect();
                #[cfg(windows)]
                let pw = crate::passwords::read_blocking(&content);
                #[cfg(not(windows))]
                let pw: Result<crate::passwords::AutofillState, String> =
                    Err("WebView2 only; not applicable on this OS".into());
                #[cfg(windows)]
                let hits = crate::chrome::probe(&window);
                #[cfg(not(windows))]
                let hits: Option<[isize; 3]> = None;
                *slot.lock().unwrap_or_else(|p| p.into_inner()) = Some(json!({
                    "webviews": labels,
                    "ipc": ipc,
                    "password": pw.map(|s| json!({
                        "IsPasswordAutosaveEnabled": s.password_autosave,
                        "IsGeneralAutofillEnabled": s.general_autofill,
                    })).unwrap_or_else(|e| json!({ "error": e })),
                    "hits": hits,
                }));
                handle.exit(0);
            });
            Ok(())
        })
        .build(context);
    match app {
        Ok(app) => {
            app.run_return(|_, _| {});
        }
        Err(e) => return failed(format!("tauri: {e}")),
    }
    let _ = std::fs::remove_dir_all(&dir);
    let Some(r) = result.lock().unwrap_or_else(|p| p.into_inner()).take() else {
        return failed("the check window never reported".into());
    };

    let labels = r["webviews"].clone();
    let two = labels
        .as_array()
        .map(|l| {
            l.len() == 2
                && l.iter().any(|x| x == "bar-pselftest")
                && l.iter().any(|x| x == "panel-pselftest")
        })
        .unwrap_or(false);
    let ipc = r["ipc"].as_str().unwrap_or_default().to_string();
    let ipc_ok = ipc.split(',').count() == 3 && ipc.split(',').all(|x| x == "denied");
    let pw = r["password"].clone();
    let pw_ok = pw["IsPasswordAutosaveEnabled"] == true && pw["IsGeneralAutofillEnabled"] == true;
    let mut out = vec![
        check(
            "panel_window_two_webviews",
            two,
            json!({ "webviews": labels }),
            true,
        ),
        check(
            "panel_webview_no_ipc",
            ipc_ok,
            json!({ "tried": ["bar_info", "app_info", "win_close"], "results": ipc }),
            true,
        ),
        check(
            "panel_password_autosave",
            pw_ok,
            json!({ "webview": "panel-pselftest", "readback": pw }),
            cfg!(windows),
        ),
    ];
    if cfg!(windows) {
        // HTMINBUTTON = 8, HTMAXBUTTON = 9 (Snap Layouts), HTCLOSE = 20.
        let hits = r["hits"].clone();
        let ok = hits == json!([8, 9, 20]);
        out.push(check(
            "caption_buttons_hit_test",
            ok,
            json!({ "expected": [8, 9, 20], "got": hits }),
            true,
        ));
    }
    out
}

/// Serves one HTML page on a free loopback port for the rest of the process.
fn serve_page(body: &'static str) -> std::io::Result<u16> {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = l.local_addr()?.port();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf);
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    Ok(port)
}
