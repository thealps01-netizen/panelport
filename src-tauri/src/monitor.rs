//! Background health loop: checks every panel, debounces state changes, sends notifications,
//! switches panel windows between the panel and the bundled "not reachable" page, and keeps the
//! tray up to date.

use crate::docker;
use crate::http::{self, Health};
use crate::state::{lock, AppState, PanelRuntime, PanelState};
use crate::{i18n, tray, urls, windows};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

/// Failed checks in a row before a panel that was up counts as down (debounce).
pub const DOWN_AFTER: u32 = 2;
pub const INTERVAL: Duration = Duration::from_secs(5);
const CHECK_TIMEOUT: Duration = Duration::from_secs(4);
/// Minimum time between two notifications for the same panel.
const NOTIFY_COOLDOWN: Duration = Duration::from_secs(60);
const DOCKER_REFRESH: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    WentDown,
    CameBack,
}

/// Pure state step, unit-tested. Returns the notification to send, if any.
pub fn step(
    rt: &mut PanelRuntime,
    health: Health,
    status: Option<u16>,
    detail: String,
    now: Instant,
) -> Option<Transition> {
    let prev = rt.state;
    rt.status = status;
    rt.detail = detail;
    match health {
        Health::Up | Health::Error => {
            rt.fails = 0;
            rt.state = if health == Health::Up {
                PanelState::Up
            } else {
                PanelState::Error
            };
        }
        Health::Down => {
            rt.fails += 1;
            // A panel never seen up is shown as down at once (no notification); a panel that
            // was up must fail DOWN_AFTER checks in a row first.
            if prev == PanelState::Checking || rt.fails >= DOWN_AFTER {
                rt.state = PanelState::Down;
            }
        }
    }
    let cooled = rt
        .last_notified
        .is_none_or(|t| now.duration_since(t) >= NOTIFY_COOLDOWN);
    let was_reachable = matches!(prev, PanelState::Up | PanelState::Error);
    if was_reachable && rt.state == PanelState::Down && cooled {
        rt.down_notified = true;
        rt.last_notified = Some(now);
        return Some(Transition::WentDown);
    }
    if prev == PanelState::Down
        && matches!(rt.state, PanelState::Up | PanelState::Error)
        && rt.down_notified
    {
        rt.down_notified = false;
        rt.last_notified = Some(now);
        return Some(Transition::CameBack);
    }
    None
}

pub fn spawn(app: AppHandle) {
    std::thread::Builder::new()
        .name("panel-monitor".into())
        .spawn(move || loop {
            let state = app.state::<Arc<AppState>>().inner().clone();
            cycle(&app, &state);
            state.sleep(INTERVAL);
        })
        .expect("spawn monitor thread");
}

fn cycle(app: &AppHandle, state: &AppState) {
    let settings = state.snapshot();
    // Check all panels in parallel.
    let results: Vec<(String, http::Check)> = std::thread::scope(|s| {
        let handles: Vec<_> = settings
            .panels
            .iter()
            .filter_map(|p| {
                let url = urls::parse_panel_url(&p.url).ok()?;
                let target = urls::health_url(&url, &p.health_path);
                let id = p.id.clone();
                Some(s.spawn(move || (id, http::check(&target, CHECK_TIMEOUT))))
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    });

    let t = i18n::t(state.lang);
    let now = Instant::now();
    let mut any_local_down = false;
    let mut toasts: Vec<(String, &str)> = Vec::new();
    {
        let mut rt = lock(&state.runtime);
        rt.retain(|id, _| settings.panels.iter().any(|p| &p.id == id));
        for (id, check) in results {
            let Some(panel) = settings.panels.iter().find(|p| p.id == id) else {
                continue;
            };
            let entry = rt.entry(id.clone()).or_default();
            let before = entry.state;
            let tr = step(
                entry,
                check.health.clone(),
                check.status,
                check.detail.clone(),
                now,
            );
            if before != entry.state {
                log::info!(
                    "panel {:?} ({}) {:?} -> {:?}: {}",
                    panel.name,
                    panel.url,
                    before,
                    entry.state,
                    check.detail
                );
            }
            if entry.state == PanelState::Down
                && urls::parse_panel_url(&panel.url)
                    .map(|u| urls::is_loopback(&u))
                    .unwrap_or(false)
            {
                any_local_down = true;
            }
            if let Some(tr) = tr {
                let (title, body) = match tr {
                    Transition::WentDown => (t.panel_down_title, t.panel_down_body),
                    Transition::CameBack => (t.panel_up_title, t.panel_up_body),
                };
                toasts.push((title.replace("{name}", &panel.name), body));
            }
        }
    }
    // Outside the lock: showing a notification must never stall the commands that read state.
    for (title, body) in toasts {
        notify(app, &title, body);
    }

    if any_local_down {
        refresh_docker(state, false);
    }
    windows::sync_panel_windows(app, state);
    tray::refresh(app, state);
    if let Err(e) = state.flush() {
        log::error!("periodic settings save failed: {e}");
    }
}

/// Re-reads Docker engine and container state (rate-limited unless `force`).
pub fn refresh_docker(state: &AppState, force: bool) {
    {
        let d = lock(&state.docker);
        if !force && d.at.is_some_and(|t| t.elapsed() < DOCKER_REFRESH) {
            return;
        }
    }
    let cli = docker::find_cli();
    let engine = docker::engine(cli.as_ref());
    let containers = match (&cli, &engine) {
        (Some(cli), docker::Engine::Running) => docker::containers(cli).unwrap_or_else(|e| {
            log::warn!("{e}");
            Vec::new()
        }),
        _ => Vec::new(),
    };
    let mut d = lock(&state.docker);
    d.engine = Some(engine);
    d.containers = containers;
    d.at = Some(Instant::now());
}

pub fn notify(app: &AppHandle, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        log::warn!("notification failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt() -> PanelRuntime {
        PanelRuntime::default()
    }

    #[test]
    fn first_result_sets_state_without_notification() {
        let now = Instant::now();
        let mut a = rt();
        assert_eq!(
            step(&mut a, Health::Down, None, "refused".into(), now),
            None
        );
        assert_eq!(a.state, PanelState::Down);
        let mut b = rt();
        assert_eq!(step(&mut b, Health::Up, Some(200), "".into(), now), None);
        assert_eq!(b.state, PanelState::Up);
    }

    #[test]
    fn down_is_debounced_and_back_is_reported() {
        let now = Instant::now();
        let mut a = rt();
        step(&mut a, Health::Up, Some(200), "".into(), now);
        // One failure: still up, no notification.
        assert_eq!(step(&mut a, Health::Down, None, "x".into(), now), None);
        assert_eq!(a.state, PanelState::Up);
        // Second failure in a row: down + notification.
        assert_eq!(
            step(&mut a, Health::Down, None, "x".into(), now),
            Some(Transition::WentDown)
        );
        assert_eq!(a.state, PanelState::Down);
        // Still down: nothing new.
        assert_eq!(step(&mut a, Health::Down, None, "x".into(), now), None);
        // Back up: reported once.
        let later = now + Duration::from_secs(120);
        assert_eq!(
            step(&mut a, Health::Up, Some(200), "".into(), later),
            Some(Transition::CameBack)
        );
        assert_eq!(step(&mut a, Health::Up, Some(200), "".into(), later), None);
    }

    #[test]
    fn single_blip_does_not_notify() {
        let now = Instant::now();
        let mut a = rt();
        step(&mut a, Health::Up, Some(200), "".into(), now);
        assert_eq!(step(&mut a, Health::Down, None, "x".into(), now), None);
        assert_eq!(step(&mut a, Health::Up, Some(200), "".into(), now), None);
        assert_eq!(a.state, PanelState::Up);
    }

    #[test]
    fn flapping_is_rate_limited() {
        let now = Instant::now();
        let mut a = rt();
        step(&mut a, Health::Up, Some(200), "".into(), now);
        step(&mut a, Health::Down, None, "x".into(), now);
        assert_eq!(
            step(&mut a, Health::Down, None, "x".into(), now),
            Some(Transition::WentDown)
        );
        // Comes back 5 s later: the "back" message is still owed and sent.
        let t1 = now + Duration::from_secs(5);
        assert_eq!(
            step(&mut a, Health::Up, Some(200), "".into(), t1),
            Some(Transition::CameBack)
        );
        // Goes down again within the cooldown: no second "down" toast.
        let t2 = now + Duration::from_secs(10);
        step(&mut a, Health::Down, None, "x".into(), t2);
        assert_eq!(step(&mut a, Health::Down, None, "x".into(), t2), None);
        assert_eq!(a.state, PanelState::Down);
        // And since no "down" was sent, no "back" either.
        assert_eq!(step(&mut a, Health::Up, Some(200), "".into(), t2), None);
    }
}
