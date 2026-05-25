use super::*;
pub fn h_flex() -> Div {
    div().flex().items_center()
}

pub fn v_flex() -> Div {
    div().flex().flex_col()
}

pub fn label(text: impl Into<SharedString>, theme: AppTheme) -> Div {
    div().text_color(theme.text).child(text.into())
}

pub fn muted_label(text: impl Into<SharedString>, theme: AppTheme) -> Div {
    div().text_color(theme.text_muted).child(text.into())
}
pub fn dock_panel(id: impl Into<gpui::ElementId>, theme: AppTheme) -> Stateful<Div> {
    v_flex()
        .id(id)
        .h_full()
        .bg(theme.panel_background)
        .border_color(theme.border)
        .overflow_hidden()
}

pub fn pane(id: impl Into<gpui::ElementId>, theme: AppTheme) -> Stateful<Div> {
    v_flex()
        .id(id)
        .min_w_0()
        .flex_1()
        .h_full()
        .bg(theme.surface_background)
        .border_color(theme.border)
        .overflow_hidden()
}

pub fn toolbar(id: impl Into<gpui::ElementId>, theme: AppTheme) -> Stateful<Div> {
    let spacing = Spacing::app();
    h_flex()
        .id(id)
        .h(px(40.0))
        .flex_none()
        .gap(spacing.cluster_gap())
        .border_b_1()
        .border_color(theme.border)
        .bg(theme.toolbar_background)
        .px(spacing.base12())
}

#[allow(dead_code)]
pub fn toolbar_group(theme: AppTheme) -> Div {
    let spacing = Spacing::app();
    h_flex()
        .gap(spacing.cluster_gap())
        .rounded_sm()
        .border_1()
        .border_color(theme.border_variant)
        .bg(theme.ghost_element_background)
        .p(spacing.base04())
}

pub fn panel_header(
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    theme: AppTheme,
    typography: Typography,
) -> Div {
    let spacing = Spacing::app();
    h_flex()
        .justify_between()
        .px(spacing.base12())
        .py(spacing.base08())
        .border_b_1()
        .border_color(theme.border_variant)
        .bg(theme.toolbar_background)
        .child(
            label(title, theme)
                .text_ui_sm(typography)
                .font_weight(gpui::FontWeight::SEMIBOLD),
        )
        .child(muted_label(detail, theme).text_ui_xs(typography))
}

pub fn input_field_shell(
    id: impl Into<gpui::ElementId>,
    focused: bool,
    theme: AppTheme,
    typography: Typography,
) -> Stateful<Div> {
    input_field_shell_with_selector(id, "url-input-shell", focused, theme, typography)
}

pub fn input_field_shell_with_selector(
    id: impl Into<gpui::ElementId>,
    debug_selector: &'static str,
    focused: bool,
    theme: AppTheme,
    typography: Typography,
) -> Stateful<Div> {
    input_field_shell_with_height(
        id,
        debug_selector,
        focused,
        ButtonSize::Medium.height(),
        theme,
        typography,
    )
}

pub fn input_field_shell_with_height(
    id: impl Into<gpui::ElementId>,
    debug_selector: &'static str,
    focused: bool,
    height: Pixels,
    theme: AppTheme,
    typography: Typography,
) -> Stateful<Div> {
    let spacing = Spacing::app();
    h_flex()
        .id(id)
        .debug_selector(move || debug_selector.into())
        .flex_1()
        .h(height)
        .min_w(px(192.0))
        .rounded_sm()
        .border_1()
        .border_color(if focused {
            theme.border_focused
        } else {
            theme.border_variant
        })
        .bg(theme.editor_background)
        .px(spacing.base08())
        .text_color(theme.editor_text)
        .text_ui(typography)
        .hover(move |this| this.border_color(theme.border_focused))
}

pub fn panel_surface(theme: AppTheme) -> Div {
    v_flex()
        .rounded_sm()
        .border_1()
        .border_color(theme.border_variant)
        .bg(theme.panel_overlay_background)
        .overflow_hidden()
}

pub fn flat_section(theme: AppTheme) -> Div {
    v_flex()
        .border_color(theme.border_variant)
        .bg(theme.panel_overlay_background)
        .overflow_hidden()
}

pub fn list_item(id: impl Into<gpui::ElementId>, selected: bool, theme: AppTheme) -> Stateful<Div> {
    let spacing = Spacing::app();
    v_flex()
        .id(id)
        .gap(spacing.control_gap())
        .p(spacing.base08())
        .rounded_sm()
        .border_1()
        .border_color(if selected {
            theme.border_selected
        } else {
            theme.border_transparent
        })
        .bg(if selected {
            theme.ghost_element_selected
        } else {
            theme.ghost_element_background
        })
        .hover(move |this| {
            this.bg(if selected {
                theme.element_selected
            } else {
                theme.ghost_element_hover
            })
        })
}

#[allow(dead_code)]
pub fn badge(
    label: impl Into<SharedString>,
    color: Hsla,
    typography: Typography,
    _theme: AppTheme,
) -> Div {
    let spacing = Spacing::app();
    div()
        .h(ButtonSize::Default.height())
        .min_w(px(46.0))
        .px(spacing.base08())
        .rounded_sm()
        .border_1()
        .border_color(color.opacity(0.35))
        .bg(color.opacity(0.12))
        .text_color(color)
        .text_ui_xs(typography)
        .font_weight(gpui::FontWeight::BOLD)
        .child(label.into())
}

pub fn status_bar(theme: AppTheme, typography: Typography) -> Stateful<Div> {
    let spacing = Spacing::app();
    h_flex()
        .id("status-bar")
        .h(spacing.fixed24())
        .flex_none()
        .border_t_1()
        .border_color(theme.border)
        .bg(theme.status_bar_background)
        .px(spacing.base12())
        .text_ui_xs(typography)
        .text_color(theme.text_muted)
}

pub fn modal_overlay(theme: AppTheme) -> Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom_0()
        .bg(theme.overlay_scrim)
}
