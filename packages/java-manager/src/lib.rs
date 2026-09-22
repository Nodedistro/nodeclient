use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Runtime {
    pub path: String,
    pub major: u32,
    pub version: String,
}
pub fn parse_major(output: &str) -> Result<u32> {
    let re = regex::Regex::new(r#"(?:openjdk|java) version "([^"]+)""#)?;
    let version = re
        .captures(output)
        .context("Java did not return a recognizable version.")?
        .get(1)
        .unwrap()
        .as_str();
    let mut parts = version.split(['.', '-', '_']);
    let first = parts.next().unwrap_or("").parse::<u32>()?;
    Ok(if first == 1 {
        parts
            .next()
            .context("Missing Java major version")?
            .parse()?
    } else {
        first
    })
}
pub async fn validate(path: &Path) -> Result<Runtime> {
    if !path.is_absolute() || !path.is_file() {
        bail!("Choose an existing absolute Java executable path.");
    }
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if !["java", "java.exe", "javaw.exe"].contains(&name.as_str()) {
        bail!("Choose java or javaw.");
    }
    let probe = if name == "javaw.exe" {
        path.with_file_name("java.exe")
    } else {
        path.to_owned()
    };
    let mut command = tokio::process::Command::new(&probe);
    command
        .arg("-version")
        .stdin(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let output = tokio::time::timeout(Duration::from_secs(10), command.output())
        .await
        .context("Java validation timed out.")??;
    if !output.status.success() {
        bail!("Java executable failed validation.");
    }
    let version = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let major = parse_major(&version)?;
    Ok(Runtime {
        path: probe.to_string_lossy().into_owned(),
        major,
        version: version.lines().next().unwrap_or("").into(),
    })
}
pub async fn detect(root: &Path) -> Vec<Runtime> {
    let executable = if cfg!(windows) { "java.exe" } else { "java" };
    let mut candidates = vec![];
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        candidates.push(PathBuf::from(home).join("bin").join(executable));
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&paths) {
            if path.is_absolute() {
                candidates.push(path.join(executable));
            }
        }
    }
    let mut bases = vec![root.join("java")];
    if cfg!(windows) {
        for variable in ["ProgramFiles", "ProgramW6432"] {
            if let Some(p) = std::env::var_os(variable) {
                for vendor in [
                    "Java",
                    "Eclipse Adoptium",
                    "Microsoft",
                    "Zulu",
                    "Amazon Corretto",
                ] {
                    bases.push(PathBuf::from(&p).join(vendor));
                }
            }
        }
    } else {
        bases.extend([
            PathBuf::from("/usr/lib/jvm"),
            PathBuf::from("/Library/Java/JavaVirtualMachines"),
        ]);
    }
    for base in bases {
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                candidates.push(entry.path().join("bin").join(executable));
                candidates.push(entry.path().join("Contents/Home/bin").join(executable));
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    let mut found = vec![];
    for p in candidates {
        if p.exists() {
            if let Ok(runtime) = validate(&p).await {
                found.push(runtime);
            }
        }
    }
    found
}
pub async fn select(root: &Path, java: &nodeclient_types::Java, major: u32) -> Result<Runtime> {
    if java.mode == "custom" {
        let runtime = validate(Path::new(
            java.path.as_deref().context("Choose a Java executable.")?,
        ))
        .await?;
        if runtime.major != major {
            bail!(
                "Java {major} is required for this Minecraft version. Selected Java is {}.",
                runtime.major
            );
        }
        return Ok(runtime);
    }
    detect(root).await.into_iter().find(|j|j.major==major).with_context(||format!("Java {major} is required for this Minecraft version. Install a compatible runtime or choose Custom Java in instance settings."))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions() {
        assert_eq!(parse_major("java version \"1.8.0_402\"").unwrap(), 8);
        assert_eq!(parse_major("openjdk version \"21.0.5\" 2024").unwrap(), 21);
        assert_eq!(parse_major("openjdk version \"25-ea\"").unwrap(), 25);
        assert!(parse_major("not java").is_err());
    }
}

pub mod managed;
