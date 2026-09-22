//! Fabric loader profile resolution via meta.fabricmc.net.
use anyhow::{bail, Context, Result};
use nodeclient_downloader::{fetch_trusted_json, Reporter};
use nodeclient_types::{safe_id, safe_join};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct FabricLoaderVersion {
    pub version: String,
    pub stable: bool,
}

#[derive(Deserialize)]
struct FabricLoaderEntry {
    loader: FabricLoaderVersion,
}

pub async fn list_loaders(game_version: &str) -> Result<Vec<FabricLoaderVersion>> {
    safe_id(game_version)?;
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{game_version}");
    let entries: Vec<FabricLoaderEntry> = serde_json::from_value(
        fetch_trusted_json(&url, 4 * 1024 * 1024)
            .await
            .context("Could not load Fabric loader versions.")?,
    )
    .context("Invalid Fabric loader version list.")?;
    Ok(entries.into_iter().map(|e| e.loader).collect())
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
        "https://meta.fabricmc.net/v2/versions/loader/{game_version}/{loader_version}/profile/json"
    );
    let profile = fetch_trusted_json(&url, 2 * 1024 * 1024)
        .await
        .context("Could not download Fabric loader profile.")?;
    let inherits = profile["inheritsFrom"]
        .as_str()
        .context("Fabric profile is missing inheritsFrom.")?;
    if inherits != game_version {
        bail!("Fabric profile inheritsFrom does not match the selected Minecraft version.");
    }
    let id = profile["id"]
        .as_str()
        .context("Fabric profile is missing id.")?;
    safe_id(id)?;
    if profile["mainClass"].as_str().unwrap_or("").is_empty() {
        bail!("Fabric profile is missing mainClass.");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_version_id_ok() {
        assert!(safe_id("1.21.1").is_ok());
        assert!(safe_id("0.19.5").is_ok());
        assert!(safe_id("fabric-loader-0.19.5-1.21.1").is_ok());
    }
}
