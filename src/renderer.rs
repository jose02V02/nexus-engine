//! Skia raster backend for Nexus Engine 1.02.

use std::path::Path;

use skia_safe::{
    images, surfaces, AlphaType, Color, ColorType, Data, EncodedImageFormat, Font,
    FontMgr, FontStyle, ImageInfo, Paint, PaintStyle, RRect, Rect,
};

use crate::css::{CssFontStyle, Rgba};
use crate::display_list::{DisplayCommand, DisplayList, ImageAsset};
use crate::error::{NexusError, NexusResult};

pub trait Renderer {
    fn render_png(&mut self, display_list: &DisplayList) -> NexusResult<Vec<u8>>;

    fn render_png_file(&mut self, display_list: &DisplayList, path: &Path) -> NexusResult<()> {
        let png = self.render_png(display_list)?;
        std::fs::write(path, png)?;
        Ok(())
    }
}

pub struct SkiaRenderer {
    font_mgr: FontMgr,
}

impl Default for SkiaRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl SkiaRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self { font_mgr: FontMgr::new() }
    }
}

impl Renderer for SkiaRenderer {
    fn render_png(&mut self, display_list: &DisplayList) -> NexusResult<Vec<u8>> {
        let width = f32_to_i32(display_list.width);
        let height = f32_to_i32(display_list.height);
        let mut surface = surfaces::raster_n32_premul((width, height))
            .ok_or_else(|| NexusError::Render("Skia non ha creato la raster surface".to_owned()))?;

        for command in &display_list.commands {
            match command {
                DisplayCommand::Clear { color } => {
                    surface.canvas().clear(to_skia_color(*color));
                }
                DisplayCommand::PushClipRect { rect } => {
                    surface.canvas().save();
                    surface.canvas().clip_rect(to_rect(*rect), None, true);
                }
                DisplayCommand::PushClipRoundedRect { rect, radius } => {
                    surface.canvas().save();
                    let rounded = RRect::new_rect_xy(to_rect(*rect), *radius, *radius);
                    surface.canvas().clip_rrect(rounded, None, true);
                }
                DisplayCommand::PopClip => {
                    if surface.canvas().save_count() > 1 {
                        surface.canvas().restore();
                    }
                }
                DisplayCommand::FillRect { rect, color, .. } => {
                    let paint = fill_paint(*color);
                    surface.canvas().draw_rect(to_rect(*rect), &paint);
                }
                DisplayCommand::FillRoundedRect { rect, radius, color, .. } => {
                    let paint = fill_paint(*color);
                    surface.canvas().draw_round_rect(to_rect(*rect), *radius, *radius, &paint);
                }
                DisplayCommand::StrokeRoundedRect { rect, radius, width, color, .. } => {
                    let mut paint = fill_paint(*color);
                    paint.set_style(PaintStyle::Stroke);
                    paint.set_stroke_width((*width).max(0.0));
                    surface.canvas().draw_round_rect(to_rect(*rect), *radius, *radius, &paint);
                }
                DisplayCommand::DrawImage { asset_index, rect, .. } => {
                    let asset = display_list.images.get(*asset_index).ok_or_else(|| {
                        NexusError::Render(format!("asset image mancante: {asset_index}"))
                    })?;
                    let image = skia_image(asset)?;
                    let mut paint = Paint::default();
                    paint.set_anti_alias(true);
                    surface.canvas().draw_image_rect(image, None, to_rect(*rect), &paint);
                }
                DisplayCommand::DrawText {
                    x,
                    baseline,
                    text,
                    font_size,
                    font_family,
                    font_weight,
                    font_style,
                    color,
                    ..
                } => {
                    let paint = fill_paint(*color);
                    let typeface = self
                        .font_mgr
                        .match_family_style(font_family, skia_font_style(*font_weight, *font_style))
                        .or_else(|| {
                            self.font_mgr
                                .match_family_style("sans-serif", skia_font_style(*font_weight, *font_style))
                        });
                    let mut font = typeface
                        .map(|typeface| Font::from_typeface(typeface, Some((*font_size).max(1.0))))
                        .unwrap_or_default();
                    font.set_size((*font_size).max(1.0));
                    font.set_subpixel(true);
                    surface.canvas().draw_str(text, (*x, *baseline), &font, &paint);
                }
            }
        }

        while surface.canvas().save_count() > 1 {
            surface.canvas().restore();
        }

        let image = surface.image_snapshot();
        let mut context = surface.direct_context();
        let data = image
            .encode(context.as_mut(), EncodedImageFormat::PNG, None)
            .ok_or_else(|| NexusError::Render("Skia non ha codificato il frame PNG".to_owned()))?;
        Ok(data.as_bytes().to_vec())
    }
}

fn skia_font_style(weight: u16, style: CssFontStyle) -> FontStyle {
    match (weight >= 600, style != CssFontStyle::Normal) {
        (true, true) => FontStyle::bold_italic(),
        (true, false) => FontStyle::bold(),
        (false, true) => FontStyle::italic(),
        (false, false) => FontStyle::normal(),
    }
}

fn skia_image(asset: &ImageAsset) -> NexusResult<skia_safe::Image> {
    let width = i32::try_from(asset.width)
        .map_err(|_| NexusError::Render("larghezza immagine fuori range".to_owned()))?;
    let height = i32::try_from(asset.height)
        .map_err(|_| NexusError::Render("altezza immagine fuori range".to_owned()))?;
    let info = ImageInfo::new(
        (width, height),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    images::raster_from_data(&info, Data::new_copy(asset.rgba.as_ref()), asset.width as usize * 4)
        .ok_or_else(|| NexusError::Render("Skia non ha creato l'immagine RGBA".to_owned()))
}

fn fill_paint(color: Rgba) -> Paint {
    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Fill);
    paint.set_anti_alias(true);
    paint.set_color(to_skia_color(color));
    paint
}

fn to_rect(rect: crate::display_list::PaintRect) -> Rect {
    Rect::from_xywh(rect.x, rect.y, rect.width.max(0.0), rect.height.max(0.0))
}

fn to_skia_color(color: Rgba) -> Color {
    Color::from_argb(color.a, color.r, color.g, color.b)
}

fn f32_to_i32(value: f32) -> i32 {
    if !value.is_finite() {
        return 1;
    }
    value.ceil().clamp(1.0, i32::MAX as f32) as i32
}
