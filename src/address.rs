//! URL parsing and resolution via `servo/rust-url` (`url` crate).

use url::Url;

use crate::error::{NexusError, NexusResult};

/// Parses user input as an HTTP(S) URL.
///
/// `example.com` becomes `https://example.com/`.
pub fn normalize_url(input: &str) -> NexusResult<Url> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(NexusError::EmptyUrl);
    }

    let candidate = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };

    let parsed = Url::parse(&candidate)?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        other => Err(NexusError::UnsupportedScheme(other.to_owned())),
    }
}

/// Resolves a relative browser URL against a base URL.
pub fn resolve_url(base: &Url, href: &str) -> Option<Url> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }

    match Url::parse(href) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => Some(url),
        Ok(_) => None,
        Err(url::ParseError::RelativeUrlWithoutBase) => base.join(href).ok(),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_https_to_plain_host() {
        let url = normalize_url("example.com/path").unwrap();
        assert_eq!(url.as_str(), "https://example.com/path");
    }

    #[test]
    fn resolves_relative_url() {
        let base = Url::parse("https://example.com/a/b/").unwrap();
        let url = resolve_url(&base, "../c").unwrap();
        assert_eq!(url.as_str(), "https://example.com/a/c");
    }
}
