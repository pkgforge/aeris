// WASM plugin export names (plugin → host)
pub const EXPORT_INIT: &str = "adapter_init";
pub const EXPORT_INFO: &str = "adapter_info";
pub const EXPORT_CAPABILITIES: &str = "adapter_capabilities";
pub const EXPORT_SEARCH: &str = "adapter_search";
pub const EXPORT_INSTALL: &str = "adapter_install";
pub const EXPORT_REMOVE: &str = "adapter_remove";
pub const EXPORT_UPDATE: &str = "adapter_update";
pub const EXPORT_LIST_INSTALLED: &str = "adapter_list_installed";
pub const EXPORT_LIST_UPDATES: &str = "adapter_list_updates";
pub const EXPORT_SYNC: &str = "adapter_sync";
pub const EXPORT_LIST_REPOS: &str = "adapter_list_repos";
pub const EXPORT_GET_CONFIG: &str = "adapter_get_config";
pub const EXPORT_SET_CONFIG: &str = "adapter_set_config";
pub const EXPORT_HEALTH_CHECK: &str = "adapter_health_check";
pub const EXPORT_ALLOCATE: &str = "allocate";
pub const EXPORT_DEALLOCATE: &str = "deallocate";
pub const EXPORT_MEMORY: &str = "memory";

// Host import function names (host → plugin, under "env" namespace)
pub const IMPORT_NAMESPACE: &str = "env";
pub const IMPORT_LOG: &str = "host_log";
pub const IMPORT_EXEC: &str = "host_exec";
pub const IMPORT_FS_READ: &str = "host_fs_read";
pub const IMPORT_FS_WRITE: &str = "host_fs_write";
pub const IMPORT_FS_EXISTS: &str = "host_fs_exists";
pub const IMPORT_HTTP_GET: &str = "host_http_get";
pub const IMPORT_REPORT_PROGRESS: &str = "host_report_progress";

// Log level constants matching plugin-side definitions
pub const LOG_ERROR: i32 = 1;
pub const LOG_WARN: i32 = 2;
pub const LOG_INFO: i32 = 3;
pub const LOG_DEBUG: i32 = 4;
