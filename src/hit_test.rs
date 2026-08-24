//! Hit testing for Nexus Engine 1.02.
//!
//! Converts viewport coordinates into document coordinates, finds the deepest
//! layout box under the pointer and resolves interactive ancestors such as
//! `<a href>` through the Nexus DOM.

use url::Url;

use crate::address::resolve_url;
use crate::dom::{DomNodeData, NexusDom, NodeId};
use crate::engine::LoadedPage;
use crate::compositing::paint_order_indices;
use crate::display_list::{point_visible_through_overflow, visual_rect_for_node};
use crate::css::{CssPointerEvents, CssVisibility};

#[derive(Debug, Clone, PartialEq)]
pub struct HitTestResult {
    pub node_id: NodeId,
    pub label: String,
    pub document_x: f32,
    pub document_y: f32,
    pub link_node_id: Option<NodeId>,
    pub link_url: Option<Url>,
    pub link_label: Option<String>,
}

impl HitTestResult {
    #[must_use]
    pub fn is_link(&self) -> bool {
        self.link_url.is_some()
    }
}

#[must_use]
pub fn hit_test_page(page: &LoadedPage, viewport_x: f32, viewport_y: f32, requested_scroll_y: f32) -> Option<HitTestResult> {
    if viewport_x < 0.0
        || viewport_y < 0.0
        || viewport_x > page.layout.viewport.width
        || viewport_y > page.layout.viewport.height
    {
        return None;
    }

    let max_scroll = page.max_scroll_y();
    let scroll_y = requested_scroll_y.clamp(0.0, max_scroll);
    let document_x = viewport_x;
    let document_y = viewport_y + scroll_y;

    let target = paint_order_indices(&page.dom, &page.styles, &page.layout)
        .into_iter()
        .rev()
        .filter_map(|index| page.layout.boxes.get(index))
        .find(|item| {
            if !pointer_target_enabled(&page.dom, &page.styles, item.node_id) {
                return false;
            }
            let Some(rect) = visual_rect_for_node(&page.dom, &page.styles, &page.layout, item.node_id, scroll_y) else {
                return false;
            };
            viewport_x >= rect.x
                && viewport_x <= rect.x + rect.width
                && viewport_y >= rect.y
                && viewport_y <= rect.y + rect.height
                && point_visible_through_overflow(
                    &page.dom,
                    &page.styles,
                    &page.layout,
                    item.node_id,
                    scroll_y,
                    viewport_x,
                    viewport_y,
                )
        })?;

    let (link_node_id, link_url, link_label) = closest_link(&page.dom, target.node_id)
        .map_or((None, None, None), |(node_id, url, label)| {
            (Some(node_id), Some(url), Some(label))
        });

    Some(HitTestResult {
        node_id: target.node_id,
        label: target.label.clone(),
        document_x,
        document_y,
        link_node_id,
        link_url,
        link_label,
    })
}

fn pointer_target_enabled(dom: &NexusDom, styles: &crate::css::StyleMap, start: NodeId) -> bool {
    let mut current = Some(start);
    while let Some(node_id) = current {
        if let Some(style) = styles.get(node_id) {
            return style.visibility == CssVisibility::Visible
                && style.pointer_events == CssPointerEvents::Auto;
        }
        current = dom.node(node_id).and_then(|node| node.parent);
    }
    true
}

fn closest_link(dom: &NexusDom, start: NodeId) -> Option<(NodeId, Url, String)> {
    let base = dom.base_url();
    let mut current = Some(start);

    while let Some(node_id) = current {
        let node = dom.node(node_id)?;
        if let DomNodeData::Element { tag_name, .. } = &node.data {
            if tag_name.eq_ignore_ascii_case("a") {
                let href = dom.attribute(node_id, "href")?.trim();
                let url = resolve_url(&base, href)?;
                let label = dom.text_content(node_id).split_whitespace().collect::<Vec<_>>().join(" ");
                return Some((node_id, url, label));
            }
        }
        current = node.parent;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::{compute_styles_for_viewport, MediaEnvironment};
    use crate::parser::parse_html;

    #[test]
    fn inherited_pointer_events_none_removes_text_targets() {
        let dom = parse_html(Url::parse("https://nexus.local/").unwrap(),
            "<a id='link' href='/next' style='pointer-events:none'>disabled</a>");
        let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 320.0, height: 240.0 });
        let link = dom.find_element_by_id("link").unwrap();
        let text = dom.node(link).unwrap().children[0];
        assert!(!pointer_target_enabled(&dom, &styles, text));
    }
}
