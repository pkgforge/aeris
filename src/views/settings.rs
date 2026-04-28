use std::collections::HashMap;

use gpui::*;

use crate::{
    app::{App, AppTheme, View},
    config::AerisConfig,
    core::{
        adapter::Adapter,
        config::{AdapterConfig, ConfigFieldType, ConfigSchema, ConfigValue},
        privilege::PackageMode,
    },
    styles, theme,
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

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            selected_theme: AppTheme::System,
            startup_view: View::Dashboard,
            notifications: true,
            aeris_dirty: false,
            aeris_save_error: None,
            aeris_save_success: false,
            adapter_schema: None,
            adapter_config: AdapterConfig::default(),
            adapter_config_original: AdapterConfig::default(),
            adapter_settings: HashMap::new(),
            adapter_dirty: false,
            adapter_save_error: None,
            adapter_save_success: false,
            saving: false,
        }
    }
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

impl App {
    pub fn render_settings(
        &mut self,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let text_muted = theme.text_muted;
        let primary = theme.primary;
        let hover = theme.hover;
        let danger = theme.danger;
        let success = theme.success;

        let mode = self.current_mode;

        let header = div()
            .text_size(px(styles::font_size::TITLE))
            .child("Settings");

        // Appearance section
        let appearance_section = div()
            .p(px(styles::spacing::LG))
            .rounded(px(styles::radius::LG))
            .bg(surface)
            .border_1()
            .border_color(border)
            .w_full()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(
                div()
                    .text_size(px(styles::font_size::HEADING))
                    .child("Appearance"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(styles::font_size::BODY))
                            .flex_1()
                            .child("Theme"),
                    )
                    .child(self.render_theme_selector(theme, cx)),
            );

        // General section
        let general_section = div()
            .p(px(styles::spacing::LG))
            .rounded(px(styles::radius::LG))
            .bg(surface)
            .border_1()
            .border_color(border)
            .w_full()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(
                div()
                    .text_size(px(styles::font_size::HEADING))
                    .child("General"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(styles::font_size::BODY))
                            .flex_1()
                            .child("Startup view"),
                    )
                    .child(
                        div()
                            .text_size(px(styles::font_size::BODY))
                            .text_color(text_muted)
                            .child(format!("{}", self.settings_state.startup_view)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(styles::font_size::BODY))
                            .flex_1()
                            .child("Notifications"),
                    )
                    .child(self.render_toggle(
                        "notifications-toggle",
                        self.settings_state.notifications,
                        theme,
                        cx,
                    )),
            );

        // Save button
        let save_enabled = self.settings_state.aeris_dirty && !self.settings_state.saving;
        let save_aeris = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.save_aeris_settings(cx);
        });

        let save_btn_bg = if save_enabled { primary } else { surface };
        let save_btn_text = if save_enabled {
            gpui::white()
        } else {
            text_muted
        };

        let mut aeris_section = div()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::MD))
            .child(appearance_section)
            .child(general_section)
            .child(
                div()
                    .id("save-aeris-btn")
                    .px(px(14.0))
                    .py(px(styles::spacing::XS))
                    .rounded(px(styles::radius::MD))
                    .bg(save_btn_bg)
                    .text_color(save_btn_text)
                    .cursor_pointer()
                    .text_size(px(styles::font_size::SMALL))
                    .on_click(save_aeris)
                    .child("Save Aeris Settings"),
            );

        if let Some(ref err) = self.settings_state.aeris_save_error {
            aeris_section = aeris_section.child(
                div()
                    .px(px(styles::spacing::MD))
                    .py(px(styles::spacing::XS))
                    .rounded(px(styles::radius::MD))
                    .bg(danger.opacity(0.15))
                    .border_1()
                    .border_color(danger.opacity(0.3))
                    .text_size(px(styles::font_size::SMALL))
                    .child(format!("Error: {err}")),
            );
        }

        if self.settings_state.aeris_save_success {
            aeris_section = aeris_section.child(
                div()
                    .px(px(10.0))
                    .py(px(styles::spacing::XXS))
                    .rounded(px(styles::radius::SM))
                    .bg(success.opacity(0.2))
                    .border_1()
                    .border_color(success.opacity(0.4))
                    .text_size(px(styles::font_size::SMALL))
                    .child("Saved"),
            );
        }

        // Adapter config section
        let mut adapter_section = div().flex().flex_col().gap(px(10.0));

        if let Some(ref schema) = self.settings_state.adapter_schema.clone() {
            let mut capitalized = schema.adapter_id.clone();
            if let Some(first) = capitalized.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            let mode_label = match mode {
                PackageMode::User => "User",
                PackageMode::System => "System",
            };
            adapter_section = adapter_section.child(
                div()
                    .text_size(px(styles::font_size::HEADING))
                    .child(format!("Adapter: {capitalized} ({mode_label})")),
            );

            let mut current_section_name: Option<Option<String>> = None;
            let mut current_group: Vec<Div> = Vec::new();

            for field in &schema.fields {
                let field_section = field.section.clone();

                if current_section_name.is_none()
                    || current_section_name.as_ref().unwrap() != &field_section
                {
                    if !current_group.is_empty() {
                        adapter_section = adapter_section.child(
                            div()
                                .p(px(styles::spacing::LG))
                                .rounded(px(styles::radius::LG))
                                .bg(surface)
                                .border_1()
                                .border_color(border)
                                .w_full()
                                .flex()
                                .flex_col()
                                .gap(px(10.0))
                                .children(std::mem::take(&mut current_group)),
                        );
                    }
                    if let Some(ref section_name) = field_section {
                        current_group.push(
                            div()
                                .text_size(px(styles::font_size::BODY))
                                .child(section_name.clone()),
                        );
                    }
                    current_section_name = Some(field_section.clone());
                }

                let value = if field.aeris_managed {
                    self.settings_state
                        .adapter_settings
                        .get(&field.key)
                        .map(|s| ConfigValue::String(s.clone()))
                } else {
                    self.settings_state
                        .adapter_config
                        .values
                        .get(&field.key)
                        .cloned()
                };

                current_group.push(self.render_config_field_row(
                    &field.key,
                    &field.label,
                    &field.field_type,
                    value.as_ref(),
                    field.default.as_ref(),
                    field.aeris_managed,
                    theme,
                    cx,
                ));
            }

            if !current_group.is_empty() {
                adapter_section = adapter_section.child(
                    div()
                        .p(px(styles::spacing::LG))
                        .rounded(px(styles::radius::LG))
                        .bg(surface)
                        .border_1()
                        .border_color(border)
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .children(current_group),
                );
            }

            // Adapter save button
            let adapter_save_enabled =
                self.settings_state.adapter_dirty && !self.settings_state.saving;
            let adapter_save_bg = if adapter_save_enabled {
                primary
            } else {
                surface
            };
            let adapter_save_text = if adapter_save_enabled {
                gpui::white()
            } else {
                text_muted
            };

            let save_adapter_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
                app.save_adapter_settings(cx);
            });

            let revert_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
                app.revert_adapter_settings(cx);
            });
            let revert_enabled = self.settings_state.adapter_dirty;
            let revert_text = if revert_enabled {
                theme.text
            } else {
                text_muted
            };

            adapter_section = adapter_section.child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(styles::spacing::SM))
                    .child(
                        div()
                            .id("save-adapter-btn")
                            .px(px(14.0))
                            .py(px(styles::spacing::XS))
                            .rounded(px(styles::radius::MD))
                            .bg(adapter_save_bg)
                            .text_color(adapter_save_text)
                            .cursor_pointer()
                            .text_size(px(styles::font_size::SMALL))
                            .on_click(save_adapter_listener)
                            .child("Save Adapter Settings"),
                    )
                    .child(
                        div()
                            .id("revert-adapter-btn")
                            .px(px(14.0))
                            .py(px(styles::spacing::XS))
                            .rounded(px(styles::radius::MD))
                            .bg(surface)
                            .border_1()
                            .border_color(border)
                            .text_color(revert_text)
                            .cursor_pointer()
                            .text_size(px(styles::font_size::SMALL))
                            .on_click(revert_listener)
                            .child("Revert"),
                    ),
            );

            if let Some(ref err) = self.settings_state.adapter_save_error {
                adapter_section = adapter_section.child(
                    div()
                        .px(px(styles::spacing::MD))
                        .py(px(styles::spacing::XS))
                        .rounded(px(styles::radius::MD))
                        .bg(danger.opacity(0.15))
                        .border_1()
                        .border_color(danger.opacity(0.3))
                        .text_size(px(styles::font_size::SMALL))
                        .child(format!("Error: {err}")),
                );
            }

            if self.settings_state.adapter_save_success {
                adapter_section = adapter_section.child(
                    div()
                        .px(px(10.0))
                        .py(px(styles::spacing::XXS))
                        .rounded(px(styles::radius::SM))
                        .bg(success.opacity(0.2))
                        .border_1()
                        .border_color(success.opacity(0.4))
                        .text_size(px(styles::font_size::SMALL))
                        .child("Saved"),
                );
            }
        }

        // Full content
        div()
            .p(px(styles::spacing::XL))
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::LG))
            .w_full()
            .child(header)
            .child(aeris_section)
            .child(div().w_full().h(px(1.0)).bg(border))
            .child(adapter_section)
    }

    fn render_theme_selector(
        &self,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let primary = theme.primary;
        let hover = theme.hover;
        let text_color = theme.text;

        let current = self.settings_state.selected_theme;

        let system_listener = cx.listener(|app, _: &ClickEvent, _window, _cx| {
            app.settings_state.selected_theme = AppTheme::System;
            app.selected_theme = AppTheme::System;
            app.settings_state.aeris_dirty = true;
        });
        let light_listener = cx.listener(|app, _: &ClickEvent, _window, _cx| {
            app.settings_state.selected_theme = AppTheme::Light;
            app.selected_theme = AppTheme::Light;
            app.settings_state.aeris_dirty = true;
        });
        let dark_listener = cx.listener(|app, _: &ClickEvent, _window, _cx| {
            app.settings_state.selected_theme = AppTheme::Dark;
            app.selected_theme = AppTheme::Dark;
            app.settings_state.aeris_dirty = true;
        });

        let make_btn = |id: &str,
                        label: &str,
                        is_active: bool,
                        listener: Box<
            dyn Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
        >| {
            let bg = if is_active { primary } else { surface };
            let text = if is_active { gpui::white() } else { text_color };

            div()
                .id(SharedString::from(id.to_string()))
                .px(px(styles::spacing::SM))
                .py(px(styles::spacing::XXS))
                .rounded(px(styles::radius::SM))
                .bg(bg)
                .text_color(text)
                .border_1()
                .border_color(border)
                .cursor_pointer()
                .text_size(px(styles::font_size::SMALL))
                .hover(move |s| if is_active { s } else { s.bg(hover) })
                .on_click(listener)
                .child(label.to_string())
        };

        div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::XXS))
            .child(make_btn(
                "theme-system",
                "System",
                current == AppTheme::System,
                Box::new(system_listener),
            ))
            .child(make_btn(
                "theme-light",
                "Light",
                current == AppTheme::Light,
                Box::new(light_listener),
            ))
            .child(make_btn(
                "theme-dark",
                "Dark",
                current == AppTheme::Dark,
                Box::new(dark_listener),
            ))
    }

    fn render_toggle(
        &self,
        id: &str,
        checked: bool,
        theme: &theme::Theme,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let primary = theme.primary;
        let surface = theme.surface;
        let border = theme.border;

        let label = if checked { "[x]" } else { "[ ]" };
        let bg = if checked { primary } else { surface };
        let text = if checked { gpui::white() } else { theme.text };

        div()
            .px(px(styles::spacing::SM))
            .py(px(styles::spacing::XXS))
            .rounded(px(styles::radius::SM))
            .bg(bg)
            .text_color(text)
            .border_1()
            .border_color(border)
            .text_size(px(styles::font_size::BODY))
            .child(label)
    }

    fn render_config_field_row(
        &self,
        key: &str,
        label: &str,
        field_type: &ConfigFieldType,
        value: Option<&ConfigValue>,
        default: Option<&ConfigValue>,
        aeris_managed: bool,
        theme: &theme::Theme,
        cx: &mut Context<App>,
    ) -> Div {
        let text_muted = theme.text_muted;

        let value_display: gpui::AnyElement = match field_type {
            ConfigFieldType::Toggle => {
                let checked = match value {
                    Some(ConfigValue::Bool(v)) => *v,
                    _ => match default {
                        Some(ConfigValue::Bool(v)) => *v,
                        _ => false,
                    },
                };
                let key_owned = key.to_string();
                let toggle_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
                    app.toggle_adapter_config(&key_owned, cx);
                });
                div()
                    .id(SharedString::from(format!("cfg-toggle-{key}")))
                    .text_size(px(styles::font_size::BODY))
                    .text_color(text_muted)
                    .cursor_pointer()
                    .on_click(toggle_listener)
                    .child(if checked { "[x]" } else { "[ ]" }.to_string())
                    .into_any_element()
            }
            ConfigFieldType::Text | ConfigFieldType::PathList | ConfigFieldType::ExecutablePath => {
                let display = match value {
                    Some(ConfigValue::String(s)) => s.clone(),
                    _ => "(not set)".to_string(),
                };
                div()
                    .text_size(px(styles::font_size::BODY))
                    .text_color(text_muted)
                    .child(display)
                    .into_any_element()
            }
            ConfigFieldType::Number => {
                let display = match value {
                    Some(ConfigValue::String(s)) => s.clone(),
                    Some(ConfigValue::Integer(n)) => n.to_string(),
                    _ => "(not set)".to_string(),
                };
                div()
                    .text_size(px(styles::font_size::BODY))
                    .text_color(text_muted)
                    .child(display)
                    .into_any_element()
            }
            ConfigFieldType::Select(_options) => {
                let display = match value {
                    Some(ConfigValue::String(s)) => s.clone(),
                    _ => match default {
                        Some(ConfigValue::String(s)) => s.clone(),
                        _ => "(not set)".to_string(),
                    },
                };
                div()
                    .text_size(px(styles::font_size::BODY))
                    .text_color(text_muted)
                    .child(display)
                    .into_any_element()
            }
        };

        let label_text = if aeris_managed {
            format!("{label} (aeris)")
        } else {
            label.to_string()
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .child(
                div()
                    .text_size(px(styles::font_size::BODY))
                    .flex_1()
                    .child(label_text),
            )
            .child(value_display)
    }
}
