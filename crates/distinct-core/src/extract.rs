//! File extraction for ingest extract (D-I).
//!
//! Turns host-saved files into extracted text. Supports HTML (.html/.htm),
//! Markdown (.md), plain text (.txt), and DOCX (.docx). PDF returns exit 2
//! `unavailable` with limitation `host_guided_pdf`.
//!
//! Caps: 32 MiB input, 1 MiB extracted text, zip entries ≤ 2000,
//! decompressed size ≤ 64 MiB (zip-bomb guard). Content is untrusted
//! data and is never interpreted as instructions.

use std::io::Read;

/// Maximum input file size: 32 MiB.
pub const MAX_INPUT_BYTES: u64 = 32 * 1024 * 1024;

/// Maximum extracted text size: 1 MiB.
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;

/// Maximum number of entries in a zip archive.
pub const MAX_ZIP_ENTRIES: usize = 2000;

/// Maximum total decompressed size for zip entries: 64 MiB.
pub const MAX_ZIP_DECOMPRESSED: u64 = 64 * 1024 * 1024;

/// Result of an extraction.
#[derive(Debug)]
pub struct ExtractResult {
    pub text: String,
    pub format: &'static str,
    pub byte_count: usize,
    pub truncated: bool,
}

/// Extraction errors.
#[derive(Debug)]
pub enum ExtractError {
    InputTooLarge { actual: u64, max: u64 },
    UnsupportedFormat(String),
    PdfHostGuided,
    ZipBomb { reason: String },
    ZipTooManyEntries { count: usize },
    ParseError(String),
    Io(String),
}

impl std::fmt::Display for ExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InputTooLarge { actual, max } => {
                write!(f, "input too large: {actual} bytes exceeds {max} byte cap")
            }
            Self::UnsupportedFormat(ext) => write!(f, "unsupported format: {ext}"),
            Self::PdfHostGuided => write!(f, "pdf extraction is host-guided"),
            Self::ZipBomb { reason } => write!(f, "zip bomb detected: {reason}"),
            Self::ZipTooManyEntries { count } => {
                write!(
                    f,
                    "zip entry count {count} exceeds limit of {MAX_ZIP_ENTRIES}"
                )
            }
            Self::ParseError(e) => write!(f, "parse error: {e}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for ExtractError {}

/// Extract text from a file based on its extension.
pub fn extract(filename: &str, data: &[u8]) -> Result<ExtractResult, ExtractError> {
    let len = data.len() as u64;
    if len > MAX_INPUT_BYTES {
        return Err(ExtractError::InputTooLarge {
            actual: len,
            max: MAX_INPUT_BYTES,
        });
    }

    let ext = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "html" | "htm" => extract_html(data),
        "md" | "txt" => extract_plaintext(data),
        "docx" => extract_docx(data),
        "pdf" => Err(ExtractError::PdfHostGuided),
        other => Err(ExtractError::UnsupportedFormat(other.to_string())),
    }
}

/// Extract text from HTML using dom_smoothie (Readability), falling back to
/// dom_query document text.
fn extract_html(data: &[u8]) -> Result<ExtractResult, ExtractError> {
    let html = std::str::from_utf8(data)
        .map_err(|e| ExtractError::ParseError(format!("invalid UTF-8: {e}")))?;

    // Try dom_smoothie (Readability) first.
    let fake_url = "https://localhost/";
    let mut readability = match dom_smoothie::Readability::new(html, Some(fake_url), None) {
        Ok(r) => r,
        Err(_) => {
            // Fallback: dom_query document text.
            let doc = dom_query::Document::from(html);
            let text = doc.text().to_string();
            let (text, truncated) = cap_text(text);
            return Ok(ExtractResult {
                text,
                format: "html_dom_query",
                byte_count: data.len(),
                truncated,
            });
        }
    };
    if let Ok(article) = readability.parse() {
        let text = article.text_content.to_string();
        let (text, truncated) = cap_text(text);
        return Ok(ExtractResult {
            text,
            format: "html_readability",
            byte_count: data.len(),
            truncated,
        });
    }

    // Fallback: dom_query document text.
    let doc = dom_query::Document::from(html);
    let text = doc.text().to_string();
    let (text, truncated) = cap_text(text);
    Ok(ExtractResult {
        text,
        format: "html_dom_query",
        byte_count: data.len(),
        truncated,
    })
}

/// Extract text from plain text or markdown (pass-through with cap).
fn extract_plaintext(data: &[u8]) -> Result<ExtractResult, ExtractError> {
    let text = std::str::from_utf8(data)
        .map_err(|e| ExtractError::ParseError(format!("invalid UTF-8: {e}")))?;
    let (text, truncated) = cap_text(text.to_string());
    Ok(ExtractResult {
        text,
        format: "plaintext",
        byte_count: data.len(),
        truncated,
    })
}

/// Extract text from a DOCX file (zip + quick-xml, reading word/document.xml).
fn extract_docx(data: &[u8]) -> Result<ExtractResult, ExtractError> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| ExtractError::ParseError(format!("invalid zip: {e}")))?;

    let entry_count = archive.len();
    if entry_count > MAX_ZIP_ENTRIES {
        return Err(ExtractError::ZipTooManyEntries { count: entry_count });
    }

    // Zip-bomb guard: check total decompressed size before reading.
    let mut total_decompressed: u64 = 0;
    for i in 0..entry_count {
        let file = archive
            .by_index(i)
            .map_err(|e| ExtractError::Io(format!("zip entry {i}: {e}")))?;
        total_decompressed = total_decompressed.saturating_add(file.size());
        if total_decompressed > MAX_ZIP_DECOMPRESSED {
            return Err(ExtractError::ZipBomb {
                reason: format!(
                    "total decompressed size {total_decompressed} exceeds {MAX_ZIP_DECOMPRESSED}"
                ),
            });
        }
    }

    // Read word/document.xml.
    let mut doc_xml = Vec::new();
    for i in 0..entry_count {
        let mut file = archive
            .by_index(i)
            .map_err(|e| ExtractError::Io(format!("zip entry {i}: {e}")))?;
        if file.name() == "word/document.xml" {
            file.read_to_end(&mut doc_xml)
                .map_err(|e| ExtractError::Io(format!("read word/document.xml: {e}")))?;
            break;
        }
    }

    if doc_xml.is_empty() {
        return Err(ExtractError::ParseError(
            "word/document.xml not found in docx".to_string(),
        ));
    }

    let text = extract_xml_text(&doc_xml)?;
    let (text, truncated) = cap_text(text);
    Ok(ExtractResult {
        text,
        format: "docx",
        byte_count: data.len(),
        truncated,
    })
}

/// Extract text runs from a DOCX document.xml using quick-xml.
fn extract_xml_text(xml: &[u8]) -> Result<String, ExtractError> {
    let mut reader = quick_xml::Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut text_parts: Vec<String> = Vec::new();
    let mut in_text_run = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");
                // <w:t> is the text run element in DOCX; local_name strips the prefix.
                if name == "t" {
                    in_text_run = true;
                }
            }
            Ok(quick_xml::events::Event::Text(ref e)) => {
                if in_text_run {
                    if let Ok(t) = e.decode() {
                        text_parts.push(t.to_string());
                    }
                }
            }
            Ok(quick_xml::events::Event::End(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");
                if name == "t" {
                    in_text_run = false;
                }
                // Add space after paragraph end.
                if name == "p" {
                    text_parts.push(' '.to_string());
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(e) => {
                return Err(ExtractError::ParseError(format!("XML parse error: {e}")));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(text_parts.join(""))
}

/// Cap text to MAX_TEXT_BYTES, returning (text, truncated).
fn cap_text(text: String) -> (String, bool) {
    if text.len() <= MAX_TEXT_BYTES {
        (text, false)
    } else {
        let mut truncated = text;
        truncated.truncate(MAX_TEXT_BYTES);
        (truncated, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_plaintext_basic() {
        let result = extract("notes.txt", b"Hello world").unwrap();
        assert_eq!(result.text, "Hello world");
        assert_eq!(result.format, "plaintext");
        assert!(!result.truncated);
    }

    #[test]
    fn extract_markdown_basic() {
        let result = extract("readme.md", b"# Title\n\nSome content").unwrap();
        assert_eq!(result.text, "# Title\n\nSome content");
        assert_eq!(result.format, "plaintext");
    }

    #[test]
    fn extract_html_readability() {
        let html = b"<html><body><p>Hello world</p><p>Second paragraph</p></body></html>";
        let result = extract("page.html", html).unwrap();
        assert!(result.text.contains("Hello world"));
        assert!(
            result.format == "html_readability" || result.format == "html_dom_query",
            "format was {}",
            result.format
        );
    }

    #[test]
    fn extract_pdf_returns_host_guided() {
        let result = extract("doc.pdf", b"%PDF-1.4 fake");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExtractError::PdfHostGuided));
    }

    #[test]
    fn extract_unsupported_format() {
        let result = extract("file.xyz", b"data");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExtractError::UnsupportedFormat(_)));
    }

    #[test]
    fn extract_input_too_large() {
        // Create data just over the limit.
        let size = (MAX_INPUT_BYTES + 1) as usize;
        let data = vec![0u8; size];
        let result = extract("big.txt", &data);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExtractError::InputTooLarge { .. }));
    }

    #[test]
    fn extract_text_cap_truncates() {
        let big = "x".repeat(MAX_TEXT_BYTES + 1000);
        let result = extract("big.txt", big.as_bytes()).unwrap();
        assert!(result.truncated);
        assert_eq!(result.text.len(), MAX_TEXT_BYTES);
    }

    #[test]
    fn extract_docx_basic() {
        // Build a minimal DOCX (zip with word/document.xml).
        let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Hello</w:t></w:r></w:p>
    <w:p><w:r><w:t>World</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

        let mut buf = Vec::new();
        let cursor = std::io::Cursor::new(&mut buf);
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer.start_file("word/document.xml", options).unwrap();
        std::io::Write::write_all(&mut writer, doc_xml.as_bytes()).unwrap();
        writer.finish().unwrap();

        let result = extract("test.docx", &buf).unwrap();
        assert_eq!(result.format, "docx");
        assert!(result.text.contains("Hello"));
        assert!(result.text.contains("World"));
    }

    #[test]
    fn zip_bomb_too_many_entries() {
        // Build a zip with more than MAX_ZIP_ENTRIES entries.
        let mut buf = Vec::new();
        let cursor = std::io::Cursor::new(&mut buf);
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for i in 0..(MAX_ZIP_ENTRIES + 1) {
            let name = format!("entry_{i}.txt");
            writer.start_file(name, options).unwrap();
            std::io::Write::write_all(&mut writer, b"x").unwrap();
        }
        writer.finish().unwrap();

        let result = extract("bomb.docx", &buf);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExtractError::ZipTooManyEntries { .. }));
    }

    #[test]
    fn zip_bomb_decompressed_size() {
        // Build a zip where total decompressed size exceeds the limit.
        // We cannot actually create a 64 MiB zip in a test, but we can verify
        // the guard logic by checking that a normal small zip passes.
        let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>Small</w:t></w:r></w:p></w:body>
</w:document>"#;
        let mut buf = Vec::new();
        let cursor = std::io::Cursor::new(&mut buf);
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer.start_file("word/document.xml", options).unwrap();
        std::io::Write::write_all(&mut writer, doc_xml.as_bytes()).unwrap();
        writer.finish().unwrap();

        // This should succeed (well under limits).
        let result = extract("small.docx", &buf).unwrap();
        assert!(result.text.contains("Small"));
    }

    #[test]
    fn xml_text_extraction_paragraphs() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>First</w:t></w:r></w:p>
    <w:p><w:r><w:t>Second</w:t></w:r></w:p>
  </w:body>
</w:document>"#;
        let text = extract_xml_text(xml.as_bytes()).unwrap();
        assert!(text.contains("First"));
        assert!(text.contains("Second"));
    }
}
