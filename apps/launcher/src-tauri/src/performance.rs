//! FPS helpers: video options patching and Modrinth performance packs.
use anyhow::{bail, Context, Result};
use nodeclient_types::{safe_id, safe_join};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// Exact JVM flags allowed for performance presets (also in Settings validation).
pub const ALLOWED_JVM_FLAGS: &[&str] = &[
    "-XX:+UseG1GC",
    "-XX:+UseZGC",
    "-XX:+UseStringDeduplication",
    "-XX:+AlwaysPreTouch",
    "-XX:+DisableExplicitGC",
    "-XX:+ParallelRefProcEnabled",
    "-XX:+PerfDisableSharedMem",
    "-XX:MaxGCPauseMillis=50",
    "-XX:MaxGCPauseMillis=200",
    "-XX:MaxTenuringThreshold=1",
    "-XX:G1NewSizePercent=30",
    "-XX:G1MaxNewSizePercent=40",
    "-XX:G1HeapRegionSize=8M",
    "-XX:G1ReservePercent=20",
    "-XX:InitiatingHeapOccupancyPercent=15",
];

pub fn jvm_preset(name: &str) -> Result<Vec<String>> {
    let flags = match name {
        "default" => vec![],
        "balanced" => vec![
            "-XX:+UseG1GC".into(),
            "-XX:+AlwaysPreTouch".into(),
            "-XX:+UseStringDeduplication".into(),
            "-XX:MaxGCPauseMillis=200".into(),
        ],
        "high" => vec![
            "-XX:+UseG1GC".into(),
            "-XX:+AlwaysPreTouch".into(),
            "-XX:+UseStringDeduplication".into(),
            "-XX:+DisableExplicitGC".into(),
            "-XX:+ParallelRefProcEnabled".into(),
            "-XX:+PerfDisableSharedMem".into(),
            "-XX:MaxGCPauseMillis=50".into(),
            "-XX:MaxTenuringThreshold=1".into(),
            "-XX:G1NewSizePercent=30".into(),
            "-XX:G1MaxNewSizePercent=40".into(),
            "-XX:G1HeapRegionSize=8M".into(),
            "-XX:G1ReservePercent=20".into(),
            "-XX:InitiatingHeapOccupancyPercent=15".into(),
        ],
        other => bail!("Unknown performance preset: {other}"),
    };
    for flag in &flags {
        if !ALLOWED_JVM_FLAGS.contains(&flag.as_str()) {
            bail!("Preset produced disallowed JVM flag: {flag}");
        }
    }
    Ok(flags)
}

/// Modrinth project ids for a practical client FPS pack.
pub fn performance_mod_projects(loader: &str) -> Result<&'static [&'static str]> {
    match loader {
        "fabric" | "quilt" | "neoforge" => Ok(&[
            "AANobbMI", // Sodium
            "gvQqBUqZ", // Lithium
            "uXXizFIs", // FerriteCore
            "5ZwdcRci", // ImmediatelyFast
            "NNAgCjsB", // Entity Culling
        ]),
        "forge" => Ok(&[
            "uXXizFIs", // FerriteCore
            "5ZwdcRci", // ImmediatelyFast
            "NNAgCjsB", // Entity Culling
        ]),
        "vanilla" => bail!("Install Fabric, Quilt, or NeoForge first to use Sodium and Lithium."),
        other => bail!("Unsupported loader for performance mods: {other}"),
    }
}

pub fn apply_fps_video_settings(root: &Path, id: &str) -> Result<PathBuf> {
    safe_id(id)?;
    let path = safe_join(root, &format!("instances/{id}/options.txt"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = read_options(&path)?;
    for (key, value) in [
        ("enableVsync", "false"),
        ("maxFps", "260"),
        ("graphicsMode", "fast"),
        ("particles", "minimal"),
        ("entityShadows", "false"),
        ("cloudStatus", "false"),
        ("ao", "false"),
        ("entityDistanceScaling", "0.75"),
        ("mipmapLevels", "2"),
        ("prioritizeChunkUpdates", "byPlayer"),
        ("simulationDistance", "8"),
        // Older option keys still read by some versions:
        ("fancyGraphics", "false"),
        ("renderClouds", "false"),
        ("advancedItemTooltips", "false"),
    ] {
        options.insert(key.into(), value.into());
    }
    write_options(&path, &options)?;
    Ok(path)
}

fn read_options(path: &Path) -> Result<BTreeMap<String, String>> {
    let mut map = BTreeMap::new();
    if !path.is_file() {
        return Ok(map);
    }
    let text = fs::read_to_string(path).context("Could not read options.txt")?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            map.insert(k.to_owned(), v.to_owned());
        }
    }
    Ok(map)
}

fn write_options(path: &Path, options: &BTreeMap<String, String>) -> Result<()> {
    let mut out = String::new();
    for (k, v) in options {
        if k.contains('\n') || v.contains('\n') || k.contains(':') {
            bail!("Invalid options.txt entry.");
        }
        out.push_str(k);
        out.push(':');
        out.push_str(v);
        out.push('\n');
    }
    fs::write(path, out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn patches_options_without_breaking_existing() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let id = "main";
        fs::create_dir_all(root.join("instances/main")).unwrap();
        fs::write(
            root.join("instances/main/options.txt"),
            "lang:en_us\nrenderDistance:12\n",
        )
        .unwrap();
        apply_fps_video_settings(root, id).unwrap();
        let text = fs::read_to_string(root.join("instances/main/options.txt")).unwrap();
        assert!(text.contains("lang:en_us"));
        assert!(text.contains("renderDistance:12"));
        assert!(text.contains("enableVsync:false"));
        assert!(text.contains("maxFps:260"));
    }

    #[test]
    fn presets_are_allowlisted() {
        for preset in ["balanced", "high"] {
            for flag in jvm_preset(preset).unwrap() {
                assert!(
                    ALLOWED_JVM_FLAGS.contains(&flag.as_str()),
                    "{flag} missing from allowlist"
                );
            }
        }
    }
}
