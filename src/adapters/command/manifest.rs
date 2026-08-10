//! The manifest that says how to drive a package manager as a command.
//!
//! An adapter written this way is data rather than code: it names the argv for
//! each operation and how to read what comes back. A manager that already
//! answers in JSON needs nothing more, which is why this is the common path and
//! a sandboxed plugin is the exception.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde::Deserialize;

/// The schema version this build reads.
///
/// A manifest declaring a newer one is refused rather than read in part: the
/// fields it gained are exactly the ones that would be ignored in silence.
pub const SCHEMA_VERSION: u32 = 1;

pub const OP_LIST: &str = "list";
pub const OP_LIST_INSTALLED: &str = "list_installed";
pub const OP_LIST_UPDATES: &str = "list_updates";
pub const OP_SEARCH: &str = "search";
pub const OP_INFO: &str = "info";
pub const OP_INSTALL: &str = "install";
pub const OP_REMOVE: &str = "remove";
pub const OP_UPDATE: &str = "update";
pub const OP_SYNC: &str = "sync";
pub const OP_LIST_REPOS: &str = "list_repos";
pub const OP_ADD_REPO: &str = "add_repo";
pub const OP_REMOVE_REPO: &str = "remove_repo";
pub const OP_SET_REPO_ENABLED: &str = "set_repo_enabled";
pub const OP_PATHS: &str = "paths";
pub const OP_DEFAULT_CONFIG: &str = "default_config";
pub const OP_APPLY: &str = "apply";
pub const OP_APPLY_PRUNE: &str = "apply_prune";
pub const OP_APPLY_CHECK: &str = "apply_check";

#[derive(Debug, Clone, Deserialize)]
pub struct CommandManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: Option<String>,
    /// Set when the manager writes colour, so output is cleaned before it is
    /// read. This is a property of the manager rather than of an operation.
    #[serde(default)]
    pub strip_ansi: bool,
    /// How a package is named back to the manager, most specific first. The
    /// first template whose placeholders are all filled is used, so a manager
    /// that can qualify a name uses the qualified form and falls back to the
    /// bare one where it cannot.
    #[serde(default)]
    pub selector: Vec<String>,
    pub detect: Detect,
    /// How the manager acts on packages for everyone rather than for the
    /// person running it. Absent means it cannot, and the mode is not
    /// offered.
    pub system: Option<SystemMode>,
    /// Set when every operation acts system wide, as it does for a manager
    /// that has no per-user notion at all.
    #[serde(default)]
    pub system_only: bool,
    /// The settings the manager can be configured with, named as they appear
    /// in its own configuration file.
    #[serde(default)]
    pub config: Vec<Setting>,
    #[serde(default)]
    pub ops: HashMap<String, Op>,
}

/// One setting, described well enough to offer and to write back.
#[derive(Debug, Clone, Deserialize)]
pub struct Setting {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: SettingKind,
    #[serde(default)]
    pub description: Option<String>,
    /// What to group this under, for a manager with more settings than fit
    /// on one screen.
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub default: Option<toml::Value>,
    /// The choices, for a setting that only accepts some.
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingKind {
    Text,
    Toggle,
    Number,
    Select,
    PathList,
}

/// What changes when acting system wide.
///
/// Managers differ in how they say it: one takes a flag, another is a
/// different binary altogether, and a third only ever works this way.
#[derive(Debug, Clone, Deserialize)]
pub struct SystemMode {
    /// A different binary to run, for a manager that ships one per scope.
    #[serde(default)]
    pub command: Option<String>,
    /// Arguments put before the ones an operation names.
    #[serde(default)]
    pub args: Vec<String>,
    /// Whether this needs privileges the person running aeris does not have.
    #[serde(default)]
    pub elevate: bool,
}

/// What decides whether the manager is usable at all.
#[derive(Debug, Clone, Deserialize)]
pub struct Detect {
    pub command: String,
    /// Arguments that make the manager print its version.
    #[serde(default)]
    pub version: Vec<String>,
    /// The oldest version that speaks the interface this manifest describes.
    #[serde(default)]
    pub min_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Op {
    pub args: Vec<String>,
    pub output: Output,
    /// Maps a field aeris knows to the field the manager reports it under.
    #[serde(default)]
    pub fields: HashMap<String, String>,
    /// Maps the parts of a progress event, for a streaming operation.
    #[serde(default)]
    pub progress: HashMap<String, String>,
    /// Named captures pulled from each line, for the `lines` format.
    #[serde(default)]
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Output {
    pub format: Format,
    /// Which part of a JSON document holds the records, as a path such as
    /// `$.items[*]`. Only field access and a trailing `[*]` are understood.
    #[serde(default)]
    pub select: Option<String>,
    /// Lines to drop before reading, for a manager that prints a header.
    #[serde(default)]
    pub skip_header: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// One JSON document holding every record.
    Json,
    /// One JSON object per line, written as the work happens.
    Ndjson,
    /// Plain text, read with `pattern`.
    Lines,
}

impl CommandManifest {
    pub fn op(&self, name: &str) -> Option<&Op> {
        self.ops.get(name)
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema_version == 0 || self.schema_version > SCHEMA_VERSION {
            return Err(format!(
                "manifest declares schema {}, and this aeris reads up to {SCHEMA_VERSION}",
                self.schema_version
            ));
        }
        if self.id.is_empty() {
            return Err("manifest has no id".into());
        }
        if self.name.is_empty() {
            return Err("manifest has no name".into());
        }
        if self.detect.command.is_empty() {
            return Err("manifest has no detect.command".into());
        }

        for (name, op) in &self.ops {
            if op.args.is_empty() {
                return Err(format!("operation {name} runs no arguments"));
            }
            if let Some(pattern) = &op.pattern {
                regex::Regex::new(pattern)
                    .map_err(|e| format!("operation {name} has an unreadable pattern: {e}"))?;
            } else if op.output.format == Format::Lines && !op.fields.is_empty() {
                // Only an operation expected to yield records needs one. An
                // operation that just runs and prints for a person to watch
                // has nothing to pull out of what it printed.
                return Err(format!("operation {name} reads lines but has no pattern"));
            }
        }

        Ok(())
    }
}

pub fn parse(text: &str) -> Result<CommandManifest, String> {
    let manifest: CommandManifest =
        toml::from_str(text).map_err(|e| format!("could not read manifest: {e}"))?;
    manifest.validate()?;
    Ok(manifest)
}

pub fn load(path: &Path) -> Result<CommandManifest, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    parse(&text)
}

/// Where a manifest written by hand is looked for.
pub fn search_paths() -> Vec<PathBuf> {
    vec![
        crate::xdg::data_home().join("aeris/adapters"),
        PathBuf::from("/usr/local/share/aeris/adapters"),
        PathBuf::from("./adapters"),
    ]
}

/// Read every manifest found on disk, keeping the first of any repeated id.
pub fn discover() -> Vec<(PathBuf, CommandManifest)> {
    let mut found: Vec<(PathBuf, CommandManifest)> = Vec::new();

    for dir in search_paths() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "toml") {
                continue;
            }

            match load(&path) {
                Ok(manifest) => {
                    if found.iter().any(|(_, m)| m.id == manifest.id) {
                        log::debug!(
                            "ignoring {}: an adapter named {} was already found",
                            path.display(),
                            manifest.id
                        );
                        continue;
                    }
                    found.push((path, manifest));
                }
                Err(e) => log::warn!("{}: {e}", path.display()),
            }
        }
    }

    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
schema_version = 1
id = "demo"
name = "Demo"

[detect]
command = "demo"

[ops.search]
args = ["search", "{query}"]
output = { format = "json", select = "$.items[*]" }
fields = { name = "name" }
"#;

    #[test]
    fn a_minimal_manifest_reads() {
        let manifest = parse(MINIMAL).expect("should read");
        assert_eq!(manifest.id, "demo");
        assert!(manifest.op(OP_SEARCH).is_some());
        assert!(manifest.op(OP_INSTALL).is_none());
    }

    #[test]
    fn a_newer_schema_is_refused() {
        let text = MINIMAL.replace("schema_version = 1", "schema_version = 2");
        let err = parse(&text).expect_err("should refuse");
        assert!(err.contains("schema 2"), "{err}");
    }

    #[test]
    fn an_operation_reading_records_from_lines_needs_a_pattern() {
        let text = MINIMAL.replace(
            r#"output = { format = "json", select = "$.items[*]" }"#,
            r#"output = { format = "lines" }"#,
        );
        let err = parse(&text).expect_err("should refuse");
        assert!(err.contains("no pattern"), "{err}");
    }

    #[test]
    fn an_operation_that_only_runs_needs_no_pattern() {
        // Installing prints for a person to watch, and nothing is read back
        // out of it, so there is nothing to describe.
        let text = MINIMAL.replace(
            r#"[ops.search]
args = ["search", "{query}"]
output = { format = "json", select = "$.items[*]" }
fields = { name = "name" }"#,
            r#"[ops.install]
args = ["install", "{selector}"]
output = { format = "lines" }"#,
        );
        parse(&text).expect("should accept");
    }

    #[test]
    fn an_unreadable_pattern_is_refused() {
        let text = MINIMAL.replace(
            r#"output = { format = "json", select = "$.items[*]" }"#,
            r#"output = { format = "lines" }
pattern = "(?P<name>""#,
        );
        let err = parse(&text).expect_err("should refuse");
        assert!(err.contains("unreadable pattern"), "{err}");
    }
}
