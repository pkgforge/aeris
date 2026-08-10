use std::{fmt::Write, path::PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// The registry format this build reads.
///
/// Same bargain as a manifest's schema version: a listing written to a newer
/// shape is refused rather than half understood. It says nothing about the
/// adapters listed, so adding or updating one leaves it alone.
pub const REGISTRY_VERSION: u32 = 1;

pub const DEFAULT_REGISTRY_URL: &str =
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

/// Where the last registry that was read is kept.
///
/// This is a copy of something fetchable, so it belongs with the caches:
/// losing it costs one request, not any state.
fn cache_path() -> PathBuf {
    crate::xdg::cache_home().join("aeris").join("registry.toml")
}

/// The registry as it was last read, and when that was.
///
/// A listing from yesterday beats an empty page, so long as it is clear it
/// is from yesterday.
pub fn cached_registry() -> Option<(Registry, std::time::SystemTime)> {
    let path = cache_path();
    let text = std::fs::read_to_string(&path).ok()?;
    let registry: Registry = toml::from_str(&text).ok()?;
    if registry.registry.version > REGISTRY_VERSION {
        return None;
    }

    let read_at = std::fs::metadata(&path).ok()?.modified().ok()?;

    Some((registry, read_at))
}

/// Whether the copy on disk is old enough to be worth replacing.
///
/// No copy at all counts as stale, and so does one whose age cannot be told.
pub fn cache_is_stale(within: std::time::Duration) -> bool {
    let Some((_, read_at)) = cached_registry() else {
        return true;
    };

    read_at.elapsed().map(|age| age > within).unwrap_or(true)
}

fn write_cache(text: &str) {
    let path = cache_path();
    let wrote = path
        .parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| std::fs::write(&path, text));

    if let Err(e) = wrote {
        // Worth saying, but not worth failing over: the listing was read.
        log::warn!("could not keep a copy of the registry: {e}");
    }
}

/// Read the registry from an HTTP(S) URL or a local path, falling back to
/// the built-in default when no source is given.
pub fn fetch_registry(url: Option<&str>) -> Result<Registry, String> {
    let url = url.unwrap_or(DEFAULT_REGISTRY_URL);

    let body = read_text(url)?;
    let registry: Registry =
        toml::from_str(&body).map_err(|e| format!("Failed to parse registry: {e}"))?;

    if registry.registry.version > REGISTRY_VERSION {
        return Err(format!(
            "the registry is written in version {}, and this aeris reads up to {REGISTRY_VERSION}",
            registry.registry.version
        ));
    }

    write_cache(&body);

    Ok(registry)
}

/// Fetch an adapter's manifest and put it where aeris looks for one.
///
/// A manifest is read before it is kept, so a broken one is refused here
/// rather than at the next start. The manifest URL may point at the network
/// or at a local file.
pub fn download_plugin(entry: &PluginEntry) -> Result<PathBuf, String> {
    if entry.manifest_url.is_empty() {
        return Err(format!("{} offers no manifest", entry.id));
    }

    let manifest = read_bytes(&entry.manifest_url)?;
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

/// The newer version an installed adapter could be updated to, if any.
///
/// Versions are compared the way a manager's own are, since a registry says
/// whatever the manager says about itself.
pub fn update_for(entry: &PluginEntry) -> Option<String> {
    let installed = installed_plugin_version(&entry.id)?;

    (!entry.version.is_empty()
        && entry.version != installed
        && crate::adapters::command::version::at_least(&entry.version, &installed))
    .then(|| entry.version.clone())
}

pub fn installed_plugin_version(id: &str) -> Option<String> {
    let text = std::fs::read_to_string(adapter_path(id)).ok()?;
    let manifest = crate::adapters::command::manifest::parse(&text).ok()?;

    (!manifest.version.is_empty()).then_some(manifest.version)
}

/// Whether a source is fetched over HTTP rather than read from disk.
fn is_remote(source: &str) -> bool {
    source.starts_with("http://") || source.starts_with("https://")
}

/// Turn a source into a path, dropping a `file://` prefix and expanding `~`.
fn local_path(source: &str) -> PathBuf {
    let stripped = source.strip_prefix("file://").unwrap_or(source);
    shellexpand::tilde(stripped).to_string().into()
}

/// Read bytes from an HTTP(S) URL or a local file.
fn read_bytes(source: &str) -> Result<Vec<u8>, String> {
    if is_remote(source) {
        let resp = ureq::get(source)
            .call()
            .map_err(|e| format!("Download failed: {e}"))?;

        return resp
            .into_body()
            .read_to_vec()
            .map_err(|e| format!("Failed to read download body: {e}"));
    }

    let path = local_path(source);
    std::fs::read(&path).map_err(|e| format!("Failed to read {}: {e}", path.display()))
}

/// Read text from an HTTP(S) URL or a local file.
fn read_text(source: &str) -> Result<String, String> {
    let bytes = read_bytes(source)?;
    String::from_utf8(bytes).map_err(|e| format!("{source} is not valid UTF-8: {e}"))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn offered(id: &str, version: &str) -> PluginEntry {
        PluginEntry {
            id: id.to_string(),
            name: id.to_string(),
            version: version.to_string(),
            description: String::new(),
            manifest_url: String::new(),
            manifest_checksum_sha256: String::new(),
            repo_url: String::new(),
        }
    }

    fn install(id: &str, body: &str) {
        let path = adapter_path(id);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    fn manifest_saying(id: &str, version: &str) -> String {
        format!(
            r#"schema_version = 1
id = "{id}"
name = "Test"
version = "{version}"

[detect]
command = "true"
"#
        )
    }

    #[test]
    fn an_adapter_nobody_has_is_not_an_update() {
        assert_eq!(update_for(&offered("absent-adapter-test", "2.0")), None);
    }

    #[test]
    fn a_newer_version_in_the_registry_is_offered() {
        let id = "update-check-test";
        install(id, &manifest_saying(id, "1.0"));

        assert_eq!(update_for(&offered(id, "1.1")).as_deref(), Some("1.1"));
        // The same version, and an older one, are both nothing to do.
        assert_eq!(update_for(&offered(id, "1.0")), None);
        assert_eq!(update_for(&offered(id, "0.9")), None);

        let _ = std::fs::remove_file(adapter_path(id));
    }

    #[test]
    fn http_is_remote_and_a_path_is_not() {
        assert!(is_remote("https://example.com/registry.toml"));
        assert!(is_remote("http://example.com/registry.toml"));
        assert!(!is_remote("/etc/aeris/registry.toml"));
        assert!(!is_remote("./registry.toml"));
        assert!(!is_remote("file:///etc/aeris/registry.toml"));
    }

    #[test]
    fn a_local_registry_is_read_from_disk() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("aeris-registry-{nanos}.toml"));
        std::fs::write(&path, "[registry]\nversion = 1\nupdated = \"now\"\n").unwrap();

        let registry =
            fetch_registry(Some(path.to_str().unwrap())).expect("should read the registry");
        assert_eq!(registry.registry.version, 1);
        assert!(registry.plugins.is_empty());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_missing_local_registry_explains_itself() {
        let err = fetch_registry(Some("/no/such/aeris-registry.toml"))
            .expect_err("should not read a missing file");
        assert!(err.contains("Failed to read"), "{err}");
    }
}
