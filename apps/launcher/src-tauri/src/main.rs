#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod instance_content;
mod modrinth;
mod performance;
mod troubleshoot;
mod updater;
use anyhow::{bail, Context, Result};
use nodeclient_auth::{
    microsoft::{self, Pending},
    token_store::{OsTokenStore, TokenStore},
};
use nodeclient_profiles::Store;
use nodeclient_types::{Instance, Profile, Settings};
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

const AUTH_WINDOW: &str = "microsoft-login";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    phase: String,
    message: String,
    process: Option<nodeclient_process::ProcessInfo>,
}
struct AppState {
    store: Mutex<Store>,
    pending: tokio::sync::Mutex<Option<Pending>>,
    busy: AtomicBool,
    cancel: Arc<AtomicBool>,
    update_cancel: Arc<AtomicBool>,
    status: Mutex<Status>,
    logs: Mutex<Vec<String>>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    settings: Settings,
    instances: Vec<Instance>,
    accounts: Vec<Profile>,
    status: Status,
    total_memory_mb: u64,
    installed: Vec<String>,
}
type CommandResult<T> = std::result::Result<T, String>;
fn error(e: anyhow::Error) -> String {
    nodeclient_types::redact_secrets(&format!("{e:#}"), &[])
}
fn config() -> microsoft::Config {
    microsoft::Config {
        client_id: std::env::var("MICROSOFT_CLIENT_ID")
            .ok()
            .or_else(|| option_env!("MICROSOFT_CLIENT_ID").map(str::to_owned))
            .unwrap_or_default(),
        redirect_uri: std::env::var("MICROSOFT_REDIRECT_URI")
            .ok()
            .or_else(|| option_env!("MICROSOFT_REDIRECT_URI").map(str::to_owned))
            .unwrap_or_else(|| microsoft::DEFAULT_REDIRECT.into()),
    }
}
fn parse_server_target(value: &str) -> Result<(String, u16)> {
    let value = value.trim();
    if value.is_empty() || value.len() > 253 || value.chars().any(|c| c.is_control() || c.is_whitespace()) {
        bail!("Enter a valid Minecraft server address.");
    }
    let (host, port) = if let Some(rest) = value.strip_prefix('[') {
        let (host, rest) = rest.split_once(']').context("Invalid bracketed server address.")?;
        let port = rest.strip_prefix(':').unwrap_or("25565");
        (host, port)
    } else if let Some((host, port)) = value.rsplit_once(':') {
        if host.contains(':') { (value, "25565") } else { (host, port) }
    } else {
        (value, "25565")
    };
    if host.is_empty() || host.len() > 253 || host.contains('/') || host.contains('\\') || host.contains('@') {
        bail!("Enter a valid Minecraft server hostname.");
    }
    let port = port.parse::<u16>().context("Server port must be between 1 and 65535.")?;
    if port == 0 { bail!("Server port must be between 1 and 65535."); }
    Ok((host.to_owned(), port))
}
fn status(
    app: &tauri::AppHandle,
    phase: &str,
    message: &str,
    process: Option<nodeclient_process::ProcessInfo>,
) {
    let state = app.state::<AppState>();
    let value = Status {
        phase: phase.into(),
        message: message.into(),
        process,
    };
    *state.status.lock().unwrap() = value.clone();
    let _ = app.emit("status", value);
}
fn log(app: &tauri::AppHandle, line: String) {
    let state = app.state::<AppState>();
    let line = nodeclient_types::redact_secrets(&line, &[]);
    let mut logs = state.logs.lock().unwrap();
    if logs.len() >= 2000 {
        logs.remove(0);
    }
    logs.push(line.clone());
    let _ = app.emit("log", line);
}
fn reporter(app: &tauri::AppHandle) -> nodeclient_downloader::Reporter {
    let app = app.clone();
    let last = Mutex::new(Instant::now() - Duration::from_secs(1));
    Arc::new(move |progress| {
        let mut last = last.lock().unwrap();
        if last.elapsed() > Duration::from_millis(120) || progress.completed == progress.total {
            *last = Instant::now();
            let _ = app.emit("download-progress", progress);
        }
    })
}
#[tauri::command]
fn snapshot(state: tauri::State<AppState>) -> CommandResult<Snapshot> {
    (|| -> Result<Snapshot> {
        let store = state.store.lock().unwrap();
        let instances = store.instances()?;
        let installed = instances
            .iter()
            .filter(|i| {
                store
                    .instance_dir(&i.id)
                    .ok()
                    .and_then(|p| std::fs::read(p.join("installed.json")).ok())
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                    .is_some_and(|marker| {
                        marker["version"] == i.minecraft_version
                            && marker["loader"].as_str().unwrap_or("vanilla") == i.loader.r#type
                            && marker
                                .get("loaderVersion")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                == i.loader.version.as_deref().unwrap_or("")
                    })
            })
            .map(|i| i.id.clone())
            .collect();
        let system = sysinfo::System::new_with_specifics(
            sysinfo::RefreshKind::nothing().with_memory(sysinfo::MemoryRefreshKind::everything()),
        );
        Ok(Snapshot {
            settings: store.settings()?,
            instances,
            accounts: store.accounts()?,
            status: state.status.lock().unwrap().clone(),
            total_memory_mb: system.total_memory() / 1024 / 1024,
            installed,
        })
    })()
    .map_err(error)
}
#[tauri::command]
fn save_settings(state: tauri::State<AppState>, settings: Settings) -> CommandResult<()> {
    state
        .store
        .lock()
        .unwrap()
        .save_settings(&settings)
        .map_err(error)
}
#[tauri::command]
async fn save_instance(state: tauri::State<'_, AppState>, instance: Instance) -> CommandResult<()> {
    if state.busy.load(Ordering::SeqCst) {
        return Err("Wait until the active operation or game finishes.".into());
    }
    if instance.java.mode == "custom" {
        nodeclient_java::validate(std::path::Path::new(
            instance.java.path.as_deref().unwrap_or(""),
        ))
        .await
        .map_err(error)?;
    }
    let store = state.store.lock().unwrap();
    if state.busy.load(Ordering::SeqCst) {
        return Err("Wait until the active operation finishes.".into());
    }
    let system = sysinfo::System::new_with_specifics(
        sysinfo::RefreshKind::nothing().with_memory(sysinfo::MemoryRefreshKind::everything()),
    );
    if instance.memory.maximum_mb as u64 > system.total_memory() / 1024 / 1024 * 3 / 4 {
        return Err(
            "Leave at least one quarter of system RAM available to the operating system.".into(),
        );
    }
    store.save(&instance).map_err(error)
}
#[tauri::command]
fn clone_instance(state: tauri::State<AppState>, id: String) -> CommandResult<Instance> {
    let store = state.store.lock().unwrap();
    if state.busy.load(Ordering::SeqCst) {
        return Err("Wait until Minecraft closes before cloning an instance.".into());
    }
    store.clone_instance(&id).map_err(error)
}
#[tauri::command]
fn delete_instance(
    state: tauri::State<AppState>,
    id: String,
    confirmed: bool,
) -> CommandResult<()> {
    let store = state.store.lock().unwrap();
    if state.busy.load(Ordering::SeqCst) {
        return Err(
            "Cannot delete an instance while Minecraft or an installation is running.".into(),
        );
    }
    store.delete(&id, confirmed).map_err(error)
}
#[tauri::command]
fn open_instance(state: tauri::State<AppState>, id: String) -> CommandResult<()> {
    (|| -> Result<()> {
        let path = state.store.lock().unwrap().instance_dir(&id)?;
        if !path.is_dir() {
            bail!("Instance folder does not exist.");
        }
        open::that(path)?;
        Ok(())
    })()
    .map_err(error)
}
#[tauri::command]
fn list_mods(
    state: tauri::State<AppState>,
    id: String,
) -> CommandResult<Vec<instance_content::ModEntry>> {
    let root = state.store.lock().unwrap().root.clone();
    instance_content::list_mods(&root, &id).map_err(error)
}
#[tauri::command]
async fn add_mod(
    state: tauri::State<'_, AppState>,
    id: String,
) -> CommandResult<Option<instance_content::ModEntry>> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Add mod (.jar or .zip)")
        .add_filter("Mods", &["jar", "zip"])
        .pick_file()
        .await;
    let Some(file) = file else {
        return Ok(None);
    };
    let root = state.store.lock().unwrap().root.clone();
    instance_content::add_mod(&root, &id, file.path())
        .map(Some)
        .map_err(error)
}
#[tauri::command]
fn remove_mod(state: tauri::State<AppState>, id: String, name: String) -> CommandResult<()> {
    let root = state.store.lock().unwrap().root.clone();
    instance_content::remove_mod(&root, &id, &name).map_err(error)
}
#[tauri::command]
fn set_mod_enabled(
    state: tauri::State<AppState>,
    id: String,
    name: String,
    enabled: bool,
) -> CommandResult<instance_content::ModEntry> {
    let root = state.store.lock().unwrap().root.clone();
    instance_content::set_mod_enabled(&root, &id, &name, enabled).map_err(error)
}
#[tauri::command]
async fn modrinth_search(
    query: String,
    game_version: String,
    loader: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> CommandResult<modrinth::SearchResult> {
    modrinth::search(
        &query,
        &game_version,
        &loader,
        limit.unwrap_or(20),
        offset.unwrap_or(0),
    )
    .await
    .map_err(error)
}
#[tauri::command]
async fn install_modrinth_mod(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
    project_id: String,
) -> CommandResult<instance_content::ModEntry> {
    if !state
        .busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        return Err("Another task is already running.".into());
    }
    state.cancel.store(false, Ordering::Relaxed);
    let result = (async {
        let (root, instance, concurrency) = {
            let store = state.store.lock().unwrap();
            (
                store.root.clone(),
                store.get(&id)?,
                store.settings()?.concurrency,
            )
        };
        let loader = instance.loader.r#type.clone();
        status(
            &app,
            "DOWNLOADING",
            "Finding a compatible Modrinth version",
            None,
        );
        let file = modrinth::latest_compatible_version(
            &project_id,
            &instance.minecraft_version,
            &loader,
        )
        .await?;
        let mods_dir = nodeclient_types::safe_join(
            &root,
            &format!("instances/{id}/mods"),
        )?;
        std::fs::create_dir_all(&mods_dir)?;
        status(
            &app,
            "DOWNLOADING",
            &format!("Downloading {}", file.filename),
            None,
        );
        let path = modrinth::install_file(
            &mods_dir,
            &file,
            concurrency,
            state.cancel.clone(),
            reporter(&app),
        )
        .await?;
        let meta = std::fs::metadata(&path)?;
        status(&app, "IDLE", "Mod installed.", None);
        Ok(instance_content::ModEntry {
            name: file.filename,
            path: path.to_string_lossy().into_owned(),
            size: meta.len(),
            modified: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs()),
            enabled: true,
            sha1: Some(file.sha1),
        })
    })
    .await;
    state.busy.store(false, Ordering::SeqCst);
    if let Err(e) = &result {
        let message = error(anyhow::anyhow!("{e:#}"));
        log(&app, message.clone());
        status(&app, "IDLE", &message, None);
    }
    result.map_err(error)
}
#[tauri::command]
async fn update_modrinth_mod(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
    name: String,
) -> CommandResult<instance_content::ModEntry> {
    if !state
        .busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        return Err("Another task is already running.".into());
    }
    state.cancel.store(false, Ordering::Relaxed);
    let result = (async {
        let (root, instance, concurrency) = {
            let store = state.store.lock().unwrap();
            (
                store.root.clone(),
                store.get(&id)?,
                store.settings()?.concurrency,
            )
        };
        let mods = instance_content::list_mods(&root, &id)?;
        let current = mods
            .into_iter()
            .find(|m| m.name == name)
            .context("Mod not found.")?;
        if !current.enabled {
            bail!("Enable the mod before updating it.");
        }
        let sha1 = current
            .sha1
            .clone()
            .context("Could not hash the installed mod.")?;
        status(&app, "DOWNLOADING", "Checking Modrinth for updates", None);
        let Some(file) = modrinth::latest_from_hash(
            &sha1,
            &instance.minecraft_version,
            &instance.loader.r#type,
        )
        .await?
        else {
            bail!("No newer compatible Modrinth version found.");
        };
        if file.sha1.eq_ignore_ascii_case(&sha1) {
            bail!("This mod is already up to date.");
        }
        let mods_dir = nodeclient_types::safe_join(&root, &format!("instances/{id}/mods"))?;
        // Remove old file then install new (may rename).
        instance_content::remove_mod(&root, &id, &name)?;
        status(
            &app,
            "DOWNLOADING",
            &format!("Updating to {}", file.version_number),
            None,
        );
        let path = modrinth::install_file(
            &mods_dir,
            &file,
            concurrency,
            state.cancel.clone(),
            reporter(&app),
        )
        .await?;
        let meta = std::fs::metadata(&path)?;
        status(&app, "IDLE", "Mod updated.", None);
        Ok(instance_content::ModEntry {
            name: file.filename,
            path: path.to_string_lossy().into_owned(),
            size: meta.len(),
            modified: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs()),
            enabled: true,
            sha1: Some(file.sha1),
        })
    })
    .await;
    state.busy.store(false, Ordering::SeqCst);
    if let Err(e) = &result {
        let message = error(anyhow::anyhow!("{e:#}"));
        log(&app, message.clone());
        status(&app, "IDLE", &message, None);
    }
    result.map_err(error)
}
#[tauri::command]
fn list_screenshots(
    state: tauri::State<AppState>,
    id: String,
) -> CommandResult<Vec<instance_content::FileEntry>> {
    let root = state.store.lock().unwrap().root.clone();
    instance_content::list_screenshots(&root, &id).map_err(error)
}
#[tauri::command]
fn delete_screenshot(
    state: tauri::State<AppState>,
    id: String,
    name: String,
) -> CommandResult<()> {
    let root = state.store.lock().unwrap().root.clone();
    instance_content::delete_screenshot(&root, &id, &name).map_err(error)
}
#[tauri::command]
fn read_screenshot(
    state: tauri::State<AppState>,
    id: String,
    name: String,
) -> CommandResult<Vec<u8>> {
    let root = state.store.lock().unwrap().root.clone();
    instance_content::read_screenshot(&root, &id, &name).map_err(error)
}
#[tauri::command]
fn open_instance_folder(
    state: tauri::State<AppState>,
    id: String,
    folder: String,
) -> CommandResult<()> {
    let root = state.store.lock().unwrap().root.clone();
    instance_content::open_subdir(&root, &id, &folder).map_err(error)
}
#[tauri::command]
fn list_vanilla_worlds() -> CommandResult<Vec<instance_content::FileEntry>> {
    instance_content::list_vanilla_worlds().map_err(error)
}
#[tauri::command]
fn list_instance_worlds(
    state: tauri::State<AppState>,
    id: String,
) -> CommandResult<Vec<instance_content::FileEntry>> {
    let root = state.store.lock().unwrap().root.clone();
    instance_content::list_instance_worlds(&root, &id).map_err(error)
}
#[tauri::command]
fn import_vanilla_worlds(
    state: tauri::State<AppState>,
    id: String,
    names: Option<Vec<String>>,
) -> CommandResult<u32> {
    let root = state.store.lock().unwrap().root.clone();
    instance_content::import_vanilla_worlds(&root, &id, names).map_err(error)
}
#[tauri::command]
fn list_servers(
    state: tauri::State<AppState>,
    id: String,
) -> CommandResult<Vec<instance_content::ServerEntry>> {
    let root = state.store.lock().unwrap().root.clone();
    instance_content::list_servers(&root, &id).map_err(error)
}
#[tauri::command]
fn save_servers(
    state: tauri::State<AppState>,
    id: String,
    servers: Vec<instance_content::ServerEntry>,
) -> CommandResult<()> {
    let root = state.store.lock().unwrap().root.clone();
    instance_content::save_servers(&root, &id, &servers).map_err(error)
}
#[tauri::command]
async fn versions(state: tauri::State<'_, AppState>) -> CommandResult<nodeclient_core::Manifest> {
    let root = state.store.lock().unwrap().root.clone();
    nodeclient_core::manifest_cached(Some(&root)).await.map_err(error)
}
#[tauri::command]
fn jvm_performance_preset(preset: String) -> CommandResult<Vec<String>> {
    performance::jvm_preset(&preset).map_err(error)
}
#[tauri::command]
fn apply_fps_video_settings(
    state: tauri::State<AppState>,
    id: String,
) -> CommandResult<()> {
    let root = state.store.lock().unwrap().root.clone();
    performance::apply_fps_video_settings(&root, &id)
        .map(|_| ())
        .map_err(error)
}
#[tauri::command]
async fn install_performance_mods(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
) -> CommandResult<u32> {
    if !state
        .busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        return Err("Another task is already running.".into());
    }
    state.cancel.store(false, Ordering::Relaxed);
    let result = (async {
        let (root, instance, concurrency) = {
            let store = state.store.lock().unwrap();
            (
                store.root.clone(),
                store.get(&id)?,
                store.settings()?.concurrency,
            )
        };
        let projects = performance::performance_mod_projects(&instance.loader.r#type)?;
        let mods_dir =
            nodeclient_types::safe_join(&root, &format!("instances/{id}/mods"))?;
        std::fs::create_dir_all(&mods_dir)?;
        let mut installed = 0u32;
        let mut errors = vec![];
        for project in projects {
            status(
                &app,
                "DOWNLOADING",
                &format!("Installing performance mod {project}"),
                None,
            );
            match modrinth::latest_compatible_version(
                project,
                &instance.minecraft_version,
                &instance.loader.r#type,
            )
            .await
            {
                Ok(file) => {
                    let dest = mods_dir.join(&file.filename);
                    let disabled = mods_dir.join(format!("{}.disabled", file.filename));
                    if dest.is_file() || disabled.is_file() {
                        installed += 1;
                        continue;
                    }
                    match modrinth::install_file(
                        &mods_dir,
                        &file,
                        concurrency,
                        state.cancel.clone(),
                        reporter(&app),
                    )
                    .await
                    {
                        Ok(_) => installed += 1,
                        Err(e) => errors.push(format!("{project}: {e:#}")),
                    }
                }
                Err(e) => errors.push(format!("{project}: {e:#}")),
            }
        }
        if installed == 0 {
            bail!(
                "No performance mods could be installed. {}",
                errors.join(" · ")
            );
        }
        status(
            &app,
            "IDLE",
            &format!("Installed {installed} performance mod(s)."),
            None,
        );
        Ok(installed)
    })
    .await;
    state.busy.store(false, Ordering::SeqCst);
    if let Err(e) = &result {
        let message = error(anyhow::anyhow!("{e:#}"));
        log(&app, message.clone());
        status(&app, "IDLE", &message, None);
    }
    result.map_err(error)
}
#[tauri::command]
async fn fabric_loaders(
    game_version: String,
) -> CommandResult<Vec<nodeclient_core::fabric::FabricLoaderVersion>> {
    nodeclient_core::fabric::list_loaders(&game_version)
        .await
        .map_err(error)
}
#[tauri::command]
async fn loader_versions(
    loader: String,
    game_version: String,
) -> CommandResult<Vec<nodeclient_core::fabric::FabricLoaderVersion>> {
    nodeclient_core::loader::list_versions(&loader, &game_version)
        .await
        .map_err(error)
}
async fn ensure_java(
    app: &tauri::AppHandle,
    root: &std::path::Path,
    instance: &nodeclient_types::Instance,
    settings: &nodeclient_types::Settings,
    version: &serde_json::Value,
    cancel: Arc<AtomicBool>,
) -> Result<nodeclient_java::Runtime> {
    let major = nodeclient_core::metadata::java_major(version);
    match nodeclient_java::select(root, &instance.java, major).await {
        Ok(runtime) => Ok(runtime),
        Err(e) if instance.java.mode == "custom" => Err(e),
        Err(_) => {
            status(
                app,
                "DOWNLOADING",
                &format!("Installing official Java {major}"),
                None,
            );
            let component = version["javaVersion"]["component"]
                .as_str()
                .unwrap_or("jre-legacy");
            nodeclient_java::managed::install(
                root,
                component,
                major,
                settings.concurrency,
                cancel,
                reporter(app),
            )
            .await
        }
    }
}
#[tauri::command]
async fn java_runtimes(
    state: tauri::State<'_, AppState>,
) -> CommandResult<Vec<nodeclient_java::Runtime>> {
    let root = state.store.lock().unwrap().root.clone();
    Ok(nodeclient_java::detect(&root).await)
}
#[tauri::command]
async fn browse_java() -> CommandResult<Option<nodeclient_java::Runtime>> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Choose java.exe or javaw.exe")
        .pick_file()
        .await;
    match file {
        Some(file) => nodeclient_java::validate(file.path())
            .await
            .map(Some)
            .map_err(error),
        None => Ok(None),
    }
}
#[tauri::command]
async fn start_microsoft_login(app: tauri::AppHandle) -> CommandResult<String> {
    start_login_inner(app).await.map_err(error)
}
fn is_oauth_redirect(expected: &reqwest::Url, navigated: &reqwest::Url) -> bool {
    navigated.origin() == expected.origin()
        && navigated.path() == expected.path()
        && navigated.fragment().is_none()
        && navigated.query().is_some()
}
fn close_auth_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(AUTH_WINDOW) {
        let _ = window.close();
    }
}
async fn start_login_inner(app: tauri::AppHandle) -> Result<String> {
    let state = app.state::<AppState>();
    let mut slot = state.pending.lock().await;
    if slot
        .as_ref()
        .is_some_and(|p| p.created.elapsed() < Duration::from_secs(600))
    {
        bail!("A Microsoft sign-in is already pending. Cancel it before starting another.");
    }
    close_auth_window(&app);
    let (pending, url) = microsoft::start(config())?;
    let redirect = reqwest::Url::parse(&pending.config.redirect_uri)?;
    *slot = Some(pending);
    drop(slot);
    let authorize: reqwest::Url = url
        .parse()
        .context("Invalid Microsoft authorize URL.")?;
    let handle = app.clone();
    let redirect_for_nav = redirect.clone();
    let window = WebviewWindowBuilder::new(&app, AUTH_WINDOW, WebviewUrl::External(authorize))
        .title("Sign in with Microsoft")
        .inner_size(520.0, 720.0)
        .resizable(true)
        .center()
        .on_navigation(move |navigated| {
            if !is_oauth_redirect(&redirect_for_nav, &navigated) {
                return true;
            }
            let handle = handle.clone();
            let callback = navigated.to_string();
            tauri::async_runtime::spawn(async move {
                close_auth_window(&handle);
                if let Err(e) = finish_login(&handle, &callback).await {
                    let _ = handle.emit("auth-error", error(e));
                }
            });
            false
        })
        .build()
        .map_err(|e| {
            let state = app.state::<AppState>();
            if let Ok(mut slot) = state.pending.try_lock() {
                *slot = None;
            }
            e
        })?;
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            let handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                let mut slot = state.pending.lock().await;
                if slot.take().is_some() {
                    let _ = handle.emit("auth-error", "Microsoft sign-in was cancelled.");
                }
            });
        }
    });
    Ok("embedded".into())
}
async fn finish_login(app: &tauri::AppHandle, callback: &str) -> Result<Profile> {
    let state = app.state::<AppState>();
    let pending = state
        .pending
        .lock()
        .await
        .take()
        .context("No Microsoft sign-in is pending. Start sign-in again.")?;
    let session = nodeclient_auth::auth_manager::finish(pending, callback).await?;
    {
        let store = state.store.lock().unwrap();
        store.save_account(session.profile.clone())?;
        let mut settings = store.settings()?;
        settings.selected_account = Some(session.profile.id.clone());
        store.save_settings(&settings)?;
    }
    let _ = app.emit("account-changed", &session.profile);
    Ok(session.profile)
}
#[tauri::command]
async fn cancel_login(app: tauri::AppHandle) -> CommandResult<()> {
    *app.state::<AppState>().pending.lock().await = None;
    close_auth_window(&app);
    Ok(())
}
#[tauri::command]
fn remove_account(state: tauri::State<AppState>, id: String) -> CommandResult<()> {
    (|| -> Result<()> {
        OsTokenStore.remove(&id)?;
        let store = state.store.lock().unwrap();
        store.remove_account(&id)?;
        let mut s = store.settings()?;
        if s.selected_account.as_deref() == Some(&id) {
            s.selected_account = None;
            store.save_settings(&s)?;
        }
        Ok(())
    })()
    .map_err(error)
}
#[tauri::command]
fn cancel_download(state: tauri::State<AppState>) {
    state.cancel.store(true, Ordering::Relaxed);
}
#[tauri::command]
async fn check_for_updates() -> CommandResult<updater::UpdateInfo> {
    updater::check().await.map_err(error)
}
#[tauri::command]
async fn install_update(
    app: tauri::AppHandle,
    info: updater::UpdateInfo,
) -> CommandResult<String> {
    let cancel = app.state::<AppState>().update_cancel.clone();
    updater::download_and_launch(&app, &info, cancel)
        .await
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(error)
}
#[tauri::command]
fn cancel_update(state: tauri::State<AppState>) {
    state.update_cancel.store(true, Ordering::Relaxed);
}
#[tauri::command]
async fn launch(
    app: tauri::AppHandle,
    id: String,
    install_only: bool,
    safe_mode: Option<bool>,
    server: Option<String>,
) -> CommandResult<()> {
    let state = app.state::<AppState>();
    if state
        .busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("Minecraft or an installation is already running.".into());
    }
    state.cancel.store(false, Ordering::Relaxed);
    let target = server.as_deref().map(parse_server_target).transpose().map_err(error)?;
    let result = launch_inner(&app, &id, install_only, safe_mode.unwrap_or(false), target).await;
    state.busy.store(false, Ordering::SeqCst);
    if let Err(e) = &result {
        let message = error(anyhow::anyhow!("{e:#}"));
        log(&app, message.clone());
        status(&app, "IDLE", &message, None);
    }
    result.map_err(error)
}
#[tauri::command]
fn launch_bedrock(app: tauri::AppHandle) -> CommandResult<()> {
    #[cfg(target_os = "windows")]
    {
        status(&app, "LAUNCHING", "Opening Minecraft for Windows", None);
        std::process::Command::new("explorer.exe")
            .arg("minecraft://")
            .spawn()
            .map_err(|e| format!("Minecraft for Windows could not be opened: {e}"))?;
        status(&app, "IDLE", "Minecraft for Windows opened.", None);
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("Bedrock launch integration is currently available on Windows only.".into())
    }
}
#[tauri::command]
fn open_bedrock_store() -> CommandResult<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg("ms-windows-store://pdp/?productid=9NBLGGH2JHXJ")
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Microsoft Store could not be opened: {e}"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Microsoft Store installation is currently available on Windows only.".into())
    }
}
#[tauri::command]
async fn repair_instance(app: tauri::AppHandle, id: String) -> CommandResult<()> {
    let state = app.state::<AppState>();
    if state
        .busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("Minecraft or an installation is already running.".into());
    }
    state.cancel.store(false, Ordering::Relaxed);
    let result = (async {
        let game = {
            let store = state.store.lock().unwrap();
            store.instance_dir(&id)?
        };
        troubleshoot::clear_install_marker(&game)?;
        status(&app, "DOWNLOADING", "Repairing instance files", None);
        launch_inner(&app, &id, true, false, None).await
    })
    .await;
    state.busy.store(false, Ordering::SeqCst);
    if let Err(e) = &result {
        let message = error(anyhow::anyhow!("{e:#}"));
        log(&app, message.clone());
        status(&app, "IDLE", &message, None);
    }
    result.map_err(error)
}
async fn launch_inner(
    app: &tauri::AppHandle,
    id: &str,
    install_only: bool,
    safe_mode: bool,
    server: Option<(String, u16)>,
) -> Result<()> {
    let state = app.state::<AppState>();
    let (root, game, instance, settings) = {
        let store = state.store.lock().unwrap();
        (
            store.root.clone(),
            store.instance_dir(id)?,
            store.get(id)?,
            store.settings()?,
        )
    };
    let disabled_mods = if safe_mode && !install_only {
        status(
            app,
            "DOWNLOADING",
            "Safe mode: temporarily disabling mods",
            None,
        );
        troubleshoot::disable_enabled_mods(&root, id)?
    } else {
        vec![]
    };
    let launch_result = launch_body(app, &root, &game, &instance, &settings, id, install_only, server).await;
    if !disabled_mods.is_empty() {
        let _ = troubleshoot::restore_mods(&root, id, &disabled_mods);
        if launch_result.is_ok() {
            log(
                app,
                format!(
                    "Safe mode restored {} mod{}.",
                    disabled_mods.len(),
                    if disabled_mods.len() == 1 { "" } else { "s" }
                ),
            );
        }
    }
    launch_result
}
async fn launch_body(
    app: &tauri::AppHandle,
    root: &std::path::Path,
    game: &std::path::Path,
    instance: &Instance,
    settings: &Settings,
    id: &str,
    install_only: bool,
    server: Option<(String, u16)>,
) -> Result<()> {
    if instance.edition == "bedrock" {
        bail!("Bedrock Edition instances are saved, but Bedrock launch support is not available yet. Java Edition uses the verified launcher flow today.");
    }
    let state = app.state::<AppState>();
    let session = if install_only {
        None
    } else {
        status(
            app,
            "AUTHENTICATING",
            "Checking your Minecraft account",
            None,
        );
        Some(
            nodeclient_auth::auth_manager::restore(
                &config(),
                settings
                    .selected_account
                    .as_deref()
                    .context("Sign in with Microsoft and select an account before playing.")?,
            )
            .await?,
        )
    };
    if !install_only {
        match instance_content::ensure_vanilla_worlds_imported(root, id) {
            Ok(n) if n > 0 => {
                status(
                    app,
                    "DOWNLOADING",
                    &format!(
                        "Imported {n} world{} from AppData/.minecraft/saves",
                        if n == 1 { "" } else { "s" }
                    ),
                    None,
                );
                log(
                    app,
                    format!(
                        "Imported {n} world{} from %AppData%/.minecraft/saves into this instance.",
                        if n == 1 { "" } else { "s" }
                    ),
                );
            }
            Ok(_) => {}
            Err(e) => {
                log(app, format!("Could not import AppData worlds: {e:#}"));
            }
        }
    }
    let needs_installer = matches!(
        instance.loader.r#type.as_str(),
        "forge" | "neoforge"
    );
    status(
        app,
        "DOWNLOADING",
        match instance.loader.r#type.as_str() {
            "fabric" => "Resolving Fabric + Minecraft files",
            "quilt" => "Resolving Quilt + Minecraft files",
            "forge" => "Preparing Forge installer",
            "neoforge" => "Preparing NeoForge installer",
            _ => "Resolving official Minecraft files",
        },
        None,
    );
    let (version, runtime) = if needs_installer {
        let vanilla = nodeclient_core::version(
            &root,
            &instance.minecraft_version,
            state.cancel.clone(),
            reporter(app),
        )
        .await?;
        let runtime =
            ensure_java(app, &root, &instance, &settings, &vanilla, state.cancel.clone()).await?;
        status(
            app,
            "DOWNLOADING",
            match instance.loader.r#type.as_str() {
                "neoforge" => "Running NeoForge installer",
                _ => "Running Forge installer",
            },
            None,
        );
        let version = nodeclient_core::loader::resolve(
            &root,
            &instance.minecraft_version,
            &instance.loader,
            state.cancel.clone(),
            reporter(app),
            Some(std::path::Path::new(&runtime.path)),
        )
        .await?;
        (version, runtime)
    } else {
        let version = nodeclient_core::loader::resolve(
            &root,
            &instance.minecraft_version,
            &instance.loader,
            state.cancel.clone(),
            reporter(app),
            None,
        )
        .await?;
        let runtime =
            ensure_java(app, &root, &instance, &settings, &version, state.cancel.clone()).await?;
        (version, runtime)
    };
    let installation = nodeclient_core::install::install(
        &root,
        &game,
        &version,
        settings.concurrency,
        state.cancel.clone(),
        reporter(app),
    )
    .await?;
    nodeclient_profiles::write_json(
        &nodeclient_types::safe_join(&game, "installed.json")?,
        &serde_json::json!({
            "version": instance.minecraft_version,
            "loader": instance.loader.r#type,
            "loaderVersion": instance.loader.version,
            "java": runtime.major
        }),
    )?;
    if install_only {
        status(app, "IDLE", "Installation verified. Ready to play.", None);
        return Ok(());
    }
    if state.cancel.load(Ordering::Relaxed) {
        bail!("Launch cancelled.");
    }
    let session = session.context("Authenticated session missing")?;
    if let Some((host, port)) = &server {
        let address = if host.contains(':') {
            format!("[{host}]:{port}")
        } else {
            format!("{host}:{port}")
        };
        let mut saved = instance_content::list_servers(root, id).unwrap_or_default();
        if !saved.iter().any(|item| item.ip.eq_ignore_ascii_case(&address)) {
            saved.push(instance_content::ServerEntry {
                name: host.clone(),
                ip: address,
            });
            instance_content::save_servers(root, id, &saved)?;
        }
    }
    let args = nodeclient_core::arguments::construct(
        &version,
        &installation,
        &game,
        &instance,
        &settings,
        &session.profile,
        &session.access_token,
        &config().client_id,
        server.as_ref().map(|(host, port)| (host.as_str(), *port)),
    )?;
    status(app, "LAUNCHING", "Starting Minecraft", None);
    let started_app = app.clone();
    let log_app = app.clone();
    let minimize = settings.minimize_on_launch;
    let info = nodeclient_process::run(
        std::path::Path::new(&runtime.path),
        &args,
        &game,
        nodeclient_process::ProcessInfo {
            instance: id.into(),
            minecraft_version: instance.minecraft_version.clone(),
            ..Default::default()
        },
        vec![session.access_token],
        Arc::new(move |info| {
            status(&started_app, "RUNNING", "Minecraft is running", Some(info));
            if minimize {
                if let Some(w) = started_app.get_webview_window("main") {
                    let _ = w.minimize();
                }
            }
        }),
        Arc::new(move |line| log(&log_app, line)),
    )
    .await?;
    {
        let store = state.store.lock().unwrap();
        let mut i = store.get(id)?;
        i.last_played = Some(info.start_time);
        i.playtime_seconds += info
            .end_time
            .unwrap_or(info.start_time)
            .saturating_sub(info.start_time);
        store.save(&i)?;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
    }
    status(
        app,
        "IDLE",
        if info.exit_code == Some(0) {
            "Minecraft closed."
        } else {
            "Minecraft appears to have crashed."
        },
        Some(info),
    );
    Ok(())
}
#[tauri::command]
fn read_logs(
    state: tauri::State<AppState>,
    id: Option<String>,
    kind: String,
) -> CommandResult<String> {
    (|| -> Result<String> {
        if kind == "NodeClient" {
            return Ok(state.logs.lock().unwrap().join("\n"));
        }
        let store = state.store.lock().unwrap();
        let game = store.instance_dir(id.as_deref().context("Select an instance first.")?)?;
        let path = if kind == "Crash Reports" {
            let directory = nodeclient_types::safe_join(&game, "crash-reports")?;
            let mut files = std::fs::read_dir(directory)?
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                .collect::<Vec<_>>();
            files.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
            match files.last() {
                Some(e) => nodeclient_types::safe_join(
                    &game,
                    &format!("crash-reports/{}", e.file_name().to_string_lossy()),
                )?,
                None => return Ok("No crash reports found.".into()),
            }
        } else {
            nodeclient_types::safe_join(&game, "logs/nodeclient-console.log")?
        };
        if !path.exists() {
            return Ok("No Minecraft output yet.".into());
        }
        let bytes = std::fs::read(path)?;
        let text = String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(2 * 1024 * 1024)..]);
        let sanitized = nodeclient_types::redact_secrets(&text, &[]);
        Ok(sanitized.replace(&store.root.to_string_lossy().to_string(), "<NODECLIENT>"))
    })()
    .map_err(error)
}
#[tauri::command]
fn explain_crash(
    state: tauri::State<AppState>,
    id: String,
) -> CommandResult<troubleshoot::CrashExplanation> {
    let root = state.store.lock().unwrap().root.clone();
    troubleshoot::explain_crash(&root, &id).map_err(error)
}
#[tauri::command]
fn list_backups(
    state: tauri::State<AppState>,
    id: String,
) -> CommandResult<Vec<troubleshoot::BackupEntry>> {
    let root = state.store.lock().unwrap().root.clone();
    troubleshoot::list_backups(&root, &id).map_err(error)
}
#[tauri::command]
fn create_backup(
    state: tauri::State<AppState>,
    id: String,
) -> CommandResult<troubleshoot::BackupEntry> {
    let root = state.store.lock().unwrap().root.clone();
    troubleshoot::create_backup(&root, &id).map_err(error)
}
#[tauri::command]
fn restore_backup(
    state: tauri::State<AppState>,
    id: String,
    name: String,
) -> CommandResult<()> {
    let root = state.store.lock().unwrap().root.clone();
    troubleshoot::restore_backup(&root, &id, &name).map_err(error)
}
#[tauri::command]
fn delete_backup(
    state: tauri::State<AppState>,
    id: String,
    name: String,
) -> CommandResult<()> {
    let root = state.store.lock().unwrap().root.clone();
    troubleshoot::delete_backup(&root, &id, &name).map_err(error)
}
fn main() {
    #[cfg(debug_assertions)]
    {
        let _ = dotenvy::from_path(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../.env"),
        );
    }
    tauri::Builder::default()
        .setup(|app| {
            let root = app.path().app_data_dir()?;
            let store = Store::new(root)?;
            let mut settings = store.settings()?;
            if !settings.remember_instance {
                settings.selected_instance = None;
                store.save_settings(&settings)?;
            }
            app.manage(AppState {
                store: Mutex::new(store),
                pending: tokio::sync::Mutex::new(None),
                busy: AtomicBool::new(false),
                cancel: Arc::new(AtomicBool::new(false)),
                update_cancel: Arc::new(AtomicBool::new(false)),
                status: Mutex::new(Status {
                    phase: "IDLE".into(),
                    message: "Ready".into(),
                    process: None,
                }),
                logs: Mutex::new(vec![]),
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.state::<AppState>().busy.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.minimize();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            save_settings,
            save_instance,
            clone_instance,
            delete_instance,
            open_instance,
            list_mods,
            add_mod,
            remove_mod,
            set_mod_enabled,
            modrinth_search,
            install_modrinth_mod,
            update_modrinth_mod,
            list_screenshots,
            delete_screenshot,
            read_screenshot,
            open_instance_folder,
            list_vanilla_worlds,
            list_instance_worlds,
            import_vanilla_worlds,
            list_servers,
            save_servers,
            versions,
            fabric_loaders,
            jvm_performance_preset,
            apply_fps_video_settings,
            install_performance_mods,
            loader_versions,
            java_runtimes,
            browse_java,
            start_microsoft_login,
            cancel_login,
            remove_account,
            cancel_download,
            check_for_updates,
            install_update,
            cancel_update,
            launch,
            launch_bedrock,
            open_bedrock_store,
            repair_instance,
            explain_crash,
            list_backups,
            create_backup,
            restore_backup,
            delete_backup,
            read_logs
        ])
        .run(tauri::generate_context!())
        .expect("NodeClient could not start");
}
