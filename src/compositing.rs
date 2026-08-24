//! Nexus 0.20 compositing/stacking foundation.
//!
//! This is intentionally renderer-neutral. Layout computes geometry; this
//! module computes a deterministic paint order and records the first reasons
//! that cause a node to behave as its own stacking context. A future GPU
//! compositor can consume the same metadata instead of rebuilding CSS policy
//! inside Skia/WebRender/Vello backends.

use crate::css::{CssPosition, CssTransform, StyleMap};
use crate::dom::{NexusDom, NodeId};
use crate::layout::LayoutTree;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackingReason {
    PositionedZIndex,
    FixedOrSticky,
    Opacity,
    Transform,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackingContextEntry {
    pub node_id: NodeId,
    pub z_index: i32,
    pub source_order: usize,
    pub reasons: Vec<StackingReason>,
}

#[must_use]
pub fn stacking_contexts(dom: &NexusDom, styles: &StyleMap, layout: &LayoutTree) -> Vec<StackingContextEntry> {
    let mut result = Vec::new();
    for (source_order, item) in layout.boxes.iter().enumerate() {
        let Some(style) = styles.get(item.node_id) else { continue };
        let mut reasons = Vec::new();
        if style.position != CssPosition::Static && style.z_index.is_some() {
            reasons.push(StackingReason::PositionedZIndex);
        }
        if matches!(style.position, CssPosition::Fixed | CssPosition::Sticky) {
            reasons.push(StackingReason::FixedOrSticky);
        }
        if style.opacity < 0.999 {
            reasons.push(StackingReason::Opacity);
        }
        if style.transform != CssTransform::default() {
            reasons.push(StackingReason::Transform);
        }
        if !reasons.is_empty() {
            result.push(StackingContextEntry {
                node_id: item.node_id,
                z_index: effective_z_index(dom, styles, item.node_id),
                source_order,
                reasons,
            });
        }
    }
    result
}

/// Returns indices into `LayoutTree::boxes` in Nexus paint order.
///
/// 0.20 implements the useful core of CSS stacking: negative z-index first,
/// normal/auto content next, positive z-index last, all stable by DOM/layout
/// source order. Nested context flattening is deliberately deferred until the
/// compositor owns layers rather than a flat Skia display list.
#[must_use]
pub fn paint_order_indices(dom: &NexusDom, styles: &StyleMap, layout: &LayoutTree) -> Vec<usize> {
    let mut indices = (0..layout.boxes.len()).collect::<Vec<_>>();
    indices.sort_by_key(|&index| {
        let node_id = layout.boxes[index].node_id;
        let z = effective_z_index(dom, styles, node_id);
        let phase = if z < 0 { 0_i8 } else if z == 0 { 1_i8 } else { 2_i8 };
        (phase, z, index)
    });
    indices
}

#[must_use]
pub fn effective_z_index(dom: &NexusDom, styles: &StyleMap, node_id: NodeId) -> i32 {
    let mut current = Some(node_id);
    while let Some(id) = current {
        if let Some(style) = styles.get(id) {
            if let Some(z) = style.z_index {
                return z;
            }
        }
        current = dom.node(id).and_then(|node| node.parent);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::{compute_styles_for_viewport, MediaEnvironment};
    use crate::layout::{compute_layout, Viewport};
    use crate::parser::parse_html;
    use url::Url;

    #[test]
    fn identifies_transform_and_position_contexts() {
        let dom = parse_html(
            Url::parse("https://nexus.local/").unwrap(),
            r#"<style>#a{position:fixed;z-index:5}#b{transform:scale(2)}</style><div id="a"></div><div id="b"></div>"#,
        );
        let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 390.0, height: 844.0 });
        let layout = compute_layout(&dom, &styles, Viewport { width: 390.0, height: 844.0 }).unwrap();
        let contexts = stacking_contexts(&dom, &styles, &layout);
        assert!(contexts.iter().any(|entry| entry.reasons.contains(&StackingReason::FixedOrSticky)));
        assert!(contexts.iter().any(|entry| entry.reasons.contains(&StackingReason::Transform)));
    }
}
