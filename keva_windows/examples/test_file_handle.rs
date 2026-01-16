//! Test FileSystemHandle API for direct file access from JavaScript.
//!
//! Run: cargo run -q --example test_file_handle

use std::ffi::c_void;
use std::sync::OnceLock;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_FILE_SYSTEM_HANDLE_PERMISSION_READ_WRITE, CreateCoreWebView2Environment,
    ICoreWebView2, ICoreWebView2_23, ICoreWebView2Controller, ICoreWebView2Environment,
    ICoreWebView2Environment14, ICoreWebView2ObjectCollection,
    ICoreWebView2WebMessageReceivedEventArgs,
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
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: 'Segoe UI', sans-serif;
            background: #1e1e1e;
            color: #d4d4d4;
            height: 100vh;
            display: flex;
            flex-direction: column;
        }
        .toolbar {
            padding: 8px;
            background: #252526;
            border-bottom: 1px solid #3c3c3c;
            display: flex;
            gap: 8px;
            align-items: center;
        }
        button {
            padding: 6px 12px;
            background: #0e639c;
            border: none;
            color: white;
            cursor: pointer;
            border-radius: 2px;
        }
        button:hover { background: #1177bb; }
        button:disabled { background: #3c3c3c; color: #808080; cursor: not-allowed; }
        .status-bar {
            padding: 4px 8px;
            background: #007acc;
            font-size: 12px;
            display: flex;
            justify-content: space-between;
        }
        #editor-container { flex: 1; }
        .log-panel {
            height: 120px;
            background: #1e1e1e;
            border-top: 1px solid #3c3c3c;
            overflow-y: auto;
            font-family: monospace;
            font-size: 12px;
            padding: 4px 8px;
        }
        .log-panel .info { color: #4ec9b0; }
        .log-panel .error { color: #f44747; }
    </style>
</head>
<body>
    <div class="toolbar">
        <button id="btn-request" onclick="requestHandle()">1. Request Handle</button>
        <button id="btn-read" onclick="readFile()" disabled>2. Read File</button>
        <button id="btn-write" onclick="writeFile()" disabled>3. Save File</button>
        <button onclick="generateLargeContent()">Generate 10MB</button>
        <span id="file-info" style="margin-left: auto; color: #808080;"></span>
    </div>
    <div id="editor-container"></div>
    <div class="log-panel" id="log"></div>
    <div class="status-bar">
        <span id="status">Ready</span>
        <span id="size-info"></span>
    </div>

<script src="https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.45.0/min/vs/loader.min.js"></script>
<script>
    let fileHandle = null;
    let editor = null;

    function log(msg, isError = false) {
        const el = document.getElementById('log');
        const line = document.createElement('div');
        line.className = isError ? 'error' : 'info';
        line.textContent = '[' + new Date().toLocaleTimeString() + '] ' + msg;
        el.appendChild(line);
        el.scrollTop = el.scrollHeight;
    }

    function setStatus(msg) {
        document.getElementById('status').textContent = msg;
    }

    function updateButtons() {
        document.getElementById('btn-read').disabled = !fileHandle;
        document.getElementById('btn-write').disabled = !fileHandle;
    }

    function requestHandle() {
        log('Requesting FileSystemHandle from native...');
        setStatus('Requesting handle...');
        window.chrome.webview.postMessage(JSON.stringify({ type: 'requestHandle' }));
    }

    async function readFile() {
        if (!fileHandle) return;
        try {
            setStatus('Reading file...');
            const start = performance.now();
            const file = await fileHandle.getFile();
            document.getElementById('file-info').textContent = file.name + ' (' + formatSize(file.size) + ')';

            const content = await file.text();
            const readTime = performance.now() - start;

            editor.setValue(content);
            log('Read ' + formatSize(file.size) + ' in ' + readTime.toFixed(0) + 'ms');
            setStatus('File loaded');
            updateSizeInfo();
        } catch (ex) {
            log('Read failed: ' + ex.toString(), true);
            setStatus('Read failed');
        }
    }

    async function writeFile() {
        if (!fileHandle) return;
        try {
            setStatus('Writing file...');
            const start = performance.now();
            const content = editor.getValue();

            const writable = await fileHandle.createWritable();
            await writable.write(content);
            await writable.close();

            const writeTime = performance.now() - start;
            log('Wrote ' + formatSize(content.length) + ' in ' + writeTime.toFixed(0) + 'ms');
            setStatus('File saved');
        } catch (ex) {
            log('Write failed: ' + ex.toString(), true);
            setStatus('Write failed');
        }
    }

    function generateLargeContent() {
        setStatus('Generating content...');
        const line = 'Line of text for testing large file performance. ';
        const linesPerChunk = 1000;
        const chunks = 200; // ~10MB
        let content = '';
        for (let i = 0; i < chunks; i++) {
            for (let j = 0; j < linesPerChunk; j++) {
                content += line + (i * linesPerChunk + j) + '\n';
            }
        }
        editor.setValue(content);
        log('Generated ' + formatSize(content.length));
        setStatus('Content generated');
        updateSizeInfo();
    }

    function formatSize(bytes) {
        if (bytes < 1024) return bytes + ' B';
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
        return (bytes / 1024 / 1024).toFixed(2) + ' MB';
    }

    function updateSizeInfo() {
        const content = editor.getValue();
        document.getElementById('size-info').textContent =
            formatSize(content.length) + ' | ' + editor.getModel().getLineCount() + ' lines';
    }

    window.chrome.webview.addEventListener('message', (e) => {
        const msg = e.data;
        if (msg.type === 'error') {
            log(msg.message, true);
            setStatus('Error');
        } else if (msg.type === 'info') {
            log(msg.message);
        } else if (msg.type === 'fileHandle') {
            if (e.additionalObjects && e.additionalObjects.length > 0) {
                fileHandle = e.additionalObjects[0];
                log('FileSystemHandle received: ' + fileHandle.name);
                setStatus('Handle ready - click Read File');
                updateButtons();
            } else {
                log('No additionalObjects in message', true);
            }
        }
    });

    // Initialize Monaco
    require.config({ paths: { vs: 'https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.45.0/min/vs' }});
    require(['vs/editor/editor.main'], function() {
        editor = monaco.editor.create(document.getElementById('editor-container'), {
            value: '// Click "Request Handle" then "Read File" to load content\n// Or click "Generate 10MB" to test large file handling\n',
            language: 'plaintext',
            theme: 'vs-dark',
            automaticLayout: true,
            minimap: { enabled: true },
            fontSize: 14,
            wordWrap: 'on',
            scrollBeyondLastLine: false
        });

        editor.onDidChangeModelContent(() => {
            updateSizeInfo();
        });

        log('Monaco editor initialized');
        log('Test file: %TEMP%\\keva_file_handle_test.txt');
    });
</script>
</body>
</html>"#;

fn main() {
    // Initialize COM
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    // Create test file
    let test_file_path = std::env::temp_dir().join("keva_file_handle_test.txt");
    std::fs::write(
        &test_file_path,
        "Hello from test file!\nEdit me and save.\n",
    )
    .ok();
    eprintln!("[Native] Test file: {}", test_file_path.display());

    unsafe {
        let instance = GetModuleHandleW(None).unwrap();
        let class_name = w!("FileHandleTest");

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
            w!("FileSystemHandle Test"),
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
                    eprintln!("{:?}", _err);
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

        if msg_str.contains("requestHandle") {
            send_file_handle(webview, env);
        }
    }
}

fn send_file_handle(webview: &ICoreWebView2, env: &ICoreWebView2Environment) {
    unsafe {
        // Try to get ICoreWebView2Environment14 for CreateWebFileSystemFileHandle
        let Ok(env14) = env.cast::<ICoreWebView2Environment14>() else {
            send_error(
                webview,
                "ICoreWebView2Environment14 not available (need SDK 1.0.2470+)",
            );
            return;
        };

        // Try to get ICoreWebView2_23 for PostWebMessageAsJsonWithAdditionalObjects
        let Ok(webview23) = webview.cast::<ICoreWebView2_23>() else {
            send_error(
                webview,
                "ICoreWebView2_23 not available (need SDK 1.0.2470+)",
            );
            return;
        };

        // Test file path
        let test_file_path = std::env::temp_dir().join("keva_file_handle_test.txt");

        eprintln!(
            "[Native] Creating FileSystemHandle for: {}",
            test_file_path.display()
        );

        // Create file system handle (returns Result<ICoreWebView2FileSystemHandle>)
        let path_pwstr = pwstr_from_str(&test_file_path.to_string_lossy());
        let handle = match env14.CreateWebFileSystemFileHandle(
            path_pwstr,
            COREWEBVIEW2_FILE_SYSTEM_HANDLE_PERMISSION_READ_WRITE,
        ) {
            Ok(h) => h,
            Err(e) => {
                send_error(
                    webview,
                    &format!("CreateWebFileSystemFileHandle failed: {:?}", e),
                );
                return;
            }
        };

        eprintln!("[Native] FileSystemHandle created");

        // Create object collection with the handle
        // The handle needs to be cast to IUnknown for the collection
        let handle_iunknown: windows::core::IUnknown = handle.cast().unwrap();
        let mut items = [Some(handle_iunknown)];
        let mut collection: Option<ICoreWebView2ObjectCollection> = None;

        if let Err(e) = env14.CreateObjectCollection(1, items.as_mut_ptr(), &mut collection) {
            send_error(webview, &format!("CreateObjectCollection failed: {:?}", e));
            return;
        }

        let Some(objects) = collection else {
            send_error(webview, "CreateObjectCollection returned None");
            return;
        };

        // Post message with additional objects
        // ICoreWebView2ObjectCollection derefs to ICoreWebView2ObjectCollectionView
        let json = pwstr_from_str(r#"{"type":"fileHandle"}"#);
        if let Err(e) = webview23.PostWebMessageAsJsonWithAdditionalObjects(json, &*objects) {
            send_error(
                webview,
                &format!("PostWebMessageAsJsonWithAdditionalObjects failed: {:?}", e),
            );
            return;
        }

        eprintln!("[Native] FileSystemHandle posted to script");
        send_info(webview, "FileSystemHandle sent successfully!");
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
