//! Adapters that drive a package manager by running it.
//!
//! Everything one of these needs is in its manifest, so a manager that can
//! already answer in JSON is supported by data alone. See [`manifest`] for the
//! shape, and [`CommandAdapter`] for what reads it.
//!
//! Stdin is closed for every run, so a manager that prompts sees the end of
//! input rather than leaving the window waiting on an answer it cannot give.

pub mod adapter;
pub mod manifest;
pub mod output;
pub mod version;

pub use adapter::CommandAdapter;

use crate::core::adapter::AdapterError;

/// What a manager that describes itself is asked to print its manifest.
pub const DESCRIBE_ARGS: &[&str] = &["plugin-manifest"];

/// Build an adapter for every manifest written on disk.
pub fn load_all() -> Vec<Result<CommandAdapter, AdapterError>> {
    manifest::discover()
        .into_iter()
        .map(|(path, manifest)| CommandAdapter::new(manifest, Some(path)))
        .collect()
}
