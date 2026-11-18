//! Background health loop: checks every panel, debounces state changes, sends notifications,
//! switches panel windows between the panel and the bundled "not reachable" page, and keeps the
//! tray up to date.

use crate::docker;
use crate::http::{self, Health};
use crate::state::{lock, AppState, PanelRuntime, PanelState};
use crate::{i18n, tray, urls, windows};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

/// Failed checks in a row before a panel that was up counts as down (debounce).
pub const DOWN_AFTER: u32 = 2;
pub const INTERVAL: Duration = Duration::from_secs(5);
const CHECK_TIMEOUT: Duration = Duration::from_secs(4);
/// Minimum time between two notifications for the same panel.
const NOTIFY_COOLDOWN: Duration = Duration::from_secs(60);
const DOCKER_REFRESH: Duration = Duration::from_secs(10);
/// How often the Docker engine state shown in the title bar is refreshed while all is well.
const DOCKER_IDLE_REFRESH: Duration = Duration::from_secs(30);

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
        // An answer that is not a web page is definite (not an outage), so no debounce.
        Health::NotPage => {
            rt.fails = 0;
            rt.state = PanelState::NotPage;
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
    // "Back" is owed only after a "down" was sent; a "not a web page" phase in between (an API
    // answering first while the panel restarts) does not cancel it.
    if matches!(rt.state, PanelState::Up | PanelState::Error) && rt.down_notified {
        rt.down_notified = false;
        rt.last_notified = Some(now);
        return Some(Transition::CameBack);
    }
    None
}

/// Records what the screens show besides the state: when the state began, the response time,
/// and the last answer that counted as up.
pub fn note(rt: &mut PanelRuntime, before: PanelState, check: &http::Check, wall: SystemTime) {
    if before != rt.state || rt.since.is_none() {
        rt.since = Some(wall);
    }
    rt.latency_ms = check.latency_ms;
    if rt.state == PanelState::Up {
        if let (Some(status), Some(ms)) = (check.status, check.latency_ms) {
            rt.last_ok = Some((wall, status, ms));
        }
    }
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
                let expect_page = p.health_path == crate::settings::DEFAULT_HEALTH_PATH;
                Some(s.spawn(move || (id, http::check(&target, CHECK_TIMEOUT, expect_page))))
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
            note(entry, before, &check, SystemTime::now());
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
    if settings.prefs.notify {
        for (title, body) in toasts {
            notify(app, &title, body);
        }
    } else if !toasts.is_empty() {
        log::info!(
            "{} notification(s) not shown: notifications are off",
            toasts.len()
        );
    }

    if any_local_down {
        refresh_docker(state, false);
    } else {
        refresh_docker_idle(state);
    }
    windows::sync_panel_windows(app, state);
    windows::capture_open_panels(app, state);
    tray::refresh(app, state);
    if let Err(e) = state.flush() {
        log::error!("periodic settings save failed: {e}");
    }
}

/// Keeps the title bar's Docker line current while every panel is fine.
fn refresh_docker_idle(state: &AppState) {
    let stale = lock(&state.docker)
        .at
        .is_none_or(|t| t.elapsed() >= DOCKER_IDLE_REFRESH);
    if stale {
        refresh_docker(state, true);
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
    let (engine, version) = docker::engine_info(cli.as_ref());
    let containers = match (&cli, &engine) {
        (Some(cli), docker::Engine::Running) => docker::containers(cli).unwrap_or_else(|e| {
            log::warn!("{e}");
            Vec::new()
        }),
        _ => Vec::new(),
    };
    let mut d = lock(&state.docker);
    d.engine = Some(engine);
    d.engine_version = version;
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

    #[test]
    fn note_tracks_since_latency_and_last_good_answer() {
        let now = Instant::now();
        let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let t1 = t0 + Duration::from_secs(5);
        let ok = |ms| http::Check {
            health: Health::Up,
            status: Some(200),
            detail: "HTTP 200".into(),
            latency_ms: Some(ms),
        };
        let down = http::Check {
            health: Health::Down,
            status: None,
            detail: "refused".into(),
            latency_ms: None,
        };
        let mut a = rt();
        let before = a.state;
        step(&mut a, Health::Up, Some(200), "".into(), now);
        note(&mut a, before, &ok(14), t0);
        assert_eq!(
            (a.since, a.latency_ms, a.last_ok),
            (Some(t0), Some(14), Some((t0, 200, 14)))
        );
        // Same state later: "since" stays at the start of the state.
        let before = a.state;
        step(&mut a, Health::Up, Some(200), "".into(), now);
        note(&mut a, before, &ok(22), t1);
        assert_eq!((a.since, a.last_ok), (Some(t0), Some((t1, 200, 22))));
        // Going down moves "since" and keeps the last good answer.
        let t2 = t1 + Duration::from_secs(5);
        for _ in 0..DOWN_AFTER {
            let before = a.state;
            step(&mut a, Health::Down, None, "refused".into(), now);
            note(&mut a, before, &down, t2);
        }
        assert_eq!(a.state, PanelState::Down);
        assert_eq!(
            (a.since, a.latency_ms, a.last_ok),
            (Some(t2), None, Some((t1, 200, 22)))
        );
    }

    #[test]
    fn not_a_page_is_its_own_state_set_at_once_and_silent() {
        let now = Instant::now();
        let mut a = rt();
        step(&mut a, Health::Up, Some(200), "".into(), now);
        // One answer is enough (it is an answer, not an outage) and it sends no notification.
        assert_eq!(
            step(&mut a, Health::NotPage, Some(401), "HTTP 401".into(), now),
            None
        );
        assert_eq!(a.state, PanelState::NotPage);
        assert_eq!(step(&mut a, Health::Up, Some(200), "".into(), now), None);
        assert_eq!(a.state, PanelState::Up);
        // A panel first seen as "not a page" is shown that way without a notification.
        let mut b = rt();
        assert_eq!(
            step(&mut b, Health::NotPage, Some(401), "".into(), now),
            None
        );
        assert_eq!(b.state, PanelState::NotPage);
    }

    #[test]
    fn back_is_reported_even_after_a_not_a_page_phase() {
        let now = Instant::now();
        let mut a = rt();
        step(&mut a, Health::Up, Some(200), "".into(), now);
        step(&mut a, Health::Down, None, "x".into(), now);
        assert_eq!(
            step(&mut a, Health::Down, None, "x".into(), now),
            Some(Transition::WentDown)
        );
        // Restarting, the panel first answers 401 from its API, then serves its page again.
        let later = now + Duration::from_secs(120);
        assert_eq!(
            step(&mut a, Health::NotPage, Some(401), "".into(), later),
            None
        );
        assert_eq!(
            step(&mut a, Health::Up, Some(200), "".into(), later),
            Some(Transition::CameBack)
        );
    }
}
