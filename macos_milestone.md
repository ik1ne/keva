# macOS Implementation Milestones

This document defines the implementation milestones for Keva on macOS. Each milestone builds upon the previous ones
and includes test cases for verification.

## Milestone Overview

| #   | Milestone            | Description                                     | Status |
|-----|----------------------|-------------------------------------------------|--------|
| M1  | Initial Setup        | Xcode project or Swift package, app launches    | ✅      |
| M2  | Borderless Window    | NSWindow without titlebar, resize, hide/show    | ✅      |
| M3  | Single Instance      | NSRunningApplication check, activate existing   | ❌      |
| M4  | Menu Bar Item        | NSStatusItem, click toggles, right-click menu   | ❌      |
| M5  | Load Frontend        | Move vite output, WKURLSchemeHandler for assets | ❌      |
| M6  | Worker Thread        | keva_core integration, message passing          | ❌      |
| M7  | Content Protocol     | Full content load/save, large file optimization | ❌      |
| M8  | Search Engine        | keva_search integration on main thread          | ❌      |
| M9  | Global Hotkey        | Cmd+Option+K system-wide                        | ❌      |
| M10 | Drag & Drop          | In + out with move semantics                    | ❌      |
| M11 | Clipboard            | NSPasteboard integration, paste interception    | ❌      |
| M12 | First-Run + Settings | SMAppService for login item, config persistence | ❌      |
| M13 | Distribution         | DMG, notarization                               | ❌      |

---

## Platform Differences from Windows

| Aspect              | Windows                        | macOS                               |
|---------------------|--------------------------------|-------------------------------------|
| WebView             | WebView2                       | WKWebView                           |
| File Access         | FileSystemHandle (direct)      | postMessage (full content transfer) |
| System Icon         | System tray                    | Menu bar (NSStatusItem)             |
| Global Hotkey       | RegisterHotKey                 | CGEventTap                          |
| Default Hotkey      | Ctrl+Alt+K                     | Cmd+Option+K                        |
| Quit Shortcut       | Alt+F4                         | Cmd+Q                               |
| Single Instance     | Named mutex                    | NSRunningApplication                |
| Launch at Login     | Registry                       | SMAppService.mainApp                |
| Data Directory      | %LOCALAPPDATA%\keva            | ~/Library/Application Support/keva  |
| Distribution        | MSI installer                  | DMG + notarization                  |
| Drag-Drop Export    | IDropTarget/DoDragDrop         | NSPasteboard/NSDraggingSource       |
| Drag-Drop Semantics | Copy only (unreliable move)    | Move default, Option for copy       |
| Window Drag         | CompositionController required | Message → performDrag (simple)      |

---

## Reusable Components

The following components from the Windows implementation are reused without changes:

- **keva_core** - Storage, attachments, lifecycle management
- **keva_search** - Nucleo-based fuzzy search
- **WebView frontend** - HTML/CSS/JS (95% reusable)
- **Monaco editor** - Text editing
- **Message protocol structure** - JSON message types

---

## M1: Initial Setup

**Goal:** Create minimal macOS application that launches.

**Description:** Set up Xcode project or Swift Package Manager project. Application launches, shows in Dock briefly,
then terminates cleanly. Establishes project structure for subsequent milestones.

**Implementation Notes:**

- Swift 5.9+ with macOS 15 (Sequoia) deployment target, supporting macOS 26 (Tahoe)
- AppKit-based (not SwiftUI) for fine-grained control
- Bundle identifier: `com.keva.app` (or similar)
- Project structure prepared for Rust integration via swift-bridge or manual FFI

**Project Structure:**

```
keva_macos/
├── Package.swift (or Keva.xcodeproj)
├── Sources/
│   └── Keva/
│       ├── main.swift
│       ├── AppDelegate.swift
│       └── ...
└── Resources/
    └── Assets.xcassets
```

**Test Cases:**

| TC       | Description                       | Status |
|----------|-----------------------------------|--------|
| TC-M1-01 | App launches without crash        | ✅      |
| TC-M1-02 | App appears in Dock during launch | ✅      |
| TC-M1-03 | App terminates cleanly            | ⚠️     |
| TC-M1-04 | Console shows no errors           | ⚠️     |

**Notes:**

- TC-M1-03: Cmd+Q unresponsive (no menu bar), dock context menu "Quit" works
- TC-M1-04: Warning:
  `-[NSApplication restoreWindowWithIdentifier:state:completionHandler:] Unable to find className=(null)` — harmless
  window restoration message

---

## M2: Borderless Window

**Goal:** Borderless window with resize and proper hide/show behavior.

**Description:** Create NSWindow without standard titlebar. Resize from edges using system metrics. Window hides
via `orderOut()` instead of being destroyed. Window shows on manual launch, centered on primary monitor. Always on top
for drag/drop workflows.

**Implementation Notes:**

- `NSWindow` with `styleMask: [.borderless, .resizable]`
- Override `windowShouldClose()` to call `orderOut()` and return `false`
- `window.level = .floating` for always-on-top
- `window.center()` for initial positioning
- Minimum size: 400×300 logical pixels
- Resize via `NSWindow.contentResizeIncrements` or `mouseDown` tracking on edges

**Window Behavior:**

| Action             | Result                                |
|--------------------|---------------------------------------|
| Manual launch      | Show window, centered                 |
| Close button / Esc | Hide window (orderOut), process stays |
| Cmd+Q              | Quit application entirely             |
| Reactivate app     | Show window if hidden                 |

**Test Cases:**

| TC       | Description                                | Status |
|----------|--------------------------------------------|--------|
| TC-M2-01 | Window appears without titlebar            | ✅      |
| TC-M2-02 | Window appears centered on primary monitor | ✅      |
| TC-M2-03 | Drag from edges resizes window             | ✅      |
| TC-M2-04 | Window respects minimum size (400×300)     | ✅      |
| TC-M2-05 | Esc hides window (process stays alive)     | ✅      |
| TC-M2-06 | Cmd+Q quits application entirely           | ✅      |
| TC-M2-07 | Window stays on top of other windows       | ✅      |
| TC-M2-08 | Corner drag resizes diagonally             | ✅      |
| TC-M2-09 | Hidden window can be shown again           | ✅      |
| TC-M2-10 | Text is crisp at Retina scaling            | N/A    |

**Notes:**

- TC-M2-10: Deferred to M5 (no content to display yet)

---

## M3: Single Instance

**Goal:** Ensure only one instance runs at a time.

**Description:** Check for existing instance using NSRunningApplication. If already running, activate existing
window instead of launching new instance.

**Implementation Notes:**

- `NSRunningApplication.runningApplications(withBundleIdentifier:)`
- If found and not self: `activate(options: .activateIgnoringOtherApps)`
- Send notification or use Distributed Notifications to signal "show window"
- Exit new instance after activating existing

**Test Cases:**

| TC       | Description                             | Status |
|----------|-----------------------------------------|--------|
| TC-M3-01 | Second launch activates existing window | ❌      |
| TC-M3-02 | Second launch exits after activation    | ❌      |
| TC-M3-03 | Works when existing window is hidden    | ❌      |

---

## M4: Menu Bar Item

**Goal:** Menu bar icon with click toggle and right-click menu.

**Description:** Add NSStatusItem to menu bar. Left-click toggles window visibility. Right-click shows context menu
matching Windows tray menu.

**Implementation Notes:**

- `NSStatusBar.system.statusItem(withLength: .squareLength)`
- `button.action` for left-click toggle
- `button.sendAction(on: [.leftMouseUp, .rightMouseUp])` to detect click type
- Or use `NSMenu` with `popUpMenu` for right-click
- Icon: Template image for automatic dark/light mode support

**Menu Items:**

| Item            | Action                                    |
|-----------------|-------------------------------------------|
| Show Keva       | Show window (disabled if already visible) |
| Settings...     | Open settings dialog                      |
| ---             | Separator                                 |
| Launch at Login | Checkbox toggle (synced with settings)    |
| ---             | Separator                                 |
| Quit Keva       | Terminate application                     |

**Test Cases:**

| TC       | Description                              | Status |
|----------|------------------------------------------|--------|
| TC-M4-01 | Menu bar icon visible                    | ❌      |
| TC-M4-02 | Left-click toggles window visibility     | ❌      |
| TC-M4-03 | Right-click shows context menu           | ❌      |
| TC-M4-04 | "Show Keva" disabled when window visible | ❌      |
| TC-M4-05 | "Quit Keva" terminates application       | ❌      |
| TC-M4-06 | Icon adapts to dark/light mode           | ❌      |
| TC-M4-07 | Tooltip shows "Keva"                     | ❌      |

---

## M5: Load Frontend

**Goal:** Load actual Keva frontend HTML in WKWebView.

**Description:** Move vite build output to shared location (outside Windows-specific folder). Load HTML via
WKURLSchemeHandler custom protocol. Establish basic Native↔WebView message bridge. Window drag works via message.

**Implementation Notes:**

- Register custom scheme: `keva-app://`
- `WKURLSchemeHandler` serves bundled HTML/CSS/JS
- `WKScriptMessageHandler` for WebView→Native messages
- `evaluateJavaScript` for Native→WebView messages
- Window drag: WebView sends `{ type: "startWindowDrag" }` → Native calls `window.performDrag(with: event)`

**Folder Structure Change:**

```
keva/
├── frontend/           # Moved from platforms/windows/
│   ├── src/
│   ├── vite.config.ts
│   └── dist/          # Build output
├── platforms/
│   ├── windows/
│   └── macos/
```

**Custom Scheme Handler:**

```swift
class KevaSchemeHandler: NSObject, WKURLSchemeHandler {
    func webView(_ webView: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        // Map keva-app://index.html → bundled resource
        // Return file data with appropriate MIME type
    }
}
```

**Test Cases:**

| TC       | Description                          | Status |
|----------|--------------------------------------|--------|
| TC-M5-01 | WebView loads and displays UI        | ❌      |
| TC-M5-02 | CSS styles applied correctly         | ❌      |
| TC-M5-03 | JavaScript executes without errors   | ❌      |
| TC-M5-04 | Monaco editor loads                  | ❌      |
| TC-M5-05 | Native↔WebView message bridge works  | ❌      |
| TC-M5-06 | Window drag via search icon works    | ❌      |
| TC-M5-07 | Theme matches system dark/light mode | ❌      |

---

## M6: Worker Thread

**Goal:** Background thread for keva_core operations.

**Description:** Spawn worker thread on startup. Main thread sends requests via channel. Worker executes keva_core
operations and posts results back. Identical architecture to Windows implementation.

**Implementation Notes:**

- Swift concurrency or DispatchQueue for threading
- Rust FFI via swift-bridge or manual C bindings
- keva_core compiled as static library
- Request/Response enums for type-safe messaging

**Threading Model:**

```
Main Thread                     Worker Thread
    │                               │
    ├─── Request::CreateKey ───────►│
    │                               ├─── keva_core.create()
    │◄── Response::KeyCreated ──────┤
    │                               │
```

**Test Cases:**

| TC       | Description                                | Status |
|----------|--------------------------------------------|--------|
| TC-M6-01 | App quits cleanly via Cmd+Q (no hang)      | ❌      |
| TC-M6-02 | App quits cleanly via menu (no hang)       | ❌      |
| TC-M6-03 | Creating key updates UI without freezing   | ❌      |
| TC-M6-04 | keva_core operations complete successfully | ❌      |

---

## M7: Content Protocol

**Goal:** Full content load/save via message passing with large file optimization.

**Description:** Replace Windows FileSystemHandle approach with postMessage-based content transfer. Native reads
file and sends content to WebView. WebView sends modified content back for save. Disable debounced auto-save for
files ≥1MB to reduce IPC overhead.

**Implementation Notes:**

- Read: Native loads file → `{ type: "contentLoaded", key, content }`
- Write: WebView sends `{ type: "saveContent", key, content }` → Native writes
- Large file detection: Check content length, disable 500ms debounce if ≥1MB
- Blur-triggered saves preserved for all file sizes
- Exit saves preserved for all file sizes

**Content Flow:**

```
Load:
1. User selects key
2. Native: read content file
3. Native → WebView: { type: "contentLoaded", key, content }
4. WebView: Monaco.setValue(content)

Save:
1. Monaco content changes
2. If <1MB: debounced save after 500ms
   If ≥1MB: skip debounced save
3. On blur/exit: WebView → Native: { type: "saveContent", key, content }
4. Native: write file
```

**Large File Optimization:**

| File Size | Debounced Auto-save | Blur-save | Exit-save |
|-----------|---------------------|-----------|-----------|
| <1MB      | 500ms               | ✓         | ✓         |
| ≥1MB      | **Disabled**        | ✓         | ✓         |

**Test Cases:**

| TC       | Description                                      | Status |
|----------|--------------------------------------------------|--------|
| TC-M7-01 | Selecting key loads content into editor          | ❌      |
| TC-M7-02 | Edits persist after switching away and back      | ❌      |
| TC-M7-03 | Content persists after app restart               | ❌      |
| TC-M7-04 | Large file (≥1MB) loads successfully             | ❌      |
| TC-M7-05 | Large file debounce disabled (no save on typing) | ❌      |
| TC-M7-06 | Large file saves on blur (Cmd+S → search focus)  | ❌      |
| TC-M7-07 | Large file saves on window hide                  | ❌      |
| TC-M7-08 | Large file saves on app exit                     | ❌      |
| TC-M7-09 | Rapid key switching does not lose unsaved edits  | ❌      |

---

## M8: Search Engine

**Goal:** Fuzzy search with progressive, stable results.

**Description:** Integrate keva_search on main thread. SearchEngine wraps Nucleo with dual indexes (active/trashed).
Identical to Windows implementation.

**Implementation Notes:**

- SearchEngine compiled into Rust static library
- Notify callback triggers UI update
- Threshold stops at 100 active, 20 trashed results
- Key mutations update indexes

**Test Cases:**

| TC       | Description                                   | Status |
|----------|-----------------------------------------------|--------|
| TC-M8-01 | Type in search bar, matching keys appear      | ❌      |
| TC-M8-02 | Empty search shows all keys                   | ❌      |
| TC-M8-03 | Results stop changing after threshold reached | ❌      |
| TC-M8-04 | Smart case matching works                     | ❌      |
| TC-M8-05 | Trashed keys appear in separate section       | ❌      |

---

## M9: Global Hotkey

**Goal:** System-wide hotkey to show window from any application.

**Description:** Register Cmd+Option+K as global hotkey using CGEventTap. Show window when pressed, even when Keva
is in background.

**Implementation Notes:**

- `CGEvent.tapCreate` with `kCGEventKeyDown` mask
- Check for Cmd+Option+K combination
- Requires Accessibility permissions (prompt user)
- Fallback: menu bar click if permissions denied
- Configurable shortcut stored in config

**Accessibility Permissions:**

```swift
let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue(): true] as CFDictionary
let trusted = AXIsProcessTrustedWithOptions(options)
```

**Test Cases:**

| TC       | Description                               | Status |
|----------|-------------------------------------------|--------|
| TC-M9-01 | Cmd+Option+K shows window from any app    | ❌      |
| TC-M9-02 | Hotkey works when window already visible  | ❌      |
| TC-M9-03 | Hotkey works when window is hidden        | ❌      |
| TC-M9-04 | Accessibility permission prompt appears   | ❌      |
| TC-M9-05 | Custom hotkey from settings is registered | ❌      |
| TC-M9-06 | Empty shortcut disables global hotkey     | ❌      |

---

## M10: Drag & Drop

**Goal:** Drag files in and out with move as default operation.

**Description:** Implement NSDraggingDestination for drag-in and NSDraggingSource for drag-out. Move is default
operation; Option key modifier switches to copy. Supports both internal (attachment→Monaco) and external drags.

**Implementation Notes:**

- Drag IN: `NSDraggingDestination` protocol on WebView or overlay view
- Drag OUT: WebView sends `{ type: "startDrag", files: [...] }` → Native creates `NSDraggingSession`
- Operation negotiation: prefer `.move`, fallback to `.copy` if source doesn't support move
- `draggingSession(_:endedAt:operation:)` callback to delete source on move

**Drag IN (External → Keva):**

| Modifier | Requested | Source Offers | Result | Cursor   |
|----------|-----------|---------------|--------|----------|
| None     | .move     | .move, .copy  | Move   | No badge |
| None     | .move     | .copy only    | Copy   | + badge  |
| Option   | .copy     | .move, .copy  | Copy   | + badge  |

**Drag OUT (Keva → External):**

| Modifier | Operation | Result                       |
|----------|-----------|------------------------------|
| None     | Move      | Attachment removed from Keva |
| Option   | Copy      | Attachment stays in Keva     |

**Internal Drag (Attachment → Monaco):**

- Insert `[filename](att:filename)` at drop position
- Images use `![filename](att:filename)` syntax

**Test Cases:**

| TC        | Description                                  | Status |
|-----------|----------------------------------------------|--------|
| TC-M10-01 | Drop file onto attachments adds it           | ❌      |
| TC-M10-02 | Drop file performs move (source deleted)     | ❌      |
| TC-M10-03 | Option+drop performs copy (source remains)   | ❌      |
| TC-M10-04 | Drop from read-only volume shows copy cursor | ❌      |
| TC-M10-05 | Drag attachment to Finder performs move      | ❌      |
| TC-M10-06 | Option+drag to Finder performs copy          | ❌      |
| TC-M10-07 | Drag attachment to Monaco inserts link       | ❌      |
| TC-M10-08 | Drag multiple attachments works              | ❌      |
| TC-M10-09 | Drag from trashed key rejected               | ❌      |
| TC-M10-10 | Drop onto trashed key rejected               | ❌      |

---

## M11: Clipboard

**Goal:** Native clipboard integration with paste interception.

**Description:** Read clipboard via NSPasteboard. Intercept paste in WebView and handle files specially. Copy
shortcuts for markdown, HTML, and files.

**Implementation Notes:**

- `NSPasteboard.general` for clipboard access
- `NSPasteboard.types` to detect content type (files vs text)
- Intercept Cmd+V via JavaScript or native key handler
- File paste: add to attachments + insert links

**Copy Shortcuts:**

| Shortcut     | Action                             | On Success  |
|--------------|------------------------------------|-------------|
| Cmd+C        | Copy selection (context-dependent) | Stay open   |
| Cmd+Option+T | Copy whole markdown as plain text  | Hide window |
| Cmd+Option+R | Copy rendered preview as HTML      | Hide window |
| Cmd+Option+F | Copy all attachments to clipboard  | Hide window |

**Test Cases:**

| TC        | Description                                     | Status |
|-----------|-------------------------------------------------|--------|
| TC-M11-01 | Paste text into search bar                      | ❌      |
| TC-M11-02 | Paste text into Monaco                          | ❌      |
| TC-M11-03 | Paste files adds attachments + inserts links    | ❌      |
| TC-M11-04 | Cmd+C in Monaco copies selected text            | ❌      |
| TC-M11-05 | Cmd+C in attachments copies selected files      | ❌      |
| TC-M11-06 | Cmd+Option+T copies markdown, hides window      | ❌      |
| TC-M11-07 | Cmd+Option+R copies rendered HTML, hides window | ❌      |
| TC-M11-08 | Cmd+Option+F copies attachments, hides window   | ❌      |
| TC-M11-09 | "Nothing to copy" shown when no target key      | ❌      |

---

## M12: First-Run + Settings

**Goal:** Welcome experience and settings with launch-at-login support.

**Description:** Detect first launch (no config exists). Show welcome dialog with launch-at-login checkbox.
Settings panel for configuration. SMAppService.mainApp for login item registration. Detect login launch vs manual
launch to determine initial window visibility.

**Implementation Notes:**

- Config path: `~/Library/Application Support/keva/config.toml`
- `SMAppService.mainApp.register()` / `.unregister()` for login item
- Detect login launch via `NSAppleEventManager.shared().currentAppleEvent`
- Settings as WebView panel (same as Windows)

**Launch Detection:**

```swift
private var launchedAsLoginItem: Bool {
    guard let event = NSAppleEventManager.shared().currentAppleEvent else {
        return false
    }
    return event.eventID == kAEOpenApplication &&
           event.paramDescriptor(forKeyword: keyAEPropData)?.enumCodeValue == keyAELaunchedAsLogInItem
}
```

**Launch Behavior:**

| Condition                  | Window                |
|----------------------------|-----------------------|
| First launch (no config)   | Show + welcome dialog |
| Manual launch              | Show                  |
| Login item launch          | Hidden                |
| Single instance reactivate | Show                  |

**Settings:**

| Category  | Setting         | Type                  | Default      |
|-----------|-----------------|-----------------------|--------------|
| General   | Theme           | Dark / Light / System | System       |
| General   | Launch at Login | Toggle                | false        |
| Shortcuts | Global Shortcut | Key capture           | Cmd+Option+K |
| Shortcuts | Copy Markdown   | Key capture           | Cmd+Option+T |
| Shortcuts | Copy HTML       | Key capture           | Cmd+Option+R |
| Shortcuts | Copy Files      | Key capture           | Cmd+Option+F |
| Lifecycle | Trash TTL       | Days (1-365000)       | 30 days      |
| Lifecycle | Purge TTL       | Days (1-365000)       | 7 days       |

**Test Cases:**

| TC        | Description                                        | Status |
|-----------|----------------------------------------------------|--------|
| TC-M12-01 | First launch shows welcome dialog                  | ❌      |
| TC-M12-02 | "Get Started" closes dialog, creates config        | ❌      |
| TC-M12-03 | Subsequent manual launches show window             | ❌      |
| TC-M12-04 | Login item launch hides window (menu bar only)     | ❌      |
| TC-M12-05 | Launch at Login toggle registers with SMAppService | ❌      |
| TC-M12-06 | App appears in System Settings > Login Items       | ❌      |
| TC-M12-07 | Cmd+, opens settings panel                         | ❌      |
| TC-M12-08 | Theme change applies immediately                   | ❌      |
| TC-M12-09 | Settings persist after restart                     | ❌      |
| TC-M12-10 | Escape closes settings without saving              | ❌      |

---

## M13: Distribution

**Goal:** DMG installer with notarization.

**Description:** Create DMG for distribution. Sign with Developer ID. Notarize with Apple. Support drag-to-Applications
installation.

**Implementation Notes:**

- `create-dmg` or similar tool
- Code signing: `codesign --sign "Developer ID Application: ..." --options runtime`
- Notarization: `xcrun notarytool submit`
- Stapling: `xcrun stapler staple`
- DMG layout: App icon + Applications folder alias

**Build Pipeline:**

```
1. cargo build --release (Rust libraries)
2. xcodebuild -configuration Release
3. codesign --deep --force --sign "Developer ID"
4. create-dmg with background image
5. notarytool submit → wait → staple
```

**Test Cases:**

| TC        | Description                                  | Status |
|-----------|----------------------------------------------|--------|
| TC-M13-01 | DMG opens and shows app + Applications alias | ❌      |
| TC-M13-02 | Drag to Applications installs app            | ❌      |
| TC-M13-03 | App launches without Gatekeeper warning      | ❌      |
| TC-M13-04 | App runs on clean macOS 14 installation      | ❌      |
| TC-M13-05 | spctl --assess reports "accepted"            | ❌      |

---

## Appendix: Frontend Changes Required

The following changes are needed in the shared frontend code:

| Area            | Change                                                    |
|-----------------|-----------------------------------------------------------|
| File I/O        | Abstract FileSystemHandle vs postMessage behind interface |
| Shortcuts       | Ctrl → Cmd mapping for macOS                              |
| Drag initiation | Send message instead of setting dataTransfer              |
| Large file mode | Disable debounce based on content length                  |

**Platform Detection:**

```javascript
const isMacOS = navigator.platform.includes('Mac');
const modifierKey = isMacOS ? 'Cmd' : 'Ctrl';
```

---

## Appendix: Rust Integration (C FFI)

Swift and Rust cannot call each other directly. Both languages can interface with C, so we expose Rust functions
with C-compatible signatures and call them from Swift.

### Rust Side

**Cargo.toml:**

```toml
[lib]
crate-type = ["staticlib"]  # Produces .a file
```

**ffi.rs:**

```rust
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

pub struct KevaCore {
    /* ... */
}

/// Create instance. Returns null on failure.
#[no_mangle]
pub extern "C" fn keva_core_new(data_dir: *const c_char) -> *mut KevaCore {
    let data_dir = unsafe {
        if data_dir.is_null() { return std::ptr::null_mut(); }
        CStr::from_ptr(data_dir).to_str().unwrap_or_default()
    };
    match KevaCore::open(data_dir) {
        Ok(core) => Box::into_raw(Box::new(core)),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Free instance. Must be called to avoid memory leak.
#[no_mangle]
pub extern "C" fn keva_core_free(ptr: *mut KevaCore) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr)); }
    }
}

/// Create key. Returns 0 on success, -1 on failure.
#[no_mangle]
pub extern "C" fn keva_core_create_key(ptr: *mut KevaCore, key: *const c_char) -> i32 {
    let core = unsafe {
        if ptr.is_null() { return -1; }
        &mut *ptr
    };
    let key = unsafe { CStr::from_ptr(key).to_str().unwrap_or_default() };
    match core.create(key) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Get content path. Caller must free with keva_string_free().
#[no_mangle]
pub extern "C" fn keva_core_content_path(ptr: *mut KevaCore, key: *const c_char) -> *mut c_char {
    // ... return CString::into_raw()
}

#[no_mangle]
pub extern "C" fn keva_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { drop(CString::from_raw(ptr)); }
    }
}
```

### C Header (Bridge)

**keva_core.h:**

```c
#ifndef KEVA_CORE_H
#define KEVA_CORE_H

typedef struct KevaCore KevaCore;

KevaCore* keva_core_new(const char* data_dir);
void keva_core_free(KevaCore* ptr);
int32_t keva_core_create_key(KevaCore* ptr, const char* key);
char* keva_core_content_path(KevaCore* ptr, const char* key);
void keva_string_free(char* ptr);

#endif
```

### Swift Side

**Keva-Bridging-Header.h:**

```c
#import "keva_core.h"
```

**KevaWrapper.swift:**

```swift
class KevaWrapper {
    private var ptr: OpaquePointer?
    
    init?(dataDir: String) {
        ptr = dataDir.withCString { keva_core_new($0) }
        if ptr == nil { return nil }
    }
    
    deinit {
        if let ptr = ptr { keva_core_free(ptr) }
    }
    
    func createKey(_ key: String) -> Bool {
        guard let ptr = ptr else { return false }
        return key.withCString { keva_core_create_key(ptr, $0) == 0 }
    }
    
    func contentPath(for key: String) -> String? {
        guard let ptr = ptr else { return nil }
        return key.withCString { cKey in
            guard let resultPtr = keva_core_content_path(ptr, cKey) else { return nil }
            defer { keva_string_free(resultPtr) }
            return String(cString: resultPtr)
        }
    }
}
```

### Memory Rules

| Allocated By   | Freed By | Function                    |
|----------------|----------|-----------------------------|
| Rust (pointer) | Rust     | `keva_core_free()`          |
| Rust (string)  | Rust     | `keva_string_free()`        |
| Swift (string) | Swift    | Automatic via `withCString` |

**Critical:** Never free Rust allocations with Swift, or vice versa.

### Type Mapping

| Swift           | C                     | Rust                  |
|-----------------|-----------------------|-----------------------|
| `Int32`         | `int32_t`             | `i32`                 |
| `Bool`          | `int32_t` (0/1)       | `i32`                 |
| `String`        | `char*`               | `*const c_char`       |
| `Data`          | `uint8_t*` + `size_t` | `*const u8` + `usize` |
| `OpaquePointer` | `struct X*`           | `*mut X`              |

### Build Integration

```bash
# Build Rust static libraries
cargo build --release --target aarch64-apple-darwin   # Apple Silicon
cargo build --release --target x86_64-apple-darwin    # Intel

# Create universal binary
lipo -create \
    target/aarch64-apple-darwin/release/libkeva_core.a \
    target/x86_64-apple-darwin/release/libkeva_core.a \
    -output libkeva_core.a

# Xcode setup:
# 1. Add libkeva_core.a to project
# 2. Add keva_core.h path to bridging header
# 3. Build Settings > Library Search Paths: directory containing .a file
```
