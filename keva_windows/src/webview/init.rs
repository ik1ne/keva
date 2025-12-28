//! WebView2 initialization.

use super::bridge::handle_webview_message;
use super::{WEBVIEW, WebView};
use crate::keva_worker::Request;
use std::ffi::c_void;
use std::sync::mpsc::Sender;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_COLOR, CreateCoreWebView2Environment, ICoreWebView2, ICoreWebView2Controller,
    ICoreWebView2Controller2, ICoreWebView2Environment, ICoreWebView2Settings9,
    ICoreWebView2WebMessageReceivedEventArgs,
};
use webview2_com::{
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    WebMessageReceivedEventHandler,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::core::{Interface, PWSTR};

/// Initializes a WebView2 at the specified position.
/// WebView2 creation is async because it may need to download the runtime.
pub fn init_webview(
    hwnd: HWND,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    request_tx: Sender<Request>,
    on_ready: impl FnOnce(&WebView) + 'static,
) {
    unsafe {
        let _ = CreateCoreWebView2Environment(
            &CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(move |_error, env| {
                let Some(env) = env else { return Ok(()) };
                create_controller(hwnd, x, y, width, height, env, request_tx, on_ready);
                Ok(())
            })),
        );
    }
}

#[expect(clippy::too_many_arguments)]
fn create_controller(
    hwnd: HWND,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    env: ICoreWebView2Environment,
    request_tx: Sender<Request>,
    on_ready: impl FnOnce(&WebView) + 'static,
) {
    unsafe {
        let _ = env.CreateCoreWebView2Controller(
            hwnd,
            &CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
                move |_error, controller| {
                    let Some(controller) = controller else {
                        return Ok(());
                    };
                    let Some(webview) = setup_webview(controller.clone(), hwnd, request_tx) else {
                        return Ok(());
                    };

                    // Set dark background color to prevent white flash during resize
                    // #1a1a1a = RGB(26, 26, 26)
                    if let Ok(controller2) = controller.cast::<ICoreWebView2Controller2>() {
                        let dark_bg = COREWEBVIEW2_COLOR {
                            A: 255,
                            R: 26,
                            G: 26,
                            B: 26,
                        };
                        let _ = controller2.SetDefaultBackgroundColor(dark_bg);
                    }

                    // Ensure WebView is visible
                    let _ = controller.SetIsVisible(true);

                    let wv = WebView {
                        controller,
                        webview,
                    };
                    wv.set_bounds(x, y, width, height);
                    on_ready(&wv);
                    WEBVIEW
                        .set(wv)
                        .unwrap_or_else(|_| panic!("Failed to set webview"));
                    Ok(())
                },
            )),
        );
    }
}

fn setup_webview(
    controller: ICoreWebView2Controller,
    parent_hwnd: HWND,
    request_tx: Sender<Request>,
) -> Option<ICoreWebView2> {
    unsafe {
        let webview = controller.CoreWebView2().ok()?;

        // Enable CSS app-region: drag support for window dragging
        if let Ok(settings) = webview.Settings()
            && let Ok(settings9) = settings.cast::<ICoreWebView2Settings9>()
        {
            let _ = settings9.SetIsNonClientRegionSupportEnabled(true);
        }

        let mut token = 0i64;
        let _ = webview.add_WebMessageReceived(
            &WebMessageReceivedEventHandler::create(Box::new(
                move |_webview_opt, args: Option<ICoreWebView2WebMessageReceivedEventArgs>| {
                    let Some(args) = args else { return Ok(()) };
                    let mut message = PWSTR::null();
                    if args.TryGetWebMessageAsString(&mut message).is_err() || message.is_null() {
                        return Ok(());
                    }

                    let msg_str = super::bridge::pwstr_to_string(message);
                    CoTaskMemFree(Some(message.as_ptr() as *const c_void));
                    handle_webview_message(&msg_str, parent_hwnd, &request_tx);
                    Ok(())
                },
            )),
            &mut token,
        );

        Some(webview)
    }
}
