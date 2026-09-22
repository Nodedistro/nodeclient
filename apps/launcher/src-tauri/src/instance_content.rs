use anyhow::{bail, Context, Result};
use nodeclient_types::{safe_id, safe_join};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified: Option<u64>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerEntry {
    pub name: String,
    pub ip: String,
}

#[derive(Deserialize, Serialize)]
struct ServersRoot {
    #[serde(default)]
    servers: Vec<NbtServer>,
}

#[derive(Deserialize, Serialize)]
struct NbtServer {
    name: String,
    ip: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
}

fn modified_secs(meta: &fs::Metadata) -> Option<u64> {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

fn instance_subdir(root: &Path, id: &str, folder: &str) -> Result<PathBuf> {
    safe_id(id)?;
    let dir = safe_join(root, &format!("instances/{id}/{folder}"))?;
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn list_mods(root: &Path, id: &str) -> Result<Vec<FileEntry>> {
    list_files(&instance_subdir(root, id, "mods")?, &["jar", "zip"])
}

pub fn list_screenshots(root: &Path, id: &str) -> Result<Vec<FileEntry>> {
    let mut files = list_files(&instance_subdir(root, id, "screenshots")?, &["png", "jpg", "jpeg"])?;
    files.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(files)
}

fn list_files(dir: &Path, extensions: &[&str]) -> Result<Vec<FileEntry>> {
    let mut files = vec![];
    if !dir.is_dir() {
        return Ok(files);
    }
    for item in fs::read_dir(dir)? {
        let item = item?;
        if !item.file_type()?.is_file() {
            continue;
        }
        let name = item.file_name().to_string_lossy().into_owned();
        let ext = Path::new(&name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !extensions.contains(&ext.as_str()) {
            continue;
        }
        safe_id(&name)?;
        let meta = item.metadata()?;
        files.push(FileEntry {
            name,
            path: item.path().to_string_lossy().into_owned(),
            size: meta.len(),
            modified: modified_secs(&meta),
        });
    }
    files.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    Ok(files)
}

pub fn add_mod(root: &Path, id: &str, source: &Path) -> Result<FileEntry> {
    let name = source
        .file_name()
        .and_then(|n| n.to_str())
        .context("Invalid mod file name.")?
        .to_owned();
    safe_id(&name)?;
    let ext = Path::new(&name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !["jar", "zip"].contains(&ext.as_str()) {
        bail!("Only .jar or .zip mod files can be added.");
    }
    let dest_dir = instance_subdir(root, id, "mods")?;
    let dest = safe_join(&dest_dir, &name)?;
    if dest.exists() {
        bail!("A mod named {name} is already in this instance.");
    }
    fs::copy(source, &dest)?;
    let meta = fs::metadata(&dest)?;
    Ok(FileEntry {
        name,
        path: dest.to_string_lossy().into_owned(),
        size: meta.len(),
        modified: modified_secs(&meta),
    })
}

pub fn remove_mod(root: &Path, id: &str, name: &str) -> Result<()> {
    safe_id(name)?;
    let path = safe_join(&instance_subdir(root, id, "mods")?, name)?;
    if !path.is_file() {
        bail!("Mod file not found.");
    }
    fs::remove_file(path)?;
    Ok(())
}

pub fn delete_screenshot(root: &Path, id: &str, name: &str) -> Result<()> {
    safe_id(name)?;
    let path = safe_join(&instance_subdir(root, id, "screenshots")?, name)?;
    if !path.is_file() {
        bail!("Screenshot not found.");
    }
    fs::remove_file(path)?;
    Ok(())
}

pub fn read_screenshot(root: &Path, id: &str, name: &str) -> Result<Vec<u8>> {
    safe_id(name)?;
    let path = safe_join(&instance_subdir(root, id, "screenshots")?, name)?;
    if !path.is_file() {
        bail!("Screenshot not found.");
    }
    let meta = fs::metadata(&path)?;
    if meta.len() > 25 * 1024 * 1024 {
        bail!("Screenshot is too large to preview.");
    }
    Ok(fs::read(path)?)
}

pub fn open_subdir(root: &Path, id: &str, folder: &str) -> Result<()> {
    if !["mods", "screenshots"].contains(&folder) {
        bail!("Unsupported instance folder.");
    }
    let path = instance_subdir(root, id, folder)?;
    open::that(path)?;
    Ok(())
}

fn servers_path(root: &Path, id: &str) -> Result<PathBuf> {
    safe_id(id)?;
    let dir = safe_join(root, &format!("instances/{id}"))?;
    fs::create_dir_all(&dir)?;
    Ok(dir.join("servers.dat"))
}

pub fn list_servers(root: &Path, id: &str) -> Result<Vec<ServerEntry>> {
    let path = servers_path(root, id)?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let bytes = fs::read(&path)?;
    let mut decoder = flate2::read::GzDecoder::new(bytes.as_slice());
    let mut raw = Vec::new();
    decoder
        .read_to_end(&mut raw)
        .context("servers.dat is not valid gzip NBT.")?;
    let root_nbt: ServersRoot =
        fastnbt::from_bytes(&raw).context("Could not parse Minecraft servers.dat.")?;
    Ok(root_nbt
        .servers
        .into_iter()
        .map(|s| ServerEntry {
            name: s.name,
            ip: s.ip,
        })
        .collect())
}

pub fn save_servers(root: &Path, id: &str, servers: &[ServerEntry]) -> Result<()> {
    if servers.len() > 200 {
        bail!("Too many servers.");
    }
    for server in servers {
        let name = server.name.trim();
        let ip = server.ip.trim();
        if name.is_empty() || name.len() > 64 {
            bail!("Server name must be 1–64 characters.");
        }
        if ip.is_empty() || ip.len() > 255 {
            bail!("Server address must be 1–255 characters.");
        }
        if ip.contains(['\n', '\r', '\0']) || name.contains(['\n', '\r', '\0']) {
            bail!("Invalid server fields.");
        }
    }
    let root_nbt = ServersRoot {
        servers: servers
            .iter()
            .map(|s| NbtServer {
                name: s.name.trim().into(),
                ip: s.ip.trim().into(),
                icon: None,
            })
            .collect(),
    };
    let nbt = fastnbt::to_bytes(&root_nbt)?;
    let path = servers_path(root, id)?;
    let mut encoder =
        flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&nbt)?;
    let gzipped = encoder.finish()?;
    let tmp = path.with_extension("dat.tmp");
    fs::write(&tmp, gzipped)?;
    fs::rename(tmp, path)?;
    Ok(())
}
