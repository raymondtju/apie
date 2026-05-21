use super::*;
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum ButtonStyle {
    Subtle,
    Filled,
    Transparent,
    Outlined,
    Tinted(TintColor),
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum TintColor {
    Accent,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ButtonSize {
    Large,
    Medium,
    Default,
    Compact,
}

impl ButtonSize {
    pub fn height(self) -> Pixels {
        match self {
            Self::Large => px(32.0),
            Self::Medium => px(28.0),
            Self::Default => px(22.0),
            Self::Compact => px(18.0),
        }
    }

    pub fn horizontal_padding(self) -> Pixels {
        match self {
            Self::Large | Self::Medium => Spacing::app().base08(),
            Self::Default | Self::Compact => Spacing::app().base06(),
        }
    }
}

pub fn button_like(
    id: impl Into<gpui::ElementId>,
    selected: bool,
    style: ButtonStyle,
    size: ButtonSize,
    theme: AppTheme,
) -> Stateful<Div> {
    let (background, text, border) = button_colors(selected, style, theme);
    let spacing = Spacing::app();
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(size.height())
        .px(size.horizontal_padding())
        .gap(spacing.control_gap())
        .rounded_sm()
        .border_1()
        .border_color(border)
        .bg(background)
        .text_color(text)
        .hover(move |this| {
            let hover_background = if selected {
                theme.ghost_element_selected
            } else {
                match style {
                    ButtonStyle::Transparent => theme.ghost_element_hover,
                    _ => theme.element_hover,
                }
            };
            this.bg(hover_background).text_color(theme.text)
        })
        .active(move |this| this.bg(theme.element_active))
}

fn button_colors(selected: bool, style: ButtonStyle, theme: AppTheme) -> (Hsla, Hsla, Hsla) {
    if selected {
        return (
            theme.ghost_element_selected,
            theme.text,
            theme.border_selected,
        );
    }

    match style {
        ButtonStyle::Filled => (theme.accent, rgb(0xffffff).into(), theme.accent),
        ButtonStyle::Subtle => (theme.element_background, theme.text, theme.border_variant),
        ButtonStyle::Transparent => (
            theme.ghost_element_background,
            theme.text_muted,
            theme.border_transparent,
        ),
        ButtonStyle::Outlined => (
            theme.ghost_element_background,
            theme.text,
            theme.border_variant,
        ),
        ButtonStyle::Tinted(tint) => {
            let color = match tint {
                TintColor::Accent => theme.accent,
                TintColor::Success => theme.success,
                TintColor::Warning => theme.warning,
                TintColor::Error => theme.error,
            };
            (color.opacity(0.12), color, color.opacity(0.35))
        }
    }
}

pub fn button_base(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    style: ButtonStyle,
    theme: AppTheme,
) -> Stateful<Div> {
    button_base_with_size(id, label, selected, style, ButtonSize::Default, theme)
}

pub fn button_base_with_size(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    style: ButtonStyle,
    size: ButtonSize,
    theme: AppTheme,
) -> Stateful<Div> {
    button_like(id, selected, style, size, theme).child(label.into())
}
#[allow(dead_code)]
pub fn select_trigger(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    color: Hsla,
    open: bool,
    theme: AppTheme,
    typography: Typography,
) -> Stateful<Div> {
    select_trigger_with_size(
        id,
        label,
        color,
        open,
        ButtonSize::Default,
        theme,
        typography,
    )
}

pub fn select_trigger_with_size(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    color: Hsla,
    open: bool,
    size: ButtonSize,
    theme: AppTheme,
    typography: Typography,
) -> Stateful<Div> {
    let spacing = Spacing::app();
    button_like(id, open, ButtonStyle::Transparent, size, theme)
        .min_w(px(76.0))
        .px(spacing.base08())
        .border_color(if open {
            color.opacity(0.45)
        } else {
            color.opacity(0.28)
        })
        .bg(if open {
            color.opacity(0.16)
        } else {
            color.opacity(0.1)
        })
        .text_color(color)
        .text_ui(typography)
        .font_weight(gpui::FontWeight::BOLD)
        .child(label.into())
        .child(icon(
            IconName::ChevronDown,
            IconSize::XSmall,
            theme.icon_muted,
        ))
}

pub fn select_menu_item(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    color: Hsla,
    selected: bool,
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
        .bg(if selected {
            color.opacity(0.14)
        } else {
            theme.ghost_element_background
        })
        .text_color(if selected { color } else { theme.text })
        .text_ui(typography)
        .font_weight(if selected {
            gpui::FontWeight::BOLD
        } else {
            gpui::FontWeight::NORMAL
        })
        .hover(move |this| this.bg(theme.ghost_element_hover).text_color(color))
        .child(label.into())
}
