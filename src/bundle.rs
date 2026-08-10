//! What a packed build has to undo before it starts anything else.
//!
//! A single-file build unpacks its own libraries and puts them first on the
//! library path. That is right for aeris and wrong for everything it runs: a
//! manager started with them inherited is loaded against the wrong libc and
//! dies before it can say anything.

/// The environment a command should be given instead of the one inherited.
///
/// Each entry is a variable and what to do with it: `Some` to set, `None` to
/// remove. Empty when this is not a packed build, so an ordinary build pays
/// nothing.
pub fn outside(root: Option<&str>) -> Vec<(&'static str, Option<String>)> {
    let Some(root) = root else {
        return Vec::new();
    };

    let mut changes = Vec::new();

    // The bundle's own libraries go; anything else on the path was not ours
    // to take away.
    if let Ok(paths) = std::env::var("LD_LIBRARY_PATH") {
        let kept: Vec<&str> = paths
            .split(':')
            .filter(|path| !path.is_empty() && !path.starts_with(root))
            .collect();

        changes.push((
            "LD_LIBRARY_PATH",
            (!kept.is_empty()).then(|| kept.join(":")),
        ));
    }

    // The same for where a renderer looks for its drivers, which a bundle
    // also points at itself.
    for name in [
        "LIBGL_DRIVERS_PATH",
        "LIBVA_DRIVERS_PATH",
        "GBM_BACKENDS_PATH",
        "__EGL_VENDOR_LIBRARY_DIRS",
    ] {
        if std::env::var(name).is_ok_and(|value| value.starts_with(root)) {
            changes.push((name, None));
        }
    }

    changes
}

/// A command that will not hand a packed build's own libraries to whatever it
/// starts.
///
/// Every command aeris runs should be made this way. A manager loaded against
/// the bundle's libc dies before it can answer, and the failure reads as the
/// manager being broken rather than as ours.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let mut command = std::process::Command::new(program);

    for (name, value) in outside(root().as_deref()) {
        match value {
            Some(value) => command.env(name, value),
            None => command.env_remove(name),
        };
    }

    command
}

/// Where this build unpacked itself, or nothing when it is not packed.
pub fn root() -> Option<String> {
    std::env::var("ONELF_DIR")
        .ok()
        .filter(|dir| !dir.is_empty())
}

#[cfg(test)]
mod tests {
    use super::outside;

    #[test]
    fn an_ordinary_build_changes_nothing() {
        assert!(outside(None).is_empty());
    }

    #[test]
    fn a_packed_build_keeps_only_what_was_not_its_own() {
        // SAFETY: single-threaded test, and the variable is read back here.
        unsafe {
            std::env::set_var(
                "LD_LIBRARY_PATH",
                "/run/user/1000/onelf-aeris-abc/lib:/usr/lib64:/opt/mine/lib",
            );
        }

        let changes = outside(Some("/run/user/1000/onelf-aeris-abc"));
        let path = changes
            .iter()
            .find(|(name, _)| *name == "LD_LIBRARY_PATH")
            .and_then(|(_, value)| value.clone());

        assert_eq!(path.as_deref(), Some("/usr/lib64:/opt/mine/lib"));

        unsafe {
            std::env::set_var("LD_LIBRARY_PATH", "/run/user/1000/onelf-aeris-abc/lib");
        }
        let changes = outside(Some("/run/user/1000/onelf-aeris-abc"));
        let entry = changes.iter().find(|(name, _)| *name == "LD_LIBRARY_PATH");

        // Nothing of ours is left, so the variable goes rather than staying
        // behind as an empty one.
        assert_eq!(entry.map(|(_, value)| value.clone()), Some(None));

        unsafe {
            std::env::remove_var("LD_LIBRARY_PATH");
        }
    }
}
