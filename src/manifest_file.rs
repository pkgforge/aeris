//! Reading and writing a declarative package file.
//!
//! The file belongs to the manager, not to aeris, so this edits it in place
//! with the comments and ordering the user wrote left alone. Nothing here
//! knows which manager it is: the location comes from the adapter.

use std::path::Path;

use crate::views::manifest::ManifestEntrySnapshot;

#[derive(Debug, Clone)]
pub enum ManifestLoadError {
    FileMissing,
    Parse(String),
    Other(String),
}

impl std::fmt::Display for ManifestLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileMissing => write!(f, "Manifest file is missing"),
            Self::Parse(e) => write!(f, "{e}"),
            Self::Other(e) => write!(f, "{e}"),
        }
    }
}

pub fn split_list(s: &str, sep: char) -> Vec<String> {
    s.split(sep)
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn upsert_str_inline(t: &mut toml_edit::InlineTable, key: &str, value: &str) {
    if value.is_empty() {
        t.remove(key);
    } else {
        t.insert(key, value.into());
    }
}

fn upsert_bool_inline(t: &mut toml_edit::InlineTable, key: &str, value: bool) {
    if value {
        t.insert(key, true.into());
    } else {
        t.remove(key);
    }
}

fn upsert_str_table(t: &mut toml_edit::Table, key: &str, value: &str) {
    if value.is_empty() {
        t.remove(key);
    } else {
        t.insert(key, toml_edit::value(value.to_string()));
    }
}

fn upsert_bool_table(t: &mut toml_edit::Table, key: &str, value: bool) {
    if value {
        t.insert(key, toml_edit::value(true));
    } else {
        t.remove(key);
    }
}

fn upsert_inline_fields(
    t: &mut toml_edit::InlineTable,
    snap: &crate::views::manifest::ManifestEntrySnapshot,
) {
    let v = if snap.version.is_empty() {
        "*".to_string()
    } else {
        snap.version.clone()
    };
    t.insert("version", v.into());

    upsert_str_inline(t, "repo", &snap.repo);
    upsert_str_inline(t, "url", &snap.url);
    upsert_str_inline(t, "github", &snap.github);
    upsert_str_inline(t, "gitlab", &snap.gitlab);
    upsert_str_inline(t, "asset_pattern", &snap.asset_pattern);
    upsert_str_inline(t, "tag_pattern", &snap.tag_pattern);
    upsert_str_inline(t, "profile", &snap.profile);
    upsert_bool_inline(t, "include_prerelease", snap.include_prerelease);
    upsert_bool_inline(t, "pinned", snap.pinned);
    upsert_bool_inline(t, "binary_only", snap.binary_only);

    if snap.install_patterns.is_empty() {
        t.remove("install_patterns");
    } else {
        let mut arr = toml_edit::Array::new();
        for p in split_list(&snap.install_patterns, ',') {
            arr.push(p);
        }
        t.insert("install_patterns", toml_edit::Value::Array(arr));
    }

    let want_commands = !snap.build_commands.is_empty();
    let want_deps = !snap.build_dependencies.is_empty();
    if want_commands || want_deps {
        let existing_build = match t.get_mut("build") {
            Some(toml_edit::Value::InlineTable(b)) => Some(b),
            _ => None,
        };
        let owned;
        let build_ref = if let Some(b) = existing_build {
            b
        } else {
            owned = toml_edit::InlineTable::new();
            t.insert("build", toml_edit::Value::InlineTable(owned.clone()));
            match t.get_mut("build").unwrap() {
                toml_edit::Value::InlineTable(b) => b,
                _ => unreachable!(),
            }
        };
        if want_commands {
            let mut arr = toml_edit::Array::new();
            for c in split_list(&snap.build_commands, ';') {
                arr.push(c);
            }
            build_ref.insert("commands", toml_edit::Value::Array(arr));
        } else {
            build_ref.remove("commands");
        }
        if want_deps {
            let mut arr = toml_edit::Array::new();
            for d in split_list(&snap.build_dependencies, ',') {
                arr.push(d);
            }
            build_ref.insert("dependencies", toml_edit::Value::Array(arr));
        } else {
            build_ref.remove("dependencies");
        }
    } else if let Some(toml_edit::Value::InlineTable(b)) = t.get_mut("build") {
        b.remove("commands");
        b.remove("dependencies");
        if b.is_empty() {
            t.remove("build");
        }
    }
}

fn upsert_table_fields(
    t: &mut toml_edit::Table,
    snap: &crate::views::manifest::ManifestEntrySnapshot,
) {
    let v = if snap.version.is_empty() {
        "*".to_string()
    } else {
        snap.version.clone()
    };
    t.insert("version", toml_edit::value(v));

    upsert_str_table(t, "repo", &snap.repo);
    upsert_str_table(t, "url", &snap.url);
    upsert_str_table(t, "github", &snap.github);
    upsert_str_table(t, "gitlab", &snap.gitlab);
    upsert_str_table(t, "asset_pattern", &snap.asset_pattern);
    upsert_str_table(t, "tag_pattern", &snap.tag_pattern);
    upsert_str_table(t, "profile", &snap.profile);
    upsert_bool_table(t, "include_prerelease", snap.include_prerelease);
    upsert_bool_table(t, "pinned", snap.pinned);
    upsert_bool_table(t, "binary_only", snap.binary_only);

    if snap.install_patterns.is_empty() {
        t.remove("install_patterns");
    } else {
        let mut arr = toml_edit::Array::new();
        for p in split_list(&snap.install_patterns, ',') {
            arr.push(p);
        }
        t.insert("install_patterns", toml_edit::value(arr));
    }

    let want_commands = !snap.build_commands.is_empty();
    let want_deps = !snap.build_dependencies.is_empty();
    if want_commands || want_deps {
        let build_item = t
            .entry("build")
            .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
        if let toml_edit::Item::Table(build) = build_item {
            build.set_implicit(false);
            if want_commands {
                let mut arr = toml_edit::Array::new();
                for c in split_list(&snap.build_commands, ';') {
                    arr.push(c);
                }
                build.insert("commands", toml_edit::value(arr));
            } else {
                build.remove("commands");
            }
            if want_deps {
                let mut arr = toml_edit::Array::new();
                for d in split_list(&snap.build_dependencies, ',') {
                    arr.push(d);
                }
                build.insert("dependencies", toml_edit::value(arr));
            } else {
                build.remove("dependencies");
            }
        }
    } else if let Some(toml_edit::Item::Table(b)) = t.get_mut("build") {
        b.remove("commands");
        b.remove("dependencies");
        if b.is_empty() {
            t.remove("build");
        }
    }
}

fn read_or_new(path: &Path) -> std::result::Result<toml_edit::DocumentMut, String> {
    if path.exists() {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        content
            .parse::<toml_edit::DocumentMut>()
            .map_err(|e| e.to_string())
    } else {
        Ok("[packages]\n"
            .parse::<toml_edit::DocumentMut>()
            .expect("static template parses"))
    }
}

fn atomic_write(path: &Path, doc: &toml_edit::DocumentMut) -> std::result::Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut tmp = path.to_path_buf();
    let file_name = tmp
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "packages.toml".to_string());
    tmp.set_file_name(format!("{file_name}.tmp"));
    std::fs::write(&tmp, doc.to_string()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Whether the file is there and readable, which is worth knowing before
/// asking the manager to work out what it would change.
///
/// A file the user has half-edited gets a clearer answer this way than the
/// manager's own complaint about it.
pub fn check_readable(path: &Path) -> std::result::Result<(), ManifestLoadError> {
    if !path.exists() {
        return Err(ManifestLoadError::FileMissing);
    }

    let text = std::fs::read_to_string(path)
        .map_err(|e| ManifestLoadError::Other(format!("could not read {}: {e}", path.display())))?;

    toml::from_str::<toml::Value>(&text)
        .map(|_| ())
        .map_err(|e| ManifestLoadError::Parse(e.to_string()))
}

/// Read one declaration, or nothing when the file does not have it.
pub fn read_entry(
    path: &Path,
    name: &str,
) -> std::result::Result<Option<ManifestEntrySnapshot>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let document: toml::Value = toml::from_str(&text).map_err(|e| e.to_string())?;

    let Some(spec) = document
        .get("packages")
        .and_then(toml::Value::as_table)
        .and_then(|packages| packages.get(name))
    else {
        return Ok(None);
    };

    let mut snapshot = ManifestEntrySnapshot {
        name: name.to_string(),
        ..Default::default()
    };

    // A declaration is either the version on its own or a table saying more.
    if let Some(version) = spec.as_str() {
        snapshot.version = unstarred(version);
        return Ok(Some(snapshot));
    }

    let Some(options) = spec.as_table() else {
        return Ok(Some(snapshot));
    };

    let text_at = |key: &str| {
        options
            .get(key)
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let flag_at = |key: &str| {
        options
            .get(key)
            .and_then(toml::Value::as_bool)
            .unwrap_or(false)
    };
    let joined = |key: &str, separator: &str| {
        options
            .get(key)
            .and_then(toml::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .collect::<Vec<_>>()
                    .join(separator)
            })
            .unwrap_or_default()
    };

    snapshot.version = unstarred(&text_at("version"));
    snapshot.repo = text_at("repo");
    snapshot.url = text_at("url");
    snapshot.github = text_at("github");
    snapshot.gitlab = text_at("gitlab");
    snapshot.asset_pattern = text_at("asset_pattern");
    snapshot.tag_pattern = text_at("tag_pattern");
    snapshot.include_prerelease = flag_at("include_prerelease");
    snapshot.profile = text_at("profile");
    snapshot.pinned = flag_at("pinned");
    snapshot.binary_only = flag_at("binary_only");
    snapshot.install_patterns = joined("install_patterns", ", ");

    if let Some(build) = options.get("build").and_then(toml::Value::as_table) {
        let list = |key: &str, separator: &str| {
            build
                .get(key)
                .and_then(toml::Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(toml::Value::as_str)
                        .collect::<Vec<_>>()
                        .join(separator)
                })
                .unwrap_or_default()
        };
        snapshot.build_commands = list("commands", "; ");
        snapshot.build_dependencies = list("dependencies", ", ");
    }

    Ok(Some(snapshot))
}

/// Write one declaration, keeping the form it was already written in.
pub fn write_entry(
    path: &Path,
    snapshot: &ManifestEntrySnapshot,
) -> std::result::Result<(), String> {
    let mut document = read_or_new(path)?;
    let packages = document
        .entry("packages")
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
    let packages = packages
        .as_table_mut()
        .ok_or_else(|| "packages entry is not a table".to_string())?;

    let existing = packages.get(&snapshot.name);
    let was_simple = matches!(
        existing,
        Some(toml_edit::Item::Value(toml_edit::Value::String(_)))
    );
    let was_inline = matches!(
        existing,
        Some(toml_edit::Item::Value(toml_edit::Value::InlineTable(_)))
    );
    let was_table = matches!(existing, Some(toml_edit::Item::Table(_)));
    let existed = existing.is_some();
    // Anything the user wrote above the declaration belongs to its key, and
    // replacing the entry would otherwise take it along.
    let surroundings = packages
        .key(&snapshot.name)
        .map(|key| key.leaf_decor().clone());

    if !snapshot.needs_detailed() && (was_simple || !existed) {
        packages.insert(&snapshot.name, toml_edit::value(starred(&snapshot.version)));

        if let (Some(surroundings), Some(mut key)) =
            (surroundings, packages.key_mut(&snapshot.name))
        {
            *key.leaf_decor_mut() = surroundings;
        }

        return atomic_write(path, &document);
    }

    if was_inline {
        if let Some(toml_edit::Item::Value(toml_edit::Value::InlineTable(table))) =
            packages.get_mut(&snapshot.name)
        {
            upsert_inline_fields(table, snapshot);
        }
    } else if was_table {
        if let Some(toml_edit::Item::Table(table)) = packages.get_mut(&snapshot.name) {
            upsert_table_fields(table, snapshot);
        }
    } else {
        let mut table = toml_edit::Table::new();
        upsert_table_fields(&mut table, snapshot);
        packages.insert(&snapshot.name, toml_edit::Item::Table(table));
    }

    atomic_write(path, &document)
}

/// Drop one declaration, leaving the rest of the file as it was.
pub fn remove_entry(path: &Path, name: &str) -> std::result::Result<(), String> {
    let mut document = read_or_new(path)?;

    if let Some(packages) = document.get_mut("packages").and_then(|i| i.as_table_mut()) {
        packages.remove(name);
    }

    atomic_write(path, &document)
}

/// Replace every declaration with the given names and versions.
pub fn replace_packages(
    path: &Path,
    entries: &[(String, String)],
) -> std::result::Result<(), String> {
    let mut document = read_or_new(path)?;
    let mut packages = toml_edit::Table::new();

    for (name, version) in entries {
        packages.insert(name, toml_edit::value(starred(version)));
    }

    document.insert("packages", toml_edit::Item::Table(packages));

    atomic_write(path, &document)
}

/// A declaration that accepts any version is written as `*` and shown empty.
fn unstarred(version: &str) -> String {
    if version == "*" {
        String::new()
    } else {
        version.to_string()
    }
}

fn starred(version: &str) -> String {
    if version.is_empty() {
        "*".to_string()
    } else {
        version.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("aeris-manifest-{name}.toml"));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn a_declaration_with_nothing_but_a_version_stays_simple() {
        let path = scratch("simple");
        let snapshot = ManifestEntrySnapshot {
            name: "rg".into(),
            version: "15.2.0".into(),
            ..Default::default()
        };

        write_entry(&path, &snapshot).expect("should write");
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains(r#"rg = "15.2.0""#), "{written}");

        let read = read_entry(&path, "rg")
            .expect("should read")
            .expect("should be there");
        assert_eq!(read.version, "15.2.0");
        assert!(read.repo.is_empty());
    }

    #[test]
    fn accepting_any_version_is_written_as_a_star_and_read_back_empty() {
        let path = scratch("any-version");
        let snapshot = ManifestEntrySnapshot {
            name: "rg".into(),
            ..Default::default()
        };

        write_entry(&path, &snapshot).expect("should write");
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains(r#"rg = "*""#)
        );

        let read = read_entry(&path, "rg")
            .expect("should read")
            .expect("should be there");
        assert_eq!(read.version, "");
    }

    #[test]
    fn a_declaration_saying_more_survives_a_round_trip() {
        let path = scratch("detailed");
        let snapshot = ManifestEntrySnapshot {
            name: "tool".into(),
            version: "1.2.3".into(),
            repo: "soarpkgs".into(),
            github: "owner/tool".into(),
            asset_pattern: "*.AppImage".into(),
            include_prerelease: true,
            pinned: true,
            build_commands: "make; make install".into(),
            build_dependencies: "gcc, make".into(),
            install_patterns: "bin/*, share/*".into(),
            ..Default::default()
        };

        write_entry(&path, &snapshot).expect("should write");
        let read = read_entry(&path, "tool")
            .expect("should read")
            .expect("should be there");

        assert_eq!(read.version, "1.2.3");
        assert_eq!(read.repo, "soarpkgs");
        assert_eq!(read.github, "owner/tool");
        assert_eq!(read.asset_pattern, "*.AppImage");
        assert!(read.include_prerelease);
        assert!(read.pinned);
        assert_eq!(read.build_commands, "make; make install");
        assert_eq!(read.build_dependencies, "gcc, make");
        assert_eq!(read.install_patterns, "bin/*, share/*");
    }

    #[test]
    fn what_the_user_wrote_around_a_declaration_is_left_alone() {
        let path = scratch("comments");
        std::fs::write(
            &path,
            "# my packages\n[packages]\n# the good one\nrg = \"1.0\"\neza = \"2.0\"\n",
        )
        .unwrap();

        let snapshot = ManifestEntrySnapshot {
            name: "rg".into(),
            version: "1.1".into(),
            ..Default::default()
        };
        write_entry(&path, &snapshot).expect("should write");

        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("# my packages"), "{written}");
        assert!(written.contains("# the good one"), "{written}");
        assert!(written.contains(r#"rg = "1.1""#), "{written}");
        assert!(written.contains(r#"eza = "2.0""#), "{written}");
    }

    #[test]
    fn a_declaration_that_is_not_there_reads_as_nothing() {
        let path = scratch("missing");
        assert!(read_entry(&path, "rg").expect("should read").is_none());

        std::fs::write(&path, "[packages]\neza = \"2.0\"\n").unwrap();
        assert!(read_entry(&path, "rg").expect("should read").is_none());
    }

    #[test]
    fn removing_and_replacing_leave_the_file_readable() {
        let path = scratch("replace");
        std::fs::write(&path, "[packages]\nrg = \"1.0\"\neza = \"2.0\"\n").unwrap();

        remove_entry(&path, "rg").expect("should remove");
        assert!(read_entry(&path, "rg").unwrap().is_none());
        assert!(read_entry(&path, "eza").unwrap().is_some());

        replace_packages(&path, &[("fd".into(), String::new())]).expect("should replace");
        assert!(read_entry(&path, "eza").unwrap().is_none());
        assert_eq!(read_entry(&path, "fd").unwrap().unwrap().version, "");
    }
}
