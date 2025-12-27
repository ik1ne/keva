# Electron and Chromium Frameless Window Implementation on Windows

Windows frameless window implementation in Electron and Chromium requires coordinated handling of **WM_NCCALCSIZE**, *
*WM_NCHITTEST**, and DWM APIs to eliminate resize jitter while preserving system functionality like Aero Snap. The key
insight: returning `0` from WM_NCCALCSIZE extends the client area to fill the entire window, but preventing
BitBlt-induced jitter requires either `WVR_VALIDRECTS` manipulation or `SWP_NOCOPYBITS` flags. Chromium's solution
combines a `ScopedRedrawLock` mechanism with careful window style selection—keeping `WS_THICKFRAME` for resize
functionality while hiding the visual frame through non-client area calculation.

## WM_NCCALCSIZE handling eliminates the visible frame

When `wParam` is `TRUE`, the `lParam` points to an `NCCALCSIZE_PARAMS` structure with three rectangles and window
position information:

```cpp
typedef struct tagNCCALCSIZE_PARAMS {
    RECT rgrc[3];      // Array of rectangles
    PWINDOWPOS lppos;  // Window position info
} NCCALCSIZE_PARAMS;
```

**On input**, `rgrc[0]` contains the new proposed window coordinates, `rgrc[1]` holds the old window coordinates, and
`rgrc[2]` stores the old client area. **On output**, `rgrc[0]` defines the new client rectangle, while `rgrc[1]` and
`rgrc[2]` become destination and source rectangles for Windows' BitBlt preservation operation.

Electron's historical implementation in `shell/browser/native_window_views_win.cc` demonstrates the pattern:

```cpp
case WM_NCCALCSIZE: {
    if (!has_frame() && w_param == TRUE) {
        NCCALCSIZE_PARAMS* params = reinterpret_cast<NCCALCSIZE_PARAMS*>(l_param);
        RECT PROPOSED = params->rgrc[0];
        RECT BEFORE = params->rgrc[1];
        
        // Call DefWindowProc for cascade/tile window support
        DefWindowProcW(GetAcceleratedWidget(), WM_NCCALCSIZE, w_param, l_param);
        
        params->rgrc[0] = PROPOSED;  // Use full window as client area
        params->rgrc[1] = BEFORE;
        return true;
    }
    return false;  // Let Chromium handle framed windows
}
```

The current Electron approach (PR #21164) delegates entirely to Chromium, which calculates client area insets through
`GetClientAreaInsets()` in `ui/views/win/hwnd_message_handler.cc`. This preserves compatibility with Windows features
like Tile & Cascade.

## Top-left resize jitter stems from BitBlt origin assumptions

The resize jitter problem occurs because Windows performs a **BitBlt copy** of old client area pixels before your
application redraws. When dragging the right or bottom border, the window origin stays fixed and content simply expands
at the drag edge—BitBlt preserves pixels at their original position with no visible shift. However, when dragging the *
*left or top border**, the window origin moves. Windows assumes the top-left corner is the "anchor" and BitBlts old
pixels to maintain that anchor. Your application then redraws at the correct position, creating a visible "jump" as
content shifts right then back left.

The return value from WM_NCCALCSIZE controls this BitBlt behavior:

| Return Value              | Effect                                                                            |
|---------------------------|-----------------------------------------------------------------------------------|
| `0`                       | Default—old client area preserved at upper-left, causes jitter on left/top resize |
| `WVR_VALIDRECTS` (0x0400) | Tells Windows that `rgrc[1]` and `rgrc[2]` contain valid BitBlt rectangles        |
| `WVR_REDRAW` (0x0300)     | Forces entire window redraw, combines `WVR_HREDRAW                                | WVR_VREDRAW` |

**Solution 1: Disable BitBlt via WVR_VALIDRECTS**

Set both destination and source rectangles to the same 1-pixel area, effectively nullifying the copy operation:

```cpp
case WM_NCCALCSIZE:
    if (wParam == TRUE) {
        NCCALCSIZE_PARAMS* params = (NCCALCSIZE_PARAMS*)lParam;
        DefWindowProc(hwnd, WM_NCCALCSIZE, wParam, lParam);
        
        // Both rectangles point to same location = no pixels actually move
        params->rgrc[1] = params->rgrc[2] = {0, 0, 1, 1};
        return WVR_VALIDRECTS;
    }
    break;
```

**Solution 2: Intercept WM_WINDOWPOSCHANGING**

```cpp
case WM_WINDOWPOSCHANGING:
    DefWindowProc(hwnd, msg, wParam, lParam);
    WINDOWPOS* wp = (WINDOWPOS*)lParam;
    wp->flags |= SWP_NOCOPYBITS;  // Disable the internal BitBlt entirely
    return 0;
```

## WM_NCHITTEST enables custom drag regions and Aero Snap

The `-webkit-app-region: drag` CSS property flows from renderer to native through a well-defined IPC path. In Blink,
`LocalFrameView::UpdateDocumentDraggableRegions()` collects all elements with the `app-region` property into `SkRegion`
objects (rectangular areas combined via union/XOR operations). This data travels via
`WebViewImpl::DraggableRegionsChanged()` through IPC to the browser process, where
`NativeWindow::UpdateDraggableRegions()` stores the combined region.

Electron's `FramelessView::NonClientHitTest()` implements the actual hit testing:

```cpp
int FramelessView::NonClientHitTest(const gfx::Point& point) {
    // Check resize borders first if resizable
    if (frame_->widget()->widget_delegate()->CanResize()) {
        int result = GetHTComponentForFrame(point, ...);
        if (result != HTNOWHERE)
            return result;
    }
    
    // Check if point is in draggable region
    if (native_window_->draggable_region().Contains(point.x(), point.y())) {
        return HTCAPTION;  // Enables window dragging + Aero Snap
    }
    
    return HTCLIENT;
}
```

Returning `HTCAPTION` automatically enables Aero Snap because Windows treats the area as a draggable title bar. The
system handles window dragging, edge snapping, double-click maximize, and right-click system menu. Resize handles use
`HTLEFT`, `HTRIGHT`, `HTTOP`, `HTBOTTOM`, and corner combinations (`HTTOPLEFT`, etc.):

```cpp
int GetHTComponentForFrame(const gfx::Point& point, int width, int height) {
    const int border = 8;
    bool left = point.x() < border;
    bool right = point.x() >= width - border;
    bool top = point.y() < border;
    bool bottom = point.y() >= height - border;
    
    if (top && left) return HTTOPLEFT;
    if (top && right) return HTTOPRIGHT;
    if (bottom && left) return HTBOTTOMLEFT;
    if (bottom && right) return HTBOTTOMRIGHT;
    if (left) return HTLEFT;
    if (right) return HTRIGHT;
    if (top) return HTTOP;
    if (bottom) return HTBOTTOM;
    return HTNOWHERE;
}
```

A critical 2023 Chromium change (crrev.com/c/4814003) disabled app-region collection by default for performance.
Electron now explicitly calls `SetSupportsAppRegion(true)` for frames that need draggable region support.

## DwmExtendFrameIntoClientArea with negative margins creates the sheet of glass effect

Negative margin values (`-1`) tell DWM to render the client area as a **solid surface with no window border**:

```cpp
HRESULT ExtendIntoClientAll(HWND hwnd) {
    MARGINS margins = {-1};  // All fields set to -1
    return DwmExtendFrameIntoClientArea(hwnd, &margins);
}
```

For frameless windows that want DWM-drawn shadows without visible frame, use small positive margins:

```cpp
const MARGINS shadow_margins = {1, 1, 1, 1};
DwmExtendFrameIntoClientArea(hwnd, &shadow_margins);
```

This minimal frame extension triggers DWM shadow drawing while keeping the appearance essentially frameless. The call
should occur in `WM_ACTIVATE` rather than `WM_CREATE` to handle maximized state correctly, and must be repeated whenever
DWM composition toggles (handle `WM_DWMCOMPOSITIONCHANGED`).

**WS_THICKFRAME** (`0x00040000L`, identical to `WS_SIZEBOX`) is often required even for visually frameless windows
because it provides DWM shadow drawing, creates transparent resize areas on edges, enables minimize/maximize animations
with corresponding box styles, and triggers `WM_GETMINMAXINFO` for proper size handling. The transparent resize areas
exist in the non-client region—when you extend the client area via WM_NCCALCSIZE, these areas become "covered" but still
function for resizing.

**WS_EX_NOREDIRECTIONBITMAP** (`0x00200000L`) tells Windows not to allocate a default redirection bitmap, essential when
presenting content via DirectComposition swap chains. However, it can cause flicker during resize if rendering takes
longer than a frame—the window briefly shows content behind it.

## Chromium uses ScopedRedrawLock and SWP_NOCOPYBITS for flicker prevention

Chromium's `hwnd_message_handler.cc` contains sophisticated flicker prevention. The `ScopedRedrawLock` class prevents
window redrawing during operations that might trigger unwanted non-client painting:

```cpp
// Messages requiring ScopedRedrawLock:
// - WM_SETTEXT
// - WM_SETICON  
// - WM_NCLBUTTONDOWN
// - EnableMenuItem (from WM_INITMENU)
```

The lock uses a `~WS_VISIBLE` technique—temporarily making the window invisible to prevent redraws. It only applies to
visible windows without child rendering windows and is disabled when DirectComposition is used (where `WS_CLIPCHILDREN`
handles the issue).

**SWP_NOCOPYBITS** appears throughout Chromium's SetWindowPos calls:

```cpp
SetWindowPos(hwnd(), nullptr, 0, 0, 0, 0,
    SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOCOPYBITS | SWP_NOMOVE |
    SWP_NOOWNERZORDER | SWP_NOREPOSITION | SWP_NOSENDCHANGING |
    SWP_NOSIZE | SWP_NOZORDER);
```

Key Electron PRs addressing resize flicker include **#21164** (allows Chromium to handle WM_NCCALCSIZE properly for
frameless windows, fixing maximize/restore flickering), **#35189** (fixed Windows 7-style frames appearing on frameless
resizable windows due to WS_THICKFRAME timing), and **#8404** (fixed borders/flickering on high-DPI displays through
proper non-client event handling).

## VS Code and Discord implement custom title bars with careful timing

Both applications use `frame: false` in BrowserWindow options combined with HTML/CSS title bars using
`-webkit-app-region: drag`. Critical implementation details:

```javascript
const mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    frame: false,
    backgroundColor: '#1A2933',  // Prevents white flash, enables subpixel AA
    show: false,                 // Don't show until ready
    webPreferences: {preload: path.join(__dirname, 'preload.js')}
});

mainWindow.on('ready-to-show', () => mainWindow.show());
```

The CSS pattern requires explicit `no-drag` on all interactive elements:

```css
#titlebar {
    -webkit-app-region: drag;
    -webkit-user-select: none;
    height: 32px;
}

#window-controls {
    -webkit-app-region: no-drag; /* Essential for button clicks */
}
```

Known limitations: drag regions consume all click events (cannot capture clicks on dragged areas), only rectangular
shapes are supported, and DevTools being open breaks drag functionality (workaround: detach to separate window).

## Complete frameless window recipe for Windows

**Window style selection:**

```cpp
// Frameless with full system functionality
DWORD style = WS_POPUP | WS_THICKFRAME | WS_CAPTION | 
              WS_MAXIMIZEBOX | WS_MINIMIZEBOX;
DWORD exStyle = WS_EX_APPWINDOW;  // Taskbar presence

HWND hwnd = CreateWindowEx(exStyle, className, title, style, ...);
```

**Message handling order:**

1. **WM_CREATE**: Call `SetWindowPos` with `SWP_FRAMECHANGED` to trigger WM_NCCALCSIZE
2. **WM_ACTIVATE**: Call `DwmExtendFrameIntoClientArea` with margins `{1,1,1,1}` for shadow
3. **WM_NCCALCSIZE**: Return `0` when `wParam == TRUE` to extend client area
4. **WM_NCHITTEST**: Return appropriate hit test values for resize borders and caption
5. **WM_NCACTIVATE**: Return `1` to prevent border redraw on focus change

**Complete WndProc pattern from Microsoft's DWM documentation:**

```cpp
LRESULT CALLBACK WndProc(HWND hWnd, UINT message, WPARAM wParam, LPARAM lParam) {
    bool fCallDWP = true;
    LRESULT lRet = 0;
    
    // Let DWM handle caption button hit testing first
    BOOL fDwmEnabled = FALSE;
    DwmIsCompositionEnabled(&fDwmEnabled);
    if (fDwmEnabled) {
        fCallDWP = !DwmDefWindowProc(hWnd, message, wParam, lParam, &lRet);
    }
    
    if (message == WM_CREATE) {
        RECT rc;
        GetWindowRect(hWnd, &rc);
        SetWindowPos(hWnd, NULL, rc.left, rc.top,
                     rc.right - rc.left, rc.bottom - rc.top,
                     SWP_FRAMECHANGED);
    }
    
    if (message == WM_ACTIVATE) {
        MARGINS margins = {1, 1, 1, 1};
        DwmExtendFrameIntoClientArea(hWnd, &margins);
    }
    
    if ((message == WM_NCCALCSIZE) && (wParam == TRUE)) {
        return 0;  // Extend client area to entire window
    }
    
    if ((message == WM_NCHITTEST) && (lRet == 0)) {
        lRet = CustomHitTest(hWnd, wParam, lParam);
        if (lRet != HTNOWHERE) fCallDWP = false;
    }
    
    if (fCallDWP) lRet = DefWindowProc(hWnd, message, wParam, lParam);
    return lRet;
}
```

**Undocumented behaviors and workarounds:**

- `WS_THICKFRAME` must be present in styles passed to `CreateWindowEx`—adding it later via `SetWindowLong` triggers
  multiple `WM_DWMNCRENDERINGCHANGED` messages causing visible frame flash
- Chromium returns `WVR_REDRAW` from WM_NCCALCSIZE (hwnd_message_handler.cc:2353), which triggers `WM_NCPAINT`
- The first WM_NCCALCSIZE must be passed to DefWindowProc for windows with `WS_CAPTION` so Windows updates internal
  caption-present structures (required for Tile & Cascade functionality)
- `DwmFlush()` before `SwapBuffers()` can synchronize with DWM composition to prevent jitter when DWM vsync and WGL
  vsync conflict

## Conclusion

The complete solution for jitter-free frameless windows combines **WS_THICKFRAME** in initial window styles (never added
later), **WM_NCCALCSIZE returning 0** to extend client area, **WM_NCHITTEST returning HTCAPTION** for drag regions, *
*DwmExtendFrameIntoClientArea with small margins** for shadows, and **SWP_NOCOPYBITS** on any SetWindowPos calls during
resize. The Electron/Chromium codebase demonstrates that the `ScopedRedrawLock` pattern—temporarily hiding windows
during operations that trigger non-client painting—provides the most robust flicker prevention for complex applications.
Modern Windows 10/11 applications can also leverage `titleBarOverlay: true` in Electron for native window controls with
custom content below, simplifying the implementation significantly while maintaining native resize behavior.