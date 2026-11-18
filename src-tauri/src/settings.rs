//! Versioned settings file (`settings.json` in the per-user app data folder).
//!
//! Every file carries `"schema": N`. Older files are backed up and upgraded in place; a file
//! written by a newer Panelport is never overwritten (the app runs read-only and says so).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub const SCHEMA: u64 = 1;
pub const DEFAULT_HEALTH_PATH: &str = "/";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub maximized: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum IconPref {
    /// Use the panel's own favicon (cached locally); falls back to a monogram.
    #[default]
    Favicon,
    /// Always use a generated monogram.
    Monogram,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Panel {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default = "default_health_path")]
    pub health_path: String,
    #[serde(default)]
    pub icon: IconPref,
    #[serde(default)]
    pub geometry: Option<Geometry>,
}

fn default_health_path() -> String {
    DEFAULT_HEALTH_PATH.to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Prefs {
    /// Notify when a panel goes down or comes back.
    #[serde(default = "yes")]
    pub notify: bool,
    /// Let panel windows save passwords (WebView2 password manager).
    #[serde(default = "yes")]
    pub save_passwords: bool,
    /// Keep a preview image of each panel for the launcher.
    #[serde(default = "yes")]
    pub keep_previews: bool,
}

fn yes() -> bool {
    true
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            notify: true,
            save_passwords: true,
            keep_previews: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub schema: u64,
    #[serde(default)]
    pub panels: Vec<Panel>,
    #[serde(default)]
    pub launcher_geometry: Option<Geometry>,
    /// The one-time "Panelport keeps running in the tray" toast was shown.
    #[serde(default)]
    pub close_hint_shown: bool,
    /// First-run panel discovery was offered.
    #[serde(default)]
    pub discovery_offered: bool,
    /// Added in 1.1.0 with defaults, so older files load unchanged (schema stays 1).
    #[serde(default)]
    pub prefs: Prefs,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            schema: SCHEMA,
            panels: Vec::new(),
            launcher_geometry: None,
            close_hint_shown: false,
            discovery_offered: false,
            prefs: Prefs::default(),
        }
    }
}

/// What happened while loading, for the log and the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadNotice {
    /// No file yet: defaults are used.
    Fresh,
    /// Loaded as is.
    Loaded,
    /// Upgraded from an older schema; the original was copied to `backup`.
    Upgraded { from: u64, backup: PathBuf },
    /// The file could not be read as settings; it was moved to `moved_to` and defaults are used.
    Corrupt { moved_to: PathBuf, error: String },
    /// Written by a newer Panelport: used read-only, never overwritten.
    TooNew { found: u64 },
}

pub struct Loaded {
    pub settings: Settings,
    pub notice: LoadNotice,
}

impl Loaded {
    pub fn read_only(&self) -> bool {
        matches!(self.notice, LoadNotice::TooNew { .. })
    }
}

pub fn new_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("p{:x}{:x}", nanos & 0xffff_ffff_ffff, n)
}

/// A valid panel id is what `new_id` makes: short, lowercase alphanumerics. Ids end up in window
/// labels and file names, so anything else is rejected.
pub fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 40
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// Schema 0 was the pre-release development format: panels had only `name` and `url`.
fn migrate_0_to_1(mut v: Value) -> Value {
    if let Some(panels) = v.get_mut("panels").and_then(|p| p.as_array_mut()) {
        for p in panels.iter_mut() {
            if let Some(obj) = p.as_object_mut() {
                obj.entry("id").or_insert_with(|| Value::String(new_id()));
                obj.entry("health_path")
                    .or_insert_with(|| Value::String(DEFAULT_HEALTH_PATH.into()));
            }
        }
    }
    v["schema"] = Value::from(1u64);
    v
}

fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

pub fn load(path: &Path) -> Loaded {
    let raw = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Loaded {
                settings: Settings::default(),
                notice: LoadNotice::Fresh,
            }
        }
        Err(e) => return corrupt(path, format!("read failed: {e}")),
    };
    let value: Value = match serde_json::from_slice(&raw) {
        Ok(v) => v,
        Err(e) => return corrupt(path, format!("not valid JSON: {e}")),
    };
    let found = match value.get("schema").and_then(Value::as_u64) {
        Some(s) => s,
        None => return corrupt(path, "missing numeric \"schema\" field".into()),
    };
    if found > SCHEMA {
        // Keep the newer file untouched; read what we understand.
        let settings = serde_json::from_value::<Settings>(value).unwrap_or_default();
        return Loaded {
            settings,
            notice: LoadNotice::TooNew { found },
        };
    }
    let mut value = value;
    let mut notice = LoadNotice::Loaded;
    if found < SCHEMA {
        let backup =
            path.with_file_name(format!("settings.schema{found}.{}.bak.json", timestamp()));
        if let Err(e) = fs::copy(path, &backup) {
            return corrupt(path, format!("could not back up before upgrade: {e}"));
        }
        if found == 0 {
            value = migrate_0_to_1(value);
        }
        notice = LoadNotice::Upgraded {
            from: found,
            backup,
        };
    }
    match serde_json::from_value::<Settings>(value) {
        Ok(mut s) => {
            s.schema = SCHEMA;
            sanitize(&mut s);
            if matches!(notice, LoadNotice::Upgraded { .. }) {
                if let Err(e) = save(path, &s) {
                    log::error!("saving upgraded settings failed: {e}");
                }
            }
            Loaded {
                settings: s,
                notice,
            }
        }
        Err(e) => corrupt(path, format!("unexpected content: {e}")),
    }
}

/// Drops entries that would be unsafe to use (bad ids, duplicate ids, non-http URLs).
fn sanitize(s: &mut Settings) {
    let mut seen = std::collections::HashSet::new();
    s.panels.retain(|p| {
        let ok = is_valid_id(&p.id)
            && seen.insert(p.id.clone())
            && crate::urls::parse_panel_url(&p.url).is_ok();
        if !ok {
            log::warn!(
                "dropping invalid panel entry from settings: id={:?} url={:?}",
                p.id,
                p.url
            );
        }
        ok
    });
    for p in s.panels.iter_mut() {
        if !p.health_path.starts_with('/') {
            p.health_path = DEFAULT_HEALTH_PATH.into();
        }
    }
}

fn corrupt(path: &Path, error: String) -> Loaded {
    let moved_to = path.with_file_name(format!("settings.unreadable.{}.json", timestamp()));
    if let Err(e) = fs::rename(path, &moved_to) {
        log::error!("could not move unreadable settings aside: {e}");
    }
    Loaded {
        settings: Settings::default(),
        notice: LoadNotice::Corrupt { moved_to, error },
    }
}

/// Atomic write: temp file in the same folder, then rename over the old file.
pub fn save(path: &Path, s: &Settings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_vec_pretty(s).map_err(std::io::Error::other)?;
    fs::write(&tmp, body)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("panelport-test-{name}-{}", new_id()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn fresh_when_missing() {
        let d = tmpdir("fresh");
        let l = load(&d.join("settings.json"));
        assert_eq!(l.notice, LoadNotice::Fresh);
        assert_eq!(l.settings, Settings::default());
    }

    #[test]
    fn roundtrip() {
        let d = tmpdir("rt");
        let p = d.join("settings.json");
        let mut s = Settings::default();
        s.panels.push(Panel {
            id: "pabc1".into(),
            name: "Arcane".into(),
            url: "http://localhost:3552/".into(),
            health_path: "/api/health".into(),
            icon: IconPref::Favicon,
            geometry: Some(Geometry {
                x: 10,
                y: 20,
                width: 900,
                height: 700,
                maximized: true,
            }),
        });
        save(&p, &s).unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert!(text.contains("\"schema\": 1"));
        let l = load(&p);
        assert_eq!(l.notice, LoadNotice::Loaded);
        assert_eq!(l.settings, s);
    }

    #[test]
    fn upgrades_schema0_after_backup() {
        let d = tmpdir("up");
        let p = d.join("settings.json");
        let v0 = r#"{"schema":0,"panels":[{"name":"Arcane","url":"http://localhost:3552/"}]}"#;
        fs::write(&p, v0).unwrap();
        let l = load(&p);
        let LoadNotice::Upgraded { from, backup } = &l.notice else {
            panic!("{:?}", l.notice)
        };
        assert_eq!(*from, 0);
        assert_eq!(fs::read_to_string(backup).unwrap(), v0);
        assert_eq!(l.settings.panels.len(), 1);
        assert_eq!(l.settings.panels[0].health_path, "/");
        assert!(is_valid_id(&l.settings.panels[0].id));
        // The upgraded file was written back with the current schema.
        let again = load(&p);
        assert_eq!(again.notice, LoadNotice::Loaded);
        assert_eq!(again.settings.schema, SCHEMA);
    }

    #[test]
    fn newer_schema_is_read_only_and_untouched() {
        let d = tmpdir("new");
        let p = d.join("settings.json");
        let v9 = r#"{"schema":9,"panels":[],"future":true}"#;
        fs::write(&p, v9).unwrap();
        let l = load(&p);
        assert!(l.read_only());
        assert_eq!(fs::read_to_string(&p).unwrap(), v9);
    }

    #[test]
    fn corrupt_file_is_moved_aside() {
        let d = tmpdir("bad");
        let p = d.join("settings.json");
        fs::write(&p, "{not json").unwrap();
        let l = load(&p);
        let LoadNotice::Corrupt { moved_to, .. } = &l.notice else {
            panic!()
        };
        assert!(moved_to.exists());
        assert!(!p.exists());
        assert_eq!(l.settings, Settings::default());
    }

    #[test]
    fn v1_0_files_get_default_prefs() {
        let d = tmpdir("prefs");
        let p = d.join("settings.json");
        // A file written by 1.0.x has no "prefs" object.
        fs::write(&p, r#"{"schema":1,"panels":[],"close_hint_shown":true}"#).unwrap();
        let l = load(&p);
        assert_eq!(l.notice, LoadNotice::Loaded);
        assert_eq!(l.settings.prefs, Prefs::default());
        assert!(
            l.settings.prefs.notify
                && l.settings.prefs.save_passwords
                && l.settings.prefs.keep_previews
        );
        fs::write(&p, r#"{"schema":1,"prefs":{"notify":false}}"#).unwrap();
        let l = load(&p);
        assert!(!l.settings.prefs.notify);
        assert!(
            l.settings.prefs.save_passwords,
            "missing keys keep their defaults"
        );
    }

    #[test]
    fn invalid_entries_are_dropped() {
        let d = tmpdir("san");
        let p = d.join("settings.json");
        fs::write(
            &p,
            r#"{"schema":1,"panels":[
                {"id":"ok1","name":"A","url":"http://localhost:1/"},
                {"id":"../evil","name":"B","url":"http://localhost:2/"},
                {"id":"ok2","name":"C","url":"file:///etc/passwd"},
                {"id":"ok1","name":"Dup","url":"http://localhost:3/"}]}"#,
        )
        .unwrap();
        let l = load(&p);
        assert_eq!(l.settings.panels.len(), 1);
        assert_eq!(l.settings.panels[0].name, "A");
    }
}
