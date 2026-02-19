use std::collections::HashMap;

use iced::{
    Alignment, Element, Length,
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
        privilege::PackageMode,
    },
    styles::{self, font_size, spacing},
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
    _aeris_managed: bool,
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
                text(label).size(font_size::BODY).width(Length::Fill),
                toggler(checked).on_toggle(move |v| {
                    Message::Settings(SettingsMessage::AdapterFieldChanged(
                        key_owned.clone(),
                        ConfigValue::Bool(v),
                    ))
                }),
            ]
            .align_y(Alignment::Center)
            .into()
        }
        ConfigFieldType::Text => {
            let current = match value {
                Some(ConfigValue::String(s)) => s.clone(),
                _ => String::new(),
            };
            row![
                text(label).size(font_size::BODY).width(Length::Fill),
                text_input("default", &current)
                    .on_input(move |s| {
                        Message::Settings(SettingsMessage::AdapterFieldChanged(
                            key_owned.clone(),
                            ConfigValue::String(s),
                        ))
                    })
                    .width(240),
            ]
            .align_y(Alignment::Center)
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
                text(label).size(font_size::BODY).width(Length::Fill),
                text_input("default", &current)
                    .on_input(move |s| {
                        Message::Settings(SettingsMessage::AdapterFieldChanged(
                            key_owned.clone(),
                            ConfigValue::String(s),
                        ))
                    })
                    .width(200),
                button(text("Browse").size(font_size::SMALL))
                    .on_press(Message::Settings(SettingsMessage::BrowseAdapterField(
                        browse_key,
                    )))
                    .padding([spacing::XXS, 10.0]),
            ]
            .spacing(spacing::XXS)
            .align_y(Alignment::Center);
            if changed {
                r = r.push(
                    button(text("Revert").size(font_size::SMALL))
                        .on_press(Message::Settings(SettingsMessage::RevertAdapterField(
                            revert_key,
                        )))
                        .style(button::secondary)
                        .padding([spacing::XXS, 10.0]),
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
                text(label).size(font_size::BODY).width(Length::Fill),
                text_input("auto-detect", &current)
                    .on_input(move |s| {
                        Message::Settings(SettingsMessage::AdapterAerisFieldChanged(
                            key_owned.clone(),
                            s,
                        ))
                    })
                    .width(200),
                button(text("Browse").size(font_size::SMALL))
                    .on_press(Message::Settings(SettingsMessage::BrowseExecutableField(
                        browse_key,
                    )))
                    .padding([spacing::XXS, 10.0]),
            ]
            .spacing(spacing::XXS)
            .align_y(Alignment::Center);
            if !current.is_empty() {
                r = r.push(
                    button(text("Clear").size(font_size::SMALL))
                        .on_press(Message::Settings(SettingsMessage::RevertAdapterAerisField(
                            revert_key,
                        )))
                        .style(button::secondary)
                        .padding([spacing::XXS, 10.0]),
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
                text(label).size(font_size::BODY).width(Length::Fill),
                text_input(&placeholder, &current)
                    .on_input(move |s| {
                        Message::Settings(SettingsMessage::AdapterFieldChanged(
                            key_owned.clone(),
                            ConfigValue::String(s),
                        ))
                    })
                    .width(80),
            ]
            .align_y(Alignment::Center)
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
                    text(label).size(font_size::BODY).width(Length::Fill),
                    text(current).size(font_size::BODY),
                ]
                .align_y(Alignment::Center)
                .into()
            } else {
                row![
                    text(label).size(font_size::BODY).width(Length::Fill),
                    pick_list(options.clone(), Some(current), move |v| {
                        Message::Settings(SettingsMessage::AdapterFieldChanged(
                            key_owned.clone(),
                            ConfigValue::String(v),
                        ))
                    },)
                    .width(160),
                ]
                .align_y(Alignment::Center)
                .into()
            }
        }
    }
}

pub fn view<'a>(state: &'a SettingsState, mode: PackageMode) -> Element<'a, Message> {
    let header = text("Settings").size(font_size::TITLE);

    let appearance_section = container(
        column![
            text("Appearance").size(font_size::HEADING),
            row![
                text("Theme").size(font_size::BODY).width(Length::Fill),
                pick_list(&AppTheme::ALL[..], Some(state.selected_theme), |t| {
                    Message::Settings(SettingsMessage::ThemeChanged(t))
                },)
                .width(160),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(10)
        .padding(spacing::LG),
    )
    .width(Length::Fill)
    .style(styles::settings_card);

    let general_section = container(
        column![
            text("General").size(font_size::HEADING),
            row![
                text("Startup view")
                    .size(font_size::BODY)
                    .width(Length::Fill),
                pick_list(&VIEW_OPTIONS[..], Some(state.startup_view), |v| {
                    Message::Settings(SettingsMessage::StartupViewChanged(v))
                })
                .width(160),
            ]
            .align_y(Alignment::Center),
            row![
                text("Notifications")
                    .size(font_size::BODY)
                    .width(Length::Fill),
                toggler(state.notifications)
                    .on_toggle(|v| { Message::Settings(SettingsMessage::NotificationsToggled(v)) }),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(10)
        .padding(spacing::LG),
    )
    .width(Length::Fill)
    .style(styles::settings_card);

    let mut aeris_save_btn = button(text("Save Aeris Settings").size(font_size::SMALL))
        .padding([spacing::XS, 14.0])
        .style(button::primary);
    if state.aeris_dirty && !state.saving {
        aeris_save_btn = aeris_save_btn.on_press(Message::Settings(SettingsMessage::SaveAeris));
    }

    let mut aeris_section =
        column![appearance_section, general_section, aeris_save_btn].spacing(spacing::MD);

    if let Some(ref err) = state.aeris_save_error {
        aeris_section = aeris_section.push(
            container(text(format!("Error: {err}")).size(font_size::CAPTION + 1.0))
                .padding([spacing::XS, spacing::MD])
                .style(styles::error_banner),
        );
    }
    if state.aeris_save_success {
        aeris_section = aeris_section.push(
            container(text("Saved").size(font_size::CAPTION + 1.0))
                .padding([spacing::XXS, 10.0])
                .style(styles::badge_success),
        );
    }

    let mut adapter_section = column![].spacing(10);

    if let Some(ref schema) = state.adapter_schema {
        let mut capitalized = schema.adapter_id.clone();
        if let Some(first) = capitalized.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        let mode_label = match mode {
            PackageMode::User => "User",
            PackageMode::System => "System",
        };
        adapter_section = adapter_section
            .push(text(format!("Adapter: {capitalized} ({mode_label})")).size(font_size::HEADING));

        let mut last_section: Option<Option<&String>> = None;
        let mut current_group: Vec<Element<'a, Message>> = Vec::new();

        for field in &schema.fields {
            let current_section = field.section.as_ref();

            if last_section.is_none() || last_section.unwrap() != current_section {
                if !current_group.is_empty() {
                    adapter_section = adapter_section.push(
                        container(
                            column(std::mem::take(&mut current_group))
                                .spacing(10)
                                .padding(spacing::LG),
                        )
                        .width(Length::Fill)
                        .style(styles::settings_card),
                    );
                }
                if let Some(section_name) = current_section {
                    current_group.push(text(section_name.as_str()).size(font_size::BODY).into());
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
            current_group.push(render_config_field(
                &field.key,
                &field.label,
                &field.field_type,
                value.as_ref(),
                field.default.as_ref(),
                original,
                field.aeris_managed,
            ));
        }

        if !current_group.is_empty() {
            adapter_section = adapter_section.push(
                container(column(current_group).spacing(10).padding(spacing::LG))
                    .width(Length::Fill)
                    .style(styles::settings_card),
            );
        }

        let mut adapter_save_btn = button(text("Save Adapter Settings").size(font_size::SMALL))
            .padding([spacing::XS, 14.0])
            .style(button::primary);
        if state.adapter_dirty && !state.saving {
            adapter_save_btn =
                adapter_save_btn.on_press(Message::Settings(SettingsMessage::SaveAdapter));
        }
        adapter_section = adapter_section.push(adapter_save_btn);

        if let Some(ref err) = state.adapter_save_error {
            adapter_section = adapter_section.push(
                container(text(format!("Error: {err}")).size(font_size::CAPTION + 1.0))
                    .padding([spacing::XS, spacing::MD])
                    .style(styles::error_banner),
            );
        }
        if state.adapter_save_success {
            adapter_section = adapter_section.push(
                container(text("Saved").size(font_size::CAPTION + 1.0))
                    .padding([spacing::XXS, 10.0])
                    .style(styles::badge_success),
            );
        }
    }

    let content = column![header, aeris_section, rule::horizontal(1), adapter_section]
        .spacing(spacing::LG)
        .width(Length::Fill);

    container(scrollable(content).height(Length::Fill))
        .padding(spacing::XL)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
