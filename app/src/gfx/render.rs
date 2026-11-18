//! Renders a [`GraphicDoc`] into a tiny-skia pixmap at any time `t` of its animation.

use std::{collections::HashMap, path::Path, sync::Arc};

use tiny_skia::{
    Color, FillRule, FilterQuality, GradientStop, LineCap, LinearGradient, Mask, Paint,
    PathBuilder, Pattern, Pixmap, PixmapPaint, Point, Rect, Shader, SpreadMode, Stroke, Transform,
};

use crate::{
    config::Brand,
    data::legend_stops,
    gfx::{
        bind::Bindings,
        ease_out, ease_out_back,
        icons::{draw_icon, IconKind},
        model::{AnimKind, Fill, GraphicDoc, ImageFit, Layer, LayerKind},
        parse_color,
        text::{draw_line, draw_text_box, measure, Align, GlyphCache, TextStyle, Weight},
        with_alpha,
    },
};

pub struct RenderCtx<'a> {
    pub bindings: &'a Bindings,
    pub brand: &'a Brand,
    /// Seconds since the graphic was taken to air (use a large value for the settled look).
    pub time: f32,
    /// Seconds since the graphic was taken off air, while it animates out.
    pub out_time: Option<f32>,
    pub media_dir: &'a Path,
}

/// The settled time for static renders (thumbnails, editor, PNG export).
pub const SETTLED: f32 = 10_000.0;

#[derive(Default)]
pub struct GraphicsRenderer {
    glyphs: GlyphCache,
    media: HashMap<String, Option<Arc<Pixmap>>>,
    /// Camera images (cam-*.jpg) are re-read when the file changes.
    live_media: HashMap<String, (Option<std::time::SystemTime>, Option<Arc<Pixmap>>)>,
}

struct AnimState {
    opacity: f32,
    dx: f32,
    dy: f32,
    scale: f32,
    wipe: f32,
    /// The wipe opens from the centre instead of the left edge.
    wipe_center: bool,
    /// Extra rotation from keyframes, degrees.
    rot: f32,
}

/// Exponential ease-out: fast start, long soft landing (modern broadcast motion).
fn ease_out_expo(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t >= 1.0 { 1.0 } else { 1.0 - 2f32.powf(-10.0 * t) }
}

/// Elastic ease-out (spring overshoot), after Robert Penner's easing equations (BSD).
fn ease_out_elastic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t <= 0.0 || t >= 1.0 {
        return t;
    }
    let c4 = (2.0 * std::f32::consts::PI) / 3.0;
    2f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
}

/// Browser compositor poses use the same easing, keyframes and timing as native output.
pub fn browser_pose(layer: &Layer, doc: &GraphicDoc, ctx: &RenderCtx) -> [f32; 6] {
    let s = anim_state(layer, doc, ctx);
    // A wipe from the centre (Reveal) is sent as 2 + amount so the page can tell it apart.
    let wipe = if s.wipe_center && s.wipe < 1.0 { 2.0 + s.wipe.max(0.0) } else { s.wipe };
    [(layer.opacity * s.opacity).clamp(0.0, 1.0), s.dx, s.dy, s.scale, layer.rotation + s.rot, wipe]
}

fn apply_anim(kind: AnimKind, p: f32, raw: f32, layer: &Layer, doc: &GraphicDoc, s: &mut AnimState) {
    let (doc_w, doc_h) = (doc.width as f32, doc.height as f32);
    match kind {
        AnimKind::None => {}
        AnimKind::Fade => s.opacity *= p,
        AnimKind::SlideLeft => s.dx += -(layer.x + layer.w) * (1.0 - p),
        AnimKind::SlideRight => s.dx += (doc_w - layer.x) * (1.0 - p),
        AnimKind::SlideUp => s.dy += (doc_h - layer.y) * (1.0 - p),
        AnimKind::SlideDown => s.dy += -(layer.y + layer.h) * (1.0 - p),
        AnimKind::Wipe => s.wipe = s.wipe.min(p),
        AnimKind::Pop => {
            s.scale *= 0.4 + 0.6 * ease_out_back(raw);
            s.opacity *= (raw * 2.0).min(1.0);
        }
        AnimKind::Rise => {
            let e = ease_out_expo(raw);
            s.dy += 70.0 * (1.0 - e);
            s.opacity *= (raw * 1.6).min(1.0);
        }
        AnimKind::Zoom => {
            let e = ease_out_expo(raw);
            s.scale *= 1.0 + 0.22 * (1.0 - e);
            s.opacity *= (raw * 1.8).min(1.0);
        }
        AnimKind::Drop => {
            s.dy += -(layer.y + layer.h) * (1.0 - crate::gfx::model::ease_curve("bounce", raw));
        }
        AnimKind::Elastic => {
            s.scale *= ease_out_elastic(raw).max(0.0);
            s.opacity *= (raw * 3.0).min(1.0);
        }
        AnimKind::Spin => {
            s.rot += -90.0 * (1.0 - p);
            s.scale *= 0.6 + 0.4 * p;
            s.opacity *= p;
        }
        AnimKind::Grow => s.wipe = s.wipe.min(ease_out_expo(raw)),
        AnimKind::Reveal => {
            s.wipe = s.wipe.min(ease_out_expo(raw));
            s.wipe_center = true;
        }
    }
}

fn anim_state(layer: &Layer, doc: &GraphicDoc, ctx: &RenderCtx) -> AnimState {
    let mut s = AnimState {
        opacity: 1.0,
        dx: 0.0,
        dy: 0.0,
        scale: 1.0,
        wipe: 1.0,
        wipe_center: false,
        rot: 0.0,
    };
    let time = doc.layer_time(layer, ctx.time);
    let a = &layer.anim_in;
    if a.kind != AnimKind::None {
        let raw = if a.duration <= 0.0 {
            if time >= a.delay { 1.0 } else { 0.0 }
        } else {
            ((time - a.delay) / a.duration).clamp(0.0, 1.0)
        };
        apply_anim(a.kind, ease_out(raw), raw, layer, doc, &mut s);
    }
    if let Some((x, y, scale, rot, opacity)) = layer.sample_keys(time) {
        s.dx += x - layer.x;
        s.dy += y - layer.y;
        s.scale *= scale.max(0.0);
        s.rot += rot - layer.rotation;
        s.opacity *= opacity.clamp(0.0, 1.0);
    }
    if let Some(out_t) = ctx.out_time {
        let a = &layer.anim_out;
        let raw = if a.duration <= 0.0 {
            if out_t >= a.delay { 0.0 } else { 1.0 }
        } else {
            1.0 - ((out_t - a.delay) / a.duration).clamp(0.0, 1.0)
        };
        let kind = if a.kind == AnimKind::None { AnimKind::Fade } else { a.kind };
        apply_anim(kind, ease_out(raw), raw, layer, doc, &mut s);
    }
    // Modern look: weather icons float gently once they are in (a slow 3 s bob, each icon a
    // little out of step), standing in for animated icon art. Still renders (thumbnails, PNG
    // export) use the settled time and stay put; Classic icons never move.
    if matches!(layer.kind, LayerKind::Icon { .. }) && ctx.time < SETTLED * 0.5 && crate::gfx::look::of(doc) == "modern" {
        let settled_at = layer.anim_in.delay + layer.anim_in.duration;
        let idle = (time - settled_at).max(0.0);
        let ramp = (idle / 0.8).min(1.0);
        let phase = idle * std::f32::consts::TAU / 3.0 + layer.x * 0.013;
        s.dy += phase.sin() * layer.h * 0.022 * ramp;
        s.scale *= 1.0 + (phase * 0.5).sin() * 0.012 * ramp;
    }
    s
}

/// Paint for a fill over the local rect (x, y, w, h).
pub fn fill_paint(fill: &Fill, brand: &Brand, rect: (f32, f32, f32, f32), opacity: f32) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.anti_alias = true;
    match fill {
        Fill::Solid { color } => paint.set_color(with_alpha(parse_color(color, brand), opacity)),
        Fill::Linear { from, to, angle } => {
            let (x, y, w, h) = rect;
            let (sin, cos) = angle.to_radians().sin_cos();
            let half = ((w * cos).abs() + (h * sin).abs()) / 2.0;
            let (cx, cy) = (x + w / 2.0, y + h / 2.0);
            let start = Point::from_xy(cx - cos * half, cy - sin * half);
            let end = Point::from_xy(cx + cos * half, cy + sin * half);
            let from = with_alpha(parse_color(from, brand), opacity);
            let to = with_alpha(parse_color(to, brand), opacity);
            paint.shader = LinearGradient::new(
                start,
                end,
                vec![GradientStop::new(0.0, from), GradientStop::new(1.0, to)],
                SpreadMode::Pad,
                Transform::identity(),
            )
            .unwrap_or(Shader::SolidColor(from));
        }
    }
    paint
}

pub fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let mut pb = PathBuilder::new();
    if r <= 0.01 {
        pb.push_rect(Rect::from_xywh(x, y, w.max(0.1), h.max(0.1))?);
        return pb.finish();
    }
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

/// A logo fitted inside a text box (w × h, device pixels): as tall as the box allows, kept to its
/// width, aligned like the text it replaces and centred vertically.
fn draw_logo(pm: &mut Pixmap, img: &Pixmap, (w, h): (f32, f32), align: Align) {
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    if iw < 1.0 || ih < 1.0 {
        return;
    }
    let s = (w / iw).min(h / ih);
    let (dw, dh) = (iw * s, ih * s);
    let dx = match align {
        Align::Left => 0.0,
        Align::Center => (w - dw) / 2.0,
        Align::Right => w - dw,
    };
    let dy = (h - dh) / 2.0;
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.shader = Pattern::new(img.as_ref(), SpreadMode::Pad, FilterQuality::Bicubic, 1.0, Transform::from_translate(dx, dy).pre_scale(s, s));
    if let Some(rect) = Rect::from_xywh(dx, dy, dw, dh) {
        pm.fill_rect(rect, &paint, Transform::identity(), None);
    }
}

fn solid_paint(color: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    paint
}

impl GraphicsRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn glyphs(&mut self) -> &mut GlyphCache {
        &mut self.glyphs
    }

    /// Forget cached media (after the library changes).
    pub fn clear_media(&mut self) {
        self.media.clear();
    }

    pub fn media_pixmap(&mut self, dir: &Path, name: &str) -> Option<Arc<Pixmap>> {
        if name.starts_with("cam-") {
            let path = dir.join(name);
            let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            if let Some((seen, img)) = self.live_media.get(name) {
                if *seen == modified {
                    return img.clone();
                }
            }
            let loaded = load_image_pixmap(&path).map(Arc::new);
            self.live_media.insert(name.to_string(), (modified, loaded.clone()));
            return loaded;
        }
        if let Some(found) = self.media.get(name) {
            return found.clone();
        }
        let loaded = load_image_pixmap(&dir.join(name)).map(Arc::new);
        self.media.insert(name.to_string(), loaded.clone());
        loaded
    }

    fn brand_wordmark(&mut self) -> Option<Arc<Pixmap>> {
        if let Some(logo) = self.media.get("{{brand}}") {
            return logo.clone();
        }
        let mut style = TextStyle {
            family: crate::ui::DISPLAY_FONT.into(),
            weight: Weight::Black,
            size: 144.0,
            color: Color::WHITE,
            letter_spacing: 0.0,
        };
        let head = measure("OPEN", &style);
        let width = head + measure("WX", &style);
        let mut logo = Pixmap::new(width.ceil() as u32 + 32, 160)?;
        draw_line(&mut logo, &mut self.glyphs, "OPEN", 16.0, 132.0, &style, 1.0, None);
        let [_, red, green, blue] = crate::ui::BRAND.to_be_bytes();
        style.color = Color::from_rgba8(red, green, blue, 255);
        draw_line(&mut logo, &mut self.glyphs, "WX", 16.0 + head, 132.0, &style, 1.0, None);
        let logo = Arc::new(logo);
        self.media.insert("{{brand}}".into(), Some(logo.clone()));
        Some(logo)
    }

    /// Render `doc` at its animation time into `pm` (scaled to the pixmap size).
    pub fn render(&mut self, pm: &mut Pixmap, doc: &GraphicDoc, ctx: &RenderCtx) {
        // Automatic reflow: stacked containers close up around empty or hidden children.
        let reflowed;
        let doc = if doc.layers.iter().any(|l| !l.stack.is_empty()) {
            reflowed = doc.reflow(|l| match &l.kind {
                LayerKind::Text { text, .. } => ctx.bindings.apply(text).trim().is_empty(),
                _ => false,
            });
            &reflowed
        } else {
            doc
        };
        let sx = pm.width() as f32 / doc.width.max(1) as f32;
        let sy = pm.height() as f32 / doc.height.max(1) as f32;
        let base = Transform::from_scale(sx, sy);

        if let Some(background) = &doc.background {
            if let Some(rect) = Rect::from_xywh(0.0, 0.0, doc.width as f32, doc.height as f32) {
                let mut opacity = 1.0;
                if let Some(out_t) = ctx.out_time {
                    opacity = 1.0 - (out_t / 0.4).clamp(0.0, 1.0);
                }
                let paint = fill_paint(
                    background,
                    ctx.brand,
                    (0.0, 0.0, doc.width as f32, doc.height as f32),
                    opacity,
                );
                pm.fill_rect(rect, &paint, base, None);
            }
        }

        for layer in doc.layers.iter().filter(|l| l.visible) {
            if !layer.show_if.is_empty() {
                let v = ctx.bindings.apply(&layer.show_if);
                if v.trim().is_empty() || v.trim() == "--" {
                    continue;
                }
            }
            self.render_layer(pm, doc, layer, ctx, base, sx.min(sy));
        }
    }

    fn render_layer(
        &mut self,
        pm: &mut Pixmap,
        doc: &GraphicDoc,
        layer: &Layer,
        ctx: &RenderCtx,
        base: Transform,
        k: f32,
    ) {
        let state = anim_state(layer, doc, ctx);
        let opacity = (layer.opacity * state.opacity).clamp(0.0, 1.0);
        if opacity <= 0.002 || state.wipe <= 0.0 || layer.w <= 0.0 || layer.h <= 0.0 {
            return;
        }
        let (w, h) = (layer.w, layer.h);
        let local = base
            .pre_translate(layer.x + state.dx + w / 2.0, layer.y + state.dy + h / 2.0)
            .pre_rotate(layer.rotation + state.rot)
            .pre_scale(state.scale, state.scale)
            .pre_translate(-w / 2.0, -h / 2.0);

        let mask = if state.wipe < 1.0 {
            let mut mask = Mask::new(pm.width(), pm.height());
            let x0 = if state.wipe_center { w * (1.0 - state.wipe) / 2.0 } else { 0.0 };
            if let (Some(mask), Some(rect)) = (mask.as_mut(), Rect::from_xywh(x0, 0.0, (w * state.wipe).max(0.01), h)) {
                let path = PathBuilder::from_rect(rect);
                mask.fill_path(&path, FillRule::Winding, true, local);
            }
            mask
        } else {
            None
        };
        let mask = mask.as_ref();

        match &layer.kind {
            LayerKind::Diagram { diagram } => {
                let font = resolve_font("$body", ctx.brand);
                crate::gfx::diagrams::draw(pm, &mut self.glyphs, local, mask, w, h, diagram, ctx.time - layer.anim_in.delay, opacity, &font);
            }
            LayerKind::Particles { effect, density, speed, color } => {
                draw_particles(pm, local, mask, w, h, effect, *density, *speed, with_alpha(parse_color(color, ctx.brand), opacity), ctx.time);
            }
            LayerKind::RiskMap { spec } => {
                let font = resolve_font("$display", ctx.brand);
                crate::gfx::riskmap::draw(pm, &mut self.glyphs, local, mask, w, h, &ctx.bindings.apply(spec), opacity, &font);
            }
            LayerKind::RiskMeter { value, style, labels } => {
                let level = parse_level(&ctx.bindings.apply(value));
                // The fill grows in after the layer's in-animation starts.
                let t = doc.layer_time(layer, ctx.time) - layer.anim_in.delay;
                let grow = ease_out_expo((t / 1.1).clamp(0.0, 1.0));
                let font = resolve_font("$display", ctx.brand);
                draw_risk_meter(pm, &mut self.glyphs, local, mask, w, h, level * grow, level, style, *labels, opacity, &font);
            }
            LayerKind::Rect { fill, stroke, radius, skew, sheen } => {
                let path = if skew.abs() > 0.001 { plate_path(w, h, *skew) } else { rounded_rect(0.0, 0.0, w, h, *radius) };
                if let Some(path) = path {
                    let paint = fill_paint(fill, ctx.brand, (0.0, 0.0, w, h), opacity);
                    pm.fill_path(&path, &paint, FillRule::Winding, local, mask);
                    if *sheen {
                        draw_sheen(pm, &path, h, opacity, local, mask);
                    }
                    if let Some(stroke) = stroke {
                        let paint = solid_paint(with_alpha(parse_color(&stroke.color, ctx.brand), opacity));
                        let st = Stroke {
                            width: stroke.width,
                            ..Stroke::default()
                        };
                        pm.stroke_path(&path, &paint, &st, local, mask);
                    }
                }
            }
            LayerKind::Ellipse { fill, stroke } => {
                let mut pb = PathBuilder::new();
                if let Some(rect) = Rect::from_xywh(0.0, 0.0, w, h) {
                    pb.push_oval(rect);
                }
                if let Some(path) = pb.finish() {
                    let paint = fill_paint(fill, ctx.brand, (0.0, 0.0, w, h), opacity);
                    pm.fill_path(&path, &paint, FillRule::Winding, local, mask);
                    if let Some(stroke) = stroke {
                        let paint = solid_paint(with_alpha(parse_color(&stroke.color, ctx.brand), opacity));
                        let st = Stroke {
                            width: stroke.width,
                            ..Stroke::default()
                        };
                        pm.stroke_path(&path, &paint, &st, local, mask);
                    }
                }
            }
            LayerKind::Line { color, width, arrow } => {
                let paint = solid_paint(with_alpha(parse_color(color, ctx.brand), opacity));
                let st = Stroke {
                    width: *width,
                    line_cap: LineCap::Round,
                    ..Stroke::default()
                };
                let head = if *arrow { (width * 3.5).min(w * 0.4) } else { 0.0 };
                let mut pb = PathBuilder::new();
                pb.move_to(0.0, h / 2.0);
                pb.line_to(w - head * 0.8, h / 2.0);
                if let Some(path) = pb.finish() {
                    pm.stroke_path(&path, &paint, &st, local, mask);
                }
                if *arrow {
                    let mut pb = PathBuilder::new();
                    pb.move_to(w, h / 2.0);
                    pb.line_to(w - head, h / 2.0 - head * 0.6);
                    pb.line_to(w - head, h / 2.0 + head * 0.6);
                    pb.close();
                    if let Some(path) = pb.finish() {
                        pm.fill_path(&path, &paint, FillRule::Winding, local, mask);
                    }
                }
            }
            LayerKind::Image { media, fit } => {
                // Media names may be bindings, e.g. {{sponsor_logo}}.
                let image = if crate::gfx::bind::is_brand_image(media) {
                    self.brand_wordmark()
                } else {
                    let media = if media.contains("{{") { ctx.bindings.apply(media) } else { media.clone() };
                    let name = media.trim();
                    let path = Path::new(name);
                    if name.starts_with("motif:") {
                        crate::gfx::motifs::raster(name, w.max(h))
                    } else if path.file_name() == Some(path.as_os_str()) {
                        self.media_pixmap(ctx.media_dir, name)
                    } else {
                        None
                    }
                };
                if let Some(img) = image {
                    let (iw, ih) = (img.width() as f32, img.height() as f32);
                    let (dx, dy, dw, dh) = match fit {
                        ImageFit::Stretch => (0.0, 0.0, w, h),
                        ImageFit::Contain => {
                            let s = (w / iw).min(h / ih);
                            ((w - iw * s) / 2.0, (h - ih * s) / 2.0, iw * s, ih * s)
                        }
                        ImageFit::Cover => {
                            let s = (w / iw).max(h / ih);
                            ((w - iw * s) / 2.0, (h - ih * s) / 2.0, iw * s, ih * s)
                        }
                    };
                    let pattern_ts = Transform::from_translate(dx, dy).pre_scale(dw / iw, dh / ih);
                    let mut paint = Paint::default();
                    paint.anti_alias = true;
                    paint.shader = Pattern::new(
                        (*img).as_ref(),
                        SpreadMode::Pad,
                        FilterQuality::Bicubic,
                        opacity,
                        pattern_ts,
                    );
                    let target = if *fit == ImageFit::Cover {
                        Rect::from_xywh(0.0, 0.0, w, h)
                    } else {
                        Rect::from_xywh(dx, dy, dw, dh)
                    };
                    if let Some(rect) = target {
                        pm.fill_rect(rect, &paint, local, mask);
                    }
                }
            }
            _ => {
                // Text-like layers render into a scratch layer at device resolution, then go
                // through the same transform (so they rotate/scale/wipe like shapes).
                let sw = (w * k).ceil().max(1.0) as u32 + 2;
                let sh = (h * k).ceil().max(1.0) as u32 + 2;
                let Some(mut scratch) = Pixmap::new(sw, sh) else {
                    return;
                };
                self.render_scratch(&mut scratch, layer, ctx, k, crate::gfx::look::of(doc) == "modern");
                let paint = PixmapPaint {
                    opacity,
                    quality: FilterQuality::Bilinear,
                    ..PixmapPaint::default()
                };
                pm.draw_pixmap(0, 0, scratch.as_ref(), &paint, local.pre_scale(1.0 / k, 1.0 / k), mask);
            }
        }
    }

    fn render_scratch(&mut self, scratch: &mut Pixmap, layer: &Layer, ctx: &RenderCtx, k: f32, modern: bool) {
        let (w, h) = (layer.w * k, layer.h * k);
        match &layer.kind {
            LayerKind::Text {
                text,
                font,
                size,
                weight,
                color,
                align,
                vcenter,
                letter_spacing,
                uppercase,
                shadow,
                wrap,
                auto_fit,
                outline,
                outline_color,
            } => {
                // A layer that is just the station name shows the station logo instead.
                if matches!(text.trim(), "{{station}}" | "{{station_short}}") {
                    let logo = ctx.bindings.get("station_logo").map(str::trim).filter(|name| !name.is_empty()).map(str::to_owned);
                    if let Some(img) = logo.and_then(|name| self.media_pixmap(ctx.media_dir, &name)) {
                        draw_logo(scratch, &img, (w, h), *align);
                        return;
                    }
                }
                let mut content = ctx.bindings.apply(text);
                if *uppercase {
                    content = content.to_uppercase();
                }
                // Animated reveal: typewriter (letters) or word by word.
                if !layer.reveal.is_empty() {
                    let secs = if layer.reveal_secs > 0.05 { layer.reveal_secs } else { 1.5 };
                    let p = ((ctx.time - layer.anim_in.delay) / secs).clamp(0.0, 1.0);
                    if layer.reveal == "words" || crate::gfx::text::has_markup(&content) {
                        let words: Vec<&str> = content.split(' ').collect();
                        let n = (p * words.len() as f32).ceil() as usize;
                        content = words[..n.min(words.len())].join(" ");
                    } else {
                        let n = (p * content.chars().count() as f32).ceil() as usize;
                        content = content.chars().take(n).collect();
                    }
                }
                let bound_color = color.contains("{{").then(|| ctx.bindings.apply(color));
                let color = bound_color.as_deref().filter(|c| !c.trim().is_empty())
                    .unwrap_or(if bound_color.is_some() { "$text" } else { color.as_str() });
                let style = TextStyle {
                    family: resolve_font(font, ctx.brand),
                    weight: *weight,
                    size: size * k * crate::gfx::text_scale(),
                    color: parse_color(color, ctx.brand),
                    letter_spacing: letter_spacing * k,
                };
                if *outline > 0.0 {
                    // Outline: the text drawn in the outline colour around a ring of offsets.
                    let mut ring = style.clone();
                    ring.color = parse_color(if outline_color.is_empty() { "#000000" } else { outline_color.as_str() }, ctx.brand);
                    let r = outline * k;
                    let steps = 12;
                    for i in 0..steps {
                        let a = i as f32 / steps as f32 * std::f32::consts::TAU;
                        let (ox, oy) = (a.cos() * r, a.sin() * r);
                        draw_text_box(scratch, &mut self.glyphs, &content, (ox, oy, w, h), &ring, *align, *vcenter, *wrap, *auto_fit, false, 1.0);
                    }
                }
                draw_text_box(
                    scratch,
                    &mut self.glyphs,
                    &content,
                    (0.0, 0.0, w, h),
                    &style,
                    *align,
                    *vcenter,
                    *wrap,
                    *auto_fit,
                    *shadow,
                    1.0,
                );
            }
            LayerKind::Icon { condition, daypart } => {
                if condition.eq_ignore_ascii_case("No icon") {
                    return;
                }
                let resolved = ctx.bindings.apply(condition);
                // Data can also say "No icon" (e.g. a holiday past the 7-day forecast).
                if resolved.trim().eq_ignore_ascii_case("No icon") {
                    return;
                }
                let kind = IconKind::ALL
                    .iter()
                    .copied()
                    .find(|k| k.label().eq_ignore_ascii_case(resolved.trim()))
                    .unwrap_or_else(|| IconKind::from_condition(&resolved));
                let day = !ctx.bindings.apply(daypart).trim().eq_ignore_ascii_case("night");
                let size = w.min(h);
                // Modern look: Meteocons artwork; Classic: the drawn icons.
                let icon = modern.then(|| crate::gfx::meteocons::raster(crate::gfx::meteocons::name(kind, day, &resolved), size)).flatten();
                match icon {
                    Some(icon) => {
                        let s = size / icon.width() as f32;
                        let paint = PixmapPaint { quality: FilterQuality::Bicubic, ..PixmapPaint::default() };
                        scratch.draw_pixmap(0, 0, (*icon).as_ref(), &paint, Transform::from_row(s, 0.0, 0.0, s, (w - size) / 2.0, (h - size) / 2.0), None);
                    }
                    None => draw_icon(scratch, kind, (w - size) / 2.0, (h - size) / 2.0, size, day),
                }
            }
            LayerKind::Chart { chart: kind, series, series2, color, color2, text_color, show_values, min, max, unit, highlight, note } => {
                let resolve = |key: &str| -> Vec<(String, f32)> {
                    let key = key.trim();
                    if key.is_empty() {
                        return Vec::new();
                    }
                    match ctx.bindings.series(key) {
                        Some(s) => s.to_vec(),
                        None => {
                            // A binding resolving to a number (gauges: "{{humidity}}"), or typed values.
                            let applied = ctx.bindings.apply(key);
                            let parsed = crate::gfx::chart::parse_inline(&applied);
                            if parsed.is_empty() {
                                let digits: String = applied.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
                                digits.parse::<f32>().map(|v| vec![(String::new(), v)]).unwrap_or_default()
                            } else {
                                parsed
                            }
                        }
                    }
                };
                let tc = if text_color.is_empty() { "$text" } else { text_color.as_str() };
                let style = crate::gfx::chart::ChartStyle {
                    color: parse_color(color, ctx.brand),
                    color2: parse_color(if color2.is_empty() { "#5EC8FF" } else { color2 }, ctx.brand),
                    text: TextStyle {
                        family: resolve_font("$body", ctx.brand),
                        weight: Weight::SemiBold,
                        size: (h / 14.0).clamp(14.0 * k, 34.0 * k),
                        color: parse_color(tc, ctx.brand),
                        letter_spacing: 0.0,
                    },
                    show_values: *show_values,
                    min: *min,
                    max: *max,
                    unit: unit.clone(),
                    // Charts with a reveal draw in over its duration (animated time series).
                    progress: if layer.reveal.is_empty() {
                        1.0
                    } else {
                        let secs = if layer.reveal_secs > 0.05 { layer.reveal_secs } else { 2.0 };
                        ((ctx.time - layer.anim_in.delay) / secs).clamp(0.0, 1.0)
                    },
                    highlight: highlight.clone(),
                    note: ctx.bindings.apply(note),
                    members: match kind {
                        crate::gfx::chart::ChartKind::Spaghetti => ctx.bindings.series_with_prefix(&format!("{}.m", series.trim())),
                        crate::gfx::chart::ChartKind::Plume => ["p10", "p25", "p50", "p75", "p90"].iter().filter_map(|p| ctx.bindings.series(&format!("{}.{p}", series.trim())).map(|s| s.iter().map(|x| x.1).collect())).collect(),
                        _ => Vec::new(),
                    },
                };
                let a = resolve(series);
                let b = resolve(series2);
                crate::gfx::chart::draw_chart(scratch, &mut self.glyphs, (0.0, 0.0, w, h), *kind, &a, &b, &style);
            }
            LayerKind::Legend { palette, text_color } => {
                let code = ctx.bindings.apply(palette);
                draw_legend(
                    scratch,
                    &mut self.glyphs,
                    code.trim(),
                    (0.0, 0.0, w, h),
                    parse_color(text_color, ctx.brand),
                    &resolve_font("$body", ctx.brand),
                );
            }
            LayerKind::Ticker {
                text,
                font,
                size,
                color,
                speed,
            } => {
                let content = ctx.bindings.apply(text);
                let style = TextStyle {
                    family: resolve_font(font, ctx.brand),
                    weight: Weight::SemiBold,
                    size: size * k,
                    color: parse_color(color, ctx.brand),
                    letter_spacing: 0.0,
                };
                let text_w = measure(&content, &style);
                let travel = text_w + w;
                // Keeps crawling past an hour (it used to freeze at 3600 s); f64 keeps the
                // offset exact on long broadcasts.
                let elapsed = if ctx.time.is_finite() { f64::from(ctx.time.max(0.0)) } else { 0.0 };
                let offset = (elapsed * f64::from(speed * k)).rem_euclid(f64::from(travel.max(1.0))) as f32;
                let (ascent, descent) = crate::gfx::text::line_metrics(&style);
                let baseline = (h - (ascent + descent)) / 2.0 + ascent;
                draw_line(
                    scratch,
                    &mut self.glyphs,
                    &content,
                    w - offset,
                    baseline,
                    &style,
                    1.0,
                    None,
                );
            }
            _ => {}
        }
    }
}

/// Parallelogram plate: the left and right edges lean by `skew` × height.
pub fn plate_path(w: f32, h: f32, skew: f32) -> Option<tiny_skia::Path> {
    let off = (skew * h).clamp(-w / 2.0, w / 2.0);
    let mut pb = PathBuilder::new();
    if off >= 0.0 {
        pb.move_to(off, 0.0);
        pb.line_to(w, 0.0);
        pb.line_to(w - off, h);
        pb.line_to(0.0, h);
    } else {
        pb.move_to(0.0, 0.0);
        pb.line_to(w + off, 0.0);
        pb.line_to(w, h);
        pb.line_to(-off, h);
    }
    pb.close();
    pb.finish()
}

/// Glossy highlight over the top of a shape: white fading out by mid-height.
fn draw_sheen(pm: &mut Pixmap, path: &tiny_skia::Path, h: f32, opacity: f32, local: Transform, mask: Option<&Mask>) {
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.shader = LinearGradient::new(
        Point::from_xy(0.0, 0.0),
        Point::from_xy(0.0, h * 0.6),
        vec![
            GradientStop::new(0.0, Color::from_rgba(1.0, 1.0, 1.0, 0.30 * opacity).unwrap_or(Color::TRANSPARENT)),
            GradientStop::new(0.55, Color::from_rgba(1.0, 1.0, 1.0, 0.08 * opacity).unwrap_or(Color::TRANSPARENT)),
            GradientStop::new(1.0, Color::TRANSPARENT),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(Color::TRANSPARENT));
    pm.fill_path(path, &paint, FillRule::Winding, local, mask);
}

/// SPC-style risk colours, level 1-5 (Marginal, Slight, Enhanced, Moderate, High).
pub const RISK_COLORS: [(u8, u8, u8); 5] = [(76, 194, 90), (246, 220, 52), (247, 150, 30), (226, 40, 40), (230, 52, 232)];
pub const RISK_SHORT: [&str; 5] = ["MRGL", "SLGT", "ENH", "MDT", "HIGH"];
/// On-air words for the five levels (SPC codes are for matching data only).
pub const RISK_WORDS: [&str; 5] = ["Marginal", "Slight", "Enhanced", "Moderate", "High"];

/// A 0-5 level from bound text: "3", "3.5", "Enhanced", "ENH", "Level 3"...
pub fn parse_level(text: &str) -> f32 {
    let t = text.trim().to_uppercase();
    if let Ok(v) = t.trim_start_matches("LEVEL").trim().parse::<f32>() {
        return v.clamp(0.0, 5.0);
    }
    for (i, (word, short)) in [("MARGINAL", "MRGL"), ("SLIGHT", "SLGT"), ("ENHANCED", "ENH"), ("MODERATE", "MDT"), ("HIGH", "HIGH")].iter().enumerate() {
        if t.contains(word) || t.split(|c: char| !c.is_alphanumeric()).any(|w| w == *short) {
            return (i + 1) as f32;
        }
    }
    0.0
}

fn risk_rgb(i: usize, alpha: f32) -> Color {
    let (r, g, b) = RISK_COLORS[i.min(4)];
    Color::from_rgba8(r, g, b, (alpha.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Draw a risk meter in the layer box. `shown` is the animated fill level, `level` the final one.
#[allow(clippy::too_many_arguments)]
fn draw_risk_meter(pm: &mut Pixmap, glyphs: &mut GlyphCache, local: Transform, mask: Option<&Mask>, w: f32, h: f32, shown: f32, level: f32, style: &str, labels: bool, opacity: f32, font: &str) {
    let dim = Color::from_rgba(0.0, 0.0, 0.0, 0.38 * opacity).unwrap_or(Color::TRANSPARENT);
    let label_at = |pm: &mut Pixmap, glyphs: &mut GlyphCache, text: &str, cx: f32, cy: f32, size: f32, max_w: f32, color: Color| {
        let mut st = TextStyle { family: font.to_string(), weight: Weight::Black, size: size * local.sy.abs().max(0.01), color, letter_spacing: 0.0 };
        let mut tw = measure(text, &st);
        // Shrink a word that would spill out of its cell (full words, not MRGL/SLGT).
        let room = max_w * local.sx.abs().max(0.01);
        if tw > room && tw > 0.0 {
            st.size *= room / tw;
            tw = measure(text, &st);
        }
        let (ascent, descent) = crate::gfx::text::line_metrics(&st);
        let px = local.tx + cx * local.sx - tw / 2.0;
        let py = local.ty + cy * local.sy - (ascent + descent) / 2.0 + ascent;
        draw_line(pm, glyphs, text, px, py, &st, opacity, None);
    };
    match style {
        "blocks" => {
            let gap = (w * 0.008).max(2.0);
            let seg = (w - gap * 9.0) / 10.0;
            for i in 0..10 {
                let x = i as f32 * (seg + gap);
                let lit = (i as f32) < (shown * 2.0).round();
                let color = if lit { risk_rgb(i / 2, opacity) } else { dim };
                if let Some(path) = rounded_rect(x, 0.0, seg, h, (h * 0.12).min(8.0)) {
                    pm.fill_path(&path, &solid_paint(color), FillRule::Winding, local, mask);
                    if lit {
                        draw_sheen(pm, &path, h, opacity, local, mask);
                    }
                }
            }
        }
        "pill" => {
            let gap = (w * 0.01).max(3.0);
            let cell = (w - gap * 4.0) / 5.0;
            let current = level.round() as usize;
            for i in 0..5 {
                let x = i as f32 * (cell + gap);
                let on = current == i + 1;
                let color = if on { risk_rgb(i, opacity) } else { risk_rgb(i, 0.28 * opacity) };
                if let Some(path) = rounded_rect(x, 0.0, cell, h, h * 0.22) {
                    pm.fill_path(&path, &solid_paint(color), FillRule::Winding, local, mask);
                    if on {
                        draw_sheen(pm, &path, h, opacity, local, mask);
                    }
                }
                let text_color = if on && i == 1 { Color::from_rgba8(20, 20, 24, 255) } else { Color::WHITE };
                label_at(pm, glyphs, &format!("{} {}", i + 1, RISK_WORDS[i]), x + cell / 2.0, h / 2.0, h * 0.34, cell * 0.9, text_color);
            }
        }
        _ => {
            // One gradient bar from Marginal green to the level's colour, rounded at the end.
            if let Some(track) = rounded_rect(0.0, 0.0, w, h, h * 0.18) {
                pm.fill_path(&track, &solid_paint(dim), FillRule::Winding, local, mask);
            }
            let fw = w * (shown / 5.0).clamp(0.0, 1.0);
            if fw > 1.0 {
                let top = ((level.ceil() as usize).clamp(1, 5)) - 1;
                if let Some(path) = rounded_rect(0.0, 0.0, fw, h, h * 0.3) {
                    let mut paint = Paint::default();
                    paint.anti_alias = true;
                    let mut stops = Vec::new();
                    for i in 0..=top {
                        let at = if top == 0 { i as f32 } else { i as f32 / top as f32 };
                        stops.push(GradientStop::new(at, risk_rgb(i, opacity)));
                    }
                    if stops.len() == 1 {
                        stops.push(GradientStop::new(1.0, risk_rgb(top, opacity)));
                    }
                    paint.shader = LinearGradient::new(Point::from_xy(0.0, 0.0), Point::from_xy(fw, 0.0), stops, SpreadMode::Pad, Transform::identity())
                        .unwrap_or(Shader::SolidColor(risk_rgb(top, opacity)));
                    pm.fill_path(&path, &paint, FillRule::Winding, local, mask);
                    draw_sheen(pm, &path, h, opacity, local, mask);
                }
            }
            if labels {
                for (i, name) in RISK_WORDS.iter().enumerate() {
                    let cx = w * (i as f32 + 0.5) / 5.0;
                    let on = shown >= i as f32 + 0.5;
                    let color = if on { Color::WHITE } else { Color::from_rgba(1.0, 1.0, 1.0, 0.35).unwrap_or(Color::WHITE) };
                    label_at(pm, glyphs, name, cx, h / 2.0, h * 0.36, w / 5.0 * 0.9, color);
                }
            }
        }
    }
}

/// "$display"/"$body" resolve to the brand fonts; anything else is a family name.
pub fn resolve_font(font: &str, brand: &Brand) -> String {
    match font {
        "$display" => brand.font_display.clone(),
        "$body" => brand.font_body.clone(),
        other => other.to_string(),
    }
}

/// Radar reflectivity legend: gradient bar, dBZ ticks and plain-language bands.
pub fn draw_legend(
    pm: &mut Pixmap,
    glyphs: &mut GlyphCache,
    palette: &str,
    rect: (f32, f32, f32, f32),
    text_color: Color,
    font: &str,
) {
    let (x, y, w, h) = rect;
    let stops = legend_stops(palette);
    if stops.is_empty() {
        return;
    }
    let min = stops[0].0 as f32;
    let max = stops[stops.len() - 1].0 as f32;
    let gradient: Vec<GradientStop> = stops
        .iter()
        .filter_map(|(dbz, hex)| {
            let color = crate::gfx::parse_hex(hex)?;
            Some(GradientStop::new((*dbz as f32 - min) / (max - min).max(1.0), color))
        })
        .collect();

    let label_h = h * 0.42;
    let bar_y = y + label_h;
    let bar_h = h * 0.32;
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.shader = LinearGradient::new(
        Point::from_xy(x, 0.0),
        Point::from_xy(x + w, 0.0),
        gradient,
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(Color::WHITE));
    if let Some(path) = rounded_rect(x, bar_y, w, bar_h, bar_h * 0.3) {
        pm.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }

    let band_style = TextStyle {
        family: font.to_string(),
        weight: Weight::Bold,
        size: label_h * 0.56,
        color: text_color,
        letter_spacing: label_h * 0.04,
    };
    let bands: [(f32, &str); 4] = [(0.0, "LIGHT"), (30.0, "MODERATE"), (45.0, "HEAVY"), (55.0, "SEVERE")];
    for (i, (from, label)) in bands.iter().enumerate() {
        let start = from.max(min);
        let end = bands.get(i + 1).map(|b| b.0).unwrap_or(max).min(max);
        if end <= start {
            continue;
        }
        let bx = x + (start - min) / (max - min) * w;
        let bw = (end - start) / (max - min) * w;
        // Keep a clear gap between neighbouring labels ("MODERATE HEAVY" read as one label):
        // shrink a label to fit its band with 24 px to spare, or leave it out.
        let tw = measure(label, &band_style);
        let room = bw - 30.0;
        let scale = (room / tw.max(1.0)).min(1.0);
        if scale >= 0.72 {
            draw_line(pm, glyphs, label, bx + (bw - tw * scale) / 2.0, y + label_h * 0.78, &band_style, scale, None);
        }
        // A thin divider at each band edge, so neighbouring labels read as separate bands.
        if i > 0 && start > min {
            if let Some(r) = Rect::from_xywh(bx - 0.75, y + label_h * 0.2, 1.5, label_h * 0.7) {
                pm.fill_rect(r, &Paint { shader: Shader::SolidColor(with_alpha(text_color, 0.35)), anti_alias: true, ..Paint::default() }, Transform::identity(), None);
            }
        }
    }

    let tick_style = TextStyle {
        family: font.to_string(),
        weight: Weight::Regular,
        size: h * 0.2,
        color: with_alpha(text_color, 0.8),
        letter_spacing: 0.0,
    };
    let mut dbz = (min / 10.0).ceil() * 10.0;
    while dbz <= max {
        let tx = x + (dbz - min) / (max - min) * w;
        let label = format!("{dbz:.0}");
        let tw = measure(&label, &tick_style);
        let lx = (tx - tw / 2.0).clamp(x, x + w - tw);
        draw_line(pm, glyphs, &label, lx, bar_y + bar_h + h * 0.22, &tick_style, 1.0, None);
        dbz += 10.0;
    }
    let _ = Align::Left;
}

/// Decode an image file into a premultiplied tiny-skia pixmap.
pub fn load_image_pixmap(path: &Path) -> Option<Pixmap> {
    let img = image::open(path).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let mut data = img.into_raw();
    for px in data.chunks_exact_mut(4) {
        let a = px[3] as u16;
        if a < 255 {
            px[0] = ((px[0] as u16 * a) / 255) as u8;
            px[1] = ((px[1] as u16 * a) / 255) as u8;
            px[2] = ((px[2] as u16 * a) / 255) as u8;
        }
    }
    Pixmap::from_vec(data, tiny_skia::IntSize::from_wh(w, h)?)
}

/// Deterministic particle field: each particle's start, speed and wobble come from a hash of its
/// index, and its position from the time, so every frame (and every export) is reproducible.
#[allow(clippy::too_many_arguments)]
fn draw_particles(pm: &mut Pixmap, local: Transform, mask: Option<&Mask>, w: f32, h: f32, effect: &str, density: f32, speed: f32, color: Color, time: f32) {
    let hash = |i: u32, salt: u32| -> f32 {
        let mut x = i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
        x ^= x >> 15;
        x = x.wrapping_mul(0x2C1B_3C6D);
        x ^= x >> 12;
        (x % 10_000) as f32 / 10_000.0
    };
    let count = ((w * h) / 10_000.0 * density.clamp(0.1, 20.0)).round().clamp(1.0, 3000.0) as u32;
    let t = time * speed.clamp(0.1, 5.0);
    let mut fill = PathBuilder::new();
    let mut lines = PathBuilder::new();
    for i in 0..count {
        let (a, b, c, d) = (hash(i, 1), hash(i, 2), hash(i, 3), hash(i, 4));
        match effect {
            "rain" => {
                let v = 900.0 + c * 500.0;
                let y = (b * h + t * v) % (h + 60.0) - 30.0;
                let x = (a * (w + 200.0) + y * 0.18) % (w + 40.0) - 20.0;
                lines.move_to(x, y);
                lines.line_to(x - 6.0, y - 28.0 - d * 14.0);
            }
            "hail" => {
                let v = 700.0 + c * 300.0;
                let y = (b * h + t * v) % (h + 40.0) - 20.0;
                let x = a * w;
                fill.push_circle(x, y, 3.0 + d * 5.0);
            }
            "sparkle" => {
                let tw = ((t * (1.5 + c * 2.0) + d * 6.283).sin() * 0.5 + 0.5).powi(3);
                if tw > 0.15 {
                    let (x, y, r) = (a * w, b * h, 1.5 + tw * 5.0);
                    lines.move_to(x - r * 2.0, y);
                    lines.line_to(x + r * 2.0, y);
                    lines.move_to(x, y - r * 2.0);
                    lines.line_to(x, y + r * 2.0);
                    fill.push_circle(x, y, r * 0.6);
                }
            }
            "leaves" => {
                let v = 60.0 + c * 70.0;
                let y = (b * h + t * v) % (h + 40.0) - 20.0;
                let x = (a * w + (t * (0.8 + d) + c * 6.0).sin() * 60.0 + t * 30.0) % (w + 40.0) - 20.0;
                if let Some(rect) = Rect::from_xywh(x - 7.0, y - 4.0, 14.0, 8.0) {
                    fill.push_oval(rect);
                }
            }
            _ => {
                // Snow: slow fall with a gentle side-to-side drift.
                let v = 40.0 + c * 80.0;
                let y = (b * h + t * v) % (h + 20.0) - 10.0;
                let x = a * w + (t * (0.6 + d) + c * 6.283).sin() * 18.0;
                fill.push_circle(x, y, 1.5 + d * 4.0);
            }
        }
    }
    let paint = solid_paint(color);
    if let Some(path) = fill.finish() {
        pm.fill_path(&path, &paint, FillRule::Winding, local, mask);
    }
    if let Some(path) = lines.finish() {
        let st = Stroke { width: if effect == "rain" { 2.0 } else { 1.6 }, line_cap: tiny_skia::LineCap::Round, ..Stroke::default() };
        pm.stroke_path(&path, &paint, &st, local, mask);
    }
}
