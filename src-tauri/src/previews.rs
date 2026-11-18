//! Panel previews for the launcher stage.
//!
//! When a panel window shows its panel, Panelport keeps a still image of it
//! (`<app data>/previews/<id>.png`), taken with WebView2's `CapturePreview` at most every few
//! minutes. The launcher only ever shows this file; it never embeds the live panel. The image
//! can contain whatever the panel showed, so it stays on this PC, can be switched off
//! ("keep previews") and deleted from Settings.

use crate::state::{lock, AppState};
use base64::Engine as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tauri::{AppHandle, Manager};

/// Minimum time between two captures of the same panel.
pub const MIN_INTERVAL: Duration = Duration::from_secs(180);

pub fn dir(state: &AppState) -> PathBuf {
    state.paths.data.join("previews")
}

pub fn file(state: &AppState, id: &str) -> PathBuf {
    dir(state).join(format!("{id}.png"))
}

/// Modification time of a panel's preview, in milliseconds since 1970 (0 when none).
pub fn stamp(state: &AppState, id: &str) -> u64 {
    std::fs::metadata(file(state, id))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn data_url(state: &AppState, id: &str) -> Option<String> {
    let bytes = std::fs::read(file(state, id)).ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

pub fn remove(state: &AppState, id: &str) {
    let p = file(state, id);
    if p.exists() {
        if let Err(e) = std::fs::remove_file(&p) {
            log::warn!("could not delete preview {}: {e}", p.display());
        }
    }
}

/// Deletes every stored preview. Returns how many files were removed.
pub fn clear(state: &AppState) -> usize {
    let mut n = 0;
    if let Ok(entries) = std::fs::read_dir(dir(state)) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("png") {
                match std::fs::remove_file(&p) {
                    Ok(()) => n += 1,
                    Err(err) => log::warn!("could not delete preview {}: {err}", p.display()),
                }
            }
        }
    }
    lock(&state.preview_at).clear();
    n
}

/// Whether a capture is due for this panel now (and records the attempt when it is).
pub fn due(state: &AppState, id: &str, force: bool) -> bool {
    if !state.settings().prefs.keep_previews {
        return false;
    }
    let mut at = lock(&state.preview_at);
    let now = Instant::now();
    if !force
        && at
            .get(id)
            .is_some_and(|t| now.duration_since(*t) < MIN_INTERVAL)
    {
        return false;
    }
    at.insert(id.to_string(), now);
    true
}

#[cfg_attr(not(windows), allow(dead_code))]
fn save(path: &Path, bytes: &[u8]) {
    if bytes.len() < 100 {
        log::warn!("preview capture returned {} bytes; not stored", bytes.len());
        return;
    }
    let res = path
        .parent()
        .map(std::fs::create_dir_all)
        .unwrap_or(Ok(()))
        .and_then(|_| {
            let tmp = path.with_extension("png.tmp");
            std::fs::write(&tmp, bytes)?;
            std::fs::rename(&tmp, path)
        });
    if let Err(e) = res {
        log::error!("storing preview {} failed: {e}", path.display());
    }
}

/// Captures the panel webview `panel-<id>` if a capture is due. Never blocks.
pub fn capture(app: &AppHandle, id: &str, force: bool) {
    let state = app.state::<Arc<AppState>>().inner().clone();
    if !due(&state, id, force) {
        return;
    }
    let Some(webview) = app.get_webview(&crate::windows::panel_label(id)) else {
        return;
    };
    let path = file(&state, id);
    platform::capture(&webview, path);
}

#[cfg(windows)]
mod platform {
    use std::path::PathBuf;
    use webview2_com::CapturePreviewCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG;
    use windows::Win32::System::Com::{IStream, STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET};
    use windows::Win32::UI::Shell::SHCreateMemStream;

    pub fn capture(webview: &tauri::Webview, path: PathBuf) {
        let label = webview.label().to_string();
        let res = webview.with_webview(move |pv| {
            // SAFETY: WebView2 COM calls on the UI thread Tauri runs this closure on.
            if let Err(e) = unsafe { start(&pv, path) } {
                log::warn!("preview capture for {label} failed: {e}");
            }
        });
        if let Err(e) = res {
            log::warn!("preview capture: {e}");
        }
    }

    unsafe fn read_all(stream: &IStream) -> windows_core::Result<Vec<u8>> {
        stream.Seek(0, STREAM_SEEK_SET, None)?;
        let mut stat = STATSTG::default();
        stream.Stat(&mut stat, STATFLAG_NONAME)?;
        let len = stat.cbSize as usize;
        let mut buf = vec![0u8; len];
        let mut read = 0u32;
        stream
            .Read(buf.as_mut_ptr() as _, len as u32, Some(&mut read))
            .ok()?;
        buf.truncate(read as usize);
        Ok(buf)
    }

    unsafe fn start(pv: &tauri::webview::PlatformWebview, path: PathBuf) -> Result<(), String> {
        let core = pv
            .controller()
            .CoreWebView2()
            .map_err(|e| format!("CoreWebView2: {e}"))?;
        let stream = SHCreateMemStream(None).ok_or("could not create a memory stream")?;
        let done = stream.clone();
        let handler = CapturePreviewCompletedHandler::create(Box::new(move |result| {
            result?;
            let bytes = read_all(&done)?;
            std::thread::spawn(move || super::save(&path, &bytes));
            Ok(())
        }));
        core.CapturePreview(
            COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
            &stream,
            &handler,
        )
        .map_err(|e| format!("CapturePreview: {e}"))
    }
}

#[cfg(not(windows))]
mod platform {
    /// WebKitGTK previews are not implemented; the launcher shows the panel's icon instead.
    pub fn capture(_webview: &tauri::Webview, _path: std::path::PathBuf) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;
    use crate::settings;
    use crate::state::Paths;

    fn state() -> AppState {
        let dir = std::env::temp_dir().join(format!("panelport-prev-{}", settings::new_id()));
        let paths = Paths::new(dir.join("data"), dir.join("logs"));
        let loaded = settings::load(&paths.settings);
        AppState::new(paths, Lang::En, loaded)
    }

    #[test]
    fn capture_is_rate_limited_and_respects_the_setting() {
        let s = state();
        assert!(due(&s, "pa", false));
        assert!(!due(&s, "pa", false), "second capture within the interval");
        assert!(due(&s, "pb", false), "other panels are independent");
        assert!(due(&s, "pa", true), "forced capture ignores the interval");
        s.update(false, |st| st.prefs.keep_previews = false)
            .unwrap();
        assert!(
            !due(&s, "pc", true),
            "no capture at all when previews are off"
        );
    }

    #[test]
    fn store_stamp_and_clear() {
        let s = state();
        assert_eq!(stamp(&s, "pa"), 0);
        save(&file(&s, "pa"), &[7u8; 200]);
        save(&file(&s, "pb"), &[7u8; 200]);
        save(&file(&s, "pc"), &[7u8; 10]); // too small to be an image: ignored
        assert!(stamp(&s, "pa") > 0);
        assert!(data_url(&s, "pa")
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert!(data_url(&s, "pc").is_none());
        assert_eq!(clear(&s), 2);
        assert_eq!(stamp(&s, "pa"), 0);
    }
}
