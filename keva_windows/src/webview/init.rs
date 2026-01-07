//! WebView2 initialization.

use super::bridge::handle_webview_message;
use super::messages::OutgoingMessage;
use super::{WEBVIEW, WebView, wm};
use crate::keva_worker::{Request, get_data_path};
use crate::platform::composition::CompositionHost;
use crate::platform::drag_out::handle_drag_starting;
use crate::render::theme::Theme;
use std::ffi::c_void;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;
#[cfg(debug_assertions)]
use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CHANNEL_SEARCH_KIND_LEAST_STABLE;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_ALLOW, CreateCoreWebView2EnvironmentWithOptions,
    ICoreWebView2, ICoreWebView2_3, ICoreWebView2CompositionController5, ICoreWebView2Controller,
    ICoreWebView2Environment, ICoreWebView2Environment3, ICoreWebView2EnvironmentOptions,
    ICoreWebView2Settings9, ICoreWebView2WebMessageReceivedEventArgs,
};
use webview2_com::{
    CoreWebView2EnvironmentOptions, CreateCoreWebView2CompositionControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler, CursorChangedEventHandler,
    DragStartingEventHandler, NavigationStartingEventHandler, WebMessageReceivedEventHandler,
};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{HCURSOR, PostMessageW, SW_SHOWNORMAL, SetCursor};
use windows::core::{Interface, PWSTR, w};

#[expect(clippy::too_many_arguments)]
pub fn init_webview(
    hwnd: HWND,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    theme: Theme,
    request_tx: Sender<Request>,
    response_rx: Receiver<OutgoingMessage>,
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
                    response_rx,
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
    response_rx: Receiver<OutgoingMessage>,
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

                    // Map virtual host to source directory for file-based loading
                    if let Ok(wv3) = wv.webview.cast::<ICoreWebView2_3>() {
                        // UI files
                        let _ = wv3.SetVirtualHostNameToFolderMapping(
                            w!("keva.local"),
                            w!("../../keva_windows/src/webview/ui"),
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
                    let _ = wv.webview.Navigate(w!("http://keva.local/index.html"));

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

                    // Start forwarder thread now that WEBVIEW is set
                    start_forwarder_thread(hwnd, response_rx);

                    Ok(())
                },
            )),
        );
    }
}

/// Spawns a thread that forwards worker responses to WebView via UI thread.
///
/// WebView2 requires PostWebMessageAsJson to be called from the UI thread.
/// This thread serializes messages and posts wm::WEBVIEW_MESSAGE to marshal
/// the call to the UI thread's wndproc.
fn start_forwarder_thread(hwnd: HWND, response_rx: Receiver<OutgoingMessage>) {
    let hwnd_raw = hwnd.0 as isize;
    thread::spawn(move || {
        let hwnd = HWND(hwnd_raw as *mut _);
        for msg in response_rx {
            let json = serde_json::to_string(&msg).expect("Failed to serialize message");
            let ptr = Box::into_raw(Box::new(json));
            unsafe {
                let _ = PostMessageW(
                    Some(hwnd),
                    wm::WEBVIEW_MESSAGE,
                    WPARAM(0),
                    LPARAM(ptr as isize),
                );
            }
        }
    });
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

            // Enable CSS app-region: drag support for window dragging
            if let Ok(settings9) = settings.cast::<ICoreWebView2Settings9>() {
                let _ = settings9.SetIsNonClientRegionSupportEnabled(true);
            }
        }

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
                if uri.starts_with("http://keva.local/")
                    || uri.starts_with("https://keva-data.local/")
                {
                    return Ok(());
                }

                // Cancel navigation and handle externally
                let _ = args.SetCancel(true);

                if let Some(relative) = uri.strip_prefix("att:") {
                    // att:{keyHash}/{encodedFilename} -> open file with default app
                    let decoded = percent_encoding::percent_decode_str(relative)
                        .decode_utf8_lossy();
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
