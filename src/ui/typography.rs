use super::*;
const BASE_REM_SIZE_IN_PX: f32 = 16.0;
pub const JETBRAINS_FONT_FAMILY: &str = "JetBrains Mono";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Typography {
    pub ui_font_size: f32,
    pub buffer_font_size: f32,
}

impl Typography {
    pub fn new(ui_font_size: f32, buffer_font_size: f32) -> Self {
        Self {
            ui_font_size,
            buffer_font_size,
        }
    }

    pub fn ui_default(self) -> Rems {
        rems_from_px(self.ui_font_size)
    }

    pub fn ui_large(self) -> Rems {
        rems_from_px(self.ui_font_size + 2.0)
    }

    pub fn ui_small(self) -> Rems {
        rems_from_px((self.ui_font_size - 2.0).max(8.0))
    }

    pub fn ui_xsmall(self) -> Rems {
        rems_from_px((self.ui_font_size - 2.0).max(8.0))
    }

    pub fn buffer(self) -> Rems {
        rems_from_px(self.buffer_font_size)
    }
}

pub trait AppTypography: Styled + Sized {
    fn text_ui(self, typography: Typography) -> Self {
        self.font_family(JETBRAINS_FONT_FAMILY)
            .text_size(typography.ui_default())
    }

    fn text_ui_lg(self, typography: Typography) -> Self {
        self.font_family(JETBRAINS_FONT_FAMILY)
            .text_size(typography.ui_large())
    }

    fn text_ui_sm(self, typography: Typography) -> Self {
        self.font_family(JETBRAINS_FONT_FAMILY)
            .text_size(typography.ui_small())
    }

    fn text_ui_xs(self, typography: Typography) -> Self {
        self.font_family(JETBRAINS_FONT_FAMILY)
            .text_size(typography.ui_xsmall())
    }

    fn text_buffer(self, typography: Typography) -> Self {
        self.font_family(JETBRAINS_FONT_FAMILY)
            .text_size(typography.buffer())
    }
}

impl<T: Styled> AppTypography for T {}

fn rems_from_px(px: f32) -> Rems {
    rems(px / BASE_REM_SIZE_IN_PX)
}
