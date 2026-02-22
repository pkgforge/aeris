use std::path::{Path, PathBuf};

use super::manifest::Permissions;

#[derive(Clone)]
pub struct HostState {
    pub adapter_id: String,
    pub permissions: Permissions,
    pub data_dir: PathBuf,
}

impl HostState {
    pub fn new(adapter_id: String, permissions: Permissions) -> Self {
        let data_dir = data_dir_for(&adapter_id);
        Self {
            adapter_id,
            permissions,
            data_dir,
        }
    }

    pub fn validate_path(&self, path: &str) -> Result<PathBuf, String> {
        let requested = Path::new(path);

        // Reject obviously malicious patterns
        let path_str = requested.to_string_lossy();
        if path_str.contains("..") {
            return Err(format!("Path traversal not allowed: {path}"));
        }

        // Canonicalize relative to data_dir if not absolute
        let resolved = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.data_dir.join(requested)
        };

        // Always allow access within the plugin's data directory
        if resolved.starts_with(&self.data_dir) {
            return Ok(resolved);
        }

        // Check against explicitly allowed filesystem paths
        for allowed in &self.permissions.filesystem {
            let allowed_path = if let Some(stripped) = allowed.strip_prefix("~/") {
                if let Some(home) = home_dir() {
                    home.join(stripped)
                } else {
                    continue;
                }
            } else {
                PathBuf::from(allowed)
            };

            if resolved.starts_with(&allowed_path) {
                return Ok(resolved);
            }
        }

        Err(format!(
            "Path not allowed for plugin '{}': {path}",
            self.adapter_id
        ))
    }

    pub fn validate_command(&self, cmd: &str) -> Result<String, String> {
        // Extract the basename (handle both "/usr/bin/apt" and "apt")
        let basename = Path::new(cmd)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(cmd);

        if !self
            .permissions
            .exec_commands
            .contains(&basename.to_string())
        {
            return Err(format!(
                "Command '{}' not in allowlist for plugin '{}'",
                basename, self.adapter_id
            ));
        }

        // Resolve to full path
        which::which(basename).map_or_else(
            |_| Err(format!("Command not found: {basename}")),
            |full_path| Ok(full_path.to_string_lossy().into_owned()),
        )
    }

    pub fn has_network_permission(&self) -> bool {
        self.permissions.network
    }
}

fn data_dir_for(adapter_id: &str) -> PathBuf {
    let base = home_dir()
        .map(|h| h.join(".local/share/aeris/plugin-data"))
        .unwrap_or_else(|| PathBuf::from("/tmp/aeris/plugin-data"));
    base.join(adapter_id)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
