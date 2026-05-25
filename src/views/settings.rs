use std::collections::HashMap;

use gpui::*;

use crate::{
    app::{App, AppTheme, View},
    components::TextInput,
    config::AerisConfig,
    core::{
        adapter::Adapter,
        config::{AdapterConfig, ConfigFieldType, ConfigSchema, ConfigValue},
        privilege::PackageMode,
    },
    styles, theme,
};

pub struct SettingsEdit {
    pub key: String,
    pub label: String,
    pub field_type: ConfigFieldType,
    pub input: Entity<TextInput>,
}

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
    pub edit: Option<SettingsEdit>,
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
            edit: None,
        }
    }
}

impl SettingsState {
    pub fn load(aeris_config: &AerisConfig, adapter: &dyn Adapter) -> Self {
        let schema = adapter.config_schema();
        let mut adapter_settings = HashMap::new();

        if let Some(ref schema) = schema {
            for field in &schema.fields {
                if field.aeris_managed
                    && let Some(value) =
                        aeris_config.get_adapter_setting(&schema.adapter_id, &field.key)
                {
                    adapter_settings.insert(field.key.clone(), value.to_string());
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
            edit: None,
        }
    }
}

impl App {
    pub fn render_settings(
        &mut self,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let border = theme.border;
        let text_muted = theme.text_muted;
        let danger = theme.danger;
        let success = theme.success;

        let mode = self.current_mode;

        let header = div()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::XXS))
            .child(
                div()
                    .text_size(px(styles::font_size::TITLE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Settings"),
            )
            .child(
                div()
                    .text_size(px(styles::font_size::SMALL))
                    .text_color(text_muted)
                    .child("Preferences for Aeris and the active adapter."),
            );

        let appearance_card = self.section_card(
            "Appearance",
            Some("How Aeris looks on your system."),
            vec![
                field_row("Theme", self.render_theme_selector(theme, cx).into_any_element()),
            ],
            theme,
        );

        let general_card = self.section_card(
            "General",
            Some("Default behavior when you launch Aeris."),
            vec![
                field_row(
                    "Startup view",
                    div()
                        .text_size(px(styles::font_size::BODY))
                        .text_color(text_muted)
                        .child(format!("{}", self.settings_state.startup_view))
                        .into_any_element(),
                ),
                field_row(
                    "Notifications",
                    self.render_notifications_toggle(theme, cx).into_any_element(),
                ),
            ],
            theme,
        );

        let aeris_actions = self.render_aeris_actions(theme, cx);

        let mut aeris_section = div()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::MD))
            .child(appearance_card)
            .child(general_card)
            .child(aeris_actions);

        if let Some(ref err) = self.settings_state.aeris_save_error {
            aeris_section = aeris_section.child(banner(err, danger, true));
        }
        if self.settings_state.aeris_save_success {
            aeris_section = aeris_section.child(banner("Saved", success, false));
        }

        let mut adapter_section = div()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::MD));

        if let Some(schema) = self.settings_state.adapter_schema.clone() {
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
                    .flex()
                    .flex_col()
                    .gap(px(styles::spacing::XXS))
                    .child(
                        div()
                            .text_size(px(styles::font_size::HEADING))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("Adapter: {capitalized}")),
                    )
                    .child(
                        div()
                            .text_size(px(styles::font_size::SMALL))
                            .text_color(text_muted)
                            .child(format!(
                                "Settings for the {capitalized} adapter in {mode_label} mode."
                            )),
                    ),
            );

            adapter_section =
                adapter_section.child(self.render_adapter_fields(&schema, theme, cx));

            adapter_section = adapter_section.child(self.render_adapter_actions(theme, cx));

            if let Some(ref err) = self.settings_state.adapter_save_error {
                adapter_section = adapter_section.child(banner(err, danger, true));
            }
            if self.settings_state.adapter_save_success {
                adapter_section = adapter_section.child(banner("Saved", success, false));
            }
        }

        div()
            .id("settings-scroll")
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .child(
                div()
                    .p(px(styles::spacing::XL))
                    .flex()
                    .flex_col()
                    .gap(px(styles::spacing::XL))
                    .w_full()
                    .child(header)
                    .child(aeris_section)
                    .child(div().w_full().h(px(1.0)).bg(border))
                    .child(adapter_section),
            )
    }

    fn section_card(
        &self,
        title: &str,
        description: Option<&str>,
        rows: Vec<Div>,
        theme: &theme::Theme,
    ) -> Div {
        let surface = theme.surface;
        let border = theme.border;
        let text_muted = theme.text_muted;

        let mut header_col = div()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::XXXS))
            .child(
                div()
                    .text_size(px(styles::font_size::HEADING))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title.to_string()),
            );
        if let Some(desc) = description {
            header_col = header_col.child(
                div()
                    .text_size(px(styles::font_size::SMALL))
                    .text_color(text_muted)
                    .child(desc.to_string()),
            );
        }

        let row_count = rows.len();
        let mut body = div().flex().flex_col();
        for (i, row) in rows.into_iter().enumerate() {
            let mut row = row.py(px(styles::spacing::SM));
            if i + 1 < row_count {
                row = row.border_b_1().border_color(border);
            }
            body = body.child(row);
        }

        div()
            .rounded(px(styles::radius::LG))
            .bg(surface)
            .border_1()
            .border_color(border)
            .w_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .px(px(styles::spacing::LG))
                    .py(px(styles::spacing::MD))
                    .border_b_1()
                    .border_color(border)
                    .child(header_col),
            )
            .child(div().px(px(styles::spacing::LG)).child(body))
    }

    fn render_adapter_fields(
        &mut self,
        schema: &ConfigSchema,
        theme: &theme::Theme,
        cx: &mut Context<App>,
    ) -> Div {
        let surface = theme.surface;
        let border = theme.border;
        let text_muted = theme.text_muted;

        let mut card = div()
            .rounded(px(styles::radius::LG))
            .bg(surface)
            .border_1()
            .border_color(border)
            .w_full()
            .flex()
            .flex_col();

        let mut current_section: Option<Option<String>> = None;
        let mut group: Vec<Div> = Vec::new();
        let mut first_group = true;

        for field in &schema.fields {
            let field_section = field.section.clone();
            if current_section.as_ref() != Some(&field_section) {
                if !group.is_empty() {
                    card = card.child(flush_group(
                        std::mem::take(&mut group),
                        first_group,
                        border,
                    ));
                    first_group = false;
                }
                if let Some(ref name) = field_section {
                    group.push(subsection_header(name, text_muted));
                }
                current_section = Some(field_section.clone());
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

            group.push(self.render_config_field_row(
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

        if !group.is_empty() {
            card = card.child(flush_group(group, first_group, border));
        }

        card
    }

    fn render_aeris_actions(
        &self,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let save_enabled = self.settings_state.aeris_dirty && !self.settings_state.saving;
        let save_aeris = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.save_aeris_settings(cx);
        });
        div()
            .flex()
            .flex_row()
            .child(action_button(
                "save-aeris-btn",
                "Save Aeris Settings",
                save_enabled,
                true,
                theme,
                Box::new(save_aeris),
            ))
    }

    fn render_adapter_actions(
        &self,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let save_enabled = self.settings_state.adapter_dirty && !self.settings_state.saving;
        let revert_enabled = self.settings_state.adapter_dirty;

        let save_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.save_adapter_settings(cx);
        });
        let revert_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.revert_adapter_settings(cx);
        });

        div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::SM))
            .child(action_button(
                "save-adapter-btn",
                "Save Adapter Settings",
                save_enabled,
                true,
                theme,
                Box::new(save_listener),
            ))
            .child(action_button(
                "revert-adapter-btn",
                "Revert",
                revert_enabled,
                false,
                theme,
                Box::new(revert_listener),
            ))
    }

    fn render_theme_selector(
        &self,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
            let btn_bg = if is_active { primary } else { transparent_black() };
            let text = if is_active { gpui::white() } else { text_color };

            div()
                .id(SharedString::from(id.to_string()))
                .px(px(styles::spacing::MD))
                .py(px(styles::spacing::XXS))
                .rounded(px(styles::radius::MD))
                .bg(btn_bg)
                .text_color(text)
                .cursor_pointer()
                .text_size(px(styles::font_size::SMALL))
                .hover(move |s| if is_active { s } else { s.bg(hover) })
                .on_click(listener)
                .child(label.to_string())
        };

        div()
            .flex()
            .flex_row()
            .p(px(styles::spacing::XXXS))
            .gap(px(styles::spacing::XXXS))
            .rounded(px(styles::radius::MD))
            .border_1()
            .border_color(border)
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

    fn render_notifications_toggle(
        &self,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let checked = self.settings_state.notifications;
        let listener = cx.listener(|app, _: &ClickEvent, _window, _cx| {
            app.settings_state.notifications = !app.settings_state.notifications;
            app.settings_state.aeris_dirty = true;
        });
        switch("notifications-toggle", checked, theme, Box::new(listener))
    }

    fn editable_field_display(
        &self,
        key: &str,
        label: &str,
        field_type: &ConfigFieldType,
        display: String,
        theme: &theme::Theme,
        cx: &mut Context<App>,
    ) -> gpui::AnyElement {
        let text_muted = theme.text_muted;
        let primary = theme.primary;
        let key_owned = key.to_string();
        let label_owned = label.to_string();
        let field_type_owned = field_type.clone();
        let listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
            app.open_settings_edit(&key_owned, &label_owned, field_type_owned.clone(), cx);
        });
        div()
            .id(SharedString::from(format!("cfg-edit-{key}")))
            .text_size(px(styles::font_size::BODY))
            .text_color(text_muted)
            .cursor_pointer()
            .hover(move |s| s.text_color(primary))
            .on_click(listener)
            .child(display)
            .into_any_element()
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
        let value_display: gpui::AnyElement = match field_type {
            ConfigFieldType::Toggle => {
                let checked = match value {
                    Some(ConfigValue::Bool(v)) => *v,
                    _ => matches!(default, Some(ConfigValue::Bool(true))),
                };
                let key_owned = key.to_string();
                let toggle_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
                    app.toggle_adapter_config(&key_owned, cx);
                });
                switch(
                    &format!("cfg-toggle-{key}"),
                    checked,
                    theme,
                    Box::new(toggle_listener),
                )
                .into_any_element()
            }
            ConfigFieldType::Text | ConfigFieldType::PathList | ConfigFieldType::ExecutablePath => {
                let display = match value {
                    Some(ConfigValue::String(s)) => s.clone(),
                    _ => "(not set)".to_string(),
                };
                self.editable_field_display(key, label, field_type, display, theme, cx)
            }
            ConfigFieldType::Number => {
                let display = match value {
                    Some(ConfigValue::String(s)) => s.clone(),
                    Some(ConfigValue::Integer(n)) => n.to_string(),
                    _ => "(not set)".to_string(),
                };
                self.editable_field_display(key, label, field_type, display, theme, cx)
            }
            ConfigFieldType::Select(_options) => {
                let display = match value {
                    Some(ConfigValue::String(s)) => s.clone(),
                    _ => match default {
                        Some(ConfigValue::String(s)) => s.clone(),
                        _ => "(not set)".to_string(),
                    },
                };
                self.editable_field_display(key, label, field_type, display, theme, cx)
            }
        };

        let label_row = div()
            .text_size(px(styles::font_size::BODY))
            .flex_1()
            .child(label.to_string());

        let mut row = div().flex().flex_row().items_center().child(label_row);

        if aeris_managed {
            row = row.child(
                div()
                    .mr(px(styles::spacing::SM))
                    .px(px(styles::spacing::XS))
                    .py(px(styles::spacing::XXXS))
                    .rounded(px(styles::radius::SM))
                    .bg(theme.primary.opacity(0.15))
                    .text_size(px(styles::font_size::CAPTION))
                    .text_color(theme.primary)
                    .child("aeris"),
            );
        }

        row.child(value_display)
    }
}

fn field_row(label: &str, value: AnyElement) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .child(
            div()
                .text_size(px(styles::font_size::BODY))
                .flex_1()
                .child(label.to_string()),
        )
        .child(value)
}

fn subsection_header(name: &str, color: Hsla) -> Div {
    div()
        .pt(px(styles::spacing::SM))
        .pb(px(styles::spacing::XXS))
        .text_size(px(styles::font_size::CAPTION))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(name.to_uppercase())
}

fn flush_group(group: Vec<Div>, first: bool, border: Hsla) -> Div {
    let count = group.len();
    let mut body = div().flex().flex_col();
    for (i, row) in group.into_iter().enumerate() {
        let mut row = row.py(px(styles::spacing::SM));
        if i + 1 < count {
            row = row.border_b_1().border_color(border);
        }
        body = body.child(row);
    }
    let mut wrap = div().px(px(styles::spacing::LG)).child(body);
    if !first {
        wrap = wrap.border_t_1().border_color(border);
    }
    wrap
}

fn switch(
    id: &str,
    checked: bool,
    theme: &theme::Theme,
    listener: Box<dyn Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static>,
) -> Stateful<Div> {
    let track_on = theme.primary;
    let track_off = theme.border;
    let thumb = gpui::white();

    let track = if checked { track_on } else { track_off };

    let thumb_el = if checked {
        div()
            .ml_auto()
            .w(px(16.0))
            .h(px(16.0))
            .rounded_full()
            .bg(thumb)
    } else {
        div().w(px(16.0)).h(px(16.0)).rounded_full().bg(thumb)
    };

    div()
        .id(SharedString::from(id.to_string()))
        .w(px(34.0))
        .h(px(20.0))
        .p(px(2.0))
        .rounded_full()
        .bg(track)
        .cursor_pointer()
        .flex()
        .flex_row()
        .items_center()
        .on_click(listener)
        .child(thumb_el)
}

fn action_button(
    id: &str,
    label: &str,
    enabled: bool,
    primary: bool,
    theme: &theme::Theme,
    listener: Box<dyn Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static>,
) -> Stateful<Div> {
    let bg = if enabled && primary {
        theme.primary
    } else if enabled {
        theme.surface
    } else {
        theme.surface
    };
    let text = if enabled && primary {
        gpui::white()
    } else if enabled {
        theme.text
    } else {
        theme.text_muted
    };
    let hover_bg = if enabled && primary {
        theme.primary.opacity(0.85)
    } else if enabled {
        theme.hover
    } else {
        theme.surface
    };

    let mut btn = div()
        .id(SharedString::from(id.to_string()))
        .px(px(styles::spacing::LG))
        .py(px(styles::spacing::XS))
        .rounded(px(styles::radius::MD))
        .bg(bg)
        .text_color(text)
        .text_size(px(styles::font_size::SMALL))
        .font_weight(FontWeight::MEDIUM)
        .border_1()
        .border_color(if primary && enabled {
            theme.primary
        } else {
            theme.border
        });

    if enabled {
        btn = btn
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg))
            .on_click(listener);
    }

    btn.child(label.to_string())
}

fn banner(text: &str, color: Hsla, is_error: bool) -> Div {
    let prefix = if is_error { "Error: " } else { "" };
    div()
        .px(px(styles::spacing::MD))
        .py(px(styles::spacing::XS))
        .rounded(px(styles::radius::MD))
        .bg(color.opacity(0.15))
        .border_1()
        .border_color(color.opacity(0.3))
        .text_size(px(styles::font_size::SMALL))
        .child(format!("{prefix}{text}"))
}
