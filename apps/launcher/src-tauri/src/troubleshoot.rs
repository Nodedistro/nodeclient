//! Repair helpers, crash heuristics, and instance backups.
use anyhow::{bail, Context, Result};
use nodeclient_types::{safe_id, safe_join};
use serde::Serialize;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::instance_content;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashExplanation {
    pub title: String,
    pub summary: String,
    pub details: Vec<String>,
    pub actions: Vec<String>,
    pub report_name: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified: Option<u64>,
}

/// Disable every currently enabled mod; return their base names for later restore.
pub fn disable_enabled_mods(root: &Path, id: &str) -> Result<Vec<String>> {
    let mods = instance_content::list_mods(root, id)?;
    let mut disabled = vec![];
    for entry in mods {
        if entry.enabled {
            instance_content::set_mod_enabled(root, id, &entry.name, false)?;
            disabled.push(entry.name);
        }
    }
    Ok(disabled)
}

pub fn restore_mods(root: &Path, id: &str, names: &[String]) -> Result<()> {
    for name in names {
        let _ = instance_content::set_mod_enabled(root, id, name, true);
    }
    Ok(())
}

pub fn clear_install_marker(game: &Path) -> Result<()> {
    let marker = safe_join(game, "installed.json")?;
    if marker.is_file() {
        fs::remove_file(marker)?;
    }
    Ok(())
}

pub fn explain_crash(root: &Path, id: &str) -> Result<CrashExplanation> {
    safe_id(id)?;
    let game = safe_join(root, &format!("instances/{id}"))?;
    let (source, text) = latest_diagnostics(&game)?;
    let lower = text.to_ascii_lowercase();
    let mut details = vec![];
    let mut actions = vec!["view_crash".into()];
    let (title, summary) = if lower.contains("outofmemoryerror")
        || lower.contains("java heap space")
        || lower.contains("gc overhead limit exceeded")
    {
        details.push("Minecraft ran out of memory.".into());
        details.push("Raise the instance RAM maximum, or close other apps.".into());
        actions.push("increase_ram".into());
        (
            "Out of memory".into(),
            "The JVM ran out of heap space while starting or playing.".into(),
        )
    } else if lower.contains("mixin")
        || lower.contains("incompatibleclasschangeerror")
        || lower.contains("modloading")
        || lower.contains("fabric loader")
            && (lower.contains("error") || lower.contains("failed"))
        || lower.contains("neoforge") && lower.contains("mod loading")
    {
        details.push("A mod or mixin conflict is the most common cause.".into());
        details.push("Try Safe mode (all mods off), then re-enable mods in small batches.".into());
        actions.insert(0, "safe_mode".into());
        actions.push("update_mods".into());
        (
            "Mod conflict likely".into(),
            "The crash report points at mod loading or mixin application.".into(),
        )
    } else if lower.contains("noclassdeffounderror")
        || lower.contains("classnotfoundexception")
        || lower.contains("nosuchmethoderror")
        || lower.contains("noclassdeffound")
    {
        details.push("A required library or mod dependency is missing or mismatched.".into());
        details.push("Repair the instance files, then update or reinstall conflicting mods.".into());
        actions.insert(0, "repair".into());
        actions.push("safe_mode".into());
        (
            "Missing class or method".into(),
            "Java could not find a class or method expected by the game or a mod.".into(),
        )
    } else if lower.contains("could not find or load main class")
        || lower.contains("error: could not create the java virtual machine")
        || lower.contains("unsupportedclassversionerror")
    {
        details.push("The Java runtime may be wrong for this Minecraft version.".into());
        details.push("Use Automatic Java, or pick a matching major version.".into());
        actions.insert(0, "repair".into());
        (
            "Java runtime problem".into(),
            "Minecraft failed before the game fully started because of Java.".into(),
        )
    } else if lower.contains("glfw")
        || lower.contains("failed to create window")
        || lower.contains("pixel format not accelerated")
        || lower.contains("opengl")
    {
        details.push("Graphics drivers or GPU settings may be blocking the game window.".into());
        details.push("Update GPU drivers and try windowed mode.".into());
        (
            "Graphics / window failure".into(),
            "The game could not create its display window.".into(),
        )
    } else if text.trim().is_empty() || text.contains("No crash reports found") {
        details.push("No detailed crash report was found yet.".into());
        details.push("Play once, then check Logs → Crash Reports after a failure.".into());
        actions.insert(0, "repair".into());
        actions.push("safe_mode".into());
        (
            "No crash report".into(),
            "Nothing useful was found in crash-reports or the console log.".into(),
        )
    } else {
        details.push("The report does not match a common pattern.".into());
        details.push("Repair files, try Safe mode, then share the crash report if it keeps failing.".into());
        actions.insert(0, "repair".into());
        actions.push("safe_mode".into());
        (
            "Crash detected".into(),
            "Minecraft exited abnormally. Review the report for the first EXCEPTION or Error line.".into(),
        )
    };

    // Surface a short excerpt of the first hard error line.
    let excerpt = text
        .lines()
        .find(|l| {
            let l = l.to_ascii_lowercase();
            l.contains("exception")
                || l.contains("error:")
                || l.contains("caused by:")
                || l.contains("fatal")
        })
        .unwrap_or("")
        .trim();
    let summary = if excerpt.is_empty() {
        summary
    } else {
        format!("{summary}\n\n{excerpt}")
    };

    Ok(CrashExplanation {
        title,
        summary,
        details,
        actions,
        report_name: if source == "none" {
            None
        } else {
            Some(source)
        },
    })
}

fn latest_diagnostics(game: &Path) -> Result<(String, String)> {
    let crash_dir = safe_join(game, "crash-reports")?;
    if crash_dir.is_dir() {
        let mut files: Vec<_> = fs::read_dir(&crash_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .collect();
        files.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
        if let Some(latest) = files.last() {
            let name = latest.file_name().to_string_lossy().into_owned();
            safe_id(&name)?;
            let path = safe_join(game, &format!("crash-reports/{name}"))?;
            let text = read_tail(&path, 512 * 1024)?;
            return Ok((format!("crash-reports/{name}"), text));
        }
    }
    let console = safe_join(game, "logs/nodeclient-console.log")?;
    if console.is_file() {
        let text = read_tail(&console, 512 * 1024)?;
        return Ok(("logs/nodeclient-console.log".into(), text));
    }
    Ok(("none".into(), String::new()))
}

fn read_tail(path: &Path, max: usize) -> Result<String> {
    let bytes = fs::read(path)?;
    let start = bytes.len().saturating_sub(max);
    Ok(String::from_utf8_lossy(&bytes[start..]).into_owned())
}

fn backups_dir(root: &Path, id: &str) -> Result<PathBuf> {
    safe_id(id)?;
    let dir = safe_join(root, &format!("instances/{id}/backups"))?;
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn list_backups(root: &Path, id: &str) -> Result<Vec<BackupEntry>> {
    let dir = backups_dir(root, id)?;
    let mut list = vec![];
    for item in fs::read_dir(dir)? {
        let item = item?;
        if !item.file_type()?.is_file() {
            continue;
        }
        let name = item.file_name().to_string_lossy().into_owned();
        if !name.to_ascii_lowercase().ends_with(".zip") {
            continue;
        }
        safe_id(&name)?;
        let meta = item.metadata()?;
        list.push(BackupEntry {
            name,
            path: item.path().to_string_lossy().into_owned(),
            size: meta.len(),
            modified: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs()),
        });
    }
    list.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(list)
}

pub fn create_backup(root: &Path, id: &str) -> Result<BackupEntry> {
    safe_id(id)?;
    let game = safe_join(root, &format!("instances/{id}"))?;
    let dir = backups_dir(root, id)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let name = format!("backup-{stamp}.zip");
    safe_id(&name)?;
    let path = safe_join(&dir, &name)?;
    if path.exists() {
        bail!("A backup with that name already exists.");
    }

    let file = File::create(&path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut total = 0u64;

    // Worlds
    let saves = safe_join(&game, "saves")?;
    if saves.is_dir() {
        add_dir_to_zip(&mut zip, &saves, Path::new("saves"), &mut total, options)?;
    }
    // Lightweight config surfaces
    for rel in ["options.txt", "servers.dat", "optionsof.txt"] {
        let file_path = safe_join(&game, rel)?;
        if file_path.is_file() {
            add_file_to_zip(&mut zip, &file_path, Path::new(rel), &mut total, options)?;
        }
    }
    let config = safe_join(&game, "config")?;
    if config.is_dir() {
        add_dir_to_zip(&mut zip, &config, Path::new("config"), &mut total, options)?;
    }

    zip.finish()?;
    let meta = fs::metadata(&path)?;
    Ok(BackupEntry {
        name,
        path: path.to_string_lossy().into_owned(),
        size: meta.len(),
        modified: Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        ),
    })
}

pub fn restore_backup(root: &Path, id: &str, name: &str) -> Result<()> {
    safe_id(id)?;
    safe_id(name)?;
    if !name.to_ascii_lowercase().ends_with(".zip") {
        bail!("Backup must be a .zip file.");
    }
    let game = safe_join(root, &format!("instances/{id}"))?;
    let archive = safe_join(&backups_dir(root, id)?, name)?;
    if !archive.is_file() {
        bail!("Backup not found.");
    }
    let mut zip = zip::ZipArchive::new(File::open(&archive)?)?;
    if zip.len() > 50000 {
        bail!("Backup archive has too many entries.");
    }
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let raw = entry.name().to_owned();
        if raw.contains('\\') || raw.starts_with('/') || raw.contains(':') {
            bail!("Unsafe path in backup archive.");
        }
        let trimmed = raw.trim_end_matches('/');
        if trimmed.is_empty() {
            continue;
        }
        // Only restore known roots.
        let first = trimmed.split('/').next().unwrap_or("");
        if !["saves", "config", "options.txt", "servers.dat", "optionsof.txt"].contains(&first)
            && !trimmed.starts_with("saves/")
            && !trimmed.starts_with("config/")
        {
            continue;
        }
        let dest = safe_join(&game, trimmed)?;
        if entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            bail!("Archive symlinks are not allowed.");
        }
        total = total
            .checked_add(entry.size())
            .context("Backup size overflow")?;
        if total > 8 * 1024 * 1024 * 1024 {
            bail!("Backup exceeds restore size limit.");
        }
        if entry.is_dir() || raw.ends_with('/') {
            fs::create_dir_all(&dest)?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&dest)?;
        std::io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

pub fn delete_backup(root: &Path, id: &str, name: &str) -> Result<()> {
    safe_id(id)?;
    safe_id(name)?;
    let path = safe_join(&backups_dir(root, id)?, name)?;
    if !path.is_file() {
        bail!("Backup not found.");
    }
    fs::remove_file(path)?;
    Ok(())
}

fn add_dir_to_zip(
    zip: &mut zip::ZipWriter<File>,
    dir: &Path,
    prefix: &Path,
    total: &mut u64,
    options: zip::write::SimpleFileOptions,
) -> Result<()> {
    for item in fs::read_dir(dir)? {
        let item = item?;
        let name = item.file_name();
        let name_str = name.to_string_lossy();
        if name_str == "." || name_str == ".." {
            continue;
        }
        // Skip session locks that can cause restore issues while running.
        if name_str == "session.lock" {
            continue;
        }
        let rel = prefix.join(&name);
        let path = item.path();
        if item.file_type()?.is_dir() {
            add_dir_to_zip(zip, &path, &rel, total, options)?;
        } else if item.file_type()?.is_file() {
            add_file_to_zip(zip, &path, &rel, total, options)?;
        }
    }
    Ok(())
}

fn add_file_to_zip(
    zip: &mut zip::ZipWriter<File>,
    path: &Path,
    rel: &Path,
    total: &mut u64,
    options: zip::write::SimpleFileOptions,
) -> Result<()> {
    let meta = fs::metadata(path)?;
    *total = total
        .checked_add(meta.len())
        .context("Backup size overflow")?;
    if *total > 8 * 1024 * 1024 * 1024 {
        bail!("Backup exceeds size limit (8 GB).");
    }
    let name = rel
        .to_str()
        .context("Invalid backup path encoding.")?
        .replace('\\', "/");
    zip.start_file(name, options)?;
    let mut file = File::open(path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    zip.write_all(&buf)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explain_oom_heuristic() {
        let dir = tempfile::tempdir().unwrap();
        let id = "demo";
        let crash = dir.path().join("instances").join(id).join("crash-reports");
        fs::create_dir_all(&crash).unwrap();
        fs::write(
            crash.join("crash-2026.txt"),
            "java.lang.OutOfMemoryError: Java heap space\n",
        )
        .unwrap();
        let explained = explain_crash(dir.path(), id).unwrap();
        assert_eq!(explained.title, "Out of memory");
        assert!(explained.actions.contains(&"increase_ram".into()));
    }
}
