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

#[derive(Debug, Clone, Deserialize)]
pub struct PluginEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub download_url: String,
    #[serde(default)]
    pub checksum_sha256: String,
    #[serde(default)]
    pub manifest_url: String,
    #[serde(default)]
    pub manifest_checksum_sha256: String,
    #[serde(default)]
    pub repo_url: String,
    #[serde(default)]
    pub architectures: Vec<String>,
    #[serde(default)]
    pub min_host_version: Option<String>,
}

fn plugins_dir() -> PathBuf {
    dirs_home()
        .map(|h| h.join(".local/share/aeris/plugins"))
        .unwrap_or_else(|| PathBuf::from("./plugins"))
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
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

pub fn download_plugin(entry: &PluginEntry) -> Result<PathBuf, String> {
    let plugin_dir = plugins_dir().join(&entry.id);
    std::fs::create_dir_all(&plugin_dir)
        .map_err(|e| format!("Failed to create plugin dir: {e}"))?;

    // Download manifest.toml
    if !entry.manifest_url.is_empty() {
        let manifest_bytes = download_bytes(&entry.manifest_url)?;
        if !entry.manifest_checksum_sha256.is_empty() {
            verify_checksum(&manifest_bytes, &entry.manifest_checksum_sha256)?;
        }
        std::fs::write(plugin_dir.join("manifest.toml"), &manifest_bytes)
            .map_err(|e| format!("Failed to write manifest: {e}"))?;
    }

    // Download plugin.wasm
    let wasm_bytes = download_bytes(&entry.download_url)?;
    if !entry.checksum_sha256.is_empty() {
        verify_checksum(&wasm_bytes, &entry.checksum_sha256)?;
    }
    std::fs::write(plugin_dir.join("plugin.wasm"), &wasm_bytes)
        .map_err(|e| format!("Failed to write plugin.wasm: {e}"))?;

    Ok(plugin_dir)
}

pub fn installed_plugin_version(id: &str) -> Option<String> {
    let manifest_path = plugins_dir().join(id).join("manifest.toml");
    let content = std::fs::read_to_string(&manifest_path).ok()?;
    let manifest: crate::adapters::wasm::PluginManifest = toml::from_str(&content).ok()?;
    Some(manifest.adapter.version)
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
