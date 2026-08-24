//! Same-Origin Policy, CORS and preflight checks for Nexus Engine 1.02.

use std::collections::HashMap;

use url::Url;

use crate::origin::Origin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchMode {
    SameOrigin,
    Cors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialsMode {
    Omit,
    SameOrigin,
    Include,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityError {
    CrossOriginBlocked,
    CorsMissingAllowOrigin,
    CorsOriginMismatch,
    CredentialsWithWildcard,
    CredentialsNotAllowed,
    PreflightMethodNotAllowed,
    PreflightHeaderNotAllowed(String),
}

#[must_use]
pub fn should_send_credentials(
    source: &Origin,
    target: &Url,
    credentials: CredentialsMode,
) -> bool {
    match credentials {
        CredentialsMode::Omit => false,
        CredentialsMode::Include => true,
        CredentialsMode::SameOrigin => source.is_same_origin(&Origin::from_url(target)),
    }
}

pub fn enforce_fetch_policy(
    source: &Origin,
    target: &Url,
    mode: FetchMode,
    credentials: CredentialsMode,
    response_headers: &HashMap<String, String>,
) -> Result<(), SecurityError> {
    let target_origin = Origin::from_url(target);
    if source.is_same_origin(&target_origin) {
        return Ok(());
    }
    if mode == FetchMode::SameOrigin {
        return Err(SecurityError::CrossOriginBlocked);
    }

    let allow_origin = header(response_headers, "access-control-allow-origin")
        .ok_or(SecurityError::CorsMissingAllowOrigin)?;
    let credentials_sent = should_send_credentials(source, target, credentials);
    if allow_origin.trim() == "*" {
        if credentials_sent {
            return Err(SecurityError::CredentialsWithWildcard);
        }
        return Ok(());
    }
    if allow_origin.trim() != source.serialize() {
        return Err(SecurityError::CorsOriginMismatch);
    }
    if credentials_sent {
        let allowed = header(response_headers, "access-control-allow-credentials")
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"));
        if !allowed {
            return Err(SecurityError::CredentialsNotAllowed);
        }
    }
    Ok(())
}

#[must_use]
pub fn cors_origin_header(source: &Origin, target: &Url) -> Option<String> {
    (!source.is_same_origin(&Origin::from_url(target))).then(|| source.serialize())
}

#[must_use]
pub fn requires_preflight(method: &str, content_type: Option<&str>) -> bool {
    let method = method.trim().to_ascii_uppercase();
    if !matches!(method.as_str(), "GET" | "HEAD" | "POST") {
        return true;
    }
    if method == "POST" {
        if let Some(content_type) = content_type {
            return !is_cors_safelisted_content_type(content_type);
        }
    }
    false
}

#[must_use]
pub fn requested_non_safelisted_headers(content_type: Option<&str>) -> Vec<String> {
    content_type
        .filter(|value| !is_cors_safelisted_content_type(value))
        .map(|_| vec!["content-type".to_owned()])
        .unwrap_or_default()
}

pub fn enforce_preflight_response(
    source: &Origin,
    target: &Url,
    credentials: CredentialsMode,
    method: &str,
    requested_headers: &[String],
    response_headers: &HashMap<String, String>,
) -> Result<(), SecurityError> {
    enforce_fetch_policy(source, target, FetchMode::Cors, credentials, response_headers)?;

    let methods = header(response_headers, "access-control-allow-methods")
        .unwrap_or("")
        .split(',')
        .map(|value| value.trim().to_ascii_uppercase())
        .collect::<Vec<_>>();
    if !methods.iter().any(|value| value == &method.to_ascii_uppercase()) {
        return Err(SecurityError::PreflightMethodNotAllowed);
    }

    if !requested_headers.is_empty() {
        let allowed = header(response_headers, "access-control-allow-headers")
            .unwrap_or("")
            .split(',')
            .map(|value| value.trim().to_ascii_lowercase())
            .collect::<Vec<_>>();
        for requested in requested_headers {
            if !allowed.iter().any(|value| value == "*" || value == &requested.to_ascii_lowercase()) {
                return Err(SecurityError::PreflightHeaderNotAllowed(requested.clone()));
            }
        }
    }
    Ok(())
}

#[must_use]
pub fn is_cors_safelisted_content_type(value: &str) -> bool {
    let essence = value
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    matches!(
        essence.as_str(),
        "application/x-www-form-urlencoded" | "multipart/form-data" | "text/plain"
    )
}

fn header<'a>(headers: &'a HashMap<String, String>, wanted: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
        .map(|(_, value)| value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_allows_credentialless_cors() {
        let source = Origin::from_url(&Url::parse("https://a.example/").unwrap());
        let target = Url::parse("https://b.example/data").unwrap();
        let mut headers = HashMap::new();
        headers.insert("access-control-allow-origin".to_owned(), "*".to_owned());
        assert!(enforce_fetch_policy(
            &source,
            &target,
            FetchMode::Cors,
            CredentialsMode::Omit,
            &headers
        )
        .is_ok());
    }

    #[test]
    fn same_origin_mode_blocks_cross_origin() {
        let source = Origin::from_url(&Url::parse("https://a.example/").unwrap());
        let target = Url::parse("https://b.example/").unwrap();
        assert_eq!(
            enforce_fetch_policy(
                &source,
                &target,
                FetchMode::SameOrigin,
                CredentialsMode::SameOrigin,
                &HashMap::new()
            ),
            Err(SecurityError::CrossOriginBlocked)
        );
    }

    #[test]
    fn json_post_requires_preflight() {
        assert!(requires_preflight("POST", Some("application/json")));
        assert!(!requires_preflight("POST", Some("text/plain;charset=UTF-8")));
        assert!(requires_preflight("PUT", None));
    }
}
