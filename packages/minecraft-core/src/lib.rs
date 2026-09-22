pub mod arguments;
pub mod install;
pub mod metadata;
pub mod rules;
pub use metadata::{manifest, manifest_cached, version, Manifest, VersionEntry};
pub mod loader;
