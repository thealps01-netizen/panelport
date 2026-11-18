//! Test-only harness (cargo feature `e2e`, never in release builds) used by
//! scripts/e2e-linux.sh. It watches `$PANELPORT_E2E_DIR` for `<name>.json` requests:
//!   {"op":"eval","label":"<window>","js":"..."}   run JS in a window
//!   {"op":"url","label":"<window>"}               report the window's current URL
//!   {"op":"labels"}                                list open windows
//!   {"op":"open","id":"<panel id>"}                open a panel window
//! and writes the answer to `<name>.out`.

use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub fn spawn(app: AppHandle) {
    let Some(dir) = std::env::var_os("PANELPORT_E2E_DIR").map(PathBuf::from) else {
        return;
    };
    log::warn!("E2E harness active on {}", dir.display());
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(200));
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else {
                continue;
            };
            let _ = std::fs::remove_file(&p);
            let answer = handle(&app, &text);
            let _ = std::fs::write(p.with_extension("out"), answer);
        }
    });
}

fn handle(app: &AppHandle, text: &str) -> String {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return "bad request".into();
    };
    let label = v["label"].as_str().unwrap_or_default();
    match v["op"].as_str().unwrap_or_default() {
        "eval" => match app.get_webview_window(label) {
            Some(w) => match w.eval(v["js"].as_str().unwrap_or_default()) {
                Ok(()) => "ok".into(),
                Err(e) => format!("error: {e}"),
            },
            None => "no such window".into(),
        },
        "url" => app
            .get_webview_window(label)
            .and_then(|w| w.url().ok())
            .map(|u| u.to_string())
            .unwrap_or_else(|| "no such window".into()),
        "labels" => app
            .webview_windows()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join(","),
        "open" => match crate::windows::open_panel(app, v["id"].as_str().unwrap_or_default()) {
            Ok(()) => "ok".into(),
            Err(e) => format!("error: {e}"),
        },
        _ => "unknown op".into(),
    }
}
