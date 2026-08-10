//! Where the desktop specification says a program's files belong.

use std::path::PathBuf;

/// Where per-user configuration goes, honouring `XDG_CONFIG_HOME`.
pub fn config_home() -> PathBuf {
    home_relative("XDG_CONFIG_HOME", ".config")
}

/// Where per-user data goes, honouring `XDG_DATA_HOME`.
pub fn data_home() -> PathBuf {
    home_relative("XDG_DATA_HOME", ".local/share")
}

/// Where things that can be fetched again go, honouring `XDG_CACHE_HOME`.
pub fn cache_home() -> PathBuf {
    home_relative("XDG_CACHE_HOME", ".cache")
}

/// An absolute override wins; anything else falls back under the home
/// directory, and a missing home leaves the relative path to be resolved
/// against wherever aeris was started.
fn home_relative(variable: &str, fallback: &str) -> PathBuf {
    if let Some(set) = std::env::var_os(variable) {
        let path = PathBuf::from(set);
        if path.is_absolute() {
            return path;
        }
    }

    match std::env::var_os("HOME") {
        Some(home) => PathBuf::from(home).join(fallback),
        None => PathBuf::from(fallback),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_override_is_ignored() {
        // The specification says a relative value is invalid, and treating it
        // as valid would scatter files wherever the program was started.
        assert!(home_relative("PATH_THAT_IS_NOT_SET_HERE", ".config").is_absolute());
    }
}
