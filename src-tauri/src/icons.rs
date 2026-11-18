//! Panel icons: the panel's own favicon, fetched from the panel's origin and cached locally.
//! Panels without one get a monogram, drawn by the bundled pages.

use crate::http;
use base64::Engine as _;
use std::path::{Path, PathBuf};
use std::time::Duration;
use url::Url;

const KINDS: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("ico", "image/x-icon"),
    ("svg", "image/svg+xml"),
    ("jpg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
];

fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0, 0, 1, 0]) {
        Some("ico")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(b"GIF8") {
        Some("gif")
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase();
        head.contains("<svg").then_some("svg")
    }
}

/// The cached icon file for a panel, if any.
pub fn cached(dir: &Path, id: &str) -> Option<(PathBuf, &'static str)> {
    KINDS.iter().find_map(|(ext, mime)| {
        let p = dir.join(format!("{id}.{ext}"));
        p.is_file().then_some((p, *mime))
    })
}

pub fn remove(dir: &Path, id: &str) {
    for (ext, _) in KINDS {
        let p = dir.join(format!("{id}.{ext}"));
        if p.exists() {
            if let Err(e) = std::fs::remove_file(&p) {
                log::warn!("could not remove cached icon {}: {e}", p.display());
            }
        }
    }
}

pub fn data_url(dir: &Path, id: &str) -> Option<String> {
    let (path, mime) = cached(dir, id)?;
    let bytes = std::fs::read(path).ok()?;
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

/// Window/taskbar icon from the cached favicon (PNG and ICO only).
pub fn window_icon(dir: &Path, id: &str) -> Option<tauri::image::Image<'static>> {
    let (path, mime) = cached(dir, id)?;
    if mime != "image/png" && mime != "image/x-icon" {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    tauri::image::Image::from_bytes(&bytes)
        .map_err(|e| {
            log::info!(
                "cached icon {} not usable as window icon: {e}",
                path.display()
            )
        })
        .ok()
}

/// Fetches the favicon of `panel` (same origin only) into `dir/<id>.<ext>`.
pub fn fetch(panel: &Url, dir: &Path, id: &str) -> Result<(), String> {
    let t = Duration::from_secs(4);
    let mut candidates: Vec<Url> = Vec::new();
    if let Ok(page) = http::get(panel, t, 512 * 1024) {
        for href in http::icon_links(&page.body) {
            if let Ok(u) = panel.join(&href) {
                candidates.push(u);
            }
        }
    }
    if let Ok(u) = panel.join("/favicon.ico") {
        candidates.push(u);
    }
    for u in candidates {
        if !crate::urls::same_origin(&u, panel) {
            continue;
        }
        let Ok(resp) = http::get(&u, t, 256 * 1024) else {
            continue;
        };
        if resp.status != 200 || resp.body.is_empty() {
            continue;
        }
        let Some(ext) = sniff(&resp.body) else {
            continue;
        };
        std::fs::create_dir_all(dir).map_err(|e| format!("create icon folder: {e}"))?;
        remove(dir, id);
        std::fs::write(dir.join(format!("{id}.{ext}")), &resp.body)
            .map_err(|e| format!("write icon: {e}"))?;
        return Ok(());
    }
    Err(format!(
        "no usable favicon at {}",
        panel.origin().ascii_serialization()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffing() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\nrest"), Some("png"));
        assert_eq!(sniff(&[0, 0, 1, 0, 1, 0]), Some("ico"));
        assert_eq!(sniff(b"<?xml version='1.0'?><svg xmlns="), Some("svg"));
        assert_eq!(sniff(b"<html>not an icon</html>"), None);
    }
}
