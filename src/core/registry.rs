use std::{fmt::Write, path::PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

const DEFAULT_REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/pkgforge/aeris-registry/main/registry.toml";

#[derive(Debug, Deserialize)]
pub struct Registry {
    pub registry: RegistryMeta,
    #[serde(default)]
    pub plugins: Vec<PluginEntry>,
}

#[derive(Debug, Deserialize)]
pub struct RegistryMeta {
    pub version: u32,
    pub updated: String,
}

/// One adapter the registry offers, which is a manifest and nothing more.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub manifest_url: String,
    #[serde(default)]
    pub manifest_checksum_sha256: String,
    #[serde(default)]
    pub repo_url: String,
}

/// Where a manifest fetched from the registry is kept, which is the same
/// place a hand-written one goes.
fn adapter_path(id: &str) -> PathBuf {
    crate::adapters::command::manifest::search_paths()
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from("./adapters"))
        .join(format!("{id}.toml"))
}

pub fn fetch_registry(url: Option<&str>) -> Result<Registry, String> {
    let url = url.unwrap_or(DEFAULT_REGISTRY_URL);

    let resp = ureq::get(url)
        .call()
        .map_err(|e| format!("Failed to fetch registry: {e}"))?;

    let body = resp
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read registry response: {e}"))?;

    let registry: Registry =
        toml::from_str(&body).map_err(|e| format!("Failed to parse registry: {e}"))?;

    Ok(registry)
}

/// Fetch an adapter's manifest and put it where aeris looks for one.
///
/// A manifest is read before it is kept, so a broken one is refused here
/// rather than at the next start.
pub fn download_plugin(entry: &PluginEntry) -> Result<PathBuf, String> {
    if entry.manifest_url.is_empty() {
        return Err(format!("{} offers no manifest", entry.id));
    }

    let manifest = download_bytes(&entry.manifest_url)?;
    if !entry.manifest_checksum_sha256.is_empty() {
        verify_checksum(&manifest, &entry.manifest_checksum_sha256)?;
    }

    let text = String::from_utf8(manifest)
        .map_err(|_| format!("{} sent a manifest that is not text", entry.id))?;
    let parsed = crate::adapters::command::manifest::parse(&text)
        .map_err(|e| format!("{}: {e}", entry.id))?;

    if parsed.id != entry.id {
        return Err(format!(
            "the registry calls this {} and the manifest calls it {}",
            entry.id, parsed.id
        ));
    }

    let path = adapter_path(&entry.id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create adapter dir: {e}"))?;
    }
    std::fs::write(&path, &text).map_err(|e| format!("Failed to write manifest: {e}"))?;

    Ok(path)
}

pub fn remove_plugin(id: &str) -> Result<(), String> {
    let path = adapter_path(id);
    if !path.exists() {
        return Ok(());
    }

    std::fs::remove_file(&path).map_err(|e| format!("Failed to remove {}: {e}", path.display()))
}

pub fn installed_plugin_version(id: &str) -> Option<String> {
    let text = std::fs::read_to_string(adapter_path(id)).ok()?;
    let manifest = crate::adapters::command::manifest::parse(&text).ok()?;

    (!manifest.version.is_empty()).then_some(manifest.version)
}

fn download_bytes(url: &str) -> Result<Vec<u8>, String> {
    let resp = ureq::get(url)
        .call()
        .map_err(|e| format!("Download failed: {e}"))?;

    resp.into_body()
        .read_to_vec()
        .map_err(|e| format!("Failed to read download body: {e}"))
}

fn verify_checksum(data: &[u8], expected_hex: &str) -> Result<(), String> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut actual_hex = String::with_capacity(64);
    for byte in result {
        write!(&mut actual_hex, "{byte:02x}").unwrap();
    }
    if actual_hex != expected_hex {
        return Err(format!(
            "Checksum mismatch: expected {expected_hex}, got {actual_hex}"
        ));
    }
    Ok(())
}
