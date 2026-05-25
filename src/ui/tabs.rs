use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabPosition {
    First,
    Middle,
    Last,
    Only,
}

#[allow(dead_code)]
pub fn tab(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    theme: AppTheme,
) -> Stateful<Div> {
    tab_with_position(id, label, selected, TabPosition::Middle, theme)
}

pub fn tab_with_position(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    position: TabPosition,
    theme: AppTheme,
) -> Stateful<Div> {
    let spacing = Spacing::app();
    let left_border = matches!(position, TabPosition::First | TabPosition::Only);
    div()
        .id(id)
        .h(spacing.fixed24())
        .min_w(px(128.0))
        .max_w(px(220.0))
        .px(spacing.base08())
        .gap(spacing.control_gap())
        .flex()
        .items_center()
        .when(left_border, |this| this.border_l_1())
        .border_r_1()
        .when(!selected, |this| this.border_b_1())
        .border_color(if selected {
            theme.border
        } else {
            theme.border_variant
        })
        .bg(if selected {
            theme.tab_active_background
        } else {
            theme.tab_inactive_background
        })
        .text_color(if selected {
            theme.text
        } else {
            theme.text_muted
        })
        .hover(move |this| {
            this.bg(if selected {
                theme.tab_active_background
            } else {
                theme.element_hover
            })
            .text_color(theme.text)
        })
        .active(move |this| this.bg(theme.element_active))
        .child(div().truncate().child(label.into()))
}

pub fn tab_shell(
    id: impl Into<gpui::ElementId>,
    selected: bool,
    position: TabPosition,
    theme: AppTheme,
) -> Stateful<Div> {
    let spacing = Spacing::app();
    let left_border = matches!(position, TabPosition::First | TabPosition::Only);
    div()
        .id(id)
        .h(spacing.fixed24())
        .min_w(px(128.0))
        .max_w(px(220.0))
        .px(spacing.base08())
        .gap(spacing.control_gap())
        .flex()
        .items_center()
        .when(left_border, |this| this.border_l_1())
        .border_r_1()
        .when(!selected, |this| this.border_b_1())
        .border_color(if selected {
            theme.border
        } else {
            theme.border_variant
        })
        .bg(if selected {
            theme.tab_active_background
        } else {
            theme.tab_inactive_background
        })
        .text_color(if selected {
            theme.text
        } else {
            theme.text_muted
        })
        .hover(move |this| {
            this.bg(if selected {
                theme.tab_active_background
            } else {
                theme.element_hover
            })
            .text_color(theme.text)
        })
        .active(move |this| this.bg(theme.element_active))
}

pub fn tab_label(id: impl Into<gpui::ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .flex_1()
        .min_w_0()
        .truncate()
        .child(label.into())
}

pub fn tab_close_button(
    id: impl Into<gpui::ElementId>,
    _visible: bool,
    theme: AppTheme,
) -> Stateful<Div> {
    let size = px(18.0);
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(size)
        .h(size)
        .flex_none()
        .rounded_sm()
        .opacity(0.0)
        .text_color(theme.icon_muted)
        .hover(move |this| {
            this.opacity(1.0)
                .bg(theme.ghost_element_hover)
                .text_color(theme.icon)
        })
        .child(icon(IconName::Close, IconSize::XSmall, theme.icon_muted))
}

pub fn context_menu_panel(id: impl Into<gpui::ElementId>, theme: AppTheme) -> Stateful<Div> {
    let spacing = Spacing::app();
    div()
        .id(id)
        .w(px(132.0))
        .p(spacing.base04())
        .rounded_sm()
        .border_1()
        .border_color(theme.panel_focused_border)
        .bg(theme.panel_overlay_background)
        .shadow_lg()
        .occlude()
        .flex()
        .flex_col()
        .gap(spacing.base04())
}

pub fn context_menu_item(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    theme: AppTheme,
    typography: Typography,
) -> Stateful<Div> {
    let spacing = Spacing::app();
    div()
        .id(id)
        .flex()
        .items_center()
        .h(ButtonSize::Default.height())
        .px(spacing.base08())
        .rounded_sm()
        .text_color(theme.text)
        .text_ui(typography)
        .hover(move |this| this.bg(theme.ghost_element_hover))
        .child(label.into())
}

pub fn tab_bar(theme: AppTheme) -> Stateful<Div> {
    h_flex()
        .id("tab-bar")
        .h(px(24.0))
        .w_full()
        .flex_none()
        .overflow_x_scroll()
        .bg(theme.tab_bar_background)
        .border_b_1()
        .border_color(theme.border)
}
