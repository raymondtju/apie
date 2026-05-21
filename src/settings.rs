use serde::{Deserialize, Serialize};
use std::{
    env, fmt, fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
};

pub const DEFAULT_UI_FONT_SIZE: f32 = 14.0;
pub const DEFAULT_BUFFER_FONT_SIZE: f32 = 14.0;
pub const MIN_UI_FONT_SIZE: f32 = 10.0;
pub const MAX_UI_FONT_SIZE: f32 = 24.0;
pub const MIN_BUFFER_FONT_SIZE: f32 = 10.0;
pub const MAX_BUFFER_FONT_SIZE: f32 = 32.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ThemePreference {
    System,
    ZedDark,
    ZedLight,
}

impl Default for ThemePreference {
    fn default() -> Self {
        Self::System
    }
}

impl ThemePreference {
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::ZedDark => "Zed Dark",
            Self::ZedLight => "Zed Light",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::System => Self::ZedDark,
            Self::ZedDark => Self::ZedLight,
            Self::ZedLight => Self::System,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub ui_font_size: f32,
    pub buffer_font_size: f32,
    #[serde(default)]
    pub theme_preference: ThemePreference,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            ui_font_size: DEFAULT_UI_FONT_SIZE,
            buffer_font_size: DEFAULT_BUFFER_FONT_SIZE,
            theme_preference: ThemePreference::System,
        }
    }
}

impl AppSettings {
    pub fn clamped(mut self) -> Self {
        self.ui_font_size = clamp_ui_font_size(self.ui_font_size);
        self.buffer_font_size = clamp_buffer_font_size(self.buffer_font_size);
        self
    }

    pub fn adjust_ui_font_size(&mut self, delta: f32) {
        self.ui_font_size = clamp_ui_font_size(self.ui_font_size + delta);
    }

    pub fn adjust_buffer_font_size(&mut self, delta: f32) {
        self.buffer_font_size = clamp_buffer_font_size(self.buffer_font_size + delta);
    }
}

#[derive(Debug)]
pub enum SettingsError {
    Io(io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "settings I/O error: {error}"),
            Self::Json(error) => write!(f, "settings JSON error: {error}"),
        }
    }
}

impl std::error::Error for SettingsError {}

impl From<io::Error> for SettingsError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for SettingsError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub type SettingsResult<T> = Result<T, SettingsError>;

pub fn clamp_ui_font_size(value: f32) -> f32 {
    value.clamp(MIN_UI_FONT_SIZE, MAX_UI_FONT_SIZE)
}

pub fn clamp_buffer_font_size(value: f32) -> f32 {
    value.clamp(MIN_BUFFER_FONT_SIZE, MAX_BUFFER_FONT_SIZE)
}

pub fn default_app_settings_path() -> PathBuf {
    let base_dir = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));

    base_dir.join("gpui-api-client").join("settings.json")
}

pub fn load_app_settings(path: impl AsRef<Path>) -> SettingsResult<AppSettings> {
    let path = path.as_ref();
    let json = match fs::read_to_string(path) {
        Ok(json) => json,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(AppSettings::default()),
        Err(error) => return Err(SettingsError::Io(error)),
    };

    Ok(serde_json::from_str::<AppSettings>(&json)
        .map(AppSettings::clamped)
        .unwrap_or_default())
}

pub fn save_app_settings(path: impl AsRef<Path>, settings: &AppSettings) -> SettingsResult<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&settings.clamped())?;
    fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_defaults_when_settings_file_is_missing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("missing-settings.json");

        assert_eq!(load_app_settings(path).unwrap(), AppSettings::default());
    }

    #[test]
    fn saves_and_loads_settings_round_trip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("nested/settings.json");
        let settings = AppSettings {
            ui_font_size: 16.0,
            buffer_font_size: 18.0,
            theme_preference: ThemePreference::ZedDark,
        };

        save_app_settings(&path, &settings).unwrap();

        assert_eq!(load_app_settings(path).unwrap(), settings);
    }

    #[test]
    fn invalid_json_falls_back_to_defaults() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("settings.json");
        fs::write(&path, "{not-json").unwrap();

        assert_eq!(load_app_settings(path).unwrap(), AppSettings::default());
    }

    #[test]
    fn clamps_loaded_and_adjusted_font_sizes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("settings.json");
        fs::write(&path, r#"{"ui_font_size": 1.0, "buffer_font_size": 100.0}"#).unwrap();

        assert_eq!(
            load_app_settings(&path).unwrap(),
            AppSettings {
                ui_font_size: MIN_UI_FONT_SIZE,
                buffer_font_size: MAX_BUFFER_FONT_SIZE,
                theme_preference: ThemePreference::System,
            }
        );

        let mut settings = AppSettings::default();
        settings.adjust_ui_font_size(100.0);
        settings.adjust_buffer_font_size(-100.0);

        assert_eq!(settings.ui_font_size, MAX_UI_FONT_SIZE);
        assert_eq!(settings.buffer_font_size, MIN_BUFFER_FONT_SIZE);
    }
}
