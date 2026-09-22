use anyhow::{bail, Context, Result};
use futures_util::{stream, StreamExt};
use serde::Serialize;
use sha1::{Digest, Sha1};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone)]
pub struct Download {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub sha256: Option<String>,
    pub size: Option<u64>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub completed: u64,
    pub total: u64,
    pub bytes: u64,
    pub bytes_per_second: u64,
    pub phase: String,
}
pub type Reporter = Arc<dyn Fn(Progress) + Send + Sync>;
pub fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent("NodeClient/0.1.6 (https://github.com/Nodedistro/nodeclient)")
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .tcp_nodelay(true)
        .tcp_keepalive(std::time::Duration::from_secs(60))
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(std::time::Duration::from_secs(300))
        .build()?)
}
pub fn official_url(value: &str) -> Result<()> {
    let url = reqwest::Url::parse(value)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some_and(|p| p != 443)
        || ![
            "piston-meta.mojang.com",
            "piston-data.mojang.com",
            "launchermeta.mojang.com",
            "launcher.mojang.com",
            "libraries.minecraft.net",
            "resources.download.minecraft.net",
            "meta.fabricmc.net",
            "maven.fabricmc.net",
            "meta.quiltmc.org",
            "maven.quiltmc.net",
            "maven.minecraftforge.net",
            "files.minecraftforge.net",
            "maven.neoforged.net",
            "mirrors.neoforged.net",
            "api.modrinth.com",
            "cdn.modrinth.com",
        ]
        .contains(&url.host_str().unwrap_or(""))
    {
        bail!("Download URL is not an approved official Minecraft host.");
    }
    Ok(())
}

/// Fetch bytes from an allowlisted HTTPS host (metadata, checksum sidecars) without a content hash.
pub async fn fetch_trusted_bytes(url: &str, max_bytes: usize) -> Result<Vec<u8>> {
    official_url(url)?;
    let response = client()?
        .get(url)
        .send()
        .await?
        .error_for_status()?;
    let bytes = response.bytes().await?;
    if bytes.len() > max_bytes {
        bail!("Trusted metadata response exceeds size limit.");
    }
    Ok(bytes.to_vec())
}

/// Fetch text from an allowlisted HTTPS host without a content hash.
pub async fn fetch_trusted_text(url: &str, max_bytes: usize) -> Result<String> {
    let bytes = fetch_trusted_bytes(url, max_bytes).await?;
    String::from_utf8(bytes).context("Trusted metadata response is not valid UTF-8")
}

/// Fetch JSON from an allowlisted HTTPS host (Fabric meta, etc.) without a content hash.
pub async fn fetch_trusted_json(url: &str, max_bytes: usize) -> Result<serde_json::Value> {
    let bytes = fetch_trusted_bytes(url, max_bytes).await?;
    Ok(serde_json::from_slice(&bytes).context("Invalid trusted metadata JSON")?)
}
pub async fn verify(item: &Download) -> Result<bool> {
    if !item.path.is_file() {
        return Ok(false);
    }
    let mut file = tokio::fs::File::open(&item.path).await?;
    if item
        .size
        .is_some_and(|size| std::fs::metadata(&item.path).is_ok_and(|m| m.len() != size))
    {
        return Ok(false);
    }
    let mut sha1 = Sha1::new();
    let mut sha256 = sha2::Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        sha1.update(&buffer[..n]);
        sha256.update(&buffer[..n]);
    }
    Ok(item
        .sha1
        .as_ref()
        .is_none_or(|h| hex::encode(sha1.finalize()).eq_ignore_ascii_case(h))
        && item
            .sha256
            .as_ref()
            .is_none_or(|h| hex::encode(sha256.finalize()).eq_ignore_ascii_case(h)))
}
pub fn verify_bytes(bytes: &[u8], item: &Download) -> bool {
    item.size.is_none_or(|n| n == bytes.len() as u64)
        && item
            .sha1
            .as_ref()
            .is_none_or(|h| hex::encode(Sha1::digest(bytes)).eq_ignore_ascii_case(h))
        && item
            .sha256
            .as_ref()
            .is_none_or(|h| hex::encode(sha2::Sha256::digest(bytes)).eq_ignore_ascii_case(h))
}
async fn fetch(
    client: &reqwest::Client,
    item: &Download,
    cancel: &AtomicBool,
    bytes: &AtomicU64,
    report: &Reporter,
    done: &AtomicU64,
    total: u64,
    start: Instant,
) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        bail!("Download cancelled.");
    }
    official_url(&item.url)?;
    if item.sha1.is_none() && item.sha256.is_none() {
        bail!("Official file has no integrity hash.");
    }
    if verify(item).await? {
        return Ok(());
    }
    let parent = item.path.parent().context("Missing download directory")?;
    tokio::fs::create_dir_all(parent).await?;
    for attempt in 0..3 {
        if cancel.load(Ordering::Relaxed) {
            bail!("Download cancelled.");
        }
        let temporary = parent.join(format!(".{}.partial", uuid::Uuid::new_v4()));
        let result: Result<()> = async {
            let response = client.get(&item.url).send().await?.error_for_status()?;
            let mut stream = response.bytes_stream();
            let mut file = tokio::fs::File::create(&temporary).await?;
            let mut count = 0;
            while let Some(chunk) = stream.next().await {
                if cancel.load(Ordering::Relaxed) {
                    bail!("Download cancelled.");
                }
                let chunk = chunk?;
                count += chunk.len() as u64;
                if count > item.size.unwrap_or(1024 * 1024 * 1024) {
                    bail!("Download exceeds expected size.");
                }
                file.write_all(&chunk).await?;
                let n = bytes.fetch_add(chunk.len() as u64, Ordering::Relaxed) + chunk.len() as u64;
                report(Progress {
                    completed: done.load(Ordering::Relaxed),
                    total,
                    bytes: n,
                    bytes_per_second: (n as f64 / start.elapsed().as_secs_f64().max(0.1)) as u64,
                    phase: "DOWNLOADING".into(),
                });
            }
            file.flush().await?;
            file.sync_all().await?;
            drop(file);
            let mut check = item.clone();
            check.path = temporary.clone();
            report(Progress {
                completed: done.load(Ordering::Relaxed),
                total,
                bytes: bytes.load(Ordering::Relaxed),
                bytes_per_second: 0,
                phase: "VERIFYING".into(),
            });
            if !verify(&check).await? {
                bail!("Failed to verify a downloaded Minecraft file.");
            }
            if item.path.exists() {
                tokio::fs::remove_file(&item.path).await?;
            }
            tokio::fs::rename(&temporary, &item.path).await?;
            Ok(())
        }
        .await;
        let _ = tokio::fs::remove_file(&temporary).await;
        match result {
            Ok(()) => return Ok(()),
            Err(e) if attempt == 2 => return Err(e.context("Download failed after 3 attempts")),
            Err(_) => {
                tokio::time::sleep(std::time::Duration::from_millis(300 * (attempt + 1))).await
            }
        }
    }
    bail!("Download failed")
}
pub async fn download_all(
    items: Vec<Download>,
    concurrency: usize,
    cancel: Arc<AtomicBool>,
    report: Reporter,
) -> Result<()> {
    let client = client()?;
    let bytes = Arc::new(AtomicU64::new(0));
    let done = Arc::new(AtomicU64::new(0));
    let total = items.len() as u64;
    let start = Instant::now();
    let results = stream::iter(items.into_iter().map(|item| {
        let client = client.clone();
        let bytes = bytes.clone();
        let done = done.clone();
        let cancel = cancel.clone();
        let report = report.clone();
        async move {
            fetch(
                &client, &item, &cancel, &bytes, &report, &done, total, start,
            )
            .await?;
            let completed = done.fetch_add(1, Ordering::Relaxed) + 1;
            report(Progress {
                completed,
                total,
                bytes: bytes.load(Ordering::Relaxed),
                bytes_per_second: (bytes.load(Ordering::Relaxed) as f64
                    / start.elapsed().as_secs_f64().max(0.1))
                    as u64,
                phase: "DOWNLOADING".into(),
            });
            Ok::<(), anyhow::Error>(())
        }
    }))
    .buffer_unordered(concurrency.clamp(1, 16))
    .collect::<Vec<_>>()
    .await;
    // Drain workers rather than dropping in-flight futures so every partial file is cleaned.
    for result in results {
        result?;
    }
    Ok(())
}
pub fn ensure_managed(root: &Path, path: &str) -> Result<PathBuf> {
    nodeclient_types::safe_join(root, path)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checksum_and_size() {
        let d = Download {
            url: String::new(),
            path: PathBuf::new(),
            sha1: Some("a9993e364706816aba3e25717850c26c9cd0d89d".into()),
            sha256: None,
            size: Some(3),
        };
        assert!(verify_bytes(b"abc", &d));
        assert!(!verify_bytes(b"abd", &d));
        assert!(!verify_bytes(b"ab", &d));
    }
    #[test]
    fn url_security() {
        assert!(official_url("https://libraries.minecraft.net/a.jar").is_ok());
        assert!(official_url("https://meta.fabricmc.net/v2/versions/loader").is_ok());
        assert!(official_url("https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.1/a.jar").is_ok());
        assert!(official_url("https://api.modrinth.com/v2/search").is_ok());
        assert!(official_url("https://cdn.modrinth.com/data/AA/versions/1/a.jar").is_ok());
        for u in [
            "http://libraries.minecraft.net/a",
            "https://evil.com/a",
            "https://libraries.minecraft.net@evil.com/a",
        ] {
            assert!(official_url(u).is_err());
        }
    }
}
