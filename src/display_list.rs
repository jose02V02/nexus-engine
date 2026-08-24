//! Nexus-owned display list for Engine 0.84.
//!
//! 0.70 adds visibility and fitted replaced-content geometry to the
//! renderer-neutral paint boundary.

use std::sync::Arc;

use crate::css::{CssFontStyle, CssLength, CssObjectFit, CssOverflow, CssPosition, CssTextAlign, CssTextDecoration, CssTextTransform, CssTransform, CssVisibility, CssWhiteSpace, EdgeSizes, PseudoElement, Rgba, StyleMap};
use crate::compositing::paint_order_indices;
use crate::dom::{DomNodeData, NexusDom, NodeId};
use crate::layout::LayoutTree;
use crate::resource::PageResources;
use crate::text::{TextLayoutEngine, TextLayoutOptions};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaintRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PaintRect {
    #[must_use]
    pub fn translated_y(self, delta: f32) -> Self {
        Self { y: self.y + delta, ..self }
    }

    #[must_use]
    pub fn intersects_viewport(self, width: f32, height: f32) -> bool {
        self.x + self.width >= 0.0
            && self.x <= width
            && self.y + self.height >= 0.0
            && self.y <= height
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageAsset {
    pub node_id: NodeId,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DisplayCommand {
    Clear {
        color: Rgba,
    },
    PushClipRect {
        rect: PaintRect,
    },
    PushClipRoundedRect {
        rect: PaintRect,
        radius: f32,
    },
    PopClip,
    FillRect {
        node_id: NodeId,
        rect: PaintRect,
        color: Rgba,
    },
    FillRoundedRect {
        node_id: NodeId,
        rect: PaintRect,
        radius: f32,
        color: Rgba,
    },
    StrokeRoundedRect {
        node_id: NodeId,
        rect: PaintRect,
        radius: f32,
        width: f32,
        color: Rgba,
    },
    DrawImage {
        node_id: NodeId,
        asset_index: usize,
        rect: PaintRect,
    },
    DrawText {
        node_id: NodeId,
        x: f32,
        baseline: f32,
        text: String,
        font_size: f32,
        font_family: String,
        font_weight: u16,
        font_style: CssFontStyle,
        decoration: CssTextDecoration,
        color: Rgba,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayList {
    pub width: f32,
    pub height: f32,
    pub content_height: f32,
    pub scroll_y: f32,
    pub commands: Vec<DisplayCommand>,
    pub images: Vec<ImageAsset>,
}

impl DisplayList {
    #[must_use]
    pub fn pretty(&self) -> String {
        let mut out = format!(
            "viewport: {:.0}x{:.0} content-height={:.0} scroll-y={:.0} commands={} images={}\n",
            self.width,
            self.height,
            self.content_height,
            self.scroll_y,
            self.commands.len(),
            self.images.len()
        );
        for (index, command) in self.commands.iter().enumerate() {
            match command {
                DisplayCommand::Clear { color } => {
                    out.push_str(&format!("{index:>4}: CLEAR {color:?}\n"));
                }
                DisplayCommand::PushClipRect { rect } => {
                    out.push_str(&format!("{index:>4}: CLIP+ rect={rect:?}\n"));
                }
                DisplayCommand::PushClipRoundedRect { rect, radius } => {
                    out.push_str(&format!("{index:>4}: CLIP+ rounded radius={radius:.1} rect={rect:?}\n"));
                }
                DisplayCommand::PopClip => out.push_str(&format!("{index:>4}: CLIP-\n")),
                DisplayCommand::FillRect { node_id, rect, color } => {
                    out.push_str(&format!("{index:>4}: FILL node=#{node_id} rect={rect:?} {color:?}\n"));
                }
                DisplayCommand::FillRoundedRect { node_id, rect, radius, color } => {
                    out.push_str(&format!("{index:>4}: ROUND-FILL node=#{node_id} r={radius:.1} rect={rect:?} {color:?}\n"));
                }
                DisplayCommand::StrokeRoundedRect { node_id, rect, radius, width, color } => {
                    out.push_str(&format!("{index:>4}: ROUND-STROKE node=#{node_id} r={radius:.1} w={width:.1} rect={rect:?} {color:?}\n"));
                }
                DisplayCommand::DrawImage { node_id, asset_index, rect } => {
                    out.push_str(&format!("{index:>4}: IMAGE node=#{node_id} asset={asset_index} rect={rect:?}\n"));
                }
                DisplayCommand::DrawText { node_id, x, baseline, text, font_size, font_family, font_weight, font_style, decoration, color } => {
                    out.push_str(&format!(
                        "{index:>4}: TEXT node=#{node_id} x={x:.1} baseline={baseline:.1} size={font_size:.1}px weight={font_weight} style={font_style:?} decoration={decoration:?} family={font_family:?} {color:?} {:?}\n",
                        truncate(text, 60)
                    ));
                }
            }
        }
        out
    }
}

pub fn build_display_list(
    dom: &NexusDom,
    styles: &StyleMap,
    layout: &LayoutTree,
    text_engine: &mut dyn TextLayoutEngine,
) -> DisplayList {
    let resources = PageResources::default();
    build_display_list_with_resources(dom, styles, layout, &resources, text_engine, 0.0)
}

pub fn build_display_list_with_resources(
    dom: &NexusDom,
    styles: &StyleMap,
    layout: &LayoutTree,
    resources: &PageResources,
    text_engine: &mut dyn TextLayoutEngine,
    requested_scroll_y: f32,
) -> DisplayList {
    let content_height = layout
        .boxes
        .iter()
        .filter(|item| effective_position(dom, styles, item.node_id) != CssPosition::Fixed)
        .map(|item| item.y + item.height)
        .fold(layout.viewport.height, f32::max)
        .ceil()
        .max(1.0);
    let max_scroll = (content_height - layout.viewport.height).max(0.0);
    let scroll_y = requested_scroll_y.clamp(0.0, max_scroll);

    let mut commands = vec![
        DisplayCommand::Clear { color: Rgba::rgb(255, 255, 255) },
        DisplayCommand::PushClipRect {
            rect: PaintRect { x: 0.0, y: 0.0, width: layout.viewport.width, height: layout.viewport.height },
        },
    ];
    let mut images = Vec::new();

    // Nexus 0.20 introduces an explicit paint-order pass. It is not yet a full
    // CSS stacking-context implementation, but positioned elements with z-index
    // are painted deterministically above/below normal-flow content.
    let paint_order = paint_order_indices(dom, styles, layout);

    for index in paint_order {
        let item = &layout.boxes[index];
        let Some(node) = dom.node(item.node_id) else { continue };
        let Some(rect) = visual_rect_for_node(dom, styles, layout, item.node_id, scroll_y) else { continue };
        if !rect.intersects_viewport(layout.viewport.width, layout.viewport.height) {
            continue;
        }

        let clip_count = push_ancestor_clips(&mut commands, dom, styles, layout, item.node_id, scroll_y);

        match &node.data {
            DomNodeData::Element { tag_name, .. } => {
                let Some(style) = styles.get(item.node_id) else { pop_clips(&mut commands, clip_count); continue };
                if style.visibility != CssVisibility::Visible {
                    pop_clips(&mut commands, clip_count);
                    continue;
                }
                let radius = radius_px(style.border_radius, rect);
                let background = with_opacity(style.background_color, style.opacity);
                let border_color = with_opacity(style.border_color, style.opacity);

                if background.a > 0 && rect.width > 0.0 && rect.height > 0.0 {
                    if radius > 0.0 {
                        commands.push(DisplayCommand::FillRoundedRect { node_id: item.node_id, rect, radius, color: background });
                    } else {
                        commands.push(DisplayCommand::FillRect { node_id: item.node_id, rect, color: background });
                    }
                }

                emit_pseudo(
                    &mut commands, dom, styles, text_engine, item.node_id, PseudoElement::Before,
                    rect, true,
                );

                if tag_name.eq_ignore_ascii_case("img") {
                    if let Some(image) = resources.images.get(&item.node_id) {
                        let asset_index = images.len();
                        images.push(ImageAsset {
                            node_id: item.node_id,
                            width: image.width,
                            height: image.height,
                            rgba: image.rgba.clone(),
                        });
                        let image_rect = object_fit_rect(rect, image.width, image.height, style.object_fit, style.object_position.x, style.object_position.y);
                        let needs_clip = radius > 0.0 || image_rect != rect;
                        if needs_clip {
                            if radius > 0.0 { commands.push(DisplayCommand::PushClipRoundedRect { rect, radius }); }
                            else { commands.push(DisplayCommand::PushClipRect { rect }); }
                        }
                        commands.push(DisplayCommand::DrawImage { node_id: item.node_id, asset_index, rect: image_rect });
                        if needs_clip { commands.push(DisplayCommand::PopClip); }
                    }
                }

                emit_border_commands(
                    &mut commands,
                    item.node_id,
                    rect,
                    style.border_width,
                    border_color,
                    radius,
                );

                emit_pseudo(
                    &mut commands, dom, styles, text_engine, item.node_id, PseudoElement::After,
                    rect, false,
                );
            }
            DomNodeData::Text(raw) => {
                let style = inherited_text_style(dom, styles, item.node_id);
                if style.is_some_and(|value| value.visibility != CssVisibility::Visible) {
                    pop_clips(&mut commands, clip_count);
                    continue;
                }
                let white_space = style.map_or(CssWhiteSpace::Normal, |value| value.white_space);
                let text_transform = style.map_or(CssTextTransform::None, |value| value.text_transform);
                let text = transform_text(prepare_text(raw, white_space), text_transform);
                if !text.is_empty() && item.width > 0.0 {
                    let transform = effective_transform(dom, styles, item.node_id);
                    let font_size = style.map_or(16.0, |value| value.font_size) * transform.scale_y.abs().max(0.01);
                    let font_family = style.map_or_else(|| "sans-serif".to_owned(), |value| value.font_family.clone());
                    let font_weight = style.map_or(400, |value| value.font_weight);
                    let font_style = style.map_or(CssFontStyle::Normal, |value| value.font_style);
                    let decoration = style.map_or(CssTextDecoration::default(), |value| value.text_decoration);
                    let text_align = style.map_or(CssTextAlign::Start, |value| value.text_align);
                    let used_line_height = style.map_or(font_size * 1.25, |value| value.line_height.used_px(font_size));
                    let text_indent = style.map_or(0.0, |value| text_indent_px(value.text_indent, rect.width));
                    let color = style.map_or(Rgba::BLACK, |value| with_opacity(value.color, value.opacity));
                    let options = text_layout_options(white_space);
                    let shaped = text_engine.layout_text_with_options(&text, font_size, rect.width.max(font_size), options);
                    for (line_index, line) in shaped.lines.into_iter().enumerate() {
                        let indent = if line_index == 0 { text_indent } else { 0.0 };
                        let x = rect.x + line.x + indent + text_align_offset(text_align, (rect.width - indent).max(0.0), line.width);
                        let baseline = rect.y + line.baseline + line_index as f32 * (used_line_height - line.line_height);
                        if baseline < -font_size || baseline > layout.viewport.height + font_size { continue; }
                        commands.push(DisplayCommand::DrawText {
                            node_id: item.node_id,
                            x,
                            baseline,
                            text: line.text,
                            font_size,
                            font_family: font_family.clone(),
                            font_weight,
                            font_style,
                            decoration,
                            color,
                        });
                        emit_text_decorations(&mut commands, item.node_id, x, baseline, line.width, font_size, decoration, color);
                    }
                }
            }
            _ => {}
        }

        pop_clips(&mut commands, clip_count);
    }

    commands.push(DisplayCommand::PopClip);
    DisplayList {
        width: layout.viewport.width.ceil().max(1.0),
        height: layout.viewport.height.ceil().max(1.0),
        content_height,
        scroll_y,
        commands,
        images,
    }
}

pub(crate) fn visual_rect_for_node(
    dom: &NexusDom,
    styles: &StyleMap,
    layout: &LayoutTree,
    node_id: NodeId,
    scroll_y: f32,
) -> Option<PaintRect> {
    let item = layout.box_for(node_id)?;
    let style = styles.get(node_id).or_else(|| inherited_text_style(dom, styles, node_id));
    Some(paint_rect_for(
        item,
        style,
        effective_position(dom, styles, node_id),
        scroll_y,
        layout.viewport.height,
        effective_transform(dom, styles, node_id),
    ))
}

pub(crate) fn point_visible_through_overflow(
    dom: &NexusDom,
    styles: &StyleMap,
    layout: &LayoutTree,
    node_id: NodeId,
    scroll_y: f32,
    viewport_x: f32,
    viewport_y: f32,
) -> bool {
    let mut current = dom.node(node_id).and_then(|node| node.parent);
    while let Some(id) = current {
        if let Some(style) = styles.get(id) {
            if style.overflow_x != CssOverflow::Visible || style.overflow_y != CssOverflow::Visible {
                let Some(rect) = visual_rect_for_node(dom, styles, layout, id, scroll_y) else { return false };
                if viewport_x < rect.x || viewport_x > rect.x + rect.width
                    || viewport_y < rect.y || viewport_y > rect.y + rect.height
                {
                    return false;
                }
            }
        }
        current = dom.node(id).and_then(|node| node.parent);
    }
    true
}

fn paint_rect_for(
    item: &crate::layout::LayoutBox,
    style: Option<&crate::css::ComputedStyle>,
    effective_position: CssPosition,
    scroll_y: f32,
    viewport_height: f32,
    transform: CssTransform,
) -> PaintRect {
    let mut y = item.y - scroll_y;
    if let Some(style) = style {
        match effective_position {
            CssPosition::Fixed => y = item.y,
            CssPosition::Sticky => {
                let top = length_px(style.inset.top).unwrap_or(0.0);
                y = y.max(top);
                if let Some(bottom) = length_px(style.inset.bottom) {
                    y = y.min((viewport_height - bottom - item.height).max(top));
                }
            }
            _ => {}
        }
    }
    transform_rect(
        PaintRect { x: item.x, y, width: item.width, height: item.height },
        transform,
    )
}

fn transform_rect(mut rect: PaintRect, transform: CssTransform) -> PaintRect {
    let cx = rect.x + rect.width * 0.5;
    let cy = rect.y + rect.height * 0.5;
    rect.width = (rect.width * transform.scale_x.abs()).max(0.0);
    rect.height = (rect.height * transform.scale_y.abs()).max(0.0);
    rect.x = cx - rect.width * 0.5 + transform.translate_x;
    rect.y = cy - rect.height * 0.5 + transform.translate_y;
    rect
}

fn effective_position(dom: &NexusDom, styles: &StyleMap, node_id: NodeId) -> CssPosition {
    let own = styles.get(node_id).map_or(CssPosition::Static, |style| style.position);
    let mut current = Some(node_id);
    while let Some(id) = current {
        if let Some(style) = styles.get(id) {
            if style.position == CssPosition::Fixed { return CssPosition::Fixed; }
            if style.position == CssPosition::Sticky { return CssPosition::Sticky; }
        }
        current = dom.node(id).and_then(|node| node.parent);
    }
    own
}

fn effective_transform(dom: &NexusDom, styles: &StyleMap, node_id: NodeId) -> CssTransform {
    let mut chain = Vec::new();
    let mut current = Some(node_id);
    while let Some(id) = current {
        if let Some(style) = styles.get(id) { chain.push(style.transform); }
        current = dom.node(id).and_then(|node| node.parent);
    }
    chain.reverse();
    chain.into_iter().fold(CssTransform::default(), |mut acc, t| {
        acc.translate_x += t.translate_x;
        acc.translate_y += t.translate_y;
        acc.scale_x *= t.scale_x;
        acc.scale_y *= t.scale_y;
        acc
    })
}

fn push_ancestor_clips(
    commands: &mut Vec<DisplayCommand>,
    dom: &NexusDom,
    styles: &StyleMap,
    layout: &LayoutTree,
    node_id: NodeId,
    scroll_y: f32,
) -> usize {
    let mut ancestors = Vec::new();
    let mut current = dom.node(node_id).and_then(|node| node.parent);
    while let Some(id) = current {
        ancestors.push(id);
        current = dom.node(id).and_then(|node| node.parent);
    }
    ancestors.reverse();
    let mut count = 0usize;
    for id in ancestors {
        let Some(style) = styles.get(id) else { continue };
        if style.overflow_x == CssOverflow::Visible && style.overflow_y == CssOverflow::Visible { continue; }
        let Some(rect) = visual_rect_for_node(dom, styles, layout, id, scroll_y) else { continue };
        let radius = radius_px(style.border_radius, rect);
        if radius > 0.0 { commands.push(DisplayCommand::PushClipRoundedRect { rect, radius }); }
        else { commands.push(DisplayCommand::PushClipRect { rect }); }
        count += 1;
    }
    count
}

fn pop_clips(commands: &mut Vec<DisplayCommand>, count: usize) {
    for _ in 0..count { commands.push(DisplayCommand::PopClip); }
}

fn emit_pseudo(
    commands: &mut Vec<DisplayCommand>,
    _dom: &NexusDom,
    styles: &StyleMap,
    text_engine: &mut dyn TextLayoutEngine,
    node_id: NodeId,
    pseudo: PseudoElement,
    rect: PaintRect,
    before: bool,
) {
    let Some(style) = styles.pseudo(node_id, pseudo) else { return };
    if style.content.is_empty() || rect.width <= 0.0 { return; }
    let shaped = text_engine.layout_text(&style.content, style.font_size, rect.width.max(style.font_size));
    let line_height = style.font_size * 1.25;
    for (index, line) in shaped.lines.into_iter().enumerate() {
        let y = if before {
            rect.y + line.baseline + index as f32 * line_height
        } else {
            (rect.y + rect.height - line_height).max(rect.y) + line.baseline + index as f32 * line_height
        };
        if style.background_color.a > 0 {
            commands.push(DisplayCommand::FillRect {
                node_id,
                rect: PaintRect { x: rect.x, y: y - style.font_size, width: rect.width, height: line_height },
                color: style.background_color,
            });
        }
        commands.push(DisplayCommand::DrawText {
            node_id,
            x: rect.x + line.x,
            baseline: y,
            text: line.text,
            font_size: style.font_size,
            font_family: "sans-serif".to_owned(),
            font_weight: 400,
            font_style: CssFontStyle::Normal,
            decoration: CssTextDecoration::default(),
            color: style.color,
        });
    }
}

fn text_align_offset(alignment: CssTextAlign, container_width: f32, line_width: f32) -> f32 {
    let remaining = (container_width - line_width).max(0.0);
    match alignment {
        CssTextAlign::Center => remaining * 0.5,
        CssTextAlign::End | CssTextAlign::Right => remaining,
        CssTextAlign::Start | CssTextAlign::Left | CssTextAlign::Justify => 0.0,
    }
}

fn emit_text_decorations(
    commands: &mut Vec<DisplayCommand>, node_id: NodeId, x: f32, baseline: f32,
    width: f32, font_size: f32, decoration: CssTextDecoration, color: Rgba,
) {
    let thickness = (font_size * 0.065).max(1.0);
    let mut line = |y: f32| commands.push(DisplayCommand::FillRect {
        node_id, rect: PaintRect { x, y, width, height: thickness }, color,
    });
    if decoration.overline { line(baseline - font_size); }
    if decoration.line_through { line(baseline - font_size * 0.32); }
    if decoration.underline { line(baseline + thickness); }
}

fn with_opacity(mut color: Rgba, opacity: f32) -> Rgba {
    color.a = ((color.a as f32) * opacity.clamp(0.0, 1.0)).round().clamp(0.0, 255.0) as u8;
    color
}

fn length_px(value: CssLength) -> Option<f32> {
    match value { CssLength::Px(v) => Some(v), _ => None }
}

fn text_indent_px(value: CssLength, containing_width: f32) -> f32 {
    match value {
        CssLength::Px(value) => value,
        CssLength::Percent(value) => containing_width * value,
        _ => 0.0,
    }
}

fn object_fit_rect(box_rect: PaintRect, image_width: u32, image_height: u32, fit: CssObjectFit, position_x: f32, position_y: f32) -> PaintRect {
    let intrinsic_width = image_width.max(1) as f32;
    let intrinsic_height = image_height.max(1) as f32;
    if fit == CssObjectFit::Fill { return box_rect; }
    let contain_scale = (box_rect.width / intrinsic_width).min(box_rect.height / intrinsic_height);
    let scale = match fit {
        CssObjectFit::Contain => contain_scale,
        CssObjectFit::Cover => (box_rect.width / intrinsic_width).max(box_rect.height / intrinsic_height),
        CssObjectFit::None => 1.0,
        CssObjectFit::ScaleDown => contain_scale.min(1.0),
        CssObjectFit::Fill => return box_rect,
    };
    let width = intrinsic_width * scale;
    let height = intrinsic_height * scale;
    PaintRect {
        x: box_rect.x + (box_rect.width - width) * position_x,
        y: box_rect.y + (box_rect.height - height) * position_y,
        width,
        height,
    }
}

fn emit_border_commands(
    commands: &mut Vec<DisplayCommand>,
    node_id: NodeId,
    rect: PaintRect,
    widths: EdgeSizes<CssLength>,
    color: Rgba,
    radius: f32,
) {
    let top = border_px(widths.top);
    let right = border_px(widths.right);
    let bottom = border_px(widths.bottom);
    let left = border_px(widths.left);

    if radius > 0.0 && top > 0.0 && approximately_equal(top, right) && approximately_equal(top, bottom) && approximately_equal(top, left) {
        commands.push(DisplayCommand::StrokeRoundedRect { node_id, rect, radius, width: top, color });
        return;
    }

    if top > 0.0 {
        commands.push(DisplayCommand::FillRect {
            node_id,
            rect: PaintRect { x: rect.x, y: rect.y, width: rect.width, height: top.min(rect.height) },
            color,
        });
    }
    if right > 0.0 {
        commands.push(DisplayCommand::FillRect {
            node_id,
            rect: PaintRect {
                x: (rect.x + rect.width - right).max(rect.x),
                y: rect.y,
                width: right.min(rect.width),
                height: rect.height,
            },
            color,
        });
    }
    if bottom > 0.0 {
        commands.push(DisplayCommand::FillRect {
            node_id,
            rect: PaintRect {
                x: rect.x,
                y: (rect.y + rect.height - bottom).max(rect.y),
                width: rect.width,
                height: bottom.min(rect.height),
            },
            color,
        });
    }
    if left > 0.0 {
        commands.push(DisplayCommand::FillRect {
            node_id,
            rect: PaintRect { x: rect.x, y: rect.y, width: left.min(rect.width), height: rect.height },
            color,
        });
    }
}

fn border_px(value: CssLength) -> f32 {
    match value {
        CssLength::Px(value) => value.max(0.0),
        CssLength::Auto | CssLength::Percent(_) => 0.0,
    }
}

fn radius_px(value: CssLength, rect: PaintRect) -> f32 {
    let max_radius = (rect.width.min(rect.height) * 0.5).max(0.0);
    match value {
        CssLength::Px(value) => value.max(0.0).min(max_radius),
        CssLength::Percent(value) => (rect.width.min(rect.height) * value).max(0.0).min(max_radius),
        CssLength::Auto => 0.0,
    }
}

fn approximately_equal(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

fn inherited_text_style<'a>(
    dom: &'a NexusDom,
    styles: &'a StyleMap,
    node_id: NodeId,
) -> Option<&'a crate::css::ComputedStyle> {
    let mut current = dom.node(node_id)?.parent;
    while let Some(id) = current {
        if let Some(style) = styles.get(id) {
            return Some(style);
        }
        current = dom.node(id).and_then(|node| node.parent);
    }
    None
}

fn normalize_text(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn prepare_text(input: &str, white_space: CssWhiteSpace) -> String {
    match white_space {
        CssWhiteSpace::Normal | CssWhiteSpace::NoWrap => normalize_text(input),
        CssWhiteSpace::Pre | CssWhiteSpace::PreWrap => input.to_owned(),
    }
}

fn text_layout_options(white_space: CssWhiteSpace) -> TextLayoutOptions {
    match white_space {
        CssWhiteSpace::Normal => TextLayoutOptions { collapse_whitespace: true, wrap: true },
        CssWhiteSpace::NoWrap => TextLayoutOptions { collapse_whitespace: true, wrap: false },
        CssWhiteSpace::Pre => TextLayoutOptions { collapse_whitespace: false, wrap: false },
        CssWhiteSpace::PreWrap => TextLayoutOptions { collapse_whitespace: false, wrap: true },
    }
}

fn transform_text(input: String, transform: CssTextTransform) -> String {
    match transform {
        CssTextTransform::None => input,
        CssTextTransform::Uppercase => input.to_uppercase(),
        CssTextTransform::Lowercase => input.to_lowercase(),
        CssTextTransform::Capitalize => input.split_inclusive(char::is_whitespace).map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| first.to_uppercase().collect::<String>() + chars.as_str())
        }).collect::<Vec<_>>().join(""),
    }
}

fn truncate(input: &str, max_chars: usize) -> String {
    let mut chars = input.chars();
    let mut result: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        result.push('…');
    }
    result
}
