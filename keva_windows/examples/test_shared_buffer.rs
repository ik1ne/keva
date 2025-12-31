//! Test SharedBuffer API for zero-copy large data transfer.
//!
//! Run: cargo run -q --example test_shared_buffer

use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::OnceLock;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_WRITE, CreateCoreWebView2Environment, ICoreWebView2,
    ICoreWebView2_17, ICoreWebView2Controller, ICoreWebView2Environment,
    ICoreWebView2Environment12, ICoreWebView2WebMessageReceivedEventArgs,
};
use webview2_com::{
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    WebMessageReceivedEventHandler, pwstr_from_str,
};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, IDC_ARROW, LoadCursorW, MSG,
    PostQuitMessage, RegisterClassW, SW_SHOW, ShowWindow, WM_CLOSE, WM_DESTROY, WNDCLASSW,
    WS_OVERLAPPEDWINDOW,
};
use windows::core::{Interface, PWSTR, w};

static WEBVIEW: OnceLock<WebViewState> = OnceLock::new();

struct WebViewState {
    #[expect(dead_code)]
    controller: ICoreWebView2Controller,
    #[expect(dead_code)]
    webview: ICoreWebView2,
    #[expect(dead_code)]
    env: ICoreWebView2Environment,
}

unsafe impl Send for WebViewState {}
unsafe impl Sync for WebViewState {}

const HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <style>
        body { font-family: monospace; background: #1e1e1e; color: #d4d4d4; padding: 20px; }
        pre { white-space: pre-wrap; }
        button { padding: 10px 20px; margin: 10px 0; }
        #status { color: #4ec9b0; }
        #error { color: #f44747; }
    </style>
</head>
<body>
    <h2>SharedBuffer API Test</h2>
    <button onclick="requestBuffer()">Request SharedBuffer from Native</button>
    <pre id="status"></pre>
    <pre id="error"></pre>
    <pre id="content"></pre>
    <script>
        function log(msg) {
            document.getElementById('status').textContent += msg + '\n';
        }
        function err(msg) {
            document.getElementById('error').textContent += 'ERROR: ' + msg + '\n';
        }

        function requestBuffer() {
            log('Requesting SharedBuffer...');
            window.chrome.webview.postMessage(JSON.stringify({ type: 'requestBuffer' }));
        }

        window.chrome.webview.addEventListener('sharedbufferreceived', (e) => {
            log('SharedBuffer received!');
            try {
                const buffer = e.getBuffer();
                log('Buffer size: ' + buffer.byteLength + ' bytes');

                const textDecoder = new TextDecoder();
                const content = textDecoder.decode(new Uint8Array(buffer));
                log('Content preview (first 500 chars):');
                document.getElementById('content').textContent = content.substring(0, 500);

                // Modify buffer to test write-back
                const view = new Uint8Array(buffer);
                view[0] = 0x4D; // 'M'
                view[1] = 0x4F; // 'O'
                view[2] = 0x44; // 'D'
                log('Modified first 3 bytes to "MOD"');

                // Release buffer
                chrome.webview.releaseBuffer(buffer);
                log('Buffer released');
            } catch (ex) {
                err(ex.toString());
            }
        });

        window.chrome.webview.addEventListener('message', (e) => {
            const msg = e.data;
            if (msg.type === 'error') {
                err(msg.message);
            } else if (msg.type === 'info') {
                log(msg.message);
            }
        });

        log('Page loaded. Click button to test SharedBuffer.');
    </script>
</body>
</html>"#;

fn main() {
    // Initialize COM
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    unsafe {
        let instance = GetModuleHandleW(None).unwrap();
        let class_name = w!("SharedBufferTest");

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            lpszClassName: class_name,
            ..Default::default()
        };

        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            w!("SharedBuffer Test"),
            WS_OVERLAPPEDWINDOW,
            100,
            100,
            800,
            600,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .unwrap();

        init_webview(hwnd);

        let _ = ShowWindow(hwnd, SW_SHOW);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            DispatchMessageW(&msg);
        }
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CLOSE => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn init_webview(hwnd: HWND) {
    unsafe {
        let _ = CreateCoreWebView2Environment(
            &CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(move |_err, env| {
                let Some(env) = env else {
                    eprintln!("Failed to create environment");
                    return Ok(());
                };
                create_controller(hwnd, env);
                Ok(())
            })),
        );
    }
}

fn create_controller(hwnd: HWND, env: ICoreWebView2Environment) {
    unsafe {
        let env_clone = env.clone();
        let _ = env.CreateCoreWebView2Controller(
            hwnd,
            &CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
                move |_err, controller| {
                    let Some(controller) = controller else {
                        eprintln!("Failed to create controller");
                        return Ok(());
                    };

                    let webview = controller.CoreWebView2().unwrap();

                    // Set bounds
                    let mut rect = RECT::default();
                    windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect).ok();
                    controller.SetBounds(rect).ok();
                    controller.SetIsVisible(true).ok();

                    // Add message handler
                    let webview_for_handler = webview.clone();
                    let env_for_handler = env_clone.clone();
                    let mut token = 0i64;
                    webview
                        .add_WebMessageReceived(
                            &WebMessageReceivedEventHandler::create(
                                Box::new(
                                    move |_wv,
                                          args: Option<
                                        ICoreWebView2WebMessageReceivedEventArgs,
                                    >| {
                                        handle_message(
                                            &webview_for_handler,
                                            &env_for_handler,
                                            args,
                                        );
                                        Ok(())
                                    },
                                ),
                            ),
                            &mut token,
                        )
                        .ok();

                    // Navigate to HTML
                    let html_pwstr = pwstr_from_str(HTML);
                    webview.NavigateToString(html_pwstr).ok();

                    WEBVIEW
                        .set(WebViewState {
                            controller,
                            webview,
                            env: env_clone,
                        })
                        .ok();

                    Ok(())
                },
            )),
        );
    }
}

fn handle_message(
    webview: &ICoreWebView2,
    env: &ICoreWebView2Environment,
    args: Option<ICoreWebView2WebMessageReceivedEventArgs>,
) {
    let Some(args) = args else { return };

    unsafe {
        let mut message = PWSTR::null();
        if args.TryGetWebMessageAsString(&mut message).is_err() || message.is_null() {
            return;
        }

        let msg_str = message.to_string().unwrap_or_default();
        CoTaskMemFree(Some(message.as_ptr() as *const c_void));

        eprintln!("[Native] Received: {}", msg_str);

        if msg_str.contains("requestBuffer") {
            send_shared_buffer(webview, env);
        }
    }
}

fn send_shared_buffer(webview: &ICoreWebView2, env: &ICoreWebView2Environment) {
    unsafe {
        // Try to get ICoreWebView2Environment12 for CreateSharedBuffer
        let Ok(env12) = env.cast::<ICoreWebView2Environment12>() else {
            send_error(
                webview,
                "ICoreWebView2Environment12 not available (need SDK 1.0.1661+)",
            );
            return;
        };

        // Try to get ICoreWebView2_17 for PostSharedBufferToScript
        let Ok(webview17) = webview.cast::<ICoreWebView2_17>() else {
            send_error(
                webview,
                "ICoreWebView2_17 not available (need SDK 1.0.1661+)",
            );
            return;
        };

        // Create test data (10MB)
        let test_data = "Hello from SharedBuffer! ".repeat(400_000); // ~10MB
        let data_bytes = test_data.as_bytes();

        eprintln!(
            "[Native] Creating SharedBuffer of {} bytes",
            data_bytes.len()
        );

        // Create shared buffer (returns Result<ICoreWebView2SharedBuffer>)
        let buffer = match env12.CreateSharedBuffer(data_bytes.len() as u64) {
            Ok(b) => b,
            Err(e) => {
                send_error(webview, &format!("CreateSharedBuffer failed: {:?}", e));
                return;
            }
        };

        // Get buffer pointer and copy data
        let mut buffer_ptr: *mut u8 = null_mut();
        if let Err(e) = buffer.Buffer(&mut buffer_ptr) {
            send_error(webview, &format!("Buffer() failed: {:?}", e));
            return;
        }

        std::ptr::copy_nonoverlapping(data_bytes.as_ptr(), buffer_ptr, data_bytes.len());
        eprintln!("[Native] Data copied to SharedBuffer");

        // Post to JavaScript
        let additional_data = pwstr_from_str(r#"{"type":"bufferData"}"#);
        if let Err(e) = webview17.PostSharedBufferToScript(
            &buffer,
            COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_WRITE,
            additional_data,
        ) {
            send_error(
                webview,
                &format!("PostSharedBufferToScript failed: {:?}", e),
            );
            return;
        }

        eprintln!("[Native] SharedBuffer posted to script");
        send_info(webview, "SharedBuffer sent successfully!");
    }
}

fn send_error(webview: &ICoreWebView2, message: &str) {
    eprintln!("[Native] Error: {}", message);
    let json = format!(
        r#"{{"type":"error","message":"{}"}}"#,
        message.replace('"', "\\\"")
    );
    unsafe {
        let _ = webview.PostWebMessageAsJson(pwstr_from_str(&json));
    }
}

fn send_info(webview: &ICoreWebView2, message: &str) {
    let json = format!(
        r#"{{"type":"info","message":"{}"}}"#,
        message.replace('"', "\\\"")
    );
    unsafe {
        let _ = webview.PostWebMessageAsJson(pwstr_from_str(&json));
    }
}
