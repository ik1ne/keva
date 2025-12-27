# Single WebView Architecture for Keva Windows

Keva Windows uses a single full-window WebView2 instead of D2D rendering or multiple WebViews. This document explains why this approach works and the technical details enabling it.

## Why Single WebView

### Previous Approach (D2D + Multiple WebViews)

The initial design used:
- Direct2D for custom rendering (search bar, key list)
- Two separate WebViews (search input, Monaco editor)
- Complex coordination between native rendering and WebViews

**Problems encountered:**
- Resize jitter from D2D/WebView boundary synchronization
- Complex hit-testing for drag regions across multiple surfaces
- Duplicate styling (CSS in WebViews, colors in D2D code)
- High complexity for relatively simple UI

### Current Approach (Single WebView)

One WebView2 fills the entire window (minus resize borders), rendering all UI:
- Search bar with drag handle
- Key list (left pane)
- Monaco editor (right pane)

**Benefits:**
- Single source of truth for UI (one HTML/CSS/JS file)
- No rendering boundary issues
- Simpler resize handling
- CSS `app-region: drag` handles window dragging natively

## Key Technical Enablers

### 1. CSS app-region for Window Dragging

WebView2 supports the `-webkit-app-region` CSS property when `SetIsNonClientRegionSupportEnabled(true)` is called:

```rust
// webview.rs
if let Ok(settings9) = settings.cast::<ICoreWebView2Settings9>() {
    let _ = settings9.SetIsNonClientRegionSupportEnabled(true);
}
```

```css
/* app.html */
.search-icon {
    -webkit-app-region: drag;
    app-region: drag;
    cursor: move;
}

.search-input {
    -webkit-app-region: no-drag;
    app-region: no-drag;
}
```

When the user drags an element with `app-region: drag`, WebView2 reports `HTCAPTION` to Windows, triggering native window drag behavior including Aero Snap.

### 2. Borderless Window with Resize Borders

The window uses `WS_POPUP | WS_SIZEBOX` for a borderless but resizable frame:

```rust
// window.rs
let style = WS_POPUP | WS_SIZEBOX | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU | WS_CLIPCHILDREN;
```

The WebView is inset by `RESIZE_BORDER` (5px) on all sides, leaving native Win32 hit-testing areas:

```
┌─────────────────────────────────┐
│ 5px resize border (native)      │
│ ┌─────────────────────────────┐ │
│ │                             │ │
│ │     WebView2 (full UI)      │ │
│ │                             │ │
│ └─────────────────────────────┘ │
│ 5px resize border (native)      │
└─────────────────────────────────┘
```

`WM_NCHITTEST` returns `HTLEFT`, `HTRIGHT`, etc. for the border areas, enabling native resize cursors and behavior.

### 3. Maximized/Snapped State Handling

When the window is maximized or snapped (Aero Snap), the resize borders are unnecessary. `WM_SIZE` detects this and removes the insets:

```rust
// window.rs - WM_SIZE handler
let size_type = wparam.0 as u32;
let is_maximized = size_type == 2; // SIZE_MAXIMIZED

let (wv_x, wv_y, wv_width, wv_height) = if is_maximized {
    (0, 0, width, height)  // Full window
} else {
    (RESIZE_BORDER, RESIZE_BORDER, width - 2 * RESIZE_BORDER, height - 2 * RESIZE_BORDER)
};
wv.set_bounds(wv_x, wv_y, wv_width, wv_height);
```

### 4. Jitter-Free Resize

Two techniques prevent white flashes during resize:

**WM_NCCALCSIZE with WVR_VALIDRECTS:**
```rust
// Nullify BitBlt source/dest rectangles
(*params).rgrc[1] = RECT { left: 0, top: 0, right: 1, bottom: 1 };
(*params).rgrc[2] = (*params).rgrc[1];
return LRESULT(WVR_VALIDRECTS as isize);
```

**WM_WINDOWPOSCHANGING with SWP_NOCOPYBITS:**
```rust
let wp = lparam.0 as *mut WINDOWPOS;
(*wp).flags |= SWP_NOCOPYBITS;
```

Both disable Windows' BitBlt optimization that causes jitter when resizing from top/left edges.

### 5. Dark Background for Resize Borders

The 5px resize border areas are painted with a dark brush matching the WebView background:

```rust
// window.rs
let bg_brush = CreateSolidBrush(COLORREF(0x001a1a1a)); // #1a1a1a in BGR
let wc = WNDCLASSW {
    hbrBackground: bg_brush,
    // ...
};
```

WebView2's default background is also set to match:

```rust
// webview.rs
let dark_bg = COREWEBVIEW2_COLOR { A: 255, R: 26, G: 26, B: 26 };
controller2.SetDefaultBackgroundColor(dark_bg);
```

## Bidirectional Message Bridge

Communication between Rust (native) and JavaScript (WebView) uses WebView2's message passing:

**WebView → Native:**
```javascript
// app.html
window.chrome.webview.postMessage(JSON.stringify({ type: 'search', query: '...' }));
```

```rust
// webview.rs - WebMessageReceivedEventHandler
fn handle_webview_message(webview: Option<&ICoreWebView2>, msg: &str) {
    if let Some(msg_type) = parse_message_type(msg) {
        match msg_type {
            "ready" => { /* respond with init */ }
            "search" => { /* handle search */ }
            // ...
        }
    }
}
```

**Native → WebView:**
```rust
// webview.rs
let response = r#"{"type":"init","timestamp":123}"#;
wv.PostWebMessageAsJson(pwstr_from_str(&response));
```

```javascript
// app.html
window.chrome.webview.addEventListener('message', event => {
    const msg = event.data;
    switch (msg.type) {
        case 'init': /* handle init */ break;
        case 'keys': /* update key list */ break;
        // ...
    }
});
```

## Trade-offs

### Advantages
- **Simplicity**: One HTML file defines entire UI
- **Consistency**: Single styling system (CSS)
- **Web ecosystem**: Monaco, future libraries trivially integrated
- **No coordination bugs**: No D2D/WebView synchronization issues

### Limitations
- **Startup latency**: WebView2 initialization adds ~100-200ms
- **Memory**: WebView2 process uses more memory than pure native
- **Native controls**: Can't embed IPreviewHandler or other HWND-based controls directly

### Mitigations
- **Startup**: Window shown immediately with dark background; WebView loads async
- **Memory**: Acceptable for desktop app (WebView2 is shared across apps)
- **Native controls**: Handle common formats in WebView (images, text, PDF.js); files show icon + "Open in default app"

## Conclusion

Single WebView architecture trades some native control flexibility for significant complexity reduction. For Keva's use case (text snippets with occasional file attachments), the benefits outweigh the limitations. The approach aligns with modern Electron/Tauri patterns while using native Win32 for window management.
