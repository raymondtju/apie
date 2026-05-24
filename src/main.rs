mod app;
mod settings;
mod ui;

use std::borrow::Cow;

use app::ApiClientApp;
use gpui::{
    App, AppContext, Application, AssetSource, Bounds, Result, SharedString, TitlebarOptions,
    WindowBackgroundAppearance, WindowBounds, WindowDecorations, WindowOptions, px, size,
};
use ui::{bind_code_input_keys, bind_text_input_keys, icon_bytes_for_path, ICON_FILENAMES};

/// Embedded asset source. All UI icons are compiled into the binary via include_bytes!
/// (see IconName::bytes and helpers in src/ui/icons.rs). This eliminates the previous
/// fragility where a stale target/ binary baked an old CARGO_MANIFEST_DIR and caused
/// silent icon load failures on `cargo run` after project moves or across machines.
struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = icon_bytes_for_path(path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        Ok(None)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        if path == "icons" || path == "icons/" || path == "icons\\" {
            return Ok(ICON_FILENAMES
                .iter()
                .map(|s| SharedString::from(*s))
                .collect());
        }
        Ok(vec![])
    }
}

fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls CryptoProvider");
    Application::new()
        .with_assets(AppAssets)
        .run(|cx: &mut App| {
            bind_text_input_keys(cx);
            bind_code_input_keys(cx);
            app::bind_app_keys(cx);
            let bounds = Bounds::centered(None, size(px(1280.), px(820.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: None,
                        appears_transparent: true,
                        traffic_light_position: None,
                    }),
                    focus: true,
                    window_background: WindowBackgroundAppearance::Transparent,
                    window_decorations: Some(WindowDecorations::Client),
                    window_min_size: Some(size(px(900.0), px(620.0))),
                    ..Default::default()
                },
                |_, cx| cx.new(|cx| ApiClientApp::new(cx)),
            )
            .unwrap();
            cx.activate(true);
        });
}
