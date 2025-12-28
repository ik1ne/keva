//! WebView2 initialization and management.

use std::ffi::c_void;
use std::time::SystemTime;

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

use crate::storage;

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
    let Some(wv) = webview else { return };
    let Some(msg_type) = parse_message_type(msg) else { return };

    match msg_type {
        "ready" => {
            eprintln!("[Native] Received 'ready' from WebView");
            send_init(wv);
            send_keys(wv);
        }
        "select" => {
            if let Some(key) = parse_message_key(msg) {
                eprintln!("[Native] Selected key: {}", key);
                send_value(wv, key);
            }
        }
        other => {
            eprintln!("[Native] Received message type: {}", other);
        }
    }
}

fn send_init(wv: &ICoreWebView2) {
    let timestamp = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let response = format!(r#"{{"type":"init","timestamp":{}}}"#, timestamp);
    post_message(wv, &response);
    eprintln!("[Native] Sent 'init' to WebView");
}

fn send_keys(wv: &ICoreWebView2) {
    let keys = storage::with_keva(|keva| {
        let active = keva.active_keys().unwrap_or_default();
        let trashed = keva.trashed_keys().unwrap_or_default();
        (active, trashed)
    });

    let active_json: Vec<String> = keys
        .0
        .iter()
        .map(|k| format!(r#"{{"name":"{}","trashed":false}}"#, escape_json(k.as_str())))
        .collect();
    let trashed_json: Vec<String> = keys
        .1
        .iter()
        .map(|k| format!(r#"{{"name":"{}","trashed":true}}"#, escape_json(k.as_str())))
        .collect();

    let all_keys: Vec<String> = active_json.into_iter().chain(trashed_json).collect();
    let response = format!(r#"{{"type":"keys","keys":[{}]}}"#, all_keys.join(","));
    post_message(wv, &response);
    eprintln!("[Native] Sent {} keys to WebView", keys.0.len() + keys.1.len());
}

fn send_value(wv: &ICoreWebView2, key_str: &str) {
    use keva_core::types::{ClipData, Key, TextContent};

    let now = SystemTime::now();
    let result = storage::with_keva(|keva| {
        let key = Key::try_from(key_str).ok()?;
        // Touch the key to update last_accessed
        let _ = keva.touch(&key, now);
        keva.get(&key).ok().flatten()
    });

    let response = match result {
        Some(value) => match &value.clip_data {
            ClipData::Text(TextContent::Inlined(s)) => {
                format!(
                    r#"{{"type":"value","value":{{"type":"text","content":"{}"}}}}"#,
                    escape_json(s)
                )
            }
            ClipData::Text(TextContent::BlobStored { path }) => {
                let content = std::fs::read_to_string(path).unwrap_or_default();
                format!(
                    r#"{{"type":"value","value":{{"type":"text","content":"{}"}}}}"#,
                    escape_json(&content)
                )
            }
            ClipData::Files(files) => {
                format!(
                    r#"{{"type":"value","value":{{"type":"files","count":{}}}}}"#,
                    files.len()
                )
            }
        },
        None => r#"{"type":"value","value":null}"#.to_string(),
    };
    post_message(wv, &response);
}

fn post_message(wv: &ICoreWebView2, json: &str) {
    let msg = pwstr_from_str(json);
    let _ = unsafe { wv.PostWebMessageAsJson(msg) };
}

/// Simple JSON parser to extract message type.
fn parse_message_type(json: &str) -> Option<&str> {
    parse_json_string_field(json, "type")
}

/// Simple JSON parser to extract key field.
fn parse_message_key(json: &str) -> Option<&str> {
    parse_json_string_field(json, "key")
}

fn parse_json_string_field<'a>(json: &'a str, field: &str) -> Option<&'a str> {
    let pattern = format!(r#""{}":""#, field);
    let field_start = json.find(&pattern)?;
    let value_start = field_start + pattern.len();
    let remaining = &json[value_start..];
    let value_end = remaining.find('"')?;
    Some(&remaining[..value_end])
}

fn escape_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str(r#"\""#),
            '\\' => result.push_str(r"\\"),
            '\n' => result.push_str(r"\n"),
            '\r' => result.push_str(r"\r"),
            '\t' => result.push_str(r"\t"),
            c if c.is_control() => {
                result.push_str(&format!(r"\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

fn pwstr_to_string(pwstr: PWSTR) -> String {
    if pwstr.is_null() {
        return String::new();
    }
    unsafe { pwstr.to_string().unwrap_or_default() }
}
