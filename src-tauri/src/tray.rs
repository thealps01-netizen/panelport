//! System tray: overall status icon and a menu listing every panel with its status dot.

use crate::state::{lock, AppState, PanelState};
use crate::{i18n, windows};
use std::sync::Arc;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, IconMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

const TRAY_ID: &str = "panelport";

const ICON_OK: &[u8] = include_bytes!("../icons/tray/ok.png");
const ICON_WARN: &[u8] = include_bytes!("../icons/tray/warn.png");
const ICON_DOWN: &[u8] = include_bytes!("../icons/tray/down.png");
const ICON_IDLE: &[u8] = include_bytes!("../icons/tray/idle.png");
const DOT_GREEN: &[u8] = include_bytes!("../icons/dots/green.png");
const DOT_AMBER: &[u8] = include_bytes!("../icons/dots/amber.png");
const DOT_RED: &[u8] = include_bytes!("../icons/dots/red.png");
const DOT_GREY: &[u8] = include_bytes!("../icons/dots/grey.png");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overall {
    Idle,
    Ok,
    Degraded,
    Down,
}

pub fn overall(states: &[PanelState]) -> Overall {
    if states.is_empty() {
        return Overall::Idle;
    }
    let up = states.iter().filter(|s| **s == PanelState::Up).count();
    let down = states.iter().filter(|s| **s == PanelState::Down).count();
    if up == states.len() {
        Overall::Ok
    } else if down == states.len() {
        Overall::Down
    } else if states.iter().all(|s| *s == PanelState::Checking) {
        Overall::Idle
    } else {
        Overall::Degraded
    }
}

fn png(bytes: &'static [u8]) -> Option<Image<'static>> {
    Image::from_bytes(bytes)
        .map_err(|e| log::error!("embedded icon: {e}"))
        .ok()
}

/// Status dot and status words shown next to a panel in the tray menu.
pub fn dot_and_word(st: PanelState, t: &'static i18n::T) -> (&'static [u8], &'static str) {
    match st {
        PanelState::Up => (DOT_GREEN, t.up),
        PanelState::Error => (DOT_AMBER, t.error),
        PanelState::Down => (DOT_RED, t.down),
        PanelState::Checking => (DOT_GREY, t.checking),
        PanelState::NotPage => (DOT_AMBER, t.not_page),
        PanelState::ClientError => (DOT_AMBER, t.client_error),
    }
}

/// Tray menu text for a panel's state; a client error names its HTTP status.
pub fn state_text(st: PanelState, status: Option<u16>, t: &'static i18n::T) -> String {
    let word = dot_and_word(st, t).1;
    match (st, status) {
        (PanelState::ClientError, Some(code)) => format!("{word} · HTTP {code}"),
        _ => word.to_string(),
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let state = app.state::<Arc<AppState>>();
    let menu = build_menu(app, &state)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(png(ICON_IDLE).expect("embedded tray icon"))
        .tooltip("Panelport")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                windows::show_launcher(tray.app_handle(), windows::View::Panels);
            }
        })
        .build(app)?;
    Ok(())
}

fn autostart_enabled(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or_else(|e| {
        log::warn!("reading autostart state failed: {e}");
        false
    })
}

fn build_menu(app: &AppHandle, state: &AppState) -> tauri::Result<Menu<tauri::Wry>> {
    let t = i18n::t(state.lang);
    let menu = Menu::new(app)?;
    let panels = state.snapshot().panels;
    if panels.is_empty() {
        menu.append(&MenuItem::with_id(
            app,
            "none",
            t.no_panels,
            false,
            None::<&str>,
        )?)?;
    }
    for p in &panels {
        let st = state.panel_state(&p.id);
        let (dot, _) = dot_and_word(st, t);
        let text = format!(
            "{} — {}",
            p.name,
            state_text(st, state.panel_status(&p.id), t)
        );
        menu.append(&IconMenuItem::with_id(
            app,
            format!("panel:{}", p.id),
            text,
            true,
            png(dot),
            None::<&str>,
        )?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "open",
        t.open_launcher,
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        "find",
        t.find_panels,
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        "settings",
        t.settings,
        true,
        None::<&str>,
    )?)?;
    menu.append(&CheckMenuItem::with_id(
        app,
        "autostart",
        t.start_with_windows,
        true,
        autostart_enabled(app),
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", t.quit, true, None::<&str>)?)?;
    Ok(menu)
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    match id {
        "open" => windows::show_launcher(app, windows::View::Panels),
        "find" => windows::show_launcher(app, windows::View::Find),
        "settings" => windows::show_launcher(app, windows::View::Settings),
        "autostart" => {
            let enable = !autostart_enabled(app);
            set_autostart(app, enable);
        }
        "quit" => {
            let state = app.state::<Arc<AppState>>();
            if let Err(e) = state.flush() {
                log::error!("saving settings on quit failed: {e}");
            }
            app.exit(0);
        }
        other => {
            if let Some(pid) = other.strip_prefix("panel:") {
                if let Err(e) = windows::open_panel(app, pid) {
                    log::error!("opening panel from tray failed: {e}");
                }
            }
        }
    }
}

pub fn set_autostart(app: &AppHandle, enable: bool) -> bool {
    let al = app.autolaunch();
    let res = if enable { al.enable() } else { al.disable() };
    if let Err(e) = res {
        log::error!("changing Start with Windows failed: {e}");
    }
    let now = autostart_enabled(app);
    // Force a menu rebuild so the check mark matches.
    lock(&app.state::<Arc<AppState>>().tray_signature).clear();
    refresh(app, &app.state::<Arc<AppState>>());
    now
}

/// Updates icon, tooltip and menu when something visible changed.
pub fn refresh(app: &AppHandle, state: &AppState) {
    let panels = state.snapshot().panels;
    let states: Vec<PanelState> = panels.iter().map(|p| state.panel_state(&p.id)).collect();
    let statuses: Vec<Option<u16>> = panels.iter().map(|p| state.panel_status(&p.id)).collect();
    let sig = format!(
        "{:?}|{:?}|{}",
        states,
        statuses,
        panels
            .iter()
            .map(|p| format!("{}={}", p.id, p.name))
            .collect::<Vec<_>>()
            .join(",")
    );
    {
        let mut last = lock(&state.tray_signature);
        if *last == sig {
            return;
        }
        *last = sig;
    }
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let t = i18n::t(state.lang);
    let (icon, line) = match overall(&states) {
        Overall::Ok => (ICON_OK, t.tray_all_up),
        Overall::Degraded => (ICON_WARN, t.tray_some_down),
        Overall::Down => (ICON_DOWN, t.tray_all_down),
        Overall::Idle => (
            ICON_IDLE,
            if panels.is_empty() {
                t.no_panels
            } else {
                t.checking
            },
        ),
    };
    if let Err(e) = tray.set_icon(png(icon)) {
        log::warn!("tray icon update failed: {e}");
    }
    let _ = tray.set_tooltip(Some(format!("Panelport — {line}")));
    match build_menu(app, state) {
        Ok(m) => {
            if let Err(e) = tray.set_menu(Some(m)) {
                log::warn!("tray menu update failed: {e}");
            }
        }
        Err(e) => log::warn!("building tray menu failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PanelState::*;

    #[test]
    fn overall_status() {
        assert_eq!(overall(&[]), Overall::Idle);
        assert_eq!(overall(&[Up, Up]), Overall::Ok);
        assert_eq!(overall(&[Up, Down]), Overall::Degraded);
        assert_eq!(overall(&[Up, Error]), Overall::Degraded);
        assert_eq!(overall(&[Down, Down]), Overall::Down);
        assert_eq!(overall(&[Checking]), Overall::Idle);
        // "Not a web page" is a problem with the panel, but not an outage: amber, never red.
        assert_eq!(overall(&[Up, NotPage]), Overall::Degraded);
        assert_eq!(overall(&[NotPage, NotPage]), Overall::Degraded);
        assert_eq!(overall(&[Down, NotPage]), Overall::Degraded);
    }

    #[test]
    fn client_error_is_amber_degraded_and_names_its_status() {
        use crate::i18n::{t, Lang};
        assert_eq!(overall(&[Up, ClientError]), Overall::Degraded);
        assert_eq!(overall(&[ClientError]), Overall::Degraded);
        assert_eq!(dot_and_word(ClientError, t(Lang::En)).0, DOT_AMBER);
        assert_eq!(
            state_text(ClientError, Some(400), t(Lang::En)),
            "error · HTTP 400"
        );
        assert_eq!(
            state_text(ClientError, Some(429), t(Lang::Tr)),
            "hata · HTTP 429"
        );
        assert_eq!(state_text(Up, Some(200), t(Lang::En)), "running");
    }

    #[test]
    fn not_a_page_has_its_own_tray_dot_and_words() {
        let (dot, word) = dot_and_word(PanelState::NotPage, crate::i18n::t(crate::i18n::Lang::En));
        assert_eq!(dot, DOT_AMBER);
        assert_eq!(word, "not a web page / no access");
        let (_, tr) = dot_and_word(PanelState::NotPage, crate::i18n::t(crate::i18n::Lang::Tr));
        assert_eq!(tr, "web sayfası değil / erişim yok");
    }
}
