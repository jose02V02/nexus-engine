//! Favicon discovery/fetching for Nexus Engine 1.02.

use std::io::Cursor;

use image::{ImageFormat, ImageReader};
use url::Url;

use crate::address::resolve_url;
use crate::engine::LoadedPage;
use crate::network::{NetworkClient, SubresourceKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaviconData {
    pub url: Url,
    pub png: Vec<u8>,
}

#[must_use]
pub fn discover_favicon_urls(page: &LoadedPage) -> Vec<Url> {
    discover_favicon_urls_from_dom(&page.dom, &page.final_url)
}

fn discover_favicon_urls_from_dom(dom: &crate::dom::NexusDom, final_url: &Url) -> Vec<Url> {
    let base = dom.base_url();
    let mut weighted = Vec::<(u8, Url)>::new();
    for node in dom.nodes() {
        if dom.element_tag_name(node.id) != Some("link") {
            continue;
        }
        let rel = dom.attribute(node.id, "rel").unwrap_or("").to_ascii_lowercase();
        if !rel.split_ascii_whitespace().any(|part| part == "icon" || part == "apple-touch-icon") {
            continue;
        }
        let Some(href) = dom.attribute(node.id, "href") else { continue };
        let Some(url) = resolve_url(&base, href) else { continue };
        let priority = if rel.split_ascii_whitespace().any(|part| part == "icon") { 0 } else { 1 };
        weighted.push((priority, url));
    }
    weighted.sort_by_key(|(priority, _)| *priority);
    let mut urls = weighted.into_iter().map(|(_, url)| url).collect::<Vec<_>>();

    if let Ok(fallback) = final_url.join("/favicon.ico") {
        if !urls.contains(&fallback) {
            urls.push(fallback);
        }
    }
    urls.truncate(6);
    urls
}

pub fn fetch_favicon(network: &NetworkClient, page: &LoadedPage) -> Option<FaviconData> {
    for url in discover_favicon_urls(page) {
        let Ok(response) = network
            .fetch_subresource(&page.security, &url, "image/*,*/*;q=0.1", SubresourceKind::Image)
        else {
            continue;
        };
        if !(200..300).contains(&response.status) || response.body.is_empty() {
            continue;
        }
        let Ok(inspect) = ImageReader::new(Cursor::new(response.body.as_slice())).with_guessed_format() else { continue };
        let Ok((width, height)) = inspect.into_dimensions() else { continue };
        let pixels = u64::from(width).saturating_mul(u64::from(height));
        if pixels == 0 || pixels > 4_000_000 {
            continue;
        }
        let Ok(reader) = ImageReader::new(Cursor::new(response.body.as_slice())).with_guessed_format() else { continue };
        let Ok(decoded) = std::panic::catch_unwind(|| reader.decode()) else { continue };
        let Ok(image) = decoded else { continue };
        let icon = image.thumbnail(64, 64);
        let mut cursor = Cursor::new(Vec::new());
        if icon.write_to(&mut cursor, ImageFormat::Png).is_ok() {
            return Some(FaviconData {
                url: response.final_url,
                png: cursor.into_inner(),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::encoding::decode_html;
    use crate::parser::parse_html;

    use super::*;

    #[test]
    fn discovers_link_icon_before_fallback() {
        let url = Url::parse("https://example.com/path/page").unwrap();
        let html = br#"<html><head><link rel='icon' href='/assets/icon.png'></head></html>"#;
        let decoded = decode_html(html, Some("text/html; charset=utf-8"));
        let dom = parse_html(url.clone(), &decoded.text);
        let urls = discover_favicon_urls_from_dom(&dom, &url);
        assert_eq!(urls[0].as_str(), "https://example.com/assets/icon.png");
        assert_eq!(urls.last().unwrap().as_str(), "https://example.com/favicon.ico");
    }
}
