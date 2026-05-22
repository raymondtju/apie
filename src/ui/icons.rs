use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum IconName {
    Send,
    Plus,
    Settings,
    Copy,
    Trash,
    Close,
    Check,
    ChevronDown,
    PanelLeft,
    PanelRight,
    Search,
    Code,
}

impl IconName {
    fn path(self) -> &'static str {
        match self {
            Self::Send => "icons/send.svg",
            Self::Plus => "icons/plus.svg",
            Self::Settings => "icons/settings.svg",
            Self::Copy => "icons/copy.svg",
            Self::Trash => "icons/trash.svg",
            Self::Close => "icons/close.svg",
            Self::Check => "icons/check.svg",
            Self::ChevronDown => "icons/chevron-down.svg",
            Self::PanelLeft => "icons/panel-left.svg",
            Self::PanelRight => "icons/panel-right.svg",
            Self::Search => "icons/search.svg",
            Self::Code => "icons/code.svg",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum IconSize {
    Indicator,
    XSmall,
    Small,
    Medium,
}

impl IconSize {
    fn pixels(self) -> Pixels {
        match self {
            Self::Indicator => px(10.0),
            Self::XSmall => px(12.0),
            Self::Small => px(14.0),
            Self::Medium => px(16.0),
        }
    }
}

pub fn icon(name: IconName, size: IconSize, color: Hsla) -> impl IntoElement {
    let size = size.pixels();
    svg()
        .path(name.path())
        .w(size)
        .h(size)
        .flex_none()
        .text_color(color)
}

pub fn icon_button_base(
    id: impl Into<gpui::ElementId>,
    icon_name: IconName,
    selected: bool,
    theme: AppTheme,
) -> Stateful<Div> {
    button_like(
        id,
        selected,
        ButtonStyle::Transparent,
        ButtonSize::Default,
        theme,
    )
    .child(icon(icon_name, IconSize::Medium, theme.icon))
    .w(ButtonSize::Default.height())
}
