//! Commands the bundled pages call. The capability in `capabilities/local-pages.json` grants
//! them to bundled pages only; panel (remote) origins get no IPC access at all.

use crate::discovery::{self, Candidate};
use crate::docker::{self, Engine};
use crate::i18n::{self, ErrKind as E};
use crate::settings::{self, IconPref, Panel};
use crate::state::{lock, AppState, Notice, PanelState};
use crate::{icons, monitor, tray, urls, windows};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;

pub const RELEASES_URL: &str = "https://github.com/thealps01-netizen/panelport/releases";

#[derive(Debug, Serialize)]
pub struct UserError {
    pub message: String,
}

fn user_err(state: &AppState, e: E) -> UserError {
    UserError {
        message: i18n::err(state.lang, e).replace("{log}", &state.paths.logs.display().to_string()),
    }
}

type R<T> = Result<T, UserError>;

#[derive(Serialize)]
pub struct AppInfo {
    name: &'static str,
    version: &'static str,
    lang: i18n::Lang,
    log_dir: String,
    data_dir: String,
    releases_url: &'static str,
    read_only: bool,
    notice: Option<Notice>,
    first_run: bool,
}

#[tauri::command]
pub fn app_info(state: State<'_, Arc<AppState>>) -> AppInfo {
    let s = state.settings();
    AppInfo {
        name: "Panelport",
        version: env!("CARGO_PKG_VERSION"),
        lang: state.lang,
        log_dir: state.paths.logs.display().to_string(),
        data_dir: state.paths.data.display().to_string(),
        releases_url: RELEASES_URL,
        read_only: state.read_only,
        notice: lock(&state.notice).clone(),
        first_run: s.panels.is_empty() && !s.discovery_offered,
    }
}

#[tauri::command]
pub fn dismiss_notice(state: State<'_, Arc<AppState>>) {
    lock(&state.notice).take();
}

#[derive(Serialize)]
pub struct PanelView {
    id: String,
    name: String,
    url: String,
    health_path: String,
    icon: IconPref,
    icon_data: Option<String>,
    state: PanelState,
    detail: String,
    status: Option<u16>,
}

fn views(state: &AppState) -> Vec<PanelView> {
    let panels = state.snapshot().panels;
    let rt = lock(&state.runtime);
    panels
        .into_iter()
        .map(|p| {
            let r = rt.get(&p.id).cloned().unwrap_or_default();
            let icon_data = match p.icon {
                IconPref::Favicon => icons::data_url(&state.paths.icons, &p.id),
                IconPref::Monogram => None,
            };
            PanelView {
                icon_data,
                state: r.state,
                detail: r.detail,
                status: r.status,
                id: p.id,
                name: p.name,
                url: p.url,
                health_path: p.health_path,
                icon: p.icon,
            }
        })
        .collect()
}

#[tauri::command]
pub fn list_panels(state: State<'_, Arc<AppState>>) -> Vec<PanelView> {
    views(&state)
}

#[derive(Deserialize, Debug)]
pub struct PanelInput {
    /// Empty for a new panel.
    #[serde(default)]
    id: String,
    name: String,
    url: String,
    #[serde(default)]
    health_path: Option<String>,
    #[serde(default)]
    icon: IconPref,
}

fn validate(state: &AppState, input: &PanelInput) -> R<(String, String, String)> {
    let name = input.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(user_err(state, E::InvalidName));
    }
    let url = urls::parse_panel_url(&input.url).map_err(|_| user_err(state, E::InvalidUrl))?;
    let hp = input.health_path.as_deref().map(str::trim).unwrap_or("");
    let hp = if hp.is_empty() {
        settings::DEFAULT_HEALTH_PATH.to_string()
    } else {
        hp.to_string()
    };
    if !hp.starts_with('/') || hp.len() > 200 || hp.chars().any(char::is_whitespace) {
        return Err(user_err(state, E::InvalidHealthPath));
    }
    Ok((name, url.to_string(), hp))
}

fn fetch_icons_later(app: &AppHandle, ids: Vec<String>) {
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<Arc<AppState>>().inner().clone();
        for id in ids {
            let Some(p) = state.snapshot().panels.into_iter().find(|p| p.id == id) else {
                continue;
            };
            let Ok(u) = urls::parse_panel_url(&p.url) else {
                continue;
            };
            if let Err(e) = icons::fetch(&u, &state.paths.icons, &id) {
                log::info!("favicon for {:?}: {e}", p.name);
            }
        }
    });
}

/// Replaces the whole panel list (add, edit, remove and reorder from the Settings page).
#[tauri::command]
pub fn save_panels(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    panels: Vec<PanelInput>,
) -> R<Vec<PanelView>> {
    if state.read_only {
        return Err(user_err(&state, E::ReadOnly));
    }
    let old = state.snapshot().panels;
    let mut next: Vec<Panel> = Vec::with_capacity(panels.len());
    let mut refetch = Vec::new();
    for input in &panels {
        let (name, url, health_path) = validate(&state, input)?;
        let existing = old
            .iter()
            .find(|p| !input.id.is_empty() && p.id == input.id);
        let id = existing
            .map(|p| p.id.clone())
            .unwrap_or_else(settings::new_id);
        if next.iter().any(|p| p.id == id) {
            continue;
        }
        if existing.is_none_or(|p| p.url != url) {
            icons::remove(&state.paths.icons, &id);
            refetch.push(id.clone());
        }
        next.push(Panel {
            geometry: existing.and_then(|p| p.geometry.clone()),
            id,
            name,
            url,
            health_path,
            icon: input.icon,
        });
    }
    for p in old.iter().filter(|o| !next.iter().any(|n| n.id == o.id)) {
        icons::remove(&state.paths.icons, &p.id);
    }
    let keep: Vec<(String, String)> = next.iter().map(|p| (p.id.clone(), p.url.clone())).collect();
    {
        // A changed address or health path starts over with a fresh check.
        let mut rt = lock(&state.runtime);
        for p in &next {
            if old
                .iter()
                .find(|o| o.id == p.id)
                .is_none_or(|o| o.url != p.url || o.health_path != p.health_path)
            {
                rt.remove(&p.id);
            }
        }
    }
    state
        .update(true, |s| {
            s.panels = next;
            s.discovery_offered = true;
        })
        .map_err(|_| user_err(&state, E::SaveFailed))?;
    windows::close_stale_panel_windows(&app, &keep);
    fetch_icons_later(&app, refetch);
    lock(&state.tray_signature).clear();
    state.wake();
    Ok(views(&state))
}

#[derive(Deserialize, Debug)]
pub struct NewPanel {
    name: String,
    url: String,
}

/// Adds the panels the user ticked in "Find panels".
#[tauri::command]
pub fn add_panels(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    items: Vec<NewPanel>,
) -> R<Vec<PanelView>> {
    if state.read_only {
        return Err(user_err(&state, E::ReadOnly));
    }
    let mut added = Vec::new();
    for item in &items {
        let input = PanelInput {
            id: String::new(),
            name: item.name.clone(),
            url: item.url.clone(),
            health_path: None,
            icon: IconPref::Favicon,
        };
        let (name, url, health_path) = validate(&state, &input)?;
        added.push(Panel {
            id: settings::new_id(),
            name,
            url,
            health_path,
            icon: IconPref::Favicon,
            geometry: None,
        });
    }
    let ids: Vec<String> = added.iter().map(|p| p.id.clone()).collect();
    state
        .update(true, |s| {
            s.panels.extend(added);
            s.discovery_offered = true;
        })
        .map_err(|_| user_err(&state, E::SaveFailed))?;
    fetch_icons_later(&app, ids);
    lock(&state.tray_signature).clear();
    state.wake();
    Ok(views(&state))
}

#[tauri::command]
pub fn mark_discovery_offered(state: State<'_, Arc<AppState>>) {
    if let Err(e) = state.update(true, |s| s.discovery_offered = true) {
        log::error!("saving discovery flag failed: {e}");
    }
}

#[tauri::command]
pub async fn discover(state: State<'_, Arc<AppState>>) -> R<Vec<Candidate>> {
    let known: Vec<url::Url> = state
        .snapshot()
        .panels
        .iter()
        .filter_map(|p| urls::parse_panel_url(&p.url).ok())
        .collect();
    let found = tauri::async_runtime::spawn_blocking(move || discovery::discover(&known))
        .await
        .unwrap_or_else(|e| {
            log::error!("discovery task failed: {e}");
            Vec::new()
        });
    log::info!("discovery found {} candidate(s)", found.len());
    Ok(found)
}

#[tauri::command]
pub async fn open_panel(app: AppHandle, state: State<'_, Arc<AppState>>, id: String) -> R<()> {
    if !settings::is_valid_id(&id) || !state.snapshot().panels.iter().any(|p| p.id == id) {
        return Err(user_err(&state, E::UnknownPanel));
    }
    windows::open_panel(&app, &id).map_err(|e| {
        log::error!("{e}");
        user_err(&state, E::OpenFailed)
    })
}

#[tauri::command]
pub async fn show_settings(app: AppHandle) {
    windows::show_settings(&app);
}

#[tauri::command]
pub async fn show_launcher(app: AppHandle, find: bool) {
    windows::show_launcher(&app, find);
}

#[derive(Serialize)]
pub struct ContainerView {
    name: String,
    state: String,
    running: bool,
}

#[derive(Serialize)]
pub struct PanelDetail {
    panel: PanelView,
    /// Docker on this computer can be behind the panel (loopback address).
    local: bool,
    engine: Option<Engine>,
    can_start_desktop: bool,
    containers: Vec<ContainerView>,
    busy: bool,
    log_dir: String,
}

#[tauri::command]
pub async fn panel_detail(state: State<'_, Arc<AppState>>, id: String) -> R<PanelDetail> {
    let panel = views(&state)
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| user_err(&state, E::UnknownPanel))?;
    let url = urls::parse_panel_url(&panel.url).map_err(|_| user_err(&state, E::InvalidUrl))?;
    let local = urls::is_loopback(&url);
    let mut engine = None;
    let mut containers = Vec::new();
    if local && panel.state == PanelState::Down {
        let st = state.inner().clone();
        let _ =
            tauri::async_runtime::spawn_blocking(move || monitor::refresh_docker(&st, false)).await;
        let d = lock(&state.docker);
        engine = d.engine.clone();
        if let Some(port) = url.port_or_known_default() {
            containers = docker::backing(&d.containers, port)
                .into_iter()
                .map(|c| ContainerView {
                    name: c.name,
                    state: c.state,
                    running: c.running,
                })
                .collect();
        }
    }
    let busy = lock(&state.docker).busy;
    Ok(PanelDetail {
        panel,
        local,
        engine,
        can_start_desktop: docker::can_start_desktop(),
        containers,
        busy,
        log_dir: state.paths.logs.display().to_string(),
    })
}

struct BusyGuard(Arc<AppState>);
impl Drop for BusyGuard {
    fn drop(&mut self) {
        lock(&self.0.docker).busy = false;
    }
}

fn claim_busy(state: &Arc<AppState>) -> R<BusyGuard> {
    let mut d = lock(&state.docker);
    if d.busy {
        return Err(user_err(state, E::Busy));
    }
    d.busy = true;
    Ok(BusyGuard(state.clone()))
}

#[tauri::command]
pub async fn start_docker(state: State<'_, Arc<AppState>>) -> R<()> {
    let st = state.inner().clone();
    if docker::find_cli().is_none() && !docker::can_start_desktop() {
        return Err(user_err(&st, E::DockerMissing));
    }
    if !docker::can_start_desktop() {
        return Err(user_err(&st, E::DockerDesktopMissing));
    }
    let _guard = claim_busy(&st)?;
    log::info!("user asked to start Docker Desktop");
    docker::start_desktop().map_err(|e| {
        log::error!("{e}");
        user_err(&st, E::DockerStartFailed)
    })?;
    lock(&st.docker).started_desktop_at = Some(std::time::Instant::now());
    st.wake();
    Ok(())
}

/// Starts the stopped containers behind a panel's port. The containers are looked up from
/// Docker itself here; the page only names the panel.
#[tauri::command]
pub async fn start_containers(state: State<'_, Arc<AppState>>, id: String) -> R<()> {
    let st = state.inner().clone();
    let panel = st
        .snapshot()
        .panels
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| user_err(&st, E::UnknownPanel))?;
    let url = urls::parse_panel_url(&panel.url).map_err(|_| user_err(&st, E::InvalidUrl))?;
    let port = url
        .port_or_known_default()
        .filter(|_| urls::is_loopback(&url));
    let Some(port) = port else {
        return Err(user_err(&st, E::NoContainers));
    };
    let guard = claim_busy(&st)?;
    let st2 = st.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(), E> {
        let _guard = guard;
        let cli = docker::find_cli().ok_or(E::DockerMissing)?;
        let all = docker::containers(&cli).map_err(|e| {
            log::error!("{e}");
            E::ContainersStartFailed
        })?;
        let backing = docker::backing(&all, port);
        if backing.is_empty() {
            return Err(E::NoContainers);
        }
        let ids: Vec<String> = backing
            .iter()
            .filter(|c| !c.running)
            .map(|c| c.id.clone())
            .collect();
        let names: Vec<&str> = backing
            .iter()
            .filter(|c| !c.running)
            .map(|c| c.name.as_str())
            .collect();
        log::info!(
            "user asked to start containers for {:?}: {:?}",
            panel.name,
            names
        );
        docker::start(&cli, &ids).map_err(|e| {
            log::error!("{e}");
            E::ContainersStartFailed
        })?;
        monitor::refresh_docker(&st2, true);
        Ok(())
    })
    .await
    .map_err(|e| {
        log::error!("start task failed: {e}");
        user_err(&st, E::ContainersStartFailed)
    })?;
    st.wake();
    result.map_err(|e| user_err(&st, e))
}

#[tauri::command]
pub fn check_now(state: State<'_, Arc<AppState>>) {
    state.wake();
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> bool {
    tray::set_autostart(&app, enabled)
}

#[tauri::command]
pub fn open_log_folder(app: AppHandle, state: State<'_, Arc<AppState>>) -> R<()> {
    let dir = &state.paths.logs;
    let _ = std::fs::create_dir_all(dir);
    app.opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(|e| {
            log::error!("opening log folder failed: {e}");
            user_err(&state, E::LogFolderFailed)
        })
}

#[tauri::command]
pub fn open_releases(app: AppHandle) {
    match url::Url::parse(RELEASES_URL) {
        Ok(u) => windows::open_external(&app, &u),
        Err(e) => log::error!("bad releases URL: {e}"),
    }
}
