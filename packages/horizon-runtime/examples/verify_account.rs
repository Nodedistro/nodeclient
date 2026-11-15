//! Checks the launcher's saved account without printing credentials or account identifiers.
use anyhow::{Context, Result};
#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let config = nodeclient_auth::microsoft::Config {
        client_id: std::env::var("MICROSOFT_CLIENT_ID")?,
        redirect_uri: std::env::var("MICROSOFT_REDIRECT_URI")
            .unwrap_or_else(|_| nodeclient_auth::microsoft::DEFAULT_REDIRECT.into()),
    };
    config.validate()?;
    let root = std::path::PathBuf::from(std::env::var("APPDATA")?).join("app.nodeclient.launcher");
    let store = nodeclient_profiles::Store { root };
    let accounts = store.accounts()?;
    let account = accounts
        .first()
        .context("No saved account; sign in through NodeClient")?;
    match nodeclient_auth::auth_manager::restore(&config, &account.id).await {
        Ok(session) => {
            session.validate_lifetime()?;
            println!("Saved account ownership and session verified.");
            if let Some(directory) = std::env::args().nth(1) {
                use std::sync::{Arc, atomic::AtomicBool};
                let data = std::env::current_dir()?.join(directory);
                let game = data.join("instances/native-verification");
                std::fs::create_dir_all(&game)?;
                let cancel = Arc::new(AtomicBool::new(false));
                let report: nodeclient_downloader::Reporter = Arc::new(|_| {});
                let runtime = Box::pin(nodeclient_horizon::install(&data, cancel.clone(), report.clone())).await?;
                if std::env::args().any(|arg| arg == "--launch-smoke") {
                    anyhow::ensure!(runtime.ready().await?, "Prepare resources before the launch smoke test");
                    session.validate_lifetime()?;
                    use tokio::io::AsyncWriteExt;
                    use std::process::Stdio;
                    let mut command = tokio::process::Command::new(&runtime.client);
                    command.args(runtime.args(&game, &nodeclient_types::Settings::default(), None))
                        .current_dir(&game).stdin(Stdio::piped()).stdout(Stdio::null())
                        .stderr(Stdio::null()).kill_on_drop(true);
                    #[cfg(windows)] command.creation_flags(0x08000000);
                    let mut child = command.spawn()?;
                    let mut input = child.stdin.take().context("Session pipe unavailable")?;
                    let payload = serde_json::to_vec(&serde_json::json!({
                        "name": session.profile.name, "uuid": session.profile.id,
                        "access_token": session.access_token, "user_type": "msa"
                    }))?;
                    input.write_all(&payload).await?;
                    input.shutdown().await?;
                    drop(input);
                    tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                    if let Some(status) = child.try_wait()? {
                        anyhow::bail!("Native client exited during startup: {status}");
                    }
                    child.kill().await?;
                    child.wait().await?;
                    println!("Native client remained running for 15 seconds with a verified session; stopped the smoke-test process. Visual gameplay not assessed.");
                    return Ok(());
                }
                let version = Box::pin(nodeclient_core::version(&data, nodeclient_horizon::MINECRAFT, cancel.clone(), report.clone())).await?;
                println!("Installing official Minecraft resources for preparation.");
                Box::pin(nodeclient_core::install::install(&data, &game, &version, 8, cancel.clone(), report.clone())).await?;
                let selection = nodeclient_types::Java { mode: "automatic".into(), path: None };
                let java = match nodeclient_java::select(&data, &selection, 25).await {
                    Ok(java) => java,
                    Err(_) => Box::pin(nodeclient_java::managed::install(&data,
                        version["javaVersion"]["component"].as_str().context("Missing Java component")?,
                        25, 8, cancel.clone(), report.clone())).await?,
                };
                let server = data.join("versions/26.3/26.3-server.jar");
                Box::pin(nodeclient_downloader::download_all(vec![nodeclient_core::install::artifact(&version["downloads"]["server"], server.clone())?], 8, cancel.clone(), report)).await?;
                Box::pin(nodeclient_horizon::prepare(&runtime, &data, &server, std::path::Path::new(&java.path), cancel, Arc::new(|line| println!("{line}")))).await?;
                println!("Owned resource preparation verified.");
            }
            Ok(())
        }
        Err(_) => anyhow::bail!("Saved account could not be restored. Sign in through NodeClient before testing native resource preparation."),
    }
}
