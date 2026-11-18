//! Window management: the launcher, and one window per panel.
//!
//! Every window is frameless; its title bar is a bundled page (see chrome.rs). A panel window
//! holds two webviews (Tauri's `unstable` multi-webview API):
//!   - `bar-<id>`: the bundled title bar (`titlebar.html`), allowed only the title-bar commands
//!     (capabilities/title-bar.json) and unable to navigate anywhere else;
//!   - `panel-<id>`: the panel itself. Its remote origin gets no IPC access at all; only the
//!     bundled "not reachable" page shown inside it may call commands (local-pages.json).

use crate::chrome;
use crate::settings::Geometry;
use crate::state::{lock, AppState, PanelState};
use crate::urls::{self, Nav};
use crate::{i18n, icons, monitor, passwords, previews};
use std::sync::Arc;
use tauri::webview::{NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::window::WindowBuilder;
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalSize, Webview,
    WebviewUrl, WebviewWindowBuilder, Window, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;
use url::Url;

pub const LAUNCHER: &str = "launcher";
const PANEL_PREFIX: &str = "panel-";
const BAR_PREFIX: &str = "bar-";
const WINDOW_PREFIX: &str = "pwin-";

/// Label of a panel's content webview.
pub fn panel_label(id: &str) -> String {
    format!("{PANEL_PREFIX}{id}")
}

/// Label of a panel window's title-bar webview.
pub fn bar_label(id: &str) -> String {
    format!("{BAR_PREFIX}{id}")
}

/// Label of a panel's window.
pub fn window_label(id: &str) -> String {
    format!("{WINDOW_PREFIX}{id}")
}

pub fn panel_id(label: &str) -> Option<&str> {
    label.strip_prefix(PANEL_PREFIX)
}

pub fn bar_id(label: &str) -> Option<&str> {
    label.strip_prefix(BAR_PREFIX)
}

pub fn window_id(label: &str) -> Option<&str> {
    label.strip_prefix(WINDOW_PREFIX)
}

/// Which part of the launcher to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Panels,
    Find,
    Settings,
}

impl View {
    fn hash(self) -> &'static str {
        match self {
            View::Panels => "panels",
            View::Find => "find",
            View::Settings => "settings",
        }
    }
}

pub fn open_external(app: &AppHandle, url: &Url) {
    // Only http(s) URLs reach this point (see urls::decide).
    if let Err(e) = app.opener().open_url(url.as_str(), None::<&str>) {
        log::error!("opening {url} in the default browser failed: {e}");
    }
}

fn webview_dir(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.state::<Arc<AppState>>().paths.webview.clone()
}

fn focus(w: &Window) {
    let _ = w.unminimize();
    let _ = w.show();
    let _ = w.set_focus();
}

fn apply_geometry(w: &Window, g: &Geometry) {
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

pub fn show_launcher(app: &AppHandle, view: View) {
    let state = app.state::<Arc<AppState>>();
    if let Some(w) = app.get_webview_window(LAUNCHER) {
        if let Some(window) = app.get_window(LAUNCHER) {
            focus(&window);
        }
        if view != View::Panels {
            let _ = w.eval(format!(
                "window.panelport&&window.panelport.show('{}')",
                view.hash()
            ));
        }
        return;
    }
    let t = i18n::t(state.lang);
    let page = format!("launcher.html#{}", view.hash());
    let nav_app = app.clone();
    let mut builder = WebviewWindowBuilder::new(app, LAUNCHER, WebviewUrl::App(page.into()))
        .title(t.launcher_title)
        .decorations(false)
        .inner_size(1200.0, 760.0)
        .min_inner_size(900.0, 600.0)
        .visible(false)
        .on_navigation(move |target| match urls::decide(target, None) {
            Nav::Allow => true,
            Nav::External(u) => {
                open_external(&nav_app, &u);
                false
            }
            Nav::Deny => {
                log::warn!("blocked navigation in the launcher to {}", target.scheme());
                false
            }
        })
        .on_new_window(|_, _| NewWindowResponse::Deny);
    if let Some(dir) = webview_dir(app) {
        builder = builder.data_directory(dir);
    }
    match builder.build() {
        Ok(_) => {
            let Some(window) = app.get_window(LAUNCHER) else {
                return;
            };
            chrome::install(&window, LAUNCHER);
            if let Some(g) = state.settings().launcher_geometry.clone() {
                apply_geometry(&window, &g);
            } else {
                let _ = window.center();
            }
            focus(&window);
        }
        Err(e) => log::error!("creating the launcher window failed: {e}"),
    }
}

fn down_page(id: &str) -> String {
    format!("down.html?panel={id}")
}

/// Where a panel webview should be right now.
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

/// The panel's content webview, with its navigation guard, data folder and page-load hook.
fn panel_webview(
    app: &AppHandle,
    id: &str,
    start: WebviewUrl,
    panel: Url,
) -> WebviewBuilder<tauri::Wry> {
    let label = panel_label(id);
    let nav_app = app.clone();
    let nav_panel = panel.clone();
    let nav_label = label.clone();
    let new_app = app.clone();
    let new_label = label.clone();
    let load_panel = panel.clone();
    let load_id = id.to_string();
    let mut b = WebviewBuilder::new(&label, start)
        .on_navigation(move |target| match urls::decide(target, Some(&nav_panel)) {
            Nav::Allow => true,
            Nav::External(u) => {
                open_external(&nav_app, &u);
                false
            }
            Nav::Deny => {
                log::warn!("blocked navigation in {nav_label} to {}", target.scheme());
                false
            }
        })
        .on_new_window(move |target, _features| {
            match urls::decide(&target, Some(&panel)) {
                // Same-origin popups (target=_blank inside the panel) open in the same webview.
                Nav::Allow if !urls::is_app_local(&target) => {
                    if let Some(w) = new_app.get_webview(&new_label) {
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
        .on_page_load(move |webview, payload| {
            if payload.event() == PageLoadEvent::Finished
                && urls::same_origin(payload.url(), &load_panel)
            {
                // Give the panel a moment to draw before the preview is taken.
                let app = webview.app_handle().clone();
                let id = load_id.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(2500));
                    previews::capture(&app, &id, false);
                });
            }
        });
    if let Some(dir) = webview_dir(app) {
        b = b.data_directory(dir);
    }
    b
}

/// The title-bar webview: its bundled page only, never anywhere else.
fn bar_webview(app: &AppHandle, id: &str) -> WebviewBuilder<tauri::Wry> {
    let label = bar_label(id);
    let nav_label = label.clone();
    let mut b = WebviewBuilder::new(&label, WebviewUrl::App("titlebar.html".into()))
        .on_navigation(move |target| match urls::decide_bar(target) {
            Nav::Allow => true,
            _ => {
                log::warn!("blocked navigation in {nav_label}");
                false
            }
        })
        .on_new_window(|_, _| NewWindowResponse::Deny);
    if let Some(dir) = webview_dir(app) {
        b = b.data_directory(dir);
    }
    b
}

/// Lays out a panel window's two webviews: the title bar on top, the panel below.
pub fn layout_panel_window(window: &Window) {
    let Some(id) = window_id(window.label()) else {
        return;
    };
    let Ok(size) = window.inner_size() else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let size: LogicalSize<f64> = size.to_logical(scale);
    let bar_h = chrome::BAR_H.min(size.height);
    let app = window.app_handle();
    if let Some(bar) = app.get_webview(&bar_label(id)) {
        let _ = bar.set_position(LogicalPosition::new(0.0, 0.0));
        let _ = bar.set_size(LogicalSize::new(size.width, bar_h));
        #[cfg(target_os = "linux")]
        pin_bar_height(&bar);
    }
    if let Some(panel) = app.get_webview(&panel_label(id)) {
        let _ = panel.set_position(LogicalPosition::new(0.0, bar_h));
        let _ = panel.set_size(LogicalSize::new(size.width, (size.height - bar_h).max(0.0)));
    }
    chrome::raise(window);
}

/// Builds a panel window with its title-bar and panel webviews. Returns the window and the panel
/// webview. Used by `open_panel` and by the self-test.
pub fn build_panel_window(
    app: &AppHandle,
    id: &str,
    name: &str,
    start: WebviewUrl,
    panel: Url,
    visible: bool,
) -> Result<(Window, Webview), String> {
    let window = WindowBuilder::new(app, window_label(id))
        .title(name)
        .decorations(false)
        .inner_size(1200.0, 800.0)
        .min_inner_size(480.0, 360.0)
        .visible(visible)
        .build()
        .map_err(|e| format!("creating panel window failed: {e}"))?;
    let size: LogicalSize<f64> = window
        .inner_size()
        .map(|s| s.to_logical(window.scale_factor().unwrap_or(1.0)))
        .unwrap_or(LogicalSize::new(1200.0, 800.0));
    let built = (|| -> tauri::Result<Webview> {
        window.add_child(
            bar_webview(app, id),
            LogicalPosition::new(0.0, 0.0),
            LogicalSize::new(size.width, chrome::BAR_H),
        )?;
        window.add_child(
            panel_webview(app, id, start, panel),
            LogicalPosition::new(0.0, chrome::BAR_H),
            LogicalSize::new(size.width, (size.height - chrome::BAR_H).max(0.0)),
        )
    })();
    match built {
        Ok(content) => {
            chrome::install(&window, &bar_label(id));
            Ok((window, content))
        }
        Err(e) => {
            let _ = window.destroy();
            Err(format!("creating panel webviews failed: {e}"))
        }
    }
}

/// On Linux, Tauri packs child webviews into a GTK box that shares the height equally and
/// ignores bounds; keep the title bar at its fixed height there.
#[cfg(target_os = "linux")]
fn pin_bar_height(bar: &Webview) {
    let res = bar.with_webview(|pv| {
        use gtk::prelude::*;
        let w = pv.inner();
        w.set_size_request(-1, chrome::BAR_H as i32);
        w.set_vexpand(false);
        if let Some(parent) = w.parent().and_then(|p| p.downcast::<gtk::Box>().ok()) {
            parent.set_child_packing(&w, false, false, 0, gtk::PackType::Start);
        }
    });
    if let Err(e) = res {
        log::warn!("title bar height: {e}");
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
    if let Some(w) = app.get_window(&window_label(&panel.id)) {
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
    let (window, content) = build_panel_window(app, &panel.id, &panel.name, start, url, false)?;
    if let Some(icon) = icons::window_icon(&state.paths.icons, &panel.id) {
        if let Err(e) = window.set_icon(icon) {
            log::warn!("panel window icon: {e}");
        }
    }
    // Panel webviews (only these) may save the logins typed into them.
    if state.settings().prefs.save_passwords {
        passwords::enable(&content);
    }
    match &panel.geometry {
        Some(g) => apply_geometry(&window, g),
        None => {
            let _ = window.center();
        }
    }
    layout_panel_window(&window);
    focus(&window);
    state.wake();
    Ok(())
}

/// Moves each open panel webview between its panel and the bundled "not reachable" page.
pub fn sync_panel_windows(app: &AppHandle, state: &AppState) {
    let panels = state.snapshot().panels;
    for (label, w) in app.webviews() {
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

/// Refreshes stored previews of open, visible panel windows that show their panel.
pub fn capture_open_panels(app: &AppHandle, state: &AppState) {
    if !state.settings().prefs.keep_previews {
        return;
    }
    let panels = state.snapshot().panels;
    for (label, w) in app.webviews() {
        let Some(id) = panel_id(&label) else { continue };
        let Some(panel) = panels.iter().find(|p| p.id == id) else {
            continue;
        };
        let window = w.window();
        if !window.is_visible().unwrap_or(false) || window.is_minimized().unwrap_or(true) {
            continue;
        }
        let shows_panel = match (w.url(), urls::parse_panel_url(&panel.url)) {
            (Ok(cur), Ok(p)) => urls::same_origin(&cur, &p),
            _ => false,
        };
        if shows_panel {
            previews::capture(app, id, false);
        }
    }
}

/// Closes the windows of panels that were removed or whose address changed.
pub fn close_stale_panel_windows(app: &AppHandle, keep: &[(String, String)]) {
    for (label, w) in app.windows() {
        let Some(id) = window_id(&label) else {
            continue;
        };
        let still = keep.iter().find(|(kid, _)| kid == id);
        let stale = match still {
            None => true,
            Some((_, url)) => {
                let Ok(u) = urls::parse_panel_url(url) else {
                    continue;
                };
                match app
                    .get_webview(&panel_label(id))
                    .and_then(|wv| wv.url().ok())
                {
                    Some(cur) => !urls::is_app_local(&cur) && !urls::same_origin(&cur, &u),
                    None => false,
                }
            }
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
        WindowEvent::Resized(_) => {
            if window_id(&label).is_some() {
                layout_panel_window(window);
            }
            if let Some(g) = read_geometry(window) {
                remember_geometry(&state, &label, g, false);
            }
        }
        WindowEvent::Moved(_) => {
            if let Some(g) = read_geometry(window) {
                remember_geometry(&state, &label, g, false);
            }
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
        } else if let Some(id) = window_id(label) {
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
