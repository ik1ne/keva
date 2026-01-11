//! WebView message bridge.

use super::messages::IncomingMessage;
use super::{FilePickerRequest, OutgoingMessage, WEBVIEW};
use crate::keva_worker::{Request, get_data_path};
use crate::platform::clipboard::{take_pending_file_paths, write_files};
use crate::platform::handlers::PREV_FOREGROUND;
use crate::platform::wm;
use crate::render::theme::Theme;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2;
use webview2_com::pwstr_from_str;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_ICONWARNING, MB_YESNO, MessageBoxW, PostMessageW, PostQuitMessage, SW_HIDE,
    SetForegroundWindow, ShowWindow,
};
use windows::core::PWSTR;
use windows_strings::w;

pub fn handle_webview_message(msg: &str, parent_hwnd: HWND, request_tx: &Sender<Request>) {
    let Ok(message) = serde_json::from_str::<IncomingMessage>(msg) else {
        eprintln!("[Bridge] Failed to parse: {}", msg);
        return;
    };

    match message {
        IncomingMessage::Ready => {
            // Send theme from config (or detect system if set to System)
            if let Some(wv) = WEBVIEW.get() {
                let config_path = keva_core::types::AppConfig::path(&get_data_path());
                let config = keva_core::types::AppConfig::load(&config_path).unwrap_or_default();
                let theme = match config.general.theme {
                    keva_core::types::Theme::Dark => Theme::Dark,
                    keva_core::types::Theme::Light => Theme::Light,
                    keva_core::types::Theme::System => Theme::detect_system(),
                };
                post_message(
                    &wv.webview,
                    &OutgoingMessage::Theme {
                        theme: theme.as_str().to_string(),
                    },
                );
            }
            // Worker responds with CoreReady after init is done
            let _ = request_tx.send(Request::WebviewReady);
        }
        IncomingMessage::Search { query } => {
            let _ = request_tx.send(Request::Search { query });
        }
        IncomingMessage::Select { key } => {
            let _ = request_tx.send(Request::GetValue { key });
        }
        IncomingMessage::Save { key, content } => {
            let _ = request_tx.send(Request::Save { key, content });
        }
        IncomingMessage::Create { key } => {
            let _ = request_tx.send(Request::Create { key });
        }
        IncomingMessage::Rename {
            old_key,
            new_key,
            force,
        } => {
            let _ = request_tx.send(Request::Rename {
                old_key,
                new_key,
                force,
            });
        }
        IncomingMessage::Trash { key } => {
            let _ = request_tx.send(Request::Trash { key });
        }
        IncomingMessage::Restore { key } => {
            let _ = request_tx.send(Request::Restore { key });
        }
        IncomingMessage::Purge { key } => {
            let _ = request_tx.send(Request::Purge { key });
        }
        IncomingMessage::Touch { key } => {
            let _ = request_tx.send(Request::Touch { key });
        }
        IncomingMessage::Hide => {
            let _ = unsafe { ShowWindow(parent_hwnd, SW_HIDE) };
            // Restore focus to the previously focused window
            let prev = PREV_FOREGROUND.load(Ordering::Relaxed);
            if prev != 0 {
                let prev_hwnd = HWND(prev as *mut _);
                let _ = unsafe { SetForegroundWindow(prev_hwnd) };
            }
            // Run maintenance on window hide
            let _ = request_tx.send(Request::Maintenance { force: true });
        }
        IncomingMessage::ShutdownAck => {
            let _ = request_tx.send(Request::Shutdown);
        }
        IncomingMessage::ShutdownBlocked => {
            let result = unsafe {
                MessageBoxW(
                    Some(parent_hwnd),
                    w!("File copy in progress. Exit anyway?"),
                    w!("Keva"),
                    MB_YESNO | MB_ICONWARNING,
                )
            };
            if result == IDYES {
                unsafe { PostQuitMessage(0) };
            }
        }
        IncomingMessage::OpenFilePicker { key } => {
            // Post to UI thread - file picker must run on UI thread
            let request = Box::new(FilePickerRequest {
                key,
                request_tx: request_tx.clone(),
            });
            let ptr = Box::into_raw(request);
            unsafe {
                if PostMessageW(
                    Some(parent_hwnd),
                    wm::OPEN_FILE_PICKER,
                    WPARAM(0),
                    LPARAM(ptr as isize),
                )
                .is_err()
                {
                    drop(Box::from_raw(ptr));
                }
            }
        }
        IncomingMessage::AddAttachments { key, files } => {
            let _ = request_tx.send(Request::AddAttachments { key, files });
        }
        IncomingMessage::RemoveAttachment { key, filename } => {
            let _ = request_tx.send(Request::RemoveAttachment { key, filename });
        }
        IncomingMessage::RenameAttachment {
            key,
            old_filename,
            new_filename,
            force,
        } => {
            let _ = request_tx.send(Request::RenameAttachment {
                key,
                old_filename,
                new_filename,
                force,
            });
        }
        IncomingMessage::AddFiles { key, files } => {
            // Get cached paths (from drop or clipboard) and match by index
            let cached_paths = take_pending_file_paths();
            let resolved_files: Vec<(std::path::PathBuf, String)> = files
                .into_iter()
                .filter_map(|(index, filename)| {
                    cached_paths.get(index).map(|path| (path.clone(), filename))
                })
                .collect();

            if !resolved_files.is_empty() {
                let _ = request_tx.send(Request::AddFiles {
                    key,
                    files: resolved_files,
                });
            }
        }
        IncomingMessage::CopyFiles { key, filenames } => {
            // Build full paths and write to clipboard
            let success = if let Ok(key_obj) = keva_core::types::Key::try_from(key.as_str()) {
                let key_hash = keva_core::core::KevaCore::key_to_path(&key_obj);
                let blobs_dir = get_data_path().join("blobs").join(&key_hash);
                let paths: Vec<std::path::PathBuf> = filenames
                    .into_iter()
                    .map(|name| blobs_dir.join(&name))
                    .filter(|path| path.exists())
                    .collect();

                if paths.is_empty() {
                    false
                } else {
                    write_files(parent_hwnd, &paths)
                }
            } else {
                false
            };

            if let Some(wv) = WEBVIEW.get() {
                post_message(&wv.webview, &OutgoingMessage::CopyResult { success });
            }
        }
        IncomingMessage::SaveSettings {
            config,
            launch_at_login,
        } => {
            // Save config to file
            let config_path = keva_core::types::AppConfig::path(&get_data_path());
            let _ = config.save(&config_path);

            // Apply settings (launch_at_login is written to registry in apply_settings)
            crate::platform::handlers::apply_settings(
                parent_hwnd,
                &config,
                launch_at_login,
                request_tx,
            );
        }
        IncomingMessage::WelcomeResult { launch_at_login } => {
            // Update cached config
            let mut config = crate::platform::handlers::get_app_config();
            config.general.welcome_shown = true;
            crate::platform::handlers::set_app_config(config.clone());

            // Save to file
            let config_path = keva_core::types::AppConfig::path(&get_data_path());
            let _ = config.save(&config_path);

            // Update launch at login in registry
            if launch_at_login {
                crate::platform::startup::enable_launch_at_login();
            }
        }
    }
}

pub fn post_message(wv: &ICoreWebView2, msg: &OutgoingMessage) {
    let json = serde_json::to_string(msg).expect("Failed to serialize message");
    let msg_pwstr = pwstr_from_str(&json);
    let _ = unsafe { wv.PostWebMessageAsJson(msg_pwstr) };
}

pub fn pwstr_to_string(pwstr: PWSTR) -> String {
    if pwstr.is_null() {
        return String::new();
    }
    unsafe { pwstr.to_string().unwrap_or_default() }
}
