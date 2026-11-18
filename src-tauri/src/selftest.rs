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

    // A real panel-type WebView2 window: password saving and autofill must read back as on.
    let (pw_ok, pw_detail) = panel_webview_check(context);
    checks.push(check(
        "panel_password_autosave",
        pw_ok,
        pw_detail,
        cfg!(windows),
    ));

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

/// Starts a minimal Tauri app with one hidden panel-type window (label `panel-*`, its own
/// temporary WebView2 data folder), runs the same `passwords::enable` that real panel windows
/// use, and reads the settings back. Touches no user data.
#[cfg(windows)]
fn panel_webview_check(context: tauri::Context<tauri::Wry>) -> (bool, Value) {
    use std::sync::{Arc, Mutex};
    let dir =
        std::env::temp_dir().join(format!("panelport-selftest-webview-{}", settings::new_id()));
    let result: Arc<Mutex<Option<Result<crate::passwords::AutofillState, String>>>> =
        Arc::new(Mutex::new(None));
    let slot = result.clone();
    let data_dir = dir.clone();
    let app = tauri::Builder::default()
        .setup(move |app| {
            let handle = app.handle().clone();
            let w = tauri::WebviewWindowBuilder::new(
                app,
                crate::windows::panel_label("selftest"),
                tauri::WebviewUrl::External("about:blank".parse().expect("static URL")),
            )
            .visible(false)
            .data_directory(data_dir.clone())
            .build()?;
            crate::passwords::enable(&w);
            std::thread::spawn(move || {
                // Queued on the UI thread after `enable`, so it reads the applied values.
                let r = crate::passwords::read_blocking(&w);
                *slot.lock().unwrap_or_else(|p| p.into_inner()) = Some(r);
                handle.exit(0);
            });
            Ok(())
        })
        .build(context);
    match app {
        Ok(app) => {
            app.run_return(|_, _| {});
        }
        Err(e) => {
            *result.lock().unwrap_or_else(|p| p.into_inner()) = Some(Err(format!("tauri: {e}")));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    let outcome = result.lock().unwrap_or_else(|p| p.into_inner()).take();
    match outcome {
        Some(Ok(s)) => (
            s.password_autosave && s.general_autofill,
            json!({
                "window": "panel-selftest",
                "IsPasswordAutosaveEnabled": s.password_autosave,
                "IsGeneralAutofillEnabled": s.general_autofill,
            }),
        ),
        Some(Err(e)) => (false, json!({ "error": e })),
        None => (false, json!({ "error": "the check window never reported" })),
    }
}

#[cfg(not(windows))]
fn panel_webview_check(_context: tauri::Context<tauri::Wry>) -> (bool, Value) {
    (false, json!("WebView2 only; not applicable on this OS"))
}
