use super::*;
use nodeclient_auth::auth_manager::Session;
use std::path::Path;

#[allow(clippy::too_many_arguments)]
pub async fn run(
    app: &tauri::AppHandle,
    root: &Path,
    game: &Path,
    instance: &Instance,
    settings: &Settings,
    id: &str,
    install_only: bool,
    server: Option<(String, u16)>,
    session: Session,
) -> Result<()> {
    let state = app.state::<AppState>();
    session.validate_lifetime()?;
    status(
        app,
        "DOWNLOADING",
        "Installing verified Horizon native engine",
        None,
    );
    let runtime = Box::pin(nodeclient_horizon::install(
        root,
        state.cancel.clone(),
        reporter(app),
    ))
    .await?;
    if !game.join("installed.json").exists() || !runtime.ready().await? {
        status(
            app,
            "DOWNLOADING",
            "Preparing owned Minecraft resources for Horizon",
            None,
        );
        let version = Box::pin(nodeclient_core::version(
            root,
            nodeclient_horizon::MINECRAFT,
            state.cancel.clone(),
            reporter(app),
        ))
        .await?;
        Box::pin(nodeclient_core::install::install(
            root,
            game,
            &version,
            settings.concurrency,
            state.cancel.clone(),
            reporter(app),
        ))
        .await?;
        let java = ensure_java(
            app,
            root,
            instance,
            settings,
            &version,
            state.cancel.clone(),
        )
        .await?;
        let server_jar = root.join(format!(
            "versions/{0}/{0}-server.jar",
            nodeclient_horizon::MINECRAFT
        ));
        Box::pin(nodeclient_downloader::download_all(
            vec![nodeclient_core::install::artifact(
                &version["downloads"]["server"],
                server_jar.clone(),
            )?],
            settings.concurrency,
            state.cancel.clone(),
            reporter(app),
        ))
        .await?;
        let log_app = app.clone();
        Box::pin(nodeclient_horizon::prepare(
            &runtime,
            root,
            &server_jar,
            Path::new(&java.path),
            state.cancel.clone(),
            Arc::new(move |line| log(&log_app, line)),
        ))
        .await?;
    }
    nodeclient_profiles::write_json(
        &game.join("installed.json"),
        &serde_json::json!({
            "engine":"horizon","build":nodeclient_horizon::BUILD,"version":instance.minecraft_version,"loader":"vanilla","loaderVersion":null
        }),
    )?;
    if install_only {
        status(app, "IDLE", "Horizon prepared. Ready to play.", None);
        return Ok(());
    }
    if state.cancel.load(Ordering::Relaxed) {
        bail!("Launch cancelled.");
    }
    session.validate_lifetime()?;
    if !state
        .sessions
        .lock()
        .unwrap()
        .contains_key(&session.profile.id)
    {
        bail!("Account was signed out. Sign in again before playing.");
    }
    let args = runtime.args(
        game,
        settings,
        server.as_ref().map(|(host, port)| (host.as_str(), *port)),
    );
    let payload = serde_json::to_vec(
        &serde_json::json!({"name":session.profile.name,"uuid":session.profile.id,"access_token":session.access_token,"user_type":"msa"}),
    )?;
    status(app, "LAUNCHING", "Starting Horizon native Minecraft", None);
    let started_app = app.clone();
    let log_app = app.clone();
    let minimize = settings.minimize_on_launch;
    let info = nodeclient_process::run_with_input(
        &runtime.client,
        &args,
        game,
        nodeclient_process::ProcessInfo {
            instance: id.into(),
            minecraft_version: instance.minecraft_version.clone(),
            ..Default::default()
        },
        vec![session.access_token],
        Arc::new(move |info| {
            status(&started_app, "RUNNING", "Horizon is running", Some(info));
            if minimize {
                if let Some(w) = started_app.get_webview_window("main") {
                    let _ = w.minimize();
                }
            }
        }),
        Arc::new(move |line| log(&log_app, line)),
        Some(payload),
    )
    .await?;
    {
        let store = state.store.lock().unwrap();
        let mut current = store.get(id)?;
        current.last_played = Some(info.start_time);
        current.playtime_seconds += info
            .end_time
            .unwrap_or(info.start_time)
            .saturating_sub(info.start_time);
        store.save(&current)?;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
    }
    status(
        app,
        "IDLE",
        if info.exit_code == Some(0) {
            "Horizon closed."
        } else {
            "Horizon exited with an error. See logs or crash reports."
        },
        Some(info),
    );
    Ok(())
}
