#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
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
                    .is_some_and(|marker| marker["version"] == i.minecraft_version)
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
async fn versions(state: tauri::State<'_, AppState>) -> CommandResult<nodeclient_core::Manifest> {
    let root = state.store.lock().unwrap().root.clone();
    nodeclient_core::manifest_cached(Some(&root)).await.map_err(error)
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
async fn launch(app: tauri::AppHandle, id: String, install_only: bool) -> CommandResult<()> {
    let state = app.state::<AppState>();
    if state
        .busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("Minecraft or an installation is already running.".into());
    }
    state.cancel.store(false, Ordering::Relaxed);
    let result = launch_inner(&app, &id, install_only).await;
    state.busy.store(false, Ordering::SeqCst);
    if let Err(e) = &result {
        let message = error(anyhow::anyhow!("{e:#}"));
        log(&app, message.clone());
        status(&app, "IDLE", &message, None);
    }
    result.map_err(error)
}
async fn launch_inner(app: &tauri::AppHandle, id: &str, install_only: bool) -> Result<()> {
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
    status(
        app,
        "DOWNLOADING",
        "Resolving official Minecraft files",
        None,
    );
    let version = nodeclient_core::version(
        &root,
        &instance.minecraft_version,
        state.cancel.clone(),
        reporter(app),
    )
    .await?;
    let major = nodeclient_core::metadata::java_major(&version);
    let runtime = match nodeclient_java::select(&root, &instance.java, major).await {
        Ok(runtime) => runtime,
        Err(e) if instance.java.mode == "custom" => return Err(e),
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
                &root,
                component,
                major,
                settings.concurrency,
                state.cancel.clone(),
                reporter(app),
            )
            .await?
        }
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
        &serde_json::json!({"version":instance.minecraft_version,"java":runtime.major}),
    )?;
    if install_only {
        status(app, "IDLE", "Installation verified. Ready to play.", None);
        return Ok(());
    }
    if state.cancel.load(Ordering::Relaxed) {
        bail!("Launch cancelled.");
    }
    let session = session.context("Authenticated session missing")?;
    let args = nodeclient_core::arguments::construct(
        &version,
        &installation,
        &game,
        &instance,
        &settings,
        &session.profile,
        &session.access_token,
        &config().client_id,
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
            versions,
            java_runtimes,
            browse_java,
            start_microsoft_login,
            cancel_login,
            remove_account,
            cancel_download,
            launch,
            read_logs
        ])
        .run(tauri::generate_context!())
        .expect("NodeClient could not start");
}
