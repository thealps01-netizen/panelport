//! Window management: launcher, settings and one window per panel.

use crate::settings::Geometry;
use crate::state::{lock, AppState, PanelState};
use crate::urls::{self, Nav};
use crate::{i18n, icons, monitor};
use std::sync::Arc;
use tauri::webview::NewWindowResponse;
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, Window, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;
use url::Url;

pub const LAUNCHER: &str = "launcher";
pub const SETTINGS: &str = "settings";
const PANEL_PREFIX: &str = "panel-";

pub fn panel_label(id: &str) -> String {
    format!("{PANEL_PREFIX}{id}")
}

pub fn panel_id(label: &str) -> Option<&str> {
    label.strip_prefix(PANEL_PREFIX)
}

pub fn open_external(app: &AppHandle, url: &Url) {
    // Only http(s) URLs reach this point (see urls::decide).
    if let Err(e) = app.opener().open_url(url.as_str(), None::<&str>) {
        log::error!("opening {url} in the default browser failed: {e}");
    }
}

/// Builder with the navigation guard every window gets. `panel` is the panel URL for panel
/// windows (its origin is the only remote origin that window may show).
fn guarded<'a>(
    app: &'a AppHandle,
    label: &str,
    url: WebviewUrl,
    panel: Option<Url>,
) -> WebviewWindowBuilder<'a, tauri::Wry, AppHandle> {
    let nav_app = app.clone();
    let nav_panel = panel.clone();
    let nav_label = label.to_string();
    let new_app = app.clone();
    let new_label = label.to_string();
    let mut builder = WebviewWindowBuilder::new(app, label, url);
    if let Some(dir) = app.state::<Arc<AppState>>().paths.webview.clone() {
        builder = builder.data_directory(dir);
    }
    builder
        .on_navigation(
            move |target| match urls::decide(target, nav_panel.as_ref()) {
                Nav::Allow => true,
                Nav::External(u) => {
                    open_external(&nav_app, &u);
                    false
                }
                Nav::Deny => {
                    log::warn!("blocked navigation in {nav_label} to {}", target.scheme());
                    false
                }
            },
        )
        .on_new_window(move |target, _features| {
            match urls::decide(&target, panel.as_ref()) {
                // Same-origin popups (target=_blank inside the panel) open in the same window.
                Nav::Allow if !urls::is_app_local(&target) => {
                    if let Some(w) = new_app.get_webview_window(&new_label) {
                        if let Err(e) = w.navigate(target) {
                            log::warn!("navigate for popup failed: {e}");
                        }
                    }
                }
                Nav::External(u) => open_external(&new_app, &u),
                _ => log::warn!("blocked popup from {new_label}"),
            }
            NewWindowResponse::Deny
        })
}

fn focus(w: &WebviewWindow) {
    let _ = w.unminimize();
    let _ = w.show();
    let _ = w.set_focus();
}

fn apply_geometry(w: &WebviewWindow, g: &Geometry) {
    // Only restore a position that is still on a connected monitor.
    let on_screen = w.available_monitors().unwrap_or_default().iter().any(|m| {
        let p = m.position();
        let s = m.size();
        g.x + 40 >= p.x
            && g.y >= p.y - 10
            && g.x + 40 < p.x + s.width as i32
            && g.y + 20 < p.y + s.height as i32
    });
    if g.width >= 320 && g.height >= 240 {
        let _ = w.set_size(PhysicalSize::new(g.width, g.height));
    }
    if on_screen {
        let _ = w.set_position(PhysicalPosition::new(g.x, g.y));
    } else {
        let _ = w.center();
    }
    if g.maximized {
        let _ = w.maximize();
    }
}

fn read_geometry(w: &Window) -> Option<Geometry> {
    if w.is_minimized().unwrap_or(false) {
        return None;
    }
    let maximized = w.is_maximized().unwrap_or(false);
    let pos = w.outer_position().ok()?;
    let size = w.inner_size().ok()?;
    Some(Geometry {
        x: pos.x,
        y: pos.y,
        width: size.width,
        height: size.height,
        maximized,
    })
}

pub fn show_launcher(app: &AppHandle, find: bool) {
    let state = app.state::<Arc<AppState>>();
    if let Some(w) = app.get_webview_window(LAUNCHER) {
        focus(&w);
        if find {
            let _ = w.eval("window.panelport && window.panelport.openFind()");
        }
        return;
    }
    let t = i18n::t(state.lang);
    let page = if find {
        "launcher.html#find"
    } else {
        "launcher.html"
    };
    let built = guarded(app, LAUNCHER, WebviewUrl::App(page.into()), None)
        .title(t.launcher_title)
        .inner_size(880.0, 620.0)
        .min_inner_size(520.0, 420.0)
        .visible(false)
        .build();
    match built {
        Ok(w) => {
            if let Some(g) = state.settings().launcher_geometry.clone() {
                apply_geometry(&w, &g);
            } else {
                let _ = w.center();
            }
            focus(&w);
        }
        Err(e) => log::error!("creating the launcher window failed: {e}"),
    }
}

pub fn show_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(SETTINGS) {
        focus(&w);
        return;
    }
    let state = app.state::<Arc<AppState>>();
    let t = i18n::t(state.lang);
    match guarded(app, SETTINGS, WebviewUrl::App("settings.html".into()), None)
        .title(t.settings_title)
        .inner_size(900.0, 680.0)
        .min_inner_size(560.0, 480.0)
        .center()
        .build()
    {
        Ok(w) => focus(&w),
        Err(e) => log::error!("creating the settings window failed: {e}"),
    }
}

fn down_page(id: &str) -> String {
    format!("down.html?panel={id}")
}

/// Where a panel window should be right now.
fn panel_target(state: &AppState, id: &str, panel_url: &Url) -> Url {
    match state.panel_state(id) {
        PanelState::Up | PanelState::Error => lock(&state.last_url)
            .get(id)
            .filter(|u| urls::same_origin(u, panel_url))
            .cloned()
            .unwrap_or_else(|| panel_url.clone()),
        _ => urls::app_url(&down_page(id)),
    }
}

pub fn open_panel(app: &AppHandle, id: &str) -> Result<(), String> {
    let state = app.state::<Arc<AppState>>();
    let panel = state
        .settings()
        .panels
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| format!("unknown panel {id}"))?;
    let label = panel_label(&panel.id);
    if let Some(w) = app.get_webview_window(&label) {
        focus(&w);
        return Ok(());
    }
    let url = urls::parse_panel_url(&panel.url).map_err(|e| format!("bad panel url: {e:?}"))?;
    let target = panel_target(&state, &panel.id, &url);
    let start = if urls::is_app_local(&target) {
        WebviewUrl::App(down_page(&panel.id).into())
    } else {
        WebviewUrl::External(target)
    };
    let mut b = guarded(app, &label, start, Some(url))
        .title(&panel.name)
        .inner_size(1200.0, 800.0)
        .min_inner_size(480.0, 360.0)
        .visible(false);
    if let Some(icon) = icons::window_icon(&state.paths.icons, &panel.id) {
        b = b.icon(icon).map_err(|e| format!("window icon: {e}"))?;
    }
    let w = b
        .build()
        .map_err(|e| format!("creating panel window failed: {e}"))?;
    // Panel windows (only these) may save the logins typed into them.
    crate::passwords::enable(&w);
    match &panel.geometry {
        Some(g) => apply_geometry(&w, g),
        None => {
            let _ = w.center();
        }
    }
    focus(&w);
    state.wake();
    Ok(())
}

/// Moves each open panel window between its panel and the bundled "not reachable" page.
pub fn sync_panel_windows(app: &AppHandle, state: &AppState) {
    let panels = state.snapshot().panels;
    for (label, w) in app.webview_windows() {
        let Some(id) = panel_id(&label) else { continue };
        let Some(panel) = panels.iter().find(|p| p.id == id) else {
            continue;
        };
        let Ok(panel_url) = urls::parse_panel_url(&panel.url) else {
            continue;
        };
        let Ok(current) = w.url() else { continue };
        let st = state.panel_state(id);
        let showing_local = urls::is_app_local(&current);
        if showing_local && matches!(st, PanelState::Up | PanelState::Error) {
            let target = panel_target(state, id, &panel_url);
            log::info!("panel {:?} is reachable again; showing it", panel.name);
            if let Err(e) = w.navigate(target) {
                log::warn!("navigate back to panel failed: {e}");
            }
        } else if !showing_local
            && matches!(st, PanelState::Down | PanelState::NotPage)
            && urls::same_origin(&current, &panel_url)
        {
            lock(&state.last_url).insert(id.to_string(), current);
            if let Err(e) = w.navigate(urls::app_url(&down_page(id))) {
                log::warn!("navigate to status page failed: {e}");
            }
        }
    }
}

/// Closes the windows of panels that were removed or whose address changed.
pub fn close_stale_panel_windows(app: &AppHandle, keep: &[(String, String)]) {
    for (label, w) in app.webview_windows() {
        let Some(id) = panel_id(&label) else { continue };
        let still = keep.iter().find(|(kid, _)| kid == id);
        let stale = match (still, w.url()) {
            (None, _) => true,
            (Some((_, url)), Ok(cur)) => {
                let Ok(u) = urls::parse_panel_url(url) else {
                    continue;
                };
                !urls::is_app_local(&cur) && !urls::same_origin(&cur, &u)
            }
            _ => false,
        };
        if stale {
            let _ = w.destroy();
        }
    }
}

pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let app = window.app_handle();
    let state = app.state::<Arc<AppState>>();
    let label = window.label().to_string();
    match event {
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
            let Some(g) = read_geometry(window) else {
                return;
            };
            remember_geometry(&state, &label, g, false);
        }
        WindowEvent::CloseRequested { api, .. } => {
            if let Some(g) = read_geometry(window) {
                remember_geometry(&state, &label, g, true);
            }
            if label == LAUNCHER {
                api.prevent_close();
                let _ = window.hide();
                let first_time = !state.settings().close_hint_shown;
                if first_time {
                    let t = i18n::t(state.lang);
                    monitor::notify(app, t.tray_hint_title, t.tray_hint_body);
                    if let Err(e) = state.update(true, |s| s.close_hint_shown = true) {
                        log::error!("saving close hint flag failed: {e}");
                    }
                }
            }
        }
        _ => {}
    }
}

fn remember_geometry(state: &AppState, label: &str, g: Geometry, persist_now: bool) {
    let res = state.update(persist_now, |s| {
        if label == LAUNCHER {
            merge(&mut s.launcher_geometry, g);
        } else if let Some(id) = panel_id(label) {
            if let Some(p) = s.panels.iter_mut().find(|p| p.id == id) {
                merge(&mut p.geometry, g);
            }
        }
    });
    if let Err(e) = res {
        log::error!("saving window position failed: {e}");
    }
}

/// While maximized, keep the last normal size/position so un-maximizing restores it.
fn merge(slot: &mut Option<Geometry>, g: Geometry) {
    match (slot.as_mut(), g.maximized) {
        (Some(old), true) => old.maximized = true,
        _ => *slot = Some(g),
    }
}
