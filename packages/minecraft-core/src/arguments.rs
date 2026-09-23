use crate::install::Installation;
use anyhow::{bail, Context, Result};
use nodeclient_types::{Instance, Profile, Settings};
use serde_json::Value;
use std::{collections::HashMap, path::Path};
pub fn substitute(value: &str, variables: &HashMap<String, String>) -> Result<String> {
    let pattern = regex::Regex::new(r"\$\{([^}]+)\}")?;
    let mut output = value.to_owned();
    for cap in pattern.captures_iter(value) {
        let replacement = variables
            .get(&cap[1])
            .with_context(|| format!("Unsupported Minecraft argument placeholder: {}", &cap[1]))?;
        output = output.replace(&cap[0], replacement);
    }
    if output.contains('\0') {
        bail!("Invalid null byte in launch argument");
    }
    Ok(output)
}
fn resolve(list: &Value, vars: &HashMap<String, String>) -> Result<Vec<String>> {
    let mut result = vec![];
    for item in list.as_array().context("Invalid launch arguments")? {
        if let Some(value) = item.as_str() {
            result.push(substitute(value, vars)?);
        } else if crate::rules::allowed(item.get("rules"), true)? {
            let value = &item["value"];
            if let Some(value) = value.as_str() {
                result.push(substitute(value, vars)?);
            } else {
                for v in value.as_array().context("Invalid argument values")? {
                    result.push(substitute(v.as_str().context("Invalid argument")?, vars)?);
                }
            }
        }
    }
    Ok(result)
}
pub fn construct(
    version: &Value,
    installation: &Installation,
    game: &Path,
    instance: &Instance,
    settings: &Settings,
    profile: &Profile,
    access_token: &str,
    client_id: &str,
    server: Option<(&str, u16)>,
) -> Result<Vec<String>> {
    instance.validate()?;
    settings.validate()?;
    let cp = std::env::join_paths(&installation.classpath)?
        .to_string_lossy()
        .into_owned();
    let mut vars = HashMap::new();
    for (key, value) in [
        ("auth_player_name", profile.name.clone()),
        ("auth_uuid", profile.id.clone()),
        ("auth_access_token", access_token.into()),
        (
            "auth_session",
            format!("token:{access_token}:{}", profile.id),
        ),
        ("user_type", "msa".into()),
        ("user_properties", "{}".into()),
        ("version_name", instance.minecraft_version.clone()),
        (
            "version_type",
            version["type"].as_str().unwrap_or("release").into(),
        ),
        ("game_directory", game.to_string_lossy().into_owned()),
        (
            "assets_root",
            installation.assets.to_string_lossy().into_owned(),
        ),
        (
            "game_assets",
            installation.game_assets.to_string_lossy().into_owned(),
        ),
        ("assets_index_name", installation.asset_index.clone()),
        (
            "natives_directory",
            installation.natives.to_string_lossy().into_owned(),
        ),
        ("launcher_name", "NodeClient".into()),
        ("launcher_version", "0.1.0".into()),
        ("classpath", cp.clone()),
        (
            "classpath_separator",
            if cfg!(windows) { ";" } else { ":" }.into(),
        ),
        (
            "library_directory",
            installation
                .assets
                .parent()
                .unwrap()
                .join("libraries")
                .to_string_lossy()
                .into_owned(),
        ),
        ("resolution_width", settings.width.to_string()),
        ("resolution_height", settings.height.to_string()),
        ("clientid", client_id.into()),
        ("auth_xuid", String::new()),
    ] {
        vars.insert(key.into(), value);
    }
    let mut args = vec![
        format!("-Xms{}M", instance.memory.minimum_mb),
        format!("-Xmx{}M", instance.memory.maximum_mb),
    ];
    args.extend(settings.jvm_arguments.clone());
    if version["arguments"]["jvm"].is_array() {
        args.extend(resolve(&version["arguments"]["jvm"], &vars)?);
    } else {
        args.extend([
            format!("-Djava.library.path={}", installation.natives.display()),
            "-cp".into(),
            cp,
        ]);
        if cfg!(target_os = "macos") {
            args.push("-XstartOnFirstThread".into());
        }
    }
    if let Some((arg, path)) = &installation.logging {
        args.push(arg.replace("${path}", &path.to_string_lossy()));
    }
    let main = version["mainClass"]
        .as_str()
        .context("Minecraft main class missing")?;
    if main.starts_with('-')
        || !main
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._$".contains(&c))
    {
        bail!("Invalid Minecraft main class");
    }
    args.push(main.into());
    if version["arguments"]["game"].is_array() {
        args.extend(resolve(&version["arguments"]["game"], &vars)?);
    } else {
        for a in version["minecraftArguments"]
            .as_str()
            .context("Game arguments missing")?
            .split_whitespace()
        {
            args.push(substitute(a, &vars)?);
        }
    }
    if settings.fullscreen {
        args.push("--fullscreen".into());
    }
    if let Some((host, port)) = server {
        args.extend(["--server".into(), host.into(), "--port".into(), port.to_string()]);
    }
    Ok(args)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_shell_split() {
        let mut vars = HashMap::new();
        vars.insert("game_directory".into(), "C:/My Games/test & more".into());
        assert_eq!(
            substitute("${game_directory}", &vars).unwrap(),
            "C:/My Games/test & more"
        );
        assert!(substitute("${missing}", &vars).is_err());
    }
}
#[cfg(test)]
mod security_regressions {
    use super::*;
    #[test]
    fn fully_resolved_arguments_keep_paths_and_tokens_as_separate_values() {
        let version = serde_json::json!({"type":"release","mainClass":"net.minecraft.client.main.Main","arguments":{"jvm":["-cp","${classpath}","-Djava.library.path=${natives_directory}"],"game":["--username","${auth_player_name}","--uuid","${auth_uuid}","--accessToken","${auth_access_token}","--gameDir","${game_directory}",{"rules":[{"action":"allow","features":{"is_demo_user":true}}],"value":"--demo"}]}});
        let installation = Installation {
            classpath: vec![std::path::PathBuf::from("C:/Game Files/client.jar")],
            natives: "C:/Game Files/natives".into(),
            asset_index: "test".into(),
            assets: "C:/Game Files/assets".into(),
            game_assets: "C:/Game Files/assets".into(),
            logging: None,
        };
        let instance:Instance=serde_json::from_value(serde_json::json!({"id":"test","name":"Test fixture","minecraftVersion":"test","loader":{"type":"vanilla"},"java":{"mode":"automatic","path":null},"memory":{"minimumMb":1024,"maximumMb":2048}})).unwrap();
        let profile = Profile {
            id: "0123456789abcdef0123456789abcdef".into(),
            name: "TestFixture".into(),
            ..Default::default()
        };
        let args = construct(
            &version,
            &installation,
            Path::new("C:/Worlds with spaces"),
            &instance,
            &Settings::default(),
            &profile,
            "test-only-token",
            "test-app-id",
            None,
        )
        .unwrap();
        let token = args.iter().position(|s| s == "--accessToken").unwrap();
        assert_eq!(args[token + 1], "test-only-token");
        assert!(args.contains(&"C:/Worlds with spaces".to_owned()));
        assert!(!args.iter().any(|s| s.contains("${") || s == "--demo"));
        assert!(
            !nodeclient_types::redact_secrets(&args.join(" "), &["test-only-token".into()])
                .contains("test-only-token")
        );
    }

    #[test]
    fn server_target_is_separate_arguments() {
        let version = serde_json::json!({"type":"release","mainClass":"net.minecraft.client.main.Main","arguments":{"jvm":[],"game":[]}});
        let installation = Installation {
            classpath: vec![std::path::PathBuf::from("client.jar")],
            natives: "natives".into(),
            asset_index: "test".into(),
            assets: "assets".into(),
            game_assets: "assets".into(),
            logging: None,
        };
        let instance: Instance = serde_json::from_value(serde_json::json!({"id":"test","name":"Test","minecraftVersion":"test","loader":{"type":"vanilla"},"java":{"mode":"automatic","path":null},"memory":{"minimumMb":1024,"maximumMb":2048}})).unwrap();
        let args = construct(&version, &installation, Path::new("game"), &instance, &Settings::default(), &Profile { id: "id".into(), name: "name".into(), ..Default::default() }, "token", "client", Some(("play.example.com", 25565))).unwrap();
        assert!(args.windows(2).any(|w| w == ["--server", "play.example.com"]));
        assert!(args.windows(2).any(|w| w == ["--port", "25565"]));
    }
}
