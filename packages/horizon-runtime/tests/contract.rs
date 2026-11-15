use nodeclient_horizon::{allowed_download, validate_api, Runtime};
use serde_json::json;
use std::path::Path;

#[test]
fn launch_contract_requires_the_supported_version_and_options() {
    let good =
        json!({"launch_api":"0.1.0","minecraft":"26.3","options":[{"name":"--session-stdin"}]});
    assert!(validate_api(&good, &["--session-stdin"]).is_ok());
    assert!(validate_api(&good, &["--game-dir"]).is_err());
    let mut bad = good.clone();
    bad["minecraft"] = "1.21".into();
    assert!(validate_api(&bad, &[]).is_err());
    let mut bad = good;
    bad["launch_api"] = "1.0.0".into();
    assert!(validate_api(&bad, &[]).is_err());
}
#[test]
fn redirects_cannot_escape_the_release_hosts() {
    for url in [
        "https://api.github.com/repos/horizon-rust/horizon",
        "https://release-assets.githubusercontent.com/test",
    ] {
        assert!(allowed_download(&reqwest::Url::parse(url).unwrap()));
    }
    for url in [
        "http://github.com/a",
        "https://github.com.evil.example/a",
        "https://user:pass@github.com/a",
        "https://github.com:444/a",
        "https://example.com/a",
    ] {
        assert!(!allowed_download(&reqwest::Url::parse(url).unwrap()));
    }
}
#[test]
fn native_arguments_keep_instance_isolation_and_secrets_off_command_line() {
    let runtime = Runtime::at(Path::new("C:/Engine Data"));
    let settings = nodeclient_types::Settings { fullscreen: true, ..Default::default() };
    let args = runtime.args(
        Path::new("C:/Worlds/My World"),
        &settings,
        Some(("::1", 25565)),
    );
    for flag in ["--home", "--game-dir"] {
        let index = args.iter().position(|v| v == flag).unwrap();
        assert_eq!(args[index + 1], "C:/Worlds/My World");
    }
    assert!(args.contains(&"--session-stdin".into()));
    assert!(args.contains(&"[::1]:25565".into()));
    assert!(args.contains(&"--fullscreen".into()));
    assert!(!args.iter().any(|s| s.contains("access_token")));
}
#[test]
fn invalid_archive_is_rejected_before_extraction() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("bad.zip");
    std::fs::write(&archive, b"invalid").unwrap();
    let output = dir.path().join("out");
    assert!(nodeclient_horizon::extract(
        &archive,
        &output,
        &std::sync::atomic::AtomicBool::new(false)
    )
    .is_err());
    assert!(!output.exists());
}
