//! Test Monaco paste event interception.
//!
//! Run: cargo run -q --example test_monaco_paste

use std::sync::OnceLock;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    CreateCoreWebView2Environment, ICoreWebView2, ICoreWebView2Controller, ICoreWebView2Environment,
};
use webview2_com::{
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    pwstr_from_str,
};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, IDC_ARROW, LoadCursorW, MSG,
    PostQuitMessage, RegisterClassW, SW_SHOW, ShowWindow, WM_CLOSE, WM_DESTROY, WNDCLASSW,
    WS_OVERLAPPEDWINDOW,
};
use windows::core::w;

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
        .info {
            padding: 12px;
            background: #252526;
            border-bottom: 1px solid #3c3c3c;
        }
        .info h3 { margin-bottom: 8px; color: #569cd6; }
        .info p { font-size: 13px; margin: 4px 0; }
        #editor-container { flex: 1; }
        .log-panel {
            height: 200px;
            background: #1e1e1e;
            border-top: 1px solid #3c3c3c;
            overflow-y: auto;
            font-family: monospace;
            font-size: 12px;
            padding: 8px;
        }
        .log-panel .ctrl-v { color: #4ec9b0; }
        .log-panel .context { color: #ce9178; }
        .log-panel .capture { color: #dcdcaa; }
        .log-panel .bubble { color: #9cdcfe; }
        .log-panel .prevented { color: #f44747; }
    </style>
</head>
<body>
    <div class="info">
        <h3>Monaco Paste Event Test</h3>
        <p>1. Copy some text to clipboard</p>
        <p>2. Try Ctrl+V in the editor</p>
        <p>3. Try right-click → Paste in the editor</p>
        <p>4. Check which events fire in the log below</p>
    </div>
    <div id="editor-container"></div>
    <div class="log-panel" id="log"></div>

<script src="https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.45.0/min/vs/loader.min.js"></script>
<script>
    let editor = null;
    let interceptEnabled = false;

    function log(msg, className = '') {
        const el = document.getElementById('log');
        const line = document.createElement('div');
        line.className = className;
        line.textContent = '[' + new Date().toLocaleTimeString() + '] ' + msg;
        el.appendChild(line);
        el.scrollTop = el.scrollHeight;
    }

    // Initialize Monaco
    require.config({ paths: { vs: 'https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.45.0/min/vs' }});
    require(['vs/editor/editor.main'], function() {
        editor = monaco.editor.create(document.getElementById('editor-container'), {
            value: '// Type or paste here\n// Watch the log panel below\n',
            language: 'javascript',
            theme: 'vs-dark',
            automaticLayout: true,
            minimap: { enabled: false },
            pasteAs: { enabled: false }  // <-- Testing this option
        });

        const container = editor.getContainerDomNode();

        // Test 1: Capture phase listener on container
        container.addEventListener('paste', (e) => {
            const types = e.clipboardData ? Array.from(e.clipboardData.types) : [];
            const hasFiles = e.clipboardData?.files?.length > 0;
            log(`CAPTURE phase paste on container - types: [${types.join(', ')}], files: ${hasFiles}`, 'capture');

            // Uncomment to test interception:
            // e.preventDefault();
            // e.stopPropagation();
            // log('  → PREVENTED and STOPPED', 'prevented');
        }, true);

        // Test 2: Bubble phase listener on container
        container.addEventListener('paste', (e) => {
            log('BUBBLE phase paste on container', 'bubble');
        }, false);

        // Test 3: Capture phase on document
        document.addEventListener('paste', (e) => {
            log('CAPTURE phase paste on document', 'capture');
        }, true);

        // Test 4: Bubble phase on document
        document.addEventListener('paste', (e) => {
            log('BUBBLE phase paste on document', 'bubble');
        }, false);

        // Test 5: Monaco's onDidPaste
        editor.onDidPaste((event) => {
            log(`Monaco onDidPaste - range: ${event.range.startLineNumber}:${event.range.startColumn}`, 'ctrl-v');
        });

        // Test 6: keydown for Ctrl+V
        editor.onKeyDown((e) => {
            if ((e.ctrlKey || e.metaKey) && e.keyCode === monaco.KeyCode.KeyV) {
                log('Monaco onKeyDown: Ctrl+V detected', 'ctrl-v');
            }
        });

        log('Monaco initialized with pasteAs: { enabled: false }');
        log('Try: 1) Ctrl+V  2) Right-click → Paste');
        log('---');
    });
</script>
</body>
</html>"#;

fn main() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    unsafe {
        let instance = GetModuleHandleW(None).unwrap();
        let class_name = w!("MonacoPasteTest");

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
            w!("Monaco Paste Test"),
            WS_OVERLAPPEDWINDOW,
            100,
            100,
            900,
            700,
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
        WM_CLOSE | WM_DESTROY => {
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

                    let mut rect = RECT::default();
                    windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect).ok();
                    controller.SetBounds(rect).ok();
                    controller.SetIsVisible(true).ok();

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
