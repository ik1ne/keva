//! Clipboard handling utilities for Keva
//!
//! This module provides utilities for detecting clipboard content types
//! and preparing data for storage. The actual clipboard access should be
//! done by the CLI/GUI using a clipboard library like clipboard-rs.

use crate::model::RichFormat;

/// Detected clipboard content
#[derive(Debug, Clone)]
pub struct ClipboardContent {
    /// Plain text content (if available and meaningful)
    pub plain_text: Option<String>,
    /// Rich format data (if available)
    pub rich: Option<RichContent>,
}

/// Rich content from clipboard
#[derive(Debug, Clone)]
pub struct RichContent {
    /// The format of the rich content
    pub format: RichFormat,
    /// The raw data
    pub data: Vec<u8>,
}

impl ClipboardContent {
    /// Create an empty clipboard content
    pub fn empty() -> Self {
        Self {
            plain_text: None,
            rich: None,
        }
    }

    /// Create clipboard content with only plain text
    pub fn text(text: String) -> Self {
        let plain_text = if is_meaningful_text(&text) {
            Some(text)
        } else {
            None
        };
        Self {
            plain_text,
            rich: None,
        }
    }

    /// Create clipboard content with rich data and optional plain text
    pub fn rich(format: RichFormat, data: Vec<u8>, plain_text: Option<String>) -> Self {
        let plain_text = plain_text.filter(|t| is_meaningful_text(t));
        Self {
            plain_text,
            rich: Some(RichContent { format, data }),
        }
    }

    /// Check if this clipboard content has any data
    pub fn is_empty(&self) -> bool {
        self.plain_text.is_none() && self.rich.is_none()
    }

    /// Check if this clipboard content has rich data
    pub fn has_rich(&self) -> bool {
        self.rich.is_some()
    }

    /// Check if this clipboard content has plain text
    pub fn has_plain_text(&self) -> bool {
        self.plain_text.is_some()
    }

    /// Check if this clipboard content has only plain text (no rich data)
    pub fn is_text_only(&self) -> bool {
        self.plain_text.is_some() && self.rich.is_none()
    }
}

/// Check if text is meaningful (non-empty, non-whitespace)
pub fn is_meaningful_text(text: &str) -> bool {
    !text.trim().is_empty()
}

/// Detect the format of binary data based on magic bytes
pub fn detect_format(data: &[u8]) -> Option<RichFormat> {
    if data.len() < 4 {
        return None;
    }

    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some(RichFormat::Png);
    }

    // JPEG: FF D8 FF
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(RichFormat::Jpeg);
    }

    // PDF: %PDF
    if data.starts_with(b"%PDF") {
        return Some(RichFormat::Pdf);
    }

    // RTF: {\rtf
    if data.starts_with(b"{\\rtf") {
        return Some(RichFormat::Rtf);
    }

    // HTML (heuristic - starts with < and contains html or doctype)
    if data.starts_with(b"<") || data.starts_with(b"<!") {
        let text = String::from_utf8_lossy(&data[..data.len().min(1000)]).to_lowercase();
        if text.contains("html") || text.contains("doctype") {
            return Some(RichFormat::Html);
        }
    }

    None
}

/// Detect MIME type from file extension
pub fn mime_from_extension(ext: &str) -> Option<RichFormat> {
    match ext.to_lowercase().as_str() {
        "png" => Some(RichFormat::Png),
        "jpg" | "jpeg" => Some(RichFormat::Jpeg),
        "pdf" => Some(RichFormat::Pdf),
        "rtf" => Some(RichFormat::Rtf),
        "html" | "htm" => Some(RichFormat::Html),
        "gif" => Some(RichFormat::Binary {
            mime_type: "image/gif".to_string(),
        }),
        "webp" => Some(RichFormat::Binary {
            mime_type: "image/webp".to_string(),
        }),
        "svg" => Some(RichFormat::Binary {
            mime_type: "image/svg+xml".to_string(),
        }),
        "mp3" => Some(RichFormat::Binary {
            mime_type: "audio/mpeg".to_string(),
        }),
        "wav" => Some(RichFormat::Binary {
            mime_type: "audio/wav".to_string(),
        }),
        "mp4" => Some(RichFormat::Binary {
            mime_type: "video/mp4".to_string(),
        }),
        "json" => Some(RichFormat::Binary {
            mime_type: "application/json".to_string(),
        }),
        "xml" => Some(RichFormat::Binary {
            mime_type: "application/xml".to_string(),
        }),
        "zip" => Some(RichFormat::Binary {
            mime_type: "application/zip".to_string(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_meaningful_text() {
        assert!(is_meaningful_text("hello"));
        assert!(is_meaningful_text("  hello  "));
        assert!(!is_meaningful_text(""));
        assert!(!is_meaningful_text("   "));
        assert!(!is_meaningful_text("\n\t"));
    }

    #[test]
    fn test_detect_png() {
        let png_header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert!(matches!(detect_format(&png_header), Some(RichFormat::Png)));
    }

    #[test]
    fn test_detect_jpeg() {
        let jpeg_header = [0xFF, 0xD8, 0xFF, 0xE0];
        assert!(matches!(detect_format(&jpeg_header), Some(RichFormat::Jpeg)));
    }

    #[test]
    fn test_detect_pdf() {
        let pdf_header = b"%PDF-1.4";
        assert!(matches!(detect_format(pdf_header), Some(RichFormat::Pdf)));
    }

    #[test]
    fn test_clipboard_content_text_only() {
        let content = ClipboardContent::text("hello".to_string());
        assert!(content.has_plain_text());
        assert!(!content.has_rich());
        assert!(content.is_text_only());
    }

    #[test]
    fn test_clipboard_content_rich() {
        let content = ClipboardContent::rich(
            RichFormat::Png,
            vec![0x89, 0x50, 0x4E, 0x47],
            Some("alt text".to_string()),
        );
        assert!(content.has_plain_text());
        assert!(content.has_rich());
        assert!(!content.is_text_only());
    }

    #[test]
    fn test_mime_from_extension() {
        assert!(matches!(mime_from_extension("png"), Some(RichFormat::Png)));
        assert!(matches!(mime_from_extension("PNG"), Some(RichFormat::Png)));
        assert!(matches!(mime_from_extension("jpg"), Some(RichFormat::Jpeg)));
        assert!(matches!(mime_from_extension("jpeg"), Some(RichFormat::Jpeg)));
        assert!(matches!(mime_from_extension("pdf"), Some(RichFormat::Pdf)));
    }
}
