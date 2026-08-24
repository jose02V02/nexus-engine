//! Layout adapter for Nexus Engine 1.02.
//!
//! Converts Nexus-owned computed styles into Taffy styles, computes Block/Flex/
//! Grid geometry, then copies the result back into Nexus-owned `LayoutTree`.

use std::collections::HashMap;

use taffy::prelude::{
    AlignContent, AlignItems, AvailableSpace, BoxSizing, Dimension, Display, FlexDirection, FlexWrap, LengthPercentage,
    LengthPercentageAuto, MaxTrackSizingFunction, MinTrackSizingFunction, Rect, RepetitionCount,
    Size, Style, TaffyTree, TrackSizingFunction,
};
use taffy::geometry::{Line, Point};
use taffy::style::{GridAutoFlow, Overflow, Position};
use taffy::style_helpers::{auto as grid_auto, fit_content, fr as grid_fr, length as grid_length, line as grid_line, max_content, min_content, minmax, percent as grid_percent, repeat as grid_repeat, span as grid_span};
use taffy::tree::NodeId as TaffyNodeId;

use crate::css::{ComputedStyle, CssBoxSizing, CssContentAlignment, CssDisplay, CssFlexDirection, CssFlexWrap, CssGridAutoFlow, CssGridBreadth, CssGridLine, CssGridPlacement, CssGridRepeat, CssGridTrack, CssItemAlignment, CssLength, CssOverflow, CssPosition, CssWhiteSpace, StyleMap};
use crate::dom::{DomNodeData, NexusDom, NodeId};
use crate::error::{NexusError, NexusResult};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self { width: 1280.0, height: 720.0 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutBox {
    pub node_id: NodeId,
    pub label: String,
    pub depth: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Default)]
pub struct LayoutTree {
    pub viewport: Viewport,
    pub boxes: Vec<LayoutBox>,
}

impl LayoutTree {
    pub fn box_for(&self, node_id: NodeId) -> Option<&LayoutBox> {
        self.boxes.iter().find(|item| item.node_id == node_id)
    }

    pub fn pretty(&self) -> String {
        let mut out = format!("viewport: {:.0}x{:.0}\n", self.viewport.width, self.viewport.height);
        for item in &self.boxes {
            out.push_str(&format!(
                "{}#{:<4} {:<22} x={:>7.1} y={:>7.1} w={:>7.1} h={:>7.1}\n",
                "  ".repeat(item.depth.min(20)),
                item.node_id,
                item.label,
                item.x,
                item.y,
                item.width,
                item.height
            ));
        }
        out
    }
}

struct BuiltNode {
    dom_id: NodeId,
    taffy_id: TaffyNodeId,
    parent_dom_id: Option<NodeId>,
    depth: usize,
}

pub type IntrinsicSizeMap = HashMap<NodeId, (f32, f32)>;

pub fn compute_layout(dom: &NexusDom, styles: &StyleMap, viewport: Viewport) -> NexusResult<LayoutTree> {
    let intrinsic = IntrinsicSizeMap::new();
    compute_layout_with_intrinsics(dom, styles, viewport, &intrinsic)
}

pub fn compute_layout_with_intrinsics(
    dom: &NexusDom,
    styles: &StyleMap,
    viewport: Viewport,
    intrinsic: &IntrinsicSizeMap,
) -> NexusResult<LayoutTree> {
    let mut taffy: TaffyTree<()> = TaffyTree::new();
    let mut built = Vec::new();

    let root = build_node(
        dom,
        styles,
        dom.root(),
        None,
        None,
        0,
        viewport,
        intrinsic,
        &mut taffy,
        &mut built,
    )?
    .ok_or_else(|| NexusError::Layout("il DOM non ha prodotto alcun box".to_owned()))?;

    taffy
        .compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(viewport.width.max(1.0)),
                height: AvailableSpace::MaxContent,
            },
        )
        .map_err(|err| NexusError::Layout(err.to_string()))?;

    let index_by_dom = built
        .iter()
        .enumerate()
        .map(|(index, item)| (item.dom_id, index))
        .collect::<HashMap<_, _>>();
    let mut absolute_cache: HashMap<NodeId, (f32, f32)> = HashMap::new();
    let mut boxes = Vec::with_capacity(built.len());

    for item in &built {
        let layout = taffy
            .layout(item.taffy_id)
            .map_err(|err| NexusError::Layout(err.to_string()))?;
        let (parent_x, parent_y) = item
            .parent_dom_id
            .and_then(|parent| absolute_position(parent, &built, &index_by_dom, &taffy, &mut absolute_cache).ok())
            .unwrap_or((0.0, 0.0));
        let x = parent_x + layout.location.x;
        let y = parent_y + layout.location.y;
        absolute_cache.insert(item.dom_id, (x, y));
        boxes.push(LayoutBox {
            node_id: item.dom_id,
            label: dom.node_label(item.dom_id),
            depth: item.depth,
            x,
            y,
            width: layout.size.width,
            height: layout.size.height,
        });
    }

    boxes.sort_by_key(|item| item.node_id);
    Ok(LayoutTree { viewport, boxes })
}

#[allow(clippy::too_many_arguments)]
fn build_node(
    dom: &NexusDom,
    styles: &StyleMap,
    node_id: NodeId,
    parent_dom_id: Option<NodeId>,
    inherited_style: Option<&ComputedStyle>,
    depth: usize,
    viewport: Viewport,
    intrinsic: &IntrinsicSizeMap,
    taffy: &mut TaffyTree<()>,
    built: &mut Vec<BuiltNode>,
) -> NexusResult<Option<TaffyNodeId>> {
    let Some(node) = dom.node(node_id) else { return Ok(None) };

    match &node.data {
        DomNodeData::Document => {
            let mut children = Vec::new();
            for &child in &node.children {
                if let Some(child_node) = build_node(
                    dom, styles, child, Some(node_id), inherited_style, depth + 1,
                    viewport, intrinsic, taffy, built,
                )? {
                    children.push(child_node);
                }
            }
            let root_style = Style {
                display: Display::Block,
                size: Size { width: Dimension::length(viewport.width), height: Dimension::auto() },
                ..Default::default()
            };
            let taffy_id = taffy
                .new_with_children(root_style, &children)
                .map_err(|err| NexusError::Layout(err.to_string()))?;
            built.push(BuiltNode { dom_id: node_id, taffy_id, parent_dom_id, depth });
            Ok(Some(taffy_id))
        }
        DomNodeData::Element { tag_name, .. } => {
            let Some(style) = styles.get(node_id) else { return Ok(None) };
            if style.display == CssDisplay::None {
                return Ok(None);
            }

            let mut ordered_children = Vec::new();
            for (source_index, &child) in node.children.iter().enumerate() {
                if let Some(child_node) = build_node(
                    dom, styles, child, Some(node_id), Some(style), depth + 1,
                    viewport, intrinsic, taffy, built,
                )? {
                    let order = styles.get(child).map_or(0, |child_style| child_style.order);
                    ordered_children.push((order, source_index, child_node));
                }
            }
            if matches!(style.display, CssDisplay::Flex | CssDisplay::Grid) {
                ordered_children.sort_by_key(|&(order, source_index, _)| (order, source_index));
            }
            let children = ordered_children.into_iter().map(|(_, _, child)| child).collect::<Vec<_>>();

            let mut taffy_style = to_taffy_style(style);
            if let (Some(area_name), Some(parent_style)) = (style.grid_area_name.as_deref(), inherited_style) {
                if let Some(area) = parent_style.grid_template_areas.as_ref().and_then(|template| template.areas.get(area_name)) {
                    taffy_style.grid_row = to_grid_line(CssGridLine {
                        start: CssGridPlacement::Line(area.row_start), end: CssGridPlacement::Line(area.row_end),
                    });
                    taffy_style.grid_column = to_grid_line(CssGridLine {
                        start: CssGridPlacement::Line(area.column_start), end: CssGridPlacement::Line(area.column_end),
                    });
                }
            }
            if tag_name.eq_ignore_ascii_case("img") {
                taffy_style.item_is_replaced = true;
                if let Some(&(intrinsic_width, intrinsic_height)) = intrinsic.get(&node_id) {
                    if intrinsic_width > 0.0 && intrinsic_height > 0.0 && style.aspect_ratio.is_none() {
                        taffy_style.aspect_ratio = Some(intrinsic_width / intrinsic_height);
                        match (style.width, style.height) {
                            (CssLength::Auto, CssLength::Auto) => {
                                taffy_style.size.width = Dimension::length(intrinsic_width);
                                taffy_style.size.height = Dimension::length(intrinsic_height);
                            }
                            (CssLength::Auto, _) => {}
                            (_, CssLength::Auto) => {}
                            _ => {}
                        }
                    }
                }
            }
            let taffy_id = taffy
                .new_with_children(taffy_style, &children)
                .map_err(|err| NexusError::Layout(err.to_string()))?;
            built.push(BuiltNode { dom_id: node_id, taffy_id, parent_dom_id, depth });
            Ok(Some(taffy_id))
        }
        DomNodeData::Text(text) => {
            let white_space = inherited_style.map_or(CssWhiteSpace::Normal, |style| style.white_space);
            let normalized = match white_space {
                CssWhiteSpace::Normal | CssWhiteSpace::NoWrap => text.split_whitespace().collect::<Vec<_>>().join(" "),
                CssWhiteSpace::Pre | CssWhiteSpace::PreWrap => text.to_owned(),
            };
            if normalized.is_empty() {
                return Ok(None);
            }
            let font_size = inherited_style.map_or(16.0, |style| style.font_size);
            let line_height = inherited_style.map_or(font_size * 1.25, |style| style.line_height.used_px(font_size));
            let chars = normalized.chars().count() as f32;
            let weight_factor = inherited_style.map_or(1.0, |style| if style.font_weight >= 600 { 1.04 } else { 1.0 });
            let width = (chars * font_size * 0.55 * weight_factor).clamp(font_size * 0.5, viewport.width.max(1.0));
            let explicit_lines = if matches!(white_space, CssWhiteSpace::Pre | CssWhiteSpace::PreWrap) {
                normalized.lines().count().max(1) as f32
            } else { 1.0 };
            let height = line_height * explicit_lines;
            let leaf_style = Style {
                display: Display::Block,
                size: Size { width: Dimension::length(width), height: Dimension::length(height) },
                ..Default::default()
            };
            let taffy_id = taffy
                .new_leaf(leaf_style)
                .map_err(|err| NexusError::Layout(err.to_string()))?;
            built.push(BuiltNode { dom_id: node_id, taffy_id, parent_dom_id, depth });
            Ok(Some(taffy_id))
        }
        DomNodeData::Doctype { .. }
        | DomNodeData::Comment(_)
        | DomNodeData::ProcessingInstruction { .. } => Ok(None),
    }
}

fn to_taffy_style(style: &ComputedStyle) -> Style {
    let fallback_columns;
    let column_tracks = if style.grid_template_columns.is_empty() {
        fallback_columns = style.grid_template_areas.as_ref()
            .map(|template| vec![CssGridTrack::Auto; template.column_count]).unwrap_or_default();
        fallback_columns.as_slice()
    } else { style.grid_template_columns.as_slice() };
    let fallback_rows;
    let row_tracks = if style.grid_template_rows.is_empty() {
        fallback_rows = style.grid_template_areas.as_ref()
            .map(|template| vec![CssGridTrack::Auto; template.rows.len()]).unwrap_or_default();
        fallback_rows.as_slice()
    } else { style.grid_template_rows.as_slice() };
    Style {
        display: match style.display {
            CssDisplay::None => Display::None,
            CssDisplay::Flex => Display::Flex,
            CssDisplay::Grid => Display::Grid,
            CssDisplay::Block | CssDisplay::Inline => Display::Block,
        },
        box_sizing: match style.box_sizing { CssBoxSizing::ContentBox => BoxSizing::ContentBox, CssBoxSizing::BorderBox => BoxSizing::BorderBox },
        aspect_ratio: style.aspect_ratio,
        size: Size { width: to_dimension(style.width), height: to_dimension(style.height) },
        min_size: Size { width: to_dimension(style.min_width), height: to_dimension(style.min_height) },
        max_size: Size { width: to_dimension(style.max_width), height: to_dimension(style.max_height) },
        margin: Rect {
            top: to_length_percentage_auto(style.margin.top),
            right: to_length_percentage_auto(style.margin.right),
            bottom: to_length_percentage_auto(style.margin.bottom),
            left: to_length_percentage_auto(style.margin.left),
        },
        padding: Rect {
            top: to_length_percentage(style.padding.top),
            right: to_length_percentage(style.padding.right),
            bottom: to_length_percentage(style.padding.bottom),
            left: to_length_percentage(style.padding.left),
        },
        border: Rect {
            top: to_length_percentage(style.border_width.top),
            right: to_length_percentage(style.border_width.right),
            bottom: to_length_percentage(style.border_width.bottom),
            left: to_length_percentage(style.border_width.left),
        },
        align_items: to_item_alignment(style.align_items),
        justify_items: to_item_alignment(style.justify_items),
        align_self: to_item_alignment(style.align_self),
        justify_self: to_item_alignment(style.justify_self),
        align_content: to_content_alignment(style.align_content),
        justify_content: to_content_alignment(style.justify_content),
        gap: Size { width: to_length_percentage(style.column_gap), height: to_length_percentage(style.row_gap) },
        flex_direction: match style.flex_direction {
            CssFlexDirection::Row => FlexDirection::Row,
            CssFlexDirection::RowReverse => FlexDirection::RowReverse,
            CssFlexDirection::Column => FlexDirection::Column,
            CssFlexDirection::ColumnReverse => FlexDirection::ColumnReverse,
        },
        flex_wrap: match style.flex_wrap { CssFlexWrap::NoWrap => FlexWrap::NoWrap, CssFlexWrap::Wrap => FlexWrap::Wrap, CssFlexWrap::WrapReverse => FlexWrap::WrapReverse },
        flex_basis: to_dimension(style.flex_basis),
        flex_grow: style.flex_grow,
        flex_shrink: style.flex_shrink,
        grid_template_columns: column_tracks.iter().map(|track| match track {
            CssGridTrack::Auto => grid_auto(), CssGridTrack::Px(v) => grid_length(*v),
            CssGridTrack::Percent(v) => grid_percent(*v), CssGridTrack::Fr(v) => grid_fr(*v),
            CssGridTrack::MinContent => min_content(), CssGridTrack::MaxContent => max_content(),
            CssGridTrack::MinMax { min, max } => minmax(to_grid_min(*min), to_grid_max(*max)),
            CssGridTrack::FitContent(limit) => fit_content(to_length_percentage(*limit)),
            CssGridTrack::AutoRepeat { mode, tracks } => grid_repeat(to_repetition_count(*mode), tracks.iter().map(to_auto_track).collect()),
        }).collect(),
        grid_template_rows: row_tracks.iter().map(|track| match track {
            CssGridTrack::Auto => grid_auto(), CssGridTrack::Px(v) => grid_length(*v),
            CssGridTrack::Percent(v) => grid_percent(*v), CssGridTrack::Fr(v) => grid_fr(*v),
            CssGridTrack::MinContent => min_content(), CssGridTrack::MaxContent => max_content(),
            CssGridTrack::MinMax { min, max } => minmax(to_grid_min(*min), to_grid_max(*max)),
            CssGridTrack::FitContent(limit) => fit_content(to_length_percentage(*limit)),
            CssGridTrack::AutoRepeat { mode, tracks } => grid_repeat(to_repetition_count(*mode), tracks.iter().map(to_auto_track).collect()),
        }).collect(),
        grid_auto_columns: style.grid_auto_columns.iter().map(|track| match track {
            CssGridTrack::Auto => grid_auto(), CssGridTrack::Px(v) => grid_length(*v),
            CssGridTrack::Percent(v) => grid_percent(*v), CssGridTrack::Fr(v) => grid_fr(*v),
            CssGridTrack::MinContent => min_content(), CssGridTrack::MaxContent => max_content(),
            CssGridTrack::MinMax { min, max } => minmax(to_grid_min(*min), to_grid_max(*max)),
            CssGridTrack::FitContent(limit) => fit_content(to_length_percentage(*limit)),
            CssGridTrack::AutoRepeat { .. } => grid_auto(),
        }).collect(),
        grid_auto_rows: style.grid_auto_rows.iter().map(|track| match track {
            CssGridTrack::Auto => grid_auto(), CssGridTrack::Px(v) => grid_length(*v),
            CssGridTrack::Percent(v) => grid_percent(*v), CssGridTrack::Fr(v) => grid_fr(*v),
            CssGridTrack::MinContent => min_content(), CssGridTrack::MaxContent => max_content(),
            CssGridTrack::MinMax { min, max } => minmax(to_grid_min(*min), to_grid_max(*max)),
            CssGridTrack::FitContent(limit) => fit_content(to_length_percentage(*limit)),
            CssGridTrack::AutoRepeat { .. } => grid_auto(),
        }).collect(),
        grid_auto_flow: match style.grid_auto_flow {
            CssGridAutoFlow::Row => GridAutoFlow::Row,
            CssGridAutoFlow::Column => GridAutoFlow::Column,
            CssGridAutoFlow::RowDense => GridAutoFlow::RowDense,
            CssGridAutoFlow::ColumnDense => GridAutoFlow::ColumnDense,
        },
        grid_column: to_grid_line(style.grid_column),
        grid_row: to_grid_line(style.grid_row),
        position: match style.position {
            CssPosition::Absolute | CssPosition::Fixed => Position::Absolute,
            CssPosition::Static | CssPosition::Relative | CssPosition::Sticky => Position::Relative,
        },
        inset: if style.position == CssPosition::Static {
            Rect {
                top: LengthPercentageAuto::auto(),
                right: LengthPercentageAuto::auto(),
                bottom: LengthPercentageAuto::auto(),
                left: LengthPercentageAuto::auto(),
            }
        } else {
            Rect {
                top: to_length_percentage_auto(style.inset.top),
                right: to_length_percentage_auto(style.inset.right),
                bottom: to_length_percentage_auto(style.inset.bottom),
                left: to_length_percentage_auto(style.inset.left),
            }
        },
        overflow: Point {
            x: to_taffy_overflow(style.overflow_x),
            y: to_taffy_overflow(style.overflow_y),
        },
        ..Default::default()
    }
}

fn to_item_alignment(value: CssItemAlignment) -> Option<AlignItems> {
    match value {
        CssItemAlignment::Auto | CssItemAlignment::Normal => None,
        CssItemAlignment::Start => Some(AlignItems::START), CssItemAlignment::End => Some(AlignItems::END),
        CssItemAlignment::FlexStart => Some(AlignItems::FLEX_START), CssItemAlignment::FlexEnd => Some(AlignItems::FLEX_END),
        CssItemAlignment::Center => Some(AlignItems::CENTER), CssItemAlignment::Baseline => Some(AlignItems::BASELINE),
        CssItemAlignment::Stretch => Some(AlignItems::STRETCH),
    }
}

fn to_content_alignment(value: CssContentAlignment) -> Option<AlignContent> {
    match value {
        CssContentAlignment::Normal => None,
        CssContentAlignment::Start => Some(AlignContent::START), CssContentAlignment::End => Some(AlignContent::END),
        CssContentAlignment::FlexStart => Some(AlignContent::FLEX_START), CssContentAlignment::FlexEnd => Some(AlignContent::FLEX_END),
        CssContentAlignment::Center => Some(AlignContent::CENTER), CssContentAlignment::Stretch => Some(AlignContent::STRETCH),
        CssContentAlignment::SpaceBetween => Some(AlignContent::SPACE_BETWEEN),
        CssContentAlignment::SpaceAround => Some(AlignContent::SPACE_AROUND),
        CssContentAlignment::SpaceEvenly => Some(AlignContent::SPACE_EVENLY),
    }
}

fn to_grid_min(breadth: CssGridBreadth) -> MinTrackSizingFunction {
    match breadth {
        CssGridBreadth::Auto => grid_auto(),
        CssGridBreadth::Px(value) => grid_length(value),
        CssGridBreadth::Percent(value) => grid_percent(value),
        CssGridBreadth::Fr(_) => grid_auto(),
        CssGridBreadth::MinContent => min_content(),
        CssGridBreadth::MaxContent => max_content(),
    }
}

fn to_repetition_count(value: CssGridRepeat) -> RepetitionCount {
    match value { CssGridRepeat::AutoFill => RepetitionCount::AutoFill, CssGridRepeat::AutoFit => RepetitionCount::AutoFit }
}

fn to_auto_track(track: &CssGridTrack) -> TrackSizingFunction {
    match track {
        CssGridTrack::Auto => grid_auto(), CssGridTrack::Px(v) => grid_length(*v),
        CssGridTrack::Percent(v) => grid_percent(*v), CssGridTrack::Fr(v) => grid_fr(*v),
        CssGridTrack::MinContent => min_content(), CssGridTrack::MaxContent => max_content(),
        CssGridTrack::MinMax { min, max } => minmax(to_grid_min(*min), to_grid_max(*max)),
        CssGridTrack::FitContent(limit) => fit_content(to_length_percentage(*limit)),
        CssGridTrack::AutoRepeat { .. } => grid_auto(),
    }
}

fn to_grid_max(breadth: CssGridBreadth) -> MaxTrackSizingFunction {
    match breadth {
        CssGridBreadth::Auto => grid_auto(), CssGridBreadth::Px(value) => grid_length(value),
        CssGridBreadth::Percent(value) => grid_percent(value), CssGridBreadth::Fr(value) => grid_fr(value),
        CssGridBreadth::MinContent => min_content(), CssGridBreadth::MaxContent => max_content(),
    }
}

fn to_grid_line(value: CssGridLine) -> Line<taffy::style::GridPlacement> {
    Line { start: to_grid_placement(value.start), end: to_grid_placement(value.end) }
}

fn to_grid_placement(value: CssGridPlacement) -> taffy::style::GridPlacement {
    match value {
        CssGridPlacement::Auto => grid_auto(),
        CssGridPlacement::Line(value) => grid_line(value),
        CssGridPlacement::Span(value) => grid_span(value),
    }
}

fn to_taffy_overflow(value: CssOverflow) -> Overflow {
    match value {
        CssOverflow::Visible => Overflow::Visible,
        CssOverflow::Hidden => Overflow::Hidden,
        CssOverflow::Clip => Overflow::Clip,
        CssOverflow::Scroll | CssOverflow::Auto => Overflow::Scroll,
    }
}

fn to_dimension(value: CssLength) -> Dimension {
    match value {
        CssLength::Auto => Dimension::auto(),
        CssLength::Px(value) => Dimension::length(value),
        CssLength::Percent(value) => Dimension::percent(value),
    }
}

fn to_length_percentage_auto(value: CssLength) -> LengthPercentageAuto {
    match value {
        CssLength::Auto => LengthPercentageAuto::auto(),
        CssLength::Px(value) => LengthPercentageAuto::length(value),
        CssLength::Percent(value) => LengthPercentageAuto::percent(value),
    }
}

fn to_length_percentage(value: CssLength) -> LengthPercentage {
    match value {
        CssLength::Auto => LengthPercentage::length(0.0),
        CssLength::Px(value) => LengthPercentage::length(value),
        CssLength::Percent(value) => LengthPercentage::percent(value),
    }
}

fn absolute_position(
    dom_id: NodeId,
    built: &[BuiltNode],
    index_by_dom: &HashMap<NodeId, usize>,
    taffy: &TaffyTree<()>,
    cache: &mut HashMap<NodeId, (f32, f32)>,
) -> Result<(f32, f32), ()> {
    if let Some(position) = cache.get(&dom_id) {
        return Ok(*position);
    }
    let index = *index_by_dom.get(&dom_id).ok_or(())?;
    let item = &built[index];
    let layout = taffy.layout(item.taffy_id).map_err(|_| ())?;
    let (px, py) = match item.parent_dom_id {
        Some(parent) => absolute_position(parent, built, index_by_dom, taffy, cache)?,
        None => (0.0, 0.0),
    };
    let position = (px + layout.location.x, py + layout.location.y);
    cache.insert(dom_id, position);
    Ok(position)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::compute_styles;
    use crate::parser::parse_html;
    use url::Url;

    #[test]
    fn taffy_computes_css_box_size() {
        let dom = parse_html(
            Url::parse("https://nexus.local/").unwrap(),
            r#"<style>#box { width: 320px; height: 90px; padding: 10px; }</style><div id="box">Nexus</div>"#,
        );
        let styles = compute_styles(&dom);
        let layout = compute_layout(&dom, &styles, Viewport { width: 800.0, height: 600.0 }).unwrap();
        let div = dom.find_first_element("div").unwrap();
        let box_ = layout.box_for(div).unwrap();
        assert!(box_.width >= 320.0);
        assert!(box_.height >= 90.0);
    }
}
