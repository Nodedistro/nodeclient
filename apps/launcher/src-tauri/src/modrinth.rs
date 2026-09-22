//! Modrinth metadata + hashed CDN downloads for instance mods.
use anyhow::{bail, Context, Result};
use nodeclient_downloader::{download_all, fetch_trusted_json, Download, Reporter};
use nodeclient_types::{safe_id, safe_join};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub categories: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    pub total_hits: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionFile {
    pub version_id: String,
    pub name: String,
    pub version_number: String,
    pub filename: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Deserialize)]
struct RawSearch {
    hits: Vec<RawHit>,
    total_hits: u64,
}

#[derive(Deserialize)]
struct RawHit {
    project_id: String,
    slug: String,
    title: String,
    description: String,
    downloads: u64,
    icon_url: Option<String>,
    #[serde(default)]
    categories: Vec<String>,
}

#[derive(Deserialize)]
struct RawVersion {
    id: String,
    name: String,
    version_number: String,
    files: Vec<RawFile>,
}

#[derive(Clone, Deserialize)]
struct RawFile {
    url: String,
    filename: String,
    primary: bool,
    size: u64,
    hashes: RawHashes,
}

#[derive(Clone, Deserialize)]
struct RawHashes {
    sha1: Option<String>,
}

fn encode_facets(game_version: &str, loader: &str) -> Result<String> {
    let mut facets = vec![vec!["project_type:mod".to_string()]];
    facets.push(vec![format!("versions:{game_version}")]);
    if loader != "vanilla" {
        facets.push(vec![format!("categories:{loader}")]);
    }
    let encoded = serde_json::to_string(&facets).context("Could not encode Modrinth facets.")?;
    Ok(percent_encode(&encoded))
}

fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 2);
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn sanitize_filename(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        bail!("Mod file name is empty.");
    }
    let mut cleaned = String::with_capacity(trimmed.len());
    for c in trimmed.chars() {
        if c.is_ascii_alphanumeric() || "._-+@ ".contains(c) {
            cleaned.push(c);
        } else {
            cleaned.push('_');
        }
    }
    while cleaned.contains("..") {
        cleaned = cleaned.replace("..", "_");
    }
    safe_id(&cleaned)?;
    let lower = cleaned.to_ascii_lowercase();
    if !(lower.ends_with(".jar") || lower.ends_with(".zip")) {
        bail!("Modrinth file must be a .jar or .zip.");
    }
    Ok(cleaned)
}

fn version_file(version: RawVersion) -> Result<VersionFile> {
    let file = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .context("Modrinth version has no downloadable files.")?
        .clone();
    let sha1 = file
        .hashes
        .sha1
        .context("Modrinth file is missing a SHA-1 hash.")?;
    Ok(VersionFile {
        version_id: version.id,
        name: version.name,
        version_number: version.version_number,
        filename: sanitize_filename(&file.filename)?,
        url: file.url,
        sha1,
        size: file.size,
    })
}

pub async fn search(
    query: &str,
    game_version: &str,
    loader: &str,
    limit: u32,
    offset: u32,
) -> Result<SearchResult> {
    safe_id(game_version)?;
    if !["vanilla", "fabric", "quilt", "forge", "neoforge"].contains(&loader) {
        bail!("Unsupported loader for Modrinth search.");
    }
    let limit = limit.clamp(1, 40);
    let facets = encode_facets(game_version, loader)?;
    let q = percent_encode(query.trim());
    let url = format!(
        "https://api.modrinth.com/v2/search?query={q}&limit={limit}&offset={offset}&index=relevance&facets={facets}"
    );
    let raw: RawSearch = serde_json::from_value(
        fetch_trusted_json(&url, 4 * 1024 * 1024)
            .await
            .context("Could not search Modrinth.")?,
    )
    .context("Invalid Modrinth search response.")?;
    Ok(SearchResult {
        total_hits: raw.total_hits,
        hits: raw
            .hits
            .into_iter()
            .map(|h| SearchHit {
                project_id: h.project_id,
                slug: h.slug,
                title: h.title,
                description: h.description,
                downloads: h.downloads,
                icon_url: h.icon_url,
                categories: h.categories,
            })
            .collect(),
    })
}

pub async fn latest_compatible_version(
    project: &str,
    game_version: &str,
    loader: &str,
) -> Result<VersionFile> {
    safe_id(game_version)?;
    let project = project.trim();
    if project.is_empty()
        || !project
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        bail!("Invalid Modrinth project id.");
    }
    let loaders = if loader == "vanilla" {
        "[]".to_string()
    } else {
        serde_json::to_string(&vec![loader]).unwrap()
    };
    let versions_param = serde_json::to_string(&vec![game_version]).unwrap();
    let url = format!(
        "https://api.modrinth.com/v2/project/{project}/version?game_versions={}&loaders={}",
        percent_encode(&versions_param),
        percent_encode(&loaders)
    );
    let versions: Vec<RawVersion> = serde_json::from_value(
        fetch_trusted_json(&url, 4 * 1024 * 1024)
            .await
            .context("Could not load Modrinth project versions.")?,
    )
    .context("Invalid Modrinth versions response.")?;
    let version = versions
        .into_iter()
        .next()
        .context("No compatible Modrinth version for this instance.")?;
    version_file(version)
}

pub async fn install_file(
    mods_dir: &Path,
    file: &VersionFile,
    concurrency: usize,
    cancel: Arc<AtomicBool>,
    report: Reporter,
) -> Result<PathBuf> {
    safe_id(&file.filename)?;
    let dest = safe_join(mods_dir, &file.filename)?;
    if dest.exists() {
        bail!("A mod named {} is already in this instance.", file.filename);
    }
    let disabled = format!("{}.disabled", file.filename);
    if mods_dir.join(&disabled).is_file() {
        bail!(
            "A disabled mod named {} is already in this instance.",
            file.filename
        );
    }
    download_all(
        vec![Download {
            url: file.url.clone(),
            path: dest.clone(),
            sha1: Some(file.sha1.clone()),
            sha256: None,
            size: Some(file.size),
        }],
        concurrency.clamp(1, 8),
        cancel,
        report,
    )
    .await?;
    Ok(dest)
}

pub async fn latest_from_hash(
    sha1: &str,
    game_version: &str,
    loader: &str,
) -> Result<Option<VersionFile>> {
    safe_id(game_version)?;
    if sha1.len() != 40 || !sha1.bytes().all(|c| c.is_ascii_hexdigit()) {
        bail!("Invalid SHA-1 hash.");
    }
    let body = json!({
        "loaders": if loader == "vanilla" { Vec::<&str>::new() } else { vec![loader] },
        "game_versions": [game_version],
    });
    let url = format!("https://api.modrinth.com/v2/version_file/{sha1}/update?algorithm=sha1");
    nodeclient_downloader::official_url(&url)?;
    let client = nodeclient_downloader::client()?;
    let response = client.post(&url).json(&body).send().await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let response = response
        .error_for_status()
        .context("Could not check Modrinth for updates.")?;
    let bytes = response.bytes().await?;
    if bytes.len() > 2 * 1024 * 1024 {
        bail!("Modrinth update response exceeds size limit.");
    }
    if bytes.is_empty() || bytes.as_ref() == b"null" {
        return Ok(None);
    }
    let version: RawVersion =
        serde_json::from_slice(&bytes).context("Invalid Modrinth update response.")?;
    Ok(Some(version_file(version)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_modrinth_filenames() {
        assert_eq!(
            sanitize_filename("sodium-fabric-0.5.jar").unwrap(),
            "sodium-fabric-0.5.jar"
        );
        assert_eq!(
            sanitize_filename("bad/name.jar").unwrap(),
            "bad_name.jar"
        );
        assert!(sanitize_filename("noext").is_err());
    }
}
