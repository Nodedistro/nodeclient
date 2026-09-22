use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tauri::{AppHandle, Emitter};

const RELEASES_API: &str = "https://api.github.com/repos/Nodedistro/nodeclient/releases/latest";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub available: bool,
    pub title: String,
    pub notes: String,
    pub html_url: String,
    pub asset_name: String,
    pub download_url: String,
    pub sha256: String,
    pub published_at: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    html_url: String,
    published_at: Option<String>,
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[allow(dead_code)]
    size: u64,
    digest: Option<String>,
}

fn parse_version(raw: &str) -> Result<(u64, u64, u64)> {
    let cleaned = raw.trim().trim_start_matches('v');
    let mut parts = cleaned.split('.');
    let major = parts
        .next()
        .unwrap_or("0")
        .parse()
        .context("Invalid version")?;
    let minor = parts
        .next()
        .unwrap_or("0")
        .parse()
        .context("Invalid version")?;
    let patch = parts
        .next()
        .unwrap_or("0")
        .parse()
        .context("Invalid version")?;
    Ok((major, minor, patch))
}

fn is_newer(latest: &str, current: &str) -> Result<bool> {
    Ok(parse_version(latest)? > parse_version(current)?)
}

fn allowed_download_url(url: &str) -> Result<reqwest::Url> {
    let parsed = reqwest::Url::parse(url).context("Invalid update URL.")?;
    if parsed.scheme() != "https" {
        bail!("Update downloads must use HTTPS.");
    }
    let host = parsed.host_str().unwrap_or("");
    if ![
        "github.com",
        "objects.githubusercontent.com",
        "release-assets.githubusercontent.com",
        "github-releases.githubusercontent.com",
    ]
    .contains(&host)
        && !host.ends_with(".githubusercontent.com")
    {
        bail!("Update download host is not allowed.");
    }
    Ok(parsed)
}

fn extract_sha256(release: &GhRelease, asset: &GhAsset) -> Result<String> {
    if let Some(digest) = &asset.digest {
        if let Some(hash) = digest
            .strip_prefix("sha256:")
            .or_else(|| digest.strip_prefix("SHA256:"))
        {
            let hash = hash.trim().to_ascii_lowercase();
            if hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
                return Ok(hash);
            }
        }
    }
    let body = release.body.as_deref().unwrap_or("");
    let re = regex::Regex::new(r"(?i)(?:SHA-?256|Checksum)\s*[:=]\s*`?([A-Fa-f0-9]{64})`?")?;
    if let Some(caps) = re.captures(body) {
        return Ok(caps[1].to_ascii_lowercase());
    }
    let named = regex::Regex::new(&format!(
        r"(?i){}\s+SHA-?256\s*[:=]\s*`?([A-Fa-f0-9]{{64}})`?",
        regex::escape(&asset.name)
    ))?;
    if let Some(caps) = named.captures(body) {
        return Ok(caps[1].to_ascii_lowercase());
    }
    bail!(
        "Latest release is missing a SHA-256 checksum for {}. Refusing to update.",
        asset.name
    );
}

pub async fn check() -> Result<UpdateInfo> {
    let client = reqwest::Client::builder()
        .user_agent(format!("NodeClient/{CURRENT_VERSION}"))
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let release: GhRelease = client
        .get(RELEASES_API)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .context("Could not read GitHub release metadata.")?;
    let latest = release.tag_name.trim().trim_start_matches('v').to_owned();
    let asset = release
        .assets
        .iter()
        .find(|a| {
            let lower = a.name.to_ascii_lowercase();
            lower.ends_with("-setup.exe") || lower.ends_with("_x64-setup.exe")
        })
        .or_else(|| {
            release.assets.iter().find(|a| {
                let lower = a.name.to_ascii_lowercase();
                lower.ends_with(".exe") && lower.contains("setup")
            })
        })
        .context("Latest release has no Windows setup installer.")?;
    allowed_download_url(&asset.browser_download_url)?;
    let sha256 = extract_sha256(&release, asset)?;
    let available = is_newer(&latest, CURRENT_VERSION)?;
    Ok(UpdateInfo {
        current_version: CURRENT_VERSION.into(),
        latest_version: latest,
        available,
        title: release
            .name
            .clone()
            .unwrap_or_else(|| format!("NodeClient {}", release.tag_name)),
        notes: release.body.clone().unwrap_or_default(),
        html_url: release.html_url,
        asset_name: asset.name.clone(),
        download_url: asset.browser_download_url.clone(),
        sha256,
        published_at: release.published_at,
    })
}

pub async fn download_and_launch(
    app: &AppHandle,
    info: &UpdateInfo,
    cancel: Arc<AtomicBool>,
) -> Result<PathBuf> {
    if !info.available {
        bail!("You are already on the latest version.");
    }
    if !info
        .asset_name
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || b"._-+".contains(&c))
        || info.asset_name.contains("..")
    {
        bail!("Unsafe update file name.");
    }
    cancel.store(false, Ordering::Relaxed);
    let url = allowed_download_url(&info.download_url)?;
    let client = reqwest::Client::builder()
        .user_agent(format!("NodeClient/{CURRENT_VERSION}"))
        .timeout(std::time::Duration::from_secs(600))
        .build()?;
    let mut response = client.get(url).send().await?.error_for_status()?;
    let total = response.content_length();
    let mut hasher = Sha256::new();
    let mut downloaded = 0u64;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if cancel.load(Ordering::Relaxed) {
            bail!("Update download cancelled.");
        }
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;
        bytes.extend_from_slice(&chunk);
        let _ = app.emit(
            "update-progress",
            UpdateProgress {
                downloaded,
                total,
            },
        );
    }
    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(&info.sha256) {
        bail!("Update checksum mismatch. Download rejected.");
    }
    let dir = std::env::temp_dir().join("nodeclient-updates");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(&info.asset_name);
    let tmp = path.with_extension("exe.partial");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &path)?;
    open::that(&path).context("Could not open the updater installer.")?;
    Ok(path)
}
