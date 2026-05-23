mod app;
mod settings;
mod ui;

use std::{borrow::Cow, fs, path::PathBuf};

use app::ApiClientApp;
use gpui::{
    App, AppContext, Application, AssetSource, Bounds, Result, SharedString, TitlebarOptions,
    WindowBackgroundAppearance, WindowBounds, WindowDecorations, WindowOptions, px, size,
};
use ui::{bind_code_input_keys, bind_text_input_keys};

struct AppAssets {
    base: PathBuf,
}

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        fs::read(self.base.join(path))
            .map(|data| Some(Cow::Owned(data)))
            .map_err(Into::into)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        fs::read_dir(self.base.join(path))
            .map(|entries| {
                entries
                    .filter_map(|entry| {
                        entry
                            .ok()
                            .and_then(|entry| entry.file_name().into_string().ok())
                            .map(SharedString::from)
                    })
                    .collect()
            })
            .map_err(Into::into)
    }
}

fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls CryptoProvider");
    Application::new()
        .with_assets(AppAssets {
            base: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
        })
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
