use anyhow::{bail, Context, Result};
use nodeclient_downloader::{client, download_all, official_url, Download, Reporter};
use nodeclient_types::{safe_id, safe_join};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc, RwLock},
    time::Instant,
};

pub const OFFICIAL_MANIFEST_URLS: &[&str] = &[
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
    "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json",
];

static IN_MEMORY_MANIFEST: RwLock<Option<(Instant, Manifest)>> = RwLock::new(None);

#[derive(Clone, Serialize, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct VersionEntry {
    pub id: String,
    pub url: String,
    pub sha1: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub latest: Latest,
    pub versions: Vec<VersionEntry>,
}
pub async fn manifest() -> Result<Manifest> {
    manifest_cached(None).await
}

pub async fn manifest_cached(root: Option<&Path>) -> Result<Manifest> {
    if let Ok(guard) = IN_MEMORY_MANIFEST.read() {
        if let Some((instant, ref cached)) = *guard {
            if instant.elapsed() < std::time::Duration::from_secs(300) {
                return Ok(cached.clone());
            }
        }
    }

    let cache_path = root.and_then(|r| safe_join(r, "versions/version_manifest_v2.json").ok());

    let mut last_error = None;
    if let Ok(c) = client() {
        for url in OFFICIAL_MANIFEST_URLS {
            for attempt in 0..2 {
                if attempt > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(500 * attempt as u64)).await;
                }
                match c.get(*url).send().await {
                    Ok(resp) => match resp.error_for_status() {
                        Ok(resp) => match resp.bytes().await {
                            Ok(bytes) => {
                                if bytes.len() > 10 * 1024 * 1024 {
                                    last_error = Some(anyhow::anyhow!("Version manifest exceeds size limit."));
                                    break;
                                }
                                match serde_json::from_slice::<Manifest>(&bytes) {
                                    Ok(result) => {
                                        if result.versions.is_empty() {
                                            last_error = Some(anyhow::anyhow!("Mojang returned an empty version manifest."));
                                            break;
                                        }
                                        if let Some(ref path) = cache_path {
                                            if let Some(parent) = path.parent() {
                                                let _ = tokio::fs::create_dir_all(parent).await;
                                            }
                                            let _ = tokio::fs::write(path, &bytes).await;
                                        }
                                        if let Ok(mut guard) = IN_MEMORY_MANIFEST.write() {
                                            *guard = Some((Instant::now(), result.clone()));
                                        }
                                        return Ok(result);
                                    }
                                    Err(e) => {
                                        last_error = Some(anyhow::anyhow!("Failed to parse version manifest: {e}"));
                                    }
                                }
                            }
                            Err(e) => {
                                last_error = Some(e.into());
                            }
                        },
                        Err(e) => {
                            last_error = Some(e.into());
                        }
                    },
                    Err(e) => {
                        last_error = Some(e.into());
                    }
                }
            }
        }
    }

    // If network requests failed, attempt to fall back to cached version manifest
    if let Some(ref path) = cache_path {
        if path.is_file() {
            if let Ok(bytes) = tokio::fs::read(path).await {
                if let Ok(result) = serde_json::from_slice::<Manifest>(&bytes) {
                    if !result.versions.is_empty() {
                        if let Ok(mut guard) = IN_MEMORY_MANIFEST.write() {
                            *guard = Some((Instant::now(), result.clone()));
                        }
                        return Ok(result);
                    }
                }
            }
        }
    }

    // Check if we have an older in-memory manifest to fall back to
    if let Ok(guard) = IN_MEMORY_MANIFEST.read() {
        if let Some((_, ref cached)) = *guard {
            return Ok(cached.clone());
        }
    }

    if let Some(e) = last_error {
        bail!("Could not connect to Mojang version servers (tried piston-meta.mojang.com and launchermeta.mojang.com): {e}. Please check your internet connection.");
    }
    bail!("Could not retrieve Minecraft version manifest.");
}

pub async fn version(
    root: &Path,
    id: &str,
    cancel: Arc<AtomicBool>,
    report: Reporter,
) -> Result<Value> {
    let manifest = manifest_cached(Some(root)).await?;
    let mut chain = vec![];
    let mut current = id.to_string();
    loop {
        safe_id(&current)?;
        if chain.len() >= 12 || chain.iter().any(|v: &Value| v["id"] == current) {
            bail!("Invalid or cyclic Minecraft version inheritance.");
        }
        let entry = manifest
            .versions
            .iter()
            .find(|v| v.id == current)
            .context("Selected version is not in Mojang's official manifest.")?;
        official_url(&entry.url)?;
        let path = safe_join(root, &format!("versions/{current}/{current}.json"))?;
        download_all(
            vec![Download {
                url: entry.url.clone(),
                path: path.clone(),
                sha1: Some(entry.sha1.clone()),
                sha256: None,
                size: None,
            }],
            1,
            cancel.clone(),
            report.clone(),
        )
        .await?;
        let value: Value = serde_json::from_slice(&tokio::fs::read(path).await?)?;
        if value["id"] != current {
            bail!("Version metadata ID does not match manifest.");
        }
        let parent = value["inheritsFrom"].as_str().map(str::to_owned);
        chain.push(value);
        if let Some(parent) = parent {
            current = parent
        } else {
            break;
        }
    }
    let mut merged = chain.pop().context("Missing version metadata")?;
    while let Some(child) = chain.pop() {
        merge(&mut merged, child)?;
    }
    Ok(merged)
}
pub fn merge(parent: &mut Value, child: Value) -> Result<()> {
    let target = parent.as_object_mut().context("Invalid parent metadata")?;
    for (key, value) in child.as_object().context("Invalid child metadata")? {
        match key.as_str() {
            "libraries" => {
                let mut libs = target
                    .get(key)
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for lib in value.as_array().context("Invalid libraries")? {
                    let name = lib["name"].as_str().unwrap_or("");
                    let coordinate = name.split(':').take(2).collect::<Vec<_>>().join(":");
                    libs.retain(|v| {
                        v["name"]
                            .as_str()
                            .unwrap_or("")
                            .split(':')
                            .take(2)
                            .collect::<Vec<_>>()
                            .join(":")
                            != coordinate
                    });
                    libs.push(lib.clone());
                }
                target.insert(key.clone(), Value::Array(libs));
            }
            "arguments" => {
                let mut args = target.get(key).cloned().unwrap_or(serde_json::json!({}));
                for (kind, list) in value.as_object().context("Invalid arguments")? {
                    let mut items = args[kind].as_array().cloned().unwrap_or_default();
                    items.extend(list.as_array().context("Invalid argument list")?.clone());
                    args[kind] = Value::Array(items);
                }
                target.insert(key.clone(), args);
            }
            "inheritsFrom" => {}
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
    Ok(())
}
pub fn java_major(value: &Value) -> u32 {
    value["javaVersion"]["majorVersion"].as_u64().unwrap_or(8) as u32
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inheritance() {
        let mut p = serde_json::json!({"libraries":[{"name":"g:a:1"}],"arguments":{"game":["--old"]},"mainClass":"Main"});
        merge(
            &mut p,
            serde_json::json!({"libraries":[{"name":"g:a:2"}],"arguments":{"game":["--new"]}}),
        )
        .unwrap();
        assert_eq!(p["libraries"].as_array().unwrap().len(), 1);
        assert_eq!(p["arguments"]["game"].as_array().unwrap().len(), 2);
        assert_eq!(p["mainClass"], "Main");
    }
}
