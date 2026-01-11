//! Window message handlers.

use crate::keva_worker::Request;
use crate::platform::drop_target::revoke_drop_target;
use crate::platform::file_picker::open_file_picker;
use crate::platform::hotkey::{unregister_global_hotkey, update_global_hotkey};
use crate::platform::startup;
use crate::platform::tray::{
    IDM_LAUNCH_AT_LOGIN, IDM_QUIT, IDM_SETTINGS, IDM_SHOW, remove_tray_icon, set_tray_visibility,
    show_tray_menu,
};
use crate::render::theme::{MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, Theme};
use crate::webview::bridge::post_message;
use crate::webview::{FilePickerRequest, OutgoingMessage, WEBVIEW};
use keva_core::types::AppConfig;
use std::sync::RwLock;
use std::sync::atomic::{AtomicIsize, AtomicU8, Ordering};
use std::sync::mpsc::Sender;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_FILE_SYSTEM_HANDLE_PERMISSION_READ_ONLY,
    COREWEBVIEW2_FILE_SYSTEM_HANDLE_PERMISSION_READ_WRITE,
    COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC, ICoreWebView2_23, ICoreWebView2Environment14,
    ICoreWebView2ObjectCollection,
};
use webview2_com::pwstr_from_str;
use windows::Win32::{
    Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute},
    Graphics::Gdi::{
        BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, PAINTSTRUCT,
        RDW_INVALIDATE, RedrawWindow,
    },
    UI::{
        HiDpi::GetDpiForSystem,
        WindowsAndMessaging::{
            GetClientRect, GetSystemMetrics, GetWindowRect, IsWindowVisible, IsZoomed, MINMAXINFO,
            NCCALCSIZE_PARAMS, PostMessageW, PostQuitMessage, SM_CXPADDEDBORDER, SM_CXSIZEFRAME,
            SW_HIDE, SW_SHOW, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE,
            SWP_NOZORDER, SetForegroundWindow, SetWindowPos, ShowWindow, USER_DEFAULT_SCREEN_DPI,
            WM_CLOSE, WM_LBUTTONUP, WM_RBUTTONUP, WVR_VALIDRECTS,
        },
    },
};
use windows::core::Interface;
use windows::core::PCWSTR;
use windows_strings::w;

/// Stores the previously focused window handle to restore focus on Esc.
pub static PREV_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

static CURRENT_THEME: AtomicU8 = AtomicU8::new(0);

/// Cached app config, initialized in run() before message loop.
static APP_CONFIG: RwLock<Option<AppConfig>> = RwLock::new(None);

pub fn set_current_theme(theme: Theme) {
    let value = match theme {
        Theme::Dark => 0,
        Theme::Light => 1,
    };
    CURRENT_THEME.store(value, Ordering::Relaxed);
}

fn get_current_theme() -> Theme {
    match CURRENT_THEME.load(Ordering::Relaxed) {
        0 => Theme::Dark,
        _ => Theme::Light,
    }
}

/// Sets the cached app config. Called on startup and when settings are saved.
pub fn set_app_config(config: AppConfig) {
    if let Ok(mut guard) = APP_CONFIG.write() {
        *guard = Some(config);
    }
}

/// Returns a clone of the cached app config, or default if not initialized.
pub fn get_app_config() -> AppConfig {
    APP_CONFIG
        .read()
        .ok()
        .and_then(|guard| guard.clone())
        .unwrap_or_default()
}

pub fn scale_for_dpi(logical: i32, dpi: u32) -> i32 {
    (logical as i64 * dpi as i64 / USER_DEFAULT_SCREEN_DPI as i64) as i32
}

/// WM_CREATE: Enable rounded corners and trigger frame recalculation.
pub fn on_create(hwnd: HWND) -> LRESULT {
    unsafe {
        // Windows 11 rounded corners
        let preference = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const _,
            size_of_val(&preference) as u32,
        );

        // SWP_FRAMECHANGED triggers WM_NCCALCSIZE to set up borderless frame
        let mut rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut rect);
        let _ = SetWindowPos(
            hwnd,
            None,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER,
        );
    }
    LRESULT(0)
}

/// WM_GETMINMAXINFO: Enforce minimum window size during resize.
pub fn on_getminmaxinfo(lparam: LPARAM) -> LRESULT {
    // lparam points to MINMAXINFO struct
    let info = lparam.0 as *mut MINMAXINFO;
    if !info.is_null() {
        unsafe {
            let dpi = GetDpiForSystem();
            (*info).ptMinTrackSize.x = scale_for_dpi(MIN_WINDOW_WIDTH, dpi);
            (*info).ptMinTrackSize.y = scale_for_dpi(MIN_WINDOW_HEIGHT, dpi);
        }
    }
    LRESULT(0)
}

/// WM_NCCALCSIZE: Remove non-client area to create borderless window.
pub fn on_nccalcsize(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // wparam == 0: simple request, just return 0 to remove non-client area
    if wparam.0 == 0 {
        return LRESULT(0);
    }

    // wparam != 0: detailed request during resize
    let params = lparam.0 as *mut NCCALCSIZE_PARAMS;
    if !params.is_null() {
        unsafe {
            // When maximized, Windows extends window beyond screen edges.
            // Compensate by adjusting client area inward by the frame size.
            if IsZoomed(hwnd).as_bool() {
                let frame = GetSystemMetrics(SM_CXSIZEFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER);
                (*params).rgrc[0].left += frame;
                (*params).rgrc[0].top += frame;
                (*params).rgrc[0].right -= frame;
                (*params).rgrc[0].bottom -= frame;
            }

            // rgrc[1], rgrc[2] = old client/window areas, set to 1px to disable BitBlt
            (*params).rgrc[1] = RECT {
                left: 0,
                top: 0,
                right: 1,
                bottom: 1,
            };
            (*params).rgrc[2] = (*params).rgrc[1];
        }
    }
    // WVR_VALIDRECTS: we've set valid rectangles, don't need system to calculate
    LRESULT(WVR_VALIDRECTS as isize)
}

/// WM_ACTIVATE: Track previously focused window to restore on Esc.
pub fn on_activate(wparam: WPARAM, lparam: LPARAM) {
    let activating = (wparam.0 & 0xFFFF) != 0;
    let previous_window = lparam.0;
    if activating && previous_window != 0 {
        PREV_FOREGROUND.store(previous_window, Ordering::Relaxed);
    }
}

/// WM_SETFOCUS: Transfer focus to WebView2 CompositionController.
pub fn on_setfocus() {
    if let Some(wv) = WEBVIEW.get() {
        let _ = unsafe {
            wv.controller
                .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
        };
    }
}

/// Shows window, brings to foreground, and signals WebView to restore focus.
pub fn show_and_focus_window(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
    if let Some(wv) = WEBVIEW.get() {
        let _ = unsafe {
            wv.controller
                .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
        };
        post_message(&wv.webview, &OutgoingMessage::Focus);
    }
}

/// WM_TRAYICON: Handle system tray icon clicks.
pub fn on_trayicon(hwnd: HWND, lparam: LPARAM) -> LRESULT {
    let mouse_msg = (lparam.0 & 0xFFFF) as u32;
    if mouse_msg == WM_LBUTTONUP {
        let is_visible = unsafe { IsWindowVisible(hwnd).as_bool() };
        if is_visible {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        } else {
            show_and_focus_window(hwnd);
        }
    } else if mouse_msg == WM_RBUTTONUP {
        show_tray_menu(hwnd);
    }
    LRESULT(0)
}

/// WM_COMMAND: Handle menu commands from tray context menu.
pub fn on_command(hwnd: HWND, wparam: WPARAM) -> LRESULT {
    let cmd_id = (wparam.0 & 0xFFFF) as u32;
    match cmd_id {
        IDM_SHOW => show_and_focus_window(hwnd),
        IDM_SETTINGS => {
            open_settings();
        }
        IDM_LAUNCH_AT_LOGIN => {
            // Toggle launch at login
            if startup::is_launch_at_login_enabled() {
                startup::disable_launch_at_login();
            } else {
                startup::enable_launch_at_login();
            }
        }
        IDM_QUIT => unsafe {
            let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
        },
        _ => {}
    }
    LRESULT(0)
}

/// Opens settings panel in WebView with current config.
fn open_settings() {
    let config = get_app_config();
    let launch_at_login = startup::is_launch_at_login_enabled();

    if let Some(wv) = WEBVIEW.get() {
        post_message(
            &wv.webview,
            &OutgoingMessage::OpenSettings {
                config,
                launch_at_login,
            },
        );
    }
}

/// Applies settings changes that take effect immediately.
pub fn apply_settings(
    hwnd: HWND,
    config: &AppConfig,
    launch_at_login: bool,
    request_tx: &Sender<Request>,
) {
    // Update cached config
    set_app_config(config.clone());

    // Apply theme
    let theme = match config.general.theme {
        keva_core::types::Theme::Dark => Theme::Dark,
        keva_core::types::Theme::Light => Theme::Light,
        keva_core::types::Theme::System => Theme::detect_system(),
    };

    if let Some(wv) = WEBVIEW.get() {
        let theme_str = match theme {
            Theme::Dark => "dark",
            Theme::Light => "light",
        };
        post_message(
            &wv.webview,
            &OutgoingMessage::Theme {
                theme: theme_str.to_string(),
            },
        );
    }

    // Update GC config in worker thread
    let _ = request_tx.send(Request::UpdateGcConfig {
        lifecycle: config.lifecycle.clone(),
    });

    // Update tray icon visibility
    set_tray_visibility(hwnd, config.general.show_tray_icon);

    // Update launch at login (registry)
    if launch_at_login {
        startup::enable_launch_at_login();
    } else {
        startup::disable_launch_at_login();
    }

    // Update global hotkey if changed
    if !update_global_hotkey(hwnd, &config.shortcuts.global_shortcut) {
        // Notify user that shortcut registration failed
        if let Some(wv) = WEBVIEW.get() {
            post_message(
                &wv.webview,
                &OutgoingMessage::Toast {
                    message: format!(
                        "Shortcut '{}' is in use by another application",
                        config.shortcuts.global_shortcut
                    ),
                },
            );
        }
    }
}

/// WM_SIZE: Resize WebView to fill entire client area.
pub fn on_size(_wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let width = (lparam.0 & 0xFFFF) as i32;
    let height = ((lparam.0 >> 16) & 0xFFFF) as i32;

    // WebView fills entire client area; resize borders overlap content edges
    // (hit-testing still handles resize via WM_NCHITTEST)
    if let Some(wv) = WEBVIEW.get() {
        wv.set_bounds(0, 0, width, height);
        wv.commit_composition();
    }

    LRESULT(0)
}

/// WM_PAINT: Paint background under WebView (covers DWM extended frame).
pub fn on_paint(hwnd: HWND) -> LRESULT {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);

        let bg_color = match get_current_theme() {
            Theme::Dark => COLORREF(0x001a1a1a),
            Theme::Light => COLORREF(0x00ffffff),
        };
        let brush = CreateSolidBrush(bg_color);

        let mut rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);
        FillRect(hdc, &rect, brush);

        let _ = DeleteObject(brush.into());
        let _ = EndPaint(hwnd, &ps);
    }
    LRESULT(0)
}

/// WM_DESTROY: Clean up and exit application.
pub fn on_destroy(hwnd: HWND) -> LRESULT {
    revoke_drop_target(hwnd);
    unregister_global_hotkey(hwnd);
    unsafe {
        remove_tray_icon(hwnd);
        PostQuitMessage(0);
    }
    LRESULT(0)
}

/// WM_SETTINGCHANGE: Detect system theme changes.
pub fn on_settingchange(hwnd: HWND, lparam: LPARAM) -> LRESULT {
    // lparam points to the setting name as a wide string
    if lparam.0 != 0 {
        let setting_ptr = lparam.0 as *const u16;
        let setting = PCWSTR::from_raw(setting_ptr);

        // "ImmersiveColorSet" is broadcast when system theme changes
        if unsafe { setting.as_wide() == w!("ImmersiveColorSet").as_wide() } {
            // Only apply system theme if user preference is "System"
            if get_app_config().general.theme != keva_core::types::Theme::System {
                return LRESULT(0);
            }

            let theme = Theme::detect_system();
            set_current_theme(theme);
            unsafe {
                let _ = RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE);
            }

            if let Some(wv) = WEBVIEW.get() {
                let msg = OutgoingMessage::Theme {
                    theme: theme.as_str().to_string(),
                };
                post_message(&wv.webview, &msg);
            }
        }
    }
    LRESULT(0)
}

/// WM_WEBVIEW_MESSAGE: Forward OutgoingMessage to WebView.
/// Value variant uses PostWebMessageAsJsonWithAdditionalObjects for FileSystemHandle.
pub fn on_webview_message(lparam: LPARAM) -> LRESULT {
    let ptr = lparam.0 as *mut OutgoingMessage;
    if ptr.is_null() {
        return LRESULT(0);
    }

    let msg = unsafe { Box::from_raw(ptr) };

    let Some(wv) = WEBVIEW.get() else {
        return LRESULT(0);
    };

    // Value messages need FileSystemHandle creation
    if let OutgoingMessage::Value {
        ref content_path,
        read_only,
        ..
    } = *msg
    {
        unsafe {
            let Ok(env14) = wv.env.cast::<ICoreWebView2Environment14>() else {
                eprintln!("[FileHandle] ICoreWebView2Environment14 not available");
                return LRESULT(0);
            };
            let Ok(webview23) = wv.webview.cast::<ICoreWebView2_23>() else {
                eprintln!("[FileHandle] ICoreWebView2_23 not available");
                return LRESULT(0);
            };

            let path_str = content_path.to_string_lossy();
            let path_pwstr = pwstr_from_str(&path_str);
            let permission = if read_only {
                COREWEBVIEW2_FILE_SYSTEM_HANDLE_PERMISSION_READ_ONLY
            } else {
                COREWEBVIEW2_FILE_SYSTEM_HANDLE_PERMISSION_READ_WRITE
            };

            let handle = match env14.CreateWebFileSystemFileHandle(path_pwstr, permission) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("[FileHandle] CreateWebFileSystemFileHandle failed: {:?}", e);
                    return LRESULT(0);
                }
            };

            let handle_iunknown: windows::core::IUnknown = match handle.cast() {
                Ok(u) => u,
                Err(e) => {
                    eprintln!("[FileHandle] Failed to cast handle to IUnknown: {:?}", e);
                    return LRESULT(0);
                }
            };
            let mut items = [Some(handle_iunknown)];
            let mut collection: Option<ICoreWebView2ObjectCollection> = None;

            if let Err(e) = env14.CreateObjectCollection(1, items.as_mut_ptr(), &mut collection) {
                eprintln!("[FileHandle] CreateObjectCollection failed: {:?}", e);
                return LRESULT(0);
            }

            let Some(objects) = collection else {
                eprintln!("[FileHandle] CreateObjectCollection returned None");
                return LRESULT(0);
            };

            let json = serde_json::to_string(&*msg).expect("Failed to serialize message");
            let json_pwstr = pwstr_from_str(&json);

            if let Err(e) =
                webview23.PostWebMessageAsJsonWithAdditionalObjects(json_pwstr, &*objects)
            {
                eprintln!(
                    "[FileHandle] PostWebMessageAsJsonWithAdditionalObjects failed: {:?}",
                    e
                );
            }
        }
    } else {
        let json = serde_json::to_string(&*msg).expect("Failed to serialize message");
        let msg_pwstr = pwstr_from_str(&json);
        let _ = unsafe { wv.webview.PostWebMessageAsJson(msg_pwstr) };
    }

    LRESULT(0)
}

/// WM_OPEN_FILE_PICKER: Open file picker and send selected files to worker.
pub fn on_open_file_picker(hwnd: HWND, lparam: LPARAM) -> LRESULT {
    let ptr = lparam.0 as *mut FilePickerRequest;
    if ptr.is_null() {
        return LRESULT(0);
    }

    let request = unsafe { Box::from_raw(ptr) };
    let files = open_file_picker(hwnd);

    if !files.is_empty() {
        let _ = request.request_tx.send(Request::FilesSelected {
            key: request.key,
            files,
        });
    }

    LRESULT(0)
}
