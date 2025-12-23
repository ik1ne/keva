mod app;
mod theme;

use app::KevaApp;
use gpui::{
    App, AppContext, Application, Bounds, KeyBinding, WindowBounds, WindowOptions, actions, px,
    size,
};
use gpui_component::Root;
use theme::window_options;

actions!(keva, [Quit]);

fn main() {
    Application::new().run(|cx: &mut App| {
        gpui_component::init(cx);

        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);

        // TODO(M7): minimize_window() doesn't work on macOS for borderless windows (no NSMiniaturizableWindowMask). Replace with proper hide/show via tray.
        let _subscription = cx.intercept_keystrokes(|event, window, _cx| {
            if event.keystroke.key.as_str() == "escape" {
                window.minimize_window();
            }
        });

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
