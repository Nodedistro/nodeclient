//! Downloads only the public native engine; no account or Minecraft resources are accessed.
use anyhow::{Context, Result};
use std::sync::{atomic::AtomicBool, Arc};
#[tokio::main]
async fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("Pass a verification data directory")?;
    let root = std::env::current_dir()?.join(path);
    let runtime =
        nodeclient_horizon::install(&root, Arc::new(AtomicBool::new(false)), Arc::new(|_| {}))
            .await?;
    runtime.check_api().await?;
    println!(
        "Verified pinned Horizon build {}. Minecraft {}. Resources ready: {}",
        nodeclient_horizon::BUILD,
        nodeclient_horizon::MINECRAFT,
        runtime.ready().await?
    );
    Ok(())
}
