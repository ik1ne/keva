//! WebView2 CompositionController Drag-Drop Timing Test
//!
//! Tests whether `ExecuteScriptAsync` injection bypasses the 200-5000ms delay
//! between native `Drop()` and JavaScript drop events.
//!
//! Run with: cargo run --example drop_timing_test
//!
//! ## Test Procedure
//! 1. Drop files onto the window
//! 2. Open DevTools console to observe timing logs:
//!    - `[native_drop]`: Native injection via ExecuteScriptAsync
//!    - `[drop]`: Standard DOM drop event via cc3.Drop()
//!    - `[delta]`: Time difference between the two
//!
//! If `[native_drop]` consistently arrives before `[drop]`, native injection
//! is viable for timing-critical applications.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::Cell;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::sync::OnceLock;
use std::time::Instant;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    CreateCoreWebView2Environment, ICoreWebView2, ICoreWebView2CompositionController,
    ICoreWebView2CompositionController3, ICoreWebView2Controller, ICoreWebView2Controller4,
    ICoreWebView2Environment, ICoreWebView2Environment3,
};
use webview2_com::{
    CreateCoreWebView2CompositionControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler, pwstr_from_str,
};
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, POINT, POINTL, RECT, WPARAM};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::Com::{DVASPECT_CONTENT, FORMATETC, IDataObject, TYMED_HGLOBAL};
use windows::Win32::System::Ole::{
    CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, IDropTarget, IDropTarget_Impl, OleInitialize,
    OleUninitialize, RegisterDragDrop, ReleaseStgMedium,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW,
    GetSystemMetrics, IDC_ARROW, LoadCursorW, MSG, PostQuitMessage, RegisterClassW, SM_CXSCREEN,
    SM_CYSCREEN, SW_SHOW, ShowWindow, TranslateMessage, WM_DESTROY, WM_SIZE, WNDCLASSW,
    WS_OVERLAPPEDWINDOW,
};
use windows::core::{Interface, Ref};

const WINDOW_WIDTH: i32 = 800;
const WINDOW_HEIGHT: i32 = 600;
const CLASS_NAME: &str = "DropTimingTest\0";

const HTML_CONTENT: &str = r#"<!DOCTYPE html>
<html>
<head></head>
<body>
  <div id="left">Left</div>
  <div id="right">Right</div>
  <script>
    let nativeDropTime = null;

    window.addEventListener('native_drop', (e) => {
      const now = performance.now();
      nativeDropTime = now;
      console.log('[native_drop]', now.toFixed(2) + 'ms', 'Files:', e.detail.count);
    });

    document.addEventListener('dragenter', (e) => {
      e.preventDefault();
      console.log('[dragenter]', performance.now().toFixed(2) + 'ms');
    });

    document.addEventListener('dragover', (e) => {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'copy';
      console.log('[dragover]', performance.now().toFixed(2) + 'ms');
    });

    document.addEventListener('dragleave', (e) => {
      console.log('[dragleave]', performance.now().toFixed(2) + 'ms');
    });

    document.addEventListener('drop', (e) => {
      e.preventDefault();
      const now = performance.now();
      console.log('[drop]', now.toFixed(2) + 'ms', 'Files:', e.dataTransfer.files.length);
      if (nativeDropTime !== null) {
        console.log('[delta]', (now - nativeDropTime).toFixed(2) + 'ms');
        nativeDropTime = null;
      }
    });

    console.log('[ready]');
  </script>
</body>
</html>"#;

/// WebView state stored globally for access from callbacks
static WEBVIEW_STATE: OnceLock<WebViewState> = OnceLock::new();

struct WebViewState {
    composition_controller: ICoreWebView2CompositionController,
    webview: ICoreWebView2,
    #[expect(dead_code)]
    composition_host: CompositionHost,
}

unsafe impl Send for WebViewState {}
unsafe impl Sync for WebViewState {}

/// DirectComposition resources for visual hosting
struct CompositionHost {
    device: IDCompositionDevice,
    #[expect(dead_code)]
    target: IDCompositionTarget,
    root_visual: IDCompositionVisual,
}

impl CompositionHost {
    fn new(hwnd: HWND) -> windows::core::Result<Self> {
        unsafe {
            // Create D3D11 device (required for DComposition)
            let mut d3d_device = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d_device),
                None,
                None,
            )?;
            let d3d_device = d3d_device.unwrap();

            // Get DXGI device for DComposition
            let dxgi_device: IDXGIDevice = d3d_device.cast()?;

            // Create DComposition device
            let dcomp_device: IDCompositionDevice = DCompositionCreateDevice(&dxgi_device)?;

            // Create composition target for the window
            let target = dcomp_device.CreateTargetForHwnd(hwnd, true)?;

            // Create root visual
            let root_visual = dcomp_device.CreateVisual()?;

            // Set the root visual on the target
            target.SetRoot(&root_visual)?;

            // Commit the composition
            dcomp_device.Commit()?;

            Ok(Self {
                device: dcomp_device,
                target,
                root_visual,
            })
        }
    }

    fn root_visual(&self) -> &IDCompositionVisual {
        &self.root_visual
    }

    fn commit(&self) -> windows::core::Result<()> {
        unsafe { self.device.Commit() }
    }
}

/// ~33ms between DragOver forwards to WebView2 (30fps)
const DRAGOVER_THROTTLE_MS: u128 = 33;

/// IDropTarget implementation for timing test
#[windows_core::implement(IDropTarget)]
struct DropTarget {
    hwnd: HWND,
    last_dragover: Cell<Option<Instant>>,
    cached_effect: Cell<DROPEFFECT>,
}

impl DropTarget {
    fn to_webview_point(&self, pt: &POINTL) -> POINT {
        let mut point = POINT { x: pt.x, y: pt.y };
        let _ = unsafe { ScreenToClient(self.hwnd, &mut point) };
        point
    }
}

fn count_files(data_obj: &IDataObject) -> u32 {
    let format = FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };

    unsafe {
        if let Ok(mut medium) = data_obj.GetData(&format) {
            let hdrop = HDROP(medium.u.hGlobal.0 as *mut _);
            let count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
            ReleaseStgMedium(&mut medium);
            count
        } else {
            0
        }
    }
}

fn get_file_names(data_obj: &IDataObject) -> Vec<String> {
    let format = FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };

    let mut names = Vec::new();
    unsafe {
        if let Ok(mut medium) = data_obj.GetData(&format) {
            let hdrop = HDROP(medium.u.hGlobal.0 as *mut _);
            let count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);

            for i in 0..count {
                let len = DragQueryFileW(hdrop, i, None);
                if len > 0 {
                    let mut buf = vec![0u16; (len + 1) as usize];
                    let actual_len = DragQueryFileW(hdrop, i, Some(&mut buf));
                    if actual_len > 0 {
                        buf.truncate(actual_len as usize);
                        let path = OsString::from_wide(&buf);
                        names.push(path.to_string_lossy().to_string());
                    }
                }
            }

            ReleaseStgMedium(&mut medium);
        }
    }
    names
}

impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(
        &self,
        pdataobj: Ref<'_, IDataObject>,
        grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        println!("[Native] DragEnter at ({}, {})", pt.x, pt.y);

        // Forward to WebView2 CompositionController3 (handles drag visual)
        if let Some(state) = WEBVIEW_STATE.get()
            && let Ok(cc3) = state
                .composition_controller
                .cast::<ICoreWebView2CompositionController3>()
        {
            let point = self.to_webview_point(pt);
            let data_obj_opt = pdataobj.ok().ok();
            let _ =
                unsafe { cc3.DragEnter(data_obj_opt, grfkeystate.0, point, pdweffect as *mut u32) };
        }

        unsafe {
            if (*pdweffect).0 == 0 {
                *pdweffect = DROPEFFECT_COPY;
            }
        }
        Ok(())
    }

    fn DragOver(
        &self,
        grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        // Debug: check if OS sends DragOver without pointer movement
        static LAST_PT: std::sync::Mutex<Option<POINTL>> = std::sync::Mutex::new(None);
        let mut last = LAST_PT.lock().unwrap();
        let moved = last.map(|l| l.x != pt.x || l.y != pt.y).unwrap_or(true);
        if moved {
            println!("[DragOver] MOVED to ({}, {})", pt.x, pt.y);
        } else {
            println!("[DragOver] STATIONARY");
        }
        *last = Some(*pt);

        // Throttle forwarding to WebView2 at 30fps
        let now = Instant::now();
        let should_forward = match self.last_dragover.get() {
            None => true,
            Some(last) => now.duration_since(last).as_millis() >= DRAGOVER_THROTTLE_MS,
        };

        if should_forward {
            self.last_dragover.set(Some(now));

            // Forward to WebView2 CompositionController3
            if let Some(state) = WEBVIEW_STATE.get()
                && let Ok(cc3) = state
                    .composition_controller
                    .cast::<ICoreWebView2CompositionController3>()
            {
                let point = self.to_webview_point(pt);
                let _ = unsafe { cc3.DragOver(grfkeystate.0, point, pdweffect as *mut u32) };
            }

            // Cache the effect returned by WebView2
            unsafe {
                if (*pdweffect).0 == 0 {
                    *pdweffect = DROPEFFECT_COPY;
                }
                self.cached_effect.set(*pdweffect);
            }
        } else {
            // Use cached effect when throttled for consistent cursor feedback
            unsafe {
                *pdweffect = self.cached_effect.get();
            }
        }

        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        println!("[Native] DragLeave");

        // Forward to WebView2 CompositionController3
        if let Some(state) = WEBVIEW_STATE.get()
            && let Ok(cc3) = state
                .composition_controller
                .cast::<ICoreWebView2CompositionController3>()
        {
            let _ = unsafe { cc3.DragLeave() };
        }
        Ok(())
    }

    fn Drop(
        &self,
        pdataobj: Ref<'_, IDataObject>,
        grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        println!("[Native] Drop at ({}, {})", pt.x, pt.y);

        let Some(state) = WEBVIEW_STATE.get() else {
            return Ok(());
        };

        // === PATH B: Immediate native injection ===
        // This should trigger the JS event before the DOM drop event
        if let Ok(data_obj) = pdataobj.ok() {
            let file_count = count_files(data_obj);
            let file_names = get_file_names(data_obj);

            println!("[Native Drop] Files: {:?}", file_names);

            let script = format!(
                r#"window.dispatchEvent(new CustomEvent('native_drop', {{detail: {{count: {}}}}}));"#,
                file_count
            );
            let script_pwstr = pwstr_from_str(&script);
            // ExecuteScript runs asynchronously despite the name
            let _ = unsafe { state.webview.ExecuteScript(script_pwstr, None) };
        }

        // === PATH A: Standard forwarding (triggers DOM drop event later) ===
        if let Ok(cc3) = state
            .composition_controller
            .cast::<ICoreWebView2CompositionController3>()
        {
            let point = self.to_webview_point(pt);
            let data_obj_opt = pdataobj.ok().ok();
            let _ = unsafe { cc3.Drop(data_obj_opt, grfkeystate.0, point, pdweffect as *mut u32) };
        }

        unsafe {
            if (*pdweffect).0 == 0 {
                *pdweffect = DROPEFFECT_COPY;
            }
        }
        Ok(())
    }
}

// Thread-local flag indicating if drop target is registered (to avoid double registration)
thread_local! {
    static DROP_TARGET_REGISTERED: Cell<bool> = const { Cell::new(false) };
}

fn register_drop_target(hwnd: HWND) -> windows::core::Result<()> {
    DROP_TARGET_REGISTERED.with(|registered| {
        if registered.get() {
            return Ok(());
        }

        let target = DropTarget {
            hwnd,
            last_dragover: Cell::new(None),
            cached_effect: Cell::new(DROPEFFECT_COPY),
        };
        let target_interface: IDropTarget = target.into();
        let result = unsafe { RegisterDragDrop(hwnd, &target_interface) };
        if result.is_ok() {
            registered.set(true);
        }
        result
    })
}

fn init_webview(hwnd: HWND) {
    unsafe {
        let _ = CreateCoreWebView2Environment(
            &CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(move |_error, env| {
                let Some(env) = env else {
                    eprintln!("[WebView] Environment creation failed");
                    return Ok(());
                };
                create_composition_controller(hwnd, env);
                Ok(())
            })),
        );
    }
}

fn create_composition_controller(hwnd: HWND, env: ICoreWebView2Environment) {
    // Create DirectComposition host first
    let composition_host = match CompositionHost::new(hwnd) {
        Ok(host) => host,
        Err(e) => {
            eprintln!("[WebView] Failed to create CompositionHost: {:?}", e);
            return;
        }
    };

    unsafe {
        let Ok(env3) = env.cast::<ICoreWebView2Environment3>() else {
            eprintln!("[WebView] ICoreWebView2Environment3 not available");
            return;
        };

        let _ = env3.CreateCoreWebView2CompositionController(
            hwnd,
            &CreateCoreWebView2CompositionControllerCompletedHandler::create(Box::new(
                move |_error, composition_controller| {
                    let Some(composition_controller) = composition_controller else {
                        eprintln!("[WebView] CompositionController creation failed");
                        return Ok(());
                    };

                    // Set the root visual target for DirectComposition
                    if let Err(e) =
                        composition_controller.SetRootVisualTarget(composition_host.root_visual())
                    {
                        eprintln!("[WebView] SetRootVisualTarget failed: {:?}", e);
                        return Ok(());
                    }

                    let Ok(controller) = composition_controller.cast::<ICoreWebView2Controller>()
                    else {
                        eprintln!("[WebView] Failed to cast to ICoreWebView2Controller");
                        return Ok(());
                    };

                    // Enable external drop
                    if let Ok(ctrl4) = controller.cast::<ICoreWebView2Controller4>() {
                        let _ = ctrl4.SetAllowExternalDrop(true);
                        println!("[WebView] AllowExternalDrop enabled");
                    }

                    let Ok(webview) = controller.CoreWebView2() else {
                        eprintln!("[WebView] Failed to get CoreWebView2");
                        return Ok(());
                    };

                    // Set bounds
                    let mut rect = RECT::default();
                    let _ = GetClientRect(hwnd, &mut rect);
                    let _ = controller.SetBounds(rect);
                    let _ = controller.SetIsVisible(true);

                    // Commit composition after WebView visual is attached
                    let _ = composition_host.commit();

                    // Open DevTools in debug builds
                    #[cfg(debug_assertions)]
                    {
                        let _ = webview.OpenDevToolsWindow();
                        println!("[WebView] DevTools window opened");
                    }

                    // Store state for drop target access
                    let _ = WEBVIEW_STATE.set(WebViewState {
                        composition_controller: composition_controller.clone(),
                        webview: webview.clone(),
                        composition_host,
                    });

                    // Register drop target now that WebView is ready
                    if let Err(e) = register_drop_target(hwnd) {
                        eprintln!("[WebView] Failed to register drop target: {:?}", e);
                    } else {
                        println!("[WebView] Drop target registered");
                    }

                    // Navigate to embedded HTML
                    let html_pwstr = pwstr_from_str(HTML_CONTENT);
                    let _ = webview.NavigateToString(html_pwstr);

                    println!("[WebView] Initialized successfully");
                    println!("[WebView] Drop files onto the window to test timing");

                    Ok(())
                },
            )),
        );
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn main() {
    unsafe {
        // OleInitialize required for drag-drop
        let ole_result = OleInitialize(None);
        if ole_result.is_err() {
            eprintln!("OleInitialize failed: {:?}", ole_result);
            return;
        }

        let instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .expect("GetModuleHandleW failed");

        let class_name = to_wide(CLASS_NAME);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };

        let atom = RegisterClassW(&wc);
        if atom == 0 {
            eprintln!("RegisterClassW failed");
            return;
        }

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_w - WINDOW_WIDTH) / 2;
        let y = (screen_h - WINDOW_HEIGHT) / 2;

        let window_name = to_wide("Drop Timing Test - WebView2 CompositionController\0");
        let hwnd = CreateWindowExW(
            Default::default(),
            windows::core::PCWSTR(class_name.as_ptr()),
            windows::core::PCWSTR(window_name.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            x,
            y,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .expect("CreateWindowExW failed");

        let _ = ShowWindow(hwnd, SW_SHOW);

        println!("=== WebView2 CompositionController Drop Timing Test ===");
        println!("Initializing WebView2...");

        init_webview(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        OleUninitialize();
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_SIZE => {
                let width = (lparam.0 & 0xFFFF) as i32;
                let height = ((lparam.0 >> 16) & 0xFFFF) as i32;

                if let Some(state) = WEBVIEW_STATE.get()
                    && let Ok(controller) = state
                        .composition_controller
                        .cast::<ICoreWebView2Controller>()
                {
                    let rect = RECT {
                        left: 0,
                        top: 0,
                        right: width,
                        bottom: height,
                    };
                    let _ = controller.SetBounds(rect);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
