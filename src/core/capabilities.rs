use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Capabilities {
    pub can_search: bool,
    pub can_install: bool,
    pub can_remove: bool,
    pub can_update: bool,
    pub can_list: bool,
    pub can_list_updates: bool,
    pub can_sync: bool,
    pub can_run: bool,

    pub can_add_repo: bool,
    pub can_remove_repo: bool,
    pub can_list_repos: bool,

    pub has_profiles: bool,

    pub has_size_info: bool,
    pub has_package_detail: bool,

    pub supports_declarative: bool,

    pub supports_user_packages: bool,
    pub supports_system_packages: bool,
}
