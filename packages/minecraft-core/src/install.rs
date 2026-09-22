use anyhow::{bail, Context, Result};
use nodeclient_downloader::{download_all, fetch_trusted_text, Download, Reporter};
use nodeclient_types::{safe_id, safe_join};
use serde_json::Value;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
pub struct Installation {
    pub classpath: Vec<PathBuf>,
    pub natives: PathBuf,
    pub asset_index: String,
    pub assets: PathBuf,
    pub game_assets: PathBuf,
    pub logging: Option<(String, PathBuf)>,
}
pub fn maven_path(coordinate: &str) -> Result<String> {
    let parts: Vec<&str> = coordinate.split(':').collect();
    let (group, artifact, version, classifier) = match parts.as_slice() {
        [g, a, v] => (*g, *a, *v, None),
        [g, a, v, c] => (*g, *a, *v, Some(*c)),
        _ => bail!("Invalid Maven library coordinate."),
    };
    if group.is_empty() || artifact.is_empty() || version.is_empty() {
        bail!("Invalid Maven library coordinate.");
    }
    let group_path = group.replace('.', "/");
    let file = match classifier {
        Some(c) => format!("{artifact}-{version}-{c}.jar"),
        None => format!("{artifact}-{version}.jar"),
    };
    Ok(format!("{group_path}/{artifact}/{version}/{file}"))
}

pub fn artifact(value: &Value, path: PathBuf) -> Result<Download> {
    Ok(Download {
        url: value["url"]
            .as_str()
            .context("Download URL missing")?
            .into(),
        path,
        sha1: Some(
            value["sha1"]
                .as_str()
                .context("Official checksum missing")?
                .into(),
        ),
        sha256: None,
        size: value["size"].as_u64(),
    })
}

async fn maven_download(lib: &Value, root: &Path) -> Result<(Download, PathBuf)> {
    let name = lib["name"]
        .as_str()
        .context("Maven library name missing")?;
    let relative = maven_path(name)?;
    let base = lib["url"]
        .as_str()
        .unwrap_or("https://libraries.minecraft.net/")
        .trim_end_matches('/');
    let url = format!("{base}/{relative}");
    let path = safe_join(root, &format!("libraries/{relative}"))?;
    let mut sha1 = lib["sha1"].as_str().map(str::to_owned);
    let mut sha256 = lib["sha256"].as_str().map(str::to_owned);
    // Fabric/Quilt profiles often omit hashes; use Maven sidecar checksums.
    if sha1.is_none() && sha256.is_none() {
        if let Ok(text) = fetch_trusted_text(&format!("{url}.sha1"), 256).await {
            sha1 = Some(parse_sidecar_hash(&text, 40).with_context(|| {
                format!("Maven library {name} returned an invalid SHA-1 sidecar.")
            })?);
        } else if let Ok(text) = fetch_trusted_text(&format!("{url}.sha256"), 128).await {
            sha256 = Some(parse_sidecar_hash(&text, 64).with_context(|| {
                format!("Maven library {name} returned an invalid SHA-256 sidecar.")
            })?);
        }
    }
    if sha1.is_none() && sha256.is_none() {
        bail!("Maven library {name} is missing an integrity hash.");
    }
    Ok((
        Download {
            url,
            path: path.clone(),
            sha1,
            sha256,
            size: lib["size"].as_u64(),
        },
        path,
    ))
}

fn parse_sidecar_hash(text: &str, expected_len: usize) -> Result<String> {
    let hash = text
        .split_whitespace()
        .next()
        .context("Checksum sidecar is empty.")?
        .to_ascii_lowercase();
    if hash.len() != expected_len || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("Checksum sidecar is invalid.");
    }
    Ok(hash)
}

pub async fn libraries(
    root: &Path,
    version: &Value,
) -> Result<(Vec<Download>, Vec<PathBuf>, Vec<PathBuf>)> {
    let mut downloads = vec![];
    let mut classpath = vec![];
    let mut native_archives = vec![];
    for lib in version["libraries"]
        .as_array()
        .context("Libraries missing")?
    {
        if !crate::rules::allowed(lib.get("rules"), false)? {
            continue;
        }
        if let Some(a) = lib["downloads"].get("artifact") {
            let path = safe_join(
                root,
                &format!(
                    "libraries/{}",
                    a["path"].as_str().context("Library path missing")?
                ),
            )?;
            downloads.push(artifact(a, path.clone())?);
            classpath.push(path);
        } else if lib.get("name").and_then(Value::as_str).is_some() {
            let (download, path) = maven_download(lib, root).await?;
            downloads.push(download);
            classpath.push(path);
        }
        if let Some(classifier) = lib["natives"][crate::rules::os_name()].as_str() {
            let classifier = classifier.replace(
                "${arch}",
                if cfg!(target_pointer_width = "64") {
                    "64"
                } else {
                    "32"
                },
            );
            let a = &lib["downloads"]["classifiers"][&classifier];
            let path = safe_join(
                root,
                &format!(
                    "libraries/{}",
                    a["path"]
                        .as_str()
                        .context("Native library unavailable for this platform")?
                ),
            )?;
            downloads.push(artifact(a, path.clone())?);
            native_archives.push(path);
        }
    }
    Ok((downloads, classpath, native_archives))
}
pub async fn install(
    root: &Path,
    game: &Path,
    version: &Value,
    concurrency: usize,
    cancel: Arc<AtomicBool>,
    report: Reporter,
) -> Result<Installation> {
    let id = version["id"].as_str().context("Version ID missing")?;
    safe_id(id)?;
    let index = &version["assetIndex"];
    let index_id = index["id"].as_str().context("Asset index missing")?;
    safe_id(index_id)?;
    let index_path = safe_join(root, &format!("assets/indexes/{index_id}.json"))?;
    download_all(
        vec![artifact(index, index_path.clone())?],
        concurrency,
        cancel.clone(),
        report.clone(),
    )
    .await?;
    let asset_index: Value = serde_json::from_slice(&tokio::fs::read(index_path).await?)?;
    let (mut downloads, mut classpath, native_archives) = libraries(root, version).await?;
    let client_path = safe_join(root, &format!("versions/{id}/{id}.jar"))?;
    downloads.push(artifact(
        &version["downloads"]["client"],
        client_path.clone(),
    )?);
    classpath.push(client_path);
    let mut hashes = HashSet::new();
    for (_, obj) in asset_index["objects"]
        .as_object()
        .context("Asset objects missing")?
    {
        let hash = obj["hash"].as_str().context("Asset hash missing")?;
        if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            bail!("Invalid asset hash");
        }
        if hashes.insert(hash.to_owned()) {
            downloads.push(Download {
                url: format!(
                    "https://resources.download.minecraft.net/{}/{hash}",
                    &hash[..2]
                ),
                path: safe_join(root, &format!("assets/objects/{}/{hash}", &hash[..2]))?,
                sha1: Some(hash.into()),
                sha256: None,
                size: obj["size"].as_u64(),
            });
        }
    }
    let mut logging = None;
    if let Some(log) = version["logging"].get("client") {
        let file = &log["file"];
        let name = file["id"].as_str().context("Logging filename missing")?;
        safe_id(name)?;
        let path = safe_join(root, &format!("assets/log_configs/{name}"))?;
        downloads.push(artifact(file, path.clone())?);
        logging = Some((
            log["argument"]
                .as_str()
                .context("Logging argument missing")?
                .into(),
            path,
        ));
    }
    // Deduplicate destinations; conflicting metadata is rejected.
    let mut seen = std::collections::HashMap::new();
    let mut unique = vec![];
    for d in downloads {
        if let Some(hash) = seen.get(&d.path) {
            if hash != &d.sha1 {
                bail!("Conflicting download destinations");
            }
        } else {
            seen.insert(d.path.clone(), d.sha1.clone());
            unique.push(d);
        }
    }
    download_all(unique, concurrency, cancel.clone(), report).await?;
    let natives = safe_join(game, "natives")?;
    if natives.exists() {
        tokio::fs::remove_dir_all(&natives).await?;
    }
    tokio::fs::create_dir_all(&natives).await?;
    for archive in native_archives {
        if cancel.load(Ordering::Relaxed) {
            bail!("Installation cancelled.");
        }
        extract_natives(&archive, &natives)?;
    }
    let assets = safe_join(root, "assets")?;
    let mut game_assets = assets.clone();
    if asset_index["virtual"].as_bool() == Some(true)
        || asset_index["map_to_resources"].as_bool() == Some(true)
    {
        let dest = if asset_index["map_to_resources"].as_bool() == Some(true) {
            safe_join(game, "resources")?
        } else {
            safe_join(root, &format!("assets/virtual/{index_id}"))?
        };
        for (name, obj) in asset_index["objects"].as_object().unwrap() {
            if cancel.load(Ordering::Relaxed) {
                bail!("Installation cancelled.");
            }
            let hash = obj["hash"].as_str().unwrap();
            let target = safe_join(&dest, name)?;
            tokio::fs::create_dir_all(target.parent().unwrap()).await?;
            tokio::fs::copy(
                safe_join(root, &format!("assets/objects/{}/{hash}", &hash[..2]))?,
                target,
            )
            .await?;
        }
        game_assets = dest;
    }
    Ok(Installation {
        classpath,
        natives,
        asset_index: index_id.into(),
        assets,
        game_assets,
        logging,
    })
}
pub fn extract_natives(archive: &Path, destination: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)?;
    if zip.len() > 10000 {
        bail!("Native archive has too many entries");
    }
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_owned();
        let path = safe_join(destination, name.trim_end_matches('/'))?;
        if entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            bail!("Archive symlinks are not allowed");
        }
        total = total
            .checked_add(entry.size())
            .context("Archive size overflow")?;
        if total > 512 * 1024 * 1024 {
            bail!("Native archive exceeds extraction limit");
        }
        if name.starts_with("META-INF/") || entry.is_dir() {
            continue;
        }
        std::fs::create_dir_all(path.parent().unwrap())?;
        let mut out = std::fs::File::create(path)?;
        std::io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn selects_platform_libraries() {
        let v = serde_json::json!({"libraries":[{"name":"a","rules":[{"action":"allow","os":{"name":"impossible"}}],"downloads":{"artifact":{"path":"bad","url":"bad","sha1":"bad"}}},{"name":"b","downloads":{"artifact":{"path":"org/b.jar","url":"https://libraries.minecraft.net/org/b.jar","sha1":"abc","size":3}}}]});
        let (d, cp, n) = libraries(Path::new("root"), &v).await.unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(cp.len(), 1);
        assert!(n.is_empty());
    }

    #[test]
    fn maven_coordinates_map_to_paths() {
        assert_eq!(
            maven_path("net.fabricmc:fabric-loader:0.19.5").unwrap(),
            "net/fabricmc/fabric-loader/0.19.5/fabric-loader-0.19.5.jar"
        );
        assert_eq!(
            maven_path("net.fabricmc:sponge-mixin:0.17.4+mixin.0.8.7").unwrap(),
            "net/fabricmc/sponge-mixin/0.17.4+mixin.0.8.7/sponge-mixin-0.17.4+mixin.0.8.7.jar"
        );
    }

    #[tokio::test]
    async fn maven_libraries_are_accepted() {
        let v = serde_json::json!({
            "libraries": [{
                "name": "org.ow2.asm:asm:9.10.1",
                "url": "https://maven.fabricmc.net/",
                "sha1": "ada2141c0cc52ee8f5c48cd5fa4ce0e794f22236",
                "size": 126151
            }]
        });
        let (d, cp, _) = libraries(Path::new("root"), &v).await.unwrap();
        assert_eq!(d.len(), 1);
        assert!(d[0].url.contains("maven.fabricmc.net/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar"));
        assert_eq!(cp.len(), 1);
    }

    #[test]
    fn sidecar_hash_parsing() {
        assert_eq!(
            parse_sidecar_hash("ff9e65cffca4a67f31523e1807fe0855940fcbfa\n", 40).unwrap(),
            "ff9e65cffca4a67f31523e1807fe0855940fcbfa"
        );
        assert!(parse_sidecar_hash("not-a-hash", 40).is_err());
    }
}
#[cfg(test)]
mod archive_security {
    use super::*;
    #[test]
    fn zip_slip_is_rejected_without_writing_outside_root() {
        use std::io::Write;
        let directory = tempfile::tempdir().unwrap();
        let archive = directory.path().join("malicious.zip");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        writer
            .start_file("../escaped.dll", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"fixture").unwrap();
        writer.finish().unwrap();
        let destination = directory.path().join("natives");
        std::fs::create_dir(&destination).unwrap();
        assert!(extract_natives(&archive, &destination).is_err());
        assert!(!directory.path().join("escaped.dll").exists());
    }
}
