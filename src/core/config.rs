use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigValue {
    String(String),
    Bool(bool),
    Integer(i64),
    StringList(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigField {
    pub key: String,
    pub label: String,
    pub description: Option<String>,
    pub field_type: ConfigFieldType,
    pub default: Option<ConfigValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigFieldType {
    Text,
    Toggle,
    Number,
    Select(Vec<String>),
    PathList,
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
