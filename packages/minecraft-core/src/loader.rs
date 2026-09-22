//! Loader boundary: Vanilla, Fabric, Quilt, Forge, and NeoForge resolve into a merged version document.
use anyhow::{bail, Context, Result};
use nodeclient_types::Loader;
use serde_json::Value;
use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

pub async fn resolve(
    root: &Path,
    minecraft_version: &str,
    loader: &Loader,
    cancel: Arc<AtomicBool>,
    report: nodeclient_downloader::Reporter,
    java: Option<&Path>,
) -> Result<Value> {
    match loader.r#type.as_str() {
        "vanilla" => crate::version(root, minecraft_version, cancel, report).await,
        "fabric" => {
            let loader_version = loader
                .version
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .context("Fabric loader version is required.")?;
            crate::fabric::resolve(root, minecraft_version, loader_version, cancel, report).await
        }
        "quilt" => {
            let loader_version = loader
                .version
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .context("Quilt loader version is required.")?;
            crate::quilt::resolve(root, minecraft_version, loader_version, cancel, report).await
        }
        "forge" => {
            let loader_version = loader
                .version
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .context("Forge version is required.")?;
            let java = java.context("Java is required to install Forge.")?;
            crate::forge::resolve_forge(
                root,
                minecraft_version,
                loader_version,
                java,
                cancel,
                report,
            )
            .await
        }
        "neoforge" => {
            let loader_version = loader
                .version
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .context("NeoForge version is required.")?;
            let java = java.context("Java is required to install NeoForge.")?;
            crate::forge::resolve_neoforge(
                root,
                minecraft_version,
                loader_version,
                java,
                cancel,
                report,
            )
            .await
        }
        other => bail!("Unsupported loader type: {other}"),
    }
}

pub async fn list_versions(
    loader: &str,
    game_version: &str,
) -> Result<Vec<crate::fabric::FabricLoaderVersion>> {
    match loader {
        "fabric" => crate::fabric::list_loaders(game_version).await,
        "quilt" => crate::quilt::list_loaders(game_version).await,
        "forge" => crate::forge::list_forge(game_version).await,
        "neoforge" => crate::forge::list_neoforge(game_version).await,
        other => bail!("Unsupported loader type: {other}"),
    }
}
