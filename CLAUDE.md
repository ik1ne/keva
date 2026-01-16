# Claude Code Guidelines for Keva

## Related Documentation

- `Spec.md` - Product specification and design decisions
- `windows_milestone.md` - Windows implementation milestones and test cases
- `keva_core.md` - Core storage module specification
- `keva_search.md` - Search engine module specification

## Cargo Commands

Working directory is the keva project root. Do not use `cd`, `--manifest-path`, or `-C` flags:

```sh
# Good
cargo build -q
cargo build -q -p keva_core

# Bad - unnecessary cd
cd /c/Users/ik1ne/Sources/Rust/keva && cargo build -q

# Bad - unnecessary path specification
cargo build -q --manifest-path "C:/Users/.../Cargo.toml"
```

Always use `-q` flag to minimize output:

```sh
cargo build -q
cargo test -q
cargo clippy -q
```

Run clippy on all code (library + tests):

```sh
cargo clippy -q --lib --tests
```

## Code Style

### Imports

Always group imports at the top of the file. Never use local imports inside functions or blocks:

```rust
// Good - imports at module top
use crate::core::KevaCore;

impl Foo {
    fn bar() {
        let ver = KevaCore::THUMB_VER;
    }
}

// Bad - local import inside function
impl Foo {
    fn bar() {
        use crate::core::KevaCore;
        let ver = KevaCore::THUMB_VER;
    }
}
```

For test submodules, use `super::*` to inherit from the parent module:

```rust
// In src/core/db/tests.rs (submodule of db/mod.rs)
use super::*;  // Gets Database, error module, etc. from parent
use crate::types::config::SavedConfig;  // Types not in parent

mod create {
    use super::*;  // Gets items from tests.rs level

    #[test]
    fn test_create() { ... }
}
```

### Win32 Message Parameters

Extract raw `wparam`/`lparam` values into named variables that explain their meaning:

```rust
// Good - reader understands the values without looking up Windows docs
let cursor_x = (lparam.0 & 0xFFFF) as i16 as i32;
let cursor_y = ((lparam.0 > > 16) & 0xFFFF) as i16 as i32;
let previous_window = lparam.0;
let virtual_key = wparam.0 as u16;

// Bad - requires Windows API knowledge to understand
let x = (lparam.0 & 0xFFFF) as i16 as i32;
if lparam.0 != 0 { ... }
if wparam.0 as u16 == VK_ESCAPE.0 { ... }
```

### Module Organization

- Split large impl blocks into multiple impl blocks with doc comments for logical grouping:
  ```rust
  /// Create operations.
  impl Foo { ... }

  /// Read operations.
  impl Foo { ... }
  ```
- Order methods logically (e.g., CRUD order for data operations).
- No section comments (e.g., `// ====`) - use module structure or impl block doc comments instead.

### Error Types

Prefer distinct error variants over reusing generic ones. Each error should represent a specific failure condition that
callers may want to handle differently.

### Doc Comments

Only add doc comments when they provide information beyond what the code already expresses:

```rust
// Bad - restates the obvious
/// Gets the name.
fn get_name() -> String;

/// A user.
struct User;

// Good - explains non-obvious behavior or error conditions
/// Returns `Err(NotFound)` if the key doesn't exist.
/// Returns `Err(Trashed)` if the key is trashed.
pub fn update(...) -> Result<(), Error>;

/// Stale entries can still be modified until GC runs.
pub fn touch(...) -> Result<(), Error>;
```

Document error conditions explicitly when a function can fail in multiple ways.

### Platform-Specific Comments

Keep comments that explain platform-specific behavior (Win32, WebView2, etc.) for readers unfamiliar with those APIs:

```rust
// Good - explains Win32-specific behavior
// WM_NCCALCSIZE with wparam=TRUE: system is calculating client area during resize
if wparam.0 != 0 { ... }

// Good - explains why this pattern is needed
// PostWebMessageAsJson is thread-safe, can be called from any thread
wv.webview.PostWebMessageAsJson(msg);

// Bad - obvious from context
// Create the window
CreateWindowExW(...);
```

Omit comments that restate what the code does, but keep comments that explain *why* or provide
platform knowledge that isn't obvious from the code.

## Unit Testing

### Test Organization

- One module per operation or logical group.
- Common helpers in `mod common` at the top.
- Order test modules to match impl organization.

### Assertions

Prefer full struct comparisons over individual field assertions:

```rust
// Good - shows expected state clearly, catches unexpected field changes
assert_eq!(
    value.metadata,
    Metadata {
        created_at: now,
        updated_at: now,
        ...
    }
);

// Avoid - verbose, may miss unexpected field values
assert_eq!(value.metadata.created_at, now);
assert_eq!(value.metadata.updated_at, now);
```

Use slice comparisons:

```rust
// Good
assert_eq!(items, &[expected]);
assert_eq!(result.keys, std::slice::from_ref(&key));

// Avoid
assert_eq!(items.len(), 1);
assert_eq!(items[0], expected);
```

Use informative panic messages:

```rust
match & value {
Variant::Expected(v) => { ... }
other => panic!("Expected Expected variant, got: {other:?}"),
}
```

### Whitebox Testing

Don't test redundant combinations if independent behaviors are already covered. If behavior A and behavior B are tested
independently, their combination doesn't need a separate test unless there's interaction.

### Test Documentation

Add doc comments to tests that document intentional design decisions:

```rust
/// Stale keys can still be rescued before GC runs.
///
/// This is intentional: GC is the point of no return, not TTL expiration.
#[test]
fn test_stale_key_can_be_rescued() { ... }
```
