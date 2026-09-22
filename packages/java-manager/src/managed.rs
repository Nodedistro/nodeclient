use anyhow::{bail, Context, Result};
use nodeclient_downloader::{client, download_all, Download, Reporter};
use nodeclient_types::{safe_id, safe_join};
use serde_json::Value;
use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};
pub async fn install(
    root: &Path,
    component: &str,
    major: u32,
    concurrency: usize,
    cancel: Arc<AtomicBool>,
    report: Reporter,
) -> Result<crate::Runtime> {
    // Stage one targets Windows x64. Other platforms can still use a validated local runtime.
    if !cfg!(all(windows, target_arch = "x86_64")) {
        bail!("Managed Java is currently available on Windows x64. Select an installed Java {major} runtime.");
    }
    safe_id(component)?;
    let all_cache = safe_join(root, "java/all.json")?;
    let mut all: Option<Value> = None;
    if let Ok(c) = client() {
        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt as u64)).await;
            }
            if let Ok(resp) = c.get("https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json").send().await {
                if let Ok(resp) = resp.error_for_status() {
                    if let Ok(bytes) = resp.bytes().await {
                        if let Ok(val) = serde_json::from_slice::<Value>(&bytes) {
                            let _ = tokio::fs::create_dir_all(safe_join(root, "java")?).await;
                            let _ = tokio::fs::write(&all_cache, &bytes).await;
                            all = Some(val);
                            break;
                        }
                    }
                }
            }
        }
    }
    let all = match all {
        Some(v) => v,
        None => {
            if all_cache.is_file() {
                let bytes = tokio::fs::read(&all_cache).await?;
                serde_json::from_slice::<Value>(&bytes)?
            } else {
                bail!("Could not download official Java runtime manifest. Please check your internet connection.");
            }
        }
    };
    let manifest = &all["windows-x64"][component][0]["manifest"];
    let directory = safe_join(root, &format!("java/{component}"))?;
    let manifest_path = safe_join(&directory, "runtime-manifest.json")?;
    let item = |v: &Value, path: std::path::PathBuf| -> Result<Download> {
        Ok(Download {
            url: v["url"]
                .as_str()
                .context("Official Java runtime unavailable")?
                .into(),
            path,
            sha1: Some(v["sha1"].as_str().context("Java checksum missing")?.into()),
            sha256: None,
            size: v["size"].as_u64(),
        })
    };
    download_all(
        vec![item(manifest, manifest_path.clone())?],
        1,
        cancel.clone(),
        report.clone(),
    )
    .await?;
    let value: Value = serde_json::from_slice(&tokio::fs::read(manifest_path).await?)?;
    let mut downloads = vec![];
    for (name, entry) in value["files"]
        .as_object()
        .context("Java manifest has no files")?
    {
        let path = safe_join(&directory, name)?;
        match entry["type"].as_str() {
            Some("directory") => {
                tokio::fs::create_dir_all(path).await?;
            }
            Some("file") => downloads.push(item(&entry["downloads"]["raw"], path)?),
            _ => bail!("Unsupported entry in Java runtime manifest; installation stopped safely."),
        }
    }
    download_all(downloads, concurrency, cancel, report).await?;
    let runtime = crate::validate(&safe_join(&directory, "bin/java.exe")?).await?;
    if runtime.major != major {
        bail!("Downloaded Java does not match the required Java {major}.");
    }
    Ok(runtime)
}
