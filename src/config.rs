use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::app::{AppTheme, View};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AerisConfig {
    pub theme: Option<String>,
    pub startup_view: Option<String>,
    pub default_adapter: Option<String>,
    pub notifications: Option<bool>,
    #[serde(default)]
    pub adapters: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub disabled_adapters: Vec<String>,
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

        let contents = toml::to_string_pretty(self).map_err(|e| e.to_string())?;

        std::fs::write(&path, contents).map_err(|e| e.to_string())
    }

    pub fn is_adapter_disabled(&self, id: &str) -> bool {
        self.disabled_adapters.iter().any(|s| s == id)
    }

    pub fn set_adapter_disabled(&mut self, id: &str, disabled: bool) {
        self.disabled_adapters.retain(|s| s != id);
        if disabled {
            self.disabled_adapters.push(id.to_string());
        }
    }

    pub fn get_adapter_setting(&self, adapter_id: &str, key: &str) -> Option<&str> {
        self.adapters
            .get(adapter_id)
            .and_then(|settings| settings.get(key))
            .map(|s| s.as_str())
    }

    pub fn set_adapter_setting(&mut self, adapter_id: &str, key: &str, value: &str) {
        self.adapters
            .entry(adapter_id.to_string())
            .or_default()
            .insert(key.to_string(), value.to_string());
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
