//! Live official-file installation test. No authentication or game process is fabricated.
use anyhow::Result;
use std::sync::{atomic::AtomicBool, Arc};
#[tokio::main]
async fn main() -> Result<()> {
    let root = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("Provide an absolute disposable data directory"))?;
    if !root.is_absolute() {
        anyhow::bail!("Directory must be absolute");
    }
    std::fs::create_dir_all(&root)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let last =
        std::sync::Mutex::new(std::time::Instant::now() - std::time::Duration::from_secs(10));
    let report: nodeclient_downloader::Reporter = Arc::new(move |p| {
        let mut t = last.lock().unwrap();
        if t.elapsed().as_secs() >= 5 || p.completed == p.total {
            println!(
                "{}: {}/{} files, {} bytes",
                p.phase, p.completed, p.total, p.bytes
            );
            *t = std::time::Instant::now();
        }
    });
    let manifest = nodeclient_core::manifest().await?;
    let id = manifest.latest.release;
    println!("Installing official release {id}");
    let version = nodeclient_core::version(&root, &id, cancel.clone(), report.clone()).await?;
    let game = nodeclient_types::safe_join(&root, "instances/installation-smoke")?;
    std::fs::create_dir_all(&game)?;
    let installation = nodeclient_core::install::install(
        &root,
        &game,
        &version,
        6,
        cancel.clone(),
        report.clone(),
    )
    .await?;
    println!(
        "Verified {} classpath entries; asset index {}",
        installation.classpath.len(),
        installation.asset_index
    );
    println!(
        "Required Java major: {}",
        nodeclient_core::metadata::java_major(&version)
    );
    Ok(())
}
