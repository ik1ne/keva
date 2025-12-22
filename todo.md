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

```toml
[dependencies]
# Core
keva_core = { path = "../core" }
nucleo = "0.5"
eframe = "0.33"

# System integration
tray-icon = "0.21"
global-hotkey = "0.7"

# Config
toml = "0.8"
serde = { version = "1", features = ["derive"] }
dirs = "5"
```

---

## Milestone Overview

| Phase | Milestones | Description |
|-------|------------|-------------|
| Foundation | M1-M2 | Window skeleton, config loading |
| Core Features | M3-M6 | Key list, search, editing, controls |
| System Integration | M7-M11 | Tray, hotkey, single instance, login, first-run |
| Polish | M12-M15 | Settings, shortcuts, drag/drop, lifecycle |

---

## Milestones

### M1: Window Skeleton + Custom Chrome ✓ (DONE)

Already implemented with eframe 0.33.

**Completed:**

- Borderless window (`with_decorations(false)`)
- 5px resize border + 3px drag border
- Search icon drag handle
- Esc hides (when focused), Cmd+Q quits (when focused)
- Three-pane layout (search bar, key list, inspector)

**Files:** `gui/src/app.rs`, `gui/src/theme.rs`, `gui/src/panels/*`

---

### M2: Config + Core Integration

**Goal:** Load full config, initialize keva_core, theme support.

**Tasks:**

1. Create `gui/src/config.rs` with config struct:
    ```rust
    pub struct GuiConfig {
        pub config_version: u32,
        pub theme: Theme,              // dark/light/system
        pub global_shortcut: String,
        pub launch_at_login: bool,
        pub show_tray_icon: bool,
        pub delete_style: DeleteStyle,
        pub large_file_threshold: u64,
        pub trash_ttl: u64,
        pub purge_ttl: u64,
        pub window: WindowConfig,      // per-monitor positions
    }
    ```
2. Implement load/save/validate with config_version migration
3. Apply theme on launch (Dark/Light/System)
4. Initialize KevaCore with config
5. Store/restore window position per monitor

**Acceptance criteria:**

- App launches with valid config
- App shows error popup with invalid config
- Theme applies correctly
- Window position remembered per monitor

---

### M3: Key List Display

**Goal:** Display actual keys in left pane.

**Tasks:**

1. Fetch keys from `keva_core.active_keys()` and `keva_core.trashed_keys()`
2. Render scrollable list using `egui::ScrollArea`
3. Each key as selectable label
4. Track selected key: `selected_key: Option<Key>`
5. Trashed keys shown at bottom with 🗑️ prefix

**Acceptance criteria:**

- All active keys displayed
- Trashed keys displayed at bottom with icon
- Clicking key highlights it

---

### M4: Search Bar Integration

**Goal:** Fuzzy search filters key list.

**Tasks:**

1. Add `search_query: String` to state
2. Create `SearchEngine` instance (from `keva_gui::search`)
3. When query changes: call `search_engine.set_query()`
4. Each frame: call `search_engine.tick()` for non-blocking updates
5. Display results from `search_engine.active_results()` and `trashed_results()`

**Acceptance criteria:**

- Typing filters key list
- Results ranked by relevance
- Empty query shows all keys

---

### M5: Right Pane (Read + Edit)

**Goal:** Display and edit values.

**Tasks:**

1. When key selected, call `keva_core.get(key)`
2. Display based on value type:
    - **Text:** Editable text area
    - **Files:** File list with names/sizes
    - **None:** Placeholder text
3. Auto-save: after 3 seconds of inactivity
4. Handle paste:
    - Text clipboard → insert at cursor
    - Files clipboard → store as files (or block if text exists)

**Acceptance criteria:**

- Selecting key shows its value
- Text editing works with auto-save
- File paste blocked when text exists (with hint)

---

### M6: Left Pane Controls

**Goal:** Rename and delete keys.

**Tasks:**

1. Show buttons on hover/selection:
    - Rename button (✏️)
    - Delete button (🗑️)
2. Rename: inline editor, confirmation if overwriting
3. Delete: respect `delete_style` (soft/immediate)

**Acceptance criteria:**

- Hover shows buttons
- Rename works with overwrite confirmation
- Delete respects configured style

---

### M7: System Tray Icon

**Goal:** Tray icon with menu.

**Dependencies:** `tray-icon` crate

**Tasks:**

1. Create tray icon on launch
2. Left-click: toggle window visibility
3. Right-click menu:
    - Show Keva (disabled if visible)
    - Settings...
    - ---
    - Launch at Login (checkbox)
    - ---
    - Quit Keva
4. Sync checkbox state with config

**Acceptance criteria:**

- Tray icon visible
- Left-click toggles window
- Right-click shows menu
- Quit from menu works

---

### M8: Global Shortcut

**Goal:** Register system-wide hotkey.

**Dependencies:** `global-hotkey` crate

**Tasks:**

1. Register `Cmd+Shift+K` / `Ctrl+Shift+K` on launch
2. Handle shortcut press → show window
3. Handle conflict:
    - Show notification
    - Open settings with shortcut focused
4. Re-register when shortcut changes in settings

**Acceptance criteria:**

- Shortcut shows hidden window
- Conflict detected and handled gracefully

---

### M9: Single Instance

**Goal:** Prevent multiple instances.

**Tasks:**

1. Windows: Named mutex
    - If exists: send message to existing, exit
2. macOS: Unix socket in data directory
    - If bound: send message to existing, exit
3. Existing instance receives message → show window
4. Handle timeout (2s) → offer to force-quit

**Acceptance criteria:**

- Second launch activates first instance
- No duplicate processes

---

### M10: Launch at Login

**Goal:** Register app to start at login.

**Tasks:**

1. macOS: `SMAppService` API
2. Windows: Registry `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run`
3. Check/set based on config
4. Sync with tray menu checkbox
5. Must appear in OS settings (Task Manager / Login Items)

**Acceptance criteria:**

- Toggle works from settings and tray
- Appears correctly in OS startup settings

---

### M11: First-Run Experience

**Goal:** Welcome dialog on first launch.

**Tasks:**

1. Detect first launch (no config.toml)
2. Show dialog:
    - Explanation: "Keva runs in background. Use Cmd+Shift+K / Ctrl+Shift+K to show."
    - Checkbox: "Launch Keva at login" (checked by default)
    - Button: "Get Started"
3. On confirm: create config, register login if checked, show window

**Acceptance criteria:**

- Dialog appears on first launch only
- Preferences applied correctly

---

### M12: Settings Dialog

**Goal:** Full settings UI.

**Settings:**

| Category  | Setting              |
|-----------|----------------------|
| General   | Theme                |
| General   | Launch at Login      |
| General   | Show Tray Icon       |
| Shortcuts | Global Shortcut      |
| Data      | Delete Style         |
| Data      | Large File Threshold |
| Lifecycle | Trash TTL            |
| Lifecycle | Purge TTL            |

**Tasks:**

1. Theme picker (Dark/Light/System)
2. Launch at Login toggle (syncs with OS)
3. Show Tray Icon toggle
4. Global shortcut picker (with conflict detection)
5. Data and lifecycle settings

**Acceptance criteria:**

- All settings editable
- Changes persist and apply immediately

---

### M13: Keyboard Shortcuts

**Goal:** Implement in-app shortcuts.

| Key           | Action               |
|---------------|----------------------|
| `↑` / `↓`     | Navigate key list    |
| `Enter`       | Focus right pane     |
| `Shift+Enter` | Copy + hide          |
| `Cmd+F`       | Focus search bar     |
| `Cmd+,`       | Open settings        |

**Acceptance criteria:**

- All shortcuts work per Spec.md

---

### M14: Drag & Drop

**Goal:** Accept file drops.

**Tasks:**

1. Enable drag-drop in eframe
2. Handle drop on right pane → store to target key
3. Handle drop on left pane key → store to that key
4. Check size against threshold, confirm if large

**Acceptance criteria:**

- Files can be dropped on both panes
- Large files trigger confirmation

---

### M15: Window Lifecycle + GC

**Goal:** Clean shutdown and garbage collection.

**Tasks:**

1. Auto-save pending edits on window hide
2. Run `keva_core.maintenance()` on hide and quit
3. Warn if unsaved changes exist before hiding

**Acceptance criteria:**

- Pending edits saved on hide
- GC runs on hide/quit
- No data loss

---

## File Structure

```
gui/
├── Cargo.toml
└── src/
    ├── main.rs           # Entry point, first-run detection
    ├── app.rs            # KevaApp, main update loop
    ├── config.rs         # GuiConfig, load/save/validate
    ├── settings.rs       # Settings dialog
    ├── tray.rs           # System tray integration
    ├── hotkey.rs         # Global shortcut registration
    ├── instance.rs       # Single instance handling
    ├── startup.rs        # Launch at login
    ├── panels/
    │   ├── mod.rs
    │   ├── search_bar.rs
    │   ├── key_list.rs
    │   └── inspector.rs
    ├── search/           # (existing)
    │   ├── mod.rs
    │   └── tests.rs
    └── theme.rs          # Colors, sizes, theme handling
```

---

## Notes

- Use `std::time::SystemTime::now()` for timestamp parameters
- `get(key)` does not require timestamp
- `rename(old, new, overwrite)` does not require timestamp
- Search is managed via `SearchEngine` from `keva_gui::search`
- Test on both Windows and macOS
