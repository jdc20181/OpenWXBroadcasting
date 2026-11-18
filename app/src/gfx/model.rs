//! Graphic documents: a 1920×1080 canvas of layers with live data bindings and in/out
//! animations. Saved as JSON in Documents\OpenWXBroadcasting\Graphics.

use serde::{Deserialize, Serialize};

use crate::gfx::text::{Align, Weight};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Fill {
    Solid { color: String },
    Linear { from: String, to: String, angle: f32 },
}

impl Fill {
    pub fn solid(color: &str) -> Self {
        Fill::Solid {
            color: color.into(),
        }
    }

    pub fn linear(from: &str, to: &str, angle: f32) -> Self {
        Fill::Linear {
            from: from.into(),
            to: to.into(),
            angle,
        }
    }

    pub fn primary_color(&self) -> &str {
        match self {
            Fill::Solid { color } => color,
            Fill::Linear { from, .. } => from,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StrokeStyle {
    pub color: String,
    pub width: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum AnimKind {
    None,
    Fade,
    SlideLeft,
    SlideRight,
    SlideUp,
    SlideDown,
    Wipe,
    Pop,
    /// Short rise with a fade (modern broadcast "settle in").
    Rise,
    /// Scale down from slightly large with a fade.
    Zoom,
    /// Drop from above and bounce to rest.
    Drop,
    /// Springy scale-in (elastic overshoot).
    Elastic,
    /// Quarter turn into place.
    Spin,
    /// Wipe that grows from the left with an exponential ease (bars, meters, plates).
    Grow,
    /// Wipe that opens from the centre outwards.
    Reveal,
}

impl AnimKind {
    pub const ALL: [AnimKind; 15] = [
        AnimKind::None,
        AnimKind::Fade,
        AnimKind::SlideLeft,
        AnimKind::SlideRight,
        AnimKind::SlideUp,
        AnimKind::SlideDown,
        AnimKind::Wipe,
        AnimKind::Pop,
        AnimKind::Rise,
        AnimKind::Zoom,
        AnimKind::Drop,
        AnimKind::Elastic,
        AnimKind::Spin,
        AnimKind::Grow,
        AnimKind::Reveal,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AnimKind::None => "None",
            AnimKind::Fade => "Fade",
            AnimKind::SlideLeft => "Slide from left",
            AnimKind::SlideRight => "Slide from right",
            AnimKind::SlideUp => "Slide up",
            AnimKind::SlideDown => "Slide down",
            AnimKind::Wipe => "Wipe",
            AnimKind::Pop => "Pop",
            AnimKind::Rise => "Rise",
            AnimKind::Zoom => "Zoom in",
            AnimKind::Drop => "Drop and bounce",
            AnimKind::Elastic => "Elastic",
            AnimKind::Spin => "Spin in",
            AnimKind::Grow => "Grow from left",
            AnimKind::Reveal => "Reveal from centre",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Anim {
    pub kind: AnimKind,
    pub delay: f32,
    pub duration: f32,
}

impl Default for Anim {
    fn default() -> Self {
        Self {
            kind: AnimKind::Fade,
            delay: 0.0,
            duration: 0.5,
        }
    }
}

impl Anim {
    pub fn new(kind: AnimKind, delay: f32, duration: f32) -> Self {
        Self {
            kind,
            delay,
            duration,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImageFit {
    Contain,
    Cover,
    Stretch,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum LayerKind {
    Rect {
        fill: Fill,
        stroke: Option<StrokeStyle>,
        radius: f32,
        /// Slant of the left and right edges as a share of the height (0 = square plate,
        /// 0.25 = broadcast-style angled plate). Negative leans the other way.
        #[serde(default)]
        skew: f32,
        /// Soft glossy highlight across the top half (modern look).
        #[serde(default)]
        sheen: bool,
    },
    /// Risk meter: a bar filled to a 0-5 level (SPC MRGL..HIGH, or any 0-5 scale) from a
    /// binding such as "{{spc.home.tornado.level}}". "bar" = one gradient bar, "blocks" = ten
    /// segments, "pill" = five labelled steps with the level highlighted.
    /// County map of an outlook around home (see gfx::riskmap): spec "tornado", "wind:2"...
    RiskMap {
        spec: String,
    },
    RiskMeter {
        value: String,
        style: String,
        /// Draw the MRGL/SLGT/ENH/MDT/HIGH scale labels inside the meter.
        #[serde(default)]
        labels: bool,
    },
    Ellipse {
        fill: Fill,
        stroke: Option<StrokeStyle>,
    },
    Text {
        /// Text with `{{bindings}}`, e.g. "{{temp}}°".
        text: String,
        font: String,
        size: f32,
        weight: Weight,
        color: String,
        align: Align,
        vcenter: bool,
        letter_spacing: f32,
        uppercase: bool,
        shadow: bool,
        wrap: bool,
        auto_fit: bool,
        /// Outline width in px (0 = none).
        #[serde(default)]
        outline: f32,
        /// Outline colour (default black).
        #[serde(default)]
        outline_color: String,
    },
    Image {
        /// File name in the media library.
        media: String,
        fit: ImageFit,
    },
    /// Weather icon picked from a condition (binding or literal), e.g. "{{day0.condition}}".
    Icon {
        condition: String,
        /// "day", "night" or a binding resolving to either.
        daypart: String,
    },
    Line {
        color: String,
        width: f32,
        arrow: bool,
    },
    /// Radar reflectivity legend for a LibreWXR palette (binding allowed, e.g. "{{radar.palette}}").
    Legend {
        palette: String,
        text_color: String,
    },
    /// Scrolling crawl.
    Ticker {
        text: String,
        font: String,
        size: f32,
        color: String,
        speed: f32,
    },
    /// Chart from a data series (`hourly.temp`, `river.stage`, …) or typed values.
    Chart {
        chart: crate::gfx::chart::ChartKind,
        series: String,
        /// Optional second series (second line, or right axis for Two-axis).
        #[serde(default)]
        series2: String,
        color: String,
        #[serde(default)]
        color2: String,
        #[serde(default)]
        text_color: String,
        #[serde(default)]
        show_values: bool,
        #[serde(default)]
        min: Option<f32>,
        #[serde(default)]
        max: Option<f32>,
        /// Unit after axis numbers ("°", "%", " ft").
        #[serde(default)]
        unit: String,
        /// Highlighted point: "max", "min", "last" or a label.
        #[serde(default)]
        highlight: String,
        /// Annotation shown at the highlighted point.
        #[serde(default)]
        note: String,
    },
    /// Animated explainer diagram (gfx::diagrams): "supercell", "radar_beam", "flood_depth"…
    Diagram {
        diagram: String,
    },
    /// Animated particles inside the layer box: "snow", "rain", "hail", "sparkle", "leaves".
    Particles {
        effect: String,
        /// Particles per 100x100 px.
        density: f32,
        /// 1.0 = natural speed.
        speed: f32,
        color: String,
    },
}

impl LayerKind {
    pub fn type_label(&self) -> &'static str {
        match self {
            LayerKind::Rect { .. } => "Shape",
            LayerKind::Ellipse { .. } => "Ellipse",
            LayerKind::Text { .. } => "Text",
            LayerKind::Image { .. } => "Image",
            LayerKind::Icon { .. } => "Weather icon",
            LayerKind::Line { .. } => "Line",
            LayerKind::Legend { .. } => "Radar legend",
            LayerKind::Ticker { .. } => "Crawl",
            LayerKind::Chart { .. } => "Chart",
            LayerKind::Particles { .. } => "Particles",
            LayerKind::Diagram { .. } => "Diagram",
            LayerKind::RiskMeter { .. } => "Risk meter",
            LayerKind::RiskMap { .. } => "Risk map",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Layer {
    pub id: String,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub kind: LayerKind,
    #[serde(default)]
    pub anim_in: Anim,
    #[serde(default = "default_out")]
    pub anim_out: Anim,
    /// Text reveal after the in-animation starts: "" (none), "typewriter" or "words".
    #[serde(default)]
    pub reveal: String,
    /// Seconds the reveal takes.
    #[serde(default)]
    pub reveal_secs: f32,
    /// Anchor for other canvas sizes: "" (scale with the canvas), or "top-left", "top",
    /// "top-right", "left", "center", "right", "bottom-left", "bottom", "bottom-right".
    #[serde(default)]
    pub anchor: String,
    /// Keep the left and right margins on other canvas sizes (the width stretches).
    #[serde(default)]
    pub stretch: bool,
    /// Container this layer sits in (moves with it; stacked when the container stacks).
    #[serde(default)]
    pub parent: String,
    /// For containers: "" (free), "vertical" or "horizontal" automatic layout of children.
    #[serde(default)]
    pub stack: String,
    /// Gap between stacked children.
    #[serde(default)]
    pub gap: f32,
    /// Keyframes (multi-track timeline): position, scale, rotation and opacity over time.
    #[serde(default)]
    pub keys: Vec<Keyframe>,
    /// Follow a smooth curve through the keyframe positions (motion path) instead of straight lines.
    #[serde(default)]
    pub smooth_path: bool,
    /// "" (play once), "loop" or "pingpong", over `loop_secs`.
    #[serde(default)]
    pub loop_mode: String,
    #[serde(default)]
    pub loop_secs: f32,
    /// Playback speed of this layer (0 = 1x).
    #[serde(default)]
    pub speed: f32,
    /// Start this layer (and everything inside it) later: nested sequences.
    #[serde(default)]
    pub time_offset: f32,
    /// Hotspot action when the layer is clicked or touched on the output: "state:Name",
    /// "graphic:Graphic name" (drill down), "toggle:Layer name", "play", "pause", "next".
    #[serde(default)]
    pub hotspot: String,
    /// Show only when this text ("{{cov3.name}}") has data: hidden when it binds to nothing
    /// or "--". Empty = always shown.
    #[serde(default)]
    pub show_if: String,
    /// Graphic states this layer shows in (empty = every state).
    #[serde(default)]
    pub states: Vec<String>,
}

/// One keyframe. Values are absolute (layer x / y in canvas pixels); the easing shapes the
/// segment that ends here: "linear", "in", "out", "inout", "bounce", "back" or
/// "cubic(x1,y1,x2,y2)" for a custom curve.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Keyframe {
    pub t: f32,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub easing: String,
}

impl Default for Keyframe {
    fn default() -> Self {
        Self { t: 0.0, x: 0.0, y: 0.0, scale: 1.0, rotation: 0.0, opacity: 1.0, easing: "inout".into() }
    }
}

/// Evaluate an easing curve at p (0-1).
pub fn ease_curve(name: &str, p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    if let Some(args) = name.strip_prefix("cubic(").and_then(|s| s.strip_suffix(')')) {
        let v: Vec<f32> = args.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if v.len() == 4 {
            return cubic_bezier(v[0], v[1], v[2], v[3], p);
        }
    }
    match name {
        "linear" => p,
        "in" => p * p * p,
        "out" => 1.0 - (1.0 - p).powi(3),
        "bounce" => {
            let (n, d) = (7.5625f32, 2.75f32);
            let x = p;
            if x < 1.0 / d {
                n * x * x
            } else if x < 2.0 / d {
                let x = x - 1.5 / d;
                n * x * x + 0.75
            } else if x < 2.5 / d {
                let x = x - 2.25 / d;
                n * x * x + 0.9375
            } else {
                let x = x - 2.625 / d;
                n * x * x + 0.984375
            }
        }
        "back" => {
            let c = 1.70158f32;
            1.0 + (c + 1.0) * (p - 1.0).powi(3) + c * (p - 1.0).powi(2)
        }
        // Penner-style curves (BSD): smooth broadcast motion.
        "expo" => if p >= 1.0 { 1.0 } else { 1.0 - 2f32.powf(-10.0 * p) },
        "circ" => (1.0 - (p - 1.0).powi(2)).max(0.0).sqrt(),
        "elastic" => {
            if p <= 0.0 || p >= 1.0 {
                p
            } else {
                2f32.powf(-10.0 * p) * ((p * 10.0 - 0.75) * (2.0 * std::f32::consts::PI / 3.0)).sin() + 1.0
            }
        }
        "smooth" => p * p * p * (p * (p * 6.0 - 15.0) + 10.0),
        _ => {
            if p < 0.5 {
                4.0 * p * p * p
            } else {
                1.0 - (-2.0 * p + 2.0).powi(3) / 2.0
            }
        }
    }
}

/// CSS-style cubic-bezier(x1, y1, x2, y2) easing, solved for x by Newton steps.
pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let bez = |a: f32, b: f32, t: f32| 3.0 * a * (1.0 - t).powi(2) * t + 3.0 * b * (1.0 - t) * t * t + t * t * t;
    let d = |a: f32, b: f32, t: f32| 3.0 * a * (1.0 - t).powi(2) + 6.0 * (b - a) * (1.0 - t) * t + 3.0 * (1.0 - b) * t * t;
    let mut t = x;
    for _ in 0..8 {
        let err = bez(x1, x2, t) - x;
        let slope = d(x1, x2, t);
        if slope.abs() < 1e-5 {
            break;
        }
        t = (t - err / slope).clamp(0.0, 1.0);
    }
    bez(y1, y2, t)
}

impl Layer {
    /// Keyframed values at time t: (x, y, scale, rotation, opacity), or None without keys.
    pub fn sample_keys(&self, t: f32) -> Option<(f32, f32, f32, f32, f32)> {
        let keys = &self.keys;
        let first = keys.first()?;
        if t <= first.t || keys.len() == 1 {
            return Some((first.x, first.y, first.scale, first.rotation, first.opacity));
        }
        let last = keys.last()?;
        if t >= last.t {
            return Some((last.x, last.y, last.scale, last.rotation, last.opacity));
        }
        let i = keys.windows(2).position(|w| t >= w[0].t && t <= w[1].t)?;
        let (a, b) = (&keys[i], &keys[i + 1]);
        let p = ease_curve(&b.easing, (t - a.t) / (b.t - a.t).max(1e-4));
        let lerp = |x: f32, y: f32| x + (y - x) * p;
        let (x, y) = if self.smooth_path {
            // Catmull-Rom through the neighbouring keys.
            let p0 = if i > 0 { (keys[i - 1].x, keys[i - 1].y) } else { (a.x, a.y) };
            let p3 = keys.get(i + 2).map(|k| (k.x, k.y)).unwrap_or((b.x, b.y));
            let cr = |q0: f32, q1: f32, q2: f32, q3: f32| 0.5 * (2.0 * q1 + (-q0 + q2) * p + (2.0 * q0 - 5.0 * q1 + 4.0 * q2 - q3) * p * p + (-q0 + 3.0 * q1 - 3.0 * q2 + q3) * p * p * p);
            (cr(p0.0, a.x, b.x, p3.0), cr(p0.1, a.y, b.y, p3.1))
        } else {
            (lerp(a.x, b.x), lerp(a.y, b.y))
        };
        Some((x, y, lerp(a.scale, b.scale), lerp(a.rotation, b.rotation), lerp(a.opacity, b.opacity)))
    }
}

fn default_out() -> Anim {
    Anim::new(AnimKind::Fade, 0.0, 0.35)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_string()
}

impl Layer {
    pub fn new(name: &str, x: f32, y: f32, w: f32, h: f32, kind: LayerKind) -> Self {
        Self {
            id: new_id(),
            name: name.into(),
            visible: true,
            locked: false,
            x,
            y,
            w,
            h,
            rotation: 0.0,
            opacity: 1.0,
            kind,
            anim_in: Anim::default(),
            anim_out: default_out(),
            reveal: String::new(),
            reveal_secs: 0.0,
            anchor: String::new(),
            stretch: false,
            parent: String::new(),
            stack: String::new(),
            gap: 0.0,
            keys: Vec::new(),
            smooth_path: false,
            loop_mode: String::new(),
            loop_secs: 0.0,
            speed: 0.0,
            time_offset: 0.0,
            hotspot: String::new(),
            show_if: String::new(),
            states: Vec::new(),
        }
    }

    /// Show this layer only while `binding` has data (see `show_if`).
    pub fn show_if(mut self, binding: &str) -> Self {
        self.show_if = binding.to_string();
        self
    }

    pub fn with_anim(mut self, kind: AnimKind, delay: f32, duration: f32) -> Self {
        self.anim_in = Anim::new(kind, delay, duration);
        self
    }

    pub fn rect(name: &str, x: f32, y: f32, w: f32, h: f32, fill: Fill, radius: f32) -> Self {
        Self::new(name, x, y, w, h, LayerKind::Rect { fill, stroke: None, radius, skew: 0.0, sheen: false })
    }

    /// Angled broadcast plate (parallelogram), optionally glossy.
    pub fn plate(name: &str, x: f32, y: f32, w: f32, h: f32, fill: Fill, skew: f32, sheen: bool) -> Self {
        Self::new(name, x, y, w, h, LayerKind::Rect { fill, stroke: None, radius: 0.0, skew, sheen })
    }

    /// County outlook map around home; `spec` is "hazard:day" (e.g. "tornado:1").
    pub fn risk_map(name: &str, x: f32, y: f32, w: f32, h: f32, spec: &str) -> Self {
        Self::new(name, x, y, w, h, LayerKind::RiskMap { spec: spec.into() })
    }

    pub fn meter(name: &str, x: f32, y: f32, w: f32, h: f32, value: &str, style: &str) -> Self {
        Self::new(name, x, y, w, h, LayerKind::RiskMeter { value: value.into(), style: style.into(), labels: false })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn text(
        name: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        text: &str,
        font: &str,
        size: f32,
        weight: Weight,
        color: &str,
        align: Align,
    ) -> Self {
        Self::new(
            name,
            x,
            y,
            w,
            h,
            LayerKind::Text {
                text: text.into(),
                font: font.into(),
                size,
                weight,
                color: color.into(),
                align,
                vcenter: true,
                letter_spacing: 0.0,
                uppercase: false,
                shadow: false,
                wrap: false,
                auto_fit: true,
                outline: 0.0,
                outline_color: String::new(),
            },
        )
    }

    pub fn icon(name: &str, x: f32, y: f32, size: f32, condition: &str, daypart: &str) -> Self {
        Self::new(
            name,
            x,
            y,
            size,
            size,
            LayerKind::Icon {
                condition: condition.into(),
                daypart: daypart.into(),
            },
        )
    }

    /// Point-in-layer test in canvas coordinates (ignores rotation, like most editors' hit boxes).
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GraphicDoc {
    pub id: String,
    pub name: String,
    pub category: String,
    pub width: u32,
    pub height: u32,
    /// None = transparent (an overlay such as a lower third or bug).
    pub background: Option<Fill>,
    /// Seconds until the graphic auto-advances in a show (0 = hold).
    pub duration: f32,
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub updated: i64,
    #[serde(default)]
    pub template: Option<String>,
    /// Library folder ("" = unfiled), e.g. "Tonight's drafts".
    #[serde(default)]
    pub folder: String,
    /// Saved sets of wording for the text layers ("Morning show", "Severe"), applied in one click.
    #[serde(default)]
    pub presets: Vec<TextPreset>,
    /// Tags for searching the library ("sponsor", "winter", "school").
    #[serde(default)]
    pub tags: Vec<String>,
    /// Location the graphic's model-based data is for (None = home). Hourly, day-part,
    /// commute, totals and nearby-city values follow it; NWS forecast text stays home's.
    #[serde(default)]
    pub place: Option<PlaceRef>,
    /// Who approved this graphic for air (empty = not approved). See Settings > require approval.
    #[serde(default)]
    pub approved_by: String,
    /// Station template this graphic is based on (graphic id). Its layers come first; a layer
    /// here with the same name overrides the template's.
    #[serde(default)]
    pub inherits: Option<String>,
    /// Data source configuration: binding -> value or other binding ("temp" -> "{{cities.1.temp}}").
    #[serde(default)]
    pub data_overrides: Vec<(String, String)>,
    /// Validation rules: (layer name, "required" | "max_chars:N" | "fits").
    #[serde(default)]
    pub rules: Vec<(String, String)>,
    /// Published as a station template (needs approval first).
    #[serde(default)]
    pub published: bool,
    /// Speed ramp: (graphic time, playback rate) points; empty = normal speed.
    #[serde(default)]
    pub speed_ramp: Vec<(f32, f32)>,
    /// Markers / cue points: (time, name). /api/cue?name= jumps the graphic on air to one.
    #[serde(default)]
    pub markers: Vec<(f32, String)>,
    /// Soundtrack (media file) played with the graphic; markers can be placed on its beats.
    #[serde(default)]
    pub audio: String,
    /// Named states (views) the presenter can switch between; the first is the default.
    #[serde(default)]
    pub states: Vec<String>,
    /// Visual look of the built-in frame: "" / "classic" (the original style) or "modern"
    /// (angled plates, glass panels, stronger type). See gfx::look.
    #[serde(default)]
    pub look: String,
}

impl GraphicDoc {
    /// The graphic as shown in `state`: layers limited to other states are hidden, and layers
    /// toggled off by hotspots stay hidden.
    pub fn in_state(&self, state: &str, hidden: &[String]) -> GraphicDoc {
        let mut out = self.clone();
        let state = if state.is_empty() { self.states.first().map(String::as_str).unwrap_or("") } else { state };
        for l in &mut out.layers {
            if (!l.states.is_empty() && !l.states.iter().any(|s| s == state)) || hidden.contains(&l.name) {
                l.visible = false;
            }
        }
        out
    }

    /// The topmost visible layer with a hotspot under (x, y) in canvas pixels.
    pub fn hotspot_at(&self, x: f32, y: f32, state: &str) -> Option<(String, String)> {
        let shown = self.in_state(state, &[]);
        shown.layers.iter().rev().filter(|l| l.visible && !l.hotspot.is_empty()).find(|l| x >= l.x && x <= l.x + l.w && y >= l.y && y <= l.y + l.h).map(|l| (l.name.clone(), l.hotspot.clone()))
    }
}

impl GraphicDoc {
    /// Real time -> animation time through the speed ramp (piecewise-constant rates).
    pub fn warp_time(&self, t: f32) -> f32 {
        if self.speed_ramp.is_empty() {
            return t;
        }
        let mut pts = self.speed_ramp.clone();
        pts.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (mut acc, mut prev_t, mut rate) = (0.0f32, 0.0f32, 1.0f32);
        for (at, r) in pts {
            if t <= at {
                break;
            }
            acc += (at - prev_t) * rate;
            prev_t = at;
            rate = r.max(0.0);
        }
        acc + (t - prev_t) * rate
    }

    /// Animation time for a layer: speed ramp, nested offsets (parents), speed, loop / ping-pong.
    pub fn layer_time(&self, layer: &Layer, t: f32) -> f32 {
        let mut t = self.warp_time(t) - layer.time_offset;
        let mut parent = layer.parent.clone();
        let mut depth = 0;
        while !parent.is_empty() && depth < 8 {
            match self.layers.iter().find(|l| l.id == parent) {
                Some(p) => {
                    t -= p.time_offset;
                    parent = p.parent.clone();
                }
                None => break,
            }
            depth += 1;
        }
        if layer.speed > 0.0 {
            t *= layer.speed;
        }
        if layer.loop_secs > 0.05 && t > 0.0 {
            let l = layer.loop_secs;
            t = match layer.loop_mode.as_str() {
                "loop" => t % l,
                "pingpong" => {
                    let m = t % (2.0 * l);
                    if m > l { 2.0 * l - m } else { m }
                }
                _ => t,
            };
        }
        t
    }
}

fn registry() -> &'static std::sync::RwLock<std::collections::HashMap<String, GraphicDoc>> {
    static R: std::sync::OnceLock<std::sync::RwLock<std::collections::HashMap<String, GraphicDoc>>> = std::sync::OnceLock::new();
    R.get_or_init(Default::default)
}

/// Keep the template registry current (the app calls this when the library changes).
pub fn set_registry(graphics: &[GraphicDoc]) {
    if let Ok(mut r) = registry().write() {
        r.clear();
        for g in graphics {
            r.insert(g.id.clone(), g.clone());
        }
    }
}

impl GraphicDoc {
    /// The graphic with its template chain applied (up to five levels).
    pub fn resolved(&self) -> GraphicDoc {
        self.resolve_depth(0)
    }

    fn resolve_depth(&self, depth: usize) -> GraphicDoc {
        let Some(parent_id) = self.inherits.as_ref().filter(|_| depth < 5) else { return self.clone() };
        let Some(parent) = registry().read().ok().and_then(|r| r.get(parent_id).cloned()) else { return self.clone() };
        let base = parent.resolve_depth(depth + 1);
        let mut out = self.clone();
        let mut layers = base.layers.clone();
        for l in &self.layers {
            match layers.iter_mut().find(|p| p.name == l.name) {
                Some(slot) => *slot = l.clone(),
                None => layers.push(l.clone()),
            }
        }
        out.layers = layers;
        if out.background.is_none() {
            out.background = base.background.clone();
        }
        let mut overrides = base.data_overrides.clone();
        overrides.extend(self.data_overrides.iter().cloned());
        out.data_overrides = overrides;
        let mut rules = base.rules.clone();
        rules.extend(self.rules.iter().cloned());
        out.rules = rules;
        out
    }

    /// Graphics based on this one (for dependency management).
    pub fn dependents(&self) -> Vec<String> {
        registry().read().map(|r| r.values().filter(|g| g.inherits.as_deref() == Some(self.id.as_str())).map(|g| g.name.clone()).collect()).unwrap_or_default()
    }

    /// Published station templates.
    pub fn templates() -> Vec<(String, String)> {
        registry().read().map(|r| r.values().filter(|g| g.published).map(|g| (g.id.clone(), g.name.clone())).collect()).unwrap_or_default()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PlaceRef {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}

impl PlaceRef {
    /// Settings with home moved to this place (for building bindings).
    pub fn apply(&self, settings: &crate::config::Settings) -> crate::config::Settings {
        let mut s = settings.clone();
        s.home.lat = self.lat;
        s.home.lon = self.lon;
        s.home_label = self.name.clone();
        s
    }
}

/// Named wording for a graphic's text layers.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TextPreset {
    pub name: String,
    /// (layer id, text) pairs.
    pub texts: Vec<(String, String)>,
}

impl GraphicDoc {
    pub fn new(name: &str, category: &str, background: Option<Fill>) -> Self {
        Self {
            id: new_id(),
            name: name.into(),
            category: category.into(),
            width: crate::config::FRAME_W,
            height: crate::config::FRAME_H,
            background,
            duration: 10.0,
            layers: Vec::new(),
            updated: chrono::Utc::now().timestamp(),
            template: None,
            folder: String::new(),
            presets: Vec::new(),
            tags: Vec::new(),
            place: None,
            approved_by: String::new(),
            inherits: None,
            data_overrides: Vec::new(),
            rules: Vec::new(),
            published: false,
            speed_ramp: Vec::new(),
            markers: Vec::new(),
            audio: String::new(),
            states: Vec::new(),
            look: String::new(),
        }
    }

    /// Save the current wording of every text layer as a preset (replacing one of the same name).
    pub fn save_preset(&mut self, name: &str) {
        let texts = self
            .layers
            .iter()
            .filter_map(|l| match &l.kind {
                LayerKind::Text { text, .. } | LayerKind::Ticker { text, .. } => Some((l.id.clone(), text.clone())),
                _ => None,
            })
            .collect();
        self.presets.retain(|p| p.name != name);
        self.presets.push(TextPreset { name: name.to_string(), texts });
    }

    /// Put a preset's wording back into the text layers.
    pub fn apply_preset(&mut self, name: &str) {
        let Some(preset) = self.presets.iter().find(|p| p.name == name).cloned() else { return };
        for (id, value) in preset.texts {
            if let Some(layer) = self.layer_mut(&id) {
                if let LayerKind::Text { text, .. } | LayerKind::Ticker { text, .. } = &mut layer.kind {
                    *text = value;
                }
            }
        }
    }

    /// A vertical (9:16, 1080x1920) copy for phones and social video: the landscape layout is
    /// scaled to the new width and centred, full-width bars stretch to the new width, and the
    /// background fills the taller frame.
    pub fn to_portrait(&self) -> GraphicDoc {
        let mut out = self.duplicate(&format!("{} (vertical)", self.name));
        let (ow, oh) = (self.width as f32, self.height as f32);
        let (nw, nh) = (1080.0f32, 1920.0f32);
        let s = nw / ow;
        let top = (nh - oh * s) / 2.0;
        out.width = nw as u32;
        out.height = nh as u32;
        for l in &mut out.layers {
            let full_width = l.x <= 1.0 && l.w >= ow - 2.0;
            l.x *= s;
            l.y = top + l.y * s;
            l.w *= s;
            l.h *= s;
            if full_width {
                l.x = 0.0;
                l.w = nw;
            }
            match &mut l.kind {
                LayerKind::Text { size, letter_spacing, .. } => {
                    *size *= s;
                    *letter_spacing *= s;
                }
                LayerKind::Ticker { size, .. } => *size *= s,
                _ => {}
            }
        }
        // Header bars go to the top, footer bars to the bottom.
        for l in &mut out.layers {
            if l.y + l.h <= top + 160.0 * s && l.x == 0.0 && l.w == nw {
                l.y -= top - 60.0;
            }
        }
        out
    }

    pub fn is_overlay(&self) -> bool {
        self.background.is_none()
    }

    /// Move a container's children with it (Graphics Studio drags).
    pub fn move_children(&mut self, parent: &str, dx: f32, dy: f32) {
        let kids: Vec<String> = self.layers.iter().filter(|l| l.parent == parent).map(|l| l.id.clone()).collect();
        for id in kids {
            if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
                l.x += dx;
                l.y += dy;
            }
            // Nested containers carry their own children too.
            self.move_children(&id, dx, dy);
        }
    }

    /// Automatic layout: children of stacking containers are placed one after another (hidden
    /// or `empty` ones take no space, so the rest close up), padded 20 px inside the container.
    pub fn reflow(&self, empty: impl Fn(&Layer) -> bool) -> GraphicDoc {
        let mut out = self.clone();
        let containers: Vec<(String, String, f32, f32, f32)> = out.layers.iter().filter(|l| !l.stack.is_empty()).map(|l| (l.id.clone(), l.stack.clone(), l.x, l.y, l.gap)).collect();
        for (id, stack, cx, cy, gap) in containers {
            let mut pos = if stack == "horizontal" { cx + 20.0 } else { cy + 20.0 };
            for l in out.layers.iter_mut().filter(|l| l.parent == id) {
                if !l.visible || empty(l) {
                    l.visible = false;
                    continue;
                }
                if stack == "horizontal" {
                    l.x = pos;
                    pos += l.w + gap;
                } else {
                    l.y = pos;
                    pos += l.h + gap;
                }
            }
        }
        out
    }

    /// The same graphic on another canvas size: anchored layers keep their distance to their
    /// edge and their size, stretched layers keep their margins, the rest scale with the canvas.
    pub fn laid_out(&self, w: u32, h: u32) -> GraphicDoc {
        let mut out = self.clone();
        let (ow, oh) = (self.width.max(1) as f32, self.height.max(1) as f32);
        let (nw, nh) = (w.max(1) as f32, h.max(1) as f32);
        let (sx, sy) = (nw / ow, nh / oh);
        let s = sx.min(sy);
        out.width = w;
        out.height = h;
        let originals: std::collections::HashMap<String, (f32, f32)> = self.layers.iter().map(|l| (l.id.clone(), (l.x, l.y))).collect();
        for l in &mut out.layers {
            let a = l.anchor.as_str();
            let (x0, w0) = (l.x, l.w);
            let (y0, h0) = (l.y, l.h);
            if a.is_empty() {
                l.x *= sx;
                l.y *= sy;
                l.w *= sx;
                l.h *= sy;
            } else {
                // Size scales uniformly; position follows the anchored edge.
                l.w = w0 * s;
                l.h = h0 * s;
                l.x = if a.ends_with("left") || a == "left" {
                    x0 * s
                } else if a.ends_with("right") {
                    nw - (ow - (x0 + w0)) * s - l.w
                } else {
                    (x0 + w0 / 2.0) * sx - l.w / 2.0
                };
                l.y = if a.starts_with("top") {
                    y0 * s
                } else if a.starts_with("bottom") {
                    nh - (oh - (y0 + h0)) * s - l.h
                } else {
                    (y0 + h0 / 2.0) * sy - l.h / 2.0
                };
            }
            if l.stretch {
                l.x = x0 * s;
                l.w = nw - x0 * s - (ow - (x0 + w0)) * s;
            }
            match &mut l.kind {
                LayerKind::Text { size, letter_spacing, .. } => {
                    *size *= s;
                    *letter_spacing *= s;
                }
                LayerKind::Ticker { size, .. } => *size *= s,
                _ => {}
            }
        }
        // Children keep their offset inside their container.
        let placed: std::collections::HashMap<String, (f32, f32)> = out.layers.iter().map(|l| (l.id.clone(), (l.x, l.y))).collect();
        for l in &mut out.layers {
            if let (Some((px0, py0)), Some((px1, py1)), Some((lx0, ly0))) = (originals.get(&l.parent), placed.get(&l.parent), originals.get(&l.id)) {
                l.x = px1 + (lx0 - px0) * s;
                l.y = py1 + (ly0 - py0) * s;
            }
        }
        out
    }

    /// Time at which every layer has finished animating in.
    pub fn intro_length(&self) -> f32 {
        self.layers
            .iter()
            .map(|l| l.anim_in.delay + l.anim_in.duration)
            .fold(0.0, f32::max)
    }

    pub fn layer_mut(&mut self, id: &str) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn layer(&self, id: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    /// Topmost visible layer under a point.
    pub fn hit(&self, x: f32, y: f32) -> Option<&Layer> {
        self.layers
            .iter()
            .rev()
            .find(|l| l.visible && !l.locked && l.contains(x, y))
    }

    /// Duplicate with fresh ids (for "Save as copy" / template instantiation).
    pub fn duplicate(&self, name: &str) -> Self {
        let mut copy = self.clone();
        copy.id = new_id();
        copy.name = name.into();
        copy.updated = chrono::Utc::now().timestamp();
        for layer in &mut copy.layers {
            layer.id = new_id();
        }
        copy
    }
}

/// Saved animation presets: a layer's keyframes (relative to its first key), loop and in/out.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AnimPreset {
    pub name: String,
    pub keys: Vec<Keyframe>,
    pub smooth_path: bool,
    pub loop_mode: String,
    pub loop_secs: f32,
    pub anim_in: Anim,
    pub anim_out: Anim,
}

pub fn presets_dir() -> std::path::PathBuf {
    crate::config::LibraryPaths::get().graphics.join("Animation presets")
}

pub fn load_anim_presets() -> Vec<AnimPreset> {
    let mut out: Vec<AnimPreset> = std::fs::read_dir(presets_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok())
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

impl AnimPreset {
    pub fn from_layer(name: &str, l: &Layer) -> Self {
        let (x0, y0) = l.keys.first().map(|k| (k.x, k.y)).unwrap_or((l.x, l.y));
        Self {
            name: name.to_string(),
            keys: l.keys.iter().map(|k| Keyframe { x: k.x - x0, y: k.y - y0, ..k.clone() }).collect(),
            smooth_path: l.smooth_path,
            loop_mode: l.loop_mode.clone(),
            loop_secs: l.loop_secs,
            anim_in: l.anim_in.clone(),
            anim_out: l.anim_out.clone(),
        }
    }

    pub fn apply(&self, l: &mut Layer) {
        l.keys = self.keys.iter().map(|k| Keyframe { x: k.x + l.x, y: k.y + l.y, ..k.clone() }).collect();
        l.smooth_path = self.smooth_path;
        l.loop_mode = self.loop_mode.clone();
        l.loop_secs = self.loop_secs;
        l.anim_in = self.anim_in.clone();
        l.anim_out = self.anim_out.clone();
    }

    pub fn save(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(presets_dir())?;
        std::fs::write(presets_dir().join(format!("{}.json", self.name.replace(['/', '\\', ':'], "_"))), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
