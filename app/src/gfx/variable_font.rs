//! Variable outlines and advances for the existing weight picker; normal fonts keep fontdue.
use super::text::Weight;
use fontdue::{Font, FontSettings, Metrics};
use std::ops::Deref;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

pub struct LoadedFont {
    font: Font,
    bytes: Vec<u8>,
    pub variable: bool,
}
impl Deref for LoadedFont {
    type Target = Font;
    fn deref(&self) -> &Font {
        &self.font
    }
}
fn weight(w: Weight) -> f32 {
    match w {
        Weight::Light => 300.,
        Weight::Regular => 400.,
        Weight::SemiBold => 600.,
        Weight::Bold => 700.,
        Weight::Black => 900.,
    }
}
impl LoadedFont {
    pub fn from_bytes(bytes: Vec<u8>) -> Option<Self> {
        let variable = ttf_parser::Face::parse(&bytes, 0)
            .ok()
            .map(|f| !f.variation_axes().is_empty())
            .unwrap_or(false);
        let font = Font::from_bytes(bytes.as_slice(), FontSettings::default()).ok()?;
        Some(Self {
            font,
            bytes,
            variable,
        })
    }
    fn face(&self, px: f32, w: Weight) -> Option<ttf_parser::Face<'_>> {
        let mut face = ttf_parser::Face::parse(&self.bytes, 0).ok()?;
        face.set_variation(ttf_parser::Tag::from_bytes(b"wght"), weight(w));
        face.set_variation(ttf_parser::Tag::from_bytes(b"opsz"), px);
        Some(face)
    }
    pub fn glyph_metrics(&self, index: u16, px: f32, w: Weight) -> Metrics {
        if !self.variable {
            return self.font.metrics_indexed(index, px);
        }
        self.outline(index, px, w)
            .map(|(m, _)| m)
            .unwrap_or_else(|| self.font.metrics_indexed(index, px))
    }
    fn outline(
        &self,
        index: u16,
        px: f32,
        w: Weight,
    ) -> Option<(Metrics, Option<tiny_skia::Path>)> {
        let f = self.face(px, w)?;
        let id = ttf_parser::GlyphId(index);
        let scale = px / f.units_per_em() as f32;
        let mut m = Metrics::default();
        m.advance_width = f.glyph_hor_advance(id).unwrap_or(0) as f32 * scale;
        let mut outline = Outline(PathBuilder::new());
        let Some(bounds) = f.outline_glyph(id, &mut outline) else {
            return Some((m, None));
        };
        m.xmin = (bounds.x_min as f32 * scale).floor() as i32;
        m.ymin = (bounds.y_min as f32 * scale).floor() as i32;
        m.width = ((bounds.x_max as f32 * scale).ceil() as i32 - m.xmin).max(0) as usize;
        m.height = ((bounds.y_max as f32 * scale).ceil() as i32 - m.ymin).max(0) as usize;
        let transform = Transform::from_row(
            scale,
            0.,
            0.,
            -scale,
            -m.xmin as f32,
            m.height as f32 + m.ymin as f32,
        );
        let path = outline.0.finish().and_then(|p| p.transform(transform));
        Some((m, path))
    }
    pub fn raster(&self, index: u16, px: f32, w: Weight) -> (Metrics, Vec<u8>) {
        if self.variable {
            if let Some((m, path)) = self.outline(index, px, w) {
                if m.width == 0 || m.height == 0 {
                    return (m, Vec::new());
                }
                if let Some(mut pix) = Pixmap::new(m.width as u32, m.height as u32) {
                    if let Some(path) = path {
                        let mut paint = Paint::default();
                        paint.set_color_rgba8(255, 255, 255, 255);
                        paint.anti_alias = true;
                        pix.fill_path(
                            &path,
                            &paint,
                            FillRule::Winding,
                            Transform::identity(),
                            None,
                        );
                    }
                    return (m, pix.data().chunks_exact(4).map(|p| p[3]).collect());
                }
            }
        }
        self.font.rasterize_indexed(index, px)
    }
}
struct Outline(PathBuilder);
impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.quad_to(x1, y1, x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.cubic_to(x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        self.0.close();
    }
}
