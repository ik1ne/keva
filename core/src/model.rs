//! Data models for Keva

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A key in the Keva store, using path-based hierarchy (e.g., `project/config/theme`)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Key(String);

impl Key {
    /// Create a new key from a string path
    pub fn new(path: impl Into<String>) -> crate::Result<Self> {
        let path = path.into();
        Self::validate(&path)?;
        Ok(Self(path))
    }

    /// Validate a key path
    fn validate(path: &str) -> crate::Result<()> {
        if path.is_empty() {
            return Err(crate::Error::InvalidKey("Key cannot be empty".to_string()));
        }
        if path.starts_with('/') || path.ends_with('/') {
            return Err(crate::Error::InvalidKey(
                "Key cannot start or end with '/'".to_string(),
            ));
        }
        if path.contains("//") {
            return Err(crate::Error::InvalidKey(
                "Key cannot contain consecutive slashes".to_string(),
            ));
        }
        Ok(())
    }

    /// Get the key as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get the parent key, if any
    pub fn parent(&self) -> Option<Key> {
        self.0.rfind('/').map(|i| Key(self.0[..i].to_string()))
    }

    /// Check if this key is a direct child of another key
    pub fn is_child_of(&self, parent: &Key) -> bool {
        if let Some(suffix) = self.0.strip_prefix(&parent.0) {
            if let Some(rest) = suffix.strip_prefix('/') {
                return !rest.contains('/');
            }
        }
        false
    }

    /// Check if this key is a descendant of another key
    pub fn is_descendant_of(&self, ancestor: &Key) -> bool {
        self.0.starts_with(&ancestor.0) && self.0.get(ancestor.0.len()..ancestor.0.len()+1) == Some("/")
    }

    /// Get the last segment of the key (the "name")
    pub fn name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    /// Get all segments of the key
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for Key {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Rich format types supported by Keva
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RichFormat {
    /// HTML content
    Html,
    /// RTF content
    Rtf,
    /// PNG image
    Png,
    /// JPEG image
    Jpeg,
    /// PDF document
    Pdf,
    /// Generic binary with MIME type
    Binary { mime_type: String },
}

impl RichFormat {
    /// Get the MIME type for this format
    pub fn mime_type(&self) -> &str {
        match self {
            RichFormat::Html => "text/html",
            RichFormat::Rtf => "application/rtf",
            RichFormat::Png => "image/png",
            RichFormat::Jpeg => "image/jpeg",
            RichFormat::Pdf => "application/pdf",
            RichFormat::Binary { mime_type } => mime_type,
        }
    }

    /// Create a RichFormat from a MIME type
    pub fn from_mime_type(mime: &str) -> Self {
        match mime {
            "text/html" => RichFormat::Html,
            "application/rtf" => RichFormat::Rtf,
            "image/png" => RichFormat::Png,
            "image/jpeg" | "image/jpg" => RichFormat::Jpeg,
            "application/pdf" => RichFormat::Pdf,
            other => RichFormat::Binary {
                mime_type: other.to_string(),
            },
        }
    }
}

/// Value stored in Keva
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value {
    /// Plain text content (optional, stored alongside rich format if meaningful)
    pub plain_text: Option<String>,
    /// Rich format data
    pub rich: Option<RichData>,
}

impl Value {
    /// Create a new plain text value
    pub fn plain_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let plain_text = if text.trim().is_empty() {
            None
        } else {
            Some(text)
        };
        Self {
            plain_text,
            rich: None,
        }
    }

    /// Create a new value with rich content
    pub fn rich(_format: RichFormat, data: RichData, plain_text: Option<String>) -> Self {
        let plain_text = plain_text.filter(|t| !t.trim().is_empty());
        Self {
            plain_text,
            rich: Some(data),
        }
    }

    /// Check if this value has any content
    pub fn is_empty(&self) -> bool {
        self.plain_text.is_none() && self.rich.is_none()
    }

    /// Check if this value has plain text
    pub fn has_plain_text(&self) -> bool {
        self.plain_text.is_some()
    }

    /// Check if this value has rich content
    pub fn has_rich(&self) -> bool {
        self.rich.is_some()
    }
}

/// Rich data storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RichData {
    /// The format of the rich data
    pub format: RichFormat,
    /// Storage location of the rich data
    pub storage: RichStorage,
}

/// How rich data is stored
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RichStorage {
    /// Data stored inline in the database (for small blobs)
    Inline(Vec<u8>),
    /// Data stored as a blob file (content-addressable by hash)
    Blob { hash: String, size: u64 },
    /// Data stored as a link to an external file
    Link { path: PathBuf },
}

/// Lifecycle state of an entry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lifecycle {
    /// Normal visibility
    Active,
    /// Soft-deleted, hidden from standard view
    Trash,
    /// Permanently deleted, pending garbage collection
    Purged,
}

/// Timestamps for lifecycle management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleTimestamps {
    /// When the entry was created
    pub created_at: DateTime<Utc>,
    /// When the entry was last modified
    pub modified_at: DateTime<Utc>,
    /// When the entry should transition to Trash (None = never)
    pub trash_at: Option<DateTime<Utc>>,
    /// When the entry should transition to Purged (None = never)
    pub purge_at: Option<DateTime<Utc>>,
}

impl LifecycleTimestamps {
    /// Create new timestamps for a freshly created entry
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            created_at: now,
            modified_at: now,
            trash_at: None,
            purge_at: None,
        }
    }

    /// Mark the entry as modified
    pub fn touch(&mut self) {
        self.modified_at = Utc::now();
    }

    /// Determine the current lifecycle state based on timestamps
    pub fn current_state(&self) -> Lifecycle {
        let now = Utc::now();
        if let Some(purge_at) = self.purge_at {
            if now >= purge_at {
                return Lifecycle::Purged;
            }
        }
        if let Some(trash_at) = self.trash_at {
            if now >= trash_at {
                return Lifecycle::Trash;
            }
        }
        Lifecycle::Active
    }
}

impl Default for LifecycleTimestamps {
    fn default() -> Self {
        Self::new()
    }
}

/// A complete entry in the Keva store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// The key for this entry
    pub key: Key,
    /// The value stored
    pub value: Value,
    /// Lifecycle timestamps
    pub timestamps: LifecycleTimestamps,
}

impl Entry {
    /// Create a new entry
    pub fn new(key: Key, value: Value) -> Self {
        Self {
            key,
            value,
            timestamps: LifecycleTimestamps::new(),
        }
    }

    /// Get the current lifecycle state
    pub fn lifecycle(&self) -> Lifecycle {
        self.timestamps.current_state()
    }

    /// Check if this entry is visible (not in Trash or Purged state)
    pub fn is_visible(&self) -> bool {
        self.lifecycle() == Lifecycle::Active
    }

    /// Check if this entry is in Trash state
    pub fn is_trash(&self) -> bool {
        self.lifecycle() == Lifecycle::Trash
    }

    /// Check if this entry is Purged
    pub fn is_purged(&self) -> bool {
        self.lifecycle() == Lifecycle::Purged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_validation() {
        assert!(Key::new("project").is_ok());
        assert!(Key::new("project/config").is_ok());
        assert!(Key::new("project/config/theme").is_ok());
        assert!(Key::new("a-b_c.d").is_ok());

        assert!(Key::new("").is_err());
        assert!(Key::new("/project").is_err());
        assert!(Key::new("project/").is_err());
        assert!(Key::new("project//config").is_err());
    }

    #[test]
    fn test_key_parent() {
        let key = Key::new("project/config/theme").unwrap();
        let parent = key.parent().unwrap();
        assert_eq!(parent.as_str(), "project/config");

        let grandparent = parent.parent().unwrap();
        assert_eq!(grandparent.as_str(), "project");

        assert!(grandparent.parent().is_none());
    }

    #[test]
    fn test_key_is_child_of() {
        let parent = Key::new("project").unwrap();
        let child = Key::new("project/config").unwrap();
        let grandchild = Key::new("project/config/theme").unwrap();

        assert!(child.is_child_of(&parent));
        assert!(!grandchild.is_child_of(&parent));
        assert!(grandchild.is_child_of(&child));
    }

    #[test]
    fn test_key_is_descendant_of() {
        let ancestor = Key::new("project").unwrap();
        let child = Key::new("project/config").unwrap();
        let grandchild = Key::new("project/config/theme").unwrap();

        assert!(child.is_descendant_of(&ancestor));
        assert!(grandchild.is_descendant_of(&ancestor));
        assert!(grandchild.is_descendant_of(&child));
        assert!(!ancestor.is_descendant_of(&child));
    }

    #[test]
    fn test_value_plain_text() {
        let value = Value::plain_text("hello");
        assert!(value.has_plain_text());
        assert!(!value.has_rich());
        assert!(!value.is_empty());

        let empty = Value::plain_text("   ");
        assert!(!empty.has_plain_text());
        assert!(empty.is_empty());
    }
}
