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
    config::{AdapterConfig, ConfigField, ConfigFieldType, ConfigSchema, ConfigValue},
    package::{InstallResult, InstalledPackage, Package, PackageDetail, Update},
    privilege::PackageMode,
    profile::Profile,
    repository::Repository,
};
use crate::views::manifest::{ManifestApplyReport, ManifestDiff, ManifestEntry};

use super::{
    manifest::{
        self, CommandManifest, Format, OP_ADD_REPO, OP_APPLY, OP_APPLY_CHECK, OP_APPLY_PRUNE,
        OP_DEFAULT_CONFIG, OP_INFO, OP_INSTALL, OP_LIST, OP_LIST_INSTALLED, OP_LIST_REPOS,
        OP_LIST_UPDATES, OP_PATHS, OP_REMOVE, OP_REMOVE_REPO, OP_SEARCH, OP_SET_REPO_ENABLED,
        OP_SYNC, OP_UPDATE, Op, Setting, SettingKind,
    },
    output, version,
};

/// How many lines of a failed run are kept to explain it.
const ERROR_LINES: usize = 3;

type Values = HashMap<String, String>;

pub struct CommandAdapter {
    manifest: Arc<CommandManifest>,
    /// The binary for each scope, where one is installed. A manager shipping
    /// one per scope may well have only one of them here, and works in that
    /// scope alone.
    user_program: Option<PathBuf>,
    system_program: Option<PathBuf>,
    info: AdapterInfo,
    capabilities: Capabilities,
}

impl CommandAdapter {
    /// Build an adapter from a manifest, refusing a manager that is missing or
    /// too old to speak the interface the manifest describes.
    pub fn new(manifest: CommandManifest, source: Option<PathBuf>) -> Result<Self> {
        let user_program = which::which(&manifest.detect.command).ok();

        // A system mode naming no command of its own is the same binary with
        // more arguments, so it stands or falls with that one.
        let system_program = match &manifest.system {
            Some(config) => match &config.command {
                Some(named) => which::which(named).ok(),
                None => user_program.clone(),
            },
            None => None,
        };

        let program = match (manifest.system_only, &user_program, &system_program) {
            // Nothing installed for any scope this manager works in.
            (_, None, None) | (true, _, None) => {
                return Err(AdapterError::PluginError(missing(&manifest)));
            }
            (true, _, Some(system)) => system.clone(),
            (false, Some(user), _) => user.clone(),
            (false, None, Some(system)) => system.clone(),
        };

        let found = detect_version(&program, &manifest);
        if let Some(required) = &manifest.detect.min_version {
            match &found {
                Some(found) if version::at_least(found, required) => {}
                Some(found) => {
                    return Err(AdapterError::PluginError(format!(
                        "{} is {found}, older than the {required} this needs",
                        manifest.detect.command
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

        let capabilities =
            scoped_capabilities(&manifest, user_program.is_some(), system_program.is_some());
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
            user_program,
            system_program,
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

    /// Mark this as one aeris ships with rather than one that was added.
    ///
    /// A built-in can be turned off but not taken away, so there is always
    /// something to turn back on.
    pub fn as_builtin(mut self) -> Self {
        self.info.is_builtin = true;
        self
    }

    /// What to run for an operation in the given mode: the binary, whatever
    /// goes before the operation's own arguments, and whether it needs
    /// privileges the person running aeris does not have.
    fn invocation(&self, mode: PackageMode) -> Result<(PathBuf, Vec<String>, bool)> {
        let system = self.manifest.system_only || mode == PackageMode::System;

        if !system {
            let program = self
                .user_program
                .clone()
                .ok_or(AdapterError::NotSupported)?;
            return Ok((program, Vec::new(), false));
        }

        let Some(config) = &self.manifest.system else {
            return Err(AdapterError::NotSupported);
        };

        let program = self
            .system_program
            .clone()
            .ok_or(AdapterError::NotSupported)?;

        Ok((program, config.args.clone(), config.elevate))
    }

    /// Whichever binary is there, for the things that are the same in any
    /// scope: asking its version, asking where its files are.
    fn program(&self) -> Result<&PathBuf> {
        self.user_program
            .as_ref()
            .or(self.system_program.as_ref())
            .ok_or(AdapterError::NotSupported)
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
        mode: PackageMode,
    ) -> Result<Ran> {
        let op = self.op(op_name)?.clone();
        let manifest = self.manifest.clone();
        let (program, before, elevate) = self.invocation(mode)?;
        let adapter_id = self.info.id.clone();

        tokio::task::spawn_blocking(move || {
            let mut args = before;
            args.extend(fill_args(&op, &values)?);
            let context = progress.map(|sender| Progress {
                sender,
                adapter_id,
                package_id,
                map: op.progress.clone(),
                format: op.output.format,
                pattern: op.pattern.clone(),
            });

            run(
                &program,
                &args,
                manifest.strip_ansi,
                context.as_ref(),
                elevate,
            )
        })
        .await
        .map_err(|e| AdapterError::Other(format!("could not wait for the run: {e}")))?
    }

    /// Read the records a query operation printed.
    async fn query(&self, op_name: &str, values: Values, mode: PackageMode) -> Result<Vec<Value>> {
        let ran = self.run(op_name, values, None, String::new(), mode).await?;

        // A question that goes unanswered is a failure however the manager
        // exited, and what it complained about says more than the empty
        // answer does.
        if ran.printed.trim().is_empty() {
            return Err(AdapterError::Other(format!(
                "{} answered nothing: {}",
                self.manifest.detect.command, ran.complained
            )));
        }

        let op = self.op(op_name)?;
        output::records(op, &ran.printed, self.manifest.strip_ansi)
            .map_err(AdapterError::ParseError)
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

    /// Where the manager keeps its files, read without waiting.
    ///
    /// A frontend needs these while it is still starting up, before there is
    /// an event loop to wait on.
    pub fn file_paths(&self) -> Result<HashMap<String, String>> {
        let op = self.op(OP_PATHS)?;
        let printed = run(
            self.program()?,
            &fill_args(op, &Values::new())?,
            false,
            None,
            false,
        )?
        .printed;

        let records = output::records(op, &printed, self.manifest.strip_ansi)
            .map_err(AdapterError::ParseError)?;
        let record = records
            .first()
            .ok_or_else(|| AdapterError::Other("the manager reported no paths".into()))?;

        Ok(op
            .fields
            .keys()
            .filter_map(|key| Some((key.clone(), output::text(record, &op.fields, key)?)))
            .collect())
    }

    /// Where the manager keeps one of its files.
    async fn file(&self, what: &str) -> Result<String> {
        self.paths().await?.remove(what).ok_or_else(|| {
            AdapterError::Other(format!("{} does not say where its {what} is", self.info.id))
        })
    }

    /// Read the manager's configuration file, which is empty until it is
    /// written for the first time.
    async fn read_config(&self) -> Result<toml::Value> {
        let path = self.file("config").await?;
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Ok(toml::Value::Table(toml::Table::new()));
        };

        toml::from_str(&text)
            .map_err(|e| AdapterError::ParseError(format!("could not read {path}: {e}")))
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
        mode: PackageMode,
    ) -> Result<Vec<InstallResult>> {
        let op = self.op(op_name)?;

        if !takes_a_package(op) {
            self.run(op_name, Values::new(), progress, String::new(), mode)
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
                    mode,
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
        mode: PackageMode,
    ) -> Result<Vec<Package>> {
        let values = Values::from([("query".into(), query.to_string())]);
        let records = self.query(OP_SEARCH, values, mode).await?;
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
        let records = self.query(OP_INFO, values, PackageMode::User).await?;
        let fields = &self.op(OP_INFO)?.fields;

        let record = records
            .first()
            .ok_or_else(|| AdapterError::PackageNotFound(package_id.to_string()))?;
        let package = self
            .to_package(record, fields)
            .ok_or_else(|| AdapterError::PackageNotFound(package_id.to_string()))?;

        // Whatever else the manifest asked to be shown, in the order it
        // asked. Nothing here knows what any of them mean.
        let extra = self
            .op(OP_INFO)?
            .extra
            .iter()
            .filter_map(|extra| Some((extra.label.clone(), output::value(record, &extra.field)?)))
            .collect();

        Ok(PackageDetail {
            package,
            pkg_type: output::text(record, fields, "pkg_type"),
            source: output::text(record, fields, "source"),
            build_date: output::text(record, fields, "build_date"),
            download_url: output::text(record, fields, "download_url"),
            extra,
        })
    }

    async fn install(
        &self,
        packages: &[Package],
        progress: Option<ProgressSender>,
        mode: PackageMode,
    ) -> Result<Vec<InstallResult>> {
        self.run_over(OP_INSTALL, packages, progress, mode).await
    }

    async fn remove(
        &self,
        packages: &[Package],
        progress: Option<ProgressSender>,
        mode: PackageMode,
    ) -> Result<()> {
        let results = self.run_over(OP_REMOVE, packages, progress, mode).await?;

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
        mode: PackageMode,
    ) -> Result<Vec<InstallResult>> {
        self.run_over(OP_UPDATE, packages, progress, mode).await
    }

    async fn list_installed(&self, mode: PackageMode) -> Result<Vec<InstalledPackage>> {
        let records = self.query(OP_LIST_INSTALLED, Values::new(), mode).await?;
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

    async fn list_updates(&self, mode: PackageMode) -> Result<Vec<Update>> {
        let records = self.query(OP_LIST_UPDATES, Values::new(), mode).await?;
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
        self.run(
            OP_SYNC,
            Values::new(),
            progress,
            String::new(),
            PackageMode::User,
        )
        .await?;
        Ok(())
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        let count = match self.query(OP_LIST, Values::new(), PackageMode::User).await {
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

    async fn list_repositories(&self) -> Result<Vec<Repository>> {
        let records = self
            .query(OP_LIST_REPOS, Values::new(), PackageMode::User)
            .await?;
        let fields = &self.op(OP_LIST_REPOS)?.fields;

        Ok(records
            .iter()
            .filter_map(|record| {
                Some(Repository {
                    name: output::text(record, fields, "name")?,
                    url: output::text(record, fields, "url").unwrap_or_default(),
                    enabled: output::flag(record, fields, "enabled").unwrap_or(true),
                    description: output::text(record, fields, "description"),
                })
            })
            .collect())
    }

    async fn add_repository(&self, repo: &Repository) -> Result<()> {
        let values = Values::from([
            ("name".into(), repo.name.clone()),
            ("url".into(), repo.url.clone()),
        ]);
        self.run(
            OP_ADD_REPO,
            values,
            None,
            repo.name.clone(),
            PackageMode::User,
        )
        .await?;

        Ok(())
    }

    async fn remove_repository(&self, repo_name: &str) -> Result<()> {
        let values = Values::from([("name".into(), repo_name.to_string())]);
        self.run(
            OP_REMOVE_REPO,
            values,
            None,
            repo_name.to_string(),
            PackageMode::User,
        )
        .await?;

        Ok(())
    }

    async fn set_repo_enabled(&self, name: &str, enabled: bool, mode: PackageMode) -> Result<()> {
        let values = Values::from([
            ("name".into(), name.to_string()),
            ("enabled".into(), enabled.to_string()),
        ]);
        self.run(OP_SET_REPO_ENABLED, values, None, name.to_string(), mode)
            .await?;

        Ok(())
    }

    async fn paths(&self) -> Result<HashMap<String, String>> {
        let records = self
            .query(OP_PATHS, Values::new(), PackageMode::User)
            .await?;
        let fields = &self.op(OP_PATHS)?.fields;
        let record = records
            .first()
            .ok_or_else(|| AdapterError::Other("the manager reported no paths".into()))?;

        Ok(fields
            .keys()
            .filter_map(|key| Some((key.clone(), output::text(record, fields, key)?)))
            .collect())
    }

    fn config_schema(&self) -> Option<ConfigSchema> {
        if self.manifest.config.is_empty() {
            return None;
        }

        Some(ConfigSchema {
            adapter_id: self.info.id.clone(),
            fields: self.manifest.config.iter().map(to_field).collect(),
        })
    }

    async fn get_config(&self) -> Result<AdapterConfig> {
        let document = self.read_config().await?;

        let values = self
            .manifest
            .config
            .iter()
            .filter_map(|setting| {
                let value = document
                    .get(&setting.key)
                    .or(setting.default.as_ref())
                    .and_then(to_value)?;

                Some((setting.key.clone(), value))
            })
            .collect();

        Ok(AdapterConfig { values })
    }

    /// What the editor starts with: the values actually written to the
    /// manager's configuration file. Declared defaults are deliberately not
    /// folded in here, so a field still at its default reads as unset and is
    /// shown (muted) from the schema rather than filled into the input.
    fn initial_config(&self) -> Option<AdapterConfig> {
        let path = self.file_paths().ok()?.get("config").cloned()?;
        let text = std::fs::read_to_string(&path).ok()?;
        let document: toml::Value = toml::from_str(&text).ok()?;

        let values = self
            .manifest
            .config
            .iter()
            .filter_map(|setting| {
                let value = document.get(&setting.key).and_then(to_value)?;
                Some((setting.key.clone(), value))
            })
            .collect();

        Some(AdapterConfig { values })
    }

    async fn set_config(&self, config: &AdapterConfig) -> Result<()> {
        use toml_edit::DocumentMut;

        let path = self.file("config").await?;

        // Only some of what a configuration file holds is described here, so
        // writing one from nothing would leave out whatever the manager needs
        // but never offered. Ask it for a whole one first.
        if !Path::new(&path).exists() && self.op(OP_DEFAULT_CONFIG).is_ok() {
            self.run(
                OP_DEFAULT_CONFIG,
                Values::new(),
                None,
                String::new(),
                PackageMode::User,
            )
            .await?;
        }

        let mut document = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| text.parse::<DocumentMut>().ok())
            .unwrap_or_default();

        // Only what the manifest declares, so a setting soar has and this
        // frontend does not know about is left as the user wrote it.
        for setting in &self.manifest.config {
            let Some(value) = config.values.get(&setting.key) else {
                continue;
            };
            write_setting(&mut document, &setting.key, value);
        }

        if let Some(parent) = Path::new(&path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| AdapterError::Other(e.to_string()))?;
        }

        std::fs::write(&path, document.to_string()).map_err(|e| AdapterError::Other(e.to_string()))
    }

    async fn list_profiles(&self) -> Result<Vec<Profile>> {
        let document = self.read_config().await?;

        let Some(profiles) = document.get("profile").and_then(toml::Value::as_table) else {
            return Ok(Vec::new());
        };

        let active = document
            .get("default_profile")
            .and_then(toml::Value::as_str)
            .unwrap_or("default");

        Ok(profiles
            .keys()
            .map(|name| Profile {
                id: name.clone(),
                name: name.clone(),
                is_active: name == active,
                package_count: 0,
            })
            .collect())
    }

    async fn declarative_diff(&self) -> Result<ManifestDiff> {
        let records = self
            .query(OP_APPLY_CHECK, Values::new(), PackageMode::User)
            .await?;
        let record = records
            .first()
            .ok_or_else(|| AdapterError::Other("the manager reported no diff".into()))?;

        Ok(ManifestDiff {
            to_install: changes(record, "to_install", false),
            to_update: changes(record, "to_update", false),
            to_remove: changes(record, "to_remove", true),
            in_sync: names(record, "in_sync"),
            not_found: names(record, "not_found"),
            invalid_profiles: HashMap::new(),
        })
    }

    async fn declarative_apply(
        &self,
        prune: bool,
        progress: Option<ProgressSender>,
    ) -> Result<ManifestApplyReport> {
        let op_name = if prune { OP_APPLY_PRUNE } else { OP_APPLY };
        let printed = self
            .run(
                op_name,
                Values::new(),
                progress,
                String::new(),
                PackageMode::User,
            )
            .await?
            .printed;

        let event_key = self
            .op(op_name)?
            .progress
            .get("event")
            .cloned()
            .unwrap_or_else(|| "type".to_string());

        Ok(apply_report(&printed, &event_key))
    }
}

fn to_field(setting: &Setting) -> ConfigField {
    ConfigField {
        key: setting.key.clone(),
        label: setting.label.clone(),
        description: setting.description.clone(),
        field_type: match setting.kind {
            SettingKind::Text => ConfigFieldType::Text,
            SettingKind::Toggle => ConfigFieldType::Toggle,
            SettingKind::Number => ConfigFieldType::Number,
            SettingKind::Select => ConfigFieldType::Select(setting.options.clone()),
            SettingKind::PathList => ConfigFieldType::PathList,
        },
        default: setting.default.as_ref().and_then(to_value),
        section: setting.section.clone(),
        aeris_managed: false,
    }
}

fn to_value(value: &toml::Value) -> Option<ConfigValue> {
    match value {
        toml::Value::String(s) => Some(ConfigValue::String(s.clone())),
        toml::Value::Boolean(b) => Some(ConfigValue::Bool(*b)),
        toml::Value::Integer(n) => Some(ConfigValue::Integer(*n)),
        toml::Value::Array(items) => Some(ConfigValue::StringList(
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect(),
        )),
        _ => None,
    }
}

fn write_setting(document: &mut toml_edit::DocumentMut, key: &str, value: &ConfigValue) {
    match value {
        ConfigValue::Bool(v) => document[key] = toml_edit::value(*v),
        ConfigValue::Integer(v) => document[key] = toml_edit::value(*v),
        ConfigValue::StringList(list) => {
            let mut array = toml_edit::Array::new();
            for item in list {
                array.push(item.as_str());
            }
            document[key] = toml_edit::value(array);
        }
        // A field cleared in the form means "unset", which is not the same as
        // an empty string: the manager should fall back to its own default.
        ConfigValue::String(s) if s.trim().is_empty() => {
            document.remove(key);
        }
        ConfigValue::String(s) => match s.trim().parse::<i64>() {
            Ok(n) => document[key] = toml_edit::value(n),
            Err(_) => document[key] = toml_edit::value(s.as_str()),
        },
    }
}

/// The name a stream uses for the event that says what an apply did.
const APPLY_COMPLETE: &str = "apply_complete";

/// Read the counts out of the event an apply ends with.
///
/// The stream is what says how it went, so a run that ends without saying
/// reports nothing rather than a guess.
fn apply_report(printed: &str, event_key: &str) -> ManifestApplyReport {
    let count =
        |record: &Value, key: &str| record.get(key).and_then(Value::as_u64).unwrap_or(0) as usize;

    printed
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|record| record.get(event_key).and_then(Value::as_str) == Some(APPLY_COMPLETE))
        .map(|record| ManifestApplyReport {
            installed: count(&record, "installed"),
            updated: count(&record, "updated"),
            removed: count(&record, "removed"),
            failed: count(&record, "failed"),
        })
        .next_back()
        .unwrap_or_default()
}

/// Read one side of a declarative diff.
fn changes(record: &Value, key: &str, removing: bool) -> Vec<ManifestEntry> {
    let text =
        |value: &Value, field: &str| value.get(field).and_then(Value::as_str).map(str::to_string);

    record
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let version = text(item, "version");
                    Some(ManifestEntry {
                        name: text(item, "name")?,
                        current_version: text(item, "current_version"),
                        // A package on its way out has no version to move to.
                        new_version: (!removing).then_some(version).flatten(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn names(record: &Value, key: &str) -> Vec<String> {
    record
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Says which command is missing, naming both where a manager ships one per
/// scope.
fn missing(manifest: &CommandManifest) -> String {
    let system = manifest
        .system
        .as_ref()
        .and_then(|config| config.command.as_deref());

    match system {
        Some(other) if other != manifest.detect.command => {
            format!(
                "neither {} nor {other} is installed",
                manifest.detect.command
            )
        }
        _ => format!("{} is not installed", manifest.detect.command),
    }
}

/// What a manifest says the manager can do, which is knowable without the
/// manager being installed.
pub fn capabilities_from(manifest: &CommandManifest) -> Capabilities {
    scoped_capabilities(manifest, true, true)
}

/// What the manager can do given which of its binaries are installed.
fn scoped_capabilities(
    manifest: &CommandManifest,
    user_available: bool,
    system_available: bool,
) -> Capabilities {
    let has = |name: &str| manifest.op(name).is_some();

    Capabilities {
        can_search: has(OP_SEARCH),
        can_install: has(OP_INSTALL),
        can_remove: has(OP_REMOVE),
        can_update: has(OP_UPDATE),
        can_list: has(OP_LIST) || has(OP_LIST_INSTALLED),
        can_list_updates: has(OP_LIST_UPDATES),
        can_sync: has(OP_SYNC),
        can_add_repo: has(OP_ADD_REPO),
        can_remove_repo: has(OP_REMOVE_REPO),
        can_list_repos: has(OP_LIST_REPOS),
        supports_declarative: has(OP_APPLY),
        has_package_detail: has(OP_INFO),
        has_size_info: manifest
            .ops
            .values()
            .any(|op| op.fields.contains_key("size")),
        // A scope is offered only when this manager works that way and the
        // binary it works with is here.
        supports_user_packages: !manifest.system_only && user_available,
        supports_system_packages: manifest.system.is_some() && system_available,
        ..Default::default()
    }
}

/// Read the name of a stage, which a manager may report as a word on its own
/// or as something carrying detail under that word.
fn stage_name(stage: &Value) -> Option<String> {
    match stage {
        Value::String(name) => Some(name.clone()),
        Value::Object(carrying) => carrying.keys().next().cloned(),
        _ => None,
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

/// What a run left behind: its answer, and whatever it said beside it.
struct Ran {
    printed: String,
    complained: String,
}

fn run(
    program: &Path,
    args: &[String],
    strip_ansi: bool,
    progress: Option<&Progress>,
    elevate: bool,
) -> Result<Ran> {
    let mut base = Command::new(program);
    base.args(args);

    // There is no terminal here, and `dumb` is the name for that. Without it
    // a manager asking a terminal what it can do gets no answer at all, and
    // elevation makes it worse: sudo hands on `unknown`, which is not a
    // terminal any more than nothing is.
    base.env("TERM", "dumb");

    // Asking through the desktop's own prompt is the only way a window can
    // ask for a password. Without it the manager would sit waiting on a
    // terminal that is not there.
    if elevate {
        base = crate::core::privilege::PrivilegeManager::new()
            .prepare_command(PackageMode::System, base)
            .map_err(|e| AdapterError::PermissionDenied(e.to_string()))?;
    }

    let mut child = base
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

    Ok(Ran {
        printed,
        complained: last_lines(&errors),
    })
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
            // Neither is whole until the run ends, so they say nothing while
            // the work is still going.
            Format::Json | Format::KeyValue => None,
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

        // The package aeris asked about, which is how it tracks the work.
        // A manager names packages its own way, so its name only stands in
        // where the operation covered no particular package.
        let package_id = Some(self.progress.package_id.clone())
            .filter(|id| !id.is_empty())
            .or_else(|| text("message"))
            .unwrap_or_default();

        match (count("current"), count("total")) {
            (Some(current), Some(total)) => Some(ProgressEvent::Download {
                adapter_id,
                package_id,
                current_bytes: current,
                total_bytes: total,
            }),
            _ => {
                // Without a fraction to report, the name of what is happening
                // is all there is to say, so an event carrying none is not
                // worth sending. The stage says more than the event does, when
                // the manager reports one.
                let phase = named("stage")
                    .and_then(stage_name)
                    .or_else(|| text("event"))?;
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

    let printed = match Command::new(program)
        .args(&manifest.detect.version)
        .stdin(Stdio::null())
        .output()
    {
        Ok(printed) => printed,
        Err(e) => {
            // Worth saying out loud: this reads the same as a manager that ran
            // and said nothing, and the two are fixed differently.
            log::warn!("could not ask {} for its version: {e}", program.display());
            return None;
        }
    };

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
            user_program: Some(PathBuf::from("demo")),
            system_program: None,
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
                package_id: "busybox/cat".into(),
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
                assert_eq!(package_id, "busybox/cat");
                assert_eq!((current_bytes, total_bytes), (50, 100));
            }
            other => panic!("reported {other:?}"),
        }
    }

    #[test]
    fn progress_is_reported_against_the_package_aeris_asked_about() {
        // The manager calls this package `cat`; aeris knows it as
        // `busybox/cat`, and that is what the rest of the app watches for.
        let (progress, mut receiver) = progress(Format::Ndjson);
        reporter_for(&progress)
            .report(r#"{"type":"installing","pkg_name":"cat","stage":"extracting"}"#);

        match receiver.try_recv().expect("should have reported") {
            ProgressEvent::Phase { package_id, .. } => assert_eq!(package_id, "busybox/cat"),
            other => panic!("reported {other:?}"),
        }
    }

    #[test]
    fn an_operation_naming_no_package_reports_the_one_the_manager_names() {
        let (mut progress, mut receiver) = progress(Format::Ndjson);
        // Updating everything at once names no package up front, so the only
        // name available is the one the manager reports as it goes.
        progress.package_id = String::new();

        reporter_for(&progress)
            .report(r#"{"type":"installing","pkg_name":"cat","stage":"extracting"}"#);

        match receiver.try_recv().expect("should have reported") {
            ProgressEvent::Phase { package_id, .. } => assert_eq!(package_id, "cat"),
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
    fn an_apply_is_reported_by_the_event_it_ends_with() {
        let printed = concat!(
            "{\"type\":\"installing\",\"pkg_name\":\"cat\"}\n",
            "{\"type\":\"apply_complete\",\"installed\":2,\"updated\":1,\"removed\":0,\"failed\":3}\n"
        );

        let report = apply_report(printed, "type");
        assert_eq!(report.installed, 2);
        assert_eq!(report.updated, 1);
        assert_eq!(report.failed, 3);
    }

    #[test]
    fn an_apply_that_never_says_how_it_went_reports_nothing() {
        let report = apply_report("{\"type\":\"installing\",\"pkg_name\":\"cat\"}\n", "type");
        assert_eq!(report.installed, 0);
        assert_eq!(report.failed, 0);
    }

    #[test]
    fn a_package_on_its_way_out_has_no_version_to_move_to() {
        let diff: Value = serde_json::from_str(
            r#"{"to_install":[{"name":"rg","version":"15.2.0","current_version":null}],
                "to_remove":[{"name":"eza","version":"0.23.3","current_version":"0.23.3"}],
                "not_found":["nope"]}"#,
        )
        .unwrap();

        let installing = changes(&diff, "to_install", false);
        assert_eq!(installing[0].new_version.as_deref(), Some("15.2.0"));
        assert_eq!(installing[0].current_version, None);

        let removing = changes(&diff, "to_remove", true);
        assert_eq!(removing[0].current_version.as_deref(), Some("0.23.3"));
        assert_eq!(removing[0].new_version, None);

        assert_eq!(names(&diff, "not_found"), ["nope"]);
        assert!(changes(&diff, "to_update", false).is_empty());
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

        // A directory of its own per run, so a stale file from an earlier run
        // and a file another test is still writing are both out of the way.
        let dir = std::env::temp_dir().join(format!("aeris-fake-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("should make a scratch directory");

        let path = dir.join("manager");
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
  repos)
    echo '{"items":[{"name":"main","url":"https://example.invalid/main","enabled":true}],"total":1}'
    ;;
  paths)
    printf '{"config":"%s.config.toml","packages_config":"%s.packages.toml"}\n' "$0" "$0"
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

        // Another test forking at the moment this file was being written
        // leaves its child holding a write handle to it, and exec refuses a
        // file anyone is writing. The child clears in moments, so wait for the
        // file to be runnable rather than let an unrelated test fail the run.
        for attempt in 0..10 {
            let ran = Command::new(&path)
                .arg("--version")
                .stdin(Stdio::null())
                .output();
            if ran.is_ok_and(|ran| ran.status.success()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20 * (attempt + 1)));
        }

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

[[config]]
key = "parallel"
label = "Parallel downloads"
type = "toggle"
default = true

[[config]]
key = "limit"
label = "Limit"
type = "number"
default = 4
section = "Advanced"

[ops.list_repos]
args = ["repos"]
output = {{ format = "json", select = "$.items[*]" }}
fields = {{ name = "name", url = "url", enabled = "enabled" }}

[ops.paths]
args = ["paths"]
output = {{ format = "json" }}
fields = {{ config = "config", packages_config = "packages_config" }}
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
    fn repositories_and_file_locations_are_read_from_the_manager() {
        let program = fake_manager("repos-and-paths");
        let manifest = manifest(&manifest_for(&program, "1.0.0"));
        let adapter = CommandAdapter::new(manifest, None).expect("should accept");

        assert!(adapter.capabilities().can_list_repos);
        assert!(!adapter.capabilities().supports_declarative);

        let repos = block_on(adapter.list_repositories()).expect("should list");
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].name, "main");
        assert!(repos[0].enabled);

        let paths = block_on(adapter.paths()).expect("should report paths");
        assert_eq!(
            paths.get("packages_config").map(String::as_str),
            Some(format!("{}.packages.toml", program.display()).as_str())
        );
    }

    #[test]
    fn settings_are_offered_as_the_manifest_declares_them() {
        let program = fake_manager("settings");
        let manifest = manifest(&manifest_for(&program, "1.0.0"));
        let adapter = CommandAdapter::new(manifest, None).expect("should accept");
        let _ = std::fs::remove_file(format!("{}.config.toml", program.display()));

        let schema = adapter.config_schema().expect("should describe settings");
        assert_eq!(schema.fields.len(), 2);
        assert!(matches!(
            schema.fields[0].field_type,
            ConfigFieldType::Toggle
        ));
        assert_eq!(schema.fields[1].section.as_deref(), Some("Advanced"));

        // Nothing written yet, so what is offered is what the manifest says.
        let config = block_on(adapter.get_config()).expect("should read");
        assert_eq!(config.values.get("limit"), Some(&ConfigValue::Integer(4)));

        let mut changed = config.clone();
        changed
            .values
            .insert("limit".into(), ConfigValue::Integer(9));
        block_on(adapter.set_config(&changed)).expect("should write");

        let after = block_on(adapter.get_config()).expect("should read back");
        assert_eq!(after.values.get("limit"), Some(&ConfigValue::Integer(9)));
    }

    #[test]
    fn initial_config_reads_what_is_written_without_the_defaults() {
        let program = fake_manager("initial-config");
        let manifest = manifest(&manifest_for(&program, "1.0.0"));
        let adapter = CommandAdapter::new(manifest, None).expect("should accept");
        let path = format!("{}.config.toml", program.display());

        // No file yet: nothing to start the editor with, even though the
        // manifest declares defaults, which are shown separately.
        let _ = std::fs::remove_file(&path);
        assert!(adapter.initial_config().is_none());

        // A value the manager wrote is read back; a key it did not write is
        // not filled in from the manifest's default.
        std::fs::write(&path, "limit = 9\n").unwrap();
        let config = adapter
            .initial_config()
            .expect("should read the written file");
        assert_eq!(config.values.get("limit"), Some(&ConfigValue::Integer(9)));
        assert!(
            !config.values.contains_key("parallel"),
            "the manifest default should not be folded in"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_setting_the_manifest_does_not_declare_is_left_as_it_was() {
        let program = fake_manager("untouched-settings");
        let manifest = manifest(&manifest_for(&program, "1.0.0"));
        let adapter = CommandAdapter::new(manifest, None).expect("should accept");

        let path = format!("{}.config.toml", program.display());
        std::fs::write(
            &path,
            "# hand written\nsomething_else = \"keep me\"\nlimit = 1\n",
        )
        .expect("should write");

        let mut config = AdapterConfig::default();
        config
            .values
            .insert("limit".into(), ConfigValue::Integer(7));
        block_on(adapter.set_config(&config)).expect("should write");

        let written = std::fs::read_to_string(&path).expect("should read");
        assert!(
            written.contains(r#"something_else = "keep me""#),
            "{written}"
        );
        assert!(written.contains("# hand written"), "{written}");
        assert!(written.contains("limit = 7"), "{written}");
    }

    #[test]
    fn a_manager_with_no_system_mode_refuses_to_act_in_one() {
        let program = fake_manager("user-only");
        let manifest = manifest(&manifest_for(&program, "1.0.0"));
        let adapter = CommandAdapter::new(manifest, None).expect("should accept");

        assert!(adapter.capabilities().supports_user_packages);
        assert!(!adapter.capabilities().supports_system_packages);
        assert!(matches!(
            adapter.invocation(PackageMode::System),
            Err(AdapterError::NotSupported)
        ));
    }

    #[test]
    fn acting_system_wide_can_mean_a_different_binary_and_more_arguments() {
        let program = fake_manager("two-scoped");
        let elsewhere = fake_manager("two-scoped-system");
        let manifest = manifest(&format!(
            "{}\n[system]\ncommand = \"{}\"\nargs = [\"--system\"]\nelevate = false\n",
            manifest_for(&program, "1.0.0"),
            elsewhere.display()
        ));
        let adapter = CommandAdapter::new(manifest, None).expect("should accept");

        assert!(adapter.capabilities().supports_system_packages);

        let (binary, before, elevate) = adapter
            .invocation(PackageMode::System)
            .expect("system mode");
        assert_eq!(binary, elsewhere);
        assert_eq!(before, ["--system"]);
        assert!(!elevate);

        // The user mode is untouched by any of that.
        let (binary, before, elevate) = adapter.invocation(PackageMode::User).expect("user mode");
        assert_eq!(binary, program);
        assert!(before.is_empty());
        assert!(!elevate);
    }

    #[test]
    fn a_manager_shipping_one_binary_per_scope_works_with_either_alone() {
        let user_only = fake_manager("user-half");
        let absent = user_only.parent().unwrap().join("not-installed-at-all");

        // Only the per-user half is here, so only that scope is offered.
        let user_half = manifest(&format!(
            "{}\n[system]\ncommand = \"{}\"\n",
            manifest_for(&user_only, "1.0.0"),
            absent.display()
        ));
        let adapter = CommandAdapter::new(user_half, None).expect("should still be usable");
        assert!(adapter.capabilities().supports_user_packages);
        assert!(!adapter.capabilities().supports_system_packages);
        assert!(adapter.invocation(PackageMode::User).is_ok());
        assert!(matches!(
            adapter.invocation(PackageMode::System),
            Err(AdapterError::NotSupported)
        ));

        // Now the other way round: only the system half exists.
        let system_only = fake_manager("system-half");
        let other_way = manifest(&format!(
            "{}\n[system]\ncommand = \"{}\"\n",
            manifest_for(&absent, "1.0.0"),
            system_only.display()
        ));
        let adapter = CommandAdapter::new(other_way, None).expect("should still be usable");
        assert!(!adapter.capabilities().supports_user_packages);
        assert!(adapter.capabilities().supports_system_packages);

        let (binary, _, _) = adapter.invocation(PackageMode::System).expect("system");
        assert_eq!(binary, system_only);
    }

    #[test]
    fn a_manager_with_neither_binary_is_refused_naming_both() {
        let here = std::env::temp_dir();
        let manifest = manifest(&format!(
            "{}\n[system]\ncommand = \"{}\"\n",
            manifest_for(&here.join("no-such-user-half"), "1.0.0"),
            here.join("no-such-system-half").display()
        ));

        let Err(e) = CommandAdapter::new(manifest, None) else {
            panic!("should refuse when nothing is installed");
        };
        assert!(e.to_string().contains("neither"), "{e}");
    }

    #[test]
    fn a_manager_that_only_acts_system_wide_says_so() {
        let program = fake_manager("system-only");
        // `system_only` is a top level key, so it has to come before any
        // table header or TOML reads it as part of that table.
        let manifest = manifest(&format!(
            "system_only = true\n{}\n[system]\nelevate = true\n",
            manifest_for(&program, "1.0.0")
        ));
        let adapter = CommandAdapter::new(manifest, None).expect("should accept");

        assert!(!adapter.capabilities().supports_user_packages);
        assert!(adapter.capabilities().supports_system_packages);

        // Even asked for the user mode, there is only the one way it works.
        let (_, _, elevate) = adapter.invocation(PackageMode::User).expect("only mode");
        assert!(elevate);
    }

    #[test]
    fn an_operation_a_manifest_does_not_declare_is_not_supported() {
        let program = fake_manager("undeclared");
        let manifest = manifest(&manifest_for(&program, "1.0.0"));
        let adapter = CommandAdapter::new(manifest, None).expect("should accept");

        assert!(matches!(
            block_on(adapter.declarative_diff()),
            Err(AdapterError::NotSupported)
        ));
        assert!(matches!(
            block_on(adapter.list_updates(PackageMode::User)),
            Err(AdapterError::NotSupported)
        ));
    }

    #[test]
    fn a_question_that_goes_unanswered_says_what_the_manager_complained_about() {
        let program = fake_manager("silent");
        let manifest = manifest(&manifest_for(&program, "1.0.0"));
        let adapter = CommandAdapter::new(manifest, None).expect("should accept");

        // `quiet` is not a case the fake handles, so it says nothing at all.
        let op = adapter.manifest.op(OP_SEARCH).unwrap().clone();
        let mut manifest = (*adapter.manifest).clone();
        manifest.ops.insert(
            OP_SEARCH.to_string(),
            Op {
                args: vec!["quiet".into()],
                ..op
            },
        );
        let adapter = CommandAdapter::new(manifest, None).expect("should accept");

        let Err(err) = block_on(adapter.search("anything", None, PackageMode::User)) else {
            panic!("should refuse an empty answer");
        };
        assert!(err.to_string().contains("answered nothing"), "{err}");
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
