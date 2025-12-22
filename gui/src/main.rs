mod app;
mod theme;

use app::KevaApp;
use gpui::{px, size, App, AppContext, Application, Bounds, WindowBounds, WindowOptions};
use gpui_component::Root;
use theme::window_options;

fn main() {
    Application::new().run(|cx: &mut App| {
        gpui_component::init(cx);

        // Global keystroke interceptor (fires before other handlers)
        // Leak subscription to keep it alive for app lifetime
        std::mem::forget(cx.intercept_keystrokes(|event, window, _cx| {
            if event.keystroke.key.as_str() == "escape" {
                window.minimize_window();
            }
        }));

        let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..window_options()
        };

        cx.open_window(options, |window, cx| {
            let app_view = cx.new(|cx| KevaApp::new(window, cx));
            cx.new(|cx| Root::new(app_view, window, cx))
        })
        .unwrap();
    });
}
