//! Page resource loading and an in-memory Nexus cache.
//!
//! Networking, decoding and renderer code stay separate: this module turns
//! resource URLs into Nexus-owned decoded assets that can later be referenced
//! by the display list.

use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::sync::Arc;

use image::ImageReader;
use url::Url;

use crate::dom::{ImageReference, NexusDom, NodeId};
use crate::network::{NetworkClient, NetworkResponse, SubresourceKind};
use crate::policy::PageSecurityContext;

pub const DEFAULT_CACHE_ENTRIES: usize = 64;
pub const DEFAULT_CACHE_BYTES: usize = 32 * 1024 * 1024;
pub const DEFAULT_MAX_IMAGE_PIXELS: u64 = 16_000_000;
pub const DEFAULT_MAX_PAGE_DECODED_IMAGE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct CachedResource {
    pub final_url: Url,
    pub content_type: Option<String>,
    pub body: Arc<[u8]>,
}

#[derive(Debug, Clone)]
pub struct ImageResource {
    pub node_id: NodeId,
    pub url: Url,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PageResources {
    pub images: HashMap<NodeId, ImageResource>,
    pub warnings: Vec<String>,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

impl PageResources {
    #[must_use]
    pub fn intrinsic_sizes(&self) -> HashMap<NodeId, (f32, f32)> {
        self.images
            .iter()
            .map(|(&node_id, image)| (node_id, (image.width as f32, image.height as f32)))
            .collect()
    }
}

#[derive(Debug)]
pub struct ResourceCache {
    entries: HashMap<String, CachedResource>,
    order: VecDeque<String>,
    current_bytes: usize,
    max_entries: usize,
    max_bytes: usize,
    hits: usize,
    misses: usize,
}

impl Default for ResourceCache {
    fn default() -> Self {
        Self::new(DEFAULT_CACHE_ENTRIES, DEFAULT_CACHE_BYTES)
    }
}

impl ResourceCache {
    #[must_use]
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            current_bytes: 0,
            max_entries: max_entries.max(1),
            max_bytes: max_bytes.max(1024 * 1024),
            hits: 0,
            misses: 0,
        }
    }

    pub fn get(&mut self, url: &Url) -> Option<CachedResource> {
        let key = url.as_str();
        if let Some(entry) = self.entries.get(key).cloned() {
            self.hits = self.hits.saturating_add(1);
            self.touch(key);
            Some(entry)
        } else {
            self.misses = self.misses.saturating_add(1);
            None
        }
    }

    pub fn insert_response(&mut self, response: NetworkResponse) -> CachedResource {
        let key = response.requested_url.as_str().to_owned();
        let entry = CachedResource {
            final_url: response.final_url,
            content_type: response.content_type,
            body: Arc::<[u8]>::from(response.body),
        };

        if entry.body.len() <= self.max_bytes {
            if let Some(previous) = self.entries.remove(&key) {
                self.current_bytes = self.current_bytes.saturating_sub(previous.body.len());
            }
            self.current_bytes = self.current_bytes.saturating_add(entry.body.len());
            self.entries.insert(key.clone(), entry.clone());
            self.order.retain(|item| item != &key);
            self.order.push_back(key);
            self.evict_if_needed();
        }
        entry
    }

    #[must_use]
    pub const fn hits(&self) -> usize {
        self.hits
    }

    #[must_use]
    pub const fn misses(&self) -> usize {
        self.misses
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub const fn bytes(&self) -> usize {
        self.current_bytes
    }

    fn touch(&mut self, key: &str) {
        self.order.retain(|item| item != key);
        self.order.push_back(key.to_owned());
    }

    fn evict_if_needed(&mut self) {
        while self.entries.len() > self.max_entries || self.current_bytes > self.max_bytes {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.current_bytes = self.current_bytes.saturating_sub(entry.body.len());
            }
        }
    }
}

pub fn load_page_resources(
    dom: &NexusDom,
    network: &NetworkClient,
    cache: &mut ResourceCache,
    security: &PageSecurityContext,
) -> PageResources {
    let start_hits = cache.hits();
    let start_misses = cache.misses();
    let mut page = PageResources::default();
    let mut decoded_image_bytes = 0usize;

    for image_ref in dom.images() {
        match load_image(image_ref, network, cache, security) {
            Ok(Some(image)) => {
                let next = decoded_image_bytes.saturating_add(image.rgba.len());
                if next > DEFAULT_MAX_PAGE_DECODED_IMAGE_BYTES {
                    page.warnings.push(format!(
                        "budget immagini decodificate superato ({} MiB): {} ignorata",
                        DEFAULT_MAX_PAGE_DECODED_IMAGE_BYTES / (1024 * 1024),
                        image.url
                    ));
                    continue;
                }
                decoded_image_bytes = next;
                page.images.insert(image.node_id, image);
            }
            Ok(None) => {}
            Err(message) => page.warnings.push(message),
        }
    }

    page.cache_hits = cache.hits().saturating_sub(start_hits);
    page.cache_misses = cache.misses().saturating_sub(start_misses);
    page
}

fn load_image(
    image_ref: ImageReference,
    network: &NetworkClient,
    cache: &mut ResourceCache,
    security: &PageSecurityContext,
) -> Result<Option<ImageResource>, String> {
    let Some(url) = image_ref.resolved_url else {
        return Ok(None);
    };
    if !matches!(url.scheme(), "http" | "https") {
        return Ok(None);
    }
    network
        .enforce_subresource_policy(security, &url, SubresourceKind::Image)
        .map_err(|err| format!("image {} blocked: {err}", url))?;

    let resource = if let Some(entry) = cache.get(&url) {
        entry
    } else {
        let response = network
            .fetch_subresource(security, &url, "image/webp,image/png,image/jpeg,image/gif,*/*;q=0.2", SubresourceKind::Image)
            .map_err(|err| format!("image {}: {err}", url))?;
        cache.insert_response(response)
    };

    let decoded = decode_image(&resource.body)
        .map_err(|err| format!("image {} non decodificata: {err}", resource.final_url))?;

    Ok(Some(ImageResource {
        node_id: image_ref.node_id,
        url: resource.final_url,
        width: decoded.0,
        height: decoded.1,
        rgba: decoded.2,
        content_type: resource.content_type,
    }))
}

fn decode_image(bytes: &[u8]) -> Result<(u32, u32, Arc<[u8]>), String> {
    let inspect = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|err| err.to_string())?;
    let (width, height) = inspect.into_dimensions().map_err(|err| err.to_string())?;
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if pixels == 0 || pixels > DEFAULT_MAX_IMAGE_PIXELS {
        return Err(format!("dimensioni rifiutate: {width}x{height}"));
    }

    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|err| err.to_string())?;
    let dynamic = std::panic::catch_unwind(|| reader.decode())
        .map_err(|_| "decoder image ha generato un panic controllato".to_owned())?
        .map_err(|err| err.to_string())?;
    let rgba = dynamic.to_rgba8();
    Ok((width, height, Arc::<[u8]>::from(rgba.into_raw())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_evicts_old_entries() {
        let mut cache = ResourceCache::new(1, 1024 * 1024);
        let make = |url: &str, body: &[u8]| NetworkResponse {
            requested_url: Url::parse(url).unwrap(),
            final_url: Url::parse(url).unwrap(),
            status: 200,
            content_type: None,
            headers: std::collections::HashMap::new(),
            body: body.to_vec(),
            from_http_cache: false,
            revalidated: false,
            hsts_upgraded: false,
        };
        cache.insert_response(make("https://nexus.local/a", b"a"));
        cache.insert_response(make("https://nexus.local/b", b"b"));
        assert_eq!(cache.len(), 1);
        assert!(cache.get(&Url::parse("https://nexus.local/a").unwrap()).is_none());
    }
}
