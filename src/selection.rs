//! Text selection and context-target discovery for Nexus Engine 1.02.
//!
//! The first Alpha selection model intentionally selects the layout/DOM node
//! under a long press. Fine-grained character handles are a later milestone.

use url::Url;

use crate::address::resolve_url;
use crate::display_list::{visual_rect_for_node, PaintRect};
use crate::dom::{DomNodeData, NodeId};
use crate::engine::LoadedPage;
use crate::hit_test::hit_test_page;

#[derive(Debug, Clone, PartialEq)]
pub struct SelectionInfo {
    pub node_id: NodeId,
    pub text: String,
    pub rect: PaintRect,
    pub link_url: Option<Url>,
    pub link_label: Option<String>,
    pub image_url: Option<Url>,
}

impl SelectionInfo {
    #[must_use]
    pub fn has_text(&self) -> bool {
        !self.text.trim().is_empty()
    }
}

#[must_use]
pub fn selection_at(
    page: &LoadedPage,
    viewport_x: f32,
    viewport_y: f32,
    scroll_y: f32,
) -> Option<SelectionInfo> {
    let hit = hit_test_page(page, viewport_x, viewport_y, scroll_y)?;
    let node = page.dom.node(hit.node_id)?;
    let text = match &node.data {
        DomNodeData::Text(value) => normalize_selection_text(value),
        _ => normalize_selection_text(&page.dom.text_content(hit.node_id)),
    };
    let visual = visual_rect_for_node(&page.dom, &page.styles, &page.layout, hit.node_id, scroll_y)?;
    let image_url = closest_image_url(page, hit.node_id);

    Some(SelectionInfo {
        node_id: hit.node_id,
        text: truncate_chars(&text, 16_384),
        // SelectionInfo keeps document-space Y because BrowserSession applies
        // the current scroll when painting the highlight. Advanced CSS visual
        // geometry is converted back to that convention here.
        rect: PaintRect {
            x: visual.x,
            y: visual.y + scroll_y,
            width: visual.width,
            height: visual.height,
        },
        link_url: hit.link_url,
        link_label: hit.link_label,
        image_url,
    })
}

fn closest_image_url(page: &LoadedPage, start: NodeId) -> Option<Url> {
    let mut current = Some(start);
    while let Some(node_id) = current {
        let node = page.dom.node(node_id)?;
        if let DomNodeData::Element { tag_name, .. } = &node.data {
            if tag_name.eq_ignore_ascii_case("img") {
                let src = page.dom.attribute(node_id, "src")?.trim();
                if !src.is_empty() {
                    return resolve_url(&page.dom.base_url(), src);
                }
            }
        }
        current = node.parent;
    }
    None
}

fn normalize_selection_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_owned();
    }
    value.chars().take(max).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::truncate_chars;

    #[test]
    fn truncation_is_unicode_safe() {
        assert_eq!(truncate_chars("àèìòù", 3), "àèì…");
    }
}
