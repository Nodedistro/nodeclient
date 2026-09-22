use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub minecraft_version: String,
    pub loader: Loader,
    pub java: Java,
    pub memory: Memory,
    #[serde(default)]
    pub last_played: Option<u64>,
    #[serde(default)]
    pub playtime_seconds: u64,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Loader {
    pub r#type: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Java {
    pub mode: String,
    pub path: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Memory {
    pub minimum_mb: u32,
    pub maximum_mb: u32,
}
impl Instance {
    pub fn validate(&self) -> Result<()> {
        safe_id(&self.id)?;
        safe_id(&self.minecraft_version)?;
        if self.name.trim().is_empty() || self.name.len() > 100 {
            bail!("Instance name must contain 1–100 characters.");
        }
        if self.loader.r#type != "vanilla" {
            bail!("Only Vanilla is supported in this milestone.");
        }
        if !["automatic", "custom"].contains(&self.java.mode.as_str()) {
            bail!("Invalid Java mode.");
        }
        if self.java.mode == "custom" && self.java.path.as_deref().unwrap_or("").is_empty() {
            bail!("Choose a Java executable.");
        }
        if self.memory.minimum_mb < 512
            || self.memory.maximum_mb < self.memory.minimum_mb
            || self.memory.maximum_mb > 65536
        {
            bail!("Memory must be between 512 and 65536 MB, with minimum no greater than maximum.");
        }
        Ok(())
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub skins: Vec<serde_json::Value>,
    #[serde(default)]
    pub capes: Vec<serde_json::Value>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Settings {
    pub setup_complete: bool,
    pub selected_instance: Option<String>,
    pub selected_account: Option<String>,
    pub theme: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub concurrency: usize,
    pub minimize_on_launch: bool,
    pub remember_instance: bool,
    pub jvm_arguments: Vec<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            setup_complete: false,
            selected_instance: None,
            selected_account: None,
            theme: "dark".into(),
            width: 1280,
            height: 720,
            fullscreen: false,
            concurrency: 6,
            minimize_on_launch: false,
            remember_instance: true,
            jvm_arguments: vec![],
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !(1..=16).contains(&self.concurrency)
            || !(640..=7680).contains(&self.width)
            || !(480..=4320).contains(&self.height)
            || !["dark", "light", "system"].contains(&self.theme.as_str())
        {
            bail!("Invalid settings.");
        }
        // Only explicitly safe tuning flags; classpath, agents, file outputs and replacement entrypoints are forbidden.
        for arg in &self.jvm_arguments {
            if ![
                "-XX:+UseG1GC",
                "-XX:+UseZGC",
                "-XX:+UseStringDeduplication",
                "-XX:+AlwaysPreTouch",
            ]
            .contains(&arg.as_str())
            {
                bail!("Unsupported JVM argument: use one of the documented GC tuning flags.");
            }
        }
        Ok(())
    }
}
pub fn safe_id(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 160
        || value == "."
        || value == ".."
        || value.ends_with('.')
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        bail!("Unsafe identifier.");
    }
    let stem = value.split('.').next().unwrap_or("").to_ascii_uppercase();
    if [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ]
    .contains(&stem.as_str())
    {
        bail!("Reserved filename.");
    }
    Ok(())
}
pub fn safe_join(root: &Path, relative: &str) -> Result<PathBuf> {
    if relative.contains('\\') || relative.contains(':') || relative.starts_with('/') {
        bail!("Unsafe path.");
    }
    for part in Path::new(relative).components() {
        match part {
            Component::Normal(s) => safe_id(
                s.to_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?,
            )?,
            _ => bail!("Path traversal rejected."),
        }
    }
    if relative.is_empty() {
        bail!("Empty path.");
    }
    let path = root.join(relative);
    let mut check = Some(path.as_path());
    while let Some(p) = check {
        if let Ok(meta) = std::fs::symlink_metadata(p) {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    bail!("Reparse points are not allowed in managed paths.");
                }
            }
            if meta.file_type().is_symlink() {
                bail!("Symlinks are not allowed in managed paths.");
            }
        }
        if p == root {
            break;
        }
        check = p.parent();
    }
    Ok(path)
}
pub fn redact_secrets(input: &str, secrets: &[String]) -> String {
    let mut output = input.to_owned();
    for secret in secrets {
        if !secret.is_empty() {
            output = output.replace(secret, "<REDACTED>");
        }
    }
    let re=regex::Regex::new(r#"(?i)((?:access[_-]?token|refresh[_-]?token|authorization|client_secret|code_verifier)[\s"':=]+)(?:Bearer\s+)?[^\s",}]+|eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+"#).unwrap();
    re.replace_all(&output, "<REDACTED>").into_owned()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_paths() {
        for x in [
            "../escape",
            "/root",
            "C:/file",
            "a\\b",
            "a/../b",
            "CON",
            "x:y",
        ] {
            assert!(safe_join(Path::new("root"), x).is_err(), "{x}");
        }
        assert!(safe_join(Path::new("root"), "assets/ab/cdef").is_ok());
    }
    #[test]
    fn redact() {
        let s = redact_secrets(
            "--accessToken secret refresh_token=hello",
            &["secret".into()],
        );
        assert!(!s.contains("secret"));
        assert!(!s.contains("hello"));
    }
}
