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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified: Option<u64>,
    pub enabled: bool,
    pub sha1: Option<String>,
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

pub fn list_mods(root: &Path, id: &str) -> Result<Vec<ModEntry>> {
    let dir = instance_subdir(root, id, "mods")?;
    let mut files = vec![];
    if !dir.is_dir() {
        return Ok(files);
    }
    for item in fs::read_dir(&dir)? {
        let item = item?;
        if !item.file_type()?.is_file() {
            continue;
        }
        let name = item.file_name().to_string_lossy().into_owned();
        let (enabled, base_name) = if let Some(base) = name.strip_suffix(".disabled") {
            let lower_base = base.to_ascii_lowercase();
            if lower_base.ends_with(".jar") || lower_base.ends_with(".zip") {
                (false, base.to_owned())
            } else {
                continue;
            }
        } else {
            let lower = name.to_ascii_lowercase();
            if lower.ends_with(".jar") || lower.ends_with(".zip") {
                (true, name.clone())
            } else {
                continue;
            }
        };
        safe_id(&name)?;
        safe_id(&base_name)?;
        let path = item.path();
        let meta = item.metadata()?;
        let sha1 = if enabled {
            file_sha1(&path).ok()
        } else {
            None
        };
        files.push(ModEntry {
            name: base_name,
            path: path.to_string_lossy().into_owned(),
            size: meta.len(),
            modified: modified_secs(&meta),
            enabled,
            sha1,
        });
    }
    files.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    Ok(files)
}

fn file_sha1(path: &Path) -> Result<String> {
    use sha1::{Digest, Sha1};
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn mod_path(dir: &Path, name: &str, enabled: bool) -> Result<PathBuf> {
    safe_id(name)?;
    let lower = name.to_ascii_lowercase();
    if !(lower.ends_with(".jar") || lower.ends_with(".zip")) {
        bail!("Only .jar or .zip mod files are supported.");
    }
    let filename = if enabled {
        name.to_owned()
    } else {
        format!("{name}.disabled")
    };
    safe_id(&filename)?;
    safe_join(dir, &filename)
}

pub fn set_mod_enabled(root: &Path, id: &str, name: &str, enabled: bool) -> Result<ModEntry> {
    let dir = instance_subdir(root, id, "mods")?;
    let from = mod_path(&dir, name, !enabled)?;
    let to = mod_path(&dir, name, enabled)?;
    if !from.is_file() {
        // Already in desired state?
        if to.is_file() {
            let meta = fs::metadata(&to)?;
            return Ok(ModEntry {
                name: name.to_owned(),
                path: to.to_string_lossy().into_owned(),
                size: meta.len(),
                modified: modified_secs(&meta),
                enabled,
                sha1: if enabled { file_sha1(&to).ok() } else { None },
            });
        }
        bail!("Mod file not found.");
    }
    if to.exists() && to != from {
        bail!("Cannot change mod state: target file already exists.");
    }
    fs::rename(&from, &to)?;
    let meta = fs::metadata(&to)?;
    Ok(ModEntry {
        name: name.to_owned(),
        path: to.to_string_lossy().into_owned(),
        size: meta.len(),
        modified: modified_secs(&meta),
        enabled,
        sha1: if enabled { file_sha1(&to).ok() } else { None },
    })
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

pub fn add_mod(root: &Path, id: &str, source: &Path) -> Result<ModEntry> {
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
    Ok(ModEntry {
        name,
        path: dest.to_string_lossy().into_owned(),
        size: meta.len(),
        modified: modified_secs(&meta),
        enabled: true,
        sha1: file_sha1(&dest).ok(),
    })
}

pub fn remove_mod(root: &Path, id: &str, name: &str) -> Result<()> {
    safe_id(name)?;
    let dir = instance_subdir(root, id, "mods")?;
    let enabled = mod_path(&dir, name, true)?;
    let disabled = mod_path(&dir, name, false)?;
    if enabled.is_file() {
        fs::remove_file(enabled)?;
        return Ok(());
    }
    if disabled.is_file() {
        fs::remove_file(disabled)?;
        return Ok(());
    }
    bail!("Mod file not found.");
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
    if !["mods", "screenshots", "saves", "backups"].contains(&folder) {
        bail!("Unsupported instance folder.");
    }
    let path = instance_subdir(root, id, folder)?;
    open::that(path)?;
    Ok(())
}

pub fn list_instance_worlds(root: &Path, id: &str) -> Result<Vec<FileEntry>> {
    let dir = instance_subdir(root, id, "saves")?;
    let mut worlds = vec![];
    if !dir.is_dir() {
        return Ok(worlds);
    }
    for item in fs::read_dir(&dir)? {
        let item = item?;
        if !item.file_type()?.is_dir() {
            continue;
        }
        let name = item.file_name().to_string_lossy().into_owned();
        if safe_id(&name).is_err() {
            continue;
        }
        let meta = item.metadata()?;
        worlds.push(FileEntry {
            name,
            path: item.path().to_string_lossy().into_owned(),
            size: meta.len(),
            modified: modified_secs(&meta),
        });
    }
    worlds.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    Ok(worlds)
}

/// Official launcher game directory (`%APPDATA%/.minecraft` on Windows).
pub fn vanilla_minecraft_dir() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        let appdata = std::env::var_os("APPDATA").context("APPDATA is not set.")?;
        Ok(PathBuf::from(appdata).join(".minecraft"))
    }
    #[cfg(not(windows))]
    {
        let home = std::env::var_os("HOME").context("HOME is not set.")?;
        Ok(PathBuf::from(home).join(".minecraft"))
    }
}

pub fn list_vanilla_worlds() -> Result<Vec<FileEntry>> {
    let saves = vanilla_minecraft_dir()?.join("saves");
    if !saves.is_dir() {
        return Ok(vec![]);
    }
    let mut worlds = vec![];
    for item in fs::read_dir(&saves)? {
        let item = item?;
        if !item.file_type()?.is_dir() {
            continue;
        }
        let name = item.file_name().to_string_lossy().into_owned();
        if safe_id(&name).is_err() {
            continue;
        }
        // Prefer folders that look like actual worlds.
        if !item.path().join("level.dat").is_file() && !item.path().join("level.dat_old").is_file()
        {
            continue;
        }
        let meta = item.metadata()?;
        worlds.push(FileEntry {
            name,
            path: item.path().to_string_lossy().into_owned(),
            size: meta.len(),
            modified: modified_secs(&meta),
        });
    }
    worlds.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    Ok(worlds)
}

/// Copy selected (or all) worlds from the official `.minecraft/saves` into an instance.
/// Never deletes or modifies the originals.
pub fn import_vanilla_worlds(root: &Path, id: &str, names: Option<Vec<String>>) -> Result<u32> {
    let source_root = vanilla_minecraft_dir()?.join("saves");
    if !source_root.is_dir() {
        bail!("No official Minecraft saves folder was found at AppData/.minecraft/saves.");
    }
    let dest_root = instance_subdir(root, id, "saves")?;
    let wanted: Option<std::collections::HashSet<String>> =
        names.map(|list| list.into_iter().collect());
    let mut imported = 0u32;
    let available = list_vanilla_worlds()?;
    for world in available {
        if wanted.as_ref().is_some_and(|set| !set.contains(&world.name)) {
            continue;
        }
        let dest = safe_join(&dest_root, &world.name)?;
        if dest.exists() {
            continue;
        }
        let source = PathBuf::from(&world.path);
        copy_dir_recursive(&source, &dest)
            .with_context(|| format!("Could not import world “{}”.", world.name))?;
        imported += 1;
    }
    Ok(imported)
}

/// Import AppData worlds when this instance has none yet.
pub fn ensure_vanilla_worlds_imported(root: &Path, id: &str) -> Result<u32> {
    if !list_instance_worlds(root, id)?.is_empty() {
        return Ok(0);
    }
    if list_vanilla_worlds()?.is_empty() {
        return Ok(0);
    }
    import_vanilla_worlds(root, id, None)
}

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for item in fs::read_dir(from)? {
        let item = item?;
        let name = item
            .file_name()
            .to_str()
            .context("Invalid world file name.")?
            .to_owned();
        safe_world_entry_name(&name)?;
        let dest = to.join(&name);
        let ty = item.file_type()?;
        if ty.is_symlink() {
            bail!("Symlinks inside world folders are not allowed.");
        }
        if ty.is_dir() {
            copy_dir_recursive(&item.path(), &dest)?;
        } else if ty.is_file() {
            fs::copy(item.path(), &dest)?;
        }
    }
    Ok(())
}

/// World folder contents use a wider charset than managed IDs (e.g. region files).
fn safe_world_entry_name(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 200
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.contains(':')
        || value.contains('\0')
    {
        bail!("Unsafe world path entry.");
    }
    if !value.is_ascii() || value.bytes().any(|b| b < 0x20) {
        bail!("Unsafe world path entry.");
    }
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
