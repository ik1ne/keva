# Keva GUI Implementation Plan

## Context

Keva is a local key-value store for clipboard-like data. The core library (`keva_core`) is implemented. This document
describes the GUI implementation using egui/eframe.

**Reference documents:**

- `Spec.md` - Product specification (source of truth for behavior)
- `implementation_detail.md` - keva_core and keva_gui API reference
- `Planned.md` - Future features (not in scope)

**Note:** Search functionality lives in `keva_gui::search`, not `keva_core`.

**Project structure:**

```
keva/
├── core/           # keva_core (implemented)
├── gui/            # keva_gui (this implementation)
├── Spec.md
├── Planned.md
└── implementation_detail.md
```

---

## Dependencies

Current `gui/Cargo.toml` dependencies:

```toml
[dependencies]
keva_core = { path = "../core" }
nucleo = "0.5"  # For search
```

Additional dependencies needed for GUI:

```toml
eframe = "0.29"
egui = "0.29"
toml = "0.8"
serde = { version = "1", features = ["derive"] }
dirs = "5"
```

---

## Milestones

### M1: Window Skeleton

**Goal:** Render three-pane layout with placeholder content.

**Tasks:**

1. Create `gui/src/main.rs` with eframe app setup
2. Create `gui/src/app.rs` with `KevaApp` struct implementing `eframe::App`
3. Implement layout:
    - Top: Search bar (text input, non-functional)
    - Left: Key list panel (static placeholder text)
    - Right: Inspector panel (static placeholder text)
4. Use `egui::TopBottomPanel` for search bar, `egui::SidePanel` for left pane, `egui::CentralPanel` for right pane

**Acceptance criteria:**

- `cargo run -p keva_gui` opens window
- Three distinct panels visible
- Window title: "Keva"

---

### M2: Config Loading + Core Integration

**Goal:** Load configuration and initialize keva_core.

**Tasks:**

1. Create `gui/src/config.rs`:
    - Define `GuiConfig` struct matching Spec.md Section 5
    - Implement `load(data_dir: PathBuf) -> Result<GuiConfig, ConfigError>`
    - Implement `save(&self, data_dir: PathBuf) -> Result<(), ConfigError>`
    - Implement validation with specific error messages per field
2. Create `gui/src/error.rs` for GUI-specific errors
3. Update `main.rs`:
    - Read `KEVA_DATA_DIR` env var or use default (`~/.keva/`)
    - Load config.toml (create with defaults if missing)
    - Show validation error popup if invalid (see Spec.md Section 5 Config Validation)
    - Initialize `KevaCore::open(data_dir, config)`
4. Store `KevaCore` instance in `KevaApp`
5. Display key count in left pane (call `keva_core.active_keys()`)

**Acceptance criteria:**

- App launches with valid config
- App shows error popup with invalid config
- Left pane shows "X keys" from actual database

**Config struct:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiConfig {
    pub delete_style: DeleteStyle,           // "soft" or "immediate"
    pub large_file_threshold: u64,           // bytes
    pub trash_ttl: u64,                      // seconds
    pub purge_ttl: u64,                      // seconds
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeleteStyle {
    Soft,
    Immediate,
}
```

---

### M3: Key List Display

**Goal:** Display actual keys in left pane.

**Tasks:**

1. Fetch keys from `keva_core.active_keys()` and `keva_core.trashed_keys()`
2. Render scrollable list in left pane using `egui::ScrollArea`
3. Each key as selectable label (`ui.selectable_label`)
4. Track selected key in `KevaApp` state: `selected_key: Option<Key>`
5. Trashed keys shown at bottom with 🗑️ prefix
6. Clicking key updates `selected_key`

**Acceptance criteria:**

- All active keys displayed
- Trashed keys displayed at bottom with icon
- Clicking key highlights it
- Selection state persists

---

### M4: Search Bar Integration

**Goal:** Fuzzy search filters key list.

**Tasks:**

1. Add `search_query: String` to `KevaApp` state
2. Create `SearchEngine` instance in `KevaApp` (from `keva_gui::search`)
3. Bind search bar input to `search_query`
4. When `search_query` changes:
    - Call `search_engine.set_query(SearchQuery::Fuzzy(query))`
5. Each frame:
    - Call `search_engine.tick()` for non-blocking updates
    - Get results via `search_engine.active_results().iter()` and `trashed_results().iter()`
6. When `search_query` is empty:
    - Show all keys (active + trashed)
7. Clear selection when search query changes

**Acceptance criteria:**

- Typing filters key list
- Results ranked by relevance
- Empty query shows all keys
- Search responds within 100ms for typical queries

---

### M5: Right Pane Read-Only

**Goal:** Display value for selected key.

**Tasks:**

1. When key selected, call `keva_core.get(key)`
2. Display based on value type:
    - **Text:** Show text content in read-only text area
    - **Files:** Show file list with names and sizes
    - **None:** Show placeholder text
3. When no key selected but search bar has text:
    - Use search bar text as target key
    - Show empty state placeholder: `Write or paste value for "<key>"`

**Acceptance criteria:**

- Selecting key shows its value
- Text values displayed correctly
- File values show list of filenames
- Empty/new keys show placeholder

---

### M6: Right Pane Editing

**Goal:** Edit text values, create new keys.

**Tasks:**

1. Replace read-only text area with `egui::TextEdit::multiline`
2. Track edit state: `editing_text: Option<String>`, `last_edit_time: Instant`
3. Auto-save logic:
    - On text change, update `last_edit_time`
    - Each frame, check if `now - last_edit_time > 3 seconds` and text differs from stored
    - If so, call `keva_core.upsert_text(key, text, now)` and refresh key list
4. For new keys (search bar text, no existing value):
    - First text input creates key via `upsert_text`
5. Handle paste (`Ctrl+V` / `Cmd+V`):
    - Check clipboard content type
    - If text: insert at cursor (default egui behavior)
    - If files and current value is text: show hint "Clear text to paste files"
    - If files and current value is empty/files: call `keva_core.import_clipboard(key, now)`

**Acceptance criteria:**

- Text editing works
- Auto-save triggers after 3 seconds of inactivity
- New keys can be created by typing in search bar + entering text
- File paste blocked when text exists (with hint)

---

### M7: Left Pane Controls

**Goal:** Rename and delete keys.

**Tasks:**

1. On hover/selection, show control buttons:
    - Rename button (✏️ or pen icon)
    - Delete button (🗑️ or trash icon)
2. Rename flow:
    - Click button → key text becomes editable
    - Enter confirms, Escape cancels
    - If new key exists, show confirmation dialog
    - Call `keva_core.rename(old, new, overwrite)`
3. Delete flow:
    - Check `config.delete_style`
    - If `Soft`: call `keva_core.trash(key, now)`
    - If `Immediate`: call `keva_core.purge(key)`
    - Refresh key list

**Acceptance criteria:**

- Hover shows buttons
- Rename works with confirmation for overwrites
- Delete respects configured delete style
- Key list refreshes after operations

---

### M8: Keyboard Shortcuts

**Goal:** Implement keyboard navigation per Spec.md.

**Tasks:**

1. Handle keyboard input in `KevaApp::update`:
    - `Enter` with key selected: focus right pane for editing
    - `Enter` with no selection + search text: focus right pane (creates key if new)
    - `Shift+Enter` with key selected: copy to clipboard, hide window
    - `Cmd+,` (macOS) / `Ctrl+,` (Windows): open settings dialog
2. For `Shift+Enter`:
    - Call `keva_core.copy_to_clipboard(key, now)`
    - Call `frame.set_visible(false)` or minimize
3. Track focus state to enable/disable shortcuts appropriately

**Acceptance criteria:**

- All shortcuts from Spec.md work
- `Shift+Enter` copies and hides window
- `Cmd+,` opens settings (see M9)

---

### M9: Settings Dialog

**Goal:** Implement settings UI.

**Tasks:**

1. Create `gui/src/settings.rs` with `SettingsDialog` struct
2. Track dialog state in `KevaApp`: `settings_open: bool`, `settings_draft: GuiConfig`
3. When `Cmd+,` pressed:
    - Set `settings_open = true`
    - Clone current config to `settings_draft`
4. Render settings as modal or separate window:
    - Delete Style: dropdown (Soft/Immediate)
    - Large File Threshold: number input with MB label
    - Trash TTL: number input with days label
    - Purge TTL: number input with days label
5. On dialog close:
    - Save `settings_draft` to config.toml
    - Call `keva_core.update_config(...)` (requires keva_core change)
    - Set `settings_open = false`

**keva_core change required:**

```rust
impl KevaCore {
    pub fn update_config(&mut self, config: KevaConfig) { ... }
}
```

**Acceptance criteria:**

- Settings dialog opens on `Cmd+,`
- All settings editable
- Changes persist to config.toml
- Changes applied immediately

---

### M10: Drag & Drop

**Goal:** Accept file drops.

**Tasks:**

1. Enable drag-drop in eframe: `NativeOptions { drag_and_drop_support: true, .. }`
2. Handle `egui::Event::DroppedFile` events
3. Determine drop target:
    - If dropped on left pane key: use that key
    - If dropped on right pane: use current target key
4. Check file size against `large_file_threshold`:
    - If exceeds: show confirmation dialog
    - If confirmed or under threshold: call `keva_core.add_files(key, paths, now)`
5. Refresh display after drop

**Acceptance criteria:**

- Files can be dropped on right pane
- Files can be dropped on specific keys in left pane
- Large files trigger confirmation
- Dropped files stored correctly

---

### M11: Window Lifecycle + GC

**Goal:** Handle window close and garbage collection.

**Tasks:**

1. On window close event:
    - Trigger auto-save if pending edits
    - Call `keva_core.maintenance(now)`
    - Exit application
2. Ensure single-instance behavior (optional for v1):
    - Check for existing instance on launch
    - If exists, activate existing window instead of launching new

**Acceptance criteria:**

- Pending edits saved on close
- GC runs on close
- App exits cleanly

---

## File Structure

Final `gui/` structure:

```
gui/
├── Cargo.toml
└── src/
    ├── main.rs          # Entry point, config loading, error popups
    ├── app.rs           # KevaApp struct, main update loop
    ├── config.rs        # GuiConfig, load/save/validate
    ├── settings.rs      # Settings dialog
    ├── panels/
    │   ├── mod.rs
    │   ├── search_bar.rs
    │   ├── key_list.rs
    │   └── inspector.rs
    └── error.rs         # GUI error types
```

---

## Notes

- Use `std::time::SystemTime::now()` for `now` parameter in keva_core write/lifecycle operations
- `get(key)` does not require timestamp (returns raw DB state)
- `rename(old, new, overwrite)` does not require timestamp
- Search is managed separately via `SearchEngine` from `keva_gui::search`
- Sync search index manually: `search_engine.add_active(key)`, `search_engine.trash(key)`, etc.
- Test with small dataset first, then verify performance with larger key counts
- Refer to Spec.md for exact UI behavior details