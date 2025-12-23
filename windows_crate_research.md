# Windows Crate Research for Keva

## Crate Overview

**Crate:** `windows`
**Latest Version:** 0.62.2 (as of 2025-10-06)
**Maintainer:** Microsoft
**Repository:** https://github.com/microsoft/windows-rs
**Documentation:** https://microsoft.github.io/windows-docs-rs/

The `windows` crate provides direct access to all Windows APIs (past, present, future) with code generated from Windows metadata.

## Recommended Version

```toml
[dependencies.windows]
version = "0.62"
features = [...]
```

Use `0.62` rather than `0.58` (from todo.md) - significantly newer with better raw-dylib support.

---

## Required Features by Keva Feature

### 1. Window Creation

**APIs:**
- `CreateWindowExW` - create window
- `RegisterClassW` - register window class
- `GetModuleHandleW` - get instance handle
- `ShowWindow`, `UpdateWindow` - show/update
- `GetMessageW`, `TranslateMessage`, `DispatchMessageW` - message loop
- `DefWindowProcW` - default message handling

**Features:**
```toml
"Win32_Foundation",
"Win32_UI_WindowsAndMessaging",
"Win32_System_LibraryLoader",
"Win32_Graphics_Gdi",
```

**Example (from windows-rs samples):**
```rust
let wc = WNDCLASSA {
    hCursor: LoadCursorW(None, IDC_ARROW)?,
    hInstance: instance.into(),
    lpszClassName: s!("KevaWindowClass"),
    style: CS_HREDRAW | CS_VREDRAW,
    lpfnWndProc: Some(wndproc),
    ..Default::default()
};
RegisterClassA(&wc);
CreateWindowExA(...)?;
```

---

### 2. Borderless Window with Resize (WM_NCHITTEST)

For borderless windows (`WS_POPUP` instead of `WS_OVERLAPPEDWINDOW`), we need custom resize handling.

**Message:** `WM_NCHITTEST` (0x84)

**Return values for resize edges:**
| Constant | Value | Description |
|----------|-------|-------------|
| `HTLEFT` | 10 | Left edge |
| `HTRIGHT` | 11 | Right edge |
| `HTTOP` | 12 | Top edge |
| `HTTOPLEFT` | 13 | Top-left corner |
| `HTTOPRIGHT` | 14 | Top-right corner |
| `HTBOTTOM` | 15 | Bottom edge |
| `HTBOTTOMLEFT` | 16 | Bottom-left corner |
| `HTBOTTOMRIGHT` | 17 | Bottom-right corner |
| `HTCAPTION` | 2 | Draggable area |

**Implementation pattern:**
```rust
WM_NCHITTEST => {
    let x = LOWORD(lparam.0 as u32) as i16 as i32;
    let y = HIWORD(lparam.0 as u32) as i16 as i32;
    // Get window rect, calculate distance to edges
    // Return HTLEFT, HTRIGHT, etc. based on cursor position
}
```

**Features:** Same as window creation (part of `Win32_UI_WindowsAndMessaging`)

---

### 3. System Tray Icon

**APIs:**
- `Shell_NotifyIconW` - add/modify/remove tray icon
- `NOTIFYICONDATAW` - icon data structure

**Constants:**
- `NIM_ADD`, `NIM_MODIFY`, `NIM_DELETE` - operations
- `NIF_ICON`, `NIF_MESSAGE`, `NIF_TIP` - flags

**Features:**
```toml
"Win32_UI_Shell",
```

**Example:**
```rust
let mut nid = NOTIFYICONDATAW::default();
nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
nid.hWnd = hwnd;
nid.uID = 1;
nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
nid.uCallbackMessage = WM_USER + 1;
nid.hIcon = LoadIconW(None, IDI_APPLICATION)?;
// Set tooltip...
Shell_NotifyIconW(NIM_ADD, &nid);
```

---

### 4. Hide from Taskbar (Keep Alt+Tab)

**API:** `ITaskbarList3::DeleteTab`

This is a COM interface. We need to:
1. Initialize COM
2. Create `ITaskbarList3` instance
3. Call `HrInit()`
4. Call `DeleteTab(hwnd)`

**Features:**
```toml
"Win32_UI_Shell",
"Win32_System_Com",
```

**Methods available on ITaskbarList3:**
- `HrInit()` - initialize
- `DeleteTab(hwnd)` - remove from taskbar
- `AddTab(hwnd)` - add to taskbar (if needed later)
- `SetProgressValue`, `SetProgressState` - progress indicators
- `SetOverlayIcon` - overlay badge

---

### 5. Clipboard

**APIs:**
- `OpenClipboard`, `CloseClipboard` - clipboard access
- `GetClipboardData` - read data
- `SetClipboardData` - write data
- `EmptyClipboard` - clear clipboard

**Formats:**
- `CF_TEXT`, `CF_UNICODETEXT` - text
- `CF_HDROP` - file list

**Features:**
```toml
"Win32_System_DataExchange",
"Win32_System_Memory",  # for GlobalAlloc/GlobalLock
```

**Alternative:** Use `clipboard-win` crate for simpler API.

---

### 6. File Preview (IPreviewHandler)

**API:** `IPreviewHandler` COM interface

**Methods:**
- `SetWindow(hwnd, rect)` - set preview area
- `SetRect(rect)` - update preview area
- `DoPreview()` - start rendering
- `Unload()` - cleanup
- `SetFocus()`, `QueryFocus()` - focus management
- `TranslateAccelerator(msg)` - keyboard handling

**To use:**
1. Get file's preview handler CLSID from registry (via `IQueryAssociations`)
2. Create instance via `CoCreateInstance`
3. Initialize with `IInitializeWithFile` or `IInitializeWithStream`
4. Call `SetWindow`, then `DoPreview`

**Features:**
```toml
"Win32_UI_Shell",
"Win32_System_Com",
```

**Complexity:** High - requires COM, registry lookup, multiple interfaces.

---

### 7. Keyboard & Input

**Messages:**
- `WM_KEYDOWN`, `WM_KEYUP` - key events
- `WM_CHAR` - character input
- `WM_HOTKEY` - global hotkey (registered with `RegisterHotKey`)

**Features:**
```toml
"Win32_UI_Input_KeyboardAndMouse",
```

---

### 8. Global Hotkey

**APIs:**
- `RegisterHotKey(hwnd, id, modifiers, vk)` - register
- `UnregisterHotKey(hwnd, id)` - unregister

**Messages:** `WM_HOTKEY`

**Features:**
```toml
"Win32_UI_Input_KeyboardAndMouse",
```

---

## Complete Feature List for Keva

```toml
[target.'cfg(windows)'.dependencies.windows]
version = "0.62"
features = [
    # Foundation
    "Win32_Foundation",

    # Window creation and management
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_LibraryLoader",
    "Win32_Graphics_Gdi",

    # System tray and taskbar
    "Win32_UI_Shell",

    # COM (for ITaskbarList3, IPreviewHandler)
    "Win32_System_Com",

    # Keyboard/hotkeys
    "Win32_UI_Input_KeyboardAndMouse",

    # Controls (for text input, list views)
    "Win32_UI_Controls",
]
```

Note: Clipboard features (`Win32_System_DataExchange`, `Win32_System_Memory`) are NOT needed - clipboard is handled by `keva_core`.

---

## Edition 2024 Considerations

Rust Edition 2024 requires `unsafe {}` blocks inside `unsafe fn`. Example:

```rust
// Edition 2024 style
unsafe extern "system" fn wndproc(...) -> LRESULT {
    match msg {
        WM_PAINT => unsafe {
            BeginPaint(hwnd, &mut ps);
            EndPaint(hwnd, &ps);
        },
        WM_DESTROY => unsafe {
            PostQuitMessage(0);
        },
        _ => unsafe {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
    }
    LRESULT(0)
}
```

---

## Alternative Crates to Consider

| Purpose | Crate | Notes |
|---------|-------|-------|
| System tray | `tray-icon` | Cross-platform, by Tauri team |
| Clipboard | `clipboard-win` | Simpler API for Windows |
| Global hotkey | `global-hotkey` | Cross-platform |

These wrap the `windows` crate but provide higher-level APIs. Trade-off: less control vs. easier implementation.

---

## Key References

- [windows crate docs](https://microsoft.github.io/windows-docs-rs/doc/windows/)
- [windows-rs GitHub](https://github.com/microsoft/windows-rs)
- [windows-rs samples](https://github.com/microsoft/windows-rs/tree/master/crates/samples)
- [ITaskbarList3](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Shell/struct.ITaskbarList3.html)
- [IPreviewHandler](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Shell/struct.IPreviewHandler.html)
- [Shell_NotifyIconW](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Shell/fn.Shell_NotifyIconW.html)

---

## Decisions Made

1. **Raw windows crate only** - No helper crates (tray-icon, clipboard-win). Full control, fewer dependencies.

2. **Clipboard:** Handled by `keva_core`, not the Windows app. The app just calls keva_core APIs.

3. **File preview:** Use `IPreviewHandler` (shell) for native Windows preview experience.

## Remaining Questions

1. **Dark mode:** How to detect/respond to Windows dark mode? (Future enhancement)
