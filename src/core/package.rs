use serde::{Deserialize, Serialize};

use super::adapter::AdapterId;

pub type PackageId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub id: PackageId,
    pub name: String,
    pub version: String,
    pub adapter_id: AdapterId,
    pub description: Option<String>,
    pub size: Option<u64>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub installed: bool,
    pub update_available: bool,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub icon_url: Option<String>,
}

/// What a manager knows about one package beyond what a listing carries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDetail {
    pub package: Package,
    /// What the package is built as, in the manager's own words.
    pub pkg_type: Option<String>,
    /// Where the package is built from.
    pub source: Option<String>,
    pub build_date: Option<String>,
    pub download_url: Option<String>,
    /// Whatever else the manager reports, labelled as it asked. Aeris shows
    /// these without knowing what any of them mean.
    #[serde(default)]
    pub extra: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub package: Package,
    pub installed_at: String,
    pub install_size: u64,
    pub install_path: Option<String>,
    pub pinned: bool,
    pub auto_installed: bool,
    pub is_healthy: bool,
    pub profile: Option<String>,
}

impl InstalledPackage {
    /// Unique key for selection/tracking, distinguishing different installs of the same package.
    pub fn unique_key(&self) -> String {
        format!(
            "{}@{}",
            super::adapter::package_key(&self.package.adapter_id, &self.package.id),
            self.package.version
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Update {
    pub package: Package,
    pub current_version: String,
    pub new_version: String,
    pub download_size: Option<u64>,
    pub is_security: bool,
    pub changelog_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    pub package_name: String,
    pub package_id: String,
    pub version: String,
    pub success: bool,
    pub error: Option<String>,
}

/// What went wrong across results that report a package at a time, or nothing
/// when every one of them went through.
///
/// An operation can answer without complaining and still have failed for each
/// package it was handed, so this is what decides whether it worked. Reading
/// the outer result alone would call a failed install a success.
pub fn failure_among(results: &[InstallResult]) -> Option<String> {
    let failed: Vec<&InstallResult> = results.iter().filter(|r| !r.success).collect();
    let first = failed.first()?;
    let reason = first
        .error
        .clone()
        .unwrap_or_else(|| "it gave no reason".to_string());

    // Naming the one package would only repeat whatever asked for it, which
    // has already said which package this is about.
    Some(if failed.len() == 1 {
        reason
    } else {
        format!(
            "{} of {} failed. {}: {reason}",
            failed.len(),
            results.len(),
            first.package_name
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{InstallResult, failure_among};

    fn result(name: &str, success: bool, error: Option<&str>) -> InstallResult {
        InstallResult {
            package_name: name.to_string(),
            package_id: name.to_string(),
            version: "1.0".into(),
            success,
            error: error.map(ToString::to_string),
        }
    }

    #[test]
    fn an_operation_that_answered_can_still_have_failed() {
        assert_eq!(failure_among(&[]), None);
        assert_eq!(failure_among(&[result("fd", true, None)]), None);

        // The whole point: the call returned without complaining, and the
        // package still did not go in.
        assert_eq!(
            failure_among(&[result("firefox-bin", false, Some("bwrap: not permitted"))]).as_deref(),
            Some("bwrap: not permitted")
        );

        assert_eq!(
            failure_among(&[result("fd", false, None)]).as_deref(),
            Some("it gave no reason")
        );

        assert_eq!(
            failure_among(&[
                result("fd", true, None),
                result("firefox-bin", false, Some("no")),
                result("jq", false, Some("also no")),
            ])
            .as_deref(),
            Some("2 of 3 failed. firefox-bin: no")
        );
    }
}
