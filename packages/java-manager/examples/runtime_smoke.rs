use anyhow::Result;
use std::sync::{atomic::AtomicBool, Arc};
#[tokio::main]
async fn main() -> Result<()> {
    let root = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("Provide an absolute disposable data directory"))?;
    let component = std::env::args()
        .nth(2)
        .ok_or_else(|| anyhow::anyhow!("Provide the official Java component"))?;
    let major = std::env::args()
        .nth(3)
        .ok_or_else(|| anyhow::anyhow!("Provide required Java major"))?
        .parse()?;
    if !root.is_absolute() {
        anyhow::bail!("Directory must be absolute");
    }
    let last =
        std::sync::Mutex::new(std::time::Instant::now() - std::time::Duration::from_secs(10));
    let report: nodeclient_downloader::Reporter = Arc::new(move |p| {
        let mut t = last.lock().unwrap();
        if t.elapsed().as_secs() >= 5 || p.completed == p.total {
            println!("{}: {}/{} files", p.phase, p.completed, p.total);
            *t = std::time::Instant::now();
        }
    });
    let runtime = nodeclient_java::managed::install(
        &root,
        &component,
        major,
        6,
        Arc::new(AtomicBool::new(false)),
        report,
    )
    .await?;
    println!("Verified and executed managed Java: {}", runtime.version);
    Ok(())
}
