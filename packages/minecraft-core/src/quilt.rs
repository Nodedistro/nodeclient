//! Quilt loader profile resolution via meta.quiltmc.org.
use anyhow::{bail, Context, Result};
use nodeclient_downloader::{fetch_trusted_json, Reporter};
use nodeclient_types::{safe_id, safe_join};
use serde::Deserialize;
use serde_json::Value;
use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

use crate::fabric::FabricLoaderVersion;

#[derive(Deserialize)]
struct QuiltLoaderEntry {
    loader: QuiltLoaderMeta,
}

#[derive(Deserialize)]
struct QuiltLoaderMeta {
    version: String,
    #[serde(default)]
    stable: bool,
}

pub async fn list_loaders(game_version: &str) -> Result<Vec<FabricLoaderVersion>> {
    safe_id(game_version)?;
    let url = format!("https://meta.quiltmc.org/v3/versions/loader/{game_version}");
    let entries: Vec<QuiltLoaderEntry> = serde_json::from_value(
        fetch_trusted_json(&url, 4 * 1024 * 1024)
            .await
            .context("Could not load Quilt loader versions.")?,
    )
    .context("Invalid Quilt loader version list.")?;
    Ok(entries
        .into_iter()
        .map(|e| {
            let version = e.loader.version;
            let stable = e.loader.stable
                || !(version.contains("beta")
                    || version.contains("rc")
                    || version.contains("pre")
                    || version.contains("alpha"));
            FabricLoaderVersion { version, stable }
        })
        .collect())
}

pub async fn resolve(
    root: &Path,
    game_version: &str,
    loader_version: &str,
    cancel: Arc<AtomicBool>,
    report: Reporter,
) -> Result<Value> {
    safe_id(game_version)?;
    safe_id(loader_version)?;
    let url = format!(
        "https://meta.quiltmc.org/v3/versions/loader/{game_version}/{loader_version}/profile/json"
    );
    let profile = fetch_trusted_json(&url, 2 * 1024 * 1024)
        .await
        .context("Could not download Quilt loader profile.")?;
    let inherits = profile["inheritsFrom"]
        .as_str()
        .context("Quilt profile is missing inheritsFrom.")?;
    if inherits != game_version {
        bail!("Quilt profile inheritsFrom does not match the selected Minecraft version.");
    }
    let id = profile["id"]
        .as_str()
        .context("Quilt profile is missing id.")?;
    safe_id(id)?;
    if profile["mainClass"].as_str().unwrap_or("").is_empty() {
        bail!("Quilt profile is missing mainClass.");
    }
    let path = safe_join(root, &format!("versions/{id}/{id}.json"))?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, serde_json::to_vec_pretty(&profile)?).await?;

    let parent = crate::version(root, game_version, cancel, report).await?;
    let mut merged = parent;
    crate::metadata::merge(&mut merged, profile)?;
    Ok(merged)
}
