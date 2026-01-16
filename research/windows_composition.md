# Building Windows Desktop Apps with Rust's `windows` Crate

The Rust `windows` crate provides comprehensive Win32 bindings that enable native Windows desktop development with type
safety and ergonomic patterns. This guide addresses dark-themed native controls, Direct2D integration, focus management,
and message handling—all critical for building polished desktop applications. **RichEdit controls offer significantly
simpler dark theme support via `EM_SETBKGNDCOLOR`**, while standard EDIT controls require parent-handled
`WM_CTLCOLOREDIT` messages combined with undocumented Windows 10+ dark mode APIs for complete styling.

## Foundation patterns for the Rust `windows` crate

The `windows` crate (distinct from `windows-sys` which offers zero-overhead raw bindings) wraps Win32 handles like
`HWND`, `HDC`, and `HBRUSH` as thin wrapper structs around raw pointers. These handles **do not implement automatic
cleanup**—you must manually call `DeleteObject`, `ReleaseDC`, or `DestroyWindow` as appropriate. The crate requires
`unsafe` blocks for all Win32 API calls since the underlying operations involve raw pointers and manual memory
management.

Window creation follows a standard pattern of registering a window class, creating the window, and running a message
loop:

```rust
use windows::{
    core::*, Win32::Foundation::*, Win32::Graphics::Gdi::*,
    Win32::System::LibraryLoader::GetModuleHandleA,
    Win32::UI::WindowsAndMessaging::*,
};

fn main() -> Result<()> {
    unsafe {
        let instance = GetModuleHandleA(None)?;
        let window_class = s!("window");

        let wc = WNDCLASSA {
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hInstance: instance.into(),
            lpszClassName: window_class,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            ..Default::default()
        };
        RegisterClassA(&wc);

        CreateWindowExA(
            WINDOW_EX_STYLE::default(), window_class, s!("App"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT, CW_USEDEFAULT, 800, 600,
            None, None, instance, None,
        )?;

        let mut message = MSG::default();
        while GetMessageA(&mut message, HWND(0), 0, 0).into() {
            TranslateMessage(&message);
            DispatchMessageA(&message);
        }
        Ok(())
    }
}

extern "system" fn wndproc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcA(window, message, wparam, lparam),
        }
    }
}
```

**Critical Rust-specific pitfall**: String lifetime management with `PCWSTR`. The following pattern creates a dangling
pointer because `HSTRING` is immediately dropped:

```rust
// WRONG - dangling pointer!
let bad = PCWSTR(HSTRING::from("hello").as_ptr());

// CORRECT - keep HSTRING alive
let h_string = HSTRING::from("hello");
let good = PCWSTR(h_string.as_ptr());
```

For string literals, use the `w!()` and `s!()` macros which handle null termination and produce compile-time
conversions. For callbacks, always use `extern "system"` for the correct ABI.

## Dark theme styling for native text controls

Standard EDIT controls rely on `WM_CTLCOLOREDIT` messages sent to the parent window before drawing. The parent returns
an `HBRUSH` for the background and sets text colors via the provided HDC:

```rust
WM_CTLCOLOREDIT => {
let hdc = HDC(wparam.0 as isize);
SetTextColor(hdc, COLORREF(0x00F0F0F0));  // Light gray text
SetBkColor(hdc, COLORREF(0x002D2D2D));    // rgb(45,45,45) background
// Return a static brush - must persist for window lifetime
static DARK_BRUSH: std::sync::OnceLock< HBRUSH > = std::sync::OnceLock::new();
let brush = DARK_BRUSH.get_or_init( | | CreateSolidBrush(COLORREF(0x002D2D2D)));
return LRESULT(brush.0);
}
```

However, **`WM_CTLCOLOREDIT` alone doesn't achieve complete dark theming** because: read-only and disabled controls send
`WM_CTLCOLORSTATIC` instead; borders remain light-themed; scrollbars are unaffected; and initial painting may flash
white before the message is processed.

**RichEdit controls offer a dramatically simpler approach** through `EM_SETBKGNDCOLOR`:

```rust
// Load the RichEdit DLL first
LoadLibraryW(w!("Msftedit.dll")) ?;

// Create RichEdit control with class "RICHEDIT50W"
let rich_edit = CreateWindowExW(
WS_EX_CLIENTEDGE, w!("RICHEDIT50W"), w!(""),
WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | ES_MULTILINE.0),
10, 10, 300, 200, parent, None, instance, None,
) ?;

// Set background directly - no message handling required
SendMessageW(rich_edit, EM_SETBKGNDCOLOR, WPARAM(0), LPARAM(0x002D2D2D));

// Set text color via CHARFORMAT2
let mut cf = CHARFORMAT2W::default ();
cf.cbSize = std::mem::size_of::<CHARFORMAT2W>() as u32;
cf.dwMask = CFM_COLOR;
cf.crTextColor = COLORREF(0x00F0F0F0);
cf.dwEffects = 0;  // Remove CFE_AUTOCOLOR
SendMessageW(rich_edit, EM_SETCHARFORMAT, WPARAM(SCF_ALL.0 as usize),
LPARAM( & cf as * const _ as isize));
```

### Complete dark theme with undocumented Windows 10+ APIs

For thorough dark mode support including scrollbars and borders, Windows 10 1809+ (build **17763**+) exposes
undocumented functions in `uxtheme.dll` accessed by ordinal:

- **Ordinal 132**: `ShouldAppsUseDarkMode` — checks system preference
- **Ordinal 133**: `AllowDarkModeForWindow(HWND, bool)` — enables dark mode for specific window
- **Ordinal 135**: `SetPreferredAppMode(mode)` — sets app-wide preference (`AllowDark = 1`, `ForceDark = 2`)

Apply dark mode to controls with:

```rust
// Call early, before creating windows
SetPreferredAppMode(1);  // AllowDark

// For each control
AllowDarkModeForWindow(edit_hwnd, true);
SetWindowTheme(edit_hwnd, w!("DarkMode_CFD"), PCWSTR::null());  // Single-line EDIT
// Or "DarkMode_Explorer" for multi-line with scrollbars
SendMessageW(edit_hwnd, WM_THEMECHANGED, WPARAM(0), LPARAM(0));
```

For the title bar, use the documented DWM attribute:

```rust
let use_dark: BOOL = TRUE;
DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE,
& use_dark as * const _ as * const c_void,
std::mem::size_of::<BOOL>() as u32);
```

**DWM composition warning**: When using `DwmExtendFrameIntoClientArea`, native controls may exhibit transparency issues.
DWM treats `RGB(0,0,0)` as transparent—use `RGB(1,1,1)` or similar near-black colors to avoid controls becoming
see-through.

## Integrating Direct2D with native Win32 controls

The recommended architecture places an `ID2D1HwndRenderTarget` on the parent window while native controls exist as
separate child HWNDs. The key to preventing Direct2D from overdrawing child controls is the **`WS_CLIPCHILDREN`** style
on the parent:

```rust
let parent = CreateWindowExW(
WINDOW_EX_STYLE::default (),
window_class,
w!("App"),
WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,  // Critical for D2D + controls
CW_USEDEFAULT, CW_USEDEFAULT, 800, 600,
None, None, instance, None,
) ?;
```

When `WS_CLIPCHILDREN` is set, the system excludes child window areas from the parent's visible region. Direct2D's
`Clear()` and drawing operations automatically respect this clipping, leaving child control areas untouched.

For child controls that may overlap each other, add **`WS_CLIPSIBLINGS`** to prevent siblings from drawing over each
other:

```rust
let edit1 = CreateWindowExW(
WS_EX_CLIENTEDGE, w!("EDIT"), w!(""),
WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_CLIPSIBLINGS.0 | ES_AUTOHSCROLL.0),
10, 10, 200, 25, parent, None, instance, None,
) ?;
```

**Paint order** follows a defined sequence: Windows sends `WM_PAINT` to the parent first, then to children. With
`WS_CLIPCHILDREN`, children don't need repainting after the parent finishes. For composited siblings (
`WS_EX_COMPOSITED`), paint messages arrive in reverse z-order so topmost windows paint last (on top).

Handle z-order with `SetWindowPos`:

```rust
SetWindowPos(child_hwnd, HWND_TOP, 0, 0, 0, 0,
SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
```

For Direct2D rendering, suppress `WM_ERASEBKGND` to prevent GDI from erasing your background before Direct2D paints:

```rust
WM_ERASEBKGND => LRESULT(1),  // Indicate we handled it
```

## Focus management between multiple text controls

Windows ensures only one window has keyboard focus at any time through the `WM_SETFOCUS`/`WM_KILLFOCUS` message pair.
When `SetFocus()` is called, Windows first sends `WM_KILLFOCUS` to the losing window (with `wParam` containing the
gaining window's handle), then `WM_SETFOCUS` to the gaining window.

Edit controls notify their parent of focus changes via `WM_COMMAND`:

```rust
WM_COMMAND => {
let notification = (wparam.0 > > 16) as u16;  // HIWORD
let control_id = (wparam.0 & 0xFFFF) as u16;  // LOWORD
let control_hwnd = HWND(lparam.0);

match notification {
EN_SETFOCUS => { /* Control gained focus */ }
EN_KILLFOCUS => { /* Control lost focus */ }
EN_CHANGE => { /* Text changed */ }
_ => {}
}
LRESULT(0)
}
```

**Critical focus handling rules** from Raymond Chen's guidance:

1. **Never call functions that display or activate windows during `WM_KILLFOCUS`**—this can cause message deadlocks or
   infinite focus cycling
2. **Never destroy windows during `WM_KILLFOCUS`**—use `PostMessage(hwnd, WM_CLOSE, 0, 0)` instead
3. **Never disable the focused control without moving focus first**:

```rust
fn safely_disable_control(dialog: HWND, control: HWND) {
    unsafe {
        if GetFocus() == control {
            SendMessageW(dialog, WM_NEXTDLGCTL, WPARAM(0), LPARAM(0));
        }
        EnableWindow(control, FALSE);
    }
}
```

For dialog-like focus management, prefer `WM_NEXTDLGCTL` over `SetFocus()` because it properly updates dialog manager
bookkeeping and default button state:

```rust
// Move focus to specific control
SendMessageW(dialog, WM_NEXTDLGCTL, WPARAM(target_control.0 as usize), LPARAM(1));
```

## Message loop patterns for multiple controls

A robust message loop that handles tab navigation between controls requires `IsDialogMessage`:

```rust
let mut msg = MSG::default ();
while GetMessageW( & mut msg, HWND(0), 0, 0).into() {
// IsDialogMessage handles TAB, SHIFT+TAB, arrow keys for control navigation
if IsDialogMessage(main_window, & msg).as_bool() {
continue;  // Don't call TranslateMessage/DispatchMessage
}

if ! TranslateAcceleratorW(main_window, accel_table, & msg).as_bool() {
TranslateMessage( & msg);
DispatchMessage( & msg);
}
}
```

For `IsDialogMessage` to work correctly:

- Add `WS_TABSTOP` to controls that should participate in tab navigation
- Add `WS_EX_CONTROLPARENT` to container windows
- Controls must have unique IDs assigned via the `HMENU` parameter

Handle `WM_COMMAND` notifications by extracting the notification code, control ID, and handle:

| Parameter        | Meaning                                              |
|------------------|------------------------------------------------------|
| `HIWORD(wParam)` | Notification code (`EN_CHANGE`, `EN_SETFOCUS`, etc.) |
| `LOWORD(wParam)` | Control ID                                           |
| `lParam`         | Control HWND                                         |

## Control visibility and lifecycle management

**Use `ShowWindow(SW_HIDE)`** when controls will be reused—this preserves state, content, and avoids recreation
overhead. The pattern works well for tabbed interfaces or wizard-style UIs:

```rust
// Switch between pages
ShowWindow(page1_container, SW_HIDE);
ShowWindow(page2_container, SW_SHOW);
```

**Use `DestroyWindow`** when controls are no longer needed, to release system resources (HWND handles, GDI objects,
window memory). Hidden windows still consume resources.

For bulk control operations, bracket changes with `WM_SETREDRAW` to prevent flickering:

```rust
SendMessageW(parent, WM_SETREDRAW, WPARAM(0), LPARAM(0));
// Create or modify many controls
SendMessageW(parent, WM_SETREDRAW, WPARAM(1), LPARAM(0));
InvalidateRect(parent, None, TRUE);
```

## Subclassing controls with SetWindowSubclass

The modern `SetWindowSubclass` API (from `Win32_UI_Shell`) is safer than the legacy `SetWindowLongPtrW` approach because
it properly chains subclass procedures:

```rust
use windows::Win32::UI::Shell::{SetWindowSubclass, DefSubclassProc, RemoveWindowSubclass};

extern "system" fn edit_subclass_proc(
    hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
    _uid_subclass: usize, _dw_ref_data: usize,
) -> LRESULT {
    unsafe {
        match msg {
            WM_ERASEBKGND => {
                // Custom background drawing
                let hdc = HDC(wparam.0 as isize);
                let mut rc = RECT::default();
                GetClientRect(hwnd, &mut rc);
                FillRect(hdc, &rc, dark_brush);
                return LRESULT(1);
            }
            WM_NCDESTROY => {
                RemoveWindowSubclass(hwnd, Some(edit_subclass_proc), 1);
            }
            _ => {}
        }
        DefSubclassProc(hwnd, msg, wparam, lparam)
    }
}

// Install subclass
SetWindowSubclass(edit_hwnd, Some(edit_subclass_proc), 1, 0);
```

**Always remove the subclass in `WM_NCDESTROY`**, which is the last message a window receives before destruction.

## Recommended control types for specific use cases

**For single-line search bars**, use the standard EDIT class with `ES_AUTOHSCROLL`:

```rust
CreateWindowExW(
WS_EX_CLIENTEDGE, w!("EDIT"), w!(""),
WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | ES_LEFT.0 | ES_AUTOHSCROLL.0),
x, y, width, 25,
parent, HMENU(SEARCH_EDIT_ID as isize), instance, None,
)
```

Standard EDIT is lighter weight, requires no DLL loading, and provides sufficient functionality for text entry.

**For multi-line text editors**, RichEdit (`RICHEDIT50W` from Msftedit.dll) offers advantages for complex editing:

```rust
LoadLibraryW(w!("Msftedit.dll")) ?;

CreateWindowExW(
WS_EX_CLIENTEDGE, w!("RICHEDIT50W"), w!(""),
WINDOW_STYLE(
WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | WS_VSCROLL.0 |
ES_MULTILINE.0 | ES_AUTOVSCROLL.0 | ES_WANTRETURN.0 | ES_NOHIDESEL.0
),
x, y, width, height,
parent, HMENU(EDITOR_ID as isize), instance, None,
)
```

RichEdit provides **multi-level undo**, larger text capacity (beyond EDIT's ~64KB limit), easier dark theme support via
`EM_SETBKGNDCOLOR`, and rich text formatting if needed. Use standard EDIT for multi-line only when minimal dependencies
and maximum simplicity are required.

## Conclusion

Building polished Windows desktop applications with the Rust `windows` crate requires understanding the interplay
between Win32's message-driven architecture and Rust's ownership model. **RichEdit controls dramatically simplify dark
theme implementation** compared to standard EDIT controls, which require both `WM_CTLCOLOREDIT` handling and
undocumented Windows APIs for complete styling. For Direct2D integration, the `WS_CLIPCHILDREN` style on the parent
window is essential—it prevents custom rendering from overdrawing native controls while maintaining proper paint
ordering.

Focus management relies on Windows' built-in single-focus guarantee, but developers must avoid modifying windows during
`WM_KILLFOCUS` handlers to prevent focus cycling bugs. The `SetWindowSubclass` API provides the safest pattern for
customizing control behavior, and `IsDialogMessage` in the message loop enables keyboard navigation between controls.
For control lifecycle, prefer hiding over destroying when controls will be reused, but ensure destroyed windows properly
release their resources. These patterns, combined with careful string lifetime management unique to Rust's binding
approach, enable robust native Windows applications.