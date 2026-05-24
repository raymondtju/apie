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

    /// Returns the embedded SVG bytes for this icon (compile-time via include_bytes!).
    /// This removes all runtime dependence on CARGO_MANIFEST_DIR / source tree location.
    ///
    /// When adding a new IconName variant:
    ///   1. Add the matching arm here with the correct include_bytes! path.
    ///   2. Add the leaf filename to ICON_FILENAMES in the helper below.
    ///   3. (If the icon will be used) add a usage site via `icon(...)` or `icon_button_base(...)`.
    pub(crate) fn bytes(self) -> &'static [u8] {
        match self {
            Self::Send => include_bytes!("../../assets/icons/send.svg"),
            Self::Plus => include_bytes!("../../assets/icons/plus.svg"),
            Self::Settings => include_bytes!("../../assets/icons/settings.svg"),
            Self::Copy => include_bytes!("../../assets/icons/copy.svg"),
            Self::Trash => include_bytes!("../../assets/icons/trash.svg"),
            Self::Close => include_bytes!("../../assets/icons/close.svg"),
            Self::Check => include_bytes!("../../assets/icons/check.svg"),
            Self::ChevronDown => include_bytes!("../../assets/icons/chevron-down.svg"),
            Self::PanelLeft => include_bytes!("../../assets/icons/panel-left.svg"),
            Self::PanelRight => include_bytes!("../../assets/icons/panel-right.svg"),
            Self::Search => include_bytes!("../../assets/icons/search.svg"),
            Self::Code => include_bytes!("../../assets/icons/code.svg"),
        }
    }
}

/// Helper used by the AssetSource in main.rs to resolve "icons/xxx.svg" paths to bytes.
/// Keeps all icon filename knowledge co-located with the enum.
pub(crate) fn icon_bytes_for_path(path: &str) -> Option<&'static [u8]> {
    // Support both forward and backslash forms (defensive)
    let leaf = path
        .strip_prefix("icons/")
        .or_else(|| path.strip_prefix("icons\\"))?;
    let name = match leaf {
        "send.svg" => IconName::Send,
        "plus.svg" => IconName::Plus,
        "settings.svg" => IconName::Settings,
        "copy.svg" => IconName::Copy,
        "trash.svg" => IconName::Trash,
        "close.svg" => IconName::Close,
        "check.svg" => IconName::Check,
        "chevron-down.svg" => IconName::ChevronDown,
        "panel-left.svg" => IconName::PanelLeft,
        "panel-right.svg" => IconName::PanelRight,
        "search.svg" => IconName::Search,
        "code.svg" => IconName::Code,
        _ => return None,
    };
    Some(name.bytes())
}

/// Static list of icon filenames for AssetSource::list("icons") queries.
pub(crate) const ICON_FILENAMES: &[&str] = &[
    "send.svg",
    "plus.svg",
    "settings.svg",
    "copy.svg",
    "trash.svg",
    "close.svg",
    "check.svg",
    "chevron-down.svg",
    "panel-left.svg",
    "panel-right.svg",
    "search.svg",
    "code.svg",
];

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
