//! Loader boundary: Vanilla is the only implementation until the acceptance gate passes.
use anyhow::Result;
use serde_json::Value;
use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};
pub trait MinecraftLoader {
    fn name(&self) -> &'static str;
    fn resolve<'a>(
        &'a self,
        root: &'a Path,
        id: &'a str,
        cancel: Arc<AtomicBool>,
        report: nodeclient_downloader::Reporter,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value>> + Send + 'a>>;
}
pub struct VanillaLoader;
impl MinecraftLoader for VanillaLoader {
    fn name(&self) -> &'static str {
        "vanilla"
    }
    fn resolve<'a>(
        &'a self,
        root: &'a Path,
        id: &'a str,
        cancel: Arc<AtomicBool>,
        report: nodeclient_downloader::Reporter,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value>> + Send + 'a>> {
        Box::pin(crate::version(root, id, cancel, report))
    }
}
