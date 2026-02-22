use std::path::PathBuf;

use serde::Deserialize;

use crate::core::capabilities::Capabilities;

#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    pub adapter: AdapterMeta,
    #[serde(default)]
    pub capabilities: Capabilities,
    #[serde(default)]
    pub permissions: Permissions,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdapterMeta {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub min_host_version: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Permissions {
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub filesystem: Vec<String>,
    #[serde(default)]
    pub exec_commands: Vec<String>,
}

pub fn plugin_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(data_dir) = dirs_path("~/.local/share/aeris/plugins") {
        paths.push(data_dir);
    }
    paths.push(PathBuf::from("/usr/local/lib/aeris/plugins"));
    paths.push(PathBuf::from("./plugins"));

    paths
}

pub fn discover_plugins() -> Vec<(PathBuf, PluginManifest)> {
    let mut plugins = Vec::new();

    for search_dir in plugin_search_paths() {
        if !search_dir.is_dir() {
            continue;
        }

        let entries = match std::fs::read_dir(&search_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let plugin_dir = entry.path();
            if !plugin_dir.is_dir() {
                continue;
            }

            let manifest_path = plugin_dir.join("manifest.toml");
            let wasm_path = plugin_dir.join("plugin.wasm");

            if !manifest_path.exists() || !wasm_path.exists() {
                continue;
            }

            match load_manifest(&manifest_path) {
                Ok(manifest) => plugins.push((plugin_dir, manifest)),
                Err(e) => {
                    log::warn!(
                        "Failed to load plugin manifest at {}: {}",
                        manifest_path.display(),
                        e
                    );
                }
            }
        }
    }

    plugins
}

pub fn load_manifest(path: &std::path::Path) -> Result<PluginManifest, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read manifest: {e}"))?;

    let manifest: PluginManifest =
        toml::from_str(&content).map_err(|e| format!("Failed to parse manifest: {e}"))?;

    if manifest.adapter.id.is_empty() {
        return Err("Manifest missing adapter.id".into());
    }
    if manifest.adapter.name.is_empty() {
        return Err("Manifest missing adapter.name".into());
    }

    Ok(manifest)
}

fn dirs_path(path: &str) -> Option<PathBuf> {
    if let Some(stripped) = path.strip_prefix("~/") {
        dirs::home_dir().map(|home| home.join(stripped))
    } else {
        Some(PathBuf::from(path))
    }
}

mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}
