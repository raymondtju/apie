use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum UiDensity {
    Compact,
    Default,
    Comfortable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Spacing {
    density: UiDensity,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            density: UiDensity::Compact,
        }
    }
}

#[allow(dead_code)]
impl Spacing {
    #[allow(dead_code)]
    pub fn new(density: UiDensity) -> Self {
        Self { density }
    }

    pub fn app() -> Self {
        Self::default()
    }

    fn dynamic(self, compact: f32, default: f32, comfortable: f32) -> Pixels {
        match self.density {
            UiDensity::Compact => px(compact),
            UiDensity::Default => px(default),
            UiDensity::Comfortable => px(comfortable),
        }
    }

    pub fn base04(self) -> Pixels {
        self.dynamic(2.0, 2.0, 4.0)
    }

    pub fn base06(self) -> Pixels {
        self.dynamic(3.0, 3.0, 4.0)
    }

    pub fn base08(self) -> Pixels {
        self.dynamic(4.0, 4.0, 6.0)
    }

    pub fn base12(self) -> Pixels {
        self.dynamic(5.0, 6.0, 8.0)
    }

    pub fn base16(self) -> Pixels {
        self.dynamic(6.0, 8.0, 10.0)
    }

    pub fn base24(self) -> Pixels {
        self.dynamic(10.0, 12.0, 14.0)
    }

    pub fn base32(self) -> Pixels {
        self.dynamic(14.0, 16.0, 18.0)
    }

    pub fn fixed24(self) -> Pixels {
        let _ = self;
        px(24.0)
    }

    pub fn fixed32(self) -> Pixels {
        let _ = self;
        px(32.0)
    }

    pub fn control_gap(self) -> Pixels {
        self.base08()
    }

    pub fn cluster_gap(self) -> Pixels {
        self.base12()
    }

    pub fn component_gap(self) -> Pixels {
        self.base24()
    }

    pub fn section_gap(self) -> Pixels {
        self.base32()
    }
}
