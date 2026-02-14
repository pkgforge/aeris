use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use toml_edit::DocumentMut;

use crate::app::{AppTheme, View};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AerisConfig {
    pub theme: Option<String>,
    pub startup_view: Option<String>,
    pub default_adapter: Option<String>,
    pub notifications: Option<bool>,
}

impl AerisConfig {
    pub fn config_path() -> PathBuf {
        soar_utils::path::xdg_config_home()
            .join("aeris")
            .join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let mut doc = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.parse::<DocumentMut>().ok())
            .unwrap_or_default();

        set_opt_str(&mut doc, "theme", self.theme.as_deref());
        set_opt_str(&mut doc, "startup_view", self.startup_view.as_deref());
        set_opt_str(&mut doc, "default_adapter", self.default_adapter.as_deref());
        set_opt_bool(&mut doc, "notifications", self.notifications);

        std::fs::write(&path, doc.to_string()).map_err(|e| e.to_string())
    }

    pub fn theme(&self) -> AppTheme {
        match self.theme.as_deref() {
            Some("light") => AppTheme::Light,
            Some("dark") => AppTheme::Dark,
            _ => AppTheme::System,
        }
    }

    pub fn startup_view(&self) -> View {
        match self.startup_view.as_deref() {
            Some("browse") => View::Browse,
            Some("installed") => View::Installed,
            Some("updates") => View::Updates,
            _ => View::Dashboard,
        }
    }
}

pub fn save_repo_enabled(repo_name: &str, enabled: bool) -> Result<(), String> {
    let config_path = soar_config::config::CONFIG_PATH
        .read()
        .unwrap()
        .to_path_buf();

    let content = std::fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
    let mut doc: DocumentMut = content
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;

    if let Some(repos) = doc
        .get_mut("repositories")
        .and_then(|v| v.as_array_of_tables_mut())
    {
        for repo in repos.iter_mut() {
            if repo.get("name").and_then(|v| v.as_str()) == Some(repo_name) {
                repo["enabled"] = toml_edit::value(enabled);
                break;
            }
        }
    }

    std::fs::write(&config_path, doc.to_string()).map_err(|e| e.to_string())
}

fn set_opt_str(doc: &mut DocumentMut, key: &str, value: Option<&str>) {
    match value {
        Some(v) => doc[key] = toml_edit::value(v),
        None => {
            doc.remove(key);
        }
    }
}

fn set_opt_bool(doc: &mut DocumentMut, key: &str, value: Option<bool>) {
    match value {
        Some(v) => doc[key] = toml_edit::value(v),
        None => {
            doc.remove(key);
        }
    }
}
