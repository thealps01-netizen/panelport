//! Shared app state.

use crate::docker::{Container, Engine};
use crate::i18n::Lang;
use crate::settings::{self, Settings};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Instant;
use url::Url;

pub struct Paths {
    pub data: PathBuf,
    pub logs: PathBuf,
    pub settings: PathBuf,
    pub icons: PathBuf,
}

impl Paths {
    pub fn new(data: PathBuf, logs: PathBuf) -> Self {
        Paths {
            settings: data.join("settings.json"),
            icons: data.join("icons"),
            data,
            logs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelState {
    Checking,
    Up,
    /// Reachable but answering with server errors.
    Error,
    Down,
}

#[derive(Debug, Clone)]
pub struct PanelRuntime {
    pub state: PanelState,
    pub fails: u32,
    pub status: Option<u16>,
    pub detail: String,
    /// A "down" notification was sent and the matching "back" one is still owed.
    pub down_notified: bool,
    pub last_notified: Option<Instant>,
}

impl Default for PanelRuntime {
    fn default() -> Self {
        PanelRuntime {
            state: PanelState::Checking,
            fails: 0,
            status: None,
            detail: String::new(),
            down_notified: false,
            last_notified: None,
        }
    }
}

#[derive(Default)]
pub struct DockerSnap {
    pub engine: Option<Engine>,
    pub containers: Vec<Container>,
    pub at: Option<Instant>,
    /// A start (Docker Desktop or containers) is in progress.
    pub busy: bool,
    pub started_desktop_at: Option<Instant>,
}

/// Settings-loading notice shown once in the launcher.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Notice {
    Upgraded { backup: String },
    Corrupt { moved_to: String },
    TooNew,
}

pub struct AppState {
    pub paths: Paths,
    pub lang: Lang,
    pub read_only: bool,
    pub notice: Mutex<Option<Notice>>,
    settings: Mutex<Settings>,
    settings_dirty: AtomicBool,
    pub runtime: Mutex<HashMap<String, PanelRuntime>>,
    pub docker: Mutex<DockerSnap>,
    /// Last in-panel URL per panel, restored when a panel comes back.
    pub last_url: Mutex<HashMap<String, Url>>,
    pub tray_signature: Mutex<String>,
    wake: (Mutex<bool>, Condvar),
}

/// Locks a mutex, recovering the data if a panicking thread poisoned it (the panic is logged
/// by the panic hook; the state itself stays usable).
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl AppState {
    pub fn new(paths: Paths, lang: Lang, loaded: settings::Loaded) -> Self {
        let read_only = loaded.read_only();
        let notice = match &loaded.notice {
            settings::LoadNotice::Upgraded { backup, .. } => Some(Notice::Upgraded {
                backup: backup.display().to_string(),
            }),
            settings::LoadNotice::Corrupt { moved_to, .. } => Some(Notice::Corrupt {
                moved_to: moved_to.display().to_string(),
            }),
            settings::LoadNotice::TooNew { .. } => Some(Notice::TooNew),
            _ => None,
        };
        AppState {
            paths,
            lang,
            read_only,
            notice: Mutex::new(notice),
            settings: Mutex::new(loaded.settings),
            settings_dirty: AtomicBool::new(false),
            runtime: Mutex::new(HashMap::new()),
            docker: Mutex::new(DockerSnap::default()),
            last_url: Mutex::new(HashMap::new()),
            tray_signature: Mutex::new(String::new()),
            wake: (Mutex::new(false), Condvar::new()),
        }
    }

    pub fn settings(&self) -> MutexGuard<'_, Settings> {
        lock(&self.settings)
    }

    pub fn snapshot(&self) -> Settings {
        self.settings().clone()
    }

    /// Changes settings in memory; `persist_now` writes the file immediately, otherwise the
    /// monitor loop writes it within a few seconds (used for window geometry while dragging).
    pub fn update<F: FnOnce(&mut Settings)>(&self, persist_now: bool, f: F) -> std::io::Result<()> {
        {
            let mut s = self.settings();
            f(&mut s);
        }
        self.settings_dirty.store(true, Ordering::SeqCst);
        if persist_now {
            self.flush()
        } else {
            Ok(())
        }
    }

    /// Writes the settings file if it changed. Never writes over a file from a newer version.
    pub fn flush(&self) -> std::io::Result<()> {
        if !self.settings_dirty.swap(false, Ordering::SeqCst) {
            return Ok(());
        }
        if self.read_only {
            log::info!("settings are read-only (written by a newer version); not saving");
            return Ok(());
        }
        let s = self.snapshot();
        settings::save(&self.paths.settings, &s).inspect_err(|e| {
            self.settings_dirty.store(true, Ordering::SeqCst);
            log::error!(
                "saving settings to {} failed: {e}",
                self.paths.settings.display()
            );
        })
    }

    pub fn panel_state(&self, id: &str) -> PanelState {
        lock(&self.runtime)
            .get(id)
            .map(|r| r.state)
            .unwrap_or(PanelState::Checking)
    }

    /// Wakes the monitor loop for an immediate check.
    pub fn wake(&self) {
        let (m, c) = &self.wake;
        *lock(m) = true;
        c.notify_all();
    }

    /// Sleeps up to `dur` or until `wake()` is called.
    pub fn sleep(&self, dur: std::time::Duration) {
        let (m, c) = &self.wake;
        let guard = lock(m);
        let (mut guard, _) = c
            .wait_timeout_while(guard, dur, |woken| !*woken)
            .unwrap_or_else(|p| p.into_inner());
        *guard = false;
    }
}
