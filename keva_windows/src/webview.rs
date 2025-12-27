//! WebView2 initialization and management.

use std::ffi::c_void;

use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_COLOR, CreateCoreWebView2Environment, ICoreWebView2, ICoreWebView2Controller,
    ICoreWebView2Controller2, ICoreWebView2Environment, ICoreWebView2Settings9,
    ICoreWebView2WebMessageReceivedEventArgs,
};
use webview2_com::{
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    WebMessageReceivedEventHandler, pwstr_from_str,
};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::core::{Interface, PWSTR};
use windows_strings::PCWSTR;

pub struct WebView {
    controller: ICoreWebView2Controller,
    webview: ICoreWebView2,
    parent_hwnd: HWND,
}

unsafe impl Send for WebView {}
unsafe impl Sync for WebView {}

impl WebView {
    /// Sets the bounds of the WebView within its parent window.
    pub fn set_bounds(&self, x: i32, y: i32, width: i32, height: i32) {
        unsafe {
            let rect = RECT {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            };
            let _ = self.controller.SetBounds(rect);
        }
    }

    pub fn navigate_html(&self, html: PCWSTR) {
        unsafe {
            let _ = self.webview.NavigateToString(html);
        }
    }

    #[expect(dead_code)]
    pub fn post_message(&self, json: &str) {
        unsafe {
            let msg = pwstr_from_str(json);
            let _ = self.webview.PostWebMessageAsJson(msg);
        }
    }

    #[expect(dead_code)]
    pub fn hwnd(&self) -> HWND {
        self.parent_hwnd
    }
}


/// Initializes a WebView2 at the specified position.
/// WebView2 creation is async because it may need to download the runtime.
pub fn init_webview(
    hwnd: HWND,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    on_ready: impl FnOnce(WebView) + 'static,
) {
    unsafe {
        let _ = CreateCoreWebView2Environment(
            &CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(move |_error, env| {
                if let Some(env) = env {
                    create_controller(hwnd, x, y, width, height, env, on_ready);
                }
                Ok(())
            })),
        );
    }
}

fn create_controller(
    hwnd: HWND,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    env: ICoreWebView2Environment,
    on_ready: impl FnOnce(WebView) + 'static,
) {
    unsafe {
        let _ = env.CreateCoreWebView2Controller(
            hwnd,
            &CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
                move |_error, controller| {
                    if let Some(controller) = controller
                        && let Some(webview) = setup_webview(controller.clone())
                    {
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
                            parent_hwnd: hwnd,
                        };
                        wv.set_bounds(x, y, width, height);
                        on_ready(wv);
                    }
                    Ok(())
                },
            )),
        );
    }
}

fn setup_webview(controller: ICoreWebView2Controller) -> Option<ICoreWebView2> {
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
                |webview_opt, args: Option<ICoreWebView2WebMessageReceivedEventArgs>| {
                    if let Some(args) = args {
                        let mut message = PWSTR::null();
                        if args.TryGetWebMessageAsString(&mut message).is_ok() && !message.is_null()
                        {
                            let msg_str = pwstr_to_string(message);
                            CoTaskMemFree(Some(message.as_ptr() as *const c_void));

                            // Handle incoming messages
                            handle_webview_message(webview_opt.as_ref(), &msg_str);
                        }
                    }
                    Ok(())
                },
            )),
            &mut token,
        );

        Some(webview)
    }
}

/// Handles messages from WebView and sends responses.
fn handle_webview_message(webview: Option<&ICoreWebView2>, msg: &str) {
    // Parse JSON message
    if let Some(msg_type) = parse_message_type(msg) {
        match msg_type {
            "ready" => {
                eprintln!("[Native] Received 'ready' from WebView");
                // Respond with init message
                if let Some(wv) = webview {
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis())
                        .unwrap_or(0);
                    let response = format!(r#"{{"type":"init","timestamp":{}}}"#, timestamp);
                    let response_pwstr = pwstr_from_str(&response);
                    let _ = unsafe { wv.PostWebMessageAsJson(response_pwstr) };
                    eprintln!("[Native] Sent 'init' to WebView");
                }
            }
            other => {
                eprintln!("[Native] Received message type: {}", other);
            }
        }
    }
}

/// Simple JSON parser to extract message type.
fn parse_message_type(json: &str) -> Option<&str> {
    // Look for "type":"<value>" pattern
    let type_start = json.find(r#""type":""#)?;
    let value_start = type_start + 8; // length of "type":"
    let remaining = &json[value_start..];
    let value_end = remaining.find('"')?;
    Some(&remaining[..value_end])
}

fn pwstr_to_string(pwstr: PWSTR) -> String {
    if pwstr.is_null() {
        return String::new();
    }
    unsafe { pwstr.to_string().unwrap_or_default() }
}
