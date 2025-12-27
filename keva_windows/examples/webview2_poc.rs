//! WebView2 + Monaco Editor Proof of Concept
//!
//! This standalone example validates the feasibility of using WebView2 with Monaco
//! for the Keva right pane editor. It demonstrates:
//!
//! 1. WebView2 initialization in a Win32 window
//! 2. Monaco editor loading from CDN
//! 3. Bidirectional messaging (native ↔ JS)
//! 4. View switching (editor ↔ file list)
//! 5. Flicker-free resize behavior
//!
//! Run with: cargo run --example webview2_poc

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender};

use once_cell::sync::Lazy;

// WebView2 COM types
use webview2_com::Microsoft::Web::WebView2::Win32::{
    CreateCoreWebView2Environment, ICoreWebView2, ICoreWebView2Controller,
    ICoreWebView2Environment, ICoreWebView2WebMessageReceivedEventArgs,
};
use webview2_com::{
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    WebMessageReceivedEventHandler, pwstr_from_str,
};

/// Window dimensions
const WINDOW_WIDTH: i32 = 1000;
const WINDOW_HEIGHT: i32 = 700;
const BUTTON_HEIGHT: i32 = 50;

/// Button IDs
const BTN_SET_CONTENT: u16 = 101;
const BTN_GET_CONTENT: u16 = 102;
const BTN_SHOW_EDITOR: u16 = 103;
const BTN_SHOW_FILES: u16 = 104;

/// Window class name
const CLASS_NAME: &str = "WebView2POC\0";

/// Global channel for WebView messages
static MESSAGE_CHANNEL: Lazy<Mutex<Option<Sender<String>>>> = Lazy::new(|| Mutex::new(None));

/// Global app state (needed for wndproc callback)
static mut APP_HWND: isize = 0;
static mut WEBVIEW: Option<WebView> = None;
static mut REQUEST_COUNTER: u32 = 0;

/// Monaco HTML with CDN loading
const MONACO_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        html, body { height: 100%; overflow: hidden; background: #1e1e1e; }
        #container { width: 100%; height: 100%; }
        #file-list {
            display: none;
            padding: 20px;
            color: #d4d4d4;
            font-family: 'Segoe UI', sans-serif;
        }
        #file-list.active { display: block; }
        #editor-container { width: 100%; height: 100%; }
        #editor-container.hidden { display: none; }
        .file-item {
            padding: 10px 15px;
            margin: 5px 0;
            background: #2d2d2d;
            border-radius: 4px;
            cursor: pointer;
        }
        .file-item:hover { background: #3d3d3d; }
        .file-name { font-weight: 500; }
        .file-size { color: #808080; font-size: 0.9em; }
    </style>
</head>
<body>
    <div id="container">
        <div id="editor-container"></div>
        <div id="file-list">
            <h3 style="margin-bottom: 15px;">Files</h3>
            <div class="file-item">
                <span class="file-name">document.pdf</span>
                <span class="file-size">1.2 MB</span>
            </div>
            <div class="file-item">
                <span class="file-name">image.png</span>
                <span class="file-size">340 KB</span>
            </div>
            <div class="file-item">
                <span class="file-name">data.csv</span>
                <span class="file-size">12 KB</span>
            </div>
        </div>
    </div>

    <script src="https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.45.0/min/vs/loader.min.js"></script>
    <script>
        let editor = null;
        let currentView = 'editor';

        // Initialize Monaco
        require.config({ paths: { 'vs': 'https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.45.0/min/vs' }});
        require(['vs/editor/editor.main'], function() {
            editor = monaco.editor.create(document.getElementById('editor-container'), {
                value: '// Welcome to Keva Monaco Editor POC\n// Type here to test...\n',
                language: 'plaintext',
                theme: 'vs-dark',
                automaticLayout: true,
                minimap: { enabled: false },
                fontSize: 14,
                lineNumbers: 'on',
                wordWrap: 'on',
                scrollBeyondLastLine: false
            });

            // Notify native that Monaco is ready
            window.chrome.webview.postMessage(JSON.stringify({
                type: 'ready'
            }));

            // Send content changes to native (debounced)
            let debounceTimer = null;
            editor.onDidChangeModelContent(() => {
                clearTimeout(debounceTimer);
                debounceTimer = setTimeout(() => {
                    window.chrome.webview.postMessage(JSON.stringify({
                        type: 'contentChanged',
                        content: editor.getValue()
                    }));
                }, 300);
            });
        });

        // Handle messages from native
        // PostWebMessageAsJson sends parsed objects, not strings
        window.chrome.webview.addEventListener('message', event => {
            const msg = event.data;

            switch (msg.type) {
                case 'setContent':
                    if (editor) {
                        editor.setValue(msg.content);
                    }
                    break;

                case 'getContent':
                    if (editor) {
                        window.chrome.webview.postMessage(JSON.stringify({
                            type: 'contentResponse',
                            requestId: msg.requestId,
                            content: editor.getValue()
                        }));
                    }
                    break;

                case 'showEditor':
                    currentView = 'editor';
                    document.getElementById('editor-container').classList.remove('hidden');
                    document.getElementById('file-list').classList.remove('active');
                    if (editor) editor.layout();
                    break;

                case 'showFiles':
                    currentView = 'files';
                    document.getElementById('editor-container').classList.add('hidden');
                    document.getElementById('file-list').classList.add('active');
                    break;
            }
        });
    </script>
</body>
</html>"#;

/// RECT for WebView2 (matches windows 0.61 layout)
#[repr(C)]
#[derive(Clone, Copy)]
struct WvRECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// HWND wrapper for WebView2
#[repr(transparent)]
#[derive(Clone, Copy)]
struct WvHWND(pub *mut c_void);

/// EventRegistrationToken for WebView2
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct WvEventToken {
    value: i64,
}

/// WebView2 wrapper
struct WebView {
    controller: ICoreWebView2Controller,
    webview: ICoreWebView2,
}

// Safety: WebView2 COM objects are accessed only from the main thread
unsafe impl Send for WebView {}
unsafe impl Sync for WebView {}

impl WebView {
    /// Resize the WebView to fill the given bounds
    fn resize(&self, left: i32, top: i32, right: i32, bottom: i32) {
        unsafe {
            let rect = WvRECT {
                left,
                top,
                right,
                bottom,
            };
            // Transmute to the expected type (same layout)
            let _ = self.controller.SetBounds(std::mem::transmute(rect));
        }
    }

    /// Navigate to HTML content
    fn load_monaco(&self) {
        unsafe {
            let html = pwstr_from_str(MONACO_HTML);
            let _ = self.webview.NavigateToString(html);
        }
    }

    /// Post a message to JavaScript
    fn post_message(&self, json: &str) {
        unsafe {
            let msg = pwstr_from_str(json);
            let _ = self.webview.PostWebMessageAsJson(msg);
        }
    }

    /// Set editor content from native
    fn set_content(&self, content: &str) {
        let escaped = content
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");
        let json = format!(r#"{{"type":"setContent","content":"{}"}}"#, escaped);
        self.post_message(&json);
    }

    /// Request content from editor
    fn get_content(&self, request_id: u32) {
        let json = format!(r#"{{"type":"getContent","requestId":{}}}"#, request_id);
        self.post_message(&json);
    }

    /// Switch to editor view
    fn show_editor(&self) {
        self.post_message(r#"{"type":"showEditor"}"#);
    }

    /// Switch to file list view
    fn show_files(&self) {
        self.post_message(r#"{"type":"showFiles"}"#);
    }
}

// Win32 API bindings (minimal, to avoid version conflicts)
#[link(name = "shcore")]
unsafe extern "system" {
    fn SetProcessDpiAwareness(value: u32) -> i32;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn SetProcessDpiAwarenessContext(value: isize) -> i32;
    fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: isize,
        menu: isize,
        instance: isize,
        param: *const c_void,
    ) -> isize;

    fn DefWindowProcW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
    fn DispatchMessageW(msg: *const MSG) -> isize;
    fn GetClientRect(hwnd: isize, rect: *mut RECT) -> i32;
    fn GetMessageW(msg: *mut MSG, hwnd: isize, filter_min: u32, filter_max: u32) -> i32;
    fn GetModuleHandleW(name: *const u16) -> isize;
    fn GetSystemMetrics(index: i32) -> i32;
    fn LoadCursorW(instance: isize, cursor: *const u16) -> isize;
    fn PostQuitMessage(exit_code: i32);
    fn RegisterClassW(wc: *const WNDCLASSW) -> u16;
    fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
    fn TranslateMessage(msg: *const MSG) -> i32;
    fn UpdateWindow(hwnd: isize) -> i32;
}

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoInitializeEx(reserved: *const c_void, co_init: u32) -> i32;
    fn CoTaskMemFree(pv: *const c_void);
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateSolidBrush(color: u32) -> isize;
}

// Win32 constants
const WS_OVERLAPPEDWINDOW: u32 = 0x00CF0000;
const WS_VISIBLE: u32 = 0x10000000;
const WS_CHILD: u32 = 0x40000000;
const BS_PUSHBUTTON: u32 = 0x00000000;
const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;
const SW_SHOW: i32 = 5;
const WM_DESTROY: u32 = 0x0002;
const WM_SIZE: u32 = 0x0005;
const WM_COMMAND: u32 = 0x0111;
const IDC_ARROW: *const u16 = 32512 as *const u16;
const COINIT_APARTMENTTHREADED: u32 = 0x2;

// DPI awareness constants
const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: isize = -4;
const PROCESS_PER_MONITOR_DPI_AWARE: u32 = 2;

#[repr(C)]
struct MSG {
    hwnd: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt_x: i32,
    pt_y: i32,
}

#[repr(C)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct WNDCLASSW {
    style: u32,
    lpfn_wnd_proc: Option<unsafe extern "system" fn(isize, u32, usize, isize) -> isize>,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: isize,
    h_icon: isize,
    h_cursor: isize,
    hbr_background: isize,
    lpsz_menu_name: *const u16,
    lpsz_class_name: *const u16,
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn main() {
    unsafe {
        // Set DPI awareness before anything else
        // Try Per-Monitor V2 first (Windows 10 1703+), fall back to Per-Monitor
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) == 0 {
            let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);
        }

        // Initialize COM
        let hr = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED);
        if hr < 0 {
            eprintln!("CoInitializeEx failed: 0x{:08X}", hr);
            return;
        }

        // Create message channel
        let (tx, rx) = mpsc::channel();
        *MESSAGE_CHANNEL.lock().unwrap() = Some(tx);

        let instance = GetModuleHandleW(std::ptr::null());

        // Register window class
        let class_name = to_wide(CLASS_NAME);
        let wc = WNDCLASSW {
            style: 0,
            lpfn_wnd_proc: Some(wndproc),
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance: instance,
            h_icon: 0,
            h_cursor: LoadCursorW(0, IDC_ARROW),
            hbr_background: CreateSolidBrush(0x1e1e1e),
            lpsz_menu_name: std::ptr::null(),
            lpsz_class_name: class_name.as_ptr(),
        };
        RegisterClassW(&wc);

        // Center on screen
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_w - WINDOW_WIDTH) / 2;
        let y = (screen_h - WINDOW_HEIGHT) / 2;

        // Create main window
        let window_name = to_wide("WebView2 + Monaco POC\0");
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_name.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            x,
            y,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            0,
            0,
            instance,
            std::ptr::null(),
        );

        if hwnd == 0 {
            eprintln!("CreateWindowExW failed");
            return;
        }

        APP_HWND = hwnd;

        // Create control buttons
        create_button(hwnd, instance, BTN_SET_CONTENT, "Set Content", 10);
        create_button(hwnd, instance, BTN_GET_CONTENT, "Get Content", 120);
        create_button(hwnd, instance, BTN_SHOW_EDITOR, "Show Editor", 230);
        create_button(hwnd, instance, BTN_SHOW_FILES, "Show Files", 340);

        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);

        // Initialize WebView2
        init_webview2(hwnd);

        // Message loop
        let mut msg = std::mem::zeroed::<MSG>();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            // Check for WebView messages (non-blocking)
            while let Ok(message) = rx.try_recv() {
                handle_webview_message(&message);
            }

            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        println!("[Native] Application exiting");
    }
}

unsafe fn create_button(parent: isize, instance: isize, id: u16, text: &str, x: i32) {
    let button_class = to_wide("BUTTON\0");
    let button_text = to_wide(text);
    unsafe {
        CreateWindowExW(
            0,
            button_class.as_ptr(),
            button_text.as_ptr(),
            WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
            x,
            10,
            100,
            30,
            parent,
            id as isize,
            instance,
            std::ptr::null(),
        );
    }
}

fn init_webview2(hwnd: isize) {
    unsafe {
        let _ = CreateCoreWebView2Environment(
            &CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(move |_error, env| {
                if let Some(env) = env {
                    create_controller(hwnd, env);
                }
                Ok(())
            })),
        );
    }
}

fn create_controller(hwnd: isize, env: ICoreWebView2Environment) {
    unsafe {
        let wv2_hwnd: WvHWND = WvHWND(hwnd as *mut c_void);

        let _ = env.CreateCoreWebView2Controller(
            std::mem::transmute(wv2_hwnd),
            &CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
                move |_error, controller| {
                    if let Some(controller) = controller {
                        setup_webview(hwnd, controller);
                    }
                    Ok(())
                },
            )),
        );
    }
}

fn setup_webview(hwnd: isize, controller: ICoreWebView2Controller) {
    unsafe {
        let webview: ICoreWebView2 = match controller.CoreWebView2() {
            Ok(wv) => wv,
            Err(e) => {
                eprintln!("[Native] Failed to get CoreWebView2: {:?}", e);
                return;
            }
        };

        // Set up message handler
        let mut token = WvEventToken { value: 0 };

        let _ = webview.add_WebMessageReceived(
            &WebMessageReceivedEventHandler::create(Box::new(
                |_webview, args: Option<ICoreWebView2WebMessageReceivedEventArgs>| {
                    if let Some(args) = args {
                        let mut message: *mut u16 = std::ptr::null_mut();
                        // Use raw pointer for PWSTR
                        if args
                            .TryGetWebMessageAsString(std::mem::transmute(&mut message))
                            .is_ok()
                        {
                            let msg_str = pwstr_to_string(message);
                            if let Some(tx) = MESSAGE_CHANNEL.lock().ok().and_then(|g| g.clone()) {
                                let _ = tx.send(msg_str);
                            }
                            CoTaskMemFree(message as *const c_void);
                        }
                    }
                    Ok(())
                },
            )),
            std::mem::transmute(&mut token),
        );

        // Get client rect and size the WebView
        let mut rect = std::mem::zeroed::<RECT>();
        GetClientRect(hwnd, &mut rect);

        let wv_rect = WvRECT {
            left: 0,
            top: BUTTON_HEIGHT,
            right: rect.right,
            bottom: rect.bottom,
        };
        let _ = controller.SetBounds(std::mem::transmute(wv_rect));

        // Create WebView wrapper
        let wv = WebView {
            controller,
            webview,
        };

        // Load Monaco
        wv.load_monaco();

        // Store WebView globally
        WEBVIEW = Some(wv);

        println!("[Native] WebView2 initialized successfully");
    }
}

fn pwstr_to_string(pwstr: *mut u16) -> String {
    if pwstr.is_null() {
        return String::new();
    }
    unsafe {
        let mut len = 0;
        let mut ptr = pwstr as *const u16;
        while *ptr != 0 {
            len += 1;
            ptr = ptr.add(1);
        }
        let slice = std::slice::from_raw_parts(pwstr as *const u16, len);
        String::from_utf16_lossy(slice)
    }
}

fn handle_webview_message(message: &str) {
    println!("[Native] Received from JS: {}", message);

    if message.contains(r#""type":"ready""#) {
        println!("[Native] Monaco editor is ready!");
    } else if message.contains(r#""type":"contentChanged""#) {
        println!("[Native] Editor content changed");
    } else if message.contains(r#""type":"contentResponse""#) {
        println!("[Native] Got content response");
        if let Some(start) = message.find(r#""content":""#) {
            let content_start = start + 11;
            if let Some(end) = message[content_start..].find('"') {
                let content = &message[content_start..content_start + end];
                let preview = if content.len() > 100 {
                    format!("{}...", &content[..100])
                } else {
                    content.to_string()
                };
                println!("[Native] Content: {}", preview);
            }
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize {
    unsafe {
        match msg {
            WM_SIZE => {
                let width = (lparam & 0xFFFF) as i32;
                let height = ((lparam >> 16) & 0xFFFF) as i32;

                if let Some(ref wv) = WEBVIEW {
                    wv.resize(0, BUTTON_HEIGHT, width, height);
                }
                0
            }
            WM_COMMAND => {
                let cmd_id = (wparam & 0xFFFF) as u16;
                if let Some(ref wv) = WEBVIEW {
                    match cmd_id {
                        BTN_SET_CONTENT => {
                            println!("[Native] Setting content...");
                            wv.set_content(
                                "Hello from Rust!\n\nThis text was set programmatically.\n\nLine 4\nLine 5",
                            );
                        }
                        BTN_GET_CONTENT => {
                            println!("[Native] Requesting content...");
                            let id = REQUEST_COUNTER;
                            REQUEST_COUNTER += 1;
                            wv.get_content(id);
                        }
                        BTN_SHOW_EDITOR => {
                            println!("[Native] Switching to editor view...");
                            wv.show_editor();
                        }
                        BTN_SHOW_FILES => {
                            println!("[Native] Switching to file list view...");
                            wv.show_files();
                        }
                        _ => {}
                    }
                }
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
