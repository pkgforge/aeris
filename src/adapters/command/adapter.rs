//! An adapter that drives a package manager by running it.

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
};

use serde_json::Value;

use crate::core::{
    adapter::{
        Adapter, AdapterError, AdapterInfo, HealthStatus, ProgressEvent, ProgressSender, Result,
    },
    capabilities::Capabilities,
    package::{InstallResult, InstalledPackage, Package, PackageDetail, Update},
    privilege::PackageMode,
};

use super::{
    manifest::{
        self, CommandManifest, Format, OP_INFO, OP_INSTALL, OP_LIST, OP_LIST_INSTALLED,
        OP_LIST_UPDATES, OP_REMOVE, OP_SEARCH, OP_SYNC, OP_UPDATE, Op,
    },
    output, version,
};

/// How many lines of a failed run are kept to explain it.
const ERROR_LINES: usize = 3;

type Values = HashMap<String, String>;

pub struct CommandAdapter {
    manifest: Arc<CommandManifest>,
    program: PathBuf,
    info: AdapterInfo,
    capabilities: Capabilities,
}

impl CommandAdapter {
    /// Build an adapter from a manifest, refusing a manager that is missing or
    /// too old to speak the interface the manifest describes.
    pub fn new(manifest: CommandManifest, source: Option<PathBuf>) -> Result<Self> {
        let program = which::which(&manifest.detect.command).map_err(|_| {
            AdapterError::PluginError(format!(
                "{} describes {}, which is not installed",
                manifest.id, manifest.detect.command
            ))
        })?;

        let found = detect_version(&program, &manifest);
        if let Some(required) = &manifest.detect.min_version {
            match &found {
                Some(found) if version::at_least(found, required) => {}
                Some(found) => {
                    return Err(AdapterError::PluginError(format!(
                        "{} is {found}, and {} needs {required} or newer",
                        manifest.detect.command, manifest.id
                    )));
                }
                None => {
                    return Err(AdapterError::PluginError(format!(
                        "{} did not report a version, so {required} could not be confirmed",
                        manifest.detect.command
                    )));
                }
            }
        }

        let capabilities = capabilities_from(&manifest);
        let info = AdapterInfo {
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            // The version of the manager being driven, not of the manifest,
            // because that is what decides how it behaves.
            version: found.unwrap_or_else(|| manifest.version.clone()),
            capabilities,
            enabled: true,
            is_builtin: false,
            plugin_path: source,
            description: manifest.description.clone(),
            icon: manifest.icon.clone(),
        };

        Ok(Self {
            manifest: Arc::new(manifest),
            program,
            info,
            capabilities,
        })
    }

    /// Ask a manager that describes itself for its own manifest.
    ///
    /// A manager shipping the description inside the binary cannot fall out of
    /// step with it, which is the whole reason to prefer this over a manifest
    /// kept somewhere else.
    pub fn from_command(command: &str, args: &[&str]) -> Result<Self> {
        let program = which::which(command)
            .map_err(|_| AdapterError::PluginError(format!("{command} is not installed")))?;

        let printed = Command::new(&program)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| AdapterError::PluginError(format!("could not run {command}: {e}")))?;

        if !printed.status.success() {
            return Err(AdapterError::PluginError(format!(
                "{command} {} did not describe itself",
                args.join(" ")
            )));
        }

        let text = String::from_utf8_lossy(&printed.stdout);
        let manifest = manifest::parse(&text)
            .map_err(|e| AdapterError::PluginError(format!("{command}: {e}")))?;

        Self::new(manifest, None)
    }

    fn op(&self, name: &str) -> Result<&Op> {
        self.manifest.op(name).ok_or(AdapterError::NotSupported)
    }

    /// Run an operation, returning everything it printed.
    async fn run(
        &self,
        op_name: &str,
        values: Values,
        progress: Option<ProgressSender>,
        package_id: String,
    ) -> Result<String> {
        let op = self.op(op_name)?.clone();
        let manifest = self.manifest.clone();
        let program = self.program.clone();
        let adapter_id = self.info.id.clone();

        tokio::task::spawn_blocking(move || {
            let args = fill_args(&op, &values)?;
            let context = progress.map(|sender| Progress {
                sender,
                adapter_id,
                package_id,
                map: op.progress.clone(),
                format: op.output.format,
                pattern: op.pattern.clone(),
            });

            run(&program, &args, manifest.strip_ansi, context.as_ref())
        })
        .await
        .map_err(|e| AdapterError::Other(format!("could not wait for the run: {e}")))?
    }

    /// Read the records a query operation printed.
    async fn query(&self, op_name: &str, values: Values) -> Result<Vec<Value>> {
        let printed = self.run(op_name, values, None, String::new()).await?;
        let op = self.op(op_name)?;
        output::records(op, &printed, self.manifest.strip_ansi).map_err(AdapterError::ParseError)
    }

    /// Name a package the way its manager expects to hear it back.
    fn selector(&self, values: &Values) -> String {
        self.manifest
            .selector
            .iter()
            .find_map(|template| output::fill(template, values))
            .or_else(|| values.get("name").cloned())
            .unwrap_or_default()
    }

    fn to_package(&self, record: &Value, fields: &HashMap<String, String>) -> Option<Package> {
        let mut values: Values = fields
            .keys()
            .filter_map(|key| Some((key.clone(), output::text(record, fields, key)?)))
            .collect();

        let name = values.get("name")?.clone();
        values.insert("name".into(), name.clone());

        Some(Package {
            id: self.selector(&values),
            name,
            version: values.get("version").cloned().unwrap_or_default(),
            adapter_id: self.info.id.clone(),
            description: values.get("description").cloned(),
            size: output::number(record, fields, "size"),
            homepage: values.get("homepage").cloned(),
            license: values.get("license").cloned(),
            installed: output::flag(record, fields, "installed").unwrap_or(false),
            update_available: false,
            category: values.get("category").cloned(),
            tags: Vec::new(),
            icon_url: None,
        })
    }

    /// The values an operation's arguments are filled from for one package.
    fn values_for(&self, package: &Package) -> Values {
        Values::from([
            ("selector".into(), package.id.clone()),
            ("name".into(), package.name.clone()),
            ("version".into(), package.version.clone()),
        ])
    }

    /// Run an operation over packages, once each where it names one and once
    /// in total where it does not.
    async fn run_over(
        &self,
        op_name: &str,
        packages: &[Package],
        progress: Option<ProgressSender>,
    ) -> Result<Vec<InstallResult>> {
        let op = self.op(op_name)?;

        if !takes_a_package(op) {
            self.run(op_name, Values::new(), progress, String::new())
                .await?;
            return Ok(Vec::new());
        }

        if packages.is_empty() {
            return Err(AdapterError::Other(format!(
                "{} names a package, and none was given",
                op_name
            )));
        }

        let mut results = Vec::with_capacity(packages.len());
        for package in packages {
            let outcome = self
                .run(
                    op_name,
                    self.values_for(package),
                    progress.clone(),
                    package.id.clone(),
                )
                .await;

            if let Some(sender) = &progress {
                let _ = sender.send(match &outcome {
                    Ok(_) => ProgressEvent::Completed {
                        adapter_id: self.info.id.clone(),
                        package_id: package.id.clone(),
                    },
                    Err(e) => ProgressEvent::Failed {
                        adapter_id: self.info.id.clone(),
                        package_id: package.id.clone(),
                        error: e.to_string(),
                    },
                });
            }

            results.push(InstallResult {
                package_name: package.name.clone(),
                package_id: package.id.clone(),
                version: package.version.clone(),
                success: outcome.is_ok(),
                error: outcome.err().map(|e| e.to_string()),
            });
        }

        Ok(results)
    }
}

#[async_trait::async_trait]
impl Adapter for CommandAdapter {
    fn info(&self) -> &AdapterInfo {
        &self.info
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    async fn search(
        &self,
        query: &str,
        limit: Option<usize>,
        _mode: PackageMode,
    ) -> Result<Vec<Package>> {
        let values = Values::from([("query".into(), query.to_string())]);
        let records = self.query(OP_SEARCH, values).await?;
        let fields = &self.op(OP_SEARCH)?.fields;

        let found = records
            .iter()
            .filter_map(|record| self.to_package(record, fields));

        Ok(match limit {
            Some(limit) => found.take(limit).collect(),
            None => found.collect(),
        })
    }

    async fn package_detail(&self, package_id: &str) -> Result<PackageDetail> {
        let values = Values::from([
            ("selector".into(), package_id.to_string()),
            ("name".into(), package_id.to_string()),
        ]);
        let records = self.query(OP_INFO, values).await?;
        let fields = &self.op(OP_INFO)?.fields;

        let record = records
            .first()
            .ok_or_else(|| AdapterError::PackageNotFound(package_id.to_string()))?;
        let package = self
            .to_package(record, fields)
            .ok_or_else(|| AdapterError::PackageNotFound(package_id.to_string()))?;

        Ok(PackageDetail {
            download_url: output::text(record, fields, "download_url"),
            package,
            dependencies: Vec::new(),
            screenshots: Vec::new(),
            readme: None,
            maintainers: Vec::new(),
            build_date: output::text(record, fields, "build_date"),
            variants: Vec::new(),
            snapshots: Vec::new(),
        })
    }

    async fn install(
        &self,
        packages: &[Package],
        progress: Option<ProgressSender>,
        _mode: PackageMode,
    ) -> Result<Vec<InstallResult>> {
        self.run_over(OP_INSTALL, packages, progress).await
    }

    async fn remove(
        &self,
        packages: &[Package],
        progress: Option<ProgressSender>,
        _mode: PackageMode,
    ) -> Result<()> {
        let results = self.run_over(OP_REMOVE, packages, progress).await?;

        match results.iter().find(|r| !r.success) {
            Some(failed) => {
                Err(AdapterError::Other(failed.error.clone().unwrap_or_else(
                    || format!("could not remove {}", failed.package_name),
                )))
            }
            None => Ok(()),
        }
    }

    async fn update(
        &self,
        packages: &[Package],
        progress: Option<ProgressSender>,
        _mode: PackageMode,
    ) -> Result<Vec<InstallResult>> {
        self.run_over(OP_UPDATE, packages, progress).await
    }

    async fn list_installed(&self, _mode: PackageMode) -> Result<Vec<InstalledPackage>> {
        let records = self.query(OP_LIST_INSTALLED, Values::new()).await?;
        let fields = &self.op(OP_LIST_INSTALLED)?.fields;

        Ok(records
            .iter()
            .filter_map(|record| {
                let mut package = self.to_package(record, fields)?;
                package.installed = true;

                Some(InstalledPackage {
                    installed_at: output::text(record, fields, "installed_at").unwrap_or_default(),
                    install_size: output::number(record, fields, "size").unwrap_or(0),
                    install_path: output::text(record, fields, "path"),
                    pinned: output::flag(record, fields, "pinned").unwrap_or(false),
                    auto_installed: false,
                    is_healthy: output::flag(record, fields, "healthy").unwrap_or(true),
                    profile: output::text(record, fields, "profile"),
                    package,
                })
            })
            .collect())
    }

    async fn list_updates(&self, _mode: PackageMode) -> Result<Vec<Update>> {
        let records = self.query(OP_LIST_UPDATES, Values::new()).await?;
        let fields = &self.op(OP_LIST_UPDATES)?.fields;

        Ok(records
            .iter()
            .filter_map(|record| {
                let package = self.to_package(record, fields)?;
                let current = output::text(record, fields, "current_version")
                    .unwrap_or_else(|| package.version.clone());
                let new_version = output::text(record, fields, "new_version")?;

                Some(Update {
                    current_version: current,
                    new_version,
                    download_size: output::number(record, fields, "size"),
                    is_security: false,
                    changelog_url: None,
                    package,
                })
            })
            .collect())
    }

    async fn sync(&self, progress: Option<ProgressSender>) -> Result<()> {
        self.run(OP_SYNC, Values::new(), progress, String::new())
            .await?;
        Ok(())
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        let count = match self.query(OP_LIST, Values::new()).await {
            Ok(records) => Some(records.len() as u64),
            Err(AdapterError::NotSupported) => None,
            Err(e) => {
                return Ok(HealthStatus {
                    healthy: false,
                    message: e.to_string(),
                    ..Default::default()
                });
            }
        };

        Ok(HealthStatus {
            healthy: true,
            message: format!("{} {}", self.manifest.detect.command, self.info.version),
            package_count: count,
            repo_count: None,
            cache_size: None,
        })
    }
}

fn capabilities_from(manifest: &CommandManifest) -> Capabilities {
    let has = |name: &str| manifest.op(name).is_some();

    Capabilities {
        can_search: has(OP_SEARCH),
        can_install: has(OP_INSTALL),
        can_remove: has(OP_REMOVE),
        can_update: has(OP_UPDATE),
        can_list: has(OP_LIST) || has(OP_LIST_INSTALLED),
        can_list_updates: has(OP_LIST_UPDATES),
        can_sync: has(OP_SYNC),
        has_package_detail: has(OP_INFO),
        has_size_info: manifest
            .ops
            .values()
            .any(|op| op.fields.contains_key("size")),
        supports_user_packages: true,
        ..Default::default()
    }
}

/// Whether an operation names the package it acts on.
fn takes_a_package(op: &Op) -> bool {
    op.args.iter().any(|arg| arg.contains('{'))
}

fn fill_args(op: &Op, values: &Values) -> Result<Vec<String>> {
    op.args
        .iter()
        .map(|arg| {
            output::fill(arg, values)
                .ok_or_else(|| AdapterError::PluginError(format!("nothing to fill `{arg}` with")))
        })
        .collect()
}

/// What a streaming operation needs to report progress.
struct Progress {
    sender: ProgressSender,
    adapter_id: String,
    package_id: String,
    map: HashMap<String, String>,
    format: Format,
    pattern: Option<String>,
}

fn run(
    program: &Path,
    args: &[String],
    strip_ansi: bool,
    progress: Option<&Progress>,
) -> Result<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AdapterError::Other(format!("could not run {}: {e}", program.display())))?;

    // Drained on its own thread: a manager writing more diagnostics than the
    // pipe holds would otherwise block while we are still reading stdout.
    let mut errors = child.stderr.take();
    let draining = std::thread::spawn(move || {
        let mut collected = String::new();
        if let Some(errors) = errors.as_mut() {
            let _ = errors.read_to_string(&mut collected);
        }
        collected
    });

    let reporter = progress.map(Reporter::new);
    let mut printed = String::new();

    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout)
            .lines()
            .map_while(std::result::Result::ok)
        {
            let line = if strip_ansi {
                output::strip_ansi(&line)
            } else {
                line
            };

            if let Some(reporter) = &reporter {
                reporter.report(&line);
            }

            printed.push_str(&line);
            printed.push('\n');
        }
    }

    let status = child.wait().map_err(|e| {
        AdapterError::Other(format!("could not wait for {}: {e}", program.display()))
    })?;
    let errors = draining.join().unwrap_or_default();

    if !status.success() {
        return Err(AdapterError::Other(format!(
            "{} {} failed: {}",
            program.display(),
            args.join(" "),
            last_lines(&errors)
        )));
    }

    Ok(printed)
}

/// Keeps the tail of what a failed run complained about.
fn last_lines(text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let tail = lines.len().saturating_sub(ERROR_LINES);
    let tail = lines[tail..].join("; ");

    if tail.is_empty() {
        "it said nothing".into()
    } else {
        tail
    }
}

struct Reporter<'a> {
    progress: &'a Progress,
    pattern: Option<regex::Regex>,
}

impl<'a> Reporter<'a> {
    fn new(progress: &'a Progress) -> Self {
        let pattern = progress
            .pattern
            .as_deref()
            .and_then(|pattern| regex::Regex::new(pattern).ok());

        Self { progress, pattern }
    }

    fn report(&self, line: &str) {
        if self.progress.map.is_empty() {
            return;
        }

        let Some(record) = self.record(line) else {
            return;
        };
        if let Some(event) = self.event(&record) {
            let _ = self.progress.sender.send(event);
        }
    }

    fn record(&self, line: &str) -> Option<Value> {
        match self.progress.format {
            Format::Ndjson => serde_json::from_str(line).ok(),
            Format::Lines => {
                let found = self.pattern.as_ref()?.captures(line)?;
                let mut record = serde_json::Map::new();

                for name in self.pattern.as_ref()?.capture_names().flatten() {
                    if let Some(matched) = found.name(name) {
                        record.insert(
                            name.to_string(),
                            Value::String(matched.as_str().to_string()),
                        );
                    }
                }

                Some(Value::Object(record))
            }
            // A document is only whole once the run ends, so it says nothing
            // while the work is still going.
            Format::Json => None,
        }
    }

    fn event(&self, record: &Value) -> Option<ProgressEvent> {
        let map = &self.progress.map;
        let adapter_id = self.progress.adapter_id.clone();

        let named = |key: &str| -> Option<&Value> { record.get(map.get(key)?) };
        let text = |key: &str| -> Option<String> {
            match named(key)? {
                Value::String(s) => Some(s.clone()),
                Value::Null => None,
                other => Some(other.to_string()),
            }
        };
        let count = |key: &str| -> Option<u64> {
            match named(key)? {
                Value::Number(n) => n.as_u64(),
                Value::String(s) => s.parse().ok(),
                _ => None,
            }
        };

        let package_id = text("message")
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| self.progress.package_id.clone());

        match (count("current"), count("total")) {
            (Some(current), Some(total)) => Some(ProgressEvent::Download {
                adapter_id,
                package_id,
                current_bytes: current,
                total_bytes: total,
            }),
            _ => {
                // Without a fraction to report, the stage name is all there is
                // to say, so an event carrying no name is not worth sending.
                let phase = text("event")?;
                Some(ProgressEvent::Phase {
                    adapter_id,
                    package_id,
                    phase,
                    progress_percent: 0.0,
                })
            }
        }
    }
}

fn detect_version(program: &Path, manifest: &CommandManifest) -> Option<String> {
    if manifest.detect.version.is_empty() {
        return None;
    }

    let printed = Command::new(program)
        .args(&manifest.detect.version)
        .stdin(Stdio::null())
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&printed.stdout);
    let text = if text.trim().is_empty() {
        String::from_utf8_lossy(&printed.stderr).into_owned()
    } else {
        text.into_owned()
    };

    version::extract(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(text: &str) -> CommandManifest {
        manifest::parse(text).expect("should read")
    }

    const DEMO: &str = r#"
schema_version = 1
id = "demo"
name = "Demo"
selector = ["{family}/{name}", "{name}"]

[detect]
command = "demo"

[ops.search]
args = ["search", "{query}"]
output = { format = "json", select = "$.items[*]" }
fields = { name = "name", family = "family", version = "version", size = "size" }

[ops.install]
args = ["install", "{selector}"]
output = { format = "ndjson" }
progress = { event = "type", current = "current", total = "total", message = "pkg_name" }
"#;

    fn adapter(manifest: CommandManifest) -> CommandAdapter {
        let capabilities = capabilities_from(&manifest);
        CommandAdapter {
            info: AdapterInfo {
                id: manifest.id.clone(),
                name: manifest.name.clone(),
                version: "1.0".into(),
                capabilities,
                enabled: true,
                is_builtin: false,
                plugin_path: None,
                description: String::new(),
                icon: None,
            },
            manifest: Arc::new(manifest),
            program: PathBuf::from("demo"),
            capabilities,
        }
    }

    #[test]
    fn capabilities_follow_the_operations_a_manifest_declares() {
        let capabilities = capabilities_from(&manifest(DEMO));
        assert!(capabilities.can_search);
        assert!(capabilities.can_install);
        assert!(!capabilities.can_remove);
        assert!(!capabilities.can_list_updates);
        assert!(capabilities.has_size_info);
    }

    #[test]
    fn a_package_takes_the_most_specific_name_its_manager_understands() {
        let adapter = adapter(manifest(DEMO));
        let fields = &adapter.manifest.op(OP_SEARCH).unwrap().fields;

        let qualified: Value =
            serde_json::from_str(r#"{"name":"cat","family":"busybox","version":"1.0"}"#).unwrap();
        assert_eq!(
            adapter.to_package(&qualified, fields).unwrap().id,
            "busybox/cat"
        );

        let bare: Value = serde_json::from_str(r#"{"name":"cat","version":"1.0"}"#).unwrap();
        assert_eq!(adapter.to_package(&bare, fields).unwrap().id, "cat");
    }

    #[test]
    fn a_record_without_a_name_is_skipped() {
        let adapter = adapter(manifest(DEMO));
        let fields = &adapter.manifest.op(OP_SEARCH).unwrap().fields;
        let record: Value = serde_json::from_str(r#"{"version":"1.0"}"#).unwrap();

        assert!(adapter.to_package(&record, fields).is_none());
    }

    #[test]
    fn arguments_are_filled_from_the_package_being_acted_on() {
        let manifest = manifest(DEMO);
        let op = manifest.op(OP_INSTALL).unwrap();
        let values = Values::from([("selector".into(), "busybox/cat".into())]);

        assert_eq!(fill_args(op, &values).unwrap(), ["install", "busybox/cat"]);
        assert!(fill_args(op, &Values::new()).is_err());
    }

    #[test]
    fn an_operation_naming_no_package_runs_once() {
        let manifest = manifest(DEMO);
        assert!(takes_a_package(manifest.op(OP_INSTALL).unwrap()));

        let sync = manifest::parse(
            r#"
schema_version = 1
id = "demo"
name = "Demo"

[detect]
command = "demo"

[ops.sync]
args = ["sync"]
output = { format = "ndjson" }
"#,
        )
        .unwrap();
        assert!(!takes_a_package(sync.op(OP_SYNC).unwrap()));
    }

    fn reporter_for(progress: &Progress) -> Reporter<'_> {
        Reporter::new(progress)
    }

    fn progress(
        format: Format,
    ) -> (
        Progress,
        tokio::sync::mpsc::UnboundedReceiver<ProgressEvent>,
    ) {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        (
            Progress {
                sender,
                adapter_id: "demo".into(),
                package_id: "cat".into(),
                map: HashMap::from([
                    ("event".to_string(), "type".to_string()),
                    ("current".to_string(), "current".to_string()),
                    ("total".to_string(), "total".to_string()),
                    ("message".to_string(), "pkg_name".to_string()),
                ]),
                format,
                pattern: None,
            },
            receiver,
        )
    }

    #[test]
    fn a_stream_carrying_a_fraction_reports_a_download() {
        let (progress, mut receiver) = progress(Format::Ndjson);
        reporter_for(&progress)
            .report(r#"{"type":"download_progress","pkg_name":"cat","current":50,"total":100}"#);

        match receiver.try_recv().expect("should have reported") {
            ProgressEvent::Download {
                package_id,
                current_bytes,
                total_bytes,
                ..
            } => {
                assert_eq!(package_id, "cat");
                assert_eq!((current_bytes, total_bytes), (50, 100));
            }
            other => panic!("reported {other:?}"),
        }
    }

    #[test]
    fn a_stream_carrying_only_a_stage_reports_a_phase() {
        let (progress, mut receiver) = progress(Format::Ndjson);
        reporter_for(&progress).report(r#"{"type":"installing","pkg_name":"cat"}"#);

        match receiver.try_recv().expect("should have reported") {
            ProgressEvent::Phase { phase, .. } => assert_eq!(phase, "installing"),
            other => panic!("reported {other:?}"),
        }
    }

    #[test]
    fn a_line_that_is_not_an_event_reports_nothing() {
        let (progress, mut receiver) = progress(Format::Ndjson);
        let reporter = reporter_for(&progress);

        reporter.report("this is not json");
        reporter.report(r#"{"unrelated":true}"#);

        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn what_a_failed_run_said_is_kept_to_the_tail() {
        assert_eq!(last_lines("a\nb\nc\nd\n"), "b; c; d");
        assert_eq!(last_lines("   \n"), "it said nothing");
    }

    /// A stand-in manager, so the running and reading are exercised for real
    /// without needing anything installed.
    fn fake_manager(name: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!("aeris-fake-{name}"));
        std::fs::write(
            &path,
            r#"#!/bin/sh
case "$1" in
  --version) echo "demo 1.2.0" ;;
  search)
    echo '{"items":[{"name":"cat","family":"busybox","version":"1.0","size":10}],"total":1}'
    ;;
  install)
    # More diagnostics than a pipe holds, to prove they are drained while
    # stdout is still being read.
    i=0
    while [ $i -lt 10000 ]; do echo "diagnostic line $i" >&2; i=$((i + 1)); done
    echo '{"type":"download_progress","pkg_name":"cat","current":5,"total":10}'
    echo '{"type":"installing","pkg_name":"cat"}'
    ;;
  boom)
    echo "it went wrong" >&2
    exit 1
    ;;
esac
"#,
        )
        .expect("should write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("should be runnable");

        path
    }

    fn manifest_for(program: &Path, min_version: &str) -> String {
        format!(
            r#"
schema_version = 1
id = "demo"
name = "Demo"
selector = ["{{family}}/{{name}}", "{{name}}"]

[detect]
command = "{}"
version = ["--version"]
min_version = "{min_version}"

[ops.search]
args = ["search", "{{query}}"]
output = {{ format = "json", select = "$.items[*]" }}
fields = {{ name = "name", family = "family", version = "version", size = "size" }}

[ops.install]
args = ["install", "{{selector}}"]
output = {{ format = "ndjson" }}
progress = {{ event = "type", current = "current", total = "total", message = "pkg_name" }}

[ops.remove]
args = ["boom", "{{selector}}"]
output = {{ format = "ndjson" }}
"#,
            program.display()
        )
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("should build a runtime")
            .block_on(future)
    }

    #[test]
    fn a_manager_older_than_the_manifest_asks_for_is_refused() {
        let program = fake_manager("too-old");
        let manifest = manifest(&manifest_for(&program, "2.0.0"));

        let Err(err) = CommandAdapter::new(manifest, None) else {
            panic!("should refuse a manager older than the manifest asks for");
        };
        assert!(err.to_string().contains("1.2.0"), "{err}");
    }

    #[test]
    fn a_manager_is_searched_and_installed_through_its_manifest() {
        let program = fake_manager("whole-path");
        let manifest = manifest(&manifest_for(&program, "1.0.0"));
        let adapter = CommandAdapter::new(manifest, None).expect("should accept");

        assert_eq!(adapter.info().version, "1.2.0");

        let found =
            block_on(adapter.search("cat", None, PackageMode::User)).expect("should search");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "busybox/cat");
        assert_eq!(found[0].size, Some(10));

        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let results = block_on(adapter.install(&found, Some(sender), PackageMode::User))
            .expect("should install");
        assert!(results[0].success, "{:?}", results[0].error);

        let mut reported = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            reported.push(event);
        }
        assert!(
            matches!(reported.first(), Some(ProgressEvent::Download { .. })),
            "reported {reported:?}"
        );
        assert!(
            matches!(reported.last(), Some(ProgressEvent::Completed { .. })),
            "reported {reported:?}"
        );
    }

    #[test]
    fn a_run_that_fails_is_reported_with_what_it_said() {
        let program = fake_manager("failing");
        let manifest = manifest(&manifest_for(&program, "1.0.0"));
        let adapter = CommandAdapter::new(manifest, None).expect("should accept");

        let package = Package {
            id: "busybox/cat".into(),
            name: "cat".into(),
            version: "1.0".into(),
            adapter_id: "demo".into(),
            description: None,
            size: None,
            homepage: None,
            license: None,
            installed: true,
            update_available: false,
            category: None,
            tags: Vec::new(),
            icon_url: None,
        };

        let err =
            block_on(adapter.remove(&[package], None, PackageMode::User)).expect_err("should fail");
        assert!(err.to_string().contains("it went wrong"), "{err}");
    }
}
