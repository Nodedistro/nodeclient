#[cfg(windows)]
#[tokio::test]
async fn passes_session_over_stdin_and_redacts_child_output() {
    use std::sync::{Arc, Mutex};
    let root=tempfile::tempdir().unwrap();std::fs::create_dir(root.path().join("logs")).unwrap();
    let observed=Arc::new(Mutex::new(Vec::new()));let sink=observed.clone();
    let exe=std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let token="fixture-private-session-value";
    let args=vec!["-NoProfile".into(),"-NonInteractive".into(),"-Command".into(),"$p = [Console]::In.ReadToEnd(); [Console]::Out.WriteLine($p)".into()];
    let info=nodeclient_process::run_with_input(&exe,&args,root.path(),Default::default(),vec![token.into()],Arc::new(|_|{}),Arc::new(move|line|sink.lock().unwrap().push(line)),Some(format!(r#"{{"access_token":"{token}"}}"#).into_bytes())).await.unwrap();
    assert_eq!(info.exit_code,Some(0));
    assert!(!observed.lock().unwrap().is_empty());
    assert!(observed.lock().unwrap().iter().all(|line|!line.contains(token)));
    for name in ["logs/nodeclient-console.log","logs/last-process.json"] {
        assert!(!std::fs::read_to_string(root.path().join(name)).unwrap().contains(token));
    }
}
