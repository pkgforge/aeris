use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConfigValue {
    String(String),
    Bool(bool),
    Integer(i64),
    StringList(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigField {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    pub field_type: ConfigFieldType,
    #[serde(default)]
    pub default: Option<ConfigValue>,
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub aeris_managed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ConfigFieldType {
    #[default]
    Text,
    Toggle,
    Number,
    Select(Vec<String>),
    PathList,
    ExecutablePath,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSchema {
    pub adapter_id: String,
    pub fields: Vec<ConfigField>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdapterConfig {
    pub values: HashMap<String, ConfigValue>,
}
