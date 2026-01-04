# WebView2 CompositionController Migration

## Problem

When files are dragged from Explorer to WebView2 in standard Controller mode, JavaScript only receives `File` objects
with `name`, `size`, and `type`—no filesystem path. This is a web security restriction. Without the path, we cannot copy
files to Keva's storage.

## Solution

Use CompositionController mode, which gives native code full control over input events including drag-drop. Native
intercepts the drop, caches file paths, forwards the event to WebView2 so JS receives normal DOM events, then JS sends
back copy instructions referencing cached files by index.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ 1. User drops files from Explorer                           │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│ 2. Native IDropTarget::Drop receives IDataObject            │
│    → Extract paths via CF_HDROP format                      │
│    → Cache as Vec<PathBuf> (index-based)                    │
│    → Forward to composition_controller.Drop()               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│ 3. JS receives normal DOM 'drop' event                      │
│    → e.dataTransfer.files contains File objects             │
│    → Existing drop target detection works (editor vs pane)  │
│    → Check for filename conflicts against current files     │
│    → Show ConflictDialog if needed (overwrite/rename/skip)  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│ 4. JS sends resolution to native                            │
│    { type: "addDroppedFiles",                               │
│      key: "mykey",                                          │
│      files: [[0, "a.txt"], [1, "b (1).txt"]] }              │
│             └─ index    └─ resolved filename                │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│ 5. Native handles copy                                      │
│    → Look up source path from cache by index                │
│    → Copy to keva storage with resolved filename            │
│    → Clear cache                                            │
│    → Send updated attachments to JS                         │
└─────────────────────────────────────────────────────────────┘
```

## Implementation Tasks

### 1. DirectComposition Setup

- Create `IDCompositionDevice` via `DCompositionCreateDevice()`
- Create `IDCompositionTarget` for the window
- Create visual tree for compositing WebView2

### 2. WebView2 Creation Change

- Replace `CreateCoreWebView2Controller` with `CreateCoreWebView2CompositionController`
- Configure composition target via `put_RootVisualTarget()`

### 3. Input Forwarding (Client Area Only)

Current `WM_NCHITTEST` handling for resize borders stays unchanged. When `WM_NCHITTEST` returns `HTCLIENT`, forward
input to WebView2:

- WM_MOUSEMOVE, WM_LBUTTONDOWN/UP, WM_RBUTTONDOWN/UP → `SendMouseInput()`
- WM_MOUSEWHEEL → `SendMouseInput()` with wheel delta
- WM_KEYDOWN, WM_KEYUP, WM_CHAR, WM_SYSCHAR → `SendKeyboardInput()`

### 4. Cursor Management

- Subscribe to `CursorChanged` event on CompositionController
- Call `SetCursor()` with the cursor handle from the event

### 5. Focus Handling

- Call `MoveFocus()` when window gains/loses focus (WM_ACTIVATE, WM_SETFOCUS)

### 6. IDropTarget Implementation

Register parent window as drop target via `RegisterDragDrop()`:

- `DragEnter`: Extract paths from IDataObject, cache them, forward to controller
- `DragOver`: Forward to controller
- `DragLeave`: Clear cache, forward to controller
- `Drop`: Forward to controller (paths already cached from DragEnter)

### 7. Path Extraction from IDataObject

```rust
fn extract_paths_from_hdrop(data_obj: &IDataObject) -> Vec<PathBuf> {
    let format = FORMATETC {
        cfFormat: CF_HDROP.0 as u16,
        ptd: null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };

    let medium = data_obj.GetData(&format)?;
    let hdrop = HDROP(medium.u.hGlobal.0 as _);
    let count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);

    (0..count).map(|i| {
        let mut buf = [0u16; MAX_PATH];
        DragQueryFileW(hdrop, i, Some(&mut buf));
        PathBuf::from(OsString::from_wide(&buf))
    }).collect()
}
```

### 8. Message Protocol Addition

Add `addDroppedFiles` incoming message type:

```json
{
    "type": "addDroppedFiles",
    "key": "mykey",
    "files": [[0, "a.txt"], [1, "b (1).txt"]]
}
```

Format: `[[index, resolved_filename], ...]`

### 9. Backend Handler

- Look up source paths from cache by index
- Copy files to keva storage with resolved filenames
- Clear cache after processing
- Send `value` message with updated attachments

## Why Index-Based Matching

When dropping `project/mod.rs` and `utils/mod.rs`, JS sees two files both named `"mod.rs"`. The only reliable way to
match JS File objects back to native paths is by preserving order (index 0, index 1, etc.).

## Cache Considerations

- Clear cache after successful copy operation
- Clear cache on `DragLeave` (user dragged away without dropping)
- Cache is per-drop; a new `DragEnter` replaces the previous cache
