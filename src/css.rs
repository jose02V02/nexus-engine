//! Nexus CSS layer.
//!
//! Nexus Engine deliberately owns its computed-style representation. The
//! parser/tokenizer is `servo/rust-cssparser`; the cascade in this version is a
//! compact Nexus implementation supporting the subset needed to feed Taffy.
//! This boundary is intentionally compatible with replacing the cascade with a
//! Stylo adapter later without changing the layout or renderer APIs.

use std::collections::HashMap;

use cssparser::{Parser, ParserInput, Token};

use crate::dom::{DomNodeData, NexusDom, NodeId};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssLength {
    Auto,
    Px(f32),
    Percent(f32),
}

impl Default for CssLength {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssDisplay {
    None,
    Block,
    Inline,
    Flex,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssFlexDirection {
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssFlexWrap { NoWrap, Wrap, WrapReverse }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssBoxSizing { ContentBox, BorderBox }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssFontStyle { Normal, Italic, Oblique }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssTextAlign { Start, End, Left, Right, Center, Justify }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssWhiteSpace { Normal, NoWrap, Pre, PreWrap }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssTextTransform { None, Uppercase, Lowercase, Capitalize }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssVisibility { Visible, Hidden, Collapse }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssPointerEvents { Auto, None }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssObjectFit { Fill, Contain, Cover, None, ScaleDown }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssObjectPosition { pub x: f32, pub y: f32 }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssLineHeight { Normal, Number(f32), Px(f32) }

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CssTextDecoration {
    pub underline: bool,
    pub overline: bool,
    pub line_through: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CssGridTrack {
    Auto,
    Px(f32),
    Percent(f32),
    Fr(f32),
    MinContent,
    MaxContent,
    MinMax { min: CssGridBreadth, max: CssGridBreadth },
    FitContent(CssLength),
    AutoRepeat { mode: CssGridRepeat, tracks: Vec<CssGridTrack> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssGridRepeat { AutoFill, AutoFit }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssGridBreadth { Auto, Px(f32), Percent(f32), Fr(f32), MinContent, MaxContent }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssGridPlacement { Auto, Line(i16), Span(u16) }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CssGridLine { pub start: CssGridPlacement, pub end: CssGridPlacement }

impl Default for CssGridLine {
    fn default() -> Self { Self { start: CssGridPlacement::Auto, end: CssGridPlacement::Auto } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CssNamedGridArea {
    pub row_start: i16,
    pub row_end: i16,
    pub column_start: i16,
    pub column_end: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssGridTemplateAreas {
    pub rows: Vec<Vec<Option<String>>>,
    pub column_count: usize,
    pub areas: HashMap<String, CssNamedGridArea>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssGridAutoFlow { Row, Column, RowDense, ColumnDense }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssItemAlignment { Auto, Normal, Start, End, FlexStart, FlexEnd, Center, Baseline, Stretch }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssContentAlignment { Normal, Start, End, FlexStart, FlexEnd, Center, Stretch, SpaceBetween, SpaceAround, SpaceEvenly }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssPosition {
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssOverflow {
    Visible,
    Hidden,
    Clip,
    Scroll,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssTransform {
    pub translate_x: f32,
    pub translate_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
}

impl Default for CssTransform {
    fn default() -> Self {
        Self { translate_x: 0.0, translate_y: 0.0, scale_x: 1.0, scale_y: 1.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaEnvironment {
    pub width: f32,
    pub height: f32,
}

impl Default for MediaEnvironment {
    fn default() -> Self { Self { width: 1280.0, height: 720.0 } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0 };

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeSizes<T: Copy> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T: Copy> EdgeSizes<T> {
    pub const fn all(value: T) -> Self {
        Self { top: value, right: value, bottom: value, left: value }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComputedStyle {
    pub custom_properties: HashMap<String, String>,
    pub display: CssDisplay,
    pub width: CssLength,
    pub height: CssLength,
    pub min_width: CssLength,
    pub min_height: CssLength,
    pub max_width: CssLength,
    pub max_height: CssLength,
    pub aspect_ratio: Option<f32>,
    pub box_sizing: CssBoxSizing,
    pub margin: EdgeSizes<CssLength>,
    pub padding: EdgeSizes<CssLength>,
    pub border_width: EdgeSizes<CssLength>,
    pub border_color: Rgba,
    pub border_radius: CssLength,
    pub gap: CssLength,
    pub row_gap: CssLength,
    pub column_gap: CssLength,
    pub flex_direction: CssFlexDirection,
    pub flex_wrap: CssFlexWrap,
    pub flex_basis: CssLength,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub order: i32,
    pub align_items: CssItemAlignment,
    pub justify_items: CssItemAlignment,
    pub align_self: CssItemAlignment,
    pub justify_self: CssItemAlignment,
    pub align_content: CssContentAlignment,
    pub justify_content: CssContentAlignment,
    pub grid_template_columns: Vec<CssGridTrack>,
    pub grid_template_rows: Vec<CssGridTrack>,
    pub grid_template_areas: Option<CssGridTemplateAreas>,
    pub grid_auto_columns: Vec<CssGridTrack>,
    pub grid_auto_rows: Vec<CssGridTrack>,
    pub grid_auto_flow: CssGridAutoFlow,
    pub grid_column: CssGridLine,
    pub grid_row: CssGridLine,
    pub grid_area_name: Option<String>,
    pub position: CssPosition,
    pub inset: EdgeSizes<CssLength>,
    pub z_index: Option<i32>,
    pub overflow_x: CssOverflow,
    pub overflow_y: CssOverflow,
    pub transform: CssTransform,
    pub opacity: f32,
    pub visibility: CssVisibility,
    pub pointer_events: CssPointerEvents,
    pub object_fit: CssObjectFit,
    pub object_position: CssObjectPosition,
    pub color: Rgba,
    pub background_color: Rgba,
    pub font_size: f32,
    pub font_family: String,
    pub font_weight: u16,
    pub font_style: CssFontStyle,
    pub line_height: CssLineHeight,
    pub text_align: CssTextAlign,
    pub white_space: CssWhiteSpace,
    pub text_transform: CssTextTransform,
    pub text_indent: CssLength,
    pub text_decoration: CssTextDecoration,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            custom_properties: HashMap::new(),
            display: CssDisplay::Inline,
            width: CssLength::Auto,
            height: CssLength::Auto,
            min_width: CssLength::Auto,
            min_height: CssLength::Auto,
            max_width: CssLength::Auto,
            max_height: CssLength::Auto,
            aspect_ratio: None,
            box_sizing: CssBoxSizing::ContentBox,
            margin: EdgeSizes::all(CssLength::Px(0.0)),
            padding: EdgeSizes::all(CssLength::Px(0.0)),
            border_width: EdgeSizes::all(CssLength::Px(0.0)),
            border_color: Rgba::BLACK,
            border_radius: CssLength::Px(0.0),
            gap: CssLength::Px(0.0),
            row_gap: CssLength::Px(0.0),
            column_gap: CssLength::Px(0.0),
            flex_direction: CssFlexDirection::Row,
            flex_wrap: CssFlexWrap::NoWrap,
            flex_basis: CssLength::Auto,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            order: 0,
            align_items: CssItemAlignment::Normal,
            justify_items: CssItemAlignment::Normal,
            align_self: CssItemAlignment::Auto,
            justify_self: CssItemAlignment::Auto,
            align_content: CssContentAlignment::Normal,
            justify_content: CssContentAlignment::Normal,
            grid_template_columns: Vec::new(),
            grid_template_rows: Vec::new(),
            grid_template_areas: None,
            grid_auto_columns: vec![CssGridTrack::Auto],
            grid_auto_rows: vec![CssGridTrack::Auto],
            grid_auto_flow: CssGridAutoFlow::Row,
            grid_column: CssGridLine::default(),
            grid_row: CssGridLine::default(),
            grid_area_name: None,
            position: CssPosition::Static,
            inset: EdgeSizes::all(CssLength::Auto),
            z_index: None,
            overflow_x: CssOverflow::Visible,
            overflow_y: CssOverflow::Visible,
            transform: CssTransform::default(),
            opacity: 1.0,
            visibility: CssVisibility::Visible,
            pointer_events: CssPointerEvents::Auto,
            object_fit: CssObjectFit::Fill,
            object_position: CssObjectPosition { x: 0.5, y: 0.5 },
            color: Rgba::BLACK,
            background_color: Rgba::TRANSPARENT,
            font_size: 16.0,
            font_family: "sans-serif".to_owned(),
            font_weight: 400,
            font_style: CssFontStyle::Normal,
            line_height: CssLineHeight::Normal,
            text_align: CssTextAlign::Start,
            white_space: CssWhiteSpace::Normal,
            text_transform: CssTextTransform::None,
            text_indent: CssLength::Px(0.0),
            text_decoration: CssTextDecoration::default(),
        }
    }
}

impl ComputedStyle {
    fn inherited(parent: Option<&Self>) -> Self {
        let mut style = Self::default();
        if let Some(parent) = parent {
            style.custom_properties.clone_from(&parent.custom_properties);
            style.color = parent.color;
            style.font_size = parent.font_size;
            style.font_family.clone_from(&parent.font_family);
            style.font_weight = parent.font_weight;
            style.font_style = parent.font_style;
            style.line_height = parent.line_height;
            style.text_align = parent.text_align;
            style.white_space = parent.white_space;
            style.text_transform = parent.text_transform;
            style.text_indent = parent.text_indent;
            style.visibility = parent.visibility;
            style.pointer_events = parent.pointer_events;
        }
        style
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PseudoElement { Before, After }

#[derive(Debug, Clone, PartialEq)]
pub struct PseudoStyle {
    pub content: String,
    pub color: Rgba,
    pub background_color: Rgba,
    pub font_size: f32,
}

#[derive(Debug, Clone, Default)]
pub struct StyleMap {
    styles: HashMap<NodeId, ComputedStyle>,
    pseudo: HashMap<(NodeId, PseudoElement), PseudoStyle>,
    pub author_rule_count: usize,
    pub parse_warnings: Vec<String>,
}

impl StyleMap {
    pub fn get(&self, node_id: NodeId) -> Option<&ComputedStyle> {
        self.styles.get(&node_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &ComputedStyle)> {
        self.styles.iter().map(|(&id, style)| (id, style))
    }

    pub fn pseudo(&self, node_id: NodeId, pseudo: PseudoElement) -> Option<&PseudoStyle> {
        self.pseudo.get(&(node_id, pseudo))
    }

    pub fn len(&self) -> usize {
        self.styles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }

    pub fn pretty(&self, dom: &NexusDom) -> String {
        let mut ids = self.styles.keys().copied().collect::<Vec<_>>();
        ids.sort_unstable();
        let mut out = String::new();
        for id in ids {
            let Some(style) = self.styles.get(&id) else { continue };
            let name = dom.node_label(id);
            out.push_str(&format!(
                "#{id:<4} {name:<24} display={:?} pos={:?} z={:?} size=({:?}, {:?}) overflow=({:?},{:?}) transform={:?} font={}px\n",
                style.display,
                style.position,
                style.z_index,
                style.width,
                style.height,
                style.overflow_x,
                style.overflow_y,
                style.transform,
                style.font_size
            ));
        }
        out
    }
}

#[derive(Debug, Clone)]
struct StyleRule {
    selectors: Vec<ComplexSelector>,
    declarations: Vec<Declaration>,
    source_order: usize,
    media: Option<MediaQuery>,
    pseudo: Option<PseudoElement>,
}

#[derive(Debug, Clone, Default)]
struct MediaQuery {
    min_width: Option<f32>,
    max_width: Option<f32>,
    min_height: Option<f32>,
    max_height: Option<f32>,
    landscape: Option<bool>,
}

impl MediaQuery {
    fn matches(&self, env: MediaEnvironment) -> bool {
        if self.min_width.is_some_and(|v| env.width < v) { return false; }
        if self.max_width.is_some_and(|v| env.width > v) { return false; }
        if self.min_height.is_some_and(|v| env.height < v) { return false; }
        if self.max_height.is_some_and(|v| env.height > v) { return false; }
        if let Some(landscape) = self.landscape {
            if (env.width >= env.height) != landscape { return false; }
        }
        true
    }
}

#[derive(Debug, Clone, Default)]
struct SimpleSelector {
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
    universal: bool,
    attributes: Vec<AttributeSelector>,
    pseudo_classes: Vec<PseudoClass>,
}

#[derive(Debug, Clone, Copy)]
enum AttributeOperator { Equals, Includes, DashMatch, Prefix, Suffix, Substring }

#[derive(Debug, Clone)]
enum AttributeSelector {
    Exists(String),
    Match { name: String, operator: AttributeOperator, value: String, case_insensitive: bool },
}

#[derive(Debug, Clone)]
enum PseudoClass {
    Root,
    FirstChild,
    LastChild,
    OnlyChild,
    FirstOfType,
    LastOfType,
    OnlyOfType,
    Empty,
    Disabled,
    Checked,
    Link,
    NthChild(i32, i32),
    NthLastChild(i32, i32),
    NthOfType(i32, i32),
    NthLastOfType(i32, i32),
    Not(Vec<SimpleSelector>),
    Is(Vec<SimpleSelector>),
    Where(Vec<SimpleSelector>),
}

#[derive(Debug, Clone, Copy)]
enum Combinator { Descendant, Child, AdjacentSibling, GeneralSibling }

#[derive(Debug, Clone)]
struct ComplexSelector {
    parts: Vec<(SimpleSelector, Option<Combinator>)>,
}

impl SimpleSelector {
    fn specificity(&self) -> u32 {
        let pseudo_specificity = self.pseudo_classes.iter().map(|pseudo| match pseudo {
            PseudoClass::Where(_) => 0,
            PseudoClass::Not(selectors) | PseudoClass::Is(selectors) => selectors
                .iter().map(SimpleSelector::specificity).max().unwrap_or(0),
            _ => 10,
        }).sum::<u32>();
        (self.id.is_some() as u32) * 100
            + u32::try_from(self.classes.len() + self.attributes.len()).unwrap_or(u32::MAX).saturating_mul(10)
            + pseudo_specificity
            + (self.tag.is_some() as u32)
    }

    fn matches(&self, dom: &NexusDom, node_id: NodeId) -> bool {
        let Some(tag) = dom.element_tag_name(node_id) else { return false };
        if let Some(expected) = &self.tag {
            if !tag.eq_ignore_ascii_case(expected) {
                return false;
            }
        }
        if let Some(expected) = &self.id {
            if dom.attribute(node_id, "id") != Some(expected.as_str()) {
                return false;
            }
        }
        if !self.classes.is_empty() {
            let actual = dom
                .attribute(node_id, "class")
                .unwrap_or("")
                .split_ascii_whitespace()
                .collect::<Vec<_>>();
            if self.classes.iter().any(|class| !actual.iter().any(|item| item == class)) {
                return false;
            }
        }
        for attribute in &self.attributes {
            match attribute {
                AttributeSelector::Exists(name) if dom.attribute(node_id, name).is_none() => return false,
                AttributeSelector::Match { name, operator, value, case_insensitive } => {
                    let Some(actual) = dom.attribute(node_id, name) else { return false };
                    let (actual, expected) = if *case_insensitive {
                        (actual.to_ascii_lowercase(), value.to_ascii_lowercase())
                    } else {
                        (actual.to_owned(), value.clone())
                    };
                    let matched = match operator {
                        AttributeOperator::Equals => actual == expected,
                        AttributeOperator::Includes => actual.split_ascii_whitespace().any(|part| part == expected.as_str()),
                        AttributeOperator::DashMatch => actual == expected || actual.strip_prefix(&expected).is_some_and(|rest| rest.starts_with('-')),
                        AttributeOperator::Prefix => actual.starts_with(&expected),
                        AttributeOperator::Suffix => actual.ends_with(&expected),
                        AttributeOperator::Substring => actual.contains(&expected),
                    };
                    if !matched { return false; }
                }
                _ => {}
            }
        }
        for pseudo in &self.pseudo_classes {
            if matches!(pseudo, PseudoClass::Root) {
                if !tag.eq_ignore_ascii_case("html") { return false; }
                continue;
            }
            if matches!(pseudo, PseudoClass::Empty) {
                let Some(node) = dom.node(node_id) else { return false };
                if node.children.iter().any(|child| match dom.node(*child).map(|n| &n.data) {
                    Some(DomNodeData::Element { .. }) => true,
                    Some(DomNodeData::Text(text)) => !text.is_empty(),
                    _ => false,
                }) { return false; }
                continue;
            }
            if matches!(pseudo, PseudoClass::Disabled) {
                if dom.attribute(node_id, "disabled").is_none() { return false; }
                continue;
            }
            if matches!(pseudo, PseudoClass::Checked) {
                if dom.attribute(node_id, "checked").is_none() { return false; }
                continue;
            }
            if matches!(pseudo, PseudoClass::Link) {
                if !tag.eq_ignore_ascii_case("a") || dom.attribute(node_id, "href").is_none() { return false; }
                continue;
            }
            match pseudo {
                PseudoClass::Not(selectors) if selectors.iter().any(|selector| selector.matches(dom, node_id)) => return false,
                PseudoClass::Is(selectors) | PseudoClass::Where(selectors)
                    if !selectors.iter().any(|selector| selector.matches(dom, node_id)) => return false,
                PseudoClass::Not(_) | PseudoClass::Is(_) | PseudoClass::Where(_) => continue,
                _ => {}
            }
            let Some(parent) = dom.parent_element(node_id) else { return false };
            let Some(parent_node) = dom.node(parent) else { return false };
            let siblings = parent_node.children.iter().copied()
                .filter(|id| dom.element_tag_name(*id).is_some()).collect::<Vec<_>>();
            let Some(index) = siblings.iter().position(|id| *id == node_id).map(|v| v + 1) else { return false };
            let typed = siblings.iter().copied().filter(|id| dom.element_tag_name(*id)
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(tag))).collect::<Vec<_>>();
            let Some(type_index) = typed.iter().position(|id| *id == node_id).map(|v| v + 1) else { return false };
            match pseudo {
                PseudoClass::FirstChild if index != 1 => return false,
                PseudoClass::LastChild if index != siblings.len() => return false,
                PseudoClass::OnlyChild if siblings.len() != 1 => return false,
                PseudoClass::FirstOfType if type_index != 1 => return false,
                PseudoClass::LastOfType if type_index != typed.len() => return false,
                PseudoClass::OnlyOfType if typed.len() != 1 => return false,
                PseudoClass::NthChild(a, b) if !nth_matches(*a, *b, index as i32) => return false,
                PseudoClass::NthLastChild(a, b) if !nth_matches(*a, *b, (siblings.len() - index + 1) as i32) => return false,
                PseudoClass::NthOfType(a, b) if !nth_matches(*a, *b, type_index as i32) => return false,
                PseudoClass::NthLastOfType(a, b) if !nth_matches(*a, *b, (typed.len() - type_index + 1) as i32) => return false,
                _ => {}
            }
        }
        self.universal || self.tag.is_some() || self.id.is_some() || !self.classes.is_empty()
            || !self.attributes.is_empty() || !self.pseudo_classes.is_empty()
    }
}

impl ComplexSelector {
    fn specificity(&self) -> u32 { self.parts.iter().map(|(part, _)| part.specificity()).sum() }

    fn matches(&self, dom: &NexusDom, node_id: NodeId) -> bool {
        self.matches_from(dom, node_id, self.parts.len().checked_sub(1))
    }

    fn matches_from(&self, dom: &NexusDom, node_id: NodeId, index: Option<usize>) -> bool {
        let Some(index) = index else { return true };
        let (simple, combinator) = &self.parts[index];
        if !simple.matches(dom, node_id) { return false; }
        if index == 0 { return true; }
        match combinator.unwrap_or(Combinator::Descendant) {
            Combinator::Child => dom.parent_element(node_id)
                .is_some_and(|parent| self.matches_from(dom, parent, Some(index - 1))),
            Combinator::Descendant => {
                let mut parent = dom.parent_element(node_id);
                while let Some(candidate) = parent {
                    if self.matches_from(dom, candidate, Some(index - 1)) { return true; }
                    parent = dom.parent_element(candidate);
                }
                false
            }
            Combinator::AdjacentSibling => previous_element_sibling(dom, node_id)
                .is_some_and(|sibling| self.matches_from(dom, sibling, Some(index - 1))),
            Combinator::GeneralSibling => {
                let Some(parent) = dom.parent_element(node_id) else { return false };
                let Some(parent_node) = dom.node(parent) else { return false };
                for sibling in &parent_node.children {
                    if *sibling == node_id { break; }
                    if dom.element_tag_name(*sibling).is_some()
                        && self.matches_from(dom, *sibling, Some(index - 1)) { return true; }
                }
                false
            }
        }
    }
}

fn previous_element_sibling(dom: &NexusDom, node_id: NodeId) -> Option<NodeId> {
    let parent = dom.parent_element(node_id)?;
    let children = &dom.node(parent)?.children;
    let index = children.iter().position(|id| *id == node_id)?;
    children[..index].iter().rev().copied().find(|id| dom.element_tag_name(*id).is_some())
}

fn nth_matches(a: i32, b: i32, index: i32) -> bool {
    if a == 0 { return index == b; }
    let delta = index - b;
    delta % a == 0 && delta / a >= 0
}

#[derive(Debug, Clone)]
struct Declaration {
    name: String,
    value: String,
    important: bool,
}

/// Computes Nexus-owned styles for every HTML element in the DOM.
pub fn compute_styles(dom: &NexusDom) -> StyleMap {
    compute_styles_for_viewport(dom, MediaEnvironment::default())
}

pub fn compute_styles_for_viewport(dom: &NexusDom, env: MediaEnvironment) -> StyleMap {
    let mut warnings = Vec::new();
    let mut rules = Vec::new();
    let mut source_order = 0usize;

    for css in dom.style_blocks() {
        rules.extend(parse_stylesheet(&css, &mut warnings, &mut source_order, None));
    }

    let author_rule_count = rules.len();
    let mut map = StyleMap {
        styles: HashMap::new(),
        pseudo: HashMap::new(),
        author_rule_count,
        parse_warnings: warnings,
    };

    compute_node_style(dom, dom.root(), None, &rules, env, &mut map);
    map
}

fn compute_node_style(
    dom: &NexusDom,
    node_id: NodeId,
    parent_style: Option<&ComputedStyle>,
    rules: &[StyleRule],
    env: MediaEnvironment,
    map: &mut StyleMap,
) {
    let Some(node) = dom.node(node_id) else { return };

    match &node.data {
        DomNodeData::Element { tag_name, .. } => {
            let mut style = ComputedStyle::inherited(parent_style);
            apply_user_agent_defaults(tag_name, &mut style);

            let mut matched = rules
                .iter()
                .filter(|rule| rule.pseudo.is_none() && rule.media.as_ref().is_none_or(|m| m.matches(env)))
                .filter_map(|rule| {
                    rule.selectors
                        .iter()
                        .filter(|selector| selector.matches(dom, node_id))
                        .map(ComplexSelector::specificity)
                        .max()
                        .map(|specificity| (specificity, rule.source_order, rule))
                })
                .collect::<Vec<_>>();
            matched.sort_by_key(|(specificity, order, _)| (*specificity, *order));

            let inline_declarations = dom.attribute(node_id, "style").map(parse_declarations).unwrap_or_default();
            for custom in [true, false] {
                for important in [false, true] {
                    for (_, _, rule) in &matched {
                        for declaration in &rule.declarations {
                            if declaration.name.starts_with("--") == custom && declaration.important == important {
                                apply_declaration_for_environment(&mut style, declaration, env, parent_style);
                            }
                        }
                    }
                    for declaration in &inline_declarations {
                        if declaration.name.starts_with("--") == custom && declaration.important == important {
                            apply_declaration_for_environment(&mut style, declaration, env, parent_style);
                        }
                    }
                }
            }

            for pseudo_kind in [PseudoElement::Before, PseudoElement::After] {
                let mut pseudo_style = PseudoStyle {
                    content: String::new(),
                    color: style.color,
                    background_color: Rgba::TRANSPARENT,
                    font_size: style.font_size,
                };
                let mut pseudo_matched = rules
                    .iter()
                    .filter(|rule| rule.pseudo == Some(pseudo_kind)
                        && rule.media.as_ref().is_none_or(|m| m.matches(env)))
                    .filter_map(|rule| {
                        rule.selectors
                            .iter()
                            .filter(|selector| selector.matches(dom, node_id))
                            .map(ComplexSelector::specificity)
                            .max()
                            .map(|specificity| (specificity, rule.source_order, rule))
                    })
                    .collect::<Vec<_>>();
                pseudo_matched.sort_by_key(|(specificity, order, _)| (*specificity, *order));
                for important in [false, true] {
                    for (_, _, rule) in &pseudo_matched {
                        for declaration in &rule.declarations {
                            if declaration.important != important || declaration.name.starts_with("--") { continue; }
                            if let Some(value) = resolve_css_value(
                                &declaration.value, &style.custom_properties, env, style.font_size, 0,
                            ) {
                                apply_pseudo_declaration(&mut pseudo_style, &Declaration {
                                    name: declaration.name.clone(), value, important,
                                });
                            }
                        }
                    }
                }
                if !pseudo_style.content.is_empty() {
                    map.pseudo.insert((node_id, pseudo_kind), pseudo_style);
                }
            }

            map.styles.insert(node_id, style.clone());
            for &child in &node.children {
                compute_node_style(dom, child, Some(&style), rules, env, map);
            }
        }
        _ => {
            for &child in &node.children {
                compute_node_style(dom, child, parent_style, rules, env, map);
            }
        }
    }
}

fn apply_user_agent_defaults(tag: &str, style: &mut ComputedStyle) {
    style.display = match tag.to_ascii_lowercase().as_str() {
        "html" | "body" | "div" | "main" | "section" | "article" | "header" | "footer"
        | "nav" | "aside" | "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
        | "ul" | "ol" | "li" | "form" | "pre" | "blockquote" | "figure" => CssDisplay::Block,
        "head" | "meta" | "link" | "style" | "script" | "title" | "template" => CssDisplay::None,
        _ => CssDisplay::Inline,
    };

    match tag.to_ascii_lowercase().as_str() {
        "body" => style.margin = EdgeSizes::all(CssLength::Px(8.0)),
        "h1" => { style.font_size = 32.0; style.font_weight = 700; style.margin = EdgeSizes { top: CssLength::Px(21.0), right: CssLength::Px(0.0), bottom: CssLength::Px(21.0), left: CssLength::Px(0.0) }; }
        "h2" => { style.font_size = 24.0; style.font_weight = 700; }
        "h3" => { style.font_size = 18.72; style.font_weight = 700; }
        "h4" | "h5" | "h6" | "b" | "strong" => style.font_weight = 700,
        "i" | "em" => style.font_style = CssFontStyle::Italic,
        "small" => style.font_size = 13.0,
        _ => {}
    }
}

fn parse_stylesheet(
    css: &str,
    warnings: &mut Vec<String>,
    source_order: &mut usize,
    inherited_media: Option<MediaQuery>,
) -> Vec<StyleRule> {
    let stripped = strip_comments(css);
    let mut rules = Vec::new();
    let mut cursor = 0usize;

    while cursor < stripped.len() {
        let rest = &stripped[cursor..];
        let Some(open_rel) = rest.find('{') else { break };
        let open = cursor + open_rel;
        let header = stripped[cursor..open].trim();
        let Some(close) = find_matching_brace(&stripped, open) else {
            warnings.push(format!("regola CSS senza '}}': {header}"));
            break;
        };
        let body = &stripped[open + 1..close];
        cursor = close + 1;

        if let Some(query_text) = header.strip_prefix("@media") {
            if let Some(mut query) = parse_media_query(query_text.trim()) {
                if let Some(parent) = &inherited_media {
                    query = merge_media_queries(parent, &query);
                }
                rules.extend(parse_stylesheet(body, warnings, source_order, Some(query)));
            } else {
                warnings.push(format!("media query non supportata: {header}"));
            }
            continue;
        }
        if let Some(condition) = header.strip_prefix("@supports") {
            match evaluate_supports_condition(condition.trim()) {
                Some(true) => rules.extend(parse_stylesheet(body, warnings, source_order, inherited_media.clone())),
                Some(false) => {}
                None => warnings.push(format!("condizione @supports non supportata: {header}")),
            }
            continue;
        }
        if header.starts_with('@') {
            // @font-face/keyframes are deferred to later compatibility milestones.
            continue;
        }

        let declarations = parse_declarations(body);
        let mut any = false;
        for raw in split_top_level(header, ',') {
            let (selector_text, pseudo) = split_pseudo_element(raw.trim());
            if let Some(selector) = parse_complex_selector(selector_text) {
                rules.push(StyleRule {
                    selectors: vec![selector],
                    declarations: declarations.clone(),
                    source_order: *source_order,
                    media: inherited_media.clone(),
                    pseudo,
                });
                *source_order = (*source_order).saturating_add(1);
                any = true;
            }
        }
        if !any {
            warnings.push(format!("selettore CSS non supportato: {header}"));
        }
    }

    rules
}

fn evaluate_supports_condition(input: &str) -> Option<bool> {
    let input = input.trim();
    if input.is_empty() { return None; }
    let or_parts = split_top_level_keyword(input, "or");
    if or_parts.len() > 1 {
        let values = or_parts.into_iter().map(evaluate_supports_condition).collect::<Option<Vec<_>>>()?;
        return Some(values.into_iter().any(|value| value));
    }
    let and_parts = split_top_level_keyword(input, "and");
    if and_parts.len() > 1 {
        let values = and_parts.into_iter().map(evaluate_supports_condition).collect::<Option<Vec<_>>>()?;
        return Some(values.into_iter().all(|value| value));
    }
    if let Some(rest) = input.strip_prefix("not ") {
        return evaluate_supports_condition(rest).map(|value| !value);
    }
    let atom = strip_balanced_outer_parentheses(input);
    if atom != input { return evaluate_supports_condition(atom); }
    if let Some(selector) = atom.strip_prefix("selector(").and_then(|value| value.strip_suffix(')')) {
        return Some(parse_complex_selector(selector.trim()).is_some());
    }
    let colon = find_top_level_char(atom, ':')?;
    let property = atom[..colon].trim().to_ascii_lowercase();
    let value = atom[colon + 1..].trim();
    Some(supports_declaration(&property, value))
}

fn split_top_level_keyword<'a>(input: &'a str, keyword: &str) -> Vec<&'a str> {
    let bytes = input.as_bytes();
    let needle = format!(" {keyword} ");
    let needle = needle.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut round = 0usize;
    let mut square = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => round += 1,
            b')' => round = round.saturating_sub(1),
            b'[' => square += 1,
            b']' => square = square.saturating_sub(1),
            _ => {}
        }
        if round == 0 && square == 0 && bytes[i..].starts_with(needle) {
            parts.push(input[start..i].trim());
            i += needle.len();
            start = i;
            continue;
        }
        i += 1;
    }
    parts.push(input[start..].trim());
    parts
}

fn strip_balanced_outer_parentheses(input: &str) -> &str {
    let input = input.trim();
    if !input.starts_with('(') || !input.ends_with(')') { return input; }
    let mut depth = 0usize;
    for (index, ch) in input.char_indices() {
        if ch == '(' { depth += 1; }
        if ch == ')' {
            depth = depth.saturating_sub(1);
            if depth == 0 && index + ch.len_utf8() != input.len() { return input; }
        }
    }
    &input[1..input.len() - 1]
}

fn supports_declaration(property: &str, value: &str) -> bool {
    if property.starts_with("--") { return !value.is_empty(); }
    match property {
        "display" => parse_display(value).is_some(),
        "position" => parse_position(value).is_some(),
        "overflow" | "overflow-x" | "overflow-y" => parse_overflow(value).is_some(),
        "z-index" => value.eq_ignore_ascii_case("auto") || parse_z_index(value).is_some(),
        "transform" => parse_transform(value).is_some(),
        "opacity" | "flex-grow" | "flex-shrink" => parse_number(value).is_some(),
        "visibility" => parse_visibility(value).is_some(),
        "pointer-events" => parse_pointer_events(value).is_some(),
        "object-fit" => parse_object_fit(value).is_some(),
        "object-position" => parse_object_position(value).is_some(),
        "color" | "background" | "background-color" | "border-color" => parse_color(value).is_some() || value.eq_ignore_ascii_case("currentcolor"),
        "flex-direction" => parse_flex_direction(value).is_some(),
        "flex-wrap" => parse_flex_wrap(value).is_some(),
        "flex-flow" => parse_flex_flow(value).is_some(),
        "flex" => parse_flex(value).is_some(),
        "flex-basis" => parse_length(value).is_some(),
        "order" => value.trim().parse::<i32>().is_ok(),
        "aspect-ratio" => parse_aspect_ratio(value).is_some(),
        "box-sizing" => parse_box_sizing(value).is_some(),
        "grid-template-columns" | "grid-template-rows" => parse_grid_track_list(value).is_some(),
        "grid-template-areas" => parse_grid_template_areas(value).is_some(),
        "grid-auto-columns" | "grid-auto-rows" => parse_grid_auto_track_list(value).is_some(),
        "grid-auto-flow" => parse_grid_auto_flow(value).is_some(),
        "grid-column" | "grid-row" => parse_grid_line(value).is_some(),
        "grid-area" => parse_grid_area_name(value).is_some(),
        "grid-column-start" | "grid-column-end" | "grid-row-start" | "grid-row-end" => parse_grid_placement(value).is_some(),
        "align-items" | "justify-items" | "align-self" | "justify-self" => parse_item_alignment(value).is_some(),
        "align-content" | "justify-content" => parse_content_alignment(value).is_some(),
        "place-items" | "place-self" => parse_place_items(value).is_some(),
        "place-content" => parse_place_content(value).is_some(),
        "font-family" => !value.trim().is_empty(),
        "font-weight" => parse_font_weight(value).is_some(),
        "font-style" => parse_font_style(value).is_some(),
        "line-height" => parse_line_height(value).is_some(),
        "text-align" => parse_text_align(value).is_some(),
        "white-space" => parse_white_space(value).is_some(),
        "text-transform" => parse_text_transform(value).is_some(),
        "text-indent" => resolved_supports_length(value).and_then(|resolved| parse_length(&resolved)).is_some(),
        "text-decoration" | "text-decoration-line" => parse_text_decoration(value).is_some(),
        "font" => parse_font_shorthand(value).is_some(),
        "width" | "height" | "min-width" | "min-height" | "max-width" | "max-height"
        | "margin-top" | "margin-right" | "margin-bottom" | "margin-left"
        | "padding-top" | "padding-right" | "padding-bottom" | "padding-left"
        | "border-top-width" | "border-right-width" | "border-bottom-width"
        | "border-left-width" | "border-radius" | "gap" | "row-gap" | "column-gap"
        | "top" | "right" | "bottom" | "left"
        | "font-size" => resolved_supports_length(value).and_then(|resolved| parse_length(&resolved)).is_some(),
        "margin" => resolved_supports_length(value).and_then(|resolved| parse_edges(&resolved, true)).is_some(),
        "padding" | "border-width" => resolved_supports_length(value).and_then(|resolved| parse_edges(&resolved, false)).is_some(),
        "border" => !value.trim().is_empty(),
        _ => false,
    }
}

fn resolved_supports_length(value: &str) -> Option<String> {
    resolve_css_value(value, &HashMap::new(), MediaEnvironment::default(), 16.0, 0)
}

fn split_pseudo_element(input: &str) -> (&str, Option<PseudoElement>) {
    let input = input.trim();
    if let Some(base) = input.strip_suffix("::before").or_else(|| input.strip_suffix(":before")) {
        return (base.trim(), Some(PseudoElement::Before));
    }
    if let Some(base) = input.strip_suffix("::after").or_else(|| input.strip_suffix(":after")) {
        return (base.trim(), Some(PseudoElement::After));
    }
    (input, None)
}

fn parse_media_query(input: &str) -> Option<MediaQuery> {
    let normalized = input.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.contains(',') || normalized.contains(" not ") {
        return None;
    }
    let mut query = MediaQuery::default();
    let mut recognized = false;
    for raw in normalized.split(" and ") {
        let part = raw.trim().trim_matches(|c| c == '(' || c == ')').trim();
        if part.is_empty() || part == "screen" || part == "all" {
            continue;
        }
        if let Some(value) = part.strip_prefix("min-width:").and_then(parse_media_px) {
            query.min_width = Some(value); recognized = true; continue;
        }
        if let Some(value) = part.strip_prefix("max-width:").and_then(parse_media_px) {
            query.max_width = Some(value); recognized = true; continue;
        }
        if let Some(value) = part.strip_prefix("min-height:").and_then(parse_media_px) {
            query.min_height = Some(value); recognized = true; continue;
        }
        if let Some(value) = part.strip_prefix("max-height:").and_then(parse_media_px) {
            query.max_height = Some(value); recognized = true; continue;
        }
        if part == "orientation: landscape" { query.landscape = Some(true); recognized = true; continue; }
        if part == "orientation: portrait" { query.landscape = Some(false); recognized = true; continue; }
        return None;
    }
    recognized.then_some(query)
}

fn parse_media_px(input: &str) -> Option<f32> {
    let value = input.trim();
    value.strip_suffix("px")?.trim().parse::<f32>().ok().filter(|v| v.is_finite() && *v >= 0.0)
}

fn merge_media_queries(a: &MediaQuery, b: &MediaQuery) -> MediaQuery {
    MediaQuery {
        min_width: match (a.min_width, b.min_width) { (Some(x), Some(y)) => Some(x.max(y)), (x, y) => x.or(y) },
        max_width: match (a.max_width, b.max_width) { (Some(x), Some(y)) => Some(x.min(y)), (x, y) => x.or(y) },
        min_height: match (a.min_height, b.min_height) { (Some(x), Some(y)) => Some(x.max(y)), (x, y) => x.or(y) },
        max_height: match (a.max_height, b.max_height) { (Some(x), Some(y)) => Some(x.min(y)), (x, y) => x.or(y) },
        landscape: b.landscape.or(a.landscape),
    }
}

fn parse_complex_selector(input: &str) -> Option<ComplexSelector> {
    let mut parts = Vec::new();
    let mut buffer = String::new();
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut pending = None;
    let flush = |buffer: &mut String, pending: &mut Option<Combinator>, parts: &mut Vec<(SimpleSelector, Option<Combinator>)>| -> Option<()> {
        let text = buffer.trim();
        if text.is_empty() { return Some(()); }
        let selector = parse_compound_selector(text)?;
        let relation = if parts.is_empty() { None } else { Some(pending.take().unwrap_or(Combinator::Descendant)) };
        parts.push((selector, relation));
        buffer.clear();
        Some(())
    };
    for ch in input.trim().chars() {
        match ch {
            '[' => { bracket_depth += 1; buffer.push(ch); }
            ']' => { bracket_depth = bracket_depth.saturating_sub(1); buffer.push(ch); }
            '(' => { paren_depth += 1; buffer.push(ch); }
            ')' => { paren_depth = paren_depth.saturating_sub(1); buffer.push(ch); }
            '>' if bracket_depth == 0 && paren_depth == 0 => {
                flush(&mut buffer, &mut pending, &mut parts)?;
                pending = Some(Combinator::Child);
            }
            '+' if bracket_depth == 0 && paren_depth == 0 => {
                flush(&mut buffer, &mut pending, &mut parts)?;
                pending = Some(Combinator::AdjacentSibling);
            }
            '~' if bracket_depth == 0 && paren_depth == 0 => {
                flush(&mut buffer, &mut pending, &mut parts)?;
                pending = Some(Combinator::GeneralSibling);
            }
            c if c.is_whitespace() && bracket_depth == 0 && paren_depth == 0 => {
                flush(&mut buffer, &mut pending, &mut parts)?;
                if pending.is_none() { pending = Some(Combinator::Descendant); }
            }
            _ => buffer.push(ch),
        }
    }
    flush(&mut buffer, &mut pending, &mut parts)?;
    (!parts.is_empty()).then_some(ComplexSelector { parts })
}

fn parse_compound_selector(input: &str) -> Option<SimpleSelector> {
    if input.is_empty() { return None; }
    let mut selector = SimpleSelector::default();
    if input == "*" {
        selector.universal = true;
        return Some(selector);
    }

    let chars = input.chars().collect::<Vec<_>>();
    let mut i = 0usize;
    let mut tag = String::new();
    while i < chars.len() && !matches!(chars[i], '.' | '#' | '[' | ':') {
        tag.push(chars[i]);
        i += 1;
    }
    if !tag.is_empty() && tag != "*" {
        selector.tag = Some(tag.to_ascii_lowercase());
    } else if tag == "*" {
        selector.universal = true;
    }

    while i < chars.len() {
        let marker = chars[i];
        i += 1;
        if marker == '[' {
            let start = i;
            while i < chars.len() && chars[i] != ']' { i += 1; }
            if i >= chars.len() { return None; }
            let expression = chars[start..i].iter().collect::<String>();
            i += 1;
            selector.attributes.push(parse_attribute_selector(&expression)?);
            continue;
        }
        if marker == ':' {
            let start = i;
            let mut depth = 0usize;
            while i < chars.len() {
                match chars[i] {
                    '(' => depth += 1,
                    ')' => depth = depth.saturating_sub(1),
                    '.' | '#' | '[' | ':' if depth == 0 => break,
                    _ => {}
                }
                i += 1;
            }
            let raw = chars[start..i].iter().collect::<String>().to_ascii_lowercase();
            if raw == "root" { selector.pseudo_classes.push(PseudoClass::Root); }
            else if raw == "first-child" { selector.pseudo_classes.push(PseudoClass::FirstChild); }
            else if raw == "last-child" { selector.pseudo_classes.push(PseudoClass::LastChild); }
            else if raw == "only-child" { selector.pseudo_classes.push(PseudoClass::OnlyChild); }
            else if raw == "first-of-type" { selector.pseudo_classes.push(PseudoClass::FirstOfType); }
            else if raw == "last-of-type" { selector.pseudo_classes.push(PseudoClass::LastOfType); }
            else if raw == "only-of-type" { selector.pseudo_classes.push(PseudoClass::OnlyOfType); }
            else if raw == "empty" { selector.pseudo_classes.push(PseudoClass::Empty); }
            else if raw == "disabled" { selector.pseudo_classes.push(PseudoClass::Disabled); }
            else if raw == "checked" { selector.pseudo_classes.push(PseudoClass::Checked); }
            else if raw == "link" || raw == "any-link" { selector.pseudo_classes.push(PseudoClass::Link); }
            else if let Some(expr) = raw.strip_prefix("nth-child(").and_then(|v| v.strip_suffix(')')) {
                selector.pseudo_classes.push(parse_nth(expr)?);
            } else if let Some(expr) = raw.strip_prefix("nth-last-child(").and_then(|v| v.strip_suffix(')')) {
                selector.pseudo_classes.push(rewrite_nth(parse_nth(expr)?, |a, b| PseudoClass::NthLastChild(a, b))?);
            } else if let Some(expr) = raw.strip_prefix("nth-of-type(").and_then(|v| v.strip_suffix(')')) {
                selector.pseudo_classes.push(rewrite_nth(parse_nth(expr)?, |a, b| PseudoClass::NthOfType(a, b))?);
            } else if let Some(expr) = raw.strip_prefix("nth-last-of-type(").and_then(|v| v.strip_suffix(')')) {
                selector.pseudo_classes.push(rewrite_nth(parse_nth(expr)?, |a, b| PseudoClass::NthLastOfType(a, b))?);
            } else if let Some(expr) = raw.strip_prefix("not(").and_then(|v| v.strip_suffix(')')) {
                selector.pseudo_classes.push(PseudoClass::Not(parse_selector_list(expr)?));
            } else if let Some(expr) = raw.strip_prefix("is(").and_then(|v| v.strip_suffix(')')) {
                selector.pseudo_classes.push(PseudoClass::Is(parse_selector_list(expr)?));
            } else if let Some(expr) = raw.strip_prefix("where(").and_then(|v| v.strip_suffix(')')) {
                selector.pseudo_classes.push(PseudoClass::Where(parse_selector_list(expr)?));
            } else { return None; }
            continue;
        }
        let start = i;
        while i < chars.len() && !matches!(chars[i], '.' | '#' | '[' | ':') {
            i += 1;
        }
        if start == i {
            return None;
        }
        let value = chars[start..i].iter().collect::<String>();
        match marker {
            '#' => selector.id = Some(value),
            '.' => selector.classes.push(value),
            _ => return None,
        }
    }

    Some(selector)
}

fn parse_attribute_selector(input: &str) -> Option<AttributeSelector> {
    let input = input.trim();
    for (symbol, operator) in [
        ("~=", AttributeOperator::Includes), ("|=", AttributeOperator::DashMatch),
        ("^=", AttributeOperator::Prefix), ("$=", AttributeOperator::Suffix),
        ("*=", AttributeOperator::Substring), ("=", AttributeOperator::Equals),
    ] {
        if let Some((name, raw_value)) = input.split_once(symbol) {
            let mut raw_value = raw_value.trim();
            let case_insensitive = raw_value.ends_with(" i") || raw_value.ends_with(" I");
            if case_insensitive { raw_value = raw_value[..raw_value.len() - 2].trim_end(); }
            let value = raw_value.trim_matches(|c| c == '"' || c == '\'').to_owned();
            let name = name.trim().to_ascii_lowercase();
            if name.is_empty() || value.is_empty() { return None; }
            return Some(AttributeSelector::Match { name, operator, value, case_insensitive });
        }
    }
    (!input.is_empty()).then(|| AttributeSelector::Exists(input.to_ascii_lowercase()))
}

fn parse_selector_list(input: &str) -> Option<Vec<SimpleSelector>> {
    let selectors = input.split(',').map(str::trim).map(parse_compound_selector)
        .collect::<Option<Vec<_>>>()?;
    (!selectors.is_empty()).then_some(selectors)
}

fn rewrite_nth<F>(pseudo: PseudoClass, constructor: F) -> Option<PseudoClass>
where F: FnOnce(i32, i32) -> PseudoClass {
    if let PseudoClass::NthChild(a, b) = pseudo { Some(constructor(a, b)) } else { None }
}

fn parse_nth(input: &str) -> Option<PseudoClass> {
    let value = input.trim().replace(' ', "").to_ascii_lowercase();
    if value == "odd" { return Some(PseudoClass::NthChild(2, 1)); }
    if value == "even" { return Some(PseudoClass::NthChild(2, 0)); }
    if let Some(n) = value.find('n') {
        let a = match &value[..n] { "" | "+" => 1, "-" => -1, v => v.parse().ok()? };
        let b = if n + 1 == value.len() { 0 } else { value[n + 1..].parse().ok()? };
        Some(PseudoClass::NthChild(a, b))
    } else {
        Some(PseudoClass::NthChild(0, value.parse().ok()?))
    }
}

fn parse_declarations(input: &str) -> Vec<Declaration> {
    split_top_level(input, ';')
        .into_iter()
        .filter_map(|part| {
            let colon = find_top_level_char(part, ':')?;
            let name = part[..colon].trim().to_ascii_lowercase();
            let mut value = part[colon + 1..].trim().to_owned();
            let important = value.to_ascii_lowercase().strip_suffix("!important").is_some();
            if important { value.truncate(value.len() - "!important".len()); value = value.trim().to_owned(); }
            (!name.is_empty() && !value.is_empty()).then_some(Declaration { name, value, important })
        })
        .collect()
}

fn apply_declaration_for_environment(
    style: &mut ComputedStyle,
    declaration: &Declaration,
    env: MediaEnvironment,
    parent: Option<&ComputedStyle>,
) {
    if declaration.name.starts_with("--") {
        match declaration.value.trim().to_ascii_lowercase().as_str() {
            "initial" => { style.custom_properties.remove(&declaration.name); }
            "inherit" | "unset" => {
                if let Some(value) = parent.and_then(|p| p.custom_properties.get(&declaration.name)) {
                    style.custom_properties.insert(declaration.name.clone(), value.clone());
                } else {
                    style.custom_properties.remove(&declaration.name);
                }
            }
            _ => { style.custom_properties.insert(declaration.name.clone(), declaration.value.clone()); }
        }
        return;
    }
    let keyword = declaration.value.trim().to_ascii_lowercase();
    if matches!(keyword.as_str(), "inherit" | "initial" | "unset") {
        let inherits = matches!(declaration.name.as_str(), "color" | "font-size" | "font-family");
        let source = if keyword == "inherit" || (keyword == "unset" && inherits) {
            parent.cloned().unwrap_or_default()
        } else {
            ComputedStyle::default()
        };
        apply_global_property(style, &source, &declaration.name);
        return;
    }
    let Some(value) = resolve_css_value(&declaration.value, &style.custom_properties, env, style.font_size, 0) else { return };
    apply_declaration(style, &Declaration { name: declaration.name.clone(), value, important: declaration.important });
}

fn apply_global_property(style: &mut ComputedStyle, source: &ComputedStyle, name: &str) {
    match name {
        "display" => style.display = source.display,
        "width" => style.width = source.width, "height" => style.height = source.height,
        "min-width" => style.min_width = source.min_width, "min-height" => style.min_height = source.min_height,
        "max-width" => style.max_width = source.max_width, "max-height" => style.max_height = source.max_height,
        "aspect-ratio" => style.aspect_ratio = source.aspect_ratio, "box-sizing" => style.box_sizing = source.box_sizing,
        "margin" => style.margin = source.margin, "margin-top" => style.margin.top = source.margin.top,
        "margin-right" => style.margin.right = source.margin.right, "margin-bottom" => style.margin.bottom = source.margin.bottom,
        "margin-left" => style.margin.left = source.margin.left,
        "padding" => style.padding = source.padding, "padding-top" => style.padding.top = source.padding.top,
        "padding-right" => style.padding.right = source.padding.right, "padding-bottom" => style.padding.bottom = source.padding.bottom,
        "padding-left" => style.padding.left = source.padding.left,
        "border" => { style.border_width = source.border_width; style.border_color = source.border_color; }
        "border-width" => style.border_width = source.border_width,
        "border-top-width" => style.border_width.top = source.border_width.top,
        "border-right-width" => style.border_width.right = source.border_width.right,
        "border-bottom-width" => style.border_width.bottom = source.border_width.bottom,
        "border-left-width" => style.border_width.left = source.border_width.left,
        "border-color" => style.border_color = source.border_color,
        "border-radius" => style.border_radius = source.border_radius,
        "gap" => { style.gap = source.gap; style.row_gap = source.row_gap; style.column_gap = source.column_gap; }
        "row-gap" => style.row_gap = source.row_gap, "column-gap" => style.column_gap = source.column_gap,
        "flex-direction" => style.flex_direction = source.flex_direction,
        "flex-wrap" => style.flex_wrap = source.flex_wrap, "flex-basis" => style.flex_basis = source.flex_basis,
        "flex-grow" => style.flex_grow = source.flex_grow, "flex-shrink" => style.flex_shrink = source.flex_shrink,
        "order" => style.order = source.order,
        "flex" => { style.flex_grow = source.flex_grow; style.flex_shrink = source.flex_shrink; style.flex_basis = source.flex_basis; }
        "flex-flow" => { style.flex_direction = source.flex_direction; style.flex_wrap = source.flex_wrap; }
        "align-items" => style.align_items = source.align_items, "justify-items" => style.justify_items = source.justify_items,
        "align-self" => style.align_self = source.align_self, "justify-self" => style.justify_self = source.justify_self,
        "align-content" => style.align_content = source.align_content, "justify-content" => style.justify_content = source.justify_content,
        "place-items" => { style.align_items = source.align_items; style.justify_items = source.justify_items; }
        "place-self" => { style.align_self = source.align_self; style.justify_self = source.justify_self; }
        "place-content" => { style.align_content = source.align_content; style.justify_content = source.justify_content; }
        "grid-template-columns" => style.grid_template_columns.clone_from(&source.grid_template_columns),
        "grid-template-rows" => style.grid_template_rows.clone_from(&source.grid_template_rows),
        "grid-template-areas" => style.grid_template_areas.clone_from(&source.grid_template_areas),
        "grid-auto-columns" => style.grid_auto_columns.clone_from(&source.grid_auto_columns),
        "grid-auto-rows" => style.grid_auto_rows.clone_from(&source.grid_auto_rows),
        "grid-auto-flow" => style.grid_auto_flow = source.grid_auto_flow,
        "grid-column" => style.grid_column = source.grid_column,
        "grid-column-start" => style.grid_column.start = source.grid_column.start,
        "grid-column-end" => style.grid_column.end = source.grid_column.end,
        "grid-row" => style.grid_row = source.grid_row,
        "grid-area" => style.grid_area_name.clone_from(&source.grid_area_name),
        "grid-row-start" => style.grid_row.start = source.grid_row.start,
        "grid-row-end" => style.grid_row.end = source.grid_row.end,
        "position" => style.position = source.position, "top" => style.inset.top = source.inset.top,
        "right" => style.inset.right = source.inset.right, "bottom" => style.inset.bottom = source.inset.bottom,
        "left" => style.inset.left = source.inset.left, "z-index" => style.z_index = source.z_index,
        "overflow" => { style.overflow_x = source.overflow_x; style.overflow_y = source.overflow_y; }
        "overflow-x" => style.overflow_x = source.overflow_x, "overflow-y" => style.overflow_y = source.overflow_y,
        "transform" => style.transform = source.transform, "opacity" => style.opacity = source.opacity,
        "visibility" => style.visibility = source.visibility, "pointer-events" => style.pointer_events = source.pointer_events,
        "object-fit" => style.object_fit = source.object_fit,
        "object-position" => style.object_position = source.object_position,
        "font-size" => style.font_size = source.font_size, "font-family" => style.font_family.clone_from(&source.font_family),
        "font-weight" => style.font_weight = source.font_weight, "font-style" => style.font_style = source.font_style,
        "line-height" => style.line_height = source.line_height, "text-align" => style.text_align = source.text_align,
        "white-space" => style.white_space = source.white_space, "text-transform" => style.text_transform = source.text_transform,
        "text-indent" => style.text_indent = source.text_indent,
        "text-decoration" | "text-decoration-line" => style.text_decoration = source.text_decoration,
        "font" => { style.font_size = source.font_size; style.font_family.clone_from(&source.font_family);
            style.font_weight = source.font_weight; style.font_style = source.font_style; style.line_height = source.line_height; }
        "color" => style.color = source.color,
        "background" | "background-color" => style.background_color = source.background_color,
        _ => {}
    }
}

fn resolve_css_value(
    input: &str,
    variables: &HashMap<String, String>,
    env: MediaEnvironment,
    font_size: f32,
    depth: usize,
) -> Option<String> {
    if depth > 8 { return None; }
    let mut value = input.trim().to_owned();
    while let Some(start) = value.find("var(") {
        let end = find_closing_paren(&value, start + 3)?;
        let body = &value[start + 4..end];
        let mut pieces = body.splitn(2, ',');
        let name = pieces.next()?.trim();
        let replacement = variables.get(name).map(String::as_str).or_else(|| pieces.next().map(str::trim))?;
        let replacement = resolve_css_value(replacement, variables, env, font_size, depth + 1)?;
        value.replace_range(start..=end, &replacement);
    }
    if let Some(body) = value.strip_prefix("calc(").and_then(|v| v.strip_suffix(')')) {
        return evaluate_calc(body, env, font_size).map(|v| format!("{v}px"));
    }
    for function in ["min", "max", "clamp"] {
        if let Some(body) = value.strip_prefix(function).and_then(|v| v.strip_prefix('(')).and_then(|v| v.strip_suffix(')')) {
            return evaluate_math_function(function, body, env, font_size).map(|v| format!("{v}px"));
        }
    }
    if let Some(converted) = convert_viewport_unit(&value, env, font_size) { return Some(converted); }
    let mut changed = false;
    let converted = value.split_ascii_whitespace().map(|token| {
        if let Some(result) = convert_viewport_unit(token, env, font_size) {
            changed = true;
            result
        } else {
            token.to_owned()
        }
    }).collect::<Vec<_>>().join(" ");
    Some(if changed { converted } else { value })
}

fn find_closing_paren(input: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in input[open..].char_indices() {
        if ch == '(' { depth += 1; }
        if ch == ')' {
            depth = depth.saturating_sub(1);
            if depth == 0 { return Some(open + offset); }
        }
    }
    None
}

fn convert_viewport_unit(value: &str, env: MediaEnvironment, font_size: f32) -> Option<String> {
    let trimmed = value.trim();
    for (unit, scale) in [("rem", 16.0), ("vw", env.width / 100.0), ("vh", env.height / 100.0), ("vmin", env.width.min(env.height) / 100.0), ("vmax", env.width.max(env.height) / 100.0), ("em", font_size)] {
        if let Some(number) = trimmed.strip_suffix(unit).and_then(|v| v.trim().parse::<f32>().ok()) {
            return Some(format!("{}px", number * scale));
        }
    }
    None
}

fn evaluate_calc(input: &str, env: MediaEnvironment, font_size: f32) -> Option<f32> {
    let normalized = input.replace('-', "+-");
    let mut total = 0.0;
    for term in normalized.split('+').map(str::trim).filter(|v| !v.is_empty()) {
        total += evaluate_calc_product(term, env, font_size)?;
    }
    total.is_finite().then_some(total)
}

fn evaluate_calc_product(input: &str, env: MediaEnvironment, font_size: f32) -> Option<f32> {
    let mut cursor = 0usize;
    let mut operator = '*';
    let mut result = 1.0;
    for (index, ch) in input.char_indices().chain(std::iter::once((input.len(), '*'))) {
        if index != input.len() && ch != '*' && ch != '/' { continue; }
        let factor = evaluate_math_length(input[cursor..index].trim(), env, font_size)?;
        result = if operator == '*' { result * factor } else {
            if factor == 0.0 { return None; }
            result / factor
        };
        operator = ch;
        cursor = index + ch.len_utf8();
    }
    result.is_finite().then_some(result)
}

fn evaluate_math_function(function: &str, body: &str, env: MediaEnvironment, font_size: f32) -> Option<f32> {
    let values = split_top_level(body, ',').into_iter().map(|part| evaluate_math_length(part, env, font_size))
        .collect::<Option<Vec<_>>>()?;
    match (function, values.as_slice()) {
        ("min", values) if !values.is_empty() => values.iter().copied().reduce(f32::min),
        ("max", values) if !values.is_empty() => values.iter().copied().reduce(f32::max),
        ("clamp", [minimum, preferred, maximum]) => Some(preferred.max(*minimum).min(*maximum)),
        _ => None,
    }
}

fn evaluate_math_length(input: &str, env: MediaEnvironment, font_size: f32) -> Option<f32> {
    let input = input.trim();
    if let Some(body) = input.strip_prefix("calc(").and_then(|v| v.strip_suffix(')')) {
        return evaluate_calc(body, env, font_size);
    }
    for function in ["min", "max", "clamp"] {
        if let Some(body) = input.strip_prefix(function).and_then(|v| v.strip_prefix('(')).and_then(|v| v.strip_suffix(')')) {
            return evaluate_math_function(function, body, env, font_size);
        }
    }
    let compact = input.split_ascii_whitespace().collect::<String>();
    let converted = convert_viewport_unit(&compact, env, font_size).unwrap_or(compact);
    let number = converted.strip_suffix("px").unwrap_or(&converted).trim().parse::<f32>().ok()?;
    number.is_finite().then_some(number)
}

fn apply_declaration(style: &mut ComputedStyle, declaration: &Declaration) {
    let value = declaration.value.trim();
    match declaration.name.as_str() {
        "display" => {
            if let Some(display) = parse_display(value) { style.display = display; }
        }
        "width" => if let Some(v) = parse_length(value) { style.width = v; },
        "height" => if let Some(v) = parse_length(value) { style.height = v; },
        "min-width" => if let Some(v) = parse_length(value) { style.min_width = v; },
        "min-height" => if let Some(v) = parse_length(value) { style.min_height = v; },
        "max-width" => if let Some(v) = parse_length(value) { style.max_width = v; },
        "max-height" => if let Some(v) = parse_length(value) { style.max_height = v; },
        "aspect-ratio" => if let Some(v) = parse_aspect_ratio(value) { style.aspect_ratio = v; },
        "box-sizing" => if let Some(v) = parse_box_sizing(value) { style.box_sizing = v; },
        "margin" => if let Some(v) = parse_edges(value, true) { style.margin = v; },
        "padding" => if let Some(v) = parse_edges(value, false) { style.padding = v; },
        "margin-top" => if let Some(v) = parse_length(value) { style.margin.top = v; },
        "margin-right" => if let Some(v) = parse_length(value) { style.margin.right = v; },
        "margin-bottom" => if let Some(v) = parse_length(value) { style.margin.bottom = v; },
        "margin-left" => if let Some(v) = parse_length(value) { style.margin.left = v; },
        "padding-top" => if let Some(v) = parse_length(value).filter(|v| !matches!(v, CssLength::Auto)) { style.padding.top = v; },
        "padding-right" => if let Some(v) = parse_length(value).filter(|v| !matches!(v, CssLength::Auto)) { style.padding.right = v; },
        "padding-bottom" => if let Some(v) = parse_length(value).filter(|v| !matches!(v, CssLength::Auto)) { style.padding.bottom = v; },
        "padding-left" => if let Some(v) = parse_length(value).filter(|v| !matches!(v, CssLength::Auto)) { style.padding.left = v; },
        "border-width" => if let Some(v) = parse_edges(value, false) { style.border_width = v; },
        "border-top-width" => if let Some(v) = parse_length(value).filter(|v| !matches!(v, CssLength::Auto)) { style.border_width.top = v; },
        "border-right-width" => if let Some(v) = parse_length(value).filter(|v| !matches!(v, CssLength::Auto)) { style.border_width.right = v; },
        "border-bottom-width" => if let Some(v) = parse_length(value).filter(|v| !matches!(v, CssLength::Auto)) { style.border_width.bottom = v; },
        "border-left-width" => if let Some(v) = parse_length(value).filter(|v| !matches!(v, CssLength::Auto)) { style.border_width.left = v; },
        "border-color" => if let Some(v) = parse_color_or_current(value, style.color) { style.border_color = v; },
        "border-radius" => if let Some(v) = parse_length(value).filter(|v| !matches!(v, CssLength::Auto)) { style.border_radius = v; },
        "border" => apply_border_shorthand(style, value),
        "gap" => if let Some(v) = parse_length(value).filter(|v| !matches!(v, CssLength::Auto)) {
            style.gap = v; style.row_gap = v; style.column_gap = v;
        },
        "row-gap" => if let Some(v) = parse_length(value).filter(|v| !matches!(v, CssLength::Auto)) { style.row_gap = v; },
        "column-gap" => if let Some(v) = parse_length(value).filter(|v| !matches!(v, CssLength::Auto)) { style.column_gap = v; },
        "flex-direction" => if let Some(v) = parse_flex_direction(value) { style.flex_direction = v; },
        "flex-wrap" => if let Some(v) = parse_flex_wrap(value) { style.flex_wrap = v; },
        "flex-basis" => if let Some(v) = parse_length(value) { style.flex_basis = v; },
        "flex-grow" => if let Some(v) = parse_number(value) { style.flex_grow = v.max(0.0); },
        "flex-shrink" => if let Some(v) = parse_number(value) { style.flex_shrink = v.max(0.0); },
        "order" => if let Ok(v) = value.trim().parse::<i32>() { style.order = v; },
        "flex" => if let Some((grow, shrink, basis)) = parse_flex(value) { style.flex_grow = grow; style.flex_shrink = shrink; style.flex_basis = basis; },
        "flex-flow" => if let Some((direction, wrap)) = parse_flex_flow(value) { style.flex_direction = direction; style.flex_wrap = wrap; },
        "align-items" => if let Some(v) = parse_item_alignment(value) { style.align_items = v; },
        "justify-items" => if let Some(v) = parse_item_alignment(value) { style.justify_items = v; },
        "align-self" => if let Some(v) = parse_item_alignment(value) { style.align_self = v; },
        "justify-self" => if let Some(v) = parse_item_alignment(value) { style.justify_self = v; },
        "align-content" => if let Some(v) = parse_content_alignment(value) { style.align_content = v; },
        "justify-content" => if let Some(v) = parse_content_alignment(value) { style.justify_content = v; },
        "place-items" => if let Some((align, justify)) = parse_place_items(value) { style.align_items = align; style.justify_items = justify; },
        "place-self" => if let Some((align, justify)) = parse_place_items(value) { style.align_self = align; style.justify_self = justify; },
        "place-content" => if let Some((align, justify)) = parse_place_content(value) { style.align_content = align; style.justify_content = justify; },
        "grid-template-columns" => if let Some(v) = parse_grid_track_list(value) { style.grid_template_columns = v; },
        "grid-template-rows" => if let Some(v) = parse_grid_track_list(value) { style.grid_template_rows = v; },
        "grid-template-areas" => if let Some(v) = parse_grid_template_areas(value) { style.grid_template_areas = v; },
        "grid-auto-columns" => if let Some(v) = parse_grid_auto_track_list(value) { style.grid_auto_columns = v; },
        "grid-auto-rows" => if let Some(v) = parse_grid_auto_track_list(value) { style.grid_auto_rows = v; },
        "grid-auto-flow" => if let Some(v) = parse_grid_auto_flow(value) { style.grid_auto_flow = v; },
        "grid-column" => if let Some(v) = parse_grid_line(value) { style.grid_column = v; style.grid_area_name = None; },
        "grid-column-start" => if let Some(v) = parse_grid_placement(value) { style.grid_column.start = v; style.grid_area_name = None; },
        "grid-column-end" => if let Some(v) = parse_grid_placement(value) { style.grid_column.end = v; style.grid_area_name = None; },
        "grid-row" => if let Some(v) = parse_grid_line(value) { style.grid_row = v; style.grid_area_name = None; },
        "grid-area" => if let Some(v) = parse_grid_area_name(value) {
            style.grid_area_name = v;
            style.grid_row = CssGridLine::default(); style.grid_column = CssGridLine::default();
        },
        "grid-row-start" => if let Some(v) = parse_grid_placement(value) { style.grid_row.start = v; style.grid_area_name = None; },
        "grid-row-end" => if let Some(v) = parse_grid_placement(value) { style.grid_row.end = v; style.grid_area_name = None; },
        "position" => if let Some(v) = parse_position(value) { style.position = v; },
        "top" => if let Some(v) = parse_length(value) { style.inset.top = v; },
        "right" => if let Some(v) = parse_length(value) { style.inset.right = v; },
        "bottom" => if let Some(v) = parse_length(value) { style.inset.bottom = v; },
        "left" => if let Some(v) = parse_length(value) { style.inset.left = v; },
        "z-index" => { style.z_index = parse_z_index(value); },
        "overflow" => if let Some(v) = parse_overflow(value) { style.overflow_x = v; style.overflow_y = v; },
        "overflow-x" => if let Some(v) = parse_overflow(value) { style.overflow_x = v; },
        "overflow-y" => if let Some(v) = parse_overflow(value) { style.overflow_y = v; },
        "transform" => if let Some(v) = parse_transform(value) { style.transform = v; },
        "opacity" => if let Some(v) = parse_number(value) { style.opacity = v.clamp(0.0, 1.0); },
        "visibility" => if let Some(v) = parse_visibility(value) { style.visibility = v; },
        "pointer-events" => if let Some(v) = parse_pointer_events(value) { style.pointer_events = v; },
        "object-fit" => if let Some(v) = parse_object_fit(value) { style.object_fit = v; },
        "object-position" => if let Some(v) = parse_object_position(value) { style.object_position = v; },
        "font-size" => if let Some(CssLength::Px(v)) = parse_length(value) { style.font_size = v.max(1.0); },
        "font-weight" => if let Some(v) = parse_font_weight(value) { style.font_weight = v; },
        "font-style" => if let Some(v) = parse_font_style(value) { style.font_style = v; },
        "line-height" => if let Some(v) = parse_line_height(value) { style.line_height = v; },
        "text-align" => if let Some(v) = parse_text_align(value) { style.text_align = v; },
        "white-space" => if let Some(v) = parse_white_space(value) { style.white_space = v; },
        "text-transform" => if let Some(v) = parse_text_transform(value) { style.text_transform = v; },
        "text-indent" => if let Some(v) = parse_length(value) { style.text_indent = v; },
        "text-decoration" | "text-decoration-line" => if let Some(v) = parse_text_decoration(value) { style.text_decoration = v; },
        "font" => if let Some((font_style, font_weight, font_size, line_height, family)) = parse_font_shorthand(value) {
            style.font_style = font_style; style.font_weight = font_weight; style.font_size = font_size;
            style.line_height = line_height; style.font_family = family;
        },
        "font-family" => {
            let family = value.split(',').next().unwrap_or(value).trim().trim_matches(|c| c == '"' || c == '\'');
            if !family.is_empty() { style.font_family = family.to_owned(); }
        },
        "color" => if let Some(v) = parse_color_or_current(value, style.color) { style.color = v; },
        "background" | "background-color" => if let Some(v) = parse_color_or_current(value, style.color) { style.background_color = v; },
        _ => {}
    }
}

fn apply_pseudo_declaration(style: &mut PseudoStyle, declaration: &Declaration) {
    let value = declaration.value.trim();
    match declaration.name.as_str() {
        "content" => {
            if let Some(content) = parse_content(value) { style.content = content; }
        }
        "color" => if let Some(v) = parse_color_or_current(value, style.color) { style.color = v; },
        "background" | "background-color" => if let Some(v) = parse_color_or_current(value, style.color) { style.background_color = v; },
        "font-size" => if let Some(CssLength::Px(v)) = parse_length(value) { style.font_size = v.max(1.0); },
        _ => {}
    }
}

fn parse_content(input: &str) -> Option<String> {
    let value = input.trim();
    if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("normal") { return Some(String::new()); }
    if value.len() >= 2 {
        let first = value.as_bytes()[0] as char;
        let last = value.as_bytes()[value.len() - 1] as char;
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            return Some(value[1..value.len() - 1].replace("\\n", "\n").replace("\\\"", "\"").replace("\\'", "'"));
        }
    }
    None
}

fn apply_border_shorthand(style: &mut ComputedStyle, input: &str) {
    let mut width = None;
    let mut color = None;

    for token in input.split_ascii_whitespace() {
        if width.is_none() {
            width = parse_length(token).filter(|value| !matches!(value, CssLength::Auto));
            if width.is_some() {
                continue;
            }
        }
        if color.is_none() {
            color = parse_color_or_current(token, style.color);
        }
    }

    if let Some(width) = width {
        style.border_width = EdgeSizes::all(width);
    }
    if let Some(color) = color {
        style.border_color = color;
    }
}

fn parse_position(input: &str) -> Option<CssPosition> {
    match parse_ident(input)?.to_ascii_lowercase().as_str() {
        "static" => Some(CssPosition::Static),
        "relative" => Some(CssPosition::Relative),
        "absolute" => Some(CssPosition::Absolute),
        "fixed" => Some(CssPosition::Fixed),
        "sticky" => Some(CssPosition::Sticky),
        _ => None,
    }
}

fn parse_overflow(input: &str) -> Option<CssOverflow> {
    match parse_ident(input)?.to_ascii_lowercase().as_str() {
        "visible" => Some(CssOverflow::Visible),
        "hidden" => Some(CssOverflow::Hidden),
        "clip" => Some(CssOverflow::Clip),
        "scroll" => Some(CssOverflow::Scroll),
        "auto" => Some(CssOverflow::Auto),
        _ => None,
    }
}

fn parse_z_index(input: &str) -> Option<i32> {
    let value = input.trim();
    if value.eq_ignore_ascii_case("auto") { return None; }
    value.parse::<i32>().ok()
}

fn parse_transform(input: &str) -> Option<CssTransform> {
    let value = input.trim();
    if value.eq_ignore_ascii_case("none") { return Some(CssTransform::default()); }
    let mut result = CssTransform::default();
    let mut recognized = false;
    for function in split_transform_functions(value) {
        if let Some(args) = function.strip_prefix("translate(").and_then(|v| v.strip_suffix(')')) {
            let parts = args.split(',').map(str::trim).collect::<Vec<_>>();
            let x = parse_px_number(parts.first().copied()?)?;
            let y = if let Some(v) = parts.get(1) { parse_px_number(v)? } else { 0.0 };
            result.translate_x += x; result.translate_y += y; recognized = true;
        } else if let Some(args) = function.strip_prefix("translatex(").and_then(|v| v.strip_suffix(')')) {
            result.translate_x += parse_px_number(args)?; recognized = true;
        } else if let Some(args) = function.strip_prefix("translatey(").and_then(|v| v.strip_suffix(')')) {
            result.translate_y += parse_px_number(args)?; recognized = true;
        } else if let Some(args) = function.strip_prefix("scale(").and_then(|v| v.strip_suffix(')')) {
            let parts = args.split(',').map(str::trim).collect::<Vec<_>>();
            let x = parts.first()?.parse::<f32>().ok()?;
            let y = parts.get(1).and_then(|v| v.parse::<f32>().ok()).unwrap_or(x);
            result.scale_x *= x; result.scale_y *= y; recognized = true;
        } else if let Some(args) = function.strip_prefix("scalex(").and_then(|v| v.strip_suffix(')')) {
            result.scale_x *= args.trim().parse::<f32>().ok()?; recognized = true;
        } else if let Some(args) = function.strip_prefix("scaley(").and_then(|v| v.strip_suffix(')')) {
            result.scale_y *= args.trim().parse::<f32>().ok()?; recognized = true;
        }
    }
    recognized.then_some(result)
}

fn split_transform_functions(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    for (i, ch) in input.char_indices() {
        if ch == '(' { depth += 1; }
        if ch == ')' { depth = depth.saturating_sub(1); }
        if ch.is_whitespace() && depth == 0 {
            if start < i { out.push(input[start..i].trim().to_ascii_lowercase()); }
            start = i + ch.len_utf8();
        }
    }
    if start < input.len() { out.push(input[start..].trim().to_ascii_lowercase()); }
    out
}

fn parse_px_number(input: &str) -> Option<f32> {
    let value = input.trim();
    if let Some(px) = value.strip_suffix("px") { px.trim().parse::<f32>().ok() }
    else if value == "0" { Some(0.0) } else { None }
}

fn parse_display(input: &str) -> Option<CssDisplay> {
    match parse_ident(input)?.to_ascii_lowercase().as_str() {
        "none" => Some(CssDisplay::None),
        "block" | "flow-root" => Some(CssDisplay::Block),
        "inline" | "inline-block" => Some(CssDisplay::Inline),
        "flex" | "inline-flex" => Some(CssDisplay::Flex),
        "grid" | "inline-grid" => Some(CssDisplay::Grid),
        _ => None,
    }
}

fn parse_flex_direction(input: &str) -> Option<CssFlexDirection> {
    match parse_ident(input)?.to_ascii_lowercase().as_str() {
        "row" => Some(CssFlexDirection::Row),
        "row-reverse" => Some(CssFlexDirection::RowReverse),
        "column" => Some(CssFlexDirection::Column),
        "column-reverse" => Some(CssFlexDirection::ColumnReverse),
        _ => None,
    }
}

fn parse_flex_wrap(input: &str) -> Option<CssFlexWrap> {
    match parse_ident(input)?.to_ascii_lowercase().as_str() {
        "nowrap" => Some(CssFlexWrap::NoWrap), "wrap" => Some(CssFlexWrap::Wrap),
        "wrap-reverse" => Some(CssFlexWrap::WrapReverse), _ => None,
    }
}

fn parse_box_sizing(input: &str) -> Option<CssBoxSizing> {
    match parse_ident(input)?.to_ascii_lowercase().as_str() {
        "content-box" => Some(CssBoxSizing::ContentBox), "border-box" => Some(CssBoxSizing::BorderBox), _ => None,
    }
}

fn parse_font_style(input: &str) -> Option<CssFontStyle> {
    match parse_ident(input)?.to_ascii_lowercase().as_str() {
        "normal" => Some(CssFontStyle::Normal), "italic" => Some(CssFontStyle::Italic),
        "oblique" => Some(CssFontStyle::Oblique), _ => None,
    }
}

fn parse_font_weight(input: &str) -> Option<u16> {
    match input.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(400), "bold" | "bolder" => Some(700), "lighter" => Some(300),
        value => value.parse::<u16>().ok().filter(|weight| (1..=1000).contains(weight)),
    }
}

fn parse_line_height(input: &str) -> Option<CssLineHeight> {
    let value = input.trim().to_ascii_lowercase();
    if value == "normal" { return Some(CssLineHeight::Normal); }
    if let Some(percent) = value.strip_suffix('%').and_then(|v| v.parse::<f32>().ok()) {
        return (percent.is_finite() && percent > 0.0).then_some(CssLineHeight::Number(percent / 100.0));
    }
    if let Some(px) = value.strip_suffix("px").and_then(|v| v.parse::<f32>().ok()) {
        return (px.is_finite() && px > 0.0).then_some(CssLineHeight::Px(px));
    }
    let number = value.parse::<f32>().ok()?;
    (number.is_finite() && number > 0.0).then_some(CssLineHeight::Number(number))
}

fn parse_text_align(input: &str) -> Option<CssTextAlign> {
    match parse_ident(input)?.to_ascii_lowercase().as_str() {
        "start" => Some(CssTextAlign::Start), "end" => Some(CssTextAlign::End),
        "left" => Some(CssTextAlign::Left), "right" => Some(CssTextAlign::Right),
        "center" => Some(CssTextAlign::Center), "justify" => Some(CssTextAlign::Justify), _ => None,
    }
}

fn parse_white_space(input: &str) -> Option<CssWhiteSpace> {
    match parse_ident(input)?.to_ascii_lowercase().as_str() {
        "normal" => Some(CssWhiteSpace::Normal), "nowrap" => Some(CssWhiteSpace::NoWrap),
        "pre" => Some(CssWhiteSpace::Pre), "pre-wrap" => Some(CssWhiteSpace::PreWrap), _ => None,
    }
}

fn parse_text_transform(input: &str) -> Option<CssTextTransform> {
    match parse_ident(input)?.to_ascii_lowercase().as_str() {
        "none" => Some(CssTextTransform::None), "uppercase" => Some(CssTextTransform::Uppercase),
        "lowercase" => Some(CssTextTransform::Lowercase), "capitalize" => Some(CssTextTransform::Capitalize), _ => None,
    }
}

fn parse_visibility(input: &str) -> Option<CssVisibility> {
    match parse_ident(input)?.to_ascii_lowercase().as_str() {
        "visible" => Some(CssVisibility::Visible), "hidden" => Some(CssVisibility::Hidden),
        "collapse" => Some(CssVisibility::Collapse), _ => None,
    }
}

fn parse_pointer_events(input: &str) -> Option<CssPointerEvents> {
    match parse_ident(input)?.to_ascii_lowercase().as_str() {
        "auto" => Some(CssPointerEvents::Auto), "none" => Some(CssPointerEvents::None), _ => None,
    }
}

fn parse_object_fit(input: &str) -> Option<CssObjectFit> {
    match parse_ident(input)?.to_ascii_lowercase().as_str() {
        "fill" => Some(CssObjectFit::Fill), "contain" => Some(CssObjectFit::Contain),
        "cover" => Some(CssObjectFit::Cover), "none" => Some(CssObjectFit::None),
        "scale-down" => Some(CssObjectFit::ScaleDown), _ => None,
    }
}

fn parse_object_position(input: &str) -> Option<CssObjectPosition> {
    let tokens = input.split_ascii_whitespace().collect::<Vec<_>>();
    if tokens.is_empty() || tokens.len() > 2 { return None; }
    let axis_value = |token: &str, horizontal: bool| -> Option<f32> {
        match token.to_ascii_lowercase().as_str() {
            "center" => Some(0.5), "left" if horizontal => Some(0.0), "right" if horizontal => Some(1.0),
            "top" if !horizontal => Some(0.0), "bottom" if !horizontal => Some(1.0),
            value => value.strip_suffix('%')?.parse::<f32>().ok().map(|number| number / 100.0),
        }
    };
    if tokens.len() == 1 {
        return axis_value(tokens[0], true).map(|x| CssObjectPosition { x, y: 0.5 })
            .or_else(|| axis_value(tokens[0], false).map(|y| CssObjectPosition { x: 0.5, y }));
    }
    Some(CssObjectPosition { x: axis_value(tokens[0], true)?, y: axis_value(tokens[1], false)? })
}

fn parse_text_decoration(input: &str) -> Option<CssTextDecoration> {
    let value = input.trim().to_ascii_lowercase();
    if value == "none" { return Some(CssTextDecoration::default()); }
    let mut decoration = CssTextDecoration::default();
    for token in value.split_ascii_whitespace() {
        match token {
            "underline" => decoration.underline = true,
            "overline" => decoration.overline = true,
            "line-through" => decoration.line_through = true,
            _ => return None,
        }
    }
    (decoration.underline || decoration.overline || decoration.line_through).then_some(decoration)
}

fn parse_font_shorthand(input: &str) -> Option<(CssFontStyle, u16, f32, CssLineHeight, String)> {
    let tokens = split_css_whitespace(input);
    let mut font_style = CssFontStyle::Normal;
    let mut font_weight = 400;
    let mut font_size = None;
    let mut line_height = CssLineHeight::Normal;
    let mut family = Vec::new();
    for token in tokens {
        if font_size.is_none() {
            let (size_token, height_token) = token.split_once('/').map_or((token.as_str(), None), |(size, height)| (size, Some(height)));
            if let Some(CssLength::Px(size)) = parse_length(size_token) {
                font_size = Some(size.max(1.0));
                if let Some(height) = height_token { line_height = parse_line_height(height)?; }
                continue;
            }
            if let Some(value) = parse_font_style(&token) { font_style = value; continue; }
            if let Some(value) = parse_font_weight(&token) { font_weight = value; continue; }
            return None;
        }
        family.push(token);
    }
    let family = family.join(" ").trim_matches(|ch| ch == '\'' || ch == '"').to_owned();
    if family.is_empty() { return None; }
    Some((font_style, font_weight, font_size?, line_height, family))
}

impl CssLineHeight {
    #[must_use]
    pub fn used_px(self, font_size: f32) -> f32 {
        match self { Self::Normal => font_size * 1.25, Self::Number(value) => font_size * value, Self::Px(value) => value }.max(1.0)
    }
}

fn parse_aspect_ratio(input: &str) -> Option<Option<f32>> {
    let value = input.trim();
    if value.eq_ignore_ascii_case("auto") { return Some(None); }
    let parts = split_top_level(value, '/');
    let ratio = match parts.as_slice() {
        [single] => single.trim().parse::<f32>().ok()?,
        [width, height] => {
            let width = width.trim().parse::<f32>().ok()?;
            let height = height.trim().parse::<f32>().ok()?;
            if height <= 0.0 { return None; }
            width / height
        }
        _ => return None,
    };
    (ratio.is_finite() && ratio > 0.0).then_some(Some(ratio))
}

fn parse_flex_flow(input: &str) -> Option<(CssFlexDirection, CssFlexWrap)> {
    let mut direction = None;
    let mut wrap = None;
    for token in input.split_ascii_whitespace() {
        if direction.is_none() { direction = parse_flex_direction(token); if direction.is_some() { continue; } }
        if wrap.is_none() { wrap = parse_flex_wrap(token); if wrap.is_some() { continue; } }
        return None;
    }
    if direction.is_none() && wrap.is_none() { return None; }
    Some((direction.unwrap_or(CssFlexDirection::Row), wrap.unwrap_or(CssFlexWrap::NoWrap)))
}

fn parse_flex(input: &str) -> Option<(f32, f32, CssLength)> {
    let value = input.trim().to_ascii_lowercase();
    match value.as_str() {
        "none" => return Some((0.0, 0.0, CssLength::Auto)),
        "auto" => return Some((1.0, 1.0, CssLength::Auto)),
        _ => {}
    }
    let tokens = value.split_ascii_whitespace().collect::<Vec<_>>();
    if tokens.is_empty() || tokens.len() > 3 { return None; }
    let mut numbers = Vec::new();
    let mut basis = None;
    for token in tokens {
        if basis.is_none() {
            if let Some(length) = parse_length(token) {
                if token.contains("px") || token.contains('%') || token == "auto" || (token == "0" && numbers.len() == 2) {
                    basis = Some(length); continue;
                }
            }
        }
        let number = token.parse::<f32>().ok()?;
        if !number.is_finite() || number < 0.0 || numbers.len() == 2 { return None; }
        numbers.push(number);
    }
    match (numbers.as_slice(), basis) {
        ([], Some(basis)) => Some((1.0, 1.0, basis)),
        ([grow], basis) => Some((*grow, 1.0, basis.unwrap_or(CssLength::Percent(0.0)))),
        ([grow, shrink], basis) => Some((*grow, *shrink, basis.unwrap_or(CssLength::Percent(0.0)))),
        _ => None,
    }
}

fn parse_grid_auto_flow(input: &str) -> Option<CssGridAutoFlow> {
    match input.trim().to_ascii_lowercase().split_ascii_whitespace().collect::<Vec<_>>().as_slice() {
        ["row"] => Some(CssGridAutoFlow::Row),
        ["column"] => Some(CssGridAutoFlow::Column),
        ["row", "dense"] | ["dense", "row"] | ["dense"] => Some(CssGridAutoFlow::RowDense),
        ["column", "dense"] | ["dense", "column"] => Some(CssGridAutoFlow::ColumnDense),
        _ => None,
    }
}

fn parse_grid_area_name(input: &str) -> Option<Option<String>> {
    let value = input.trim();
    if value.eq_ignore_ascii_case("auto") { return Some(None); }
    let name = parse_ident(value)?;
    let lower = name.to_ascii_lowercase();
    if matches!(lower.as_str(), "span" | "none" | "inherit" | "initial" | "unset") { return None; }
    Some(Some(name))
}

fn parse_grid_template_areas(input: &str) -> Option<Option<CssGridTemplateAreas>> {
    if input.trim().eq_ignore_ascii_case("none") { return Some(None); }
    let raw_rows = parse_css_string_sequence(input)?;
    if raw_rows.is_empty() { return None; }
    let mut rows = Vec::new();
    let mut column_count = None;
    for raw_row in raw_rows {
        let cells = raw_row.split_ascii_whitespace().map(|cell| {
            if cell.chars().all(|ch| ch == '.') { Some(None) }
            else if is_grid_area_identifier(cell) { Some(Some(cell.to_owned())) }
            else { None }
        }).collect::<Option<Vec<_>>>()?;
        if cells.is_empty() { return None; }
        if let Some(expected) = column_count { if cells.len() != expected { return None; } }
        else { column_count = Some(cells.len()); }
        rows.push(cells);
    }
    let mut bounds = HashMap::<String, (usize, usize, usize, usize)>::new();
    for (row, cells) in rows.iter().enumerate() {
        for (column, name) in cells.iter().enumerate() {
            let Some(name) = name else { continue };
            bounds.entry(name.clone()).and_modify(|area| {
                area.0 = area.0.min(row); area.1 = area.1.max(row);
                area.2 = area.2.min(column); area.3 = area.3.max(column);
            }).or_insert((row, row, column, column));
        }
    }
    let mut areas = HashMap::new();
    for (name, (row_min, row_max, column_min, column_max)) in bounds {
        for row in row_min..=row_max {
            for column in column_min..=column_max {
                if rows[row][column].as_deref() != Some(name.as_str()) { return None; }
            }
        }
        areas.insert(name, CssNamedGridArea {
            row_start: row_min as i16 + 1, row_end: row_max as i16 + 2,
            column_start: column_min as i16 + 1, column_end: column_max as i16 + 2,
        });
    }
    Some(Some(CssGridTemplateAreas { rows, column_count: column_count?, areas }))
}

fn parse_css_string_sequence(input: &str) -> Option<Vec<String>> {
    let chars = input.trim().chars().collect::<Vec<_>>();
    let mut rows = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        while index < chars.len() && chars[index].is_whitespace() { index += 1; }
        if index == chars.len() { break; }
        let quote = chars[index];
        if quote != '\'' && quote != '"' { return None; }
        index += 1;
        let mut row = String::new();
        let mut closed = false;
        while index < chars.len() {
            let ch = chars[index]; index += 1;
            if ch == quote { closed = true; break; }
            if ch == '\\' {
                if index >= chars.len() { return None; }
                row.push(chars[index]); index += 1;
            } else { row.push(ch); }
        }
        if !closed { return None; }
        rows.push(row);
    }
    Some(rows)
}

fn is_grid_area_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else { return false };
    (first == '_' || first == '-' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '-' || ch.is_ascii_alphanumeric())
}

fn parse_item_alignment(input: &str) -> Option<CssItemAlignment> {
    match input.trim().to_ascii_lowercase().as_str() {
        "auto" => Some(CssItemAlignment::Auto), "normal" => Some(CssItemAlignment::Normal),
        "start" | "self-start" => Some(CssItemAlignment::Start),
        "end" | "self-end" => Some(CssItemAlignment::End),
        "flex-start" => Some(CssItemAlignment::FlexStart), "flex-end" => Some(CssItemAlignment::FlexEnd),
        "center" => Some(CssItemAlignment::Center), "baseline" | "first baseline" => Some(CssItemAlignment::Baseline),
        "stretch" => Some(CssItemAlignment::Stretch), _ => None,
    }
}

fn parse_content_alignment(input: &str) -> Option<CssContentAlignment> {
    match input.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(CssContentAlignment::Normal), "start" => Some(CssContentAlignment::Start),
        "end" => Some(CssContentAlignment::End), "flex-start" => Some(CssContentAlignment::FlexStart),
        "flex-end" => Some(CssContentAlignment::FlexEnd), "center" => Some(CssContentAlignment::Center),
        "stretch" => Some(CssContentAlignment::Stretch), "space-between" => Some(CssContentAlignment::SpaceBetween),
        "space-around" => Some(CssContentAlignment::SpaceAround), "space-evenly" => Some(CssContentAlignment::SpaceEvenly),
        _ => None,
    }
}

fn parse_place_items(input: &str) -> Option<(CssItemAlignment, CssItemAlignment)> {
    let values = input.split_ascii_whitespace().collect::<Vec<_>>();
    if values.is_empty() || values.len() > 2 { return None; }
    let first = parse_item_alignment(values[0])?;
    Some((first, if values.len() == 2 { parse_item_alignment(values[1])? } else { first }))
}

fn parse_place_content(input: &str) -> Option<(CssContentAlignment, CssContentAlignment)> {
    let values = input.split_ascii_whitespace().collect::<Vec<_>>();
    if values.is_empty() || values.len() > 2 { return None; }
    let first = parse_content_alignment(values[0])?;
    Some((first, if values.len() == 2 { parse_content_alignment(values[1])? } else { first }))
}

fn parse_grid_track_list(input: &str) -> Option<Vec<CssGridTrack>> {
    parse_grid_track_list_inner(input, 0)
}

fn parse_grid_auto_track_list(input: &str) -> Option<Vec<CssGridTrack>> {
    let tracks = parse_grid_track_list(input)?;
    (!tracks.iter().any(|track| matches!(track, CssGridTrack::AutoRepeat { .. }))).then_some(tracks)
}

fn parse_grid_track_list_inner(input: &str, depth: usize) -> Option<Vec<CssGridTrack>> {
    if depth > 8 { return None; }
    if input.trim().eq_ignore_ascii_case("none") { return Some(Vec::new()); }
    let mut tracks = Vec::new();
    for token in split_css_whitespace(input) {
        if let Some(body) = token.strip_prefix("repeat(").and_then(|value| value.strip_suffix(')')) {
            let parts = split_top_level(body, ',');
            if parts.len() != 2 { return None; }
            let repetition = parts[0].trim().to_ascii_lowercase();
            if repetition == "auto-fill" || repetition == "auto-fit" {
                let pattern = parse_grid_track_list_inner(parts[1], depth + 1)?;
                if pattern.is_empty() || pattern.iter().any(|track| matches!(track, CssGridTrack::AutoRepeat { .. })) { return None; }
                tracks.push(CssGridTrack::AutoRepeat {
                    mode: if repetition == "auto-fill" { CssGridRepeat::AutoFill } else { CssGridRepeat::AutoFit },
                    tracks: pattern,
                });
                continue;
            }
            let count = parts[0].trim().parse::<usize>().ok()?.min(64);
            if count == 0 { return None; }
            let pattern = parse_grid_track_list_inner(parts[1], depth + 1)?;
            if pattern.is_empty() { return None; }
            if tracks.len().saturating_add(count.saturating_mul(pattern.len())) > 256 { return None; }
            for _ in 0..count { tracks.extend(pattern.iter().cloned()); }
        } else {
            tracks.push(parse_grid_track(&token)?);
            if tracks.len() > 256 { return None; }
        }
    }
    (!tracks.is_empty()).then_some(tracks)
}

fn split_css_whitespace(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut round = 0usize;
    for (index, ch) in input.char_indices() {
        match ch { '(' => round += 1, ')' => round = round.saturating_sub(1), _ => {} }
        if ch.is_whitespace() && round == 0 {
            if start < index { out.push(input[start..index].trim().to_ascii_lowercase()); }
            start = index + ch.len_utf8();
        }
    }
    if start < input.len() { out.push(input[start..].trim().to_ascii_lowercase()); }
    out
}

fn parse_grid_track(input: &str) -> Option<CssGridTrack> {
    let value = input.trim().to_ascii_lowercase();
    if let Some(body) = value.strip_prefix("minmax(").and_then(|value| value.strip_suffix(')')) {
        let parts = split_top_level(body, ',');
        if parts.len() != 2 { return None; }
        let min = parse_grid_breadth(parts[0])?;
        if matches!(min, CssGridBreadth::Fr(_)) { return None; }
        return Some(CssGridTrack::MinMax { min, max: parse_grid_breadth(parts[1])? });
    }
    if let Some(body) = value.strip_prefix("fit-content(").and_then(|value| value.strip_suffix(')')) {
        let limit = parse_length(body)?;
        if matches!(limit, CssLength::Auto) { return None; }
        return Some(CssGridTrack::FitContent(limit));
    }
    match value.as_str() {
        "auto" => Some(CssGridTrack::Auto),
        "min-content" => Some(CssGridTrack::MinContent),
        "max-content" => Some(CssGridTrack::MaxContent),
        _ => {
            if let Some(number) = value.strip_suffix("fr").and_then(|v| v.parse::<f32>().ok()) {
                return (number.is_finite() && number >= 0.0).then_some(CssGridTrack::Fr(number));
            }
            if let Some(number) = value.strip_suffix("px").and_then(|v| v.parse::<f32>().ok()) {
                return (number.is_finite() && number >= 0.0).then_some(CssGridTrack::Px(number));
            }
            if let Some(number) = value.strip_suffix('%').and_then(|v| v.parse::<f32>().ok()) {
                return (number.is_finite() && number >= 0.0).then_some(CssGridTrack::Percent(number / 100.0));
            }
            None
        }
    }
}

fn parse_grid_breadth(input: &str) -> Option<CssGridBreadth> {
    match parse_grid_track(input)? {
        CssGridTrack::Auto => Some(CssGridBreadth::Auto),
        CssGridTrack::Px(v) => Some(CssGridBreadth::Px(v)),
        CssGridTrack::Percent(v) => Some(CssGridBreadth::Percent(v)),
        CssGridTrack::Fr(v) => Some(CssGridBreadth::Fr(v)),
        CssGridTrack::MinContent => Some(CssGridBreadth::MinContent),
        CssGridTrack::MaxContent => Some(CssGridBreadth::MaxContent),
        CssGridTrack::MinMax { .. } | CssGridTrack::FitContent(_) | CssGridTrack::AutoRepeat { .. } => None,
    }
}

fn parse_grid_placement(input: &str) -> Option<CssGridPlacement> {
    let value = input.trim().to_ascii_lowercase();
    if value == "auto" { return Some(CssGridPlacement::Auto); }
    if let Some(rest) = value.strip_prefix("span ") {
        let span = rest.trim().parse::<u16>().ok()?;
        return (span > 0).then_some(CssGridPlacement::Span(span));
    }
    let line = value.parse::<i16>().ok()?;
    (line != 0).then_some(CssGridPlacement::Line(line))
}

fn parse_grid_line(input: &str) -> Option<CssGridLine> {
    let parts = split_top_level(input, '/');
    if parts.is_empty() || parts.len() > 2 { return None; }
    Some(CssGridLine {
        start: parse_grid_placement(parts[0])?,
        end: if parts.len() == 2 { parse_grid_placement(parts[1])? } else { CssGridPlacement::Auto },
    })
}

fn parse_ident(input: &str) -> Option<String> {
    let mut source = ParserInput::new(input.trim());
    let mut parser = Parser::new(&mut source);
    let ident = parser.expect_ident_cloned().ok()?.to_string();
    parser.expect_exhausted().ok()?;
    Some(ident)
}

fn parse_number(input: &str) -> Option<f32> {
    let mut source = ParserInput::new(input.trim());
    let mut parser = Parser::new(&mut source);
    let value = parser.expect_number().ok()?;
    parser.expect_exhausted().ok()?;
    Some(value)
}

fn parse_length(input: &str) -> Option<CssLength> {
    let mut source = ParserInput::new(input.trim());
    let mut parser = Parser::new(&mut source);
    let token = parser.next().ok()?.clone();
    let value = match token {
        Token::Ident(value) if value.eq_ignore_ascii_case("auto") => CssLength::Auto,
        Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("px") => CssLength::Px(value),
        Token::Percentage { unit_value, .. } => CssLength::Percent(unit_value),
        Token::Number { value, .. } if value == 0.0 => CssLength::Px(0.0),
        _ => return None,
    };
    parser.expect_exhausted().ok()?;
    Some(value)
}

fn parse_edges(input: &str, allow_auto: bool) -> Option<EdgeSizes<CssLength>> {
    let values = input
        .split_ascii_whitespace()
        .map(parse_length)
        .collect::<Option<Vec<_>>>()?;
    if values.is_empty() || values.len() > 4 || (!allow_auto && values.iter().any(|v| matches!(v, CssLength::Auto))) {
        return None;
    }
    Some(match values.as_slice() {
        [a] => EdgeSizes::all(*a),
        [v, h] => EdgeSizes { top: *v, right: *h, bottom: *v, left: *h },
        [t, h, b] => EdgeSizes { top: *t, right: *h, bottom: *b, left: *h },
        [t, r, b, l] => EdgeSizes { top: *t, right: *r, bottom: *b, left: *l },
        _ => return None,
    })
}

fn parse_color(input: &str) -> Option<Rgba> {
    let trimmed = input.trim();
    if let Some(color) = parse_function_color(trimmed) { return Some(color); }
    let mut source = ParserInput::new(trimmed);
    let mut parser = Parser::new(&mut source);
    let token = parser.next().ok()?.clone();
    let color = match token {
        Token::Hash(value) | Token::IDHash(value) => parse_hex_color(value.as_ref())?,
        Token::Ident(value) => match value.to_ascii_lowercase().as_str() {
            "black" => Rgba::rgb(0, 0, 0),
            "white" => Rgba::rgb(255, 255, 255),
            "red" => Rgba::rgb(255, 0, 0),
            "green" => Rgba::rgb(0, 128, 0),
            "blue" => Rgba::rgb(0, 0, 255),
            "yellow" => Rgba::rgb(255, 255, 0),
            "cyan" | "aqua" => Rgba::rgb(0, 255, 255),
            "magenta" | "fuchsia" => Rgba::rgb(255, 0, 255),
            "orange" => Rgba::rgb(255, 165, 0),
            "purple" => Rgba::rgb(128, 0, 128),
            "lime" => Rgba::rgb(0, 255, 0),
            "navy" => Rgba::rgb(0, 0, 128),
            "teal" => Rgba::rgb(0, 128, 128),
            "silver" => Rgba::rgb(192, 192, 192),
            "maroon" => Rgba::rgb(128, 0, 0),
            "olive" => Rgba::rgb(128, 128, 0),
            "gray" | "grey" => Rgba::rgb(128, 128, 128),
            "transparent" => Rgba::TRANSPARENT,
            _ => return None,
        },
        _ => return None,
    };
    parser.expect_exhausted().ok()?;
    Some(color)
}

fn parse_hex_color(value: &str) -> Option<Rgba> {
    match value.len() {
        3 => {
            let r = u8::from_str_radix(&value[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&value[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&value[2..3], 16).ok()? * 17;
            Some(Rgba::rgb(r, g, b))
        }
        6 => Some(Rgba::rgb(
            u8::from_str_radix(&value[0..2], 16).ok()?,
            u8::from_str_radix(&value[2..4], 16).ok()?,
            u8::from_str_radix(&value[4..6], 16).ok()?,
        )),
        4 => Some(Rgba {
            r: u8::from_str_radix(&value[0..1], 16).ok()? * 17,
            g: u8::from_str_radix(&value[1..2], 16).ok()? * 17,
            b: u8::from_str_radix(&value[2..3], 16).ok()? * 17,
            a: u8::from_str_radix(&value[3..4], 16).ok()? * 17,
        }),
        8 => Some(Rgba {
            r: u8::from_str_radix(&value[0..2], 16).ok()?,
            g: u8::from_str_radix(&value[2..4], 16).ok()?,
            b: u8::from_str_radix(&value[4..6], 16).ok()?,
            a: u8::from_str_radix(&value[6..8], 16).ok()?,
        }),
        _ => None,
    }
}

fn parse_color_or_current(input: &str, current: Rgba) -> Option<Rgba> {
    if input.trim().eq_ignore_ascii_case("currentcolor") { Some(current) } else { parse_color(input) }
}

fn parse_function_color(input: &str) -> Option<Rgba> {
    let open = input.find('(')?;
    if !input.ends_with(')') { return None; }
    let name = input[..open].trim().to_ascii_lowercase();
    let body = &input[open + 1..input.len() - 1];
    match name.as_str() {
        "rgb" | "rgba" => parse_rgb_function(body),
        "hsl" | "hsla" => parse_hsl_function(body),
        _ => None,
    }
}

fn color_function_tokens(body: &str) -> Vec<&str> {
    body.split(|ch: char| ch == ',' || ch == '/' || ch.is_ascii_whitespace())
        .filter(|token| !token.is_empty()).collect()
}

fn parse_rgb_function(body: &str) -> Option<Rgba> {
    let tokens = color_function_tokens(body);
    if tokens.len() != 3 && tokens.len() != 4 { return None; }
    let channel = |token: &str| -> Option<u8> {
        let value = if let Some(percent) = token.strip_suffix('%') {
            percent.parse::<f32>().ok()? * 2.55
        } else { token.parse::<f32>().ok()? };
        value.is_finite().then_some(value.round().clamp(0.0, 255.0) as u8)
    };
    Some(Rgba { r: channel(tokens[0])?, g: channel(tokens[1])?, b: channel(tokens[2])?, a: parse_alpha(tokens.get(3).copied())? })
}

fn parse_hsl_function(body: &str) -> Option<Rgba> {
    let tokens = color_function_tokens(body);
    if tokens.len() != 3 && tokens.len() != 4 { return None; }
    let raw_hue = tokens[0].trim_end_matches("deg").parse::<f32>().ok()?;
    let raw_saturation = tokens[1].strip_suffix('%')?.parse::<f32>().ok()?;
    let raw_lightness = tokens[2].strip_suffix('%')?.parse::<f32>().ok()?;
    if !raw_hue.is_finite() || !raw_saturation.is_finite() || !raw_lightness.is_finite() { return None; }
    let hue = raw_hue.rem_euclid(360.0) / 360.0;
    let saturation = raw_saturation.clamp(0.0, 100.0) / 100.0;
    let lightness = raw_lightness.clamp(0.0, 100.0) / 100.0;
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue * 6.0;
    let x = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = match sector.floor() as i32 {
        0 => (chroma, x, 0.0), 1 => (x, chroma, 0.0), 2 => (0.0, chroma, x),
        3 => (0.0, x, chroma), 4 => (x, 0.0, chroma), _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma * 0.5;
    Some(Rgba {
        r: ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        g: ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        b: ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        a: parse_alpha(tokens.get(3).copied())?,
    })
}

fn parse_alpha(token: Option<&str>) -> Option<u8> {
    let Some(token) = token else { return Some(255) };
    let value = if let Some(percent) = token.strip_suffix('%') {
        percent.parse::<f32>().ok()? / 100.0
    } else { token.parse::<f32>().ok()? };
    value.is_finite().then_some((value.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') { i += 1; }
            i = (i + 2).min(bytes.len());
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn find_matching_brace(input: &str, open: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        if escaped { escaped = false; continue; }
        if b == b'\\' { escaped = true; continue; }
        if let Some(q) = quote {
            if b == q { quote = None; }
            continue;
        }
        if b == b'\'' || b == b'"' { quote = Some(b); continue; }
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 { return Some(i); }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level(input: &str, separator: char) -> Vec<&str> {
    let bytes = input.as_bytes();
    let sep = separator as u8;
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut round = 0usize;
    let mut square = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate() {
        if escaped { escaped = false; continue; }
        if b == b'\\' { escaped = true; continue; }
        if let Some(q) = quote {
            if b == q { quote = None; }
            continue;
        }
        if b == b'\'' || b == b'"' { quote = Some(b); continue; }
        match b {
            b'(' => round += 1,
            b')' => round = round.saturating_sub(1),
            b'[' => square += 1,
            b']' => square = square.saturating_sub(1),
            _ if b == sep && round == 0 && square == 0 => {
                parts.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&input[start..]);
    parts
}

fn find_top_level_char(input: &str, needle: char) -> Option<usize> {
    let bytes = input.as_bytes();
    let needle = needle as u8;
    let mut round = 0usize;
    let mut quote = None;
    for (i, &b) in bytes.iter().enumerate() {
        if let Some(q) = quote {
            if b == q { quote = None; }
            continue;
        }
        if b == b'\'' || b == b'"' { quote = Some(b); continue; }
        match b {
            b'(' => round += 1,
            b')' => round = round.saturating_sub(1),
            _ if b == needle && round == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_html;
    use url::Url;

    #[test]
    fn cssparser_parses_web_lengths() {
        assert_eq!(parse_length("12px"), Some(CssLength::Px(12.0)));
        assert_eq!(parse_length("50%"), Some(CssLength::Percent(0.5)));
        assert_eq!(parse_length("auto"), Some(CssLength::Auto));
    }

    #[test]
    fn cascade_applies_tag_class_id_and_inline() {
        let html = r#"
            <style>
              div { width: 100px; color: red; }
              .card { width: 200px; display: flex; }
              #hero { width: 300px; }
            </style>
            <div id="hero" class="card" style="height: 80px"></div>
        "#;
        let dom = parse_html(Url::parse("https://nexus.local/").unwrap(), html);
        let styles = compute_styles(&dom);
        let div = dom.find_first_element("div").unwrap();
        let style = styles.get(div).unwrap();
        assert_eq!(style.width, CssLength::Px(300.0));
        assert_eq!(style.height, CssLength::Px(80.0));
        assert_eq!(style.display, CssDisplay::Flex);
        assert_eq!(style.color, Rgba::rgb(255, 0, 0));
    }
}
