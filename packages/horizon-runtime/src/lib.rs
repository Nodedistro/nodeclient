//! Version-pinned Horizon Launch API adapter. Tokens are passed by stdin by the process manager.
use anyhow::{bail, Context, Result};
use nodeclient_downloader::{Download, Reporter};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub const BUILD: &str = "nightly-2026-11-13-9f42064";
pub const MINECRAFT: &str = "26.3";
pub const ARCHIVE_SHA256: &str = "f47f9f988b7c30b727dfb35aed2b74f01428925b2eccc9bafff2352f4847f7d6";
const ARCHIVE_SIZE: u64 = 59252227;
const ASSET_URL: &str =
    "https://api.github.com/repos/horizon-rust/horizon/releases/assets/742769793";
const CLIENT_SHA256: &str = "fdf016250a1e2b5e203e97a9af5e8c217e67ba1dd44d25286b50ea913f716504";
const PREPARE_SHA256: &str = "ed5b7b3ebaef031e52037c6664a501a25b7389d3e663611503207a885960a85e";
pub type Log = Arc<dyn Fn(String) + Send + Sync>;

pub struct Runtime {
    pub root: PathBuf,
    pub client: PathBuf,
    pub prepare: PathBuf,
    pub resources: PathBuf,
}
impl Runtime {
    pub fn at(data: &Path) -> Self {
        let root = data
            .join("engines/horizon")
            .join(BUILD)
            .join("horizon-windows-x64");
        Self {
            client: root.join("horizon-client.exe"),
            prepare: root.join("horizon-prepare.exe"),
            resources: root.join("prepared"),
            root,
        }
    }
    pub async fn check_api(&self) -> Result<()> {
        let client = probe(&self.client, &self.root, &["--launch-api"]).await?;
        validate_api(
            &client,
            &[
                "--resources",
                "--home",
                "--game-dir",
                "--session-stdin",
                "--width",
                "--height",
                "--fullscreen",
                "--quickPlayMultiplayer",
            ],
        )?;
        let prepare = probe(&self.prepare, &self.root, &["--launch-api"]).await?;
        validate_api(
            &prepare,
            &[
                "--prepare",
                "--root",
                "--output",
                "--complete",
                "--progress",
                "--version-json",
                "--client-jar",
                "--libraries-dir",
                "--assets-dir",
                "--server-jar",
                "--java-home",
            ],
        )?;
        if prepare["java_major"] != 25 {
            bail!("Horizon preparer requires an unsupported Java version");
        }
        Ok(())
    }
    pub async fn ready(&self) -> Result<bool> {
        let mut cmd = command(&self.prepare, &self.root);
        cmd.arg("--check").arg("--output").arg(&self.resources);
        let out = tokio::time::timeout(Duration::from_secs(30), cmd.output())
            .await
            .context("Horizon resource check timed out")??;
        Ok(out.status.success())
    }
    pub fn args(
        &self,
        game: &Path,
        settings: &nodeclient_types::Settings,
        server: Option<(&str, u16)>,
    ) -> Vec<String> {
        let mut args = vec![
            "--resources".into(),
            self.resources.to_string_lossy().into_owned(),
            "--game-dir".into(),
            game.to_string_lossy().into_owned(),
            "--home".into(),
            game.to_string_lossy().into_owned(),
            "--width".into(),
            settings.width.to_string(),
            "--height".into(),
            settings.height.to_string(),
            "--session-stdin".into(),
        ];
        if settings.fullscreen {
            args.push("--fullscreen".into());
        }
        if let Some((host, port)) = server {
            args.push("--quickPlayMultiplayer".into());
            args.push(if host.contains(':') {
                format!("[{host}]:{port}")
            } else {
                format!("{host}:{port}")
            });
        }
        args
    }
}
pub fn validate_api(api: &Value, options: &[&str]) -> Result<()> {
    if api["launch_api"] != "0.1.0" || api["minecraft"] != MINECRAFT {
        bail!("Horizon runtime does not match the supported launch API or Minecraft version");
    }
    let advertised = api["options"]
        .as_array()
        .context("Horizon API options missing")?;
    for required in options {
        if !advertised
            .iter()
            .any(|v| v["name"].as_str() == Some(required))
        {
            bail!("Horizon runtime does not support {required}");
        }
    }
    Ok(())
}
fn command(exe: &Path, cwd: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(exe);
    cmd.current_dir(cwd).stdin(Stdio::null()).kill_on_drop(true);
    for key in [
        "JAVA_TOOL_OPTIONS",
        "_JAVA_OPTIONS",
        "JDK_JAVA_OPTIONS",
        "CLASSPATH",
        "HORIZON_HOME",
        "HORIZON_RESOURCES",
    ] {
        cmd.env_remove(key);
    }
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    cmd
}
async fn probe(exe: &Path, cwd: &Path, args: &[&str]) -> Result<Value> {
    let result = tokio::time::timeout(
        Duration::from_secs(30),
        command(exe, cwd).args(args).output(),
    )
    .await
    .context("Horizon API check timed out")??;
    if !result.status.success() {
        bail!(
            "Horizon API check failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    serde_json::from_slice(&result.stdout).context("Invalid Horizon API response")
}
pub fn allowed_download(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none_or(|p| p == 443)
        && matches!(
            url.host_str(),
            Some(
                "api.github.com"
                    | "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}
fn file_hash(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut bytes = [0u8; 65536];
    loop {
        let n = file.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        hash.update(&bytes[..n]);
    }
    Ok(hex::encode(hash.finalize()))
}
pub fn extract(archive: &Path, destination: &Path, cancel: &AtomicBool) -> Result<()> {
    if file_hash(archive)? != ARCHIVE_SHA256 {
        bail!("Horizon archive checksum mismatch; nothing was installed");
    }
    let mut zip = zip::ZipArchive::new(File::open(archive)?)?;
    if zip.len() > 20000 {
        bail!("Horizon archive has too many entries");
    }
    let mut total = 0u64;
    for index in 0..zip.len() {
        if cancel.load(Ordering::Relaxed) {
            bail!("Horizon installation cancelled");
        }
        let mut entry = zip.by_index(index)?;
        let enclosed = entry
            .enclosed_name()
            .context("Unsafe path in Horizon archive")?;
        if !enclosed.starts_with("horizon-windows-x64")
            || entry.name().contains('\\')
            || entry.name().contains(':')
            || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
        {
            bail!("Unsupported Horizon archive entry");
        }
        total = total
            .checked_add(entry.size())
            .context("Archive size overflow")?;
        if total > 1_000_000_000 {
            bail!("Horizon archive exceeds extraction limit");
        }
        let out = destination.join(enclosed);
        if entry.is_dir() {
            std::fs::create_dir_all(out)?;
        } else {
            std::fs::create_dir_all(out.parent().context("Archive parent missing")?)?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(out)?;
            std::io::copy(&mut entry, &mut file)?;
            file.flush()?;
        }
    }
    Ok(())
}
pub async fn install(data: &Path, cancel: Arc<AtomicBool>, report: Reporter) -> Result<Runtime> {
    if !data.is_absolute() {
        bail!("Horizon data directory must be absolute");
    }
    if !cfg!(all(windows, target_arch = "x86_64")) {
        bail!("The bundled Horizon integration currently supports Windows x64");
    }
    let runtime = Runtime::at(data);
    let parent = data.join("engines/horizon");
    tokio::fs::create_dir_all(&parent).await?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(parent.join("install.lock"))?;
    fs2::FileExt::try_lock_exclusive(&lock).context("Another NodeClient is installing Horizon")?;
    if runtime.client.is_file() && runtime.prepare.is_file() {
        let client = runtime.client.clone();
        let prepare = runtime.prepare.clone();
        let valid = tokio::task::spawn_blocking(move || -> Result<bool> {
            Ok(file_hash(&client)? == CLIENT_SHA256 && file_hash(&prepare)? == PREPARE_SHA256)
        })
        .await??;
        if valid {
            runtime.check_api().await?;
            return Ok(runtime);
        }
        bail!("Horizon runtime is damaged. Rename its managed engine folder and install again; instance worlds are separate.");
    }
    let archive = parent.join(format!("{BUILD}.zip"));
    let item = Download {
        url: ASSET_URL.into(),
        path: archive.clone(),
        sha1: None,
        sha256: Some(ARCHIVE_SHA256.into()),
        size: Some(ARCHIVE_SIZE),
    };
    if !nodeclient_downloader::verify(&item).await? {
        let client = reqwest::Client::builder()
            .user_agent("NodeClient Horizon integration")
            .https_only(true)
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(600))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() < 5 && allowed_download(attempt.url()) {
                    attempt.follow()
                } else {
                    attempt.error("Unapproved Horizon redirect")
                }
            }))
            .build()?;
        let mut response = client
            .get(ASSET_URL)
            .header("Accept", "application/octet-stream")
            .send()
            .await?
            .error_for_status()
            .context("Pinned Horizon build unavailable; update NodeClient's engine pin")?;
        let temp = tempfile::NamedTempFile::new_in(&parent)?;
        let mut output = tokio::fs::File::from_std(temp.reopen()?);
        let mut received = 0u64;
        let mut hash = Sha256::new();
        while let Some(chunk) = response.chunk().await? {
            if cancel.load(Ordering::Relaxed) {
                bail!("Horizon download cancelled");
            }
            received += chunk.len() as u64;
            if received > ARCHIVE_SIZE {
                bail!("Horizon download exceeds pinned size");
            }
            hash.update(&chunk);
            output.write_all(&chunk).await?;
            report(nodeclient_downloader::Progress {
                completed: received,
                total: ARCHIVE_SIZE,
                bytes: received,
                bytes_per_second: 0,
                phase: "Horizon engine".into(),
            });
        }
        output.flush().await?;
        drop(output);
        if received != ARCHIVE_SIZE || hex::encode(hash.finalize()) != ARCHIVE_SHA256 {
            bail!("Horizon download checksum mismatch");
        }
        temp.persist(&archive).map_err(|e| e.error)?;
    }
    let stage = tempfile::tempdir_in(&parent)?;
    let stage_path = stage.path().to_owned();
    let cancelled = cancel.clone();
    tokio::task::spawn_blocking(move || extract(&archive, &stage_path, &cancelled)).await??;
    let destination = parent.join(BUILD);
    if destination.exists() {
        bail!("Incomplete Horizon runtime exists; rename its engine folder and retry");
    }
    std::fs::rename(stage.path(), &destination)?;
    runtime.check_api().await?;
    Ok(runtime)
}

pub async fn prepare(
    runtime: &Runtime,
    root: &Path,
    server_jar: &Path,
    java: &Path,
    cancel: Arc<AtomicBool>,
    log: Log,
) -> Result<()> {
    let lock = std::fs::OpenOptions::new()
        .create(true).truncate(false).read(true).write(true)
        .open(runtime.root.join("prepare.lock"))?;
    fs2::FileExt::try_lock_exclusive(&lock)
        .context("Another NodeClient is preparing Horizon resources")?;
    let java_home = java
        .parent()
        .and_then(Path::parent)
        .context("Java runtime has no home directory")?;
    let mut cmd = command(&runtime.prepare, &runtime.root);
    cmd.arg("--prepare")
        .arg("--complete")
        .arg("--progress")
        .arg("json")
        .arg("--root")
        .arg(&runtime.root)
        .arg("--output")
        .arg(&runtime.resources)
        .arg("--version-json")
        .arg(root.join(format!("versions/{MINECRAFT}/{MINECRAFT}.json")))
        .arg("--client-jar")
        .arg(root.join(format!("versions/{MINECRAFT}/{MINECRAFT}.jar")))
        .arg("--libraries-dir")
        .arg(root.join("libraries"))
        .arg("--assets-dir")
        .arg(root.join("assets"))
        .arg("--server-jar")
        .arg(server_jar)
        .arg("--java-home")
        .arg(java_home)
        .env("JAVA_HOME", java_home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .context("Could not start Horizon resource preparation")?;
    let stdout = child.stdout.take().context("Preparer stdout missing")?;
    let stderr = child.stderr.take().context("Preparer stderr missing")?;
    let pump = |reader: Box<dyn tokio::io::AsyncRead + Unpin + Send>| {
        let log = log.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(mut line)) = lines.next_line().await {
                line.truncate(line.floor_char_boundary(16384));
                log(line);
            }
        })
    };
    let out = pump(Box::new(stdout));
    let err = pump(Box::new(stderr));
    let result = loop {
        tokio::select! {
            status=child.wait()=>break status?,
            _=tokio::time::sleep(Duration::from_millis(250))=>if cancel.load(Ordering::Relaxed){
                #[cfg(windows)] if let Some(pid)=child.id(){let mut kill=tokio::process::Command::new("taskkill.exe");kill.creation_flags(0x08000000);let _=kill.args(["/PID",&pid.to_string(),"/T","/F"]).output().await;}
                let _=child.kill().await;out.abort();err.abort();bail!("Horizon preparation cancelled");
            }
        }
    };
    let _ = out.await;
    let _ = err.await;
    if !result.success() {
        bail!("Horizon resource preparation failed. See launcher logs for the missing file or Java requirement.");
    }
    if !runtime.ready().await? {
        bail!("Horizon resource preparation did not pass its completeness check");
    }
    Ok(())
}
