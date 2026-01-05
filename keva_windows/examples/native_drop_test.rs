//! Minimal native Windows IDropTarget test - NO WebView2
//!
//! Tests whether DragOver is called continuously when mouse is stationary.
//! This isolates the behavior to determine if it's OLE or WebView2-specific.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::Cell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINTL, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT, TextOutA};
use windows::Win32::System::Com::IDataObject;
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::System::Ole::{
    DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl, OleInitialize,
    OleUninitialize, RegisterDragDrop,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetSystemMetrics, IDC_ARROW,
    LoadCursorW, MSG, PostQuitMessage, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW,
    ShowWindow, WM_DESTROY, WM_PAINT, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};
use windows::core::Ref;

const WINDOW_WIDTH: i32 = 600;
const WINDOW_HEIGHT: i32 = 400;

static DRAG_OVER_COUNT: AtomicU32 = AtomicU32::new(0);
static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Minimal IDropTarget - just logs timing
#[windows_core::implement(IDropTarget)]
struct MinimalDropTarget {
    last_pt: Cell<Option<POINTL>>,
    last_call: Cell<Option<Instant>>,
}

impl IDropTarget_Impl for MinimalDropTarget_Impl {
    fn DragEnter(
        &self,
        _pdataobj: Ref<'_, IDataObject>,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        println!("[DragEnter] at ({}, {})", pt.x, pt.y);
        DRAG_OVER_COUNT.store(0, Ordering::Relaxed);
        START_TIME.get_or_init(Instant::now);
        self.last_pt.set(Some(*pt));
        self.last_call.set(Some(Instant::now()));

        unsafe { *pdweffect = DROPEFFECT_COPY };
        Ok(())
    }

    fn DragOver(
        &self,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let now = Instant::now();
        let count = DRAG_OVER_COUNT.fetch_add(1, Ordering::Relaxed);

        // Check if position changed
        let moved = match self.last_pt.get() {
            Some(last) => last.x != pt.x || last.y != pt.y,
            None => true,
        };

        // Calculate delta since last call
        let delta_ms = match self.last_call.get() {
            Some(last) => now.duration_since(last).as_micros() as f64 / 1000.0,
            None => 0.0,
        };

        // Log every call for first 20, then every 50th
        if count < 20 || count % 50 == 0 {
            let status = if moved { "MOVED" } else { "STATIONARY" };
            println!(
                "[DragOver #{:>4}] {} at ({:>4}, {:>4}) delta: {:>6.2}ms",
                count, status, pt.x, pt.y, delta_ms
            );
        }

        self.last_pt.set(Some(*pt));
        self.last_call.set(Some(now));

        unsafe { *pdweffect = DROPEFFECT_COPY };
        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        let count = DRAG_OVER_COUNT.load(Ordering::Relaxed);
        let elapsed = START_TIME
            .get()
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        let rate = if elapsed > 0.0 {
            count as f64 / elapsed
        } else {
            0.0
        };

        println!(
            "[DragLeave] Total DragOver calls: {}, elapsed: {:.2}s, rate: {:.1}/sec",
            count, elapsed, rate
        );
        Ok(())
    }

    fn Drop(
        &self,
        _pdataobj: Ref<'_, IDataObject>,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let count = DRAG_OVER_COUNT.load(Ordering::Relaxed);
        let elapsed = START_TIME
            .get()
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        let rate = if elapsed > 0.0 {
            count as f64 / elapsed
        } else {
            0.0
        };

        println!("[Drop] at ({}, {})", pt.x, pt.y);
        println!(
            "[Drop] Total DragOver calls: {}, elapsed: {:.2}s, rate: {:.1}/sec",
            count, elapsed, rate
        );

        unsafe { *pdweffect = DROPEFFECT_NONE };
        Ok(())
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

thread_local! {
    static DROP_TARGET_REGISTERED: Cell<bool> = const { Cell::new(false) };
}

fn register_drop_target(hwnd: HWND) -> windows::core::Result<()> {
    DROP_TARGET_REGISTERED.with(|registered| {
        if registered.get() {
            return Ok(());
        }

        let target = MinimalDropTarget {
            last_pt: Cell::new(None),
            last_call: Cell::new(None),
        };
        let target_interface: IDropTarget = target.into();
        let result = unsafe { RegisterDragDrop(hwnd, &target_interface) };
        if result.is_ok() {
            registered.set(true);
            println!("[Init] Drop target registered");
        }
        result
    })
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let text = b"Drag a file here - watch console for DragOver timing";
                let _ = TextOutA(hdc, 20, 20, text);
                let text2 = b"Hold mouse STATIONARY to test continuous polling";
                let _ = TextOutA(hdc, 20, 50, text2);
                let _ = EndPaint(hwnd, &ps);
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

fn main() {
    unsafe {
        // Initialize OLE (required for drag-drop)
        let ole_result = OleInitialize(None);
        if ole_result.is_err() {
            eprintln!("OleInitialize failed: {:?}", ole_result);
            return;
        }

        let instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .expect("GetModuleHandleW failed");

        let class_name = to_wide("NativeDropTest\0");
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

        let window_name = to_wide("Native IDropTarget Test - NO WebView2\0");
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

        // Register drop target
        if let Err(e) = register_drop_target(hwnd) {
            eprintln!("Failed to register drop target: {:?}", e);
            return;
        }

        println!("=== Native IDropTarget Test (NO WebView2) ===");
        println!("Drag a file onto the window and hold it stationary.");
        println!("Watch for STATIONARY logs - if they appear continuously,");
        println!("then OLE DoDragDrop polls regardless of mouse movement.");
        println!();

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = DispatchMessageW(&msg);
        }

        OleUninitialize();
    }
}
