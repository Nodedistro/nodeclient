use anyhow::{bail, Context, Result};
use serde_json::Value;
pub fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}
pub fn arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86"
    }
}
pub fn allowed(rules: Option<&Value>, custom_resolution: bool) -> Result<bool> {
    let Some(rules) = rules else { return Ok(true) };
    let mut allow = false;
    for rule in rules.as_array().context("Invalid rules array")? {
        let mut matches = true;
        if let Some(os) = rule.get("os") {
            if let Some(name) = os["name"].as_str() {
                matches &= name == os_name();
            }
            if let Some(a) = os["arch"].as_str() {
                matches &= a == arch()
                    || (a == "amd64" && arch() == "x86_64")
                    || (a == "aarch64" && arch() == "arm64");
            }
            let version = os_version();
            if let Some(pattern) = os["version"].as_str() {
                matches &= regex::Regex::new(pattern)?.is_match(&version);
            }
            if let Some(range) = os.get("versionRange") {
                let parse = |s: &str| {
                    s.split('.')
                        .map(|p| p.parse::<u32>().unwrap_or(0))
                        .collect::<Vec<_>>()
                };
                if let Some(min) = range["min"].as_str() {
                    matches &= parse(&version) >= parse(min);
                }
                if let Some(max) = range["max"].as_str() {
                    matches &= parse(&version) < parse(max);
                }
            }
        }
        if let Some(features) = rule.get("features") {
            for (key, value) in features.as_object().context("Invalid feature rule")? {
                let actual = key == "has_custom_resolution" && custom_resolution;
                matches &= actual == value.as_bool().context("Invalid feature value")?;
            }
        }
        if matches {
            allow = match rule["action"].as_str() {
                Some("allow") => true,
                Some("disallow") => false,
                _ => bail!("Unknown rule action"),
            };
        }
    }
    Ok(allow)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rule_order() {
        assert!(allowed(None, false).unwrap());
        assert!(!allowed(
            Some(&serde_json::json!([{"action":"allow"},{"action":"disallow"}])),
            false
        )
        .unwrap());
        assert!(!allowed(
            Some(&serde_json::json!([{"action":"allow","features":{"is_demo_user":true}}])),
            false
        )
        .unwrap());
        assert!(allowed(
            Some(
                &serde_json::json!([{"action":"allow","features":{"has_custom_resolution":true}}])
            ),
            true
        )
        .unwrap());
    }
}

fn os_version() -> String {
    #[cfg(windows)]
    {
        let version = windows_version::OsVersion::current();
        format!("{}.{}.{}", version.major, version.minor, version.build)
    }
    #[cfg(not(windows))]
    {
        sysinfo::System::os_version().unwrap_or_default()
    }
}
#[cfg(test)]
mod platform_tests {
    #[test]
    fn version_rules_use_numeric_os_version() {
        #[cfg(windows)]
        assert!(super::os_version().starts_with("10.0."));
        assert!(super::allowed(
            Some(&serde_json::json!([{"action":"allow","os":{"version":".*"}}])),
            false
        )
        .unwrap());
    }
}
