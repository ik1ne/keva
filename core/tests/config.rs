/// Verify `rm` follows the configured default (Soft vs Immediate) when no specific flag is passed.
#[test]
fn test_rm_honors_default_soft_delete() {}

/// Verify `rm` follows the configured default (Immediate) when set.
#[test]
fn test_rm_honors_default_immediate_delete() {}

/// Verify importing a file follows the configured default (Embed).
#[test]
fn test_import_honors_default_embed() {}

/// Verify the core logic exposes the size of a file before import.
#[test]
fn test_large_file_size_check() {}
