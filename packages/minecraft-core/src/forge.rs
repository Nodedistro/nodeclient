//! Forge and NeoForge via the official silent `--installClient` installer.
use anyhow::{bail, Context, Result};
use nodeclient_downloader::{
    download_all, fetch_trusted_json, fetch_trusted_text, Download, Reporter,
};
use nodeclient_types::{safe_id, safe_join};
use serde::Deserialize;
use serde_json::Value;
use std::{
    path::Path,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use crate::fabric::FabricLoaderVersion;

#[derive(Deserialize)]
struct ForgePromos {
    promos: std::collections::HashMap<String, String>,
}

#[derive(Deserialize)]
struct NeoVersions {
    versions: Vec<String>,
}

pub fn forge_profile_id(game_version: &str, loader_version: &str) -> String {
    format!("{game_version}-forge-{loader_version}")
}

pub fn neoforge_profile_id(loader_version: &str) -> String {
    format!("neoforge-{loader_version}")
}

/// Map a Minecraft version id to the NeoForge maven version prefix.
///
/// Classic ids (`1.21`, `1.21.1`) drop the leading `1.` (`21.0.`, `21.1.`).
/// Newer ids (`26.1`, `26.1.2`) keep the full version (`26.1.`, `26.1.2.`).
pub fn neoforge_prefix(game_version: &str) -> Result<String> {
    let parts: Vec<&str> = game_version.split('.').collect();
    if parts.is_empty() || parts.iter().any(|p| p.is_empty()) {
        bail!("Unsupported Minecraft version for NeoForge.");
    }
    if !parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_digit())) {
        bail!("Unsupported Minecraft version for NeoForge.");
    }
    if parts.first() == Some(&"1") {
        return match parts.as_slice() {
            [_, minor] => Ok(format!("{minor}.0.")),
            [_, minor, patch] => Ok(format!("{minor}.{patch}.")),
            _ => bail!("Unsupported Minecraft version for NeoForge."),
        };
    }
    // Post-1.x Mojang ids (e.g. 26.1 / 26.1.2): NeoForge versions start with the same id.
    if parts.len() < 2 || parts.len() > 3 {
        bail!("Unsupported Minecraft version for NeoForge.");
    }
    Ok(format!("{game_version}."))
}

pub async fn list_forge(game_version: &str) -> Result<Vec<FabricLoaderVersion>> {
    safe_id(game_version)?;
    let xml = fetch_trusted_text(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml",
        8 * 1024 * 1024,
    )
    .await
    .context("Could not load Forge version metadata.")?;
    let prefix = format!("{game_version}-");
    let re = regex::Regex::new(r"<version>([^<]+)</version>").context("Invalid version regex.")?;
    let mut versions: Vec<String> = re
        .captures_iter(&xml)
        .filter_map(|c| {
            let full = c.get(1)?.as_str();
            full.strip_prefix(&prefix).map(str::to_owned)
        })
        .filter(|v| safe_id(v).is_ok())
        .collect();
    versions.sort_by(|a, b| compare_version(b, a));
    versions.dedup();

    let recommended = fetch_trusted_json(
        "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json",
        2 * 1024 * 1024,
    )
    .await
    .ok()
    .and_then(|v| serde_json::from_value::<ForgePromos>(v).ok())
    .and_then(|p| p.promos.get(&format!("{game_version}-recommended")).cloned());

    if versions.is_empty() {
        bail!("No Forge builds are available for Minecraft {game_version}.");
    }
    Ok(versions
        .into_iter()
        .map(|version| FabricLoaderVersion {
            stable: recommended.as_ref() == Some(&version),
            version,
        })
        .collect())
}

pub async fn list_neoforge(game_version: &str) -> Result<Vec<FabricLoaderVersion>> {
    safe_id(game_version)?;
    let prefix = neoforge_prefix(game_version)?;
    let raw = fetch_trusted_json(
        "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge",
        4 * 1024 * 1024,
    )
    .await
    .context("Could not load NeoForge versions.")?;
    let list: NeoVersions =
        serde_json::from_value(raw).context("Invalid NeoForge version list.")?;
    let matching: Vec<String> = list
        .versions
        .into_iter()
        .filter(|v| v.starts_with(&prefix))
        .filter(|v| safe_id(v).is_ok())
        .collect();
    let mut stable: Vec<String> = matching
        .iter()
        .filter(|v| !v.contains("beta") && !v.contains("alpha"))
        .cloned()
        .collect();
    // Newer Minecraft releases (e.g. 26.3) may only have NeoForge betas yet.
    let mut versions = if stable.is_empty() {
        matching
    } else {
        std::mem::take(&mut stable)
    };
    versions.sort_by(|a, b| compare_version(b, a));
    versions.dedup();
    if versions.is_empty() {
        bail!("No NeoForge builds are available for Minecraft {game_version}.");
    }
    let recommended = versions
        .iter()
        .find(|v| !v.contains("beta") && !v.contains("alpha"))
        .cloned()
        .or_else(|| versions.first().cloned());
    Ok(versions
        .into_iter()
        .map(|version| FabricLoaderVersion {
            stable: recommended.as_ref() == Some(&version),
            version,
        })
        .collect())
}

pub async fn resolve_forge(
    root: &Path,
    game_version: &str,
    loader_version: &str,
    java: &Path,
    cancel: Arc<AtomicBool>,
    report: Reporter,
) -> Result<Value> {
    safe_id(game_version)?;
    safe_id(loader_version)?;
    let profile_id = forge_profile_id(game_version, loader_version);
    safe_id(&profile_id)?;
    if profile_exists(root, &profile_id)? {
        return load_merged_profile(root, &profile_id, game_version, cancel, report).await;
    }
    let maven_ver = format!("{game_version}-{loader_version}");
    safe_id(&maven_ver)?;
    let jar_name = format!("forge-{maven_ver}-installer.jar");
    let url = format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{maven_ver}/{jar_name}"
    );
    let sha1 = fetch_sha1(&format!("{url}.sha1")).await?;
    let jar = safe_join(
        root,
        &format!("libraries/net/minecraftforge/forge/{maven_ver}/{jar_name}"),
    )?;
    download_installer(url, jar.clone(), sha1, cancel.clone(), report.clone()).await?;
    // Ensure parent Minecraft metadata is present for the installer.
    let _ = crate::version(root, game_version, cancel.clone(), report.clone()).await?;
    run_installer(java, &jar, root, cancel.clone()).await?;
    load_merged_profile(root, &profile_id, game_version, cancel, report).await
}

pub async fn resolve_neoforge(
    root: &Path,
    game_version: &str,
    loader_version: &str,
    java: &Path,
    cancel: Arc<AtomicBool>,
    report: Reporter,
) -> Result<Value> {
    safe_id(game_version)?;
    safe_id(loader_version)?;
    let prefix = neoforge_prefix(game_version)?;
    if !loader_version.starts_with(&prefix) {
        bail!("NeoForge version does not match Minecraft {game_version}.");
    }
    let profile_id = neoforge_profile_id(loader_version);
    safe_id(&profile_id)?;
    if profile_exists(root, &profile_id)? {
        return load_merged_profile(root, &profile_id, game_version, cancel, report).await;
    }
    let jar_name = format!("neoforge-{loader_version}-installer.jar");
    let url = format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{loader_version}/{jar_name}"
    );
    let sha1 = fetch_sha1(&format!("{url}.sha1")).await?;
    let jar = safe_join(
        root,
        &format!("libraries/net/neoforged/neoforge/{loader_version}/{jar_name}"),
    )?;
    download_installer(url, jar.clone(), sha1, cancel.clone(), report.clone()).await?;
    let _ = crate::version(root, game_version, cancel.clone(), report.clone()).await?;
    run_installer(java, &jar, root, cancel.clone()).await?;
    load_merged_profile(root, &profile_id, game_version, cancel, report).await
}

fn profile_exists(root: &Path, id: &str) -> Result<bool> {
    Ok(safe_join(root, &format!("versions/{id}/{id}.json"))?.is_file())
}

async fn load_merged_profile(
    root: &Path,
    id: &str,
    expected_inherits: &str,
    cancel: Arc<AtomicBool>,
    report: Reporter,
) -> Result<Value> {
    let path = safe_join(root, &format!("versions/{id}/{id}.json"))?;
    if !path.is_file() {
        bail!("Loader profile was not created by the installer.");
    }
    let profile: Value = serde_json::from_slice(&tokio::fs::read(&path).await?)
        .context("Invalid installed loader profile.")?;
    if profile["id"].as_str() != Some(id) {
        bail!("Installed loader profile id does not match.");
    }
    let inherits = profile["inheritsFrom"]
        .as_str()
        .context("Installed loader profile is missing inheritsFrom.")?;
    if inherits != expected_inherits {
        bail!("Installed loader profile inheritsFrom does not match the selected Minecraft version.");
    }
    if profile["mainClass"].as_str().unwrap_or("").is_empty() {
        bail!("Installed loader profile is missing mainClass.");
    }
    let parent = crate::version(root, inherits, cancel, report).await?;
    let mut merged = parent;
    crate::metadata::merge(&mut merged, profile)?;
    Ok(merged)
}

async fn fetch_sha1(url: &str) -> Result<String> {
    let text = fetch_trusted_text(url, 256)
        .await
        .context("Could not download installer checksum.")?;
    let hash = text
        .split_whitespace()
        .next()
        .context("Installer checksum is empty.")?
        .to_ascii_lowercase();
    if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("Installer checksum is invalid.");
    }
    Ok(hash)
}

async fn download_installer(
    url: String,
    path: std::path::PathBuf,
    sha1: String,
    cancel: Arc<AtomicBool>,
    report: Reporter,
) -> Result<()> {
    download_all(
        vec![Download {
            url,
            path,
            sha1: Some(sha1),
            sha256: None,
            size: Some(64 * 1024 * 1024),
        }],
        1,
        cancel,
        report,
    )
    .await
    .context("Could not download the mod loader installer.")
}

async fn run_installer(
    java: &Path,
    jar: &Path,
    root: &Path,
    cancel: Arc<AtomicBool>,
) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        bail!("Download cancelled.");
    }
    if !java.is_file() {
        bail!("Java executable is missing for the mod loader installer.");
    }
    if !jar.is_file() {
        bail!("Mod loader installer jar is missing.");
    }
    let mut command = tokio::process::Command::new(java);
    command
        .arg("-jar")
        .arg(jar)
        .arg("--installClient")
        .arg(root)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let output = tokio::time::timeout(Duration::from_secs(900), command.output())
        .await
        .context("Mod loader installer timed out.")?
        .context("Could not start the mod loader installer.")?;
    if cancel.load(Ordering::Relaxed) {
        bail!("Download cancelled.");
    }
    if !output.status.success() {
        let detail = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        let detail = detail.lines().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
        bail!(
            "Mod loader installer failed{}.",
            if detail.trim().is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            }
        );
    }
    Ok(())
}

fn compare_version(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u64> {
        s.split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .filter_map(|p| p.parse().ok())
            .collect()
    };
    let av = parse(a);
    let bv = parse(b);
    av.cmp(&bv).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neoforge_prefixes() {
        assert_eq!(neoforge_prefix("1.21.1").unwrap(), "21.1.");
        assert_eq!(neoforge_prefix("1.20.1").unwrap(), "20.1.");
        assert_eq!(neoforge_prefix("1.21").unwrap(), "21.0.");
        assert_eq!(neoforge_prefix("26.1").unwrap(), "26.1.");
        assert_eq!(neoforge_prefix("26.1.2").unwrap(), "26.1.2.");
        assert_eq!(neoforge_prefix("26.3").unwrap(), "26.3.");
        assert!(neoforge_prefix("snapshot").is_err());
        assert!(neoforge_prefix("1").is_err());
    }

    #[test]
    fn profile_ids() {
        assert_eq!(
            forge_profile_id("1.21.1", "52.1.0"),
            "1.21.1-forge-52.1.0"
        );
        assert_eq!(neoforge_profile_id("21.1.251"), "neoforge-21.1.251");
    }

    #[test]
    fn version_order() {
        assert_eq!(compare_version("52.1.16", "52.1.0"), std::cmp::Ordering::Greater);
        assert_eq!(compare_version("21.1.251", "21.1.200"), std::cmp::Ordering::Greater);
    }
}
