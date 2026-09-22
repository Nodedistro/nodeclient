use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    path::Path,
    process::Stdio,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: Option<u32>,
    pub instance: String,
    pub minecraft_version: String,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub exit_code: Option<i32>,
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub type LogSink = Arc<dyn Fn(String) + Send + Sync>;
pub async fn run(
    java: &Path,
    args: &[String],
    game: &Path,
    mut info: ProcessInfo,
    secrets: Vec<String>,
    started: Arc<dyn Fn(ProcessInfo) + Send + Sync>,
    logs: LogSink,
) -> Result<ProcessInfo> {
    let history = nodeclient_types::safe_join(game, "logs/last-process.json")?;
    let log_path = nodeclient_types::safe_join(game, "logs/nodeclient-console.log")?;
    let file = Arc::new(tokio::sync::Mutex::new(
        tokio::fs::File::create(log_path).await?,
    ));
    let mut command = tokio::process::Command::new(java);
    command
        .args(args)
        .current_dir(game)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS")
        .env_remove("CLASSPATH");
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command.spawn().context("Java could not start Minecraft.")?;
    info.pid = child.id();
    info.start_time = now();
    started(info.clone());

    // Public process metadata only. Recording failure must never abandon a running child.
    if let Ok(bytes) = serde_json::to_vec_pretty(&info) {
        if tokio::fs::write(&history, bytes).await.is_err() {
            logs("Could not save process history.".into());
        }
    }
    let stdout = child
        .stdout
        .take()
        .context("Minecraft stdout unavailable")?;
    let stderr = child
        .stderr
        .take()
        .context("Minecraft stderr unavailable")?;
    let pump = |reader: Box<dyn tokio::io::AsyncRead + Unpin + Send>| {
        let logs = logs.clone();
        let secrets = secrets.clone();
        let file = file.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut clean = nodeclient_types::redact_secrets(&line, &secrets);
                clean.truncate(clean.floor_char_boundary(16384));
                logs(clean.clone());
                clean.push('\n');
                let _ = file.lock().await.write_all(clean.as_bytes()).await;
            }
        })
    };
    let out = pump(Box::new(stdout));
    let err = pump(Box::new(stderr));
    let status = child.wait().await?;
    let _ = out.await;
    let _ = err.await;
    info.end_time = Some(now());
    info.exit_code = Some(status.code().unwrap_or(-1));
    if let Ok(bytes) = serde_json::to_vec_pretty(&info) {
        let _ = tokio::fs::write(history, bytes).await;
    }
    Ok(info)
}
