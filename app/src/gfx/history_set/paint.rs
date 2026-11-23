//! Small drawing kit shared by the historical graphics and the explainer drawings: an
//! offscreen canvas in design units (the layer's own w × h), scaled to the output, with
//! gradients, rounded shapes, scaled text and a state / county map fit.

use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Mask, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Point, RadialGradient, Rect,
    Shader, SpreadMode, Stroke, Transform,
};

use crate::gfx::text::{draw_line, measure, GlyphCache, TextStyle, Weight};

pub fn hex(s: &str) -> Color {
    crate::gfx::parse_hex(s).unwrap_or(Color::WHITE)
}

pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color::from_rgba8(r, g, b, (a.clamp(0., 1.) * 255.) as u8)
}

pub fn with_alpha(mut c: Color, a: f32) -> Color {
    c.set_alpha(a.clamp(0., 1.));
    c
}

/// Mix two colours (t = 0 → a, 1 → b).
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0., 1.);
    Color::from_rgba(
        a.red() + (b.red() - a.red()) * t,
        a.green() + (b.green() - a.green()) * t,
        a.blue() + (b.blue() - a.blue()) * t,
        a.alpha() + (b.alpha() - a.alpha()) * t,
    )
    .unwrap_or(a)
}

pub fn solid(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

pub fn linear(x0: f32, y0: f32, x1: f32, y1: f32, stops: &[(f32, Color)]) -> Paint<'static> {
    let st: Vec<GradientStop> = stops.iter().map(|(p, c)| GradientStop::new(*p, *c)).collect();
    let mut p = Paint::default();
    p.anti_alias = true;
    match LinearGradient::new(Point::from_xy(x0, y0), Point::from_xy(x1, y1), st, SpreadMode::Pad, Transform::identity()) {
        Some(s) => p.shader = s,
        None => p.set_color(stops.first().map(|s| s.1).unwrap_or(Color::WHITE)),
    }
    p
}

pub fn radial(cx: f32, cy: f32, r: f32, stops: &[(f32, Color)]) -> Paint<'static> {
    let st: Vec<GradientStop> = stops.iter().map(|(p, c)| GradientStop::new(*p, *c)).collect();
    let mut p = Paint::default();
    p.anti_alias = true;
    match RadialGradient::new(Point::from_xy(cx, cy), 0.0, Point::from_xy(cx, cy), r.max(0.5), st, SpreadMode::Pad, Transform::identity()) {
        Some(s) => p.shader = s,
        None => p.set_color(stops.first().map(|s| s.1).unwrap_or(Color::WHITE)),
    }
    p
}

pub fn shader_of(p: &Paint<'static>) -> Shader<'static> {
    p.shader.clone()
}

/// Rounded rectangle path.
pub fn rrect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.).min(h / 2.).max(0.);
    let mut pb = PathBuilder::new();
    if r <= 0.01 {
        pb.push_rect(Rect::from_xywh(x, y, w.max(0.01), h.max(0.01))?);
        return pb.finish();
    }
    let k = 0.5523 * r;
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

pub fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.push_oval(Rect::from_xywh(cx - rx, cy - ry, (rx * 2.).max(0.01), (ry * 2.).max(0.01))?);
    pb.finish()
}

pub fn poly(pts: &[(f32, f32)], closed: bool) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for (i, &(x, y)) in pts.iter().enumerate() {
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    if closed {
        pb.close();
    }
    pb.finish()
}

/// A smooth closed blob through the points (Catmull-Rom → cubic).
pub fn smooth(pts: &[(f32, f32)], closed: bool) -> Option<Path> {
    let n = pts.len();
    if n < 3 {
        return poly(pts, closed);
    }
    let get = |i: isize| -> (f32, f32) {
        if closed {
            pts[((i % n as isize) + n as isize) as usize % n]
        } else {
            pts[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].0, pts[0].1);
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs as isize {
        let (p0, p1, p2, p3) = (get(i - 1), get(i), get(i + 1), get(i + 2));
        let c1 = (p1.0 + (p2.0 - p0.0) / 6., p1.1 + (p2.1 - p0.1) / 6.);
        let c2 = (p2.0 - (p3.0 - p1.0) / 6., p2.1 - (p3.1 - p1.1) / 6.);
        pb.cubic_to(c1.0, c1.1, c2.0, c2.1, p2.0, p2.1);
    }
    if closed {
        pb.close();
    }
    pb.finish()
}

#[derive(Clone, Copy, PartialEq)]
pub enum Al {
    Left,
    Center,
    Right,
}

/// Offscreen canvas in design units.
pub struct Canvas<'a> {
    pub pm: Pixmap,
    pub g: &'a mut GlyphCache,
    pub s: f32,
    pub w: f32,
    pub h: f32,
    pub font: String,
    /// Optional clip (e.g. the state outline) for fills and strokes.
    pub clip: Option<Mask>,
}

impl<'a> Canvas<'a> {
    pub fn new(g: &'a mut GlyphCache, w: f32, h: f32, local: Transform, font: &str) -> Option<Self> {
        let dw = (w * local.sx.abs()).round().clamp(1., 4096.) as u32;
        let dh = (h * local.sy.abs()).round().clamp(1., 4096.) as u32;
        let pm = Pixmap::new(dw, dh)?;
        Some(Self { pm, g, s: dw as f32 / w.max(1.), w, h, font: font.to_string(), clip: None })
    }

    /// Clip later drawing to path (None clears it).
    pub fn set_clip(&mut self, path: Option<&Path>) {
        self.clip = path.and_then(|p| {
            let mut m = Mask::new(self.pm.width(), self.pm.height())?;
            m.fill_path(p, FillRule::Winding, true, self.ts());
            Some(m)
        });
    }

    pub fn ts(&self) -> Transform {
        Transform::from_scale(self.s, self.s)
    }

    pub fn fill(&mut self, path: Option<Path>, paint: &Paint) {
        if let Some(p) = path {
            let ts = self.ts();
            self.pm.fill_path(&p, paint, FillRule::EvenOdd, ts, self.clip.as_ref());
        }
    }

    pub fn fill_c(&mut self, path: Option<Path>, c: Color) {
        self.fill(path, &solid(c));
    }

    pub fn stroke(&mut self, path: Option<Path>, c: Color, width: f32) {
        self.stroke_p(path, &solid(c), width);
    }

    pub fn stroke_p(&mut self, path: Option<Path>, paint: &Paint, width: f32) {
        if let Some(p) = path {
            let st = Stroke { width, line_cap: tiny_skia::LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
            let ts = self.ts();
            self.pm.stroke_path(&p, paint, &st, ts, self.clip.as_ref());
        }
    }

    pub fn dashed(&mut self, path: Option<Path>, c: Color, width: f32, on: f32, off: f32) {
        if let Some(p) = path {
            let st = Stroke { width, line_cap: tiny_skia::LineCap::Butt, dash: tiny_skia::StrokeDash::new(vec![on, off], 0.), ..Default::default() };
            let ts = self.ts();
            self.pm.stroke_path(&p, &solid(c), &st, ts, None);
        }
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Color) {
        self.fill(rrect(x, y, w, h, 0.), &solid(c));
    }

    pub fn round(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, paint: &Paint) {
        self.fill(rrect(x, y, w, h, r), paint);
    }

    pub fn line(&mut self, a: (f32, f32), b: (f32, f32), c: Color, width: f32) {
        self.stroke(poly(&[a, b], false), c, width);
    }

    /// Arrow from a to b with a filled head.
    pub fn arrow(&mut self, a: (f32, f32), b: (f32, f32), c: Color, width: f32) {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt().max(0.01);
        let (ux, uy) = (dx / len, dy / len);
        let head = width * 3.2;
        let base = (b.0 - ux * head, b.1 - uy * head);
        self.line(a, base, c, width);
        let (nx, ny) = (-uy * head * 0.6, ux * head * 0.6);
        self.fill_c(poly(&[b, (base.0 + nx, base.1 + ny), (base.0 - nx, base.1 - ny)], true), c);
    }

    /// Curved arrow along a quadratic (a → control → b).
    pub fn curved_arrow(&mut self, a: (f32, f32), ctl: (f32, f32), b: (f32, f32), c: Color, width: f32) {
        let mut pts = Vec::new();
        for i in 0..=24 {
            let t = i as f32 / 24.;
            let u = 1. - t;
            pts.push((u * u * a.0 + 2. * u * t * ctl.0 + t * t * b.0, u * u * a.1 + 2. * u * t * ctl.1 + t * t * b.1));
        }
        let head = width * 3.2;
        let end = pts[pts.len() - 1];
        let prev = pts[pts.len() - 4];
        let (dx, dy) = (end.0 - prev.0, end.1 - prev.1);
        let len = (dx * dx + dy * dy).sqrt().max(0.01);
        let (ux, uy) = (dx / len, dy / len);
        let base = (end.0 - ux * head, end.1 - uy * head);
        let mut body: Vec<(f32, f32)> = pts[..pts.len() - 3].to_vec();
        body.push(base);
        self.stroke(poly(&body, false), c, width);
        let (nx, ny) = (-uy * head * 0.6, ux * head * 0.6);
        self.fill_c(poly(&[end, (base.0 + nx, base.1 + ny), (base.0 - nx, base.1 - ny)], true), c);
    }

    pub fn style(&self, size: f32, weight: Weight, c: Color) -> TextStyle {
        TextStyle { family: self.font.clone(), weight, size: size * self.s, color: c, letter_spacing: 0. }
    }

    pub fn measure(&self, text: &str, size: f32, weight: Weight) -> f32 {
        measure(text, &self.style(size, weight, Color::WHITE)) / self.s
    }

    /// One line of text at baseline `y`, shrunk to fit `max_w` (0 = no limit).
    #[allow(clippy::too_many_arguments)]
    pub fn text(&mut self, text: &str, x: f32, y: f32, size: f32, weight: Weight, c: Color, al: Al, max_w: f32) {
        let mut st = self.style(size, weight, c);
        let mut tw = measure(text, &st);
        if max_w > 0. && tw > max_w * self.s {
            st.size *= max_w * self.s / tw;
            tw = measure(text, &st);
        }
        let x0 = match al {
            Al::Left => x * self.s,
            Al::Center => x * self.s - tw / 2.,
            Al::Right => x * self.s - tw,
        };
        let clip = Some((0, 0, self.pm.width() as i32, self.pm.height() as i32));
        draw_line(&mut self.pm, self.g, text, x0, y * self.s, &st, 1., clip);
    }

    /// Text with a dark halo so it reads on any map colour.
    #[allow(clippy::too_many_arguments)]
    pub fn text_halo(&mut self, text: &str, x: f32, y: f32, size: f32, weight: Weight, c: Color, al: Al) {
        let halo = rgba(0, 0, 0, 0.75);
        for (dx, dy) in [(-1.6, 0.), (1.6, 0.), (0., -1.6), (0., 1.6), (-1.1, -1.1), (1.1, 1.1), (-1.1, 1.1), (1.1, -1.1)] {
            self.text(text, x + dx, y + dy, size, weight, halo, al, 0.);
        }
        self.text(text, x, y, size, weight, c, al, 0.);
    }

    /// Calm "not available" state in the middle of the box.
    pub fn missing(&mut self, what: &str) {
        let (w, h) = (self.w, self.h);
        self.round(w * 0.12, h * 0.40, w * 0.76, h * 0.2, 18., &solid(rgba(8, 18, 34, 0.82)));
        self.text(what, w / 2., h * 0.49, (h * 0.045).clamp(18., 40.), Weight::SemiBold, Color::WHITE, Al::Center, w * 0.7);
        self.text("Data not available yet", w / 2., h * 0.555, (h * 0.032).clamp(14., 28.), Weight::Regular, rgba(200, 214, 232, 1.), Al::Center, w * 0.7);
    }

    pub fn finish(self, pm: &mut Pixmap, local: Transform, mask: Option<&Mask>, opacity: f32) {
        pm.draw_pixmap(
            0,
            0,
            self.pm.as_ref(),
            &PixmapPaint { opacity: opacity.clamp(0., 1.), ..Default::default() },
            Transform::from_translate(local.tx, local.ty),
            mask,
        );
    }
}

/// A state's counties fitted into a box (design units).
pub struct StateMap {
    /// (name, 5-digit FIPS, rings)
    pub counties: Vec<(String, String, Vec<Vec<(f64, f64)>>)>,
    west: f64,
    north: f64,
    k: f64,
    scale: f64,
    ox: f64,
    oy: f64,
}

impl StateMap {
    pub fn new(state_fips: &str, x: f32, y: f32, w: f32, h: f32) -> Option<Self> {
        let all = crate::map::boundaries::counties_named()?;
        let counties: Vec<_> = all.iter().filter(|c| c.state == state_fips).map(|c| (c.name.clone(), c.fips.clone(), c.rings.clone())).collect();
        if counties.is_empty() {
            return None;
        }
        let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for (_, _, rings) in &counties {
            for &(lon, lat) in rings.iter().flatten() {
                b = (b.0.min(lon), b.1.min(lat), b.2.max(lon), b.3.max(lat));
            }
        }
        let k = ((b.1 + b.3) / 2.).to_radians().cos();
        let (gw, gh) = ((b.2 - b.0) * k, b.3 - b.1);
        let scale = (w as f64 / gw).min(h as f64 / gh);
        Some(Self { counties, west: b.0, north: b.3, k, scale, ox: x as f64 + (w as f64 - gw * scale) / 2., oy: y as f64 + (h as f64 - gh * scale) / 2. })
    }

    pub fn to(&self, lon: f64, lat: f64) -> (f32, f32) {
        ((self.ox + (lon - self.west) * self.k * self.scale) as f32, (self.oy + (self.north - lat) * self.scale) as f32)
    }

    pub fn path(&self, rings: &[Vec<(f64, f64)>]) -> Option<Path> {
        let mut pb = PathBuilder::new();
        for ring in rings {
            for (i, &(lon, lat)) in ring.iter().enumerate() {
                let (x, y) = self.to(lon, lat);
                if i == 0 {
                    pb.move_to(x, y);
                } else {
                    pb.line_to(x, y);
                }
            }
            pb.close();
        }
        pb.finish()
    }

    /// Label point: the average of the county's outer ring.
    pub fn centre(rings: &[Vec<(f64, f64)>]) -> (f64, f64) {
        let ring = rings.iter().max_by_key(|r| r.len());
        let Some(r) = ring else { return (0., 0.) };
        let n = r.len().max(1) as f64;
        (r.iter().map(|p| p.0).sum::<f64>() / n, r.iter().map(|p| p.1).sum::<f64>() / n)
    }

    /// Land fill with a soft drop shadow, then county lines.
    pub fn base(&self, cv: &mut Canvas, land: Color, lines: Color) {
        for (_, _, rings) in &self.counties {
            let p = self.path(rings).map(|p| p.transform(Transform::from_translate(6., 8.))).and_then(|p| p);
            cv.fill_c(p, rgba(0, 0, 0, 0.35));
        }
        for (_, _, rings) in &self.counties {
            cv.fill_c(self.path(rings), land);
        }
        self.lines(cv, lines, 1.4);
    }

    pub fn lines(&self, cv: &mut Canvas, c: Color, width: f32) {
        for (_, _, rings) in &self.counties {
            cv.stroke(self.path(rings), c, width);
        }
    }

    pub fn outline(&self, cv: &mut Canvas, c: Color, width: f32) {
        // The state edge = county edges drawn thick underneath, then the county lines over.
        for (_, _, rings) in &self.counties {
            cv.stroke(self.path(rings), c, width);
        }
    }
}
