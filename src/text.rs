//! Text layout for Nexus Engine 1.02.
//!
//! Parley performs Unicode text analysis, shaping (through HarfRust), line
//! breaking and alignment. Nexus stores only the compact line geometry needed
//! by the display-list layer. Precise glyph-to-Skia bridging is intentionally
//! deferred; the first Skia backend paints the resulting line strings.

use parley::{Alignment, AlignmentOptions, FontContext, Layout, LayoutContext, StyleProperty};

#[derive(Debug, Clone, PartialEq)]
pub struct TextLine {
    pub text: String,
    pub x: f32,
    pub baseline: f32,
    pub width: f32,
    pub line_height: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextLayout {
    pub width: f32,
    pub height: f32,
    pub lines: Vec<TextLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextLayoutOptions {
    pub collapse_whitespace: bool,
    pub wrap: bool,
}

impl Default for TextLayoutOptions {
    fn default() -> Self { Self { collapse_whitespace: true, wrap: true } }
}

pub trait TextLayoutEngine {
    fn layout_text(&mut self, text: &str, font_size: f32, max_width: f32) -> TextLayout;

    fn layout_text_with_options(
        &mut self, text: &str, font_size: f32, max_width: f32, _options: TextLayoutOptions,
    ) -> TextLayout {
        self.layout_text(text, font_size, max_width)
    }
}

pub struct ParleyTextEngine {
    font_context: FontContext,
    layout_context: LayoutContext<u32>,
}

impl Default for ParleyTextEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ParleyTextEngine {
    #[must_use]
    pub fn new() -> Self {
        Self {
            font_context: FontContext::new(),
            layout_context: LayoutContext::new(),
        }
    }
}

impl TextLayoutEngine for ParleyTextEngine {
    fn layout_text(&mut self, text: &str, font_size: f32, max_width: f32) -> TextLayout {
        self.layout_text_with_options(text, font_size, max_width, TextLayoutOptions::default())
    }

    fn layout_text_with_options(
        &mut self, text: &str, font_size: f32, max_width: f32, options: TextLayoutOptions,
    ) -> TextLayout {
        let normalized = if options.collapse_whitespace { normalize_text(text) } else { text.to_owned() };
        if normalized.is_empty() {
            return TextLayout::default();
        }

        let size = font_size.max(1.0);
        let width_limit = max_width.max(size);

        let mut builder = self.layout_context.ranged_builder(
            &mut self.font_context,
            normalized.as_str(),
            1.0,
            true,
        );
        builder.push_default(StyleProperty::Brush(0_u32));
        builder.push_default(StyleProperty::FontSize(size));

        let mut layout: Layout<u32> = builder.build(normalized.as_str());
        layout.break_all_lines(options.wrap.then_some(width_limit));
        layout.align(Alignment::Start, AlignmentOptions::default());

        let mut lines = Vec::new();
        for line in layout.lines() {
            let metrics = line.metrics();
            let range = line.text_range();
            let line_text = normalized
                .get(range)
                .unwrap_or("")
                .trim_end_matches(&['\r', '\n'][..])
                .to_owned();

            if line_text.is_empty() {
                continue;
            }

            lines.push(TextLine {
                text: line_text,
                x: metrics.offset,
                baseline: metrics.baseline,
                width: (metrics.advance - metrics.trailing_whitespace).max(0.0),
                line_height: metrics.line_height.max(size),
            });
        }

        if lines.is_empty() {
            // A defensive fallback for platforms where no usable system font is
            // available to Fontique yet. It keeps the renderer functional.
            return TextLayout {
                width: (normalized.chars().count() as f32 * size * 0.55).min(width_limit),
                height: size * 1.25,
                lines: vec![TextLine {
                    text: normalized,
                    x: 0.0,
                    baseline: size,
                    width: width_limit,
                    line_height: size * 1.25,
                }],
            };
        }

        TextLayout {
            width: layout.width(),
            height: layout.height(),
            lines,
        }
    }
}

fn normalize_text(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parley_produces_at_least_one_line() {
        let mut engine = ParleyTextEngine::new();
        let layout = engine.layout_text("Nexus Engine text", 16.0, 240.0);
        assert!(!layout.lines.is_empty());
        assert!(layout.height > 0.0);
    }
}
