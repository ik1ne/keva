# M12: Edit/Preview Toggle - Implementation Plan

## Overview

Add a two-tab interface (Edit/Preview) to the right-top pane. Edit shows Monaco editor (existing), Preview shows
rendered markdown with inline images from attachments.

---

## Chat Agent Review

### Agreed Items

| Suggestion                       | Status  | Notes                                                    |
|----------------------------------|---------|----------------------------------------------------------|
| FileHandleRequest + key_hash     | Agreed  | Required for blob URL construction                       |
| Link transformation (JS regex)   | Agreed  | Pre-process before markdown-it                           |
| Non-image att: clicks → openFile | Revised | No message needed - NavigationStarting handles att: URLs |
| External links → ShellExecuteW   | Agreed  | NavigationStarting handler (same handler for both)       |
| DOMPurify sanitization           | Agreed  | XSS prevention required                                  |
| markdown-it + highlight.js       | Agreed  | Tables built-in, lighter than Prism                      |
| Caching rendered HTML            | Agreed  | Hash-based cache invalidation                            |
| Scroll preservation              | Agreed  | Save/restore on toggle                                   |

### Revised Items

| Original Suggestion                   | Revision                                                                                                                                          |
|---------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------|
| Add `keva.blobs` virtual host mapping | **Not needed** - existing `keva-data.local` already maps to data_path, so blobs accessible at `https://keva-data.local/blobs/{key_hash}/filename` |

### Code Block Limitation

Accept the limitation that `att:` links inside fenced code blocks will be incorrectly transformed. The two-pass
approach (extract→transform→restore) adds complexity for an unlikely edge case.

---

## Implementation Tasks

### Phase 1: Native Changes (key_hash Support)

**1.1 Add key_hash to FileHandleRequest**

File: `keva_windows/src/webview/mod.rs`

```rust
pub struct FileHandleRequest {
    pub key: String,
    pub key_hash: String,      // NEW: blake3 hex hash
    pub content_path: PathBuf,
    pub read_only: bool,
    pub attachments: Vec<AttachmentInfo>,
}
```

File: `keva_windows/src/keva_worker.rs` (handle_get_value)

```rust
use blake3;

fn handle_get_value(...) {
    let key_hash = blake3::hash(key.as_str().as_bytes()).to_hex().to_string();
    // Include in FileHandleRequest
}
```

File: `keva_windows/src/webview/platform/handlers.rs` (on_send_file_handle)

- Add `keyHash` to JSON message

**1.2 Add NavigationStarting handler for att: and external links**

File: `keva_windows/src/webview/init.rs`

```rust
wv.webview.add_NavigationStarting(..., |args| {
    let uri = args.Uri()?;

    // Allow internal navigation (our virtual hosts)
    if uri.starts_with("http://keva.local/") || uri.starts_with("https://keva-data.local/") {
        return Ok(());
    }

    // Handle att: scheme - open attachment file
    if uri.starts_with("att:") {
        args.SetCancel(true)?;
        let relative = &uri[4..];
        let path = get_data_path().join("blobs").join(relative);
        ShellExecuteW(None, "open", &path);
        return Ok(());
    }

    // All other URLs (http, https, mailto, tel, file, etc.) - delegate to OS
    args.SetCancel(true)?;
    ShellExecuteW(None, "open", &uri);
    Ok(())
});
```

No separate `openFile` message needed - NavigationStarting handles everything:
- Internal hosts (`keva.local`, `keva-data.local`) → allow navigation
- `att:{keyHash}/{filename}` → open file with default app
- Everything else (`http`, `https`, `mailto`, `tel`, etc.) → delegate to OS

---

### Phase 2: Frontend - Tab UI

**2.1 Add tab bar structure**

File: `index.html`

```html

<div class="right-pane">
    <div class="editor-tabs" id="editor-tabs">
        <button class="tab active" data-tab="edit">Edit</button>
        <button class="tab" data-tab="preview">Preview</button>
    </div>
    <div class="editor-viewport">
        <div id="editor-container"></div>
        <div id="preview-container" class="hidden"></div>
    </div>
    <!-- attachments unchanged -->
</div>
```

**2.2 Tab bar styles**

File: `styles.css`

```css
.editor-tabs {
    flex-shrink: 0;
    height: 36px;
    display: flex;
    gap: 4px;
    padding: 4px 8px;
}

.editor-tabs .tab {
    border: none;
    background: transparent;
    padding: 6px 12px;
    cursor: pointer;
}

.editor-tabs .tab.active {
    background: var(--bg-secondary);
    border-radius: 4px;
}

.editor-viewport {
    flex: 1;
    position: relative;
    overflow: hidden;
}

#preview-container {
    position: absolute;
    inset: 0;
    overflow: auto;
    padding: 16px;
}

#preview-container.hidden {
    display: none;
}
```

**2.3 Tab state management**

File: `state.js`

```javascript
State.data.editorMode = 'edit';  // 'edit' | 'preview'
```

File: `editor.js`

```javascript
Editor.keyHash = null;  // Set when loading content
Editor.previewCache = {hash: null, html: null};
```

---

### Phase 3: Frontend - Markdown Rendering

**3.1 Add dependencies (CDN)**

File: `index.html`

```html

<script src="https://cdn.jsdelivr.net/npm/markdown-it@14/dist/markdown-it.min.js"></script>
<script src="https://cdn.jsdelivr.net/npm/dompurify@3/dist/purify.min.js"></script>
<script src="https://cdn.jsdelivr.net/npm/@highlightjs/cdn-assets@11/highlight.min.js"></script>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@highlightjs/cdn-assets@11/styles/github-dark.min.css">
```

**3.2 Link transformation**

File: `editor.js` (new function)

Transform `att:` links based on markdown syntax:

- `![text](att:image.jpg)` → `![text](https://keva-data.local/blobs/{keyHash}/image.jpg)` (inline image)
- `[text](att:file)` → `[text](att:{keyHash}/file)` (clickable link via NavigationStarting)

```javascript
Editor.IMAGE_EXTENSIONS = new Set(['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'bmp', 'ico']);

Editor.isImageFile = function (filename) {
    const ext = filename.split('.').pop()?.toLowerCase() || '';
    return Editor.IMAGE_EXTENSIONS.has(ext);
};

Editor.transformAttLinks = function (markdown, keyHash, attachments) {
    const attSet = new Set(attachments.map(a => a.filename));
    // Match both ![text](att:file) and [text](att:file)
    return markdown.replace(/(!?)\[([^\]]*)\]\(att:([^)]+)\)/g, (match, bang, text, filename) => {
        if (!attSet.has(filename)) {
            return match;  // Keep broken links unchanged
        }
        const encodedName = encodeURIComponent(filename);
        if (bang && Editor.isImageFile(filename)) {
            // ![text](att:file) for images: render inline via virtual host
            return `![${text}](https://keva-data.local/blobs/${keyHash}/${encodedName})`;
        } else {
            // [text](att:file) or non-images: clickable link via NavigationStarting
            return `[${text}](att:${keyHash}/${encodedName})`;
        }
    });
};
```

**3.3 Render pipeline**

File: `editor.js` (new function)

```javascript
Editor.renderPreview = function () {
    const content = Editor.instance.getValue();
    const hash = Utils.hashString(content);

    if (Editor.previewCache.hash === hash) {
        return Editor.previewCache.html;
    }

    const md = window.markdownit({
        html: false,
        linkify: true,
        highlight: (str, lang) => {
            if (lang && hljs.getLanguage(lang)) {
                return hljs.highlight(str, {language: lang}).value;
            }
            return '';
        }
    });

    const transformed = Editor.transformAttLinks(content, Editor.keyHash, State.data.attachments);
    const rawHtml = md.render(transformed);
    const cleanHtml = DOMPurify.sanitize(rawHtml, {ADD_ATTR: ['target']});

    Editor.previewCache = {hash, html: cleanHtml};
    return cleanHtml;
};
```

**3.4 Broken link handling**

File: `styles.css`

```css
#preview-container img[src*="keva-data.local"]:not([src]) {
    display: inline-block;
    width: 24px;
    height: 24px;
    background: url('data:image/svg+xml,...') center/contain no-repeat;
}
```

Use `onerror` handler in JS to swap broken images with placeholder.

---

### Phase 4: Frontend - Tab Switching

**4.1 Tab click handling**

File: `main.js`

```javascript
function setupTabHandlers() {
    document.querySelectorAll('.editor-tabs .tab').forEach(tab => {
        tab.addEventListener('click', () => {
            const mode = tab.dataset.tab;
            if (State.data.editorMode === mode) return;

            switchEditorMode(mode);
        });
    });
}

function switchEditorMode(mode) {
    const editorContainer = document.getElementById('editor-container');
    const previewContainer = document.getElementById('preview-container');

    // Save scroll position before switch
    if (State.data.editorMode === 'preview') {
        Editor.previewScrollTop = previewContainer.scrollTop;
    } else {
        Editor.editScrollTop = Editor.instance.getScrollTop();
    }

    State.data.editorMode = mode;
    updateTabUI();

    if (mode === 'preview') {
        editorContainer.style.display = 'none';
        previewContainer.classList.remove('hidden');
        previewContainer.innerHTML = Editor.renderPreview();
        setupPreviewLinks(previewContainer);
        previewContainer.scrollTop = Editor.previewScrollTop || 0;
    } else {
        previewContainer.classList.add('hidden');
        editorContainer.style.display = '';
        Editor.instance.focus();
        if (Editor.editScrollTop != null) {
            Editor.instance.setScrollTop(Editor.editScrollTop);
        }
    }
}
```

**4.2 Image error handling**

File: `main.js`

```javascript
function setupPreviewImages(container) {
    container.querySelectorAll('img').forEach(img => {
        img.addEventListener('error', () => {
            img.classList.add('broken-image');
            img.title = 'Attachment not found: ' + img.alt;
        });
    });
}
```

Note: No JS click handling needed for links. NavigationStarting handles both:

- `att:{keyHash}/{filename}` → ShellExecuteW to open file
- `http(s)://...` → ShellExecuteW to open in browser

---

### Phase 5: Preview Styles

File: `styles.css`

```css
/* Markdown preview */
#preview-container {
    font-family: system-ui, sans-serif;
    font-size: 14px;
    line-height: 1.6;
    color: var(--text-primary);
}

#preview-container h1, h2, h3, h4, h5, h6 {
    margin-top: 1.5em;
    margin-bottom: 0.5em;
}

#preview-container p {
    margin: 1em 0;
}

#preview-container code {
    background: var(--bg-tertiary);
    padding: 2px 4px;
    border-radius: 3px;
}

#preview-container pre {
    background: var(--bg-tertiary);
    padding: 12px;
    border-radius: 6px;
    overflow-x: auto;
}

#preview-container pre code {
    background: none;
    padding: 0;
}

#preview-container img {
    max-width: 100%;
    border-radius: 4px;
}

#preview-container a {
    color: var(--accent);
}

#preview-container blockquote {
    border-left: 3px solid var(--border);
    padding-left: 12px;
    margin-left: 0;
    opacity: 0.9;
}

#preview-container table {
    border-collapse: collapse;
    width: 100%;
}

#preview-container th, td {
    border: 1px solid var(--border);
    padding: 8px;
}

/* Broken image placeholder */
.broken-image {
    display: inline-block;
    min-width: 24px;
    min-height: 24px;
    background: var(--bg-tertiary);
    border: 1px dashed var(--border);
    border-radius: 4px;
}
```

---

## Edge Cases

| Case                        | Handling                                        |
|-----------------------------|-------------------------------------------------|
| Switch key while in preview | Reset to Edit mode, load new content            |
| Trashed key                 | Both tabs work, content read-only               |
| Empty content               | Preview shows empty state                       |
| Large images                | CSS `max-width: 100%`                           |
| Missing attachment          | img onerror → placeholder + tooltip             |
| External link click         | NavigationStarting cancels, ShellExecuteW opens |
| `att:` in code block        | Incorrectly transformed (accepted limitation)   |

---

## Test Cases (from windows_milestone.md)

| TC        | Description                              | Implementation Notes                       |
|-----------|------------------------------------------|--------------------------------------------|
| TC-M12-01 | Edit tab shows Monaco editor             | Default state, click Edit tab              |
| TC-M12-02 | Preview tab shows rendered markdown      | Click Preview, verify HTML render          |
| TC-M12-03 | `![](att:img)` displays inline image     | Transform + `<img src="...">`              |
| TC-M12-04 | `[](att:file)` links are clickable       | Click → NavigationStarting → ShellExecuteW |
| TC-M12-05 | Preview updates when switching from Edit | Re-render on tab switch                    |
| TC-M12-06 | Preview is read-only                     | No cursor, no editing, scroll only         |
| TC-M12-07 | Broken att: link shows placeholder       | img onerror handler                        |
| TC-M12-08 | External links open in default browser   | NavigationStarting handler                 |

---

## File Changes Summary

### Native (Rust)

- `keva_windows/src/webview/mod.rs` - Add key_hash to FileHandleRequest
- `keva_windows/src/keva_worker.rs` - Calculate key_hash in handle_get_value
- `keva_windows/src/webview/platform/handlers.rs` - Include keyHash in JSON
- `keva_windows/src/webview/init.rs` - Add NavigationStarting handler for att: and http(s):// URLs

### Frontend (JS/HTML/CSS)

- `index.html` - Tab structure, CDN scripts
- `styles.css` - Tab styles, preview styles, broken image placeholder
- `state.js` - Add editorMode
- `editor.js` - Add keyHash, previewCache, renderPreview, transformAttLinks
- `main.js` - Tab handlers, preview link handlers

### Dependencies (CDN)

- markdown-it 14.x
- DOMPurify 3.x
- highlight.js 11.x

---

## Questions for Clarification

1. **Keyboard shortcut for tab toggle?** Consider `Ctrl+Shift+P` for preview toggle, or leave as mouse-only for M12?

2. **Tab visibility for trashed keys?** Both tabs visible (read-only for both) or hide Preview tab when trashed?

3. **Code syntax highlighting theme?** Use highlight.js github-dark for dark mode, github for light mode (switch on
   theme change)?
