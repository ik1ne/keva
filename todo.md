# Keva GUI Implementation Plan

## Context

Keva is a local key-value store for clipboard-like data. The core library (`keva_core`) is implemented in Rust.

**Architecture Decision:** Hybrid native approach.

- **Windows:** Pure Rust (`windows` crate for Win32 API)
- **macOS:** Swift/AppKit with FFI to `keva_core`
- **Shared:** `keva_core` (Rust)

**Rationale:**
- gpui/tao can't handle borderless+resize on macOS
- Windows: `windows` crate is Microsoft-maintained, well-documented
- macOS: Swift is first-class for Cocoa, QuickLook integration is trivial
- FFI overhead for keva_core is minimal (not high-frequency calls)

**Reference documents:**

- `Spec.md` - Product specification (source of truth for behavior)
- `implementation_detail.md` - keva_core API reference
- `Planned.md` - Future features (not in scope)

**Project structure:**

```
keva/
├── core/           # keva_core (Rust library)
├── ffi/            # C FFI bindings for macOS (Rust, builds dylib)
├── app-windows/    # Windows app (Rust + windows crate)
├── app-macos/      # macOS app (Swift/AppKit, links keva_ffi)
├── Spec.md
├── Planned.md
└── implementation_detail.md
```

---

## Phase 0: Foundation

### M0-ffi: Core FFI Layer (for macOS)

**Goal:** Expose keva_core to Swift via C FFI.

**Dependencies:**
```toml
[dependencies]
keva_core = { path = "../core" }

[build-dependencies]
cbindgen = "0.27"
```

**Tasks:**

1. Create `ffi` crate with `crate-type = ["cdylib"]`
2. Define C-compatible API with `#[no_mangle]` and `extern "C"`
3. Handle memory management (Box for heap, CString for strings)
4. Generate `keva.h` via cbindgen
5. Build as `libkeva.dylib`

**API:**

```c
// Lifecycle
KevaHandle* keva_open(const char* path);
void keva_close(KevaHandle* handle);

// CRUD
int32_t keva_set_text(KevaHandle* h, const char* key, const char* text);
int32_t keva_set_files(KevaHandle* h, const char* key, const char** paths, size_t count);
KevaValue* keva_get(KevaHandle* h, const char* key);
int32_t keva_delete(KevaHandle* h, const char* key);
int32_t keva_rename(KevaHandle* h, const char* old_key, const char* new_key);

// Listing
KevaKeyList* keva_list_keys(KevaHandle* h);

// Memory
void keva_free_value(KevaValue* value);
void keva_free_key_list(KevaKeyList* list);
```

**Acceptance criteria:**

- `libkeva.dylib` builds
- `keva.h` generated
- Can call from Swift playground

---

## Phase 1: Windows App (Pure Rust)

### M1-win: Window Skeleton ✅

**Goal:** Basic borderless window with system tray.

**Status:** Complete

**Dependencies:**
```toml
[dependencies]
keva_core = { path = "../core" }
windows = { version = "0.62", features = [
    "Win32_Foundation",
    "Win32_System_LibraryLoader",
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_Controls",
    "Win32_UI_Shell",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_Graphics_Gdi",
    "Win32_Graphics_Dwm",
] }
```

**Completed:**

1. ✅ Create `app-windows` crate as `[[bin]]`
2. ✅ Register window class, create borderless window (WS_POPUP)
3. ✅ Implement WM_NCHITTEST for resize edges
4. ✅ System tray icon (Shell_NotifyIconW)
5. ✅ Message loop with tray events
6. ✅ Tray click toggles window visibility
7. ⚠️ Taskbar icon visible (hiding from taskbar while keeping Alt+Tab is impossible on Windows)
8. ✅ DwmExtendFrameIntoClientArea for smooth resize compositing
9. ✅ WM_NCACTIVATE handling to prevent gray border on activation
10. ✅ Window centered on screen
11. ✅ Esc key hides window (WM_KEYDOWN)

**Acceptance criteria:** ✅

- ✅ Window appears, can resize from edges
- ✅ Tray icon visible, click toggles window
- ✅ Visible in Alt+Tab
- ⚠️ Taskbar icon visible (Windows limitation - hiding breaks Alt+Tab)
- ✅ Esc hides window and restores focus to previous app

### M2-win: Core Integration

**Goal:** Connect UI to keva_core.

**Tasks:**

1. Load keys on startup
2. Render key list (custom draw or list control)
3. Text preview (Rich Edit control)
4. File preview (IPreviewHandler)
5. Clipboard paste to create key

### M3-win: Full Features

**Goal:** All Spec.md features.

**Tasks:**

1. Fuzzy search (nucleo)
2. Edit/rename/delete keys
3. Copy to clipboard
4. Trash support
5. Settings dialog

---

## Phase 2: macOS App (Swift)

### M1-mac: Window Skeleton

**Goal:** Basic borderless window with menu bar icon.

**Build:** Swift Package Manager or xcodebuild (no Xcode IDE required)

**Tasks:**

1. Create Swift package or minimal Xcode project
2. Link `libkeva.dylib`, import `keva.h` via bridging header
3. Borderless window (NSWindow, styleMask)
4. Custom resize handling if needed
5. Menu bar icon (NSStatusItem)
6. Cmd+Q quits, Esc hides window
7. Set LSUIElement=true in Info.plist (hide from Dock/Cmd+Tab)

**Acceptance criteria:**

- App launches to menu bar
- Window shows/hides on click
- Window resizes properly
- No Dock icon, hidden from Cmd+Tab

### M2-mac: Core Integration

**Goal:** Connect UI to keva_core via FFI.

**Tasks:**

1. Swift wrapper around C FFI
2. Load/display keys
3. Text preview (NSTextView)
4. File preview (QLPreviewView)
5. Clipboard paste to create key

### M3-mac: Full Features

**Goal:** All Spec.md features.

**Tasks:**

1. Fuzzy search (nucleo via FFI, or native NSPredicate)
2. Edit/rename/delete keys
3. Copy to clipboard (NSPasteboard)
4. Trash support
5. Settings window

---

## Phase 3: Polish

### M4: Distribution

**Windows:**
- Installer (WiX or MSIX)
- Launch at Login (Registry)
- Code signing (optional)

**macOS:**
- App bundle structure
- Launch at Login (LaunchAgent or SMLoginItemSetEnabled)
- Code signing + notarization

---

## Notes

### Windows Crate Features

Current features for `windows` crate (v0.62):
```toml
[target.'cfg(windows)'.dependencies.windows]
version = "0.62"
features = [
    "Win32_Foundation",
    "Win32_System_LibraryLoader",
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_Controls",
    "Win32_UI_Shell",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_Graphics_Gdi",
    "Win32_Graphics_Dwm",
    # Future: "Win32_System_Com" for IPreviewHandler
]
```

### macOS Borderless + Resize

```swift
let styleMask: NSWindow.StyleMask = [
    .borderless,
    .resizable,  // This should work in native Swift
]
window = NSWindow(contentRect: rect, styleMask: styleMask, ...)
```

If `.borderless` + `.resizable` doesn't work, implement `mouseDown`/`mouseDragged` for edge resizing.

### FFI Memory Rules

- Caller allocates path strings, FFI copies internally
- FFI allocates return values (KevaValue, KevaKeyList)
- Caller must call `keva_free_*` to release
- Error codes: 0 = success, negative = error
