//! Web text decoding using `encoding_rs`.
//!
//! Nexus 0.2 implements a pragmatic subset of HTML encoding sniffing:
//! BOM -> HTTP Content-Type charset -> early <meta charset> -> windows-1252.

use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingSource {
    Bom,
    HttpHeader,
    MetaTag,
    Fallback,
}

#[derive(Debug)]
pub struct DecodedDocument {
    pub text: String,
    pub encoding: &'static str,
    pub source: EncodingSource,
    pub had_errors: bool,
}

pub fn decode_html(bytes: &[u8], content_type: Option<&str>) -> DecodedDocument {
    let (encoding, source) = if let Some((encoding, _bom_len)) = Encoding::for_bom(bytes) {
        (encoding, EncodingSource::Bom)
    } else if let Some(label) = charset_from_content_type(content_type) {
        (
            Encoding::for_label(label.as_bytes()).unwrap_or(WINDOWS_1252),
            EncodingSource::HttpHeader,
        )
    } else if let Some(label) = sniff_meta_charset(bytes) {
        (
            Encoding::for_label(label.as_bytes()).unwrap_or(WINDOWS_1252),
            EncodingSource::MetaTag,
        )
    } else {
        (WINDOWS_1252, EncodingSource::Fallback)
    };

    let (decoded, actual_encoding, had_errors) = encoding.decode(bytes);

    DecodedDocument {
        text: decoded.into_owned(),
        encoding: actual_encoding.name(),
        source,
        had_errors,
    }
}


/// Decode a classic JavaScript resource. Nexus 0.6 follows the modern Web
/// default more closely here than `decode_html`: BOM -> HTTP charset -> UTF-8.
/// Script resources do not use HTML `<meta charset>` sniffing.
pub fn decode_script(bytes: &[u8], content_type: Option<&str>) -> DecodedDocument {
    let (encoding, source) = if let Some((encoding, _bom_len)) = Encoding::for_bom(bytes) {
        (encoding, EncodingSource::Bom)
    } else if let Some(label) = charset_from_content_type(content_type) {
        (
            Encoding::for_label(label.as_bytes()).unwrap_or(UTF_8),
            EncodingSource::HttpHeader,
        )
    } else {
        (UTF_8, EncodingSource::Fallback)
    };

    let (decoded, actual_encoding, had_errors) = encoding.decode(bytes);
    DecodedDocument {
        text: decoded.into_owned(),
        encoding: actual_encoding.name(),
        source,
        had_errors,
    }
}

fn charset_from_content_type(content_type: Option<&str>) -> Option<String> {
    let value = content_type?;
    value.split(';').skip(1).find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        if !name.trim().eq_ignore_ascii_case("charset") {
            return None;
        }
        let value = value.trim().trim_matches(['\'', '"']).trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

/// Lightweight early-meta sniffer for Nexus 0.2.
///
/// It intentionally examines only the first 1024 bytes. The complete WHATWG
/// prescan algorithm will replace this in a later engine revision.
fn sniff_meta_charset(bytes: &[u8]) -> Option<String> {
    let prefix = &bytes[..bytes.len().min(1024)];
    let ascii = String::from_utf8_lossy(prefix).to_ascii_lowercase();

    // <meta charset="utf-8">
    if let Some(index) = ascii.find("charset") {
        let tail = &ascii[index + "charset".len()..];
        let tail = tail.trim_start();
        if let Some(tail) = tail.strip_prefix('=') {
            let tail = tail.trim_start();
            let quote = tail.chars().next().filter(|c| *c == '"' || *c == '\'');
            let raw = if let Some(q) = quote {
                let rest = &tail[q.len_utf8()..];
                rest.split(q).next().unwrap_or("")
            } else {
                tail.split(|c: char| c.is_ascii_whitespace() || c == '>' || c == ';')
                    .next()
                    .unwrap_or("")
            };
            let label = raw.trim();
            if !label.is_empty() {
                return Some(label.to_owned());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charset_header_wins_without_bom() {
        let page = decode_html(b"hello", Some("text/html; charset=utf-8"));
        assert_eq!(page.encoding, "UTF-8");
        assert_eq!(page.source, EncodingSource::HttpHeader);
    }

    #[test]
    fn sniffs_meta_charset() {
        let page = decode_html(
            br#"<html><head><meta charset="utf-8"></head><body>x</body></html>"#,
            None,
        );
        assert_eq!(page.encoding, "UTF-8");
        assert_eq!(page.source, EncodingSource::MetaTag);
    }

    #[test]
    fn javascript_defaults_to_utf8() {
        let page = decode_script("console.log('✓')".as_bytes(), Some("text/javascript"));
        assert_eq!(page.encoding, "UTF-8");
        assert_eq!(page.source, EncodingSource::Fallback);
        assert!(!page.had_errors);
    }
}
