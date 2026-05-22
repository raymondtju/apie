use super::*;

impl ApiClientApp {
    pub(crate) fn open_settings_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft_settings = self.settings;
        self.settings_dialog_open = true;
        self.method_menu_open = false;
        self.clear_request_overlays();
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub(crate) fn open_settings_from_action(
        &mut self,
        _: &OpenSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft_settings = self.settings;
        self.settings_dialog_open = true;
        self.method_menu_open = false;
        self.clear_request_overlays();
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub(crate) fn close_settings_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_dialog_open = false;
        cx.notify();
    }

    pub(crate) fn apply_draft_settings(&mut self, cx: &mut Context<Self>) {
        self.settings = self.draft_settings.clamped();
        self.draft_settings = self.settings;
        self.theme_mode = Self::theme_mode_from_settings(self.settings, cx);
        self.persist_settings();
        cx.notify();
    }

    pub(crate) fn reset_settings_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft_settings = AppSettings::default();
        self.apply_draft_settings(cx);
    }

    pub(crate) fn cycle_draft_theme(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.draft_settings.theme_preference = self.draft_settings.theme_preference.next();
        self.apply_draft_settings(cx);
    }

    pub(crate) fn decrease_draft_ui_font(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft_settings.adjust_ui_font_size(-1.0);
        self.apply_draft_settings(cx);
    }

    pub(crate) fn increase_draft_ui_font(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft_settings.adjust_ui_font_size(1.0);
        self.apply_draft_settings(cx);
    }

    pub(crate) fn decrease_draft_buffer_font(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft_settings.adjust_buffer_font_size(-1.0);
        self.apply_draft_settings(cx);
    }

    pub(crate) fn increase_draft_buffer_font(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft_settings.adjust_buffer_font_size(1.0);
        self.apply_draft_settings(cx);
    }

    pub(crate) fn toggle_method_menu(
        &mut self,
        event: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_request().is_some() {
            self.request_context_menu = None;
            self.method_menu_open = !self.method_menu_open;
            self.method_menu_position = if self.method_menu_open {
                Some(point(
                    event.position().x - px(42.0),
                    event.position().y + px(16.0),
                ))
            } else {
                None
            };
            cx.notify();
        }
    }

    pub(crate) fn set_active_request_method(
        &mut self,
        method: Method,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(request) = self.active_request_mut() {
            request.method = method;
            if !method.uses_body() && self.active_panel == Panel::Body {
                self.active_panel = Panel::Params;
            }
            self.status_line = format!("Changed request method to {}.", method.as_str()).into();
            self.persist_workspace();
        }
        self.method_menu_open = false;
        self.method_menu_position = None;
        self.request_context_menu = None;
        cx.notify();
    }

    pub(crate) fn set_active_request_method_from_mouse_down(
        &mut self,
        method: Method,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(request) = self.active_request_mut() {
            request.method = method;
            if !method.uses_body() && self.active_panel == Panel::Body {
                self.active_panel = Panel::Params;
            }
            self.status_line = format!("Changed request method to {}.", method.as_str()).into();
            self.persist_workspace();
        }
        self.method_menu_open = false;
        self.method_menu_position = None;
        self.request_context_menu = None;
        cx.notify();
    }

    pub(crate) fn dismiss_method_menu(&mut self, _: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.method_menu_open = false;
        self.method_menu_position = None;
        cx.notify();
    }
}
