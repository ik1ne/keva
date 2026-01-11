//! Global hotkey registration using Win32 RegisterHotKey.
//!
//! Shortcut format: `[Ctrl+][Alt+][Shift+][Win+]<e.code>`
//! where `<e.code>` is the DOM KeyboardEvent.code value (e.g., "KeyA", "Digit1", "F12").
//!
//! Uses the `keycode` crate to convert DOM e.code to Windows scan codes,
//! then `MapVirtualKeyW` to convert scan codes to virtual key codes.

use keycode::{KeyMap, KeyMappingCode};
use std::sync::RwLock;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MAPVK_VSC_TO_VK, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
    MapVirtualKeyW, RegisterHotKey, UnregisterHotKey,
};

/// Unique ID for our global hotkey registration.
const HOTKEY_ID: i32 = 1;

/// Currently registered shortcut string (empty if none registered).
static CURRENT_SHORTCUT: RwLock<String> = RwLock::new(String::new());

/// Represents a parsed keyboard shortcut with modifiers and virtual key code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutBinding {
    pub modifiers: HOT_KEY_MODIFIERS,
    pub vk_code: u32,
}

impl ShortcutBinding {
    /// Parses a shortcut string like "Ctrl+Alt+KeyK" into a ShortcutBinding.
    ///
    /// Format: `[Ctrl+][Alt+][Shift+][Win+]<e.code>`
    ///
    /// Returns `None` if the shortcut string is empty or invalid.
    pub fn parse(shortcut: &str) -> Option<Self> {
        let shortcut = shortcut.trim();
        if shortcut.is_empty() {
            return None;
        }

        let mut modifiers = HOT_KEY_MODIFIERS(0);
        let mut key_part: Option<&str> = None;

        for part in shortcut.split('+') {
            let part = part.trim();
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= MOD_CONTROL,
                "alt" => modifiers |= MOD_ALT,
                "shift" => modifiers |= MOD_SHIFT,
                "win" | "meta" | "super" => modifiers |= MOD_WIN,
                _ => {
                    if key_part.is_some() {
                        return None;
                    }
                    key_part = Some(part);
                }
            }
        }

        let key = key_part?;
        let vk_code = Self::code_to_vk(key)?;

        Some(Self { modifiers, vk_code })
    }

    /// Converts a DOM e.code string to a Windows virtual key code.
    ///
    /// Uses `keycode` crate to parse e.code → scan code, then `MapVirtualKeyW` for scan → VK.
    fn code_to_vk(code: &str) -> Option<u32> {
        let key_code: KeyMappingCode = code.parse().ok()?;
        let key_map = KeyMap::from(key_code);
        let scan_code = key_map.win as u32;

        if scan_code == 0 {
            return None;
        }

        // Convert scan code to virtual key code
        let vk = unsafe { MapVirtualKeyW(scan_code, MAPVK_VSC_TO_VK) };
        if vk == 0 {
            return None;
        }

        Some(vk)
    }
}

/// Registers the global hotkey from the given shortcut string.
///
/// Returns `true` if registration succeeded or shortcut was empty (no registration needed).
/// Returns `false` if registration failed (shortcut in use by another application).
pub fn register_global_hotkey(hwnd: HWND, shortcut: &str) -> bool {
    let shortcut = shortcut.trim();

    // Empty shortcut means no global hotkey
    if shortcut.is_empty() {
        if let Ok(mut guard) = CURRENT_SHORTCUT.write() {
            *guard = String::new();
        }
        return true;
    }

    let Some(binding) = ShortcutBinding::parse(shortcut) else {
        eprintln!("[Hotkey] Failed to parse shortcut: {}", shortcut);
        return false;
    };

    // Require Ctrl or Alt (matches JS validation in settings.js)
    if (binding.modifiers & (MOD_CONTROL | MOD_ALT)).0 == 0 {
        eprintln!("[Hotkey] Shortcut must include Ctrl or Alt: {}", shortcut);
        return false;
    }

    // Add MOD_NOREPEAT to prevent repeated WM_HOTKEY when held
    let modifiers = binding.modifiers | MOD_NOREPEAT;

    let result = unsafe { RegisterHotKey(Some(hwnd), HOTKEY_ID, modifiers, binding.vk_code) };

    if result.is_ok() {
        if let Ok(mut guard) = CURRENT_SHORTCUT.write() {
            *guard = shortcut.to_string();
        }
        true
    } else {
        eprintln!(
            "[Hotkey] RegisterHotKey failed for '{}': {}",
            shortcut,
            std::io::Error::last_os_error()
        );
        false
    }
}

/// Unregisters the currently registered global hotkey.
pub fn unregister_global_hotkey(hwnd: HWND) {
    let current = CURRENT_SHORTCUT
        .read()
        .map(|g| g.clone())
        .unwrap_or_default();

    if !current.is_empty() {
        let result = unsafe { UnregisterHotKey(Some(hwnd), HOTKEY_ID) };
        if result.is_err() {
            eprintln!(
                "[Hotkey] UnregisterHotKey failed: {}",
                std::io::Error::last_os_error()
            );
        }
        if let Ok(mut guard) = CURRENT_SHORTCUT.write() {
            *guard = String::new();
        }
    }
}

/// Updates the global hotkey if the shortcut has changed.
///
/// Returns `true` if update succeeded (or no change needed).
/// Returns `false` if registration failed.
pub fn update_global_hotkey(hwnd: HWND, new_shortcut: &str) -> bool {
    let new_shortcut = new_shortcut.trim();
    let current = CURRENT_SHORTCUT
        .read()
        .map(|g| g.clone())
        .unwrap_or_default();

    // No change needed
    if current == new_shortcut {
        return true;
    }

    // Unregister old hotkey if any
    if !current.is_empty() {
        let _ = unsafe { UnregisterHotKey(Some(hwnd), HOTKEY_ID) };
    }

    // Register new hotkey
    register_global_hotkey(hwnd, new_shortcut)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_vk(shortcut: &str) -> Option<u32> {
        ShortcutBinding::parse(shortcut).map(|b| b.vk_code)
    }

    #[test]
    fn parse_letters() {
        // KeyA-KeyZ should map to VK 0x41-0x5A
        assert_eq!(parse_vk("Ctrl+KeyA"), Some(0x41));
        assert_eq!(parse_vk("Ctrl+KeyZ"), Some(0x5A));
        assert_eq!(parse_vk("Ctrl+KeyM"), Some(0x4D));
    }

    #[test]
    fn parse_digits() {
        // Digit0-Digit9 should map to VK 0x30-0x39
        assert_eq!(parse_vk("Ctrl+Digit0"), Some(0x30));
        assert_eq!(parse_vk("Ctrl+Digit9"), Some(0x39));
        assert_eq!(parse_vk("Ctrl+Digit5"), Some(0x35));
    }

    #[test]
    fn parse_function_keys() {
        // F1-F12 should map to VK 0x70-0x7B
        assert_eq!(parse_vk("Alt+F1"), Some(0x70));
        assert_eq!(parse_vk("Alt+F12"), Some(0x7B));
    }

    #[test]
    fn parse_arrow_keys() {
        // Arrow keys
        assert_eq!(parse_vk("Ctrl+ArrowUp"), Some(0x26));
        assert_eq!(parse_vk("Ctrl+ArrowDown"), Some(0x28));
        assert_eq!(parse_vk("Ctrl+ArrowLeft"), Some(0x25));
        assert_eq!(parse_vk("Ctrl+ArrowRight"), Some(0x27));
    }

    #[test]
    fn parse_special_keys() {
        assert_eq!(parse_vk("Ctrl+Space"), Some(0x20));
        assert_eq!(parse_vk("Ctrl+Tab"), Some(0x09));
        assert_eq!(parse_vk("Ctrl+Enter"), Some(0x0D));
        assert_eq!(parse_vk("Ctrl+Escape"), Some(0x1B));
        assert_eq!(parse_vk("Ctrl+Backspace"), Some(0x08));
    }

    #[test]
    fn parse_modifiers() {
        let binding = ShortcutBinding::parse("Ctrl+Alt+KeyK").unwrap();
        assert_eq!(binding.modifiers, MOD_CONTROL | MOD_ALT);

        let binding = ShortcutBinding::parse("Ctrl+Shift+KeyA").unwrap();
        assert_eq!(binding.modifiers, MOD_CONTROL | MOD_SHIFT);

        let binding = ShortcutBinding::parse("Alt+Shift+F1").unwrap();
        assert_eq!(binding.modifiers, MOD_ALT | MOD_SHIFT);
    }

    #[test]
    fn parse_empty_returns_none() {
        assert!(ShortcutBinding::parse("").is_none());
        assert!(ShortcutBinding::parse("   ").is_none());
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert!(ShortcutBinding::parse("Ctrl+InvalidKey").is_none());
        assert!(ShortcutBinding::parse("Ctrl+").is_none());
        assert!(ShortcutBinding::parse("Ctrl+KeyA+KeyB").is_none()); // Two keys
    }
}
