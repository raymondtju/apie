use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    ZedDark,
    ZedLight,
}

impl ThemeMode {
    pub fn from_window_appearance(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::ZedLight,
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::ZedDark,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ZedDark => "Zed Dark",
            Self::ZedLight => "Zed Light",
        }
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct AppTheme {
    pub background: Hsla,
    pub title_bar_background: Hsla,
    pub toolbar_background: Hsla,
    pub panel_background: Hsla,
    pub panel_overlay_background: Hsla,
    pub surface_background: Hsla,
    pub ghost_element_background: Hsla,
    pub ghost_element_hover: Hsla,
    pub ghost_element_active: Hsla,
    pub ghost_element_selected: Hsla,
    pub element_background: Hsla,
    pub element_hover: Hsla,
    pub element_active: Hsla,
    pub element_selected: Hsla,
    pub tab_bar_background: Hsla,
    pub tab_active_background: Hsla,
    pub tab_inactive_background: Hsla,
    pub status_bar_background: Hsla,
    pub border: Hsla,
    pub border_variant: Hsla,
    pub border_focused: Hsla,
    pub border_selected: Hsla,
    pub border_transparent: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub text_placeholder: Hsla,
    pub text_disabled: Hsla,
    pub icon: Hsla,
    pub icon_muted: Hsla,
    pub icon_disabled: Hsla,
    pub accent: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    pub error: Hsla,
    pub editor_background: Hsla,
    pub editor_text: Hsla,
    pub panel_focused_border: Hsla,
    pub pane_focused_border: Hsla,
    pub overlay_scrim: Hsla,
}

impl AppTheme {
    pub fn timing_phase_color(&self, index: usize) -> Hsla {
        match index {
            0 => rgb(0xeab308).into(), // DNS - yellow
            1 => rgb(0xf97316).into(), // Connect - orange
            2 => rgb(0xa855f7).into(), // TLS - purple
            3 => rgb(0x3b82f6).into(), // TTFB - blue
            _ => rgb(0x22c55e).into(), // Transfer - green
        }
    }

    pub fn timing_phase_name(index: usize) -> &'static str {
        match index {
            0 => "DNS Lookup",
            1 => "Connect",
            2 => "TLS Handshake",
            3 => "TTFB",
            _ => "Transfer",
        }
    }

    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::ZedDark => Self {
                background: rgb(0x181818).into(),
                title_bar_background: rgb(0x202020).into(),
                toolbar_background: rgb(0x242424).into(),
                panel_background: rgb(0x202020).into(),
                panel_overlay_background: rgb(0x2a2a2a).into(),
                surface_background: rgb(0x1f1f1f).into(),
                ghost_element_background: rgba(0x00000000).into(),
                ghost_element_hover: rgba(0xffffff12).into(),
                ghost_element_active: rgba(0xffffff1f).into(),
                ghost_element_selected: rgba(0x4f8cff24).into(),
                element_background: rgb(0x2a2a2a).into(),
                element_hover: rgb(0x333333).into(),
                element_active: rgb(0x3a3a3a).into(),
                element_selected: rgb(0x30343f).into(),
                tab_bar_background: rgb(0x1f1f1f).into(),
                tab_active_background: rgb(0x2b2b2b).into(),
                tab_inactive_background: rgb(0x202020).into(),
                status_bar_background: rgb(0x202020).into(),
                border: rgb(0x3d3d3d).into(),
                border_variant: rgb(0x333333).into(),
                border_focused: rgb(0x4f8cff).into(),
                border_selected: rgba(0x4f8cff99).into(),
                border_transparent: rgba(0x00000000).into(),
                text: rgb(0xd6d6d6).into(),
                text_muted: rgb(0x9a9a9a).into(),
                text_placeholder: rgb(0x747474).into(),
                text_disabled: rgb(0x666666).into(),
                icon: rgb(0xc2c2c2).into(),
                icon_muted: rgb(0x8a8a8a).into(),
                icon_disabled: rgb(0x5d5d5d).into(),
                accent: rgb(0x4f8cff).into(),
                success: rgb(0x4ec26f).into(),
                warning: rgb(0xd9a441).into(),
                error: rgb(0xff6b6b).into(),
                editor_background: rgb(0x151515).into(),
                editor_text: rgb(0xd7d7d7).into(),
                panel_focused_border: rgba(0x4f8cff70).into(),
                pane_focused_border: rgba(0x4f8cff55).into(),
                overlay_scrim: rgba(0x00000080).into(),
            },
            ThemeMode::ZedLight => Self {
                background: rgb(0xf5f5f5).into(),
                title_bar_background: rgb(0xe9e9e9).into(),
                toolbar_background: rgb(0xf0f0f0).into(),
                panel_background: rgb(0xefefef).into(),
                panel_overlay_background: rgb(0xffffff).into(),
                surface_background: rgb(0xffffff).into(),
                ghost_element_background: rgba(0x00000000).into(),
                ghost_element_hover: rgba(0x00000014).into(),
                ghost_element_active: rgba(0x00000020).into(),
                ghost_element_selected: rgba(0x1f6feb18).into(),
                element_background: rgb(0xffffff).into(),
                element_hover: rgb(0xe8e8e8).into(),
                element_active: rgb(0xdfdfdf).into(),
                element_selected: rgb(0xdce9ff).into(),
                tab_bar_background: rgb(0xe9e9e9).into(),
                tab_active_background: rgb(0xffffff).into(),
                tab_inactive_background: rgb(0xe9e9e9).into(),
                status_bar_background: rgb(0xe9e9e9).into(),
                border: rgb(0xc9c9c9).into(),
                border_variant: rgb(0xd9d9d9).into(),
                border_focused: rgb(0x1f6feb).into(),
                border_selected: rgba(0x1f6feb99).into(),
                border_transparent: rgba(0x00000000).into(),
                text: rgb(0x222222).into(),
                text_muted: rgb(0x666666).into(),
                text_placeholder: rgb(0x8c8c8c).into(),
                text_disabled: rgb(0xaaaaaa).into(),
                icon: rgb(0x333333).into(),
                icon_muted: rgb(0x6c6c6c).into(),
                icon_disabled: rgb(0xa0a0a0).into(),
                accent: rgb(0x1f6feb).into(),
                success: rgb(0x15803d).into(),
                warning: rgb(0xb7791f).into(),
                error: rgb(0xc24141).into(),
                editor_background: rgb(0xfbfbfb).into(),
                editor_text: rgb(0x222222).into(),
                panel_focused_border: rgba(0x1f6feb70).into(),
                pane_focused_border: rgba(0x1f6feb55).into(),
                overlay_scrim: rgba(0x00000040).into(),
            },
        }
    }
}
