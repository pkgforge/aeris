use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::app::{AppTheme, View};

/// How often to read the registry again when nothing says otherwise.
const DEFAULT_REGISTRY_SYNC: Duration = Duration::from_secs(3 * 60 * 60);

/// Read a duration written the way a person would: `30m`, `3h`, `1d`.
fn parse_interval(written: &str) -> Option<Duration> {
    let (count, unit) = written.split_at(written.len().checked_sub(1)?);
    let count: u64 = count.trim().parse().ok()?;

    let seconds = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 60 * 60,
        "d" => 24 * 60 * 60,
        _ => return None,
    };

    Some(Duration::from_secs(count * seconds))
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AerisConfig {
    pub theme: Option<String>,
    pub startup_view: Option<String>,
    pub default_adapter: Option<String>,
    pub notifications: Option<bool>,
    /// Where the adapter registry is read from: an HTTP(S) URL or a local
    /// path (a bare path or a `file://` URL). Falls back to the built-in
    /// default when unset.
    pub registry_url: Option<String>,
    /// How long the copy of the registry on disk stays good for. Takes the
    /// words soar uses: `always`, `never`, `auto`, or a duration such as
    /// `3h` or `1d`.
    pub registry_sync_interval: Option<String>,
    #[serde(default)]
    pub adapters: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub disabled_adapters: Vec<String>,
}

impl AerisConfig {
    pub fn config_path() -> PathBuf {
        crate::xdg::config_home().join("aeris").join("config.toml")
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

    /// How long to keep the registry listing before reading it again.
    ///
    /// `None` means never look again on its own, which leaves refreshing to
    /// whoever asks for it.
    pub fn registry_sync_interval(&self) -> Option<Duration> {
        match self
            .registry_sync_interval
            .as_deref()
            .unwrap_or("auto")
            .trim()
        {
            "always" => Some(Duration::ZERO),
            "never" => None,
            "auto" => Some(DEFAULT_REGISTRY_SYNC),
            written => parse_interval(written).or(Some(DEFAULT_REGISTRY_SYNC)),
        }
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

#[cfg(test)]
mod tests {
    use super::{AerisConfig, DEFAULT_REGISTRY_SYNC, parse_interval};
    use std::time::Duration;

    fn with_interval(written: &str) -> AerisConfig {
        AerisConfig {
            registry_sync_interval: Some(written.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn an_interval_is_read_the_way_it_is_written() {
        assert_eq!(parse_interval("90s"), Some(Duration::from_secs(90)));
        assert_eq!(parse_interval("30m"), Some(Duration::from_secs(1800)));
        assert_eq!(parse_interval("3h"), Some(Duration::from_secs(10_800)));
        assert_eq!(parse_interval("1d"), Some(Duration::from_secs(86_400)));
        assert_eq!(parse_interval("soon"), None);
        assert_eq!(parse_interval(""), None);
    }

    #[test]
    fn the_words_mean_what_they_do_in_soar() {
        assert_eq!(
            with_interval("always").registry_sync_interval(),
            Some(Duration::ZERO)
        );
        assert_eq!(with_interval("never").registry_sync_interval(), None);
        assert_eq!(
            with_interval("auto").registry_sync_interval(),
            Some(DEFAULT_REGISTRY_SYNC)
        );
    }

    #[test]
    fn saying_nothing_and_saying_nonsense_both_fall_back() {
        assert_eq!(
            AerisConfig::default().registry_sync_interval(),
            Some(DEFAULT_REGISTRY_SYNC)
        );
        assert_eq!(
            with_interval("whenever").registry_sync_interval(),
            Some(DEFAULT_REGISTRY_SYNC)
        );
    }
}
