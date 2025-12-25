# Claude Code Guidelines for Keva

## Related Documentation

- `Spec.md` - Product specification and design decisions
- `Planned.md` - Planned features and roadmap
- `implementation_detail.md` - Implementation details
- `todo.md` - Current tasks

## Cargo Commands

Working directory is the keva project root. Do not use `--manifest-path` or `-C` flags:
```sh
# Good
cargo build -q
cargo build -q -p keva_core

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

Prefer distinct error variants over reusing generic ones. Each error should represent a specific failure condition that callers may want to handle differently.

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
match &value {
    Variant::Expected(v) => { ... }
    other => panic!("Expected Expected variant, got: {other:?}"),
}
```

### Whitebox Testing

Don't test redundant combinations if independent behaviors are already covered. If behavior A and behavior B are tested independently, their combination doesn't need a separate test unless there's interaction.

### Test Documentation

Add doc comments to tests that document intentional design decisions:
```rust
/// Stale keys can still be rescued before GC runs.
///
/// This is intentional: GC is the point of no return, not TTL expiration.
#[test]
fn test_stale_key_can_be_rescued() { ... }
```
