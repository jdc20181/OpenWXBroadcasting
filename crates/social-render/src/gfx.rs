//! Drawing helpers shared by every graphic: colours, text, icons, images and shapes.
//! All coordinates are in "design pixels" (the graphic's 1x size); the canvas multiplies
//! by its scale so a 1200 x 630 card can be drawn at 2400 x 1260.

use base64::Engine;
use fontdue::{Font, FontSettings};
use resvg::tiny_skia::{
    self, Color, FillRule, GradientStop, LinearGradient, Mask, Paint, PathBuilder, Pixmap, PixmapPaint,
    Point, RadialGradient, Rect, Shader, SpreadMode, Stroke, Transform,
};
use std::cell::RefCell;
use std::collections::HashMap;

// ---------------------------------------------------------------- colours ---

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Rgba { r, g, b, a: 1.0 }
    }
    pub fn alpha(self, a: f32) -> Self {
        Rgba { a: (self.a * a).clamp(0.0, 1.0), ..self }
    }
    pub fn color(self) -> Color {
        Color::from_rgba8(self.r, self.g, self.b, (self.a * 255.0).round() as u8)
    }
    pub fn mix(self, other: Rgba, t: f32) -> Rgba {
        let l = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgba { r: l(self.r, other.r), g: l(self.g, other.g), b: l(self.b, other.b), a: self.a + (other.a - self.a) * t }
    }
    /// WCAG relative luminance.
    pub fn luminance(self) -> f32 {
        let ch = |c: u8| {
            let c = c as f32 / 255.0;
            if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(self.r) + 0.7152 * ch(self.g) + 0.0722 * ch(self.b)
    }
    pub fn contrast(self, other: Rgba) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
}

pub const WHITE: Rgba = Rgba::rgb(255, 255, 255);
pub const INK_DARK: Rgba = Rgba::rgb(17, 20, 26);

/// Text colour for large bold type on `bg`: white while it meets the 3:1 large-text contrast
/// (red, purple, dark colours), near-black on pale colours (yellow watches, moccasin statements).
pub fn text_on(bg: Rgba) -> Rgba {
    if WHITE.contrast(bg) >= 3.0 { WHITE } else { INK_DARK }
}

/// Parses #rgb, #rrggbb, #rrggbbaa, rgb(..) and rgba(..). Anything else gives `fallback`.
pub fn css(s: &str, fallback: Rgba) -> Rgba {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        let v = |i: usize, n: usize| u8::from_str_radix(&hex[i..i + n], 16).ok();
        let dup = |i: usize| u8::from_str_radix(&hex[i..i + 1].repeat(2), 16).ok();
        let parsed = match hex.len() {
            3 => (dup(0), dup(1), dup(2), Some(255)),
            6 => (v(0, 2), v(2, 2), v(4, 2), Some(255)),
            8 => (v(0, 2), v(2, 2), v(4, 2), v(6, 2)),
            _ => (None, None, None, None),
        };
        if let (Some(r), Some(g), Some(b), Some(a)) = parsed {
            return Rgba { r, g, b, a: a as f32 / 255.0 };
        }
        return fallback;
    }
    let lower = s.to_ascii_lowercase();
    let inner = lower
        .strip_prefix("rgba(")
        .or_else(|| lower.strip_prefix("rgb("))
        .and_then(|x| x.strip_suffix(')'));
    if let Some(inner) = inner {
        let parts: Vec<f32> = inner.split(|c| c == ',' || c == '/' || c == ' ').filter(|p| !p.is_empty()).filter_map(|p| p.trim().parse().ok()).collect();
        if parts.len() >= 3 {
            return Rgba {
                r: parts[0].clamp(0.0, 255.0) as u8,
                g: parts[1].clamp(0.0, 255.0) as u8,
                b: parts[2].clamp(0.0, 255.0) as u8,
                a: parts.get(3).copied().unwrap_or(1.0).clamp(0.0, 1.0),
            };
        }
    }
    fallback
}

// ------------------------------------------------------------------ fonts ---

/// Barlow Condensed, the font the site already uses for headings (SIL Open Font License).
static FONT_FILES: [(u16, &[u8]); 5] = [
    (500, include_bytes!("../assets/fonts/BarlowCondensed-500.ttf")),
    (600, include_bytes!("../assets/fonts/BarlowCondensed-600.ttf")),
    (700, include_bytes!("../assets/fonts/BarlowCondensed-700.ttf")),
    (800, include_bytes!("../assets/fonts/BarlowCondensed-800.ttf")),
    (900, include_bytes!("../assets/fonts/BarlowCondensed-900.ttf")),
];

pub struct Fonts {
    fonts: Vec<(u16, Font)>,
    glyphs: RefCell<HashMap<(u16, char, u32), (fontdue::Metrics, Vec<u8>)>>,
}

impl Fonts {
    pub fn load() -> Fonts {
        let fonts = FONT_FILES
            .iter()
            .map(|(w, bytes)| (*w, Font::from_bytes(*bytes, FontSettings::default()).expect("embedded font")))
            .collect();
        Fonts { fonts, glyphs: RefCell::new(HashMap::new()) }
    }

    fn font(&self, weight: u16) -> &Font {
        self.fonts
            .iter()
            .min_by_key(|(w, _)| (*w as i32 - weight as i32).abs())
            .map(|(_, f)| f)
            .unwrap()
    }
}

/// How a run of text looks.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub px: f32,
    pub weight: u16,
    pub color: Rgba,
    /// Extra space between letters, as a fraction of the size (CSS letter-spacing in em).
    pub tracking: f32,
}

impl Style {
    pub fn new(px: f32, weight: u16, color: Rgba) -> Style {
        Style { px, weight, color, tracking: 0.0 }
    }
    pub fn track(mut self, em: f32) -> Style {
        self.tracking = em;
        self
    }
    pub fn color(mut self, c: Rgba) -> Style {
        self.color = c;
        self
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
}

// ----------------------------------------------------------------- canvas ---

pub struct Canvas<'f> {
    pub pm: Pixmap,
    pub scale: f32,
    pub w: f32,
    pub h: f32,
    fonts: &'f Fonts,
    clip: Option<Mask>,
}

impl<'f> Canvas<'f> {
    pub fn new(w: f32, h: f32, scale: f32, fonts: &'f Fonts) -> Canvas<'f> {
        let pm = Pixmap::new((w * scale).round() as u32, (h * scale).round() as u32).expect("canvas size");
        Canvas { pm, scale, w, h, fonts, clip: None }
    }

    fn t(&self) -> Transform {
        Transform::from_scale(self.scale, self.scale)
    }

    /// Restrict drawing to a rounded rectangle until `unclip`.
    pub fn clip_rrect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        let mut mask = Mask::new(self.pm.width(), self.pm.height()).unwrap();
        if let Some(path) = rrect_path(x, y, w, h, r) {
            mask.fill_path(&path, FillRule::Winding, true, self.t());
        }
        self.clip = Some(mask);
    }
    pub fn unclip(&mut self) {
        self.clip = None;
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Rgba) {
        self.fill_rrect(x, y, w, h, 0.0, c);
    }

    pub fn fill_rrect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgba) {
        if let Some(path) = rrect_path(x, y, w, h, r) {
            let mut paint = Paint::default();
            paint.set_color(c.color());
            paint.anti_alias = true;
            let t = self.t();
            self.pm.fill_path(&path, &paint, FillRule::Winding, t, self.clip.as_ref());
        }
    }

    pub fn stroke_rrect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, width: f32, c: Rgba) {
        if let Some(path) = rrect_path(x, y, w, h, r) {
            let mut paint = Paint::default();
            paint.set_color(c.color());
            paint.anti_alias = true;
            let stroke = Stroke { width, ..Stroke::default() };
            let t = self.t();
            self.pm.stroke_path(&path, &paint, &stroke, t, self.clip.as_ref());
        }
    }

    /// Fill a rounded rectangle with a gradient. `vertical` runs top to bottom, else left to right.
    pub fn gradient_rrect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, stops: &[(f32, Rgba)], vertical: bool) {
        let (p0, p1) = if vertical { (Point::from_xy(x, y), Point::from_xy(x, y + h)) } else { (Point::from_xy(x, y), Point::from_xy(x + w, y)) };
        let stops: Vec<GradientStop> = stops.iter().map(|(p, c)| GradientStop::new(*p, c.color())).collect();
        if let (Some(path), Some(shader)) = (rrect_path(x, y, w, h, r), LinearGradient::new(p0, p1, stops, SpreadMode::Pad, Transform::identity())) {
            self.fill_shader(&path, shader);
        }
    }

    /// A soft round glow (stands in for a CSS blur).
    pub fn glow(&mut self, cx: f32, cy: f32, radius: f32, c: Rgba) {
        let stops = vec![
            GradientStop::new(0.0, c.color()),
            GradientStop::new(0.45, c.alpha(0.55).color()),
            GradientStop::new(1.0, c.alpha(0.0).color()),
        ];
        if let Some(shader) = RadialGradient::new(Point::from_xy(cx, cy), 0.0, Point::from_xy(cx, cy), radius, stops, SpreadMode::Pad, Transform::identity()) {
            if let Some(path) = rrect_path(cx - radius, cy - radius, radius * 2.0, radius * 2.0, 0.0) {
                self.fill_shader(&path, shader);
            }
        }
    }

    /// Darkens the edges of a rectangle (map vignette).
    pub fn vignette(&mut self, x: f32, y: f32, w: f32, h: f32, strength: f32) {
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let r = (w * w + h * h).sqrt() / 2.0;
        let stops = vec![
            GradientStop::new(0.0, Color::from_rgba8(0, 0, 0, 0)),
            GradientStop::new(0.55, Color::from_rgba8(0, 0, 0, 0)),
            GradientStop::new(1.0, Rgba::rgb(4, 7, 12).alpha(strength).color()),
        ];
        if let (Some(shader), Some(path)) = (
            RadialGradient::new(Point::from_xy(cx, cy), 0.0, Point::from_xy(cx, cy), r, stops, SpreadMode::Pad, Transform::identity()),
            rrect_path(x, y, w, h, 0.0),
        ) {
            self.fill_shader(&path, shader);
        }
    }

    fn fill_shader(&mut self, path: &tiny_skia::Path, shader: Shader) {
        let paint = Paint { shader, anti_alias: true, ..Paint::default() };
        let t = self.t();
        self.pm.fill_path(path, &paint, FillRule::Winding, t, self.clip.as_ref());
    }

    pub fn fill_path(&mut self, path: &tiny_skia::Path, c: Rgba) {
        let mut paint = Paint::default();
        paint.set_color(c.color());
        paint.anti_alias = true;
        let t = self.t();
        self.pm.fill_path(path, &paint, FillRule::EvenOdd, t, self.clip.as_ref());
    }

    pub fn stroke_path(&mut self, path: &tiny_skia::Path, width: f32, c: Rgba) {
        let mut paint = Paint::default();
        paint.set_color(c.color());
        paint.anti_alias = true;
        let stroke = Stroke {
            width,
            line_join: tiny_skia::LineJoin::Round,
            line_cap: tiny_skia::LineCap::Round,
            ..Stroke::default()
        };
        let t = self.t();
        self.pm.stroke_path(path, &paint, &stroke, t, self.clip.as_ref());
    }

    pub fn dashed_line(&mut self, x0: f32, y: f32, x1: f32, width: f32, dash: f32, c: Rgba) {
        let mut pb = PathBuilder::new();
        pb.move_to(x0, y);
        pb.line_to(x1, y);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(c.color());
            let stroke = Stroke { width, dash: tiny_skia::StrokeDash::new(vec![dash, dash * 0.8], 0.0), ..Stroke::default() };
            let t = self.t();
            self.pm.stroke_path(&path, &paint, &stroke, t, self.clip.as_ref());
        }
    }

    // ------------------------------------------------------------- text ---

    pub fn measure(&self, text: &str, s: &Style) -> f32 {
        let font = self.fonts.font(s.weight);
        let mut w = 0.0;
        let mut prev: Option<char> = None;
        for ch in text.chars() {
            let m = font.metrics(ch, s.px);
            if let Some(p) = prev {
                w += font.horizontal_kern(p, ch, s.px).unwrap_or(0.0);
            }
            w += m.advance_width + s.tracking * s.px;
            prev = Some(ch);
        }
        if prev.is_some() {
            w -= s.tracking * s.px;
        }
        w
    }

    /// Height from the top of capitals to the baseline, for vertical centring.
    pub fn cap_height(&self, s: &Style) -> f32 {
        let m = self.fonts.font(s.weight).metrics('H', s.px);
        m.height as f32 + m.ymin as f32
    }

    /// Draws one line with its baseline at `y`. Returns the width drawn.
    pub fn text(&mut self, x: f32, y: f32, text: &str, s: &Style, align: Align) -> f32 {
        let width = self.measure(text, s);
        let mut pen = match align {
            Align::Left => x,
            Align::Center => x - width / 2.0,
            Align::Right => x - width,
        };
        let px = s.px * self.scale;
        let font = self.fonts.font(s.weight);
        let mut prev: Option<char> = None;
        for ch in text.chars() {
            if let Some(p) = prev {
                pen += font.horizontal_kern(p, ch, s.px).unwrap_or(0.0);
            }
            let key = (s.weight, ch, px.to_bits());
            let (m, bitmap) = {
                let mut cache = self.fonts.glyphs.borrow_mut();
                cache.entry(key).or_insert_with(|| font.rasterize(ch, px)).clone()
            };
            let gx = (pen * self.scale).round() as i32 + m.xmin;
            let gy = (y * self.scale).round() as i32 - m.height as i32 - m.ymin;
            self.blit_coverage(gx, gy, m.width, m.height, &bitmap, s.color);
            pen += font.metrics(ch, s.px).advance_width + s.tracking * s.px;
            prev = Some(ch);
        }
        width
    }

    /// Text with a dark outline so it reads over radar and maps.
    pub fn text_halo(&mut self, x: f32, y: f32, text: &str, s: &Style, align: Align, halo: Rgba, r: f32) {
        let shadow = s.color(halo);
        for (dx, dy) in [(-r, 0.0), (r, 0.0), (0.0, -r), (0.0, r), (-r * 0.7, -r * 0.7), (r * 0.7, r * 0.7), (-r * 0.7, r * 0.7), (r * 0.7, -r * 0.7)] {
            self.text(x + dx, y + dy, text, &shadow, align);
        }
        self.text(x, y, text, s, align);
    }

    fn blit_coverage(&mut self, gx: i32, gy: i32, w: usize, h: usize, cov: &[u8], c: Rgba) {
        let (pw, ph) = (self.pm.width() as i32, self.pm.height() as i32);
        let clip = self.clip.as_ref().map(|m| m.data().to_vec());
        let data = self.pm.data_mut();
        for row in 0..h as i32 {
            let py = gy + row;
            if py < 0 || py >= ph {
                continue;
            }
            for col in 0..w as i32 {
                let px = gx + col;
                if px < 0 || px >= pw {
                    continue;
                }
                let mut a = cov[(row * w as i32 + col) as usize] as f32 / 255.0 * c.a;
                if let Some(m) = &clip {
                    a *= m[(py * pw + px) as usize] as f32 / 255.0;
                }
                if a <= 0.0 {
                    continue;
                }
                let i = ((py * pw + px) * 4) as usize;
                let inv = 1.0 - a;
                // Premultiplied source-over.
                data[i] = (c.r as f32 * a + data[i] as f32 * inv).round() as u8;
                data[i + 1] = (c.g as f32 * a + data[i + 1] as f32 * inv).round() as u8;
                data[i + 2] = (c.b as f32 * a + data[i + 2] as f32 * inv).round() as u8;
                data[i + 3] = (255.0 * a + data[i + 3] as f32 * inv).round() as u8;
            }
        }
    }

    /// Splits text into lines no wider than `max_w`, breaking only between words.
    pub fn wrap(&self, text: &str, s: &Style, max_w: f32) -> Vec<String> {
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in text.split_whitespace() {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if self.measure(&candidate, s) <= max_w || line.is_empty() {
                line = candidate;
            } else {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
        lines
    }

    /// Largest size between `s.px` and `min_px` at which the text fits in `max_lines` lines of
    /// `max_w`. If it still doesn't fit at `min_px`, the last line ends on a whole word + "…".
    pub fn fit(&self, text: &str, s: &Style, min_px: f32, max_w: f32, max_lines: usize) -> (Style, Vec<String>) {
        let mut style = *s;
        loop {
            let lines = self.wrap(text, &style, max_w);
            let widest = lines.iter().map(|l| self.measure(l, &style)).fold(0.0, f32::max);
            if lines.len() <= max_lines && widest <= max_w + 0.5 {
                return (style, lines);
            }
            if style.px - 1.0 < min_px {
                let mut lines = lines;
                lines.truncate(max_lines);
                if let Some(last) = lines.last_mut() {
                    let mut words: Vec<&str> = last.split_whitespace().collect();
                    while words.len() > 1 && self.measure(&format!("{}…", words.join(" ")), &style) > max_w {
                        words.pop();
                    }
                    *last = format!("{}…", words.join(" ").trim_end_matches([',', ';', ':', '.']));
                }
                return (style, lines);
            }
            style.px -= 1.0;
        }
    }

    /// Draws wrapped lines from `top`, returns the y below the block.
    pub fn paragraph(&mut self, x: f32, top: f32, lines: &[String], s: &Style, leading: f32, align: Align) -> f32 {
        let lh = s.px * leading;
        let cap = self.cap_height(s);
        let mut y = top;
        for line in lines {
            self.text(x, y + cap + (lh - s.px) / 2.0, line, s, align);
            y += lh;
        }
        y
    }

    // ------------------------------------------------------ icons/images ---

    /// A Remix icon, tinted, in a `size` square.
    pub fn icon(&mut self, name: &str, x: f32, y: f32, size: f32, c: Rgba) {
        let Some(svg) = icon_svg(name) else { return };
        let hex = format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b);
        let svg = svg.replace("currentColor", &hex).replacen("<svg ", &format!("<svg fill-opacity=\"{}\" ", c.a), 1);
        self.svg(svg.as_bytes(), x, y, size, size, 1.0);
    }

    /// An SVG fitted (contain) in a box.
    pub fn svg(&mut self, data: &[u8], x: f32, y: f32, w: f32, h: f32, opacity: f32) -> bool {
        let opt = resvg::usvg::Options::default();
        let Ok(tree) = resvg::usvg::Tree::from_data(data, &opt) else { return false };
        let size = tree.size();
        let k = (w / size.width()).min(h / size.height());
        let (dw, dh) = (size.width() * k, size.height() * k);
        let (ox, oy) = (x + (w - dw) / 2.0, y + (h - dh) / 2.0);
        let (pw, ph) = ((w * self.scale).ceil() as u32 + 2, (h * self.scale).ceil() as u32 + 2);
        let Some(mut layer) = Pixmap::new(pw.max(1), ph.max(1)) else { return false };
        let t = Transform::from_scale(k * self.scale, k * self.scale)
            .post_translate((ox - x) * self.scale, (oy - y) * self.scale);
        resvg::render(&tree, t, &mut layer.as_mut());
        let paint = PixmapPaint { opacity, ..PixmapPaint::default() };
        self.pm.draw_pixmap(
            (x * self.scale).round() as i32,
            (y * self.scale).round() as i32,
            layer.as_ref(),
            &paint,
            Transform::identity(),
            self.clip.as_ref(),
        );
        true
    }

    /// A PNG / JPEG / WebP / GIF / SVG image fitted (contain) in a box, from raw bytes.
    pub fn image(&mut self, bytes: &[u8], x: f32, y: f32, w: f32, h: f32, opacity: f32, align: Align) -> Option<(f32, f32)> {
        if looks_like_svg(bytes) {
            return self.svg(bytes, x, y, w, h, opacity).then_some((w, h));
        }
        let img = decode(bytes)?;
        let k = (w / img.width() as f32).min(h / img.height() as f32);
        let (dw, dh) = (img.width() as f32 * k, img.height() as f32 * k);
        let ox = match align {
            Align::Left => x,
            Align::Center => x + (w - dw) / 2.0,
            Align::Right => x + w - dw,
        };
        let oy = y + (h - dh) / 2.0;
        let t = Transform::from_scale(k * self.scale, k * self.scale).post_translate(ox * self.scale, oy * self.scale);
        let paint = PixmapPaint { opacity, quality: tiny_skia::FilterQuality::Bicubic, ..PixmapPaint::default() };
        self.pm.draw_pixmap(0, 0, img.as_ref(), &paint, t, self.clip.as_ref());
        Some((dw, dh))
    }

    /// Draws an already-decoded pixmap with a transform in design pixels.
    pub fn draw_pixmap(&mut self, img: &Pixmap, t: Transform, opacity: f32) {
        let t = t.post_scale(self.scale, self.scale);
        // Bicubic: map tiles are scaled by up to 2x either way, and bilinear left them soft.
        let paint = PixmapPaint { opacity, quality: tiny_skia::FilterQuality::Bicubic, ..PixmapPaint::default() };
        self.pm.draw_pixmap(0, 0, img.as_ref(), &paint, t, self.clip.as_ref());
    }

    pub fn png(&self) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(self.pm.data().len());
        for p in self.pm.pixels() {
            let c = p.demultiply();
            rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
        }
        let mut out = Vec::new();
        let enc = image::codecs::png::PngEncoder::new_with_quality(&mut out, image::codecs::png::CompressionType::Fast, image::codecs::png::FilterType::Adaptive);
        image::ImageEncoder::write_image(enc, &rgba, self.pm.width(), self.pm.height(), image::ExtendedColorType::Rgba8).expect("png encode");
        out
    }
}

pub fn rrect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    if r == 0.0 {
        return Some(PathBuilder::from_rect(Rect::from_xywh(x, y, w, h)?));
    }
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
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

pub fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_ascii_lowercase();
    head.contains("<svg") || head.trim_start().starts_with("<?xml")
}

/// Raw image bytes to a premultiplied pixmap.
pub fn decode(bytes: &[u8]) -> Option<Pixmap> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let mut pm = Pixmap::new(w, h)?;
    for (dst, src) in pm.pixels_mut().iter_mut().zip(img.pixels()) {
        let [r, g, b, a] = src.0;
        *dst = tiny_skia::ColorU8::from_rgba(r, g, b, a).premultiply();
    }
    Some(pm)
}

/// Accepts a data: URI or plain base64.
pub fn b64(data: &str) -> Option<Vec<u8>> {
    let raw = match data.find("base64,") {
        Some(i) => &data[i + 7..],
        None if data.starts_with("data:") => {
            // data:image/svg+xml;utf8,<svg ...>
            let i = data.find(',')?;
            return Some(data[i + 1..].as_bytes().to_vec());
        }
        None => data,
    };
    base64::engine::general_purpose::STANDARD.decode(raw.trim()).ok()
}

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        fn icon_svg(name: &str) -> Option<String> {
            match name {
                $($name => Some(String::from_utf8_lossy(include_bytes!(concat!("../assets/icons/", $name, ".svg"))).into_owned()),)*
                _ => None,
            }
        }
        #[cfg(test)]
        pub const ICON_NAMES: &[&str] = &[$($name),*];
    };
}

icons!(
    "alert-fill", "checkbox-circle-fill", "cloud-windy-fill", "cloudy-2-fill", "cloudy-fill", "drizzle-fill",
    "error-warning-fill", "fire-fill", "flag-fill", "flood-fill", "foggy-fill", "gift-fill", "hail-fill",
    "haze-2-fill", "heavy-showers-fill", "information-fill", "map-pin-2-fill", "mist-fill", "moon-clear-fill",
    "moon-cloudy-fill", "moon-foggy-fill", "radar-fill", "rainy-fill", "shield-check-fill", "showers-fill",
    "signal-tower-fill", "snowflake-fill", "snowy-fill", "sparkling-fill", "sun-cloudy-fill", "sun-fill",
    "sun-foggy-fill", "temp-cold-fill", "temp-hot-fill", "thunderstorms-fill", "time-fill", "tornado-fill",
    "typhoon-fill", "windy-fill",
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_colours() {
        assert_eq!(css("#FFE4B5", WHITE), Rgba::rgb(255, 228, 181));
        assert_eq!(css("#fff", INK_DARK), WHITE);
        let c = css("rgba(10, 19, 37, 0.78)", WHITE);
        assert_eq!((c.r, c.g, c.b), (10, 19, 37));
        assert!((c.a - 0.78).abs() < 1e-6);
        assert_eq!(css("nonsense", INK_DARK), INK_DARK);
    }

    #[test]
    fn pale_banners_get_dark_text() {
        assert_eq!(text_on(css("#FFE4B5", WHITE)), INK_DARK); // Special Weather Statement
        assert_eq!(text_on(css("#FFFF00", WHITE)), INK_DARK); // watch yellow
        assert_eq!(text_on(css("#FF0000", WHITE)), WHITE);
    }

    #[test]
    fn every_icon_parses() {
        let fonts = Fonts::load();
        let mut cv = Canvas::new(40.0, 40.0, 1.0, &fonts);
        for name in ICON_NAMES {
            assert!(icon_svg(name).is_some());
            cv.icon(name, 0.0, 0.0, 40.0, WHITE);
        }
        assert!(cv.pm.pixels().iter().any(|p| p.alpha() > 0));
    }

    #[test]
    fn fit_never_breaks_words() {
        let fonts = Fonts::load();
        let cv = Canvas::new(400.0, 100.0, 1.0, &fonts);
        let s = Style::new(40.0, 700, WHITE);
        let text = "Damaging winds up to seventy miles per hour and quarter size hail are expected with this storm";
        let (st, lines) = cv.fit(text, &s, 22.0, 300.0, 3);
        assert!(st.px >= 22.0 && lines.len() <= 3);
        for l in &lines {
            for w in l.trim_end_matches('…').split_whitespace() {
                assert!(text.contains(w), "{w} is not a whole word");
            }
        }
    }
}
