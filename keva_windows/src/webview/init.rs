//! WebView2 initialization.

use super::bridge::{handle_webview_message, post_message};
use super::{CopyAction, OutgoingMessage, WEBVIEW, WebView};
use crate::keva_worker::{Request, get_data_path};
use crate::platform::clipboard::{read_clipboard, set_pending_file_paths};
use crate::platform::composition::CompositionHost;
use crate::platform::drag_out::handle_drag_starting;
use crate::platform::handlers::get_app_config;
use crate::platform::hotkey::ShortcutBinding;
use crate::platform::tray::IDM_SETTINGS;
use crate::render::theme::Theme;
use std::ffi::c_void;
use std::sync::mpsc::Sender;
#[cfg(debug_assertions)]
use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CHANNEL_SEARCH_KIND_LEAST_STABLE;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_ALLOW, COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN,
    COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN, COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ,
    COREWEBVIEW2_PERMISSION_STATE_ALLOW, CreateCoreWebView2EnvironmentWithOptions,
    ICoreWebView2, ICoreWebView2_3, ICoreWebView2AcceleratorKeyPressedEventArgs,
    ICoreWebView2CompositionController5, ICoreWebView2Controller, ICoreWebView2Environment,
    ICoreWebView2Environment3, ICoreWebView2EnvironmentOptions,
    ICoreWebView2PermissionRequestedEventArgs, ICoreWebView2Settings3, ICoreWebView2Settings4,
    ICoreWebView2Settings5, ICoreWebView2Settings6, ICoreWebView2Settings8, ICoreWebView2Settings9,
    ICoreWebView2WebMessageReceivedEventArgs,
};
use webview2_com::{
    AcceleratorKeyPressedEventHandler, CoreWebView2EnvironmentOptions,
    CreateCoreWebView2CompositionControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler, CursorChangedEventHandler,
    DragStartingEventHandler, NavigationStartingEventHandler, PermissionRequestedEventHandler,
    WebMessageReceivedEventHandler,
};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CONTROL, VK_MENU, VK_OEM_COMMA, VK_SHIFT, VK_V,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    HCURSOR, PostMessageW, SW_SHOWNORMAL, SetCursor, WM_COMMAND,
};
use windows::core::{Interface, PWSTR, w};

pub fn init_webview(
    hwnd: HWND,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    theme: Theme,
    request_tx: Sender<Request>,
) {
    // Create DirectComposition host first
    let composition_host = match CompositionHost::new(hwnd) {
        Ok(host) => host,
        Err(e) => {
            eprintln!("[WebView] Failed to create CompositionHost: {:?}", e);
            return;
        }
    };

    let options = CoreWebView2EnvironmentOptions::default();
    // In debug builds, prefer Beta/Dev/Canary channels which have
    // ICoreWebView2CompositionController5 for DragStarting event support
    #[cfg(debug_assertions)]
    unsafe {
        options.set_channel_search_kind(COREWEBVIEW2_CHANNEL_SEARCH_KIND_LEAST_STABLE);
    }
    let options: ICoreWebView2EnvironmentOptions = options.into();

    unsafe {
        let _ = CreateCoreWebView2EnvironmentWithOptions(
            None,
            None,
            &options,
            &CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(move |_error, env| {
                let Some(env) = env else { return Ok(()) };
                create_composition_controller(
                    hwnd,
                    x,
                    y,
                    width,
                    height,
                    theme,
                    env,
                    request_tx,
                    composition_host,
                );
                Ok(())
            })),
        );
    }
}

#[expect(clippy::too_many_arguments)]
fn create_composition_controller(
    hwnd: HWND,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    theme: Theme,
    env: ICoreWebView2Environment,
    request_tx: Sender<Request>,
    composition_host: CompositionHost,
) {
    unsafe {
        // Cast to Environment3 for CompositionController support
        let Ok(env3) = env.cast::<ICoreWebView2Environment3>() else {
            eprintln!("[WebView] ICoreWebView2Environment3 not available");
            return;
        };

        let env_for_webview = env.clone();
        let _ = env3.CreateCoreWebView2CompositionController(
            hwnd,
            &CreateCoreWebView2CompositionControllerCompletedHandler::create(Box::new(
                move |_error, composition_controller| {
                    let Some(composition_controller) = composition_controller else {
                        eprintln!("[WebView] CompositionController creation failed");
                        return Ok(());
                    };

                    // Set the root visual target for DirectComposition
                    let root_visual = composition_host.root_visual();
                    if let Err(e) = composition_controller.SetRootVisualTarget(root_visual) {
                        eprintln!("[WebView] SetRootVisualTarget failed: {:?}", e);
                        return Ok(());
                    }

                    // Subscribe to cursor change events for immediate feedback
                    let comp_controller_for_cursor = composition_controller.clone();
                    let mut cursor_token = 0i64;
                    let _ = composition_controller.add_CursorChanged(
                        &CursorChangedEventHandler::create(Box::new(move |_sender, _args| {
                            let mut cursor = HCURSOR::default();
                            if comp_controller_for_cursor.Cursor(&mut cursor).is_ok() {
                                SetCursor(Some(cursor));
                            }
                            Ok(())
                        })),
                        &mut cursor_token,
                    );

                    // Subscribe to DragStarting for attachment drag-out to external apps
                    if let Ok(cc5) =
                        composition_controller.cast::<ICoreWebView2CompositionController5>()
                    {
                        let mut drag_token = 0i64;
                        let _ = cc5.add_DragStarting(
                            &DragStartingEventHandler::create(Box::new(move |_sender, args| {
                                if let Some(args) = args
                                    && handle_drag_starting(&args).unwrap_or(false)
                                {
                                    let _ = args.SetHandled(true);
                                }
                                Ok(())
                            })),
                            &mut drag_token,
                        );
                    }

                    // Get the base controller interface
                    let Ok(controller) = composition_controller.cast::<ICoreWebView2Controller>()
                    else {
                        eprintln!("[WebView] Failed to cast to ICoreWebView2Controller");
                        return Ok(());
                    };

                    let Some(webview) = setup_webview(controller.clone(), hwnd, request_tx) else {
                        return Ok(());
                    };

                    // Subscribe to AcceleratorKeyPressed to intercept shortcuts
                    {
                        let webview = webview.clone();
                        let mut accel_token = 0i64;
                        let _ =
                            controller.add_AcceleratorKeyPressed(
                                &AcceleratorKeyPressedEventHandler::create(Box::new(
                                    move |_sender,
                                          args: Option<
                                        ICoreWebView2AcceleratorKeyPressedEventArgs,
                                    >| {
                                        if let Some(args) = args {
                                            handle_accelerator_key(hwnd, &webview, &args);
                                        }
                                        Ok(())
                                    },
                                )),
                                &mut accel_token,
                            );
                    }

                    let _ = controller.SetIsVisible(true);

                    let wv = WebView {
                        composition_controller,
                        controller,
                        webview,
                        env: env_for_webview.clone(),
                        composition_host,
                    };
                    wv.set_bounds(x, y, width, height);

                    // Commit composition after WebView visual is attached
                    wv.commit_composition();

                    // Map virtual host to dist directory (Vite build output)
                    if let Ok(wv3) = wv.webview.cast::<ICoreWebView2_3>() {
                        // UI files
                        let _ = wv3.SetVirtualHostNameToFolderMapping(
                            w!("keva.local"),
                            w!("../../keva_windows/src/webview/vite/dist"),
                            COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_ALLOW,
                        );

                        // Data directory (thumbnails, blobs, content)
                        let data_path = get_data_path();
                        let data_path_wide: Vec<u16> = data_path
                            .to_string_lossy()
                            .encode_utf16()
                            .chain(std::iter::once(0))
                            .collect();
                        let _ = wv3.SetVirtualHostNameToFolderMapping(
                            w!("keva-data.local"),
                            PWSTR(data_path_wide.as_ptr() as *mut u16),
                            COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_ALLOW,
                        );
                    }
                    let _ = wv.webview.Navigate(w!("https://keva.local/index.html"));

                    let script = match theme {
                        Theme::Dark => w!("document.documentElement.dataset.theme='dark';"),
                        Theme::Light => w!("document.documentElement.dataset.theme='light';"),
                    };
                    let _ = wv.webview.ExecuteScript(script, None);

                    #[cfg(debug_assertions)]
                    let _ = wv.webview.OpenDevToolsWindow();

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

        if let Ok(settings) = webview.Settings() {
            #[cfg(not(debug_assertions))]
            let _ = settings.SetAreDevToolsEnabled(false);

            // Disable browser context menu (Inspect, Reload, etc.)
            let _ = settings.SetAreDefaultContextMenusEnabled(false);
            // Disable built-in error pages ("This site can't be reached")
            let _ = settings.SetIsBuiltInErrorPageEnabled(false);
            // Disable status bar (shows URL on hover)
            let _ = settings.SetIsStatusBarEnabled(false);

            // Settings3: Disable browser accelerator keys (F5, Ctrl+R, Ctrl+P, etc.)
            if let Ok(settings3) = settings.cast::<ICoreWebView2Settings3>() {
                let _ = settings3.SetAreBrowserAcceleratorKeysEnabled(false);
            }

            // Settings4: Disable form autofill
            if let Ok(settings4) = settings.cast::<ICoreWebView2Settings4>() {
                let _ = settings4.SetIsGeneralAutofillEnabled(false);
            }

            // Settings5: Disable pinch zoom
            if let Ok(settings5) = settings.cast::<ICoreWebView2Settings5>() {
                let _ = settings5.SetIsPinchZoomEnabled(false);
            }

            // Settings6: Disable swipe navigation
            if let Ok(settings6) = settings.cast::<ICoreWebView2Settings6>() {
                let _ = settings6.SetIsSwipeNavigationEnabled(false);
            }

            // Settings8: Disable SmartScreen (not needed for local content)
            if let Ok(settings8) = settings.cast::<ICoreWebView2Settings8>() {
                let _ = settings8.SetIsReputationCheckingRequired(false);
            }

            // Settings9: Enable CSS app-region: drag support for window dragging
            if let Ok(settings9) = settings.cast::<ICoreWebView2Settings9>() {
                let _ = settings9.SetIsNonClientRegionSupportEnabled(true);
            }
        }

        // Auto-grant clipboard permission (needed for navigator.clipboard.writeText)
        let mut perm_token = 0i64;
        let _ = webview.add_PermissionRequested(
            &PermissionRequestedEventHandler::create(Box::new(
                |_webview_opt, args: Option<ICoreWebView2PermissionRequestedEventArgs>| {
                    let Some(args) = args else { return Ok(()) };
                    let mut kind = Default::default();
                    if args.PermissionKind(&mut kind).is_ok()
                        && kind == COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ
                    {
                        let _ = args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW);
                    }
                    Ok(())
                },
            )),
            &mut perm_token,
        );

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

        // Handle navigation for att: links and external URLs
        let mut nav_token = 0i64;
        let _ = webview.add_NavigationStarting(
            &NavigationStartingEventHandler::create(Box::new(move |_webview_opt, args| {
                let Some(args) = args else { return Ok(()) };

                let mut uri_pwstr = PWSTR::null();
                if args.Uri(&mut uri_pwstr).is_err() || uri_pwstr.is_null() {
                    return Ok(());
                }
                let uri = super::bridge::pwstr_to_string(uri_pwstr);
                CoTaskMemFree(Some(uri_pwstr.as_ptr() as *const c_void));

                // Allow internal navigation to our virtual hosts
                if uri.starts_with("https://keva.local/")
                    || uri.starts_with("https://keva-data.local/")
                {
                    return Ok(());
                }

                // Cancel navigation and handle externally
                let _ = args.SetCancel(true);

                if let Some(relative) = uri.strip_prefix("att:") {
                    // att:{keyHash}/{encodedFilename} -> open file with default app
                    let decoded =
                        percent_encoding::percent_decode_str(relative).decode_utf8_lossy();
                    let path = get_data_path().join("blobs").join(decoded.as_ref());
                    let path_wide: Vec<u16> = path
                        .to_string_lossy()
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();
                    ShellExecuteW(
                        None,
                        w!("open"),
                        PWSTR(path_wide.as_ptr() as *mut _),
                        None,
                        None,
                        SW_SHOWNORMAL,
                    );
                } else {
                    // External URL (http, https, mailto, etc.) -> delegate to OS
                    let uri_wide: Vec<u16> = uri.encode_utf16().chain(std::iter::once(0)).collect();
                    ShellExecuteW(
                        None,
                        w!("open"),
                        PWSTR(uri_wide.as_ptr() as *mut _),
                        None,
                        None,
                        SW_SHOWNORMAL,
                    );
                }

                Ok(())
            })),
            &mut nav_token,
        );

        Some(webview)
    }
}

/// Handles accelerator key events intercepted from WebView2.
///
/// This runs when WebView2 is about to process an accelerator key (Ctrl+*, Alt+*, Escape).
/// We intercept specific shortcuts here and call `SetHandled(true)` to prevent WebView
/// from processing them.
fn handle_accelerator_key(
    hwnd: HWND,
    webview: &ICoreWebView2,
    args: &ICoreWebView2AcceleratorKeyPressedEventArgs,
) {
    unsafe {
        // Only handle key down events
        let mut kind =
            webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_KEY_EVENT_KIND(0);
        if args.KeyEventKind(&mut kind).is_err() {
            return;
        }
        if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
            && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
        {
            return;
        }

        // Filter out auto-repeated keys
        let mut status = webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_PHYSICAL_KEY_STATUS::default();
        if args.PhysicalKeyStatus(&mut status).is_ok() && status.WasKeyDown.as_bool() {
            return;
        }

        let mut virtual_key = 0u32;
        if args.VirtualKey(&mut virtual_key).is_err() {
            return;
        }

        let ctrl_down = GetKeyState(VK_CONTROL.0 as i32) < 0;
        let alt_down = GetKeyState(VK_MENU.0 as i32) < 0;
        let shift_down = GetKeyState(VK_SHIFT.0 as i32) < 0;

        // Ctrl+V: check for files in clipboard
        if virtual_key == VK_V.0 as u32 && ctrl_down && !alt_down && !shift_down {
            let content = read_clipboard(hwnd);
            if !content.files.is_empty() {
                let _ = args.SetHandled(true);
                let filenames: Vec<String> = content
                    .files
                    .iter()
                    .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                    .collect();
                set_pending_file_paths(content.files);
                post_message(webview, &OutgoingMessage::FilesPasted { files: filenames });
            }
            // If no files, let WebView handle text paste
            return;
        }

        // Configurable copy shortcuts (parsed on demand from config)
        let config = get_app_config();

        // Copy Markdown
        if let Some(binding) = ShortcutBinding::parse(&config.shortcuts.copy_markdown)
            && binding.matches(virtual_key, ctrl_down, alt_down, shift_down)
        {
            let _ = args.SetHandled(true);
            post_message(
                webview,
                &OutgoingMessage::DoCopy {
                    action: CopyAction::Markdown,
                },
            );
            return;
        }

        // Copy HTML (rendered preview)
        if let Some(binding) = ShortcutBinding::parse(&config.shortcuts.copy_html)
            && binding.matches(virtual_key, ctrl_down, alt_down, shift_down)
        {
            let _ = args.SetHandled(true);
            post_message(
                webview,
                &OutgoingMessage::DoCopy {
                    action: CopyAction::Html,
                },
            );
            return;
        }

        // Copy Files
        if let Some(binding) = ShortcutBinding::parse(&config.shortcuts.copy_files)
            && binding.matches(virtual_key, ctrl_down, alt_down, shift_down)
        {
            let _ = args.SetHandled(true);
            post_message(
                webview,
                &OutgoingMessage::DoCopy {
                    action: CopyAction::Files,
                },
            );
            return;
        }

        // Ctrl+,: Open settings
        if virtual_key == VK_OEM_COMMA.0 as u32 && ctrl_down && !alt_down {
            let _ = args.SetHandled(true);
            let _ = PostMessageW(
                Some(hwnd),
                WM_COMMAND,
                WPARAM(IDM_SETTINGS as usize),
                LPARAM(0),
            );
        }
    }
}
