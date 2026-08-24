//! Networking backend for Nexus Engine 1.02.
//!
//! Reqwest remains the synchronous page/resource transport. Nexus 0.20 adds
//! HSTS upgrades, mixed-content/CSP gates, Referrer-Policy and CORS preflight.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::str::FromStr;
use std::time::Duration;

use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::{
    HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE, IF_MODIFIED_SINCE, IF_NONE_MATCH, ORIGIN,
    REFERER,
};
use reqwest::redirect::Policy;
use reqwest::Method;
use url::Url;

use crate::error::{NexusError, NexusResult};
use crate::origin::Origin;
use crate::policy::{is_mixed_active_content, PageSecurityContext};
use crate::security::{
    cors_origin_header, enforce_fetch_policy, enforce_preflight_response,
    requested_non_safelisted_headers, requires_preflight, should_send_credentials, CredentialsMode,
    FetchMode,
};
use crate::state::BrowserState;

pub const DEFAULT_MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubresourceKind {
    Script,
    Image,
    Connect,
}

#[derive(Debug, Clone)]
pub struct NetworkResponse {
    pub requested_url: Url,
    pub final_url: Url,
    pub status: u16,
    pub content_type: Option<String>,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub from_http_cache: bool,
    pub revalidated: bool,
    pub hsts_upgraded: bool,
}


#[derive(Debug, Clone)]
pub struct DownloadTransfer {
    pub requested_url: Url,
    pub final_url: Url,
    pub status: u16,
    pub content_type: Option<String>,
    pub bytes_written: u64,
    pub hsts_upgraded: bool,
}

#[derive(Clone)]
pub struct NetworkClient {
    client: Client,
    anonymous_client: Client,
    max_body_bytes: usize,
    state: Arc<BrowserState>,
}

impl NetworkClient {
    pub fn new(max_body_bytes: usize) -> NexusResult<Self> {
        Self::with_state(max_body_bytes, BrowserState::new(None))
    }

    pub fn with_state(max_body_bytes: usize, state: Arc<BrowserState>) -> NexusResult<Self> {
        let user_agent = format!("NexusEngine/{} (browser-core prototype)", env!("CARGO_PKG_VERSION"));
        let client = Client::builder()
            .user_agent(user_agent.clone())
            .cookie_provider(state.cookies())
            .redirect(Policy::limited(10))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(30))
            .build()?;
        let anonymous_client = Client::builder()
            .user_agent(user_agent)
            .redirect(Policy::limited(10))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { client, anonymous_client, max_body_bytes, state })
    }

    #[must_use]
    pub fn browser_state(&self) -> Arc<BrowserState> {
        Arc::clone(&self.state)
    }

    /// Streams a user-requested download to disk using the browser's shared
    /// cookie jar, redirects and learned HSTS state. The partial file is
    /// removed by the DownloadManager if this function returns an error.
    pub fn download_to_file(&self, url: &Url, path: &Path, max_bytes: u64) -> NexusResult<DownloadTransfer> {
        let requested_url = url.clone();
        let effective_url = self.state.hsts_upgrade(url).unwrap_or_else(|| url.clone());
        let hsts_upgraded = effective_url != requested_url;
        let mut response = self.client.get(effective_url).header(ACCEPT, "*/*").send()?;
        let final_url = response.url().clone();
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(NexusError::InvalidInput(format!("download returned HTTP {status}")));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        if let Some(declared) = response.content_length() {
            if declared > max_bytes {
                return Err(NexusError::BodyTooLarge {
                    limit: usize::try_from(max_bytes).unwrap_or(usize::MAX),
                    actual: usize::try_from(declared).unwrap_or(usize::MAX),
                });
            }
        }
        self.state.persist_cookies_best_effort();
        if final_url.scheme() == "https" {
            if let Some(value) = response
                .headers()
                .get("strict-transport-security")
                .and_then(|value| value.to_str().ok())
            {
                self.state.observe_hsts(&final_url, value);
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp_name = format!("{}.nexus-part", path.file_name().and_then(|value| value.to_str()).unwrap_or("download"));
        let temp_path = path.with_file_name(temp_name);
        let mut file = std::fs::File::create(&temp_path)?;
        let mut buffer = [0_u8; 64 * 1024];
        let mut bytes_written = 0_u64;
        loop {
            let read = response.read(&mut buffer)?;
            if read == 0 { break; }
            bytes_written = bytes_written.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
            if bytes_written > max_bytes {
                drop(file);
                let _ = std::fs::remove_file(&temp_path);
                return Err(NexusError::BodyTooLarge {
                    limit: usize::try_from(max_bytes).unwrap_or(usize::MAX),
                    actual: usize::try_from(bytes_written).unwrap_or(usize::MAX),
                });
            }
            file.write_all(&buffer[..read])?;
        }
        file.flush()?;
        drop(file);
        std::fs::rename(&temp_path, path)?;
        Ok(DownloadTransfer { requested_url, final_url, status, content_type, bytes_written, hsts_upgraded })
    }

    pub fn fetch(&self, url: &Url) -> NexusResult<NetworkResponse> {
        self.fetch_with_accept(url, "text/html,application/xhtml+xml;q=0.9,*/*;q=0.1")
    }

    pub fn fetch_with_accept(&self, url: &Url, accept: &str) -> NexusResult<NetworkResponse> {
        self.request(url, "GET", accept, None, None)
    }

    pub fn request(
        &self,
        url: &Url,
        method: &str,
        accept: &str,
        body: Option<&[u8]>,
        content_type: Option<&str>,
    ) -> NexusResult<NetworkResponse> {
        let method = parse_method(method)?;
        let use_cache = method == Method::GET;
        self.request_internal(&self.client, url, method, accept, body, content_type, None, None, &[], use_cache)
    }

    pub fn fetch_subresource(
        &self,
        security: &PageSecurityContext,
        url: &Url,
        accept: &str,
        kind: SubresourceKind,
    ) -> NexusResult<NetworkResponse> {
        self.enforce_subresource_policy(security, url, kind)?;
        let referer = security.referrer_policy.referer(&security.document_url, url);
        let response = self.request_internal(
            &self.client,
            url,
            Method::GET,
            accept,
            None,
            None,
            None,
            referer.as_deref(),
            &[],
            true,
        )?;
        // Redirects can change origin/scheme. Re-run the policy against the final URL
        // so an apparently safe request cannot redirect into blocked mixed content or CSP.
        self.enforce_subresource_policy(security, &response.final_url, kind)?;
        Ok(response)
    }

    pub fn web_request(
        &self,
        security: &PageSecurityContext,
        url: &Url,
        method: &str,
        accept: &str,
        body: Option<&[u8]>,
        content_type: Option<&str>,
        mode: FetchMode,
        credentials: CredentialsMode,
    ) -> NexusResult<NetworkResponse> {
        self.enforce_subresource_policy(security, url, SubresourceKind::Connect)?;
        let method = parse_method(method)?;
        let source_origin = Origin::from_url(&security.document_url);
        let target_origin = Origin::from_url(url);
        if mode == FetchMode::SameOrigin && !source_origin.is_same_origin(&target_origin) {
            return Err(NexusError::Security("Same-Origin Policy blocked a cross-origin fetch".to_owned()));
        }
        let send_credentials = should_send_credentials(&source_origin, url, credentials);
        let client = if send_credentials { &self.client } else { &self.anonymous_client };
        let origin_header = cors_origin_header(&source_origin, url);
        let referer = security.referrer_policy.referer(&security.document_url, url);

        if mode == FetchMode::Cors
            && !source_origin.is_same_origin(&target_origin)
            && requires_preflight(method.as_str(), content_type)
        {
            let requested_headers = requested_non_safelisted_headers(content_type);
            self.preflight(
                security,
                &source_origin,
                url,
                method.as_str(),
                credentials,
                &requested_headers,
                referer.as_deref(),
            )?;
        }

        let response = self.request_internal(
            client,
            url,
            method,
            accept,
            body,
            content_type,
            origin_header.as_deref(),
            referer.as_deref(),
            &[],
            false,
        )?;
        // Fetch redirects are security-relevant too: apply mixed-content/CSP to the
        // final response URL before exposing its body to JavaScript.
        self.enforce_subresource_policy(security, &response.final_url, SubresourceKind::Connect)?;
        enforce_fetch_policy(&source_origin, &response.final_url, mode, credentials, &response.headers)
            .map_err(|error| NexusError::Security(format!("CORS blocked fetch: {error:?}")))?;
        Ok(response)
    }

    pub fn enforce_subresource_policy(
        &self,
        security: &PageSecurityContext,
        target: &Url,
        kind: SubresourceKind,
    ) -> NexusResult<()> {
        if is_mixed_active_content(&security.document_url, target) {
            return Err(NexusError::Security(format!("mixed active content blocked: {target}")));
        }
        let allowed = match kind {
            SubresourceKind::Script => security.csp.allows_script_url(&security.document_url, target),
            SubresourceKind::Image => security.csp.allows_image_url(&security.document_url, target),
            SubresourceKind::Connect => security.csp.allows_connect_url(&security.document_url, target),
        };
        if !allowed {
            return Err(NexusError::Security(format!("CSP blocked {kind:?} resource: {target}")));
        }
        Ok(())
    }

    fn preflight(
        &self,
        security: &PageSecurityContext,
        source_origin: &Origin,
        url: &Url,
        method: &str,
        credentials: CredentialsMode,
        requested_headers: &[String],
        referer: Option<&str>,
    ) -> NexusResult<()> {
        let mut extra = vec![("access-control-request-method", method.to_ascii_uppercase())];
        if !requested_headers.is_empty() {
            extra.push(("access-control-request-headers", requested_headers.join(", ")));
        }
        let origin = source_origin.serialize();
        let response = self.request_internal(
            &self.anonymous_client,
            url,
            Method::OPTIONS,
            "*/*",
            None,
            None,
            Some(&origin),
            referer,
            &extra,
            false,
        )?;
        if !(200..300).contains(&response.status) {
            return Err(NexusError::Security(format!("CORS preflight returned HTTP {}", response.status)));
        }
        self.enforce_subresource_policy(security, &response.final_url, SubresourceKind::Connect)?;
        enforce_preflight_response(
            source_origin,
            &response.final_url,
            credentials,
            method,
            requested_headers,
            &response.headers,
        )
        .map_err(|error| NexusError::Security(format!("CORS preflight blocked fetch: {error:?}")))
    }

    #[allow(clippy::too_many_arguments)]
    fn request_internal(
        &self,
        client: &Client,
        url: &Url,
        method: Method,
        accept: &str,
        body: Option<&[u8]>,
        content_type: Option<&str>,
        origin: Option<&str>,
        referer: Option<&str>,
        extra_headers: &[(&str, String)],
        use_cache: bool,
    ) -> NexusResult<NetworkResponse> {
        let requested_url = url.clone();
        let effective_url = self.state.hsts_upgrade(url).unwrap_or_else(|| url.clone());
        let hsts_upgraded = effective_url != requested_url;
        let cached = if use_cache && method == Method::GET {
            self.state.http_cache().lock().ok().and_then(|mut cache| cache.lookup(&effective_url))
        } else {
            None
        };
        if let Some(entry) = cached.as_ref().filter(|entry| entry.is_fresh()) {
            return Ok(NetworkResponse {
                requested_url,
                final_url: entry.final_url.clone(),
                status: entry.status,
                content_type: entry.content_type.clone(),
                headers: entry.headers.clone(),
                body: entry.body.clone(),
                from_http_cache: true,
                revalidated: false,
                hsts_upgraded,
            });
        }

        let mut request = client.request(method.clone(), effective_url.clone()).header(ACCEPT, accept);
        if let Some(value) = content_type { request = request.header(CONTENT_TYPE, value); }
        if let Some(value) = origin { request = request.header(ORIGIN, value); }
        if let Some(value) = referer { request = request.header(REFERER, value); }
        for (name, value) in extra_headers {
            if let (Ok(name), Ok(value)) = (HeaderName::from_bytes(name.as_bytes()), HeaderValue::from_str(value)) {
                request = request.header(name, value);
            }
        }
        if let Some(body) = body { request = request.body(body.to_vec()); }
        if let Some(entry) = &cached {
            let (etag, last_modified) = entry.validators();
            request = apply_validators(request, etag, last_modified);
        }

        let mut response = request.send()?;
        let final_url = response.url().clone();
        let status = response.status().as_u16();
        let headers = response_headers(response.headers());
        let content_type = response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()).map(str::to_owned);
        self.state.persist_cookies_best_effort();
        if final_url.scheme() == "https" {
            if let Some(value) = headers.get("strict-transport-security") {
                self.state.observe_hsts(&final_url, value);
            }
        }

        if status == 304 {
            if let Some(entry) = cached {
                if let Ok(mut cache) = self.state.http_cache().lock() {
                    cache.refresh_after_304(&effective_url, &headers);
                }
                return Ok(NetworkResponse {
                    requested_url,
                    final_url: entry.final_url,
                    status: entry.status,
                    content_type: entry.content_type,
                    headers: entry.headers,
                    body: entry.body,
                    from_http_cache: true,
                    revalidated: true,
                    hsts_upgraded,
                });
            }
        }

        if let Some(declared) = response.content_length() {
            if declared > self.max_body_bytes as u64 {
                return Err(NexusError::BodyTooLarge { limit: self.max_body_bytes, actual: usize::try_from(declared).unwrap_or(usize::MAX) });
            }
        }
        let mut bytes = Vec::with_capacity(
            response.content_length().and_then(|size| usize::try_from(size).ok()).unwrap_or(64 * 1024).min(self.max_body_bytes),
        );
        let mut limited = (&mut response).take(self.max_body_bytes as u64 + 1);
        limited.read_to_end(&mut bytes)?;
        if bytes.len() > self.max_body_bytes {
            return Err(NexusError::BodyTooLarge { limit: self.max_body_bytes, actual: bytes.len() });
        }

        if use_cache && method == Method::GET {
            if let Ok(mut cache) = self.state.http_cache().lock() {
                cache.insert(&effective_url, final_url.clone(), status, content_type.clone(), headers.clone(), bytes.clone());
            }
        }

        Ok(NetworkResponse {
            requested_url,
            final_url,
            status,
            content_type,
            headers,
            body: bytes,
            from_http_cache: false,
            revalidated: false,
            hsts_upgraded,
        })
    }
}

fn parse_method(value: &str) -> NexusResult<Method> {
    match value.trim().to_ascii_uppercase().as_str() {
        "GET" => Ok(Method::GET),
        "HEAD" => Ok(Method::HEAD),
        "POST" => Ok(Method::POST),
        "PUT" => Ok(Method::PUT),
        "PATCH" => Ok(Method::PATCH),
        "DELETE" => Ok(Method::DELETE),
        "OPTIONS" => Ok(Method::OPTIONS),
        other => Err(NexusError::InvalidInput(format!("HTTP method {other} is not supported by Nexus 0.20"))),
    }
}

fn apply_validators(mut request: RequestBuilder, etag: Option<&str>, last_modified: Option<&str>) -> RequestBuilder {
    if let Some(value) = etag { request = request.header(IF_NONE_MATCH, value); }
    if let Some(value) = last_modified { request = request.header(IF_MODIFIED_SINCE, value); }
    request
}

fn response_headers(headers: &reqwest::header::HeaderMap) -> HashMap<String, String> {
    let mut output = HashMap::new();
    for (name, value) in headers {
        if let Ok(value) = value.to_str() {
            output.insert(name.as_str().to_ascii_lowercase(), value.to_owned());
        }
    }
    output
}
