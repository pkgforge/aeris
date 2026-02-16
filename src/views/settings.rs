use std::collections::HashMap;

use iced::{
    Element, Length,
    widget::{
        button, column, container, pick_list, row, rule, scrollable, text, text_input, toggler,
    },
};

use crate::{
    app::{
        AppTheme, View,
        message::{Message, SettingsMessage},
    },
    config::AerisConfig,
    core::{
        adapter::Adapter,
        config::{AdapterConfig, ConfigFieldType, ConfigSchema, ConfigValue},
    },
};

#[derive(Debug)]
pub struct SettingsState {
    pub selected_theme: AppTheme,
    pub startup_view: View,
    pub notifications: bool,
    pub aeris_dirty: bool,
    pub aeris_save_error: Option<String>,
    pub aeris_save_success: bool,

    pub adapter_schema: Option<ConfigSchema>,
    pub adapter_config: AdapterConfig,
    pub adapter_config_original: AdapterConfig,
    pub adapter_settings: HashMap<String, String>,
    pub adapter_dirty: bool,
    pub adapter_save_error: Option<String>,
    pub adapter_save_success: bool,
    pub saving: bool,
}

impl SettingsState {
    pub fn load(aeris_config: &AerisConfig, adapter: &dyn Adapter) -> Self {
        let schema = adapter.config_schema();
        let mut adapter_settings = HashMap::new();

        if let Some(ref schema) = schema {
            for field in &schema.fields {
                if field.aeris_managed {
                    if let Some(value) =
                        aeris_config.get_adapter_setting(&schema.adapter_id, &field.key)
                    {
                        adapter_settings.insert(field.key.clone(), value.to_string());
                    }
                }
            }
        }

        Self {
            selected_theme: aeris_config.theme(),
            startup_view: aeris_config.startup_view(),
            notifications: aeris_config.notifications.unwrap_or(true),
            aeris_dirty: false,
            aeris_save_error: None,
            aeris_save_success: false,

            adapter_schema: schema,
            adapter_config: adapter.initial_config().unwrap_or_default(),
            adapter_config_original: adapter.initial_config().unwrap_or_default(),
            adapter_settings,
            adapter_dirty: false,
            adapter_save_error: None,
            adapter_save_success: false,
            saving: false,
        }
    }
}

const VIEW_OPTIONS: [View; 4] = [
    View::Dashboard,
    View::Browse,
    View::Installed,
    View::Updates,
];

fn render_config_field<'a>(
    key: &str,
    label: &'a str,
    field_type: &ConfigFieldType,
    value: Option<&ConfigValue>,
    default: Option<&ConfigValue>,
    original: Option<&ConfigValue>,
    aeris_managed: bool,
) -> Element<'a, Message> {
    let key_owned = key.to_string();

    match field_type {
        ConfigFieldType::Toggle => {
            let checked = match value {
                Some(ConfigValue::Bool(v)) => *v,
                _ => match default {
                    Some(ConfigValue::Bool(v)) => *v,
                    _ => false,
                },
            };
            row![
                text(label).size(14).width(Length::Fill),
                toggler(checked).on_toggle(move |v| {
                    Message::Settings(SettingsMessage::AdapterFieldChanged(
                        key_owned.clone(),
                        ConfigValue::Bool(v),
                    ))
                }),
            ]
            .align_y(iced::Alignment::Center)
            .into()
        }
        ConfigFieldType::Text => {
            let current = match value {
                Some(ConfigValue::String(s)) => s.clone(),
                _ => String::new(),
            };
            row![
                text(label).size(14).width(Length::Fill),
                text_input("default", &current)
                    .on_input(move |s| {
                        Message::Settings(SettingsMessage::AdapterFieldChanged(
                            key_owned.clone(),
                            ConfigValue::String(s),
                        ))
                    })
                    .width(240),
            ]
            .align_y(iced::Alignment::Center)
            .into()
        }
        ConfigFieldType::PathList => {
            let current = match value {
                Some(ConfigValue::String(s)) => s.clone(),
                _ => String::new(),
            };
            let original_str = match original {
                Some(ConfigValue::String(s)) => s.as_str(),
                _ => "",
            };
            let changed = current != original_str;
            let browse_key = key_owned.clone();
            let revert_key = key_owned.clone();
            let mut r = row![
                text(label).size(14).width(Length::Fill),
                text_input("default", &current)
                    .on_input(move |s| {
                        Message::Settings(SettingsMessage::AdapterFieldChanged(
                            key_owned.clone(),
                            ConfigValue::String(s),
                        ))
                    })
                    .width(200),
                button(text("Browse").size(13))
                    .on_press(Message::Settings(SettingsMessage::BrowseAdapterField(
                        browse_key,
                    )))
                    .padding([4, 10]),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center);
            if changed {
                r = r.push(
                    button(text("Revert").size(13))
                        .on_press(Message::Settings(SettingsMessage::RevertAdapterField(
                            revert_key,
                        )))
                        .style(button::secondary)
                        .padding([4, 10]),
                );
            }
            r.into()
        }
        ConfigFieldType::ExecutablePath => {
            let current = match value {
                Some(ConfigValue::String(s)) => s.clone(),
                _ => String::new(),
            };
            let browse_key = key_owned.clone();
            let revert_key = key_owned.clone();
            let mut r = row![
                text(label).size(14).width(Length::Fill),
                text_input("auto-detect", &current)
                    .on_input(move |s| {
                        Message::Settings(SettingsMessage::AdapterAerisFieldChanged(
                            key_owned.clone(),
                            s,
                        ))
                    })
                    .width(200),
                button(text("Browse").size(13))
                    .on_press(Message::Settings(SettingsMessage::BrowseExecutableField(
                        browse_key,
                    )))
                    .padding([4, 10]),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center);
            if !current.is_empty() {
                r = r.push(
                    button(text("Clear").size(13))
                        .on_press(Message::Settings(SettingsMessage::RevertAdapterAerisField(
                            revert_key,
                        )))
                        .style(button::secondary)
                        .padding([4, 10]),
                );
            }
            r.into()
        }
        ConfigFieldType::Number => {
            let current = match value {
                Some(ConfigValue::String(s)) => s.clone(),
                Some(ConfigValue::Integer(n)) => n.to_string(),
                _ => String::new(),
            };
            let placeholder = match default {
                Some(ConfigValue::Integer(n)) => n.to_string(),
                _ => String::new(),
            };
            row![
                text(label).size(14).width(Length::Fill),
                text_input(&placeholder, &current)
                    .on_input(move |s| {
                        Message::Settings(SettingsMessage::AdapterFieldChanged(
                            key_owned.clone(),
                            ConfigValue::String(s),
                        ))
                    })
                    .width(80),
            ]
            .align_y(iced::Alignment::Center)
            .into()
        }
        ConfigFieldType::Select(options) => {
            let current = match value {
                Some(ConfigValue::String(s)) => s.clone(),
                _ => match default {
                    Some(ConfigValue::String(s)) => s.clone(),
                    _ => String::new(),
                },
            };
            if options.is_empty() {
                row![
                    text(label).size(14).width(Length::Fill),
                    text(current).size(14),
                ]
                .align_y(iced::Alignment::Center)
                .into()
            } else {
                row![
                    text(label).size(14).width(Length::Fill),
                    pick_list(options.clone(), Some(current), move |v| {
                        Message::Settings(SettingsMessage::AdapterFieldChanged(
                            key_owned.clone(),
                            ConfigValue::String(v),
                        ))
                    },)
                    .width(160),
                ]
                .align_y(iced::Alignment::Center)
                .into()
            }
        }
    }
}

pub fn view(state: &SettingsState) -> Element<'_, Message> {
    let header = text("Settings").size(22);

    let appearance_header = text("Appearance").size(16);
    let theme_row = row![
        text("Theme").size(14).width(Length::Fill),
        pick_list(&AppTheme::ALL[..], Some(state.selected_theme), |t| {
            Message::Settings(SettingsMessage::ThemeChanged(t))
        },)
        .width(160),
    ]
    .align_y(iced::Alignment::Center);

    let general_header = text("General").size(16);
    let startup_row = row![
        text("Startup view").size(14).width(Length::Fill),
        pick_list(&VIEW_OPTIONS[..], Some(state.startup_view), |v| {
            Message::Settings(SettingsMessage::StartupViewChanged(v))
        })
        .width(160),
    ]
    .align_y(iced::Alignment::Center);

    let notifications_row = row![
        text("Notifications").size(14).width(Length::Fill),
        toggler(state.notifications)
            .on_toggle(|v| { Message::Settings(SettingsMessage::NotificationsToggled(v)) }),
    ]
    .align_y(iced::Alignment::Center);

    let mut aeris_save_btn = button(text("Save Aeris Settings").size(13))
        .padding([6, 14])
        .style(button::primary);
    if state.aeris_dirty && !state.saving {
        aeris_save_btn = aeris_save_btn.on_press(Message::Settings(SettingsMessage::SaveAeris));
    }

    let mut aeris_section = column![
        appearance_header,
        theme_row,
        general_header,
        startup_row,
        notifications_row,
        aeris_save_btn,
    ]
    .spacing(10);

    if let Some(ref err) = state.aeris_save_error {
        aeris_section = aeris_section.push(text(format!("Error: {err}")).size(12));
    }
    if state.aeris_save_success {
        aeris_section = aeris_section.push(text("Saved").size(12));
    }

    // Adapter section - dynamic rendering from schema
    let mut adapter_section = column![].spacing(10);

    if let Some(ref schema) = state.adapter_schema {
        let mut capitalized = schema.adapter_id.clone();
        if let Some(first) = capitalized.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        adapter_section = adapter_section.push(text(format!("Adapter: {capitalized}")).size(16));

        let mut last_section: Option<Option<&String>> = None;

        for field in &schema.fields {
            let current_section = field.section.as_ref();

            if last_section.is_none() || last_section.unwrap() != current_section {
                if last_section.is_some() {
                    adapter_section = adapter_section.push(rule::horizontal(1));
                }
                if let Some(section_name) = current_section {
                    adapter_section = adapter_section.push(text(section_name.as_str()).size(14));
                }
                last_section = Some(current_section);
            }

            let value = if field.aeris_managed {
                state
                    .adapter_settings
                    .get(&field.key)
                    .map(|s| ConfigValue::String(s.clone()))
            } else {
                state.adapter_config.values.get(&field.key).cloned()
            };
            let original = if field.aeris_managed {
                None
            } else {
                state.adapter_config_original.values.get(&field.key)
            };
            adapter_section = adapter_section.push(render_config_field(
                &field.key,
                &field.label,
                &field.field_type,
                value.as_ref(),
                field.default.as_ref(),
                original,
                field.aeris_managed,
            ));
        }

        let mut adapter_save_btn = button(text("Save Adapter Settings").size(13))
            .padding([6, 14])
            .style(button::primary);
        if state.adapter_dirty && !state.saving {
            adapter_save_btn =
                adapter_save_btn.on_press(Message::Settings(SettingsMessage::SaveAdapter));
        }
        adapter_section = adapter_section.push(adapter_save_btn);

        if let Some(ref err) = state.adapter_save_error {
            adapter_section = adapter_section.push(text(format!("Error: {err}")).size(12));
        }
        if state.adapter_save_success {
            adapter_section = adapter_section.push(text("Saved").size(12));
        }
    }

    let content = column![header, aeris_section, rule::horizontal(1), adapter_section,]
        .spacing(16)
        .width(Length::Fill);

    container(scrollable(content).height(Length::Fill))
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
