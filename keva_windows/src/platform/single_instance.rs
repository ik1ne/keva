//! Single instance enforcement using a named mutex.

use crate::platform::handlers::show_and_focus_window;
use crate::platform::wm;
use windows::Win32::{
    Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND},
    System::Threading::{CreateMutexW, ReleaseMutex},
    UI::WindowsAndMessaging::{FindWindowW, SendMessageW},
};
use windows::core::PCWSTR;
use windows_strings::w;

#[cfg(debug_assertions)]
const MUTEX_NAME: PCWSTR = w!("Local\\Keva_SingleInstance_Debug");

#[cfg(not(debug_assertions))]
const MUTEX_NAME: PCWSTR = w!("Local\\Keva_SingleInstance");

/// Guard that releases the mutex when dropped.
pub struct SingleInstanceGuard(HANDLE);

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.0);
            let _ = CloseHandle(self.0);
        }
    }
}

/// Checks if this is the only running instance.
///
/// Returns `Ok(guard)` if first instance. Returns `Err(())` if another instance
/// exists (activates it before returning).
pub fn check_single_instance(class_name: PCWSTR) -> Result<SingleInstanceGuard, ()> {
    let handle = unsafe { CreateMutexW(None, false, MUTEX_NAME) }.map_err(|_| ())?;

    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        let _ = unsafe { CloseHandle(handle) };
        activate_existing_window(class_name);
        return Err(());
    }

    Ok(SingleInstanceGuard(handle))
}

fn activate_existing_window(class_name: PCWSTR) {
    let Ok(hwnd) = (unsafe { FindWindowW(class_name, None) }) else {
        return;
    };
    if hwnd == HWND::default() {
        return;
    }

    let _ = unsafe { SendMessageW(hwnd, wm::ACTIVATE_INSTANCE, None, None) };
}
