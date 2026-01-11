//! Launch at Login registry operations.
//!
//! Manages the `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` registry key
//! to enable or disable automatic launch at user login.

use std::env;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ, RegCloseKey, RegDeleteValueW,
    RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::w;

const RUN_KEY_PATH: windows::core::PCWSTR = w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run");

#[cfg(debug_assertions)]
const VALUE_NAME: windows::core::PCWSTR = w!("Keva (Debug)");

#[cfg(not(debug_assertions))]
const VALUE_NAME: windows::core::PCWSTR = w!("Keva");

/// Returns true if Keva is set to launch at login.
pub fn is_launch_at_login_enabled() -> bool {
    unsafe {
        let mut hkey = HKEY::default();
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY_PATH,
            Some(0),
            KEY_READ,
            &mut hkey,
        );
        if result.is_err() {
            return false;
        }

        let mut data_type = REG_SZ;
        let mut data_size = 0u32;
        let query_result = RegQueryValueExW(
            hkey,
            VALUE_NAME,
            None,
            Some(&mut data_type),
            None,
            Some(&mut data_size),
        );

        let _ = RegCloseKey(hkey);

        query_result.is_ok() && data_size > 0
    }
}

/// Enables launch at login by adding the registry entry.
pub fn enable_launch_at_login() -> bool {
    let exe_path = match env::current_exe() {
        Ok(path) => path,
        Err(_) => return false,
    };

    let path_str = exe_path.to_string_lossy();
    let path_wide: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let mut hkey = HKEY::default();
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY_PATH,
            Some(0),
            KEY_WRITE,
            &mut hkey,
        );
        if result.is_err() {
            return false;
        }

        let byte_len = (path_wide.len() * 2) as u32;
        let set_result = RegSetValueExW(
            hkey,
            VALUE_NAME,
            Some(0),
            REG_SZ,
            Some(std::slice::from_raw_parts(
                path_wide.as_ptr() as *const u8,
                byte_len as usize,
            )),
        );

        let _ = RegCloseKey(hkey);

        set_result.is_ok()
    }
}

/// Disables launch at login by removing the registry entry.
pub fn disable_launch_at_login() -> bool {
    unsafe {
        let mut hkey = HKEY::default();
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY_PATH,
            Some(0),
            KEY_WRITE,
            &mut hkey,
        );
        if result.is_err() {
            return false;
        }

        let delete_result = RegDeleteValueW(hkey, VALUE_NAME);
        let _ = RegCloseKey(hkey);

        delete_result.is_ok()
    }
}
