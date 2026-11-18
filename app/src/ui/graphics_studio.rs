//! Graphics Studio: build broadcast graphics from templates or scratch, with live data
//! bindings, animations, PNG export and one-click Take to Air.

use std::{rc::Rc, sync::Arc, time::Instant};

use gpui_kit::{
    component::input::{Input, InputEvent, InputState},
    prelude::FluentBuilder as _,
    *,
};
use tiny_skia::Pixmap;

use crate::{
    app::{AppState, ToastKind},
    config::{FRAME_H, FRAME_W},
    data::RADAR_PALETTES,
    gfx::{
        bind::CATALOG,
        bind_ext::CATALOG_EXT,
        chart::{ChartKind, SERIES},
        check::PreviewData,
        icons::IconKind,
        model::{AnimKind, Fill, GraphicDoc, ImageFit, Layer, LayerKind, StrokeStyle},
        parse_color,
        render::SETTLED,
        templates,
        text::{family_names, Align, Weight},
    },
    library::{self, MediaKind},
    program::{checkerboard, render_graphic, render_graphic_mode},
    show::{Scene, Transition},
    ui::{
        badge, button_danger, button_small, c, chip, col,
        frame_view::{fit_16_9, ImageSlot},
        heading, label, muted, row, section, slider, swatch, toggle, BG, BORDER, CYAN, MUTED,
        PANEL, PANEL_2, RAISED, RED, TEXT,
    },
};

#[path = "social_panel.rs"]
mod social;
#[path = "graphics_nav.rs"]
mod nav;

/// Landing sidebar entries that aren't categories, and how many template cards show per page.
const HOME_FAV: &str = "Favourites";
const HOME_USED: &str = "Recently used";
const HOME_PAGE: usize = 60;

const PREVIEW_W: u32 = 1280;
const PREVIEW_H: u32 = 720;

pub const GFX_COLORS: &[&str] = &[
    "$primary", "$secondary", "$panel", "$text", "$muted", "$alert", "$warning", "#FFFFFF", "#000000",
    "#FFD60A", "#3DD68C", "#5EC8FF", "#FF7A00", "rgba(255,255,255,0.07)", "rgba(0,0,0,0.5)",
];

/// Toolbar button with a Remix icon, highlighted when `active` (same look as Map Studio).
fn gnav_btn(id: impl Into<ElementId>, icon: &'static str, text: impl Into<SharedString>, active: bool) -> Stateful<Div> {
    let color = if active { CYAN } else { TEXT };
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(28.))
        .px(px(9.))
        .flex_shrink_0()
        .whitespace_nowrap()
        .rounded(px(6.))
        .cursor_pointer()
        .text_size(px(12.))
        .border_1()
        .when(active, |d| d.bg(crate::ui::ca(CYAN, 0.14)).border_color(crate::ui::ca(CYAN, 0.6)))
        .when(!active, |d| d.border_color(c(BORDER)).hover(|s| s.bg(c(PANEL_2))))
        .text_color(c(color))
        .child(crate::ui::icons::ri(icon, 15., color))
        .child(text.into())
}

/// Toolbar menus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GMenu {
    Add,
    Preview,
    Export,
    Send,
}

/// Remix icon for a template category on the landing page.
fn category_icon(category: &str) -> &'static str {
    match category {
        "Forecast" => "sun-cloudy-line",
        "Alerts" => "alarm-warning-line",
        "Severe" | "Lightning" => "thunderstorms-line",
        "Overlays" => "stack-line",
        "Show" => "slideshow-line",
        "Flooding" => "flood-line",
        "Travel" => "map-2-line",
        "Community" => "hand-heart-line",
        "Power" => "flashlight-line",
        "Safety" => "error-warning-line",
        "Explainers" => "lightbulb-line",
        "Tornado" => "tornado-line",
        "Hurricane" | "Tropical" => "windy-line",
        "Winter" => "snowy-line",
        "Science" => "earth-line",
        "Fire" => "fire-line",
        "Climate" => "temp-hot-line",
        "History" => "history-line",
        "Sky" => "star-line",
        "Accessibility" => "contrast-2-line",
        "Español" => "text",
        social::CATEGORY => "send-plane-line",
        _ => "gallery-line",
    }
}

#[derive(Clone, Copy)]
enum DragMode {
    Move,
    Resize,
}

struct GDrag {
    mode: DragMode,
    start: (f64, f64),
    orig: (f32, f32, f32, f32),
}

/// What a gallery card's right-click menu acts on.
#[derive(Clone)]
enum CardTarget {
    /// A built-in template (by key).
    Template(&'static str),
    /// A graphic already in the workspace.
    Graphic(Box<GraphicDoc>),
}

pub struct GraphicsStudio {
    app: Entity<AppState>,
    pub doc: GraphicDoc,
    template_preview: Option<GraphicDoc>,
    selected: Option<String>,
    dirty: bool,
    undo: Vec<GraphicDoc>,
    redo: Vec<GraphicDoc>,
    preview: Option<Arc<RenderImage>>,
    slot: Rc<ImageSlot>,
    render_gen: u64,
    rendering: bool,
    playing: Option<Instant>,
    drag: Option<GDrag>,
    syncing: bool,
    doc_name: Entity<InputState>,
    folder_input: Entity<InputState>,
    /// Folders collapsed in the library list.
    collapsed: std::collections::HashSet<String>,
    /// Graphic waiting for a second click to confirm deletion.
    confirm_delete: Option<String>,
    layer_name: Entity<InputState>,
    content: Entity<InputState>,
    pos: [Entity<InputState>; 4],
    hex: Entity<InputState>,
    /// Filter for the live-data picker.
    bind_filter: Entity<InputState>,
    bind_query: String,
    /// Library search (name, folder, tag, category).
    lib_filter: Entity<InputState>,
    lib_query: String,
    /// Name for a new wording preset.
    preset_input: Entity<InputState>,
    /// "binding = value" or "Layer: rule" entry for template settings.
    template_input: Entity<InputState>,
    /// Results of the last "Test this graphic".
    test_results: Vec<crate::gfx::testing::TestResult>,
    /// Comma-separated tags of the open graphic.
    tags_input: Entity<InputState>,
    /// Data the preview uses (live, sample, missing, long text).
    preview_data: PreviewData,
    /// Breakpoint preview: show the graphic laid out at another canvas size.
    breakpoint: Option<(u32, u32)>,
    /// Timeline position for keyframes and markers (seconds).
    key_time: f32,
    /// Custom easing being edited: (x1, y1, x2, y2).
    bezier: (f32, f32, f32, f32),
    /// Layout grid on the canvas (and snapping to it).
    show_grid: bool,
    /// Smart guides shown while dragging: (vertical?, position in doc pixels).
    guides: Vec<(bool, f32)>,
    /// Versions list open in the document panel.
    versions_open: bool,
    /// The landing page (categories, recent graphics) is showing instead of the editor.
    home: bool,
    /// Template category open on the landing page.
    home_category: Option<&'static str>,
    /// Landing filters: sub-group inside the category, format, live vs static, cards shown.
    home_sub: Option<String>,
    home_format: Option<nav::Format>,
    home_live: Option<bool>,
    home_limit: usize,
    /// Toolbar menu open (Add, Preview, Export, Send).
    open_menu: Option<GMenu>,
    /// When a menu was closed by an outside click (so its own button doesn't reopen it).
    menu_closed: Option<(GMenu, Instant)>,
    /// Rendered template thumbnails for the landing page, keyed by "look:key".
    thumbs: std::collections::HashMap<String, Arc<RenderImage>>,
    thumbs_pending: std::collections::HashSet<String>,
    /// Right-click menu on a gallery card: what it's for and where it opened (window pixels).
    card_menu: Option<(CardTarget, Point<Pixels>)>,
    /// Social Graphics category (website alert cards and forecast posts).
    social: social::SocialState,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

/// Brand tokens shown as real colors on swatches.
fn display_hex(token: &str, brand: &crate::config::Brand) -> String {
    let color = parse_color(token, brand).to_color_u8();
    format!("#{:02X}{:02X}{:02X}", color.red(), color.green(), color.blue())
}

/// Thumbnail render size that keeps the graphic's own shape (portrait, square or wide), 384 px on the long side.
fn thumb_dims(width: u32, height: u32) -> (u32, u32) {
    let (w, h) = (width.max(1) as f32, height.max(1) as f32);
    let scale = 384. / w.max(h);
    (((w * scale).round() as u32).max(1), ((h * scale).round() as u32).max(1))
}

fn pixmap_to_image(pm: Pixmap) -> Arc<RenderImage> {
    let (w, h) = (pm.width(), pm.height());
    let mut data = pm.take();
    for px in data.chunks_exact_mut(4) {
        px.swap(0, 2);
        px[3] = 255;
    }
    let buffer = image::RgbaImage::from_raw(w, h, data).expect("image buffer");
    Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]))
}

impl GraphicsStudio {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let doc = app
            .read(cx)
            .graphics
            .first()
            .cloned()
            .or_else(|| templates::build("current"))
            .unwrap_or_else(|| templates::blank(false));

        let make = |window: &mut Window, cx: &mut Context<Self>, placeholder: &str| {
            let placeholder = placeholder.to_string();
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let doc_name = make(window, cx, "Graphic name");
        let folder_input = make(window, cx, "Folder (e.g. Tonight's drafts)");
        let layer_name = make(window, cx, "Layer name");
        let content = make(window, cx, "Text - use {{bindings}} for live data");
        let pos = [
            make(window, cx, "X"),
            make(window, cx, "Y"),
            make(window, cx, "W"),
            make(window, cx, "H"),
        ];
        let hex = make(window, cx, "#RRGGBB");
        let bind_filter = make(window, cx, "Search live data (temp, river, closing…)");
        let lib_filter = make(window, cx, "Search graphics");
        let preset_input = make(window, cx, "Preset name (e.g. Morning show)");
        let template_input = make(window, cx, "temp = {{cities.1.temp}}   or   Headline: max_chars:40");
        let tags_input = make(window, cx, "Tags, comma separated");

        let mut subscriptions = vec![cx.observe_in(&app, window, |this, app, window, cx| {
            // A graphic created elsewhere (Data Discovery, Map Studio): open it here.
            let pending = app.update(cx, |s, _| s.pending_graphic.take());
            if let Some(id) = pending {
                let doc = app.read(cx).graphics.iter().find(|g| g.id == id).cloned();
                if let Some(doc) = doc {
                    this.open(doc, window, cx);
                    return;
                }
            }
            // Live data changed: re-render so bindings stay current.
            this.request_render(cx);
        })];

        subscriptions.push(cx.subscribe_in(&folder_input, window, |this, state, e: &InputEvent, _, cx| {
            if matches!(e, InputEvent::Change) && !this.syncing {
                this.doc.folder = state.read(cx).value().trim().to_string();
                this.dirty = true;
                cx.notify();
            }
        }));
        subscriptions.push(cx.subscribe_in(&doc_name, window, |this, state, e: &InputEvent, _, cx| {
            if matches!(e, InputEvent::Change) && !this.syncing {
                let v = state.read(cx).value().to_string();
                this.doc.name = v;
                this.dirty = true;
                cx.notify();
            }
        }));
        subscriptions.push(cx.subscribe_in(&layer_name, window, |this, state, e: &InputEvent, _, cx| {
            if matches!(e, InputEvent::Change) && !this.syncing {
                let v = state.read(cx).value().to_string();
                this.edit_layer(cx, false, |l| l.name = v);
            }
        }));
        subscriptions.push(cx.subscribe_in(&content, window, |this, state, e: &InputEvent, _, cx| {
            if matches!(e, InputEvent::Change) && !this.syncing {
                let v = state.read(cx).value().to_string();
                this.edit_layer(cx, false, |l| match &mut l.kind {
                    LayerKind::Text { text, .. } | LayerKind::Ticker { text, .. } => *text = v,
                    LayerKind::Image { media, .. } => *media = v.trim().to_string(),
                    LayerKind::RiskMeter { value, .. } => *value = v.trim().to_string(),
                    LayerKind::RiskMap { spec } => *spec = v.trim().to_string(),
                    _ => {}
                });
            }
        }));
        for (i, input) in pos.iter().enumerate() {
            subscriptions.push(cx.subscribe_in(input, window, move |this, state, e: &InputEvent, _, cx| {
                if matches!(e, InputEvent::Change) && !this.syncing {
                    if let Ok(v) = state.read(cx).value().trim().parse::<f32>() {
                        this.edit_layer(cx, false, |l| match i {
                            0 => l.x = v,
                            1 => l.y = v,
                            2 => l.w = v.max(1.0),
                            _ => l.h = v.max(1.0),
                        });
                    }
                }
            }));
        }
        subscriptions.push(cx.subscribe_in(&hex, window, |this, state, e: &InputEvent, _, cx| {
            if matches!(e, InputEvent::Change) && !this.syncing {
                let v = state.read(cx).value().trim().to_string();
                if crate::gfx::parse_hex(&v).is_some() {
                    this.set_color(v, cx);
                }
            }
        }));

        subscriptions.push(cx.subscribe_in(&bind_filter, window, |this, state, e: &InputEvent, _, cx| {
            if matches!(e, InputEvent::Change) {
                this.bind_query = state.read(cx).value().trim().to_lowercase();
                cx.notify();
            }
        }));
        subscriptions.push(cx.subscribe_in(&lib_filter, window, |this, state, e: &InputEvent, _, cx| {
            if matches!(e, InputEvent::Change) {
                this.lib_query = state.read(cx).value().trim().to_lowercase();
                cx.notify();
            }
            // Enter in the landing search opens the first matching template.
            if matches!(e, InputEvent::PressEnter { .. }) && this.home && !this.lib_query.is_empty() {
                let q = this.lib_query.clone();
                if let Some(doc) = templates::all().find(|t| nav::matches(t, &q)).and_then(|t| templates::build(t.key)) {
                    this.preview_template(doc, cx);
                }
            }
        }));
        subscriptions.push(cx.subscribe_in(&tags_input, window, |this, state, e: &InputEvent, _, cx| {
            if matches!(e, InputEvent::Change) && !this.syncing {
                this.doc.tags = state
                    .read(cx)
                    .value()
                    .split(',')
                    .map(|t| t.trim().to_lowercase())
                    .filter(|t| !t.is_empty())
                    .collect();
                this.dirty = true;
                cx.notify();
            }
        }));

        let mut studio = Self {
            app,
            doc,
            selected: None,
            template_preview: None,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            preview: None,
            slot: ImageSlot::new(),
            render_gen: 0,
            rendering: false,
            playing: None,
            drag: None,
            syncing: false,
            doc_name,
            folder_input,
            collapsed: Default::default(),
            confirm_delete: None,
            layer_name,
            content,
            pos,
            hex,
            bind_filter,
            bind_query: String::new(),
            lib_filter,
            lib_query: String::new(),
            preset_input,
            template_input,
            test_results: Vec::new(),
            tags_input,
            preview_data: PreviewData::Live,
            breakpoint: None,
            key_time: 0.0,
            bezier: (0.25, 0.1, 0.25, 1.0),
            show_grid: false,
            guides: Vec::new(),
            versions_open: false,
            home: true,
            home_category: None,
            home_sub: None,
            home_format: None,
            home_live: None,
            home_limit: HOME_PAGE,
            open_menu: None,
            menu_closed: None,
            thumbs: Default::default(),
            thumbs_pending: Default::default(),
            card_menu: None,
            social: social::SocialState::new(window, cx),
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        studio.load_inputs(window, cx);
        studio.request_render(cx);
        studio
    }

    // ---- Rendering ---------------------------------------------------------------------------

    pub fn request_render(&mut self, cx: &mut Context<Self>) {
        self.render_gen += 1;
        if !self.rendering {
            self.start_render(cx);
        }
    }

    fn start_render(&mut self, cx: &mut Context<Self>) {
        self.rendering = true;
        let gen = self.render_gen;
        let doc = match (&self.template_preview, self.breakpoint) {
            (Some(template), _) => template.clone(),
            (None, Some((w, h))) => self.doc.laid_out(w, h),
            (None, None) => self.doc.clone(),
        };
        let rendered_id = doc.id.clone();
        let state = self.app.read(cx);
        let settings = state.settings.clone();
        let data = state.data.clone();
        let map_frame = if doc.is_overlay() { state.compositor.map_frame() } else { None };
        let mode = self.preview_data;
        let (pw, ph) = preview_size(&doc);
        let time = self
            .playing
            .map(|t| t.elapsed().as_secs_f32())
            .unwrap_or(SETTLED);

        let task = cx.background_executor().spawn(async move {
            // Overlays are previewed over the live map (or a checkerboard).
            let background = if doc.is_overlay() {
                match map_frame {
                    Some(frame) => {
                        let mut rgba = frame.bgra.as_ref().clone();
                        for px in rgba.chunks_exact_mut(4) {
                            px.swap(0, 2);
                        }
                        tiny_skia::IntSize::from_wh(frame.width, frame.height)
                            .and_then(|size| Pixmap::from_vec(rgba, size))
                    }
                    None => Some(checkerboard(pw, ph)),
                }
            } else {
                None
            };
            let pm = render_graphic_mode(&doc, &settings, &data, time, pw, ph, background.as_ref(), mode);
            pixmap_to_image(pm)
        });

        cx.spawn(async move |this, cx| {
            let image = task.await;
            let _ = this.update(cx, |this, cx| {
                let displayed = this.template_preview.as_ref().unwrap_or(&this.doc);
                if displayed.id == rendered_id {
                    this.preview = Some(image);
                }
                this.rendering = false;
                let animating = this
                    .playing
                    .map(|t| t.elapsed().as_secs_f32() < this.doc.intro_length() + 1.5)
                    .unwrap_or(false);
                if !animating {
                    this.playing = None;
                }
                if this.render_gen != gen || animating {
                    this.start_render(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ---- Editing -----------------------------------------------------------------------------

    fn snapshot(&mut self) {
        self.undo.push(self.doc.clone());
        if self.undo.len() > 120 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.dirty = true;
        self.request_render(cx);
        cx.notify();
    }

    fn edit_layer(&mut self, cx: &mut Context<Self>, snapshot: bool, f: impl FnOnce(&mut Layer)) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        if snapshot {
            self.snapshot();
        }
        if let Some(layer) = self.doc.layer_mut(&id) {
            f(layer);
        }
        self.changed(cx);
    }

    fn set_color(&mut self, color: String, cx: &mut Context<Self>) {
        self.edit_layer(cx, true, |l| match &mut l.kind {
            LayerKind::Text { color: c, .. }
            | LayerKind::Line { color: c, .. }
            | LayerKind::Ticker { color: c, .. }
            | LayerKind::Legend { text_color: c, .. }
            | LayerKind::Chart { color: c, .. }
            | LayerKind::Particles { color: c, .. } => *c = color,
            LayerKind::Rect { fill, .. } | LayerKind::Ellipse { fill, .. } => match fill {
                Fill::Solid { color: c } => *c = color,
                Fill::Linear { from, .. } => *from = color,
            },
            _ => {}
        });
    }

    fn select(&mut self, id: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = id;
        self.load_inputs(window, cx);
        cx.notify();
    }

    fn set_input(input: &Entity<InputState>, value: String, window: &mut Window, cx: &mut Context<Self>) {
        input.update(cx, |state, cx| state.set_value(value, window, cx));
    }

    /// Fill the inspector inputs from the current selection (without echoing edits back).
    fn load_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing = true;
        Self::set_input(&self.doc_name.clone(), self.doc.name.clone(), window, cx);
        Self::set_input(&self.folder_input.clone(), self.doc.folder.clone(), window, cx);
        Self::set_input(&self.tags_input.clone(), self.doc.tags.join(", "), window, cx);
        if let Some(layer) = self.selected.as_ref().and_then(|id| self.doc.layer(id)).cloned() {
            Self::set_input(&self.layer_name.clone(), layer.name.clone(), window, cx);
            let text = match &layer.kind {
                LayerKind::Text { text, .. } | LayerKind::Ticker { text, .. } => text.clone(),
                LayerKind::Image { media, .. } => media.clone(),
                LayerKind::RiskMeter { value, .. } => value.clone(),
                LayerKind::RiskMap { spec } => spec.clone(),
                _ => String::new(),
            };
            Self::set_input(&self.content.clone(), text, window, cx);
            let values = [layer.x, layer.y, layer.w, layer.h];
            for (input, v) in self.pos.clone().iter().zip(values) {
                Self::set_input(input, format!("{v:.0}"), window, cx);
            }
            let color = match &layer.kind {
                LayerKind::Text { color, .. } | LayerKind::Line { color, .. } | LayerKind::Ticker { color, .. } | LayerKind::Particles { color, .. } => color.clone(),
                LayerKind::Rect { fill, .. } | LayerKind::Ellipse { fill, .. } => fill.primary_color().to_string(),
                LayerKind::Legend { text_color, .. } => text_color.clone(),
                _ => String::new(),
            };
            Self::set_input(&self.hex.clone(), color, window, cx);
        }
        self.syncing = false;
    }

    pub fn open(&mut self, doc: GraphicDoc, window: &mut Window, cx: &mut Context<Self>) {
        self.autosave(cx);
        if self.template_preview.take().is_some() { self.preview = None; }
        self.home = false;
        self.open_menu = None;
        self.doc = doc;
        self.selected = None;
        self.dirty = false;
        self.undo.clear();
        self.redo.clear();
        self.playing = None;
        self.load_inputs(window, cx);
        self.request_render(cx);
        cx.notify();
    }

    fn preview_template(&mut self, doc: GraphicDoc, cx: &mut Context<Self>) {
        if let Some(key) = doc.template.clone() {
            self.remember_template(&key, cx);
        }
        self.home = false;
        self.open_menu = None;
        self.template_preview = Some(doc);
        self.preview = None;
        self.drag = None;
        self.playing = None;
        self.request_render(cx);
        cx.notify();
    }

    fn autosave(&mut self, cx: &mut Context<Self>) {
        if self.dirty {
            let doc = self.doc.clone();
            self.app.update(cx, |s, _| {
                s.save_graphic(doc);
            });
            self.dirty = false;
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let doc = self.doc.clone();
        let saved = self.app.update(cx, |s, cx| {
            let saved = s.save_graphic(doc);
            if saved.is_some() {
                s.toast(ToastKind::Success, "Graphic saved");
            }
            cx.notify();
            saved
        });
        if let Some(saved) = saved {
            self.doc.updated = saved.updated;
            self.dirty = false;
            self.write_thumbnail(cx);
        }
        cx.notify();
    }

    /// Small PNG of the graphic for the library list (written in the background).
    fn write_thumbnail(&mut self, cx: &mut Context<Self>) {
        let doc = self.doc.clone();
        let state = self.app.read(cx);
        let settings = state.settings.clone();
        let data = state.data.clone();
        cx.background_executor()
            .spawn(async move {
                let (w, h) = thumb_dims(doc.width, doc.height);
                let pm = render_graphic(&doc, &settings, &data, SETTLED, w, h, Some(&checkerboard(w, h)).filter(|_| doc.is_overlay()));
                let path = library::thumb_path(&doc.id);
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Some(png) = crate::ops::pixmap_png(&pm) {
                    let _ = std::fs::write(path, png);
                }
            })
            .detach();
    }

    /// Export at the graphic's size, or as a square / vertical social crop.
    fn export_variant(&mut self, variant: &'static str, cx: &mut Context<Self>) {
        let doc = self.doc.clone();
        let state = self.app.read(cx);
        let settings = state.settings.clone();
        let data = state.data.clone();
        let task = cx.background_executor().spawn(async move {
            let pm = render_graphic(&doc, &settings, &data, SETTLED, doc.width, doc.height, None);
            let out = match variant {
                "square" | "vertical" => {
                    let (tw, th) = if variant == "square" { (1080u32, 1080u32) } else { (1080, 1920) };
                    // Centre crop to the target shape, then scale to the target size.
                    let (sw, sh) = (pm.width() as f32, pm.height() as f32);
                    let target = tw as f32 / th as f32;
                    let (cw, ch) = if sw / sh > target { (sh * target, sh) } else { (sw, sw / target) };
                    let (cx0, cy0) = ((sw - cw) / 2.0, (sh - ch) / 2.0);
                    let mut dst = Pixmap::new(tw, th).expect("pixmap");
                    let s = tw as f32 / cw;
                    dst.draw_pixmap(
                        0,
                        0,
                        pm.as_ref(),
                        &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() },
                        tiny_skia::Transform::from_translate(-cx0, -cy0).post_scale(s, s),
                        None,
                    );
                    dst
                }
                _ => pm,
            };
            let name = match variant {
                "square" => format!("{} (square)", doc.name),
                "vertical" => format!("{} (vertical)", doc.name),
                _ => doc.name.clone(),
            };
            library::export_png(&out, &name, None)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.app.update(cx, |s, cx| {
                    match result {
                        Ok(path) => {
                            s.toast(ToastKind::Success, format!("Exported {}", path.display()));
                            s.exports = library::list_exports();
                        }
                        Err(e) => s.toast(ToastKind::Error, format!("Export failed: {e}")),
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    /// Export the intro animation as numbered PNG frames at 30 fps with transparency (overlays
    /// stay see-through), for video editors and switchers that take alpha image sequences.
    fn export_sequence(&mut self, cx: &mut Context<Self>) {
        let doc = self.doc.clone();
        let state = self.app.read(cx);
        let settings = state.settings.clone();
        let data = state.data.clone();
        let task = cx.background_executor().spawn(async move {
            let safe: String = doc.name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
            let dir = crate::config::LibraryPaths::get()
                .exports
                .join(format!("{safe} frames {}", chrono::Local::now().format("%Y%m%d-%H%M%S")));
            std::fs::create_dir_all(&dir)?;
            let seconds = (doc.intro_length() + 1.0).clamp(1.0, 20.0);
            let frames = (seconds * 30.0).ceil() as u32;
            for f in 0..frames {
                let pm = render_graphic(&doc, &settings, &data, f as f32 / 30.0, doc.width, doc.height, None);
                let png = crate::ops::pixmap_png(&pm).ok_or_else(|| anyhow::anyhow!("PNG encode failed"))?;
                std::fs::write(dir.join(format!("frame_{f:04}.png")), png)?;
            }
            anyhow::Ok((dir, frames))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.app.update(cx, |s, cx| {
                    match result {
                        Ok((dir, n)) => {
                            s.toast(ToastKind::Success, format!("{n} transparent frames saved to {}", dir.display()));
                            let _ = open::that(&dir);
                        }
                        Err(e) => s.toast(ToastKind::Error, format!("Animation export failed: {e}")),
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    /// Frame coordinates (1920x1080 canvas space) to this graphic's coordinates.
    fn to_doc(&self, (fx, fy): (f64, f64)) -> (f64, f64) {
        (
            fx * self.doc.width as f64 / FRAME_W as f64,
            fy * self.doc.height as f64 / FRAME_H as f64,
        )
    }

    fn add_layer(&mut self, layer: Layer, window: &mut Window, cx: &mut Context<Self>) {
        self.snapshot();
        let id = layer.id.clone();
        self.doc.layers.push(layer);
        self.select(Some(id), window, cx);
        self.changed(cx);
    }

    /// Keyframes, easing (with a custom curve editor), loop, speed, offset, presets, and the
    /// graphic's markers, speed ramp and soundtrack, with a multi-track timeline.
    fn animation_section(&mut self, layer: &Layer, cx: &mut Context<Self>) -> Div {
        let dur = if self.doc.duration > 0.0 { self.doc.duration } else { 10.0 };
        let kt = self.key_time;
        let keys = layer.keys.clone();
        let (bx1, by1, bx2, by2) = self.bezier;
        let custom = format!("cubic({bx1:.2},{by1:.2},{bx2:.2},{by2:.2})");
        // Multi-track timeline: one row per layer, keyframe dots, markers and the playhead.
        let tracks = self
            .doc
            .layers
            .iter()
            .map(|l| {
                let dots = l.keys.iter().map(|k| {
                    div().absolute().top(px(3.)).left(relative((k.t / dur).clamp(0.0, 1.0))).w(px(8.)).h(px(8.)).rounded(px(4.)).bg(c(if l.id == layer.id { CYAN } else { 0x8A93A6 }))
                }).collect::<Vec<_>>();
                row()
                    .gap(px(6.))
                    .child(div().w(px(90.)).truncate().text_size(px(11.)).text_color(c(MUTED)).child(l.name.clone()))
                    .child(div().relative().flex_1().h(px(14.)).rounded(px(3.)).bg(c(0x1A2030)).children(dots).child(div().absolute().top_0().left(relative((kt / dur).clamp(0.0, 1.0))).w(px(2.)).h_full().bg(c(0xFF3EA5))))
            })
            .collect::<Vec<_>>();
        let marker_ticks = self.doc.markers.iter().map(|(t, _)| div().absolute().top_0().left(relative((t / dur).clamp(0.0, 1.0))).w(px(2.)).h_full().bg(c(0xF5C542))).collect::<Vec<_>>();
        let key_rows = keys
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let easings = [("linear", "Linear"), ("in", "In"), ("out", "Out"), ("inout", "In-out"), ("bounce", "Bounce"), ("back", "Back")]
                    .into_iter()
                    .enumerate()
                    .map(|(j, (key, text))| {
                        chip(("key-ease", i * 10 + j), text, k.easing == key).on_click(cx.listener(move |this, _, _, cx| {
                            this.edit_layer(cx, true, |l| {
                                if let Some(k) = l.keys.get_mut(i) {
                                    k.easing = key.to_string();
                                }
                            });
                        }))
                    })
                    .collect::<Vec<_>>();
                let custom_now = custom.clone();
                col()
                    .gap(px(2.))
                    .child(
                        row()
                            .gap(px(6.))
                            .child(div().flex_1().text_size(px(12.)).text_color(c(TEXT)).child(format!("{:.1}s • {:.0},{:.0} • {:.0}% • {:.0}° • {}", k.t, k.x, k.y, k.scale * 100.0, k.rotation, k.easing)))
                            .child(button_small(("key-custom", i), "Use curve").on_click(cx.listener(move |this, _, _, cx| {
                                let c = custom_now.clone();
                                this.edit_layer(cx, true, |l| {
                                    if let Some(k) = l.keys.get_mut(i) {
                                        k.easing = c;
                                    }
                                });
                            })))
                            .child(button_small(("key-del", i), "✕").on_click(cx.listener(move |this, _, _, cx| {
                                this.edit_layer(cx, true, |l| {
                                    if i < l.keys.len() {
                                        l.keys.remove(i);
                                    }
                                });
                            }))),
                    )
                    .child(row().flex_wrap().gap(px(2.)).children(easings))
            })
            .collect::<Vec<_>>();
        // Curve preview of the custom easing.
        let curve = row().h(px(44.)).items_end().gap(px(1.)).children((0..30).map(|i| {
            let v = crate::gfx::model::cubic_bezier(bx1, by1, bx2, by2, i as f32 / 29.0);
            div().w(px(6.)).h(px((2.0 + v.clamp(-0.2, 1.3) * 40.0).max(1.0))).bg(c(0x4F8FD6))
        }).collect::<Vec<_>>());
        let presets = crate::gfx::model::load_anim_presets()
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                let name = p.name.clone();
                button_small(("anim-preset", i), name).on_click(cx.listener(move |this, _, _, cx| {
                    let p = p.clone();
                    this.edit_layer(cx, true, |l| p.apply(l));
                    this.playing = Some(Instant::now());
                }))
            })
            .collect::<Vec<_>>();
        let loop_now = layer.loop_mode.clone();
        let speed_now = if layer.speed > 0.0 { layer.speed } else { 1.0 };
        let audio_now = self.doc.audio.clone();
        let audio_files: Vec<String> = self.app.read(cx).media.iter().map(|m| m.name.clone()).filter(|n| [".mp3", ".wav", ".ogg", ".flac"].iter().any(|e| n.to_lowercase().ends_with(e))).collect();
        let doc_states = self.doc.states.clone();
        let layer_states = layer.states.clone();
        let hotspot = layer.hotspot.clone();
        section("Animation")
            .child(muted("Interactive: states (views) the presenter switches between, and what touching this layer does"))
            .child(row().flex_wrap().gap(px(3.)).children(doc_states.iter().enumerate().map(|(i, st)| {
                let st2 = st.clone();
                chip(("layer-state", i), format!("Shows in {st}"), layer_states.contains(st)).on_click(cx.listener(move |this, _, _, cx| {
                    let s = st2.clone();
                    this.edit_layer(cx, true, |l| {
                        if let Some(p) = l.states.iter().position(|x| *x == s) {
                            l.states.remove(p);
                        } else {
                            l.states.push(s);
                        }
                    });
                }))
            }).collect::<Vec<_>>()).child(button_small("state-add", "Add state (preset box text)").on_click(cx.listener(|this, _, _, cx| {
                let name = this.preset_input.read(cx).value().trim().to_string();
                let name = if name.is_empty() { format!("State {}", this.doc.states.len() + 1) } else { name };
                if !this.doc.states.contains(&name) {
                    this.doc.states.push(name);
                }
                this.changed(cx);
            }))))
            .child(row().flex_wrap().gap(px(3.)).children(
                std::iter::once(("".to_string(), "No hotspot".to_string()))
                    .chain(doc_states.iter().map(|s| (format!("state:{s}"), format!("Switch to {s}"))))
                    .chain([("play".to_string(), "Play".to_string()), ("pause".to_string(), "Pause".to_string()), ("next".to_string(), "Next scene".to_string())])
                    .chain(self.app.read(cx).graphics.iter().take(12).map(|g| (format!("graphic:{}", g.name), format!("Open {}", g.name))))
                    .enumerate()
                    .map(|(i, (action, text))| {
                        chip(("hotspot", i), text, hotspot == action).on_click(cx.listener(move |this, _, _, cx| {
                            let a = action.clone();
                            this.edit_layer(cx, true, |l| l.hotspot = a);
                        }))
                    })
                    .collect::<Vec<_>>(),
            ))
            .child(muted(format!("Timeline (graphic is {dur:.0} s) • position {kt:.1} s")))
            .child(row().gap(px(6.)).child(div().w(px(90.)).text_size(px(11.)).text_color(c(MUTED)).child("Markers")).child(div().relative().flex_1().h(px(10.)).bg(c(0x1A2030)).children(marker_ticks)))
            .children(tracks)
            .child(slider("key-time", kt, 0.0, dur, cx, |this, v, _, cx| {
                this.key_time = (v * 10.0).round() / 10.0;
                cx.notify();
            }))
            .child(
                row()
                    .flex_wrap()
                    .gap(px(4.))
                    .child(button_small("key-add", "Add keyframe here").on_click(cx.listener(|this, _, _, cx| {
                        let t = this.key_time;
                        this.edit_layer(cx, true, |l| {
                            l.keys.retain(|k| (k.t - t).abs() > 0.05);
                            l.keys.push(crate::gfx::model::Keyframe { t, x: l.x, y: l.y, scale: 1.0, rotation: l.rotation, opacity: 1.0, easing: "inout".into() });
                            l.keys.sort_by(|a, b| a.t.total_cmp(&b.t));
                        });
                    })))
                    .child(chip("smooth-path", "Curved path", layer.smooth_path).on_click(cx.listener(|this, _, _, cx| {
                        this.edit_layer(cx, true, |l| l.smooth_path = !l.smooth_path);
                    })))
                    .child(button_small("key-play", "Play").on_click(cx.listener(|this, _, _, cx| {
                        this.playing = Some(Instant::now());
                        this.request_render(cx);
                    }))),
            )
            .child(muted("Move or resize the layer, then Add keyframe at another time. The newest key's values come from the layer."))
            .children(key_rows)
            .child(muted(format!("Custom curve {custom}")))
            .child(curve)
            .child(slider("bz-x1", bx1, 0.0, 1.0, cx, |this, v, _, cx| { this.bezier.0 = v; cx.notify(); }))
            .child(slider("bz-y1", by1, -0.5, 1.5, cx, |this, v, _, cx| { this.bezier.1 = v; cx.notify(); }))
            .child(slider("bz-x2", bx2, 0.0, 1.0, cx, |this, v, _, cx| { this.bezier.2 = v; cx.notify(); }))
            .child(slider("bz-y2", by2, -0.5, 1.5, cx, |this, v, _, cx| { this.bezier.3 = v; cx.notify(); }))
            .child(row().flex_wrap().gap(px(3.)).children([("", "Play once"), ("loop", "Loop"), ("pingpong", "Ping-pong")].into_iter().enumerate().map(|(i, (key, text))| {
                chip(("loop", i), text, loop_now == key).on_click(cx.listener(move |this, _, _, cx| {
                    this.edit_layer(cx, true, |l| {
                        l.loop_mode = key.to_string();
                        if l.loop_secs <= 0.0 {
                            l.loop_secs = l.keys.last().map(|k| k.t).unwrap_or(2.0).max(0.5);
                        }
                    });
                }))
            }).collect::<Vec<_>>()))
            .child(row().flex_wrap().gap(px(3.)).children([0.5f32, 1.0, 1.5, 2.0].into_iter().enumerate().map(|(i, v)| {
                chip(("lspeed", i), format!("{v}x"), (speed_now - v).abs() < 0.01).on_click(cx.listener(move |this, _, _, cx| {
                    this.edit_layer(cx, true, |l| l.speed = v);
                }))
            }).collect::<Vec<_>>()))
            .child(row().justify_between().child(muted("Start later by (nested: children follow)")).child(muted(format!("{:.1} s", layer.time_offset))))
            .child(slider("time-offset", layer.time_offset, 0.0, 10.0, cx, |this, v, _, cx| {
                this.edit_layer(cx, false, |l| l.time_offset = (v * 10.0).round() / 10.0);
            }))
            .child(
                row()
                    .flex_wrap()
                    .gap(px(4.))
                    .child(button_small("marker-add", "Add marker here").on_click(cx.listener(|this, _, _, cx| {
                        let t = this.key_time;
                        let name = format!("Cue {}", this.doc.markers.len() + 1);
                        this.doc.markers.push((t, name));
                        this.doc.markers.sort_by(|a, b| a.0.total_cmp(&b.0));
                        this.changed(cx);
                    })))
                    .child(button_small("marker-clear", "Clear markers").on_click(cx.listener(|this, _, _, cx| {
                        this.doc.markers.clear();
                        this.changed(cx);
                    })))
                    .child(button_small("ramp-slow", "Slow-motion from here").on_click(cx.listener(|this, _, _, cx| {
                        let t = this.key_time;
                        this.doc.speed_ramp.push((t, 0.4));
                        this.changed(cx);
                    })))
                    .child(button_small("ramp-normal", "Normal speed from here").on_click(cx.listener(|this, _, _, cx| {
                        let t = this.key_time;
                        this.doc.speed_ramp.push((t, 1.0));
                        this.changed(cx);
                    })))
                    .child(button_small("ramp-clear", "Clear speed ramp").on_click(cx.listener(|this, _, _, cx| {
                        this.doc.speed_ramp.clear();
                        this.changed(cx);
                    }))),
            )
            .child(muted(if self.doc.markers.is_empty() { "No markers. Markers are cue points: /api/cue?name=Cue 1 jumps the graphic on air there.".to_string() } else { self.doc.markers.iter().map(|(t, n)| format!("{n} @ {t:.1}s")).collect::<Vec<_>>().join(" • ") }))
            .child(muted("Soundtrack (plays with the graphic on air; put markers on its beats)"))
            .child(row().flex_wrap().gap(px(3.)).children(std::iter::once(String::new()).chain(audio_files).enumerate().map(|(i, name)| {
                let on = audio_now == name;
                let label = if name.is_empty() { "None".to_string() } else { name.clone() };
                chip(("gfx-audio", i), label, on).on_click(cx.listener(move |this, _, _, cx| {
                    this.doc.audio = name.clone();
                    this.changed(cx);
                }))
            }).collect::<Vec<_>>()))
            .child(muted("Animation presets"))
            .child(row().flex_wrap().gap(px(3.)).children(presets).child(button_small("anim-preset-save", "Save this layer's animation").on_click(cx.listener(|this, _, _, cx| {
                let Some(l) = this.selected.as_ref().and_then(|id| this.doc.layer(id)).cloned() else { return };
                let name = if this.preset_input.read(cx).value().trim().is_empty() { l.name.clone() } else { this.preset_input.read(cx).value().trim().to_string() };
                let result = crate::gfx::model::AnimPreset::from_layer(&name, &l).save();
                this.app.update(cx, |s, cx| {
                    match result {
                        Ok(()) => s.toast(ToastKind::Success, format!("Animation preset \"{name}\" saved")),
                        Err(e) => s.toast(ToastKind::Error, format!("Couldn't save the preset: {e}")),
                    }
                    cx.notify();
                });
            }))))
    }

    /// Save the selected container and everything inside it as a reusable component.
    fn save_component(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.selected.as_ref().and_then(|id| self.doc.layer(id)).cloned() else { return };
        let mut ids = vec![root.id.clone()];
        let mut i = 0;
        while i < ids.len() {
            let kids: Vec<String> = self.doc.layers.iter().filter(|l| l.parent == ids[i]).map(|l| l.id.clone()).collect();
            ids.extend(kids);
            i += 1;
        }
        let layers: Vec<crate::gfx::model::Layer> = self.doc.layers.iter().filter(|l| ids.contains(&l.id)).cloned().collect();
        let dir = crate::config::LibraryPaths::get().graphics.join("Components");
        let result = std::fs::create_dir_all(&dir)
            .map_err(anyhow::Error::from)
            .and_then(|_| serde_json::to_string_pretty(&layers).map_err(anyhow::Error::from))
            .and_then(|json| std::fs::write(dir.join(format!("{}.json", root.name.replace(['/', '\\', ':'], "_"))), json).map_err(anyhow::Error::from));
        self.app.update(cx, |s, cx| {
            match result {
                Ok(()) => s.toast(ToastKind::Success, format!("Component \"{}\" saved ({} layers)", root.name, layers.len())),
                Err(e) => s.toast(ToastKind::Error, format!("Could not save the component: {e}")),
            }
            cx.notify();
        });
    }

    /// Insert a saved component with fresh layer ids (parents remapped).
    fn insert_component(&mut self, path: std::path::PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut layers) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Vec<crate::gfx::model::Layer>>(&t).ok()) else { return };
        let map: std::collections::HashMap<String, String> = layers.iter().map(|l| (l.id.clone(), crate::gfx::model::new_id())).collect();
        for l in &mut layers {
            l.id = map[&l.id].clone();
            if let Some(p) = map.get(&l.parent) {
                l.parent = p.clone();
            }
        }
        self.snapshot();
        let first = layers.first().map(|l| l.id.clone());
        self.doc.layers.extend(layers);
        self.select(first, window, cx);
        self.changed(cx);
    }

    fn export_png(&mut self, cx: &mut Context<Self>) {
        self.export_as(false, cx);
    }

    fn export_jpeg(&mut self, cx: &mut Context<Self>) {
        self.export_as(true, cx);
    }

    /// Animated export: MP4 (H.264) or WebM with transparency. Runs off the UI thread.
    fn export_video(&mut self, webm: bool, cx: &mut Context<Self>) {
        let doc = self.doc.clone();
        let state = self.app.read(cx);
        let settings = state.settings.clone();
        let data = state.data.clone();
        let seconds = if doc.duration > 0.0 { doc.duration } else { 6.0 };
        let dir = library::exports_dir();
        let clean: String = doc.name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        let path = dir.join(format!("{clean}_{}.{}", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"), if webm { "webm" } else { "mp4" }));
        // The graphic's soundtrack goes into the file too.
        let audio = (!doc.audio.trim().is_empty())
            .then(|| crate::config::LibraryPaths::get().media.join(doc.audio.trim()))
            .filter(|p| p.exists());
        self.app.update(cx, |s, cx| {
            s.toast(ToastKind::Info, format!("Rendering {seconds:.0} s of video…"));
            cx.notify();
        });
        let task = cx.background_executor().spawn(async move {
            let render = |t: f32| {
                let (w, h) = (doc.width, doc.height);
                if webm {
                    render_graphic(&doc, &settings, &data, t, w, h, None)
                } else {
                    // MP4 has no transparency: overlays go over black.
                    let mut bg = tiny_skia::Pixmap::new(w.max(1), h.max(1)).expect("pixmap");
                    bg.fill(tiny_skia::Color::BLACK);
                    render_graphic(&doc, &settings, &data, t, w, h, Some(&bg))
                }
            };
            let result = if webm {
                crate::record::export_animation_webm(render, seconds, 30, path)
            } else {
                crate::record::export_animation_mp4(render, seconds, 30, path)
            };
            result.map(|path| {
                let note = audio.map(|a| match crate::record::mux_audio(&path, &a, seconds) {
                    Ok(()) => " with its music".to_string(),
                    Err(e) => {
                        log::warn!("video export music: {e}");
                        format!(" without music: {e}")
                    }
                });
                (path, note.unwrap_or_default())
            })
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.app.update(cx, |s, cx| {
                    match result {
                        Ok((path, note)) => {
                            let kind = if note.starts_with(" without") { ToastKind::Info } else { ToastKind::Success };
                            s.toast(kind, format!("Exported {}{note}", path.display()));
                            s.exports = library::list_exports();
                        }
                        Err(e) => s.toast(ToastKind::Error, format!("Video export: {e}")),
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn export_as(&mut self, jpeg: bool, cx: &mut Context<Self>) {
        let doc = self.doc.clone();
        let state = self.app.read(cx);
        let settings = state.settings.clone();
        let data = state.data.clone();
        let task = cx.background_executor().spawn(async move {
            let pm = render_graphic(&doc, &settings, &data, SETTLED, doc.width, doc.height, None);
            if jpeg {
                library::export_jpeg(&pm, &doc.name, 92)
            } else {
                library::export_png(&pm, &doc.name, None)
            }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.app.update(cx, |s, cx| {
                    match result {
                        Ok(path) => {
                            s.toast(ToastKind::Success, format!("Exported {}", path.display()));
                            s.exports = library::list_exports();
                        }
                        Err(e) => s.toast(ToastKind::Error, format!("Export failed: {e}")),
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn export_browser(&mut self, cx: &mut Context<Self>) {
        let doc=self.doc.clone();let state=self.app.read(cx);
        let settings=state.settings.clone();let data=state.data.clone();
        let work=cx.background_executor().spawn(async move {crate::gfx::browser::export(&doc,&settings,&data)});
        cx.spawn(async move|this,cx|{let result=work.await;let _=this.update(cx,|this,cx|{
            this.app.update(cx,|s,cx|{match result {
                Ok(path)=>{s.toast(ToastKind::Success,format!("Browser graphic exported to {}. Weather values are captured at export.",path.display()));s.exports=library::list_exports();}
                Err(e)=>s.toast(ToastKind::Error,format!("Browser export failed: {e}")),
            }cx.notify();});
        });}).detach();
    }

    // ---- Canvas pointer ----------------------------------------------------------------------

    fn on_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let Some(frame) = self.slot.to_frame(event.position) else {
            return;
        };
        let (fx, fy) = self.to_doc(frame);
        let (x, y) = (fx as f32, fy as f32);

        // Resize handle of the current selection.
        if let Some(layer) = self.selected.as_ref().and_then(|id| self.doc.layer(id)) {
            let hx = layer.x + layer.w;
            let hy = layer.y + layer.h;
            if (x - hx).abs() < 22.0 && (y - hy).abs() < 22.0 && !layer.locked {
                let orig = (layer.x, layer.y, layer.w, layer.h);
                self.snapshot();
                self.drag = Some(GDrag {
                    mode: DragMode::Resize,
                    start: (fx, fy),
                    orig,
                });
                return;
            }
        }

        let hit = self.doc.hit(x, y).map(|l| (l.id.clone(), (l.x, l.y, l.w, l.h)));
        match hit {
            Some((id, orig)) => {
                if self.selected.as_deref() != Some(id.as_str()) {
                    self.select(Some(id), window, cx);
                }
                self.snapshot();
                self.drag = Some(GDrag {
                    mode: DragMode::Move,
                    start: (fx, fy),
                    orig,
                });
            }
            None => self.select(None, window, cx),
        }
    }

    fn on_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if event.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
            return;
        }
        let Some(drag) = &self.drag else {
            return;
        };
        let (fx, fy) = self.to_doc(self.slot.to_frame_clamped(event.position));
        let dx = (fx - drag.start.0) as f32;
        let dy = (fy - drag.start.1) as f32;
        let (ox, oy, ow, oh) = drag.orig;
        let mode = drag.mode;
        let grid = if self.show_grid { 20.0 } else { 5.0 };
        let snap = |v: f32| (v / grid).round() * grid;
        let Some(id) = self.selected.clone() else {
            return;
        };
        // Smart guides: edges and centres of the other layers, the canvas centre and the
        // title-safe margins. Within 8 px, the moving edge or centre snaps to them.
        let (dw, dh) = (self.doc.width as f32, self.doc.height as f32);
        let mut xs: Vec<f32> = vec![0.0, dw / 2.0, dw, dw * 0.05, dw * 0.95];
        let mut ys: Vec<f32> = vec![0.0, dh / 2.0, dh, dh * 0.05, dh * 0.95];
        for l in self.doc.layers.iter().filter(|l| l.id != id && l.visible) {
            xs.extend([l.x, l.x + l.w / 2.0, l.x + l.w]);
            ys.extend([l.y, l.y + l.h / 2.0, l.y + l.h]);
        }
        let best = |targets: &[f32], candidates: &[f32]| -> Option<(f32, f32)> {
            // (offset to apply, guide position)
            let mut found: Option<(f32, f32)> = None;
            for c in candidates {
                for t in targets {
                    let d = t - c;
                    if d.abs() <= 8.0 && found.map(|f| d.abs() < f.0.abs()).unwrap_or(true) {
                        found = Some((d, *t));
                    }
                }
            }
            found
        };
        let mut guides = Vec::new();
        let mut moved: Option<(f32, f32)> = None;
        if let Some(layer) = self.doc.layer_mut(&id) {
            match mode {
                DragMode::Move => {
                    let before = (layer.x, layer.y);
                    let (mut nx, mut ny) = (snap(ox + dx), snap(oy + dy));
                    if let Some((d, g)) = best(&xs, &[nx, nx + ow / 2.0, nx + ow]) {
                        nx += d;
                        guides.push((true, g));
                    }
                    if let Some((d, g)) = best(&ys, &[ny, ny + oh / 2.0, ny + oh]) {
                        ny += d;
                        guides.push((false, g));
                    }
                    layer.x = nx;
                    layer.y = ny;
                    moved = Some((nx - before.0, ny - before.1));
                }
                DragMode::Resize => {
                    let (mut nw, mut nh) = (snap((ow + dx).max(8.0)), snap((oh + dy).max(8.0)));
                    if let Some((d, g)) = best(&xs, &[ox + nw]) {
                        nw += d;
                        guides.push((true, g));
                    }
                    if let Some((d, g)) = best(&ys, &[oy + nh]) {
                        nh += d;
                        guides.push((false, g));
                    }
                    layer.w = nw.max(8.0);
                    layer.h = nh.max(8.0);
                }
            }
        }
        if let Some((ddx, ddy)) = moved {
            self.doc.move_children(&id, ddx, ddy);
        }
        self.guides = guides;
        self.changed(cx);
    }

    fn on_up(&mut self, _event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.guides.clear();
        if self.drag.take().is_some() {
            self.load_inputs(window, cx);
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.template_preview.is_some() {
            if event.keystroke.key == "escape" {
                self.template_preview = None;
                self.preview = None;
                self.request_render(cx);
                cx.notify();
            }
            return;
        }
        let key = event.keystroke.key.as_str();
        let ctrl = event.keystroke.modifiers.control;
        match (ctrl, key) {
            (true, "z") => self.undo(window, cx),
            (true, "y") => self.redo(window, cx),
            (true, "s") => self.save(cx),
            (true, "d") => self.duplicate_layer(window, cx),
            (false, "delete") => self.delete_layer(window, cx),
            (false, "left") | (false, "right") | (false, "up") | (false, "down") => {
                let step = if event.keystroke.modifiers.shift { 10.0 } else { 1.0 };
                let (dx, dy) = match key {
                    "left" => (-step, 0.0),
                    "right" => (step, 0.0),
                    "up" => (0.0, -step),
                    _ => (0.0, step),
                };
                self.edit_layer(cx, true, |l| {
                    l.x += dx;
                    l.y += dy;
                });
                self.load_inputs(window, cx);
            }
            _ => {}
        }
    }

    fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.doc, prev));
            self.load_inputs(window, cx);
            self.changed(cx);
        }
    }

    fn redo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.doc, next));
            self.load_inputs(window, cx);
            self.changed(cx);
        }
    }

    fn delete_layer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.selected.clone() {
            self.snapshot();
            self.doc.layers.retain(|l| l.id != id);
            self.select(None, window, cx);
            self.changed(cx);
        }
    }

    fn duplicate_layer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layer) = self.selected.as_ref().and_then(|id| self.doc.layer(id)).cloned() {
            let mut copy = layer;
            copy.id = crate::gfx::model::new_id();
            copy.name = format!("{} copy", copy.name);
            copy.x += 20.0;
            copy.y += 20.0;
            self.add_layer(copy, window, cx);
        }
    }

    fn move_layer(&mut self, delta: i32, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        if let Some(i) = self.doc.layers.iter().position(|l| l.id == id) {
            let j = (i as i32 + delta).clamp(0, self.doc.layers.len() as i32 - 1) as usize;
            if i != j {
                self.snapshot();
                let layer = self.doc.layers.remove(i);
                self.doc.layers.insert(j, layer);
                self.changed(cx);
            }
        }
    }

    // ---- Panels ------------------------------------------------------------------------------

    fn library_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let q = self.lib_query.clone();
        let graphics: Vec<GraphicDoc> = self
            .app
            .read(cx)
            .graphics
            .iter()
            .filter(|g| {
                q.is_empty()
                    || g.name.to_lowercase().contains(&q)
                    || g.folder.to_lowercase().contains(&q)
                    || g.category.to_lowercase().contains(&q)
                    || g.tags.iter().any(|t| t.contains(&q))
            })
            .cloned()
            .collect();
        let current = self.doc.id.clone();

        // Group by folder: named folders A-Z, then unfiled graphics.
        let mut folders: Vec<String> = graphics.iter().map(|g| g.folder.clone()).collect();
        folders.sort_by(|a, b| (a.is_empty(), a.to_lowercase()).cmp(&(b.is_empty(), b.to_lowercase())));
        folders.dedup();
        let confirm = self.confirm_delete.clone();
        let mut items: Vec<AnyElement> = Vec::new();
        let mut n = 0usize;
        for folder in folders {
            let in_folder: Vec<GraphicDoc> = graphics.iter().filter(|g| g.folder == folder).cloned().collect();
            let collapsed = self.collapsed.contains(&folder);
            let key = folder.clone();
            if !(folder.is_empty() && items.is_empty()) || !folder.is_empty() {
                let label = if folder.is_empty() { "Unfiled".to_string() } else { folder.clone() };
                items.push(
                    row()
                        .id(("gfx-folder", n))
                        .gap(px(6.))
                        .pt(px(6.))
                        .cursor_pointer()
                        .child(div().text_size(px(11.)).text_color(c(MUTED)).child(if collapsed { "▸" } else { "▾" }))
                        .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(format!("{label} ({})", in_folder.len())))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.collapsed.remove(&key) {
                                this.collapsed.insert(key.clone());
                            }
                            cx.notify();
                        }))
                        .into_any_element(),
                );
            }
            if collapsed {
                continue;
            }
            for g in in_folder {
                n += 1;
                let i = n;
                let active = g.id == current;
                let doc = g.clone();
                let del_id = g.id.clone();
                let confirming = confirm.as_deref() == Some(g.id.as_str());
                items.push(
                    row()
                        .id(("gfx", i))
                        .gap(px(8.))
                        .px(px(10.))
                        .py(px(7.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .bg(if active { c(crate::ui::CYAN_DIM) } else { c(PANEL_2) })
                        .hover(|s| s.bg(c(crate::ui::HOVER)))
                        .child({
                            let thumb = library::thumb_path(&g.id);
                            if thumb.exists() {
                                img(thumb).w(px(64.)).h(px(36.)).rounded(px(3.)).into_any_element()
                            } else {
                                badge(if g.is_overlay() { "Overlay" } else { "Full" }, if g.is_overlay() { 0xB48CFF } else { CYAN }).into_any_element()
                            }
                        })
                        .child(
                            col()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_size(px(13.)).text_color(c(TEXT)).truncate().child(g.name.clone()))
                                .child(div().text_size(px(11.)).text_color(c(MUTED)).child(g.category.clone())),
                        )
                        .child(
                            div()
                                .id(("gfx-del", i))
                                .px(px(6.))
                                .py(px(2.))
                                .rounded(px(4.))
                                .text_size(px(11.))
                                .when(confirming, |d| d.bg(c(crate::ui::RED)).text_color(c(0xFFFFFF)).child("Delete?"))
                                .when(!confirming, |d| d.text_color(c(MUTED)).hover(|s| s.text_color(c(crate::ui::RED))).child("Delete"))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    if this.confirm_delete.as_deref() != Some(del_id.as_str()) {
                                        this.confirm_delete = Some(del_id.clone());
                                        cx.notify();
                                        return;
                                    }
                                    this.confirm_delete = None;
                                    let was_open = this.doc.id == del_id;
                                    let next = this.app.update(cx, |s, cx| {
                                        s.delete_graphic(&del_id);
                                        s.toast(ToastKind::Info, "Graphic moved to the library trash");
                                        cx.notify();
                                        s.graphics.first().cloned()
                                    });
                                    if was_open {
                                        this.dirty = false;
                                        let doc = next.unwrap_or_else(|| templates::blank(false));
                                        this.open(doc, window, cx);
                                    }
                                    cx.notify();
                                })),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| this.open(doc.clone(), window, cx)))
                        .into_any_element(),
                );
            }
        }

        let template_items = templates::all()
            .filter(|t| {
                q.is_empty() || t.name.to_lowercase().contains(&q) || t.category.to_lowercase().contains(&q) || t.description.to_lowercase().contains(&q)
            })
            .enumerate()
            .map(|(i, t)| {
                let key = t.key;
                col()
                    .id(("tpl", i))
                    .gap(px(2.))
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .bg(c(PANEL_2))
                    .hover(|s| s.bg(c(0x232C40)))
                    .child(
                        row()
                            .justify_between()
                            .child(div().text_size(px(13.)).text_color(c(TEXT)).child(t.name))
                            .child(badge(t.category.clone(), 0x7A8599)),
                    )
                    .child(div().text_size(px(11.)).text_color(c(MUTED)).child(t.description))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(doc) = templates::build(key) {
                            this.preview_template(doc, cx);
                        }
                    }))
            })
            .collect::<Vec<_>>();

        col()
            .id("gfx-library")
            .w(px(280.))
            .h_full()
            .flex_shrink_0()
            .bg(c(PANEL))
            .border_r_1()
            .border_color(c(BORDER))
            .overflow_y_scroll()
            .child(
                section("New")
                    .child(
                        row()
                            .gap(px(6.))
                            .child(button_small("new-full", "+ Full screen").on_click(cx.listener(|this, _, window, cx| {
                                let doc = templates::blank(false);
                                this.open(doc, window, cx);
                                this.dirty = true;
                            })))
                            .child(button_small("new-ovl", "+ Overlay").on_click(cx.listener(|this, _, window, cx| {
                                let doc = templates::blank(true);
                                this.open(doc, window, cx);
                                this.dirty = true;
                            }))),
                    ),
            )
            .child(section("Workspace").gap(px(5.)).child(Input::new(&self.lib_filter)).children(items))
            .child(
                section("Library tools")
                    .child(
                        row()
                            .gap(px(6.))
                            .flex_wrap()
                            .child(button_small("pkg-export", "Export package").on_click(cx.listener(|_this, _, _, cx| {
                                crate::ui::pick_off_thread(cx, || rfd::FileDialog::new()
                                    .set_file_name("OpenWX graphics.cwxpack")
                                    .add_filter("OpenWX graphics package", &["cwxpack", "zip"])
                                    .save_file(), |this: &mut Self, path, cx| {
                                    let graphics = this.app.read(cx).graphics.clone();
                                    let result = library::export_package(&graphics, &path);
                                    this.app.update(cx, |s, _| match result {
                                        Ok(n) => s.toast(ToastKind::Success, format!("Exported {n} graphics and their images")),
                                        Err(e) => s.toast(ToastKind::Error, format!("Package export failed: {e}")),
                                    });
                                });
                            })))
                            .child(button_small("pkg-import", "Import package").on_click(cx.listener(|_this, _, _, cx| {
                                crate::ui::pick_off_thread(cx, || rfd::FileDialog::new().add_filter("OpenWX graphics package", &["cwxpack", "zip"]).pick_file(), |this: &mut Self, path, cx| {
                                    let result = library::import_package(&path);
                                    this.app.update(cx, |s, cx| {
                                        match result {
                                            Ok(n) => s.toast(ToastKind::Success, format!("Imported {n} graphics into the Imported folder")),
                                            Err(e) => s.toast(ToastKind::Error, format!("Package import failed: {e}")),
                                        }
                                        s.reload_library();
                                        cx.notify();
                                    });
                                });
                            })))
                            .child(button_small("media-check", "Find missing images").on_click(cx.listener(|this, _, _, cx| {
                                let missing = library::missing_media(&this.app.read(cx).graphics);
                                this.app.update(cx, |s, _| {
                                    if missing.is_empty() {
                                        s.toast(ToastKind::Success, "Every image used by a graphic is in the media library");
                                    } else {
                                        for (g, m) in &missing {
                                            log::warn!("missing image: {m} (used by {g})");
                                        }
                                        s.toast(ToastKind::Error, format!("{} missing image(s); see Developer Logs for the list", missing.len()));
                                    }
                                });
                            })))
                            .child(button_small("media-clean", "Clean unused media").on_click(cx.listener(|this, _, _, cx| {
                                let (graphics, settings, show) = {
                                    let s = this.app.read(cx);
                                    (s.graphics.clone(), s.settings.clone(), s.show.clone())
                                };
                                let result = library::clean_unused_media(&graphics, &settings, &show);
                                this.app.update(cx, |s, cx| {
                                    match result {
                                        Ok(n) => s.toast(ToastKind::Success, format!("Moved {n} unused file(s) to the library trash")),
                                        Err(e) => s.toast(ToastKind::Error, format!("Cleanup failed: {e}")),
                                    }
                                    s.reload_library();
                                    cx.notify();
                                });
                            }))),
                    ),
            )
            .child(section("Templates").gap(px(5.)).children(template_items))
    }

    /// A toolbar menu: the button stays in the bar, the panel floats under it.
    fn menu(&mut self, which: GMenu, icon: &'static str, text: &'static str, width: f32, cx: &mut Context<Self>, content: impl FnOnce(&mut Self, &mut Context<Self>) -> AnyElement) -> Div {
        let open = self.open_menu == Some(which);
        let trigger = gnav_btn(("gmenu", which as usize), icon, text, open)
            .child(crate::ui::icons::ri("arrow-down-s-line", 14., MUTED))
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.menu_closed.is_some_and(|(m, at)| m == which && at.elapsed() < std::time::Duration::from_millis(300)) {
                    this.menu_closed = None;
                    return;
                }
                this.open_menu = if this.open_menu == Some(which) { None } else { Some(which) };
                cx.notify();
            }));
        let mut d = div().relative().flex_shrink_0().child(trigger);
        if open {
            let body = content(self, cx);
            d = d.child(
                deferred(
                    anchored().snap_to_window_with_margin(px(8.)).child(
                        div()
                            .id(("gmenu-panel", which as usize))
                            .occlude()
                            .mt(px(4.))
                            .w(px(width))
                            .max_h(px(560.))
                            .overflow_y_scroll()
                            .p(px(10.))
                            .rounded(px(8.))
                            .bg(c(PANEL))
                            .border_1()
                            .border_color(c(BORDER))
                            .shadow_lg()
                            .on_mouse_down_out(cx.listener(move |this, _, _, cx| {
                                if this.open_menu == Some(which) {
                                    this.open_menu = None;
                                    this.menu_closed = Some((which, Instant::now()));
                                    cx.notify();
                                }
                            }))
                            .child(body),
                    ),
                )
                .with_priority(3),
            );
        }
        d
    }

    /// Restyle the open graphic in a look (Modern / Classic), keeping its content.
    fn set_look(&mut self, look: &'static str, cx: &mut Context<Self>) {
        // Applying the current look again is allowed: it re-tidies the frame.
        self.snapshot();
        crate::gfx::look::apply(&mut self.doc, look);
        self.changed(cx);
    }

    fn toolbar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_overlay = self.doc.is_overlay();
        let playing = self.playing.is_some();
        let look_now = crate::gfx::look::of(&self.doc);
        let sep = || div().w(px(1.)).h(px(22.)).bg(c(BORDER));

        let add_menu = self.menu(GMenu::Add, "add-line", "Add", 520., cx, |this, cx| {
            let add = |id: &'static str, text: &'static str, make: fn() -> Layer, cx: &mut Context<Self>| {
                button_small(id, text).on_click(cx.listener(move |this, _, window, cx| {
                    this.open_menu = None;
                    this.add_layer(make(), window, cx);
                }))
            };
            let components: Vec<_> = std::fs::read_dir(crate::config::LibraryPaths::get().graphics.join("Components")).into_iter().flatten().flatten().take(12).enumerate().map(|(i, e)| {
                let path = e.path();
                let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                button_small(("component", i), name).on_click(cx.listener(move |this, _, window, cx| {
                    this.open_menu = None;
                    this.insert_component(path.clone(), window, cx);
                }))
            }).collect();
            col().gap(px(8.))
                .child(muted("Text and shapes"))
                .child(row().gap(px(6.)).flex_wrap()
                    .child(add("add-text", "Text", || Layer::text("Text", 200.0, 200.0, 800.0, 120.0, "New text", "$display", 72.0, Weight::Bold, "$text", Align::Left), cx))
                    .child(add("add-box", "Box", || Layer::rect("Box", 200.0, 200.0, 600.0, 300.0, Fill::solid("$secondary"), 12.0), cx))
                    .child(add("add-plate", "Angled plate", || Layer::plate("Plate", 200.0, 200.0, 900.0, 110.0, Fill::linear("#FFFFFF", "#E3E9F2", 90.0), 0.18, false), cx))
                    .child(add("add-ellipse", "Ellipse", || Layer::new("Ellipse", 300.0, 300.0, 300.0, 300.0, LayerKind::Ellipse { fill: Fill::solid("$primary"), stroke: None }), cx))
                    // Live countdown text: {{until:date}} or {{until:date HH:MM}} (gfx::bind::dynamic).
                    .child(add("add-countdown", "Countdown", || {
                        use chrono::Datelike;
                        let today = chrono::Local::now().date_naive();
                        let year = if (today.month(), today.day()) > (12, 25) { today.year() + 1 } else { today.year() };
                        Layer::text("Countdown", 200.0, 200.0, 900.0, 140.0, &format!("{{{{until:{year}-12-25}}}} to Christmas"), "$display", 80.0, Weight::Bold, "$text", Align::Left)
                    }, cx))
                    .child(add("add-line", "Line", || Layer::new("Line", 200.0, 500.0, 600.0, 40.0, LayerKind::Line { color: "$primary".into(), width: 8.0, arrow: false }), cx)))
                .child(muted("Weather"))
                .child(row().gap(px(6.)).flex_wrap()
                    .child(add("add-icon", "Weather icon", || Layer::icon("Icon", 300.0, 300.0, 240.0, "{{condition}}", "{{daypart}}"), cx))
                    .child(add("add-meter", "Risk meter", || Layer::meter("Risk meter", 200.0, 400.0, 1200.0, 110.0, "{{spc.home.level}}", "bar"), cx))
                    .child(add("add-riskmap", "County risk map", || Layer::risk_map("County risk map", 160.0, 200.0, 1100.0, 760.0, "categorical:1"), cx))
                    .child(add("add-legend", "Radar legend", || Layer::new("Legend", 80.0, 940.0, 620.0, 96.0, LayerKind::Legend { palette: "{{radar.palette}}".into(), text_color: "#FFFFFF".into() }), cx))
                    .child(add("add-chart", "Chart", || Layer::new("Chart", 160.0, 260.0, 1600.0, 620.0, LayerKind::Chart {
                        chart: ChartKind::Line,
                        series: "hourly.temp".into(),
                        series2: String::new(),
                        color: "$primary".into(),
                        color2: "#5EC8FF".into(),
                        text_color: "$text".into(),
                        show_values: true,
                        min: None,
                        max: None,
                        unit: "°".into(),
                        highlight: String::new(),
                        note: String::new(),
                    }), cx))
                    .child(add("add-diagram", "Diagram", || Layer::new("Diagram", 80.0, 190.0, 1100.0, 760.0, LayerKind::Diagram { diagram: "supercell".into() }), cx))
                    .child(add("add-particles", "Particles", || Layer::new("Snowfall", 0.0, 0.0, 1920.0, 1080.0, LayerKind::Particles { effect: "snow".into(), density: 3.0, speed: 1.0, color: "#FFFFFF".into() }), cx)))
                .child(muted("Media and crawl"))
                .child(row().gap(px(6.)).flex_wrap()
                    .child(add("add-image", "Image", || Layer::new("Image", 200.0, 200.0, 640.0, 360.0, LayerKind::Image { media: String::new(), fit: ImageFit::Contain }), cx))
                    .child(add("add-crawl", "Crawl", || Layer::new("Crawl", 0.0, 985.0, 1920.0, 95.0, LayerKind::Ticker { text: "{{alerts.ticker}}".into(), font: "$body".into(), size: 44.0, color: "#FFFFFF".into(), speed: 150.0 }), cx)))
                .when(!components.is_empty(), |d| d.child(muted("Saved components")).child(row().gap(px(6.)).flex_wrap().children(components)))
                .into_any_element()
        });

        let preview_menu = self.menu(GMenu::Preview, "contrast-2-line", "Preview", 420., cx, |this, cx| {
            col().gap(px(8.))
                .child(muted("Preview with"))
                .child(row().gap(px(6.)).flex_wrap().children(PreviewData::ALL.iter().enumerate().map(|(i, m)| {
                    let m = *m;
                    chip(("pdata", i), m.label(), this.preview_data == m).on_click(cx.listener(move |this, _, _, cx| {
                        this.preview_data = m;
                        this.request_render(cx);
                        cx.notify();
                    }))
                }).collect::<Vec<_>>()))
                .child(muted("Canvas shape"))
                .child(row().gap(px(6.)).flex_wrap().children([(None, "16:9"), (Some((1080u32, 1920u32)), "9:16"), (Some((1080, 1080)), "1:1"), (Some((1080, 1350)), "4:5")].into_iter().enumerate().map(|(i, (bp, text))| {
                    chip(("breakpoint", i), text, this.breakpoint == bp).on_click(cx.listener(move |this, _, _, cx| {
                        this.breakpoint = bp;
                        this.request_render(cx);
                        cx.notify();
                    }))
                }).collect::<Vec<_>>()))
                .child(chip("grid", "Layout grid and title-safe frame", this.show_grid).on_click(cx.listener(|this, _, _, cx| {
                    this.show_grid = !this.show_grid;
                    cx.notify();
                })))
                .into_any_element()
        });

        let export_menu = self.menu(GMenu::Export, "download-2-line", "Export", 380., cx, |_this, cx| {
            let item = |id: &'static str, text: &'static str, f: fn(&mut Self, &mut Window, &mut Context<Self>), cx: &mut Context<Self>| {
                button_small(id, text).on_click(cx.listener(move |this, _, window, cx| {
                    this.open_menu = None;
                    f(this, window, cx);
                }))
            };
            col().gap(px(8.))
                .child(muted("Still"))
                .child(row().gap(px(6.)).flex_wrap()
                    .child(item("export", "PNG", |this, _, cx| this.export_png(cx), cx))
                    .child(item("export-jpg", "JPEG", |this, _, cx| this.export_jpeg(cx), cx))
                    .child(item("export-sq", "Square (social)", |this, _, cx| this.export_variant("square", cx), cx))
                    .child(item("export-vert", "Vertical (social)", |this, _, cx| this.export_variant("vertical", cx), cx)))
                .child(muted("Motion"))
                .child(row().gap(px(6.)).flex_wrap()
                    .child(item("export-mp4", "MP4", |this, _, cx| this.export_video(false, cx), cx))
                    .child(item("export-webm", "WebM", |this, _, cx| this.export_video(true, cx), cx))
                    .child(item("export-seq", "Animation (PNG frames)", |this, _, cx| this.export_sequence(cx), cx))
                    .child(item("export-browser", "Browser graphic", |this, _, cx| this.export_browser(cx), cx)))
                .child(muted("Copies"))
                .child(item("make-vert", "Make a 9:16 copy for phones", |this, window, cx| {
                    let copy = this.doc.to_portrait();
                    let saved = this.app.update(cx, |s, cx| {
                        let saved = s.save_graphic(copy);
                        s.toast(ToastKind::Success, "Vertical (9:16) copy made for phones and social video");
                        cx.notify();
                        saved
                    });
                    if let Some(doc) = saved {
                        this.open(doc, window, cx);
                    }
                }, cx))
                .into_any_element()
        });

        let approved = !self.doc.approved_by.is_empty();
        let send_menu = self.menu(GMenu::Send, "send-plane-line", "Send", 360., cx, move |_this, cx| {
            col().gap(px(6.))
                .child(button_small("to-show", "Add to the show").on_click(cx.listener(|this, _, _, cx| {
                    this.open_menu = None;
                    this.save(cx);
                    let doc = this.doc.clone();
                    this.app.update(cx, |s, cx| {
                        s.show.scenes.push(Scene::graphic(&doc));
                        s.toast(ToastKind::Success, format!("Added \"{}\" to {}", doc.name, s.show.name));
                        cx.notify();
                    });
                })))
                .when(is_overlay, |d| d.child(button_small("as-overlay", "Put on the map").on_click(cx.listener(|this, _, _, cx| {
                    this.open_menu = None;
                    this.save(cx);
                    let id = this.doc.id.clone();
                    this.app.update(cx, |s, cx| {
                        s.set_map_overlay(Some(id));
                        s.toast(ToastKind::Success, "Overlay is on the map");
                        cx.notify();
                    });
                }))))
                .child(button_small("publish-all", "Publish to web and social").on_click(cx.listener(|this, _, _, cx| {
                    this.open_menu = None;
                    let doc = this.doc.clone();
                    this.app.update(cx, |s, cx| {
                        let targets: Vec<String> = s.settings.publish_targets.iter().map(|t| t.name.clone()).collect();
                        if targets.is_empty() {
                            s.modal = crate::app::Modal::Publishing;
                            s.toast(ToastKind::Info, "Add a publishing target first");
                        } else {
                            match crate::publish::enqueue(&doc, &s.settings, &targets) {
                                Ok(n) => s.toast(ToastKind::Success, format!("\"{}\" queued for {n} target(s)", doc.name)),
                                Err(e) => s.toast(ToastKind::Error, e.to_string()),
                            }
                        }
                        cx.notify();
                    });
                })))
                .child(button_small("approve", if approved { "Approved for air ✓ (click to withdraw)" } else { "Approve for air" }).on_click(cx.listener(|this, _, _, cx| {
                    let who = this.app.read(cx).settings.operator_name.clone();
                    if this.doc.approved_by.is_empty() {
                        this.doc.approved_by = who.clone();
                        crate::broadcast_ops::audit(&format!("APPROVED graphic {}", this.doc.name), &who);
                    } else {
                        this.doc.approved_by.clear();
                    }
                    this.changed(cx);
                })))
                .into_any_element()
        });

        let looks = row().h(px(28.)).rounded(px(6.)).border_1().border_color(c(BORDER)).bg(c(BG)).overflow_hidden()
            .children(crate::gfx::look::LOOKS.iter().enumerate().map(|(i, (key, text))| {
                let key: &'static str = *key;
                let active = look_now == key;
                div().id(("look", i)).h_full().flex().items_center().px(px(10.)).cursor_pointer().text_size(px(12.))
                    .when(i > 0, |d| d.border_l_1().border_color(c(BORDER)))
                    .when(active, |d| d.bg(crate::ui::ca(CYAN, 0.16)).text_color(c(CYAN)))
                    .when(!active, |d| d.text_color(c(TEXT)).hover(|s| s.bg(c(PANEL_2))))
                    .child(*text)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_look(key, cx)))
            }).collect::<Vec<_>>());

        row()
            .w_full()
            .items_center()
            .gap(px(6.))
            .px(px(10.))
            .py(px(7.))
            .bg(c(PANEL))
            .border_b_1()
            .border_color(c(BORDER))
            .flex_wrap()
            .child(gnav_btn("gfx-home", "apps-line", "All graphics", false).on_click(cx.listener(|this, _, _, cx| {
                this.autosave(cx);
                this.home = true;
                this.open_menu = None;
                cx.notify();
            })))
            .child(sep())
            .child(add_menu)
            .child(preview_menu)
            .child(sep())
            .child(muted("Look"))
            .child(looks)
            .child(sep())
            .child(button_small("undo", "Undo").on_click(cx.listener(|this, _, window, cx| this.undo(window, cx))))
            .child(button_small("redo", "Redo").on_click(cx.listener(|this, _, window, cx| this.redo(window, cx))))
            .child(
                button_small("play-anim", if playing { "■ Stop" } else { "▶ Play animation" }).on_click(cx.listener(|this, _, _, cx| {
                    this.playing = if this.playing.is_some() { None } else { Some(Instant::now()) };
                    this.request_render(cx);
                    cx.notify();
                })),
            )
            .child(div().flex_1())
            .child(button_small("save", if self.dirty { "Save •" } else { "Save" }).on_click(cx.listener(|this, _, _, cx| this.save(cx))))
            .child(export_menu)
            .child(send_menu)
            .child(button_danger("take", "TAKE TO AIR").h(px(26.)).on_click(cx.listener(|this, _, _, cx| {
                this.save(cx);
                let doc = this.doc.clone();
                this.app.update(cx, |s, cx| {
                    s.take_graphic(doc, Transition::Fade);
                    cx.notify();
                });
            })))
    }

    // ---- Landing page ------------------------------------------------------------------------

    /// Workspace graphics added without the editor (Build a show for me, a template taken to air)
    /// have no thumbnail file, so their Recent cards showed only an icon: draw the missing ones
    /// in the background, once.
    fn ensure_doc_thumbs(&mut self, docs: Vec<GraphicDoc>, cx: &mut Context<Self>) {
        let todo: Vec<GraphicDoc> = docs
            .into_iter()
            .filter(|g| !library::thumb_path(&g.id).exists() && self.thumbs_pending.insert(format!("doc:{}", g.id)))
            .collect();
        if todo.is_empty() {
            return;
        }
        let state = self.app.read(cx);
        let settings = state.settings.clone();
        let data = state.data.clone();
        let task = cx.background_executor().spawn(async move {
            for doc in todo {
                let (w, h) = thumb_dims(doc.width, doc.height);
                let pm = render_graphic(&doc, &settings, &data, SETTLED, w, h, Some(&checkerboard(w, h)).filter(|_| doc.is_overlay()));
                let path = library::thumb_path(&doc.id);
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Some(png) = crate::ops::pixmap_png(&pm) {
                    let _ = std::fs::write(path, png);
                }
            }
        });
        cx.spawn(async move |this, cx| {
            task.await;
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    /// Render small previews of templates (in the default look) in the background.
    fn ensure_thumbs(&mut self, keys: Vec<&'static str>, cx: &mut Context<Self>) {
        let look = crate::gfx::look::default();
        let todo: Vec<&'static str> = keys
            .into_iter()
            .filter(|k| {
                let id = format!("{look}:{k}");
                !self.thumbs.contains_key(&id) && !self.thumbs_pending.contains(&id)
            })
            .collect();
        if todo.is_empty() {
            return;
        }
        for k in &todo {
            self.thumbs_pending.insert(format!("{look}:{k}"));
        }
        let state = self.app.read(cx);
        let settings = state.settings.clone();
        let data = state.data.clone();
        let task = cx.background_executor().spawn(async move {
            todo.into_iter()
                .filter_map(|k| {
                    let doc = templates::build_in(k, &look)?;
                    // Render at the graphic's own shape (portrait, square or wide); the card letterboxes it.
                    let (w, h) = thumb_dims(doc.width, doc.height);
                    let pm = render_graphic(&doc, &settings, &data, SETTLED, w, h, Some(&checkerboard(w, h)).filter(|_| doc.is_overlay()));
                    Some((format!("{look}:{k}"), pixmap_to_image(pm)))
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let done = task.await;
            let _ = this.update(cx, |this, cx| {
                for (id, image) in done {
                    this.thumbs_pending.remove(&id);
                    this.thumbs.insert(id, image);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The document a card menu acts on (templates are built in the current default look).
    fn card_doc(target: &CardTarget) -> Option<GraphicDoc> {
        match target {
            CardTarget::Template(key) => templates::build_in(key, &crate::gfx::look::default()),
            CardTarget::Graphic(doc) => Some((**doc).clone()),
        }
    }

    fn card_action(&mut self, action: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some((target, _)) = self.card_menu.take() else { return };
        let Some(doc) = Self::card_doc(&target) else { return };
        match action {
            "show" => self.app.update(cx, |s, cx| {
                s.show.scenes.push(Scene::graphic(&doc));
                s.toast(ToastKind::Success, format!("Added \"{}\" to {}", doc.name, s.show.name));
                cx.notify();
            }),
            "air" => self.app.update(cx, |s, cx| {
                // Straight to Program: a template is not added to the workspace.
                s.take_graphic(doc, Transition::Fade);
                cx.notify();
            }),
            "workspace" => {
                self.app.update(cx, |s, cx| {
                    if let Some(g) = s.save_graphic(doc) {
                        s.toast(ToastKind::Success, format!("Added \"{}\" to your workspace", g.name));
                    }
                    cx.notify();
                });
            }
            "edit" => match target {
                CardTarget::Template(_) => {
                    let saved = self.app.update(cx, |s, cx| {
                        let saved = s.save_graphic(doc);
                        cx.notify();
                        saved
                    });
                    if let Some(doc) = saved {
                        self.open(doc, window, cx);
                    }
                }
                CardTarget::Graphic(_) => self.open(doc, window, cx),
            },
            "preview" => self.preview_template(doc, cx),
            "favourite" => {
                if let CardTarget::Template(key) = target {
                    self.toggle_favorite(key, cx);
                }
            }
            _ => {}
        }
        cx.notify();
    }

    /// Right-click menu for a gallery card: add to the show, add to the workspace, take to air.
    fn card_menu_popup(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (target, at) = self.card_menu.clone()?;
        let show_name = self.app.read(cx).show.name.clone();
        let is_template = matches!(target, CardTarget::Template(_));
        let fav = match &target {
            CardTarget::Template(key) => self.app.read(cx).settings.graphics_favorites.iter().any(|k| k == key),
            CardTarget::Graphic(_) => false,
        };
        let title = match &target {
            CardTarget::Template(key) => templates::all().find(|t| t.key == *key).map(|t| t.name.to_string()).unwrap_or_default(),
            CardTarget::Graphic(doc) => doc.name.clone(),
        };
        let item = |i: usize, icon: &'static str, text: String, action: &'static str, danger: bool| {
            row().id(("card-menu-item", i)).gap(px(8.)).px(px(10.)).py(px(7.)).rounded(px(6.)).cursor_pointer()
                .hover(|s| s.bg(c(PANEL_2)))
                .child(crate::ui::icons::ri(icon, 16., if danger { RED } else { TEXT }))
                .child(div().text_size(px(13.)).text_color(c(if danger { RED } else { TEXT })).child(text))
                .on_click(cx.listener(move |this, _, window, cx| this.card_action(action, window, cx)))
        };
        let mut items = vec![
            item(0, "play-list-add-line", format!("Add to show \"{show_name}\""), "show", false).into_any_element(),
        ];
        if is_template {
            items.push(item(1, "folder-add-line", "Add to workspace".into(), "workspace", false).into_any_element());
            items.push(item(2, "eye-line", "Preview".into(), "preview", false).into_any_element());
            items.push(item(3, "pencil-line", "Edit a copy".into(), "edit", false).into_any_element());
            items.push(item(4, "star-line", if fav { "Remove from favourites".into() } else { "Add to favourites".into() }, "favourite", false).into_any_element());
        } else {
            items.push(item(3, "pencil-line", "Open in the editor".into(), "edit", false).into_any_element());
        }
        items.push(div().h(px(1.)).my(px(4.)).bg(c(BORDER)).into_any_element());
        items.push(item(5, "broadcast-line", "Take to air".into(), "air", true).into_any_element());
        Some(
            deferred(
                anchored().position(at).snap_to_window_with_margin(px(8.)).child(
                    col()
                        .id("card-menu")
                        .occlude()
                        .w(px(260.))
                        .p(px(6.))
                        .gap(px(1.))
                        .rounded(px(8.))
                        .bg(c(PANEL))
                        .border_1()
                        .border_color(c(BORDER))
                        .shadow_lg()
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.card_menu = None;
                            cx.notify();
                        }))
                        .child(div().px(px(10.)).pt(px(4.)).pb(px(6.)).text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(c(MUTED)).truncate().child(title))
                        .children(items),
                ),
            )
            .with_priority(4)
            .into_any_element(),
        )
    }

    fn landing(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let q = self.lib_query.clone();
        let look = crate::gfx::look::default();
        let (favs, used) = {
            let s = &self.app.read(cx).settings;
            (s.graphics_favorites.clone(), s.graphics_recent_templates.clone())
        };
        let mut recent: Vec<GraphicDoc> = self.app.read(cx).graphics.clone();
        recent.sort_by_key(|g| std::cmp::Reverse(g.updated));
        let all: Vec<&'static templates::TemplateInfo> = templates::all().collect();
        let searching = !q.is_empty();
        let (fmt, live) = (self.home_format, self.home_live);
        let pass = |t: &&'static templates::TemplateInfo| fmt.is_none_or(|f| nav::format_of(t) == f) && live.is_none_or(|l| nav::is_live(t) == l);
        let by_keys = |keys: &[String]| -> Vec<&'static templates::TemplateInfo> { keys.iter().filter_map(|k| all.iter().copied().find(|t| t.key == k.as_str())).collect() };
        let mut shown: Vec<&'static templates::TemplateInfo> = if searching {
            all.iter().copied().filter(|t| nav::matches(t, &q)).collect()
        } else {
            match self.home_category {
                Some(HOME_FAV) => by_keys(&favs),
                Some(HOME_USED) => by_keys(&used),
                Some(cat) => all.iter().copied().filter(|t| t.category.eq_ignore_ascii_case(cat)).collect(),
                None => Vec::new(),
            }
        };
        shown.retain(|t| pass(t));
        // Sub-groups (explainer topics, history periods, a set's variants) inside a category.
        let in_category = !searching && self.home_category.is_some_and(|c| c != HOME_FAV && c != HOME_USED);
        let mut subs: Vec<String> = Vec::new();
        if in_category {
            for t in &shown {
                let s = nav::subgroup(t, &all);
                if !subs.contains(&s) {
                    subs.push(s);
                }
            }
            if let Some(sub) = &self.home_sub {
                shown.retain(|t| &nav::subgroup(t, &all) == sub);
            }
            // Keep sub-groups together, in first-seen order.
            shown.sort_by_key(|t| subs.iter().position(|s| *s == nav::subgroup(t, &all)).unwrap_or(0));
        }
        let total = shown.len();
        let limited: Vec<&'static templates::TemplateInfo> = shown.iter().copied().take(self.home_limit).collect();
        self.ensure_thumbs(limited.iter().map(|t| t.key).collect(), cx);

        let card = |id: ElementId, image: Option<AnyElement>, icon: &'static str, title: String, sub: String| {
            col()
                .id(id)
                .w(px(248.))
                .flex_shrink_0()
                .relative()
                .rounded(px(8.))
                .overflow_hidden()
                .bg(c(PANEL_2))
                .border_1()
                .border_color(c(BORDER))
                .cursor_pointer()
                .hover(|s| s.border_color(c(CYAN)))
                .child(match image {
                    Some(image) => image,
                    None => div().w(px(248.)).h(px(140.)).flex_shrink_0().bg(c(0x0E1320)).flex().items_center().justify_center().child(crate::ui::icons::ri(icon, 34., MUTED)).into_any_element(),
                })
                .child(
                    // The text block is drawn on its own background so nothing above can show through it.
                    col().p(px(10.)).gap(px(3.)).min_w_0().flex_shrink_0().bg(c(PANEL_2))
                        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).truncate().child(title))
                        .child(div().h(px(30.)).overflow_hidden().text_size(px(11.)).text_color(c(MUTED)).child(sub)),
                )
        };

        // Recent graphics from the workspace.
        self.ensure_doc_thumbs(recent.iter().take(if searching { 24 } else { 8 }).cloned().collect(), cx);
        let recent_cards = recent
            .iter()
            .filter(|g| !searching || g.name.to_lowercase().contains(&q) || g.category.to_lowercase().contains(&q) || g.tags.iter().any(|t| t.contains(&q)))
            .take(if searching { 24 } else { 8 })
            .enumerate()
            .map(|(i, g)| {
                let thumb = library::thumb_path(&g.id);
                let image = thumb.exists().then(|| crate::ui::thumb_box_path(thumb, 248., 140., 0x05070B, false).into_any_element());
                let doc = g.clone();
                let sub = format!("{} • {}", g.category, if crate::gfx::look::of(g) == "modern" { "Modern" } else { "Classic" });
                let menu_doc = g.clone();
                card(("home-recent", i).into(), image, category_icon(&g.category), g.name.clone(), sub)
                    .on_click(cx.listener(move |this, _, window, cx| this.open(doc.clone(), window, cx)))
                    .on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        this.card_menu = Some((CardTarget::Graphic(Box::new(menu_doc.clone())), e.position));
                        cx.notify();
                    }))
            })
            .collect::<Vec<_>>();

        // Category tiles (landing, nothing picked yet). Social Graphics comes first.
        let social_open = self.home_category == Some(social::CATEGORY);
        let mut categories = vec![social::CATEGORY];
        categories.extend(templates::categories().iter().copied());
        let count_of = |cat: &str| all.iter().filter(|t| t.category.eq_ignore_ascii_case(cat)).count();
        let tiles = categories
            .iter()
            .enumerate()
            .map(|(i, cat)| {
                let cat: &'static str = *cat;
                let count_text = if cat == social::CATEGORY { "Alert cards, forecast posts".to_string() } else { format!("{} templates", count_of(cat)) };
                row()
                    .id(("home-cat", i))
                    .w(px(212.))
                    .h(px(64.))
                    .px(px(12.))
                    .gap(px(10.))
                    .items_center()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .border_1()
                    .bg(c(PANEL_2))
                    .border_color(c(BORDER))
                    .hover(|s| s.border_color(c(CYAN)))
                    .child(crate::ui::icons::ri(category_icon(cat), 22., TEXT))
                    .child(col().min_w_0()
                        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).truncate().child(cat))
                        .child(div().text_size(px(11.)).text_color(c(MUTED)).truncate().child(count_text)))
                    .on_click(cx.listener(move |this, _, _, cx| this.pick_home_category(Some(cat), cx)))
            })
            .collect::<Vec<_>>();

        // Template cards, with a favourite star and sub-group headings.
        let mut grid_sections: Vec<(String, Vec<AnyElement>)> = Vec::new();
        for (i, t) in limited.iter().enumerate() {
            let key = t.key;
            let image = self.thumbs.get(&format!("{look}:{key}")).cloned().map(|image| crate::ui::thumb_box(image, 248., 140., 0x05070B).into_any_element());
            let fav = favs.iter().any(|k| k == key);
            let fmt_label = nav::format_of(t).label();
            let sub = format!("{} • {}", fmt_label, t.description);
            let star = div()
                .id(("home-fav", i))
                .absolute()
                .top(px(6.))
                .right(px(6.))
                .p(px(4.))
                .rounded(px(6.))
                .bg(if fav { crate::ui::ca(0x6B5200, 0.85) } else { crate::ui::ca(0x000000, 0.55) })
                .cursor_pointer()
                .hover(|s| s.bg(crate::ui::ca(0x000000, 0.8)))
                .child(crate::ui::icons::ri("star-line", 18., if fav { 0xFFD34D } else { 0xFFFFFF }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.toggle_favorite(key, cx);
                }));
            let el = card(("home-tpl", i).into(), image, category_icon(t.category), t.name.to_string(), sub)
                .child(star)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(doc) = templates::build(key) {
                        this.preview_template(doc, cx);
                    }
                }))
                .on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.card_menu = Some((CardTarget::Template(key), e.position));
                    cx.notify();
                }))
                .into_any_element();
            let heading = if in_category && subs.len() > 1 && self.home_sub.is_none() { nav::subgroup(t, &all) } else { String::new() };
            match grid_sections.last_mut() {
                Some((h, items)) if *h == heading => items.push(el),
                _ => grid_sections.push((heading, vec![el])),
            }
        }

        let look_chips = crate::gfx::look::LOOKS.iter().enumerate().map(|(i, (key, text))| {
            let key: &'static str = *key;
            chip(("home-look", i), *text, look == key).on_click(cx.listener(move |this, _, _, cx| {
                this.app.update(cx, |s, cx| {
                    let mut next = s.settings.clone();
                    next.graphics_look = key.to_string();
                    s.apply_settings(next);
                    cx.notify();
                });
                cx.notify();
            }))
        }).collect::<Vec<_>>();

        let (profiles, active_brand) = {
            let s = &self.app.read(cx).settings;
            (s.brand_profiles_or_defaults(), s.current_brand_profile().name)
        };
        let brand_chips = profiles.into_iter().enumerate().map(|(i, p)| {
            let name = p.name.clone();
            chip(("home-brand", i), p.name.clone(), p.name == active_brand).on_click(cx.listener(move |this, _, _, cx| {
                let name = name.clone();
                this.app.update(cx, |s, cx| {
                    let mut next = s.settings.clone();
                    next.switch_brand(&name);
                    s.apply_settings(next);
                    s.toast(ToastKind::Success, format!("Graphics now use the {name} branding"));
                    cx.notify();
                });
                this.thumbs.clear();
                this.request_render(cx);
                cx.notify();
            }))
        }).collect::<Vec<_>>();

        // Filters (sticky, under the search).
        let fmt_chips = std::iter::once(None).chain(nav::Format::ALL.iter().copied().map(Some)).enumerate().map(|(i, f)| {
            chip(("home-fmt", i), f.map(|f| f.label()).unwrap_or("All"), self.home_format == f).on_click(cx.listener(move |this, _, _, cx| {
                this.home_format = f;
                this.home_limit = HOME_PAGE;
                cx.notify();
            }))
        }).collect::<Vec<_>>();
        let live_chips = [(None, "All"), (Some(true), "Live data"), (Some(false), "Explainers / static")].into_iter().enumerate().map(|(i, (l, text))| {
            chip(("home-live", i), text, self.home_live == l).on_click(cx.listener(move |this, _, _, cx| {
                this.home_live = l;
                this.home_limit = HOME_PAGE;
                cx.notify();
            }))
        }).collect::<Vec<_>>();

        // Sidebar: favourites, recently used, then category groups with counts.
        let side_item = |id: ElementId, icon: &'static str, text: String, count: String, active: bool, indent: bool| {
            row()
                .id(id)
                .w_full()
                .pl(px(if indent { 30. } else { 10. }))
                .pr(px(10.))
                .py(px(if indent { 4. } else { 6. }))
                .gap(px(8.))
                .items_center()
                .rounded(px(6.))
                .cursor_pointer()
                .when(active, |d| d.bg(crate::ui::ca(CYAN, 0.16)))
                .when(!active, |d| d.hover(|s| s.bg(crate::ui::ca(0xFFFFFF, 0.06))))
                .when(!indent, |d| d.child(crate::ui::icons::ri(icon, 16., if active { CYAN } else { MUTED })))
                .child(div().flex_1().min_w_0().truncate().text_size(px(if indent { 12. } else { 13. })).text_color(c(if active { TEXT } else { if indent { MUTED } else { TEXT } })).child(text))
                .child(div().text_size(px(11.)).text_color(c(MUTED)).child(count))
        };
        let mut side: Vec<AnyElement> = Vec::new();
        side.push(side_item("home-side-fav".into(), "star-line", "Favourites".into(), favs.len().to_string(), self.home_category == Some(HOME_FAV), false)
            .on_click(cx.listener(|this, _, _, cx| this.pick_home_category(Some(HOME_FAV), cx))).into_any_element());
        side.push(side_item("home-side-used".into(), "history-line", "Recently used".into(), used.len().to_string(), self.home_category == Some(HOME_USED), false)
            .on_click(cx.listener(|this, _, _, cx| this.pick_home_category(Some(HOME_USED), cx))).into_any_element());
        let mut n: usize = 0;
        for group in nav::GROUPS {
            let cats: Vec<&'static str> = categories.iter().copied().filter(|c| if *c == social::CATEGORY { *group == "Social & sharing" } else { nav::group_of(c) == *group }).collect();
            if cats.is_empty() {
                continue;
            }
            side.push(div().pt(px(12.)).pb(px(4.)).px(px(10.)).text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(c(MUTED)).child(group.to_uppercase()).into_any_element());
            for cat in cats {
                n += 1;
                let active = self.home_category == Some(cat);
                let count = if cat == social::CATEGORY { String::new() } else { count_of(cat).to_string() };
                side.push(side_item(("home-side-cat", n).into(), category_icon(cat), cat.to_string(), count, active, false)
                    .on_click(cx.listener(move |this, _, _, cx| this.pick_home_category(Some(cat), cx))).into_any_element());
                if active && subs.len() > 1 {
                    for (j, s) in subs.iter().enumerate() {
                        let s2 = s.clone();
                        let label = if s.is_empty() { "Other".to_string() } else { s.clone() };
                        let count = all.iter().filter(|t| t.category.eq_ignore_ascii_case(cat) && nav::subgroup(t, &all) == *s).count();
                        side.push(side_item(("home-side-sub", j).into(), "", label, count.to_string(), self.home_sub.as_deref() == Some(s.as_str()), true)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.home_sub = if this.home_sub.as_deref() == Some(s2.as_str()) { None } else { Some(s2.clone()) };
                                this.home_limit = HOME_PAGE;
                                cx.notify();
                            })).into_any_element());
                    }
                }
            }
        }

        let section_title = |text: String| div().pt(px(18.)).pb(px(8.)).text_size(px(15.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).child(text);
        let social_panel = (social_open && !searching).then(|| self.social_panel(cx));
        let listing = searching || (self.home_category.is_some() && !social_open);
        let title = if searching {
            format!("Templates matching \"{q}\" ({total})")
        } else {
            match self.home_category {
                Some(HOME_FAV) => format!("Favourites ({total})"),
                Some(HOME_USED) => format!("Recently used ({total})"),
                Some(cat) => match &self.home_sub {
                    Some(s) => format!("{cat} › {s} ({total})"),
                    None => format!("{cat} ({total})"),
                },
                None => String::new(),
            }
        };
        let empty_hint = match self.home_category {
            Some(HOME_FAV) if !searching => "No favourites yet. Click the star on any template to keep it here.",
            Some(HOME_USED) if !searching => "Templates you open will show here.",
            _ => "Nothing matches. Try another word or clear the filters.",
        };
        let more = total.saturating_sub(limited.len());

        let top_bar = col()
            .w_full()
            .flex_shrink_0()
            .px(px(24.))
            .pt(px(16.))
            .pb(px(10.))
            .gap(px(8.))
            .border_b_1()
            .border_color(c(BORDER))
            .bg(c(BG))
            .child(
                row().w_full().items_center().gap(px(10.)).flex_wrap()
                    .child(div().text_size(px(22.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).child("Graphics Studio"))
                    .child(div().w(px(360.)).child(Input::new(&self.lib_filter)))
                    .child(muted("Enter opens the first match"))
                    .child(div().flex_1())
                    .child(gnav_btn("home-new-full", "add-line", "New full screen", false).on_click(cx.listener(|this, _, window, cx| {
                        let mut doc = templates::blank(false);
                        let look = crate::gfx::look::default();
                        crate::gfx::look::apply(&mut doc, &look);
                        this.open(doc, window, cx);
                        this.dirty = true;
                    })))
                    .child(gnav_btn("home-new-ovl", "stack-line", "New overlay", false).on_click(cx.listener(|this, _, window, cx| {
                        let doc = templates::blank(true);
                        this.open(doc, window, cx);
                        this.dirty = true;
                    })))
                    .child(gnav_btn("home-editor", "pencil-line", format!("Back to \"{}\"", self.doc.name), false).on_click(cx.listener(|this, _, _, cx| {
                        this.home = false;
                        this.request_render(cx);
                        cx.notify();
                    }))),
            )
            .child(
                row().gap(px(8.)).items_center().flex_wrap()
                    .child(label("Format"))
                    .children(fmt_chips)
                    .child(div().w(px(14.)))
                    .child(label("Data"))
                    .children(live_chips),
            )
            .child(
                row().gap(px(8.)).items_center().flex_wrap()
                    .child(muted("New graphics use the"))
                    .children(look_chips)
                    .child(muted("look. Each graphic can switch with Look in the editor."))
                    .child(div().w(px(16.)))
                    .child(muted("Branding"))
                    .children(brand_chips)
                    .child(div().flex_1())
                    .child(button_small("home-all-modern", "Make all my graphics Modern").on_click(cx.listener(|this, _, _, cx| this.restyle_library("modern", cx))))
                    .child(button_small("home-all-classic", "Make all Classic").on_click(cx.listener(|this, _, _, cx| this.restyle_library("classic", cx)))),
            );

        let content = col()
            .id("gfx-home")
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .px(px(24.))
            .pb(px(24.))
            .when(!recent_cards.is_empty() && (searching || self.home_category.is_none()), |d| d
                .child(section_title(if searching { "Your graphics".into() } else { "Recent".into() }))
                .child(row().w_full().gap(px(12.)).flex_wrap().children(recent_cards)))
            .when(!searching && self.home_category.is_none(), |d| d
                .child(section_title("Templates by category".into()))
                .child(row().w_full().gap(px(10.)).flex_wrap().children(tiles))
                .child(div().pt(px(10.)).child(muted("Pick a category here or in the list on the left. Star templates to keep them under Favourites."))))
            .when_some(social_panel, |d, panel| d.child(section_title(social::CATEGORY.into())).child(panel))
            .when(listing, |d| d.child(section_title(title)))
            .when(listing && total == 0, |d| d.child(muted(empty_hint)))
            .children(grid_sections.into_iter().enumerate().map(|(i, (heading, items))| {
                col().id(("home-sec", i)).w_full()
                    .when(!heading.is_empty(), |d| d.child(div().pt(px(12.)).pb(px(6.)).text_size(px(13.)).font_weight(FontWeight::BOLD).text_color(c(CYAN)).child(heading)))
                    .child(row().w_full().gap(px(12.)).flex_wrap().children(items))
            }))
            .when(more > 0, |d| d.child(
                row().pt(px(14.)).gap(px(10.)).items_center()
                    .child(button_small("home-more", format!("Show {} more", more.min(HOME_PAGE))).on_click(cx.listener(|this, _, _, cx| {
                        this.home_limit += HOME_PAGE;
                        cx.notify();
                    })))
                    .child(muted(format!("{more} not shown yet"))),
            ));

        row()
            .size_full()
            .child(
                col()
                    .id("gfx-home-side")
                    .w(px(260.))
                    .h_full()
                    .flex_shrink_0()
                    .overflow_y_scroll()
                    .p(px(8.))
                    .gap(px(1.))
                    .bg(c(PANEL))
                    .border_r_1()
                    .border_color(c(BORDER))
                    .child(div().px(px(10.)).py(px(8.)).text_size(px(15.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).child("Templates"))
                    .children(side),
            )
            .child(col().flex_1().min_w_0().h_full().child(top_bar).child(content))
    }

    /// Open a landing category (or Favourites / Recently used); clicking the open one closes it.
    fn pick_home_category(&mut self, cat: Option<&'static str>, cx: &mut Context<Self>) {
        self.home_category = if self.home_category == cat { None } else { cat };
        self.home_sub = None;
        self.home_limit = HOME_PAGE;
        cx.notify();
    }

    fn toggle_favorite(&mut self, key: &'static str, cx: &mut Context<Self>) {
        self.app.update(cx, |s, cx| {
            let mut next = s.settings.clone();
            if let Some(i) = next.graphics_favorites.iter().position(|k| k == key) {
                next.graphics_favorites.remove(i);
            } else {
                next.graphics_favorites.push(key.to_string());
            }
            s.apply_settings(next);
            cx.notify();
        });
        cx.notify();
    }

    /// Remember an opened template (newest first, 24 kept).
    fn remember_template(&mut self, key: &str, cx: &mut Context<Self>) {
        self.app.update(cx, |s, _| {
            let mut next = s.settings.clone();
            next.graphics_recent_templates.retain(|k| k != key);
            next.graphics_recent_templates.insert(0, key.to_string());
            next.graphics_recent_templates.truncate(24);
            s.apply_settings(next);
        });
    }

    /// Restyle every workspace graphic in a look (reversible: switch back the same way).
    fn restyle_library(&mut self, look: &'static str, cx: &mut Context<Self>) {
        let graphics = self.app.read(cx).graphics.clone();
        let mut changed = 0;
        self.app.update(cx, |s, cx| {
            for mut g in graphics {
                if crate::gfx::look::of(&g) != look || look == "modern" {
                    crate::gfx::look::apply(&mut g, look);
                    if s.save_graphic(g).is_some() {
                        changed += 1;
                    }
                }
            }
            s.toast(ToastKind::Success, format!("{changed} graphic(s) switched to the {} look", if look == "modern" { "Modern" } else { "Classic" }));
            cx.notify();
        });
        if crate::gfx::look::of(&self.doc) != look {
            crate::gfx::look::apply(&mut self.doc, look);
            self.request_render(cx);
        }
        cx.notify();
    }

    fn inspector(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let brand = self.app.read(cx).settings.brand.clone();
        // (name, file) of each library image: the image layer's picker shows thumbnails.
        let media: Vec<(String, std::path::PathBuf)> = self
            .app
            .read(cx)
            .media
            .iter()
            .filter(|m| m.kind == MediaKind::Image)
            .map(|m| (m.name.clone(), m.path.clone()))
            .collect();
        let layer = self.selected.as_ref().and_then(|id| self.doc.layer(id)).cloned();

        let panel = col()
            .id("gfx-inspector")
            .w(px(320.))
            .h_full()
            .flex_shrink_0()
            .bg(c(PANEL))
            .border_l_1()
            .border_color(c(BORDER))
            .overflow_y_scroll();

        let Some(layer) = layer else {
            return panel.child(self.document_panel(cx)).child(self.layers_list(cx));
        };

        let color_swatches = GFX_COLORS
            .iter()
            .enumerate()
            .map(|(i, token)| {
                let token = token.to_string();
                swatch(("color", i), &display_hex(&token, &brand), false).on_click(cx.listener(move |this, _, window, cx| {
                    this.set_color(token.clone(), cx);
                    this.load_inputs(window, cx);
                }))
            })
            .collect::<Vec<_>>();

        let palette_swatches = brand_palette(&brand)
            .into_iter()
            .enumerate()
            .map(|(i, hex)| {
                let h = hex.clone();
                swatch(("pal", i), &hex, false).on_click(cx.listener(move |this, _, window, cx| {
                    this.set_color(h.clone(), cx);
                    this.load_inputs(window, cx);
                }))
            })
            .collect::<Vec<_>>();
        let fit_note = {
            let state = self.app.read(cx);
            let bindings = crate::gfx::bind::Bindings::build(&state.data, &state.settings, None, &state.settings.radar_palette);
            crate::gfx::check::text_fit(&layer, &bindings, &brand)
        };

        let containers: Vec<(String, String)> = self
            .doc
            .layers
            .iter()
            .filter(|l| l.id != layer.id && matches!(l.kind, LayerKind::Rect { .. }))
            .map(|l| (l.id.clone(), l.name.clone()))
            .collect();
        let anchor_now = layer.anchor.clone();
        let parent_now = layer.parent.clone();
        let stack_now = layer.stack.clone();
        let is_rect = matches!(layer.kind, LayerKind::Rect { .. });
        let layout_section = section("Layout")
            .child(muted("Anchor (other sizes: vertical, square, social)"))
            .child(row().flex_wrap().gap(px(3.)).children(["", "top-left", "top", "top-right", "left", "center", "right", "bottom-left", "bottom", "bottom-right"].into_iter().enumerate().map(|(i, a)| {
                let label = if a.is_empty() { "Scale".to_string() } else { a.replace('-', " ") };
                chip(("anchor", i), label, anchor_now == a).on_click(cx.listener(move |this, _, _, cx| {
                    this.edit_layer(cx, true, |l| l.anchor = a.to_string());
                }))
            }).collect::<Vec<_>>()))
            .child(toggle("stretch", "Keep side margins (width stretches)", layer.stretch).on_click(cx.listener(|this, _, _, cx| {
                this.edit_layer(cx, true, |l| l.stretch = !l.stretch);
            })))
            .child(muted("Inside container"))
            .child(row().flex_wrap().gap(px(3.)).children(std::iter::once((String::new(), "None".to_string())).chain(containers.into_iter()).enumerate().map(|(i, (id, name))| {
                let on = parent_now == id;
                chip(("parent", i), name, on).on_click(cx.listener(move |this, _, _, cx| {
                    let id = id.clone();
                    this.edit_layer(cx, true, |l| l.parent = id);
                }))
            }).collect::<Vec<_>>()))
            .when(is_rect, |d| {
                d.child(muted("Arrange what is inside this container"))
                    .child(row().gap(px(3.)).children([("", "Free"), ("vertical", "Stack down"), ("horizontal", "Stack across")].into_iter().enumerate().map(|(i, (key, text))| {
                        chip(("stack", i), text, stack_now == key).on_click(cx.listener(move |this, _, _, cx| {
                            this.edit_layer(cx, true, |l| {
                                l.stack = key.to_string();
                                if l.gap <= 0.0 {
                                    l.gap = 16.0;
                                }
                            });
                        }))
                    }).collect::<Vec<_>>()))
                    .child(button_small("save-component", "Save as component").on_click(cx.listener(|this, _, _, cx| this.save_component(cx))))
            });
        let anim_section = self.animation_section(&layer, cx);
        let mut kind_section = section(layer.kind.type_label());
        if matches!(layer.kind, LayerKind::Text { .. }) {
            let reveal = layer.reveal.clone();
            kind_section = kind_section
                .child(muted("Reveal (after the in-animation starts). Rich text: **bold**, [c=#FF3B3B]colour[/c], [s=1.4]size[/s]"))
                .child(row().flex_wrap().gap(px(4.)).children([("", "None"), ("typewriter", "Typewriter"), ("words", "Word by word")].into_iter().enumerate().map(|(i, (key, text))| {
                    chip(("reveal", i), text, reveal == key).on_click(cx.listener(move |this, _, _, cx| {
                        this.edit_layer(cx, true, |l| {
                            l.reveal = key.to_string();
                            if l.reveal_secs <= 0.0 {
                                l.reveal_secs = 1.5;
                            }
                        });
                        this.playing = Some(Instant::now());
                    }))
                }).collect::<Vec<_>>()))
                .child(button_small("import-fonts", "Add fonts…").on_click(cx.listener(|_this, _, _, cx| {
                    crate::ui::pick_off_thread(cx, || rfd::FileDialog::new().add_filter("Fonts", &["ttf", "otf"]).pick_files(), |this: &mut Self, files, cx| {
                        let result = crate::gfx::text::import_fonts(&files);
                        this.app.update(cx, |s, cx| {
                            match result {
                                Ok(n) => s.toast(ToastKind::Success, format!("{n} font file(s) added. Restart OpenWX to use them.")),
                                Err(e) => s.toast(ToastKind::Error, format!("Could not add fonts: {e}")),
                            }
                            cx.notify();
                        });
                    });
                })));
        }
        match &layer.kind {
            LayerKind::Text { size, weight, align, uppercase, shadow, wrap, auto_fit, font, letter_spacing, .. } => {
                let fonts = std::iter::once("$display")
                    .chain(std::iter::once("$body"))
                    .chain(family_names())
                    .enumerate()
                    .map(|(i, f)| {
                        let name = f.to_string();
                        let shown = match f {
                            "$display" => format!("Brand display ({})", brand.font_display),
                            "$body" => format!("Brand body ({})", brand.font_body),
                            other => other.to_string(),
                        };
                        chip(("font", i), shown, *font == name).on_click(cx.listener(move |this, _, _, cx| {
                            let name = name.clone();
                            this.edit_layer(cx, true, |l| {
                                if let LayerKind::Text { font, .. } = &mut l.kind {
                                    *font = name;
                                }
                            });
                        }))
                    })
                    .collect::<Vec<_>>();
                let weights = Weight::ALL
                    .iter()
                    .enumerate()
                    .map(|(i, w)| {
                        let w = *w;
                        chip(("weight", i), w.label(), *weight == w).on_click(cx.listener(move |this, _, _, cx| {
                            this.edit_layer(cx, true, |l| {
                                if let LayerKind::Text { weight, .. } = &mut l.kind {
                                    *weight = w;
                                }
                            });
                        }))
                    })
                    .collect::<Vec<_>>();
                let aligns = [(Align::Left, "Left"), (Align::Center, "Center"), (Align::Right, "Right")]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (a, text))| {
                        chip(("align", i), text, *align == a).on_click(cx.listener(move |this, _, _, cx| {
                            this.edit_layer(cx, true, |l| {
                                if let LayerKind::Text { align, .. } = &mut l.kind {
                                    *align = a;
                                }
                            });
                        }))
                    })
                    .collect::<Vec<_>>();
                let q = self.bind_query.clone();
                let bindings = CATALOG
                    .iter()
                    .chain(CATALOG_EXT.iter())
                    .filter(|(key, desc)| q.is_empty() || key.to_lowercase().contains(&q) || desc.to_lowercase().contains(&q))
                    .take(if q.is_empty() { 30 } else { 80 })
                    .enumerate()
                    .map(|(i, (key, desc))| {
                        let token = format!("{{{{{key}}}}}");
                        chip(("bind", i), format!("{{{{{key}}}}}"), false)
                            .tooltip({
                                let desc = desc.to_string();
                                move |_, cx| cx.new(|_| TooltipText(desc.clone())).into()
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                let token = token.clone();
                                let current = this.content.read(cx).value().to_string();
                                let next = format!("{current}{token}");
                                Self::set_input(&this.content.clone(), next.clone(), window, cx);
                                this.edit_layer(cx, true, |l| {
                                    if let LayerKind::Text { text, .. } = &mut l.kind {
                                        *text = next;
                                    }
                                });
                            }))
                    })
                    .collect::<Vec<_>>();
                let (size, spacing) = (*size, *letter_spacing);
                let (uppercase, shadow, wrap, auto_fit) = (*uppercase, *shadow, *wrap, *auto_fit);
                let outline_w = match &layer.kind {
                    LayerKind::Text { outline, .. } => *outline,
                    _ => 0.0,
                };
                // Typography presets: (name, font, size, weight, uppercase, spacing).
                let presets: [(&str, &str, f32, Weight, bool, f32); 5] = [
                    ("Headline", "$display", 96.0, Weight::Black, true, 3.0),
                    ("Title", "$display", 64.0, Weight::Bold, false, 0.0),
                    ("Body", "$body", 40.0, Weight::Regular, false, 0.0),
                    ("Caption", "$body", 28.0, Weight::SemiBold, true, 2.0),
                    ("Big number", "$display", 200.0, Weight::Black, false, 0.0),
                ];
                let preset_chips = presets
                    .into_iter()
                    .enumerate()
                    .map(|(i, (name, f, s, w, up, sp))| {
                        chip(("typo", i), name, false).on_click(cx.listener(move |this, _, _, cx| {
                            this.edit_layer(cx, true, |l| {
                                if let LayerKind::Text { font, size, weight, uppercase, letter_spacing, .. } = &mut l.kind {
                                    *font = f.to_string();
                                    *size = s;
                                    *weight = w;
                                    *uppercase = up;
                                    *letter_spacing = sp;
                                }
                            });
                        }))
                    })
                    .collect::<Vec<_>>();
                let fit_text = match fit_note {
                    Some(f) if f.overflows => Some(("Text spills out of its box. Turn on Shrink to fit or make the box bigger.", true)),
                    Some(f) if f.scale < 0.7 => Some(("Text is shrunk a lot to fit. Consider a bigger box or shorter wording.", false)),
                    _ => None,
                };
                kind_section = kind_section
                    .child(Input::new(&self.content))
                    .when_some(fit_text, |d, (t, bad)| d.child(div().text_size(px(12.)).text_color(c(if bad { RED } else { crate::ui::AMBER })).child(t)))
                    .child(muted("Style"))
                    .child(row().flex_wrap().gap(px(4.)).children(preset_chips))
                    .child(muted("Live data (click to insert)"))
                    .child(Input::new(&self.bind_filter))
                    .child(row().flex_wrap().gap(px(4.)).children(bindings))
                    .child(row().justify_between().child(muted("Outline")).child(muted(format!("{outline_w:.0}px"))))
                    .child(slider("text-outline", outline_w, 0.0, 12.0, cx, |this, v, _, cx| {
                        this.edit_layer(cx, false, |l| {
                            if let LayerKind::Text { outline, .. } = &mut l.kind {
                                *outline = v.round();
                            }
                        });
                    }))
                    .child(row().justify_between().child(muted("Size")).child(muted(format!("{size:.0}px"))))
                    .child(slider("text-size", size, 10.0, 320.0, cx, |this, v, _, cx| {
                        this.edit_layer(cx, false, |l| {
                            if let LayerKind::Text { size, .. } = &mut l.kind {
                                *size = v.round();
                            }
                        });
                    }))
                    .child(row().justify_between().child(muted("Letter spacing")).child(muted(format!("{spacing:.0}"))))
                    .child(slider("text-spacing", spacing, -4.0, 30.0, cx, |this, v, _, cx| {
                        this.edit_layer(cx, false, |l| {
                            if let LayerKind::Text { letter_spacing, .. } = &mut l.kind {
                                *letter_spacing = v.round();
                            }
                        });
                    }))
                    .child(row().flex_wrap().gap(px(4.)).children(weights))
                    .child(row().gap(px(4.)).children(aligns))
                    .child(toggle("upper", "UPPERCASE", uppercase).on_click(cx.listener(|this, _, _, cx| {
                        this.edit_layer(cx, true, |l| {
                            if let LayerKind::Text { uppercase, .. } = &mut l.kind {
                                *uppercase = !*uppercase;
                            }
                        });
                    })))
                    .child(toggle("shadow", "Drop shadow", shadow).on_click(cx.listener(|this, _, _, cx| {
                        this.edit_layer(cx, true, |l| {
                            if let LayerKind::Text { shadow, .. } = &mut l.kind {
                                *shadow = !*shadow;
                            }
                        });
                    })))
                    .child(toggle("wrap", "Wrap lines", wrap).on_click(cx.listener(|this, _, _, cx| {
                        this.edit_layer(cx, true, |l| {
                            if let LayerKind::Text { wrap, .. } = &mut l.kind {
                                *wrap = !*wrap;
                            }
                        });
                    })))
                    .child(toggle("autofit", "Shrink to fit", auto_fit).on_click(cx.listener(|this, _, _, cx| {
                        this.edit_layer(cx, true, |l| {
                            if let LayerKind::Text { auto_fit, .. } = &mut l.kind {
                                *auto_fit = !*auto_fit;
                            }
                        });
                    })))
                    .child(muted("Font"))
                    .child(row().flex_wrap().gap(px(4.)).children(fonts));
            }
            LayerKind::RiskMap { spec } => {
                // Pickers write the same "hazard:day" spec that Content takes.
                let (hazard_now, day_now) = {
                    let (h, d) = spec.trim().split_once(':').unwrap_or((spec.trim(), "1"));
                    (h.trim().to_ascii_lowercase(), d.trim().parse::<u8>().unwrap_or(1).clamp(1, 3))
                };
                let hazard_now = if hazard_now.is_empty() { "categorical".to_string() } else { hazard_now };
                let set = move |this: &mut Self, cx: &mut Context<Self>, hazard: String, day: u8| {
                    this.edit_layer(cx, true, |l| {
                        if let LayerKind::RiskMap { spec } = &mut l.kind {
                            *spec = format!("{hazard}:{day}");
                        }
                    });
                };
                let hazards = [("categorical", "Overall risk"), ("tornado", "Tornado"), ("wind", "Wind"), ("hail", "Hail"), ("flood", "Flooding rain")];
                kind_section = kind_section
                    .child(muted("County outlook map around home (SPC; flooding rain is WPC)."))
                    .child(row().flex_wrap().gap(px(4.)).children(hazards.into_iter().enumerate().map(|(i, (key, text))| {
                        chip(("riskmap-hazard", i), text, hazard_now == key).on_click(cx.listener(move |this, _, _, cx| set(this, cx, key.to_string(), day_now)))
                    }).collect::<Vec<_>>()))
                    .child(row().flex_wrap().gap(px(4.)).children([(1u8, "Day 1"), (2, "Day 2"), (3, "Day 3")].into_iter().map(|(d, text)| {
                        let hz = hazard_now.clone();
                        chip(("riskmap-day", d as usize), text, day_now == d).on_click(cx.listener(move |this, _, _, cx| set(this, cx, hz.clone(), d)))
                    }).collect::<Vec<_>>()));
            }
            LayerKind::RiskMeter { style, labels, .. } => {
                let (style_now, labels_now) = (style.clone(), *labels);
                kind_section = kind_section
                    .child(muted("Level (0-5): type a number, a risk word or a binding like {{spc.home.tornado.level}} in Content"))
                    .child(row().flex_wrap().gap(px(4.)).children([("bar", "Bar"), ("blocks", "Blocks"), ("pill", "Five steps")].into_iter().enumerate().map(|(i, (key, text))| {
                        chip(("meter-style", i), text, style_now == key || (style_now.is_empty() && key == "bar")).on_click(cx.listener(move |this, _, _, cx| {
                            this.edit_layer(cx, true, |l| {
                                if let LayerKind::RiskMeter { style, .. } = &mut l.kind {
                                    *style = key.to_string();
                                }
                            });
                        }))
                    }).collect::<Vec<_>>()))
                    .child(toggle("meter-labels", "Scale words inside (MRGL … HIGH)", labels_now).on_click(cx.listener(|this, _, _, cx| {
                        this.edit_layer(cx, true, |l| {
                            if let LayerKind::RiskMeter { labels, .. } = &mut l.kind {
                                *labels = !*labels;
                            }
                        });
                    })));
            }
            LayerKind::Rect { fill, radius, stroke, skew, sheen } => {
                let (radius, gradient, has_stroke, skew_now, sheen_now) = (*radius, matches!(fill, Fill::Linear { .. }), stroke.is_some(), *skew, *sheen);
                kind_section = kind_section
                    .child(row().justify_between().child(muted("Corner radius")).child(muted(format!("{radius:.0}"))))
                    .child(slider("radius", radius, 0.0, 120.0, cx, |this, v, _, cx| {
                        this.edit_layer(cx, false, |l| {
                            if let LayerKind::Rect { radius, .. } = &mut l.kind {
                                *radius = v.round();
                            }
                        });
                    }))
                    .child(row().justify_between().child(muted("Slant (angled plate)")).child(muted(format!("{skew_now:.2}"))))
                    .child(slider("skew", skew_now, -0.6, 0.6, cx, |this, v, _, cx| {
                        this.edit_layer(cx, false, |l| {
                            if let LayerKind::Rect { skew, .. } = &mut l.kind {
                                *skew = (v * 20.0).round() / 20.0;
                            }
                        });
                    }))
                    .child(toggle("sheen", "Glossy highlight", sheen_now).on_click(cx.listener(|this, _, _, cx| {
                        this.edit_layer(cx, true, |l| {
                            if let LayerKind::Rect { sheen, .. } = &mut l.kind {
                                *sheen = !*sheen;
                            }
                        });
                    })))
                    .child(toggle("gradient", "Gradient fill", gradient).on_click(cx.listener(|this, _, _, cx| {
                        this.edit_layer(cx, true, |l| {
                            if let LayerKind::Rect { fill, .. } = &mut l.kind {
                                *fill = match fill.clone() {
                                    Fill::Solid { color } => Fill::linear(&color, "$panel", 0.0),
                                    Fill::Linear { from, .. } => Fill::solid(&from),
                                };
                            }
                        });
                    })))
                    .child(toggle("stroke", "Outline", has_stroke).on_click(cx.listener(|this, _, _, cx| {
                        this.edit_layer(cx, true, |l| {
                            if let LayerKind::Rect { stroke, .. } = &mut l.kind {
                                *stroke = if stroke.is_some() {
                                    None
                                } else {
                                    Some(StrokeStyle { color: "$primary".into(), width: 4.0 })
                                };
                            }
                        });
                    })));
            }
            LayerKind::Icon { condition, daypart } => {
                let options: Vec<String> = ["{{condition}}", "{{day0.condition}}", "{{day1.condition}}", "{{day2.condition}}"]
                    .iter()
                    .map(|s| s.to_string())
                    .chain(std::iter::once("No icon".to_string()))
                    .chain(IconKind::ALL.iter().map(|k| k.label().to_string()))
                    .collect();
                let chips = options
                    .into_iter()
                    .enumerate()
                    .map(|(i, option)| {
                        let active = *condition == option;
                        let title = match option.as_str() {
                            "{{condition}}" => "Live conditions",
                            "{{day0.condition}}" => "Today's forecast",
                            "{{day1.condition}}" => "Tomorrow's forecast",
                            _ => &option,
                        }.to_string();
                        chip(("cond", i), title, active).on_click(cx.listener(move |this, _, _, cx| {
                            let option = option.clone();
                            this.edit_layer(cx, true, |l| {
                                if let LayerKind::Icon { condition, .. } = &mut l.kind {
                                    *condition = option;
                                }
                            });
                        }))
                    })
                    .collect::<Vec<_>>();
                let dayparts = ["{{daypart}}", "day", "night"]
                    .into_iter()
                    .enumerate()
                    .map(|(i, p)| {
                        chip(("daypart", i), p, daypart == p).on_click(cx.listener(move |this, _, _, cx| {
                            this.edit_layer(cx, true, |l| {
                                if let LayerKind::Icon { daypart, .. } = &mut l.kind {
                                    *daypart = p.to_string();
                                }
                            });
                        }))
                    })
                    .collect::<Vec<_>>();
                kind_section = kind_section
                    .child(muted("Condition"))
                    .child(row().flex_wrap().gap(px(4.)).children(chips))
                    .child(muted("Day / night"))
                    .child(row().gap(px(4.)).children(dayparts));
            }
            LayerKind::Image { media: current, fit } => {
                // Thumbnails (CWXB-44): pick an image by sight, name underneath.
                let chips = media
                    .iter()
                    .enumerate()
                    .map(|(i, (name, path))| {
                        let name = name.clone();
                        let on = *current == name;
                        div()
                            .id(("media", i))
                            .w(px(92.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .p(px(3.))
                            .rounded(px(5.))
                            .cursor_pointer()
                            .border_1()
                            .when(on, |d| d.border_color(c(CYAN)).bg(crate::ui::ca(CYAN, 0.12)))
                            .when(!on, |d| d.border_color(c(BORDER)).hover(|s| s.bg(c(PANEL_2))))
                            .child(div().w_full().h(px(52.)).rounded(px(3.)).overflow_hidden().bg(c(BG)).child(img(path.as_path()).size_full().object_fit(ObjectFit::Cover)))
                            .child(div().w_full().truncate().text_size(px(10.)).text_color(c(if on { TEXT } else { MUTED })).child(name.clone()))
                            .on_click(cx.listener(move |this, _, window, cx| {
                            let name = name.clone();
                            this.edit_layer(cx, true, |l| {
                                if let LayerKind::Image { media, .. } = &mut l.kind {
                                    *media = name;
                                }
                            });
                            this.load_inputs(window, cx);
                        }))
                    })
                    .collect::<Vec<_>>();
                let fits = [(ImageFit::Contain, "Contain"), (ImageFit::Cover, "Cover"), (ImageFit::Stretch, "Stretch")]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (f, text))| {
                        chip(("fit", i), text, *fit == f).on_click(cx.listener(move |this, _, _, cx| {
                            this.edit_layer(cx, true, |l| {
                                if let LayerKind::Image { fit, .. } = &mut l.kind {
                                    *fit = f;
                                }
                            });
                        }))
                    })
                    .collect::<Vec<_>>();
                kind_section = kind_section
                    .child(row().gap(px(4.))
                        .child(button_small("import-layer-image", "Import image…").on_click(cx.listener(|_this, _, window, cx| {
                            crate::ui::pick_off_thread_in(window, cx, || rfd::FileDialog::new().add_filter("Images", library::IMPORT_EXTS).pick_file(), |this, path, window, cx| {
                            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
                            if !library::IMPORT_EXTS.iter().any(|supported| ext.eq_ignore_ascii_case(supported)) {
                                this.app.update(cx, |s, cx| { s.toast(ToastKind::Error, "Choose a PNG, JPEG, WebP, GIF, BMP or SVG image"); cx.notify(); });
                                return;
                            }
                            match library::import_media(&[path]) {
                                Ok(names) => {
                                    if let Some(name) = names.into_iter().next() {
                                        this.app.update(cx, |s, cx| { s.media = library::list_media(); cx.notify(); });
                                        this.edit_layer(cx, true, |layer| {
                                            if let LayerKind::Image { media, .. } = &mut layer.kind { *media = name; }
                                        });
                                        this.load_inputs(window, cx);
                                    }
                                }
                                Err(error) => this.app.update(cx, |s, cx| { s.toast(ToastKind::Error, format!("Could not import image: {error}")); cx.notify(); }),
                            }
                            });
                        })))
                        .child(chip("use-brand-logo", "OpenWX logo", crate::gfx::bind::is_brand_image(current)).on_click(cx.listener(|this, _, window, cx| {
                            this.edit_layer(cx, true, |layer| {
                                if let LayerKind::Image { media, .. } = &mut layer.kind { *media = "{{brand}}".into(); }
                            });
                            this.load_inputs(window, cx);
                        }))))
                    .child(muted("Image source • use {brand} for the OpenWX logo"))
                    .child(Input::new(&self.content))
                    .child(muted("Choose from Media"))
                    .when(media.is_empty(), |d| d.child(muted("Your Media library has no images yet.")))
                    .child(div().id("media-thumbs").max_h(px(260.)).overflow_y_scroll().child(row().flex_wrap().gap(px(6.)).children(chips)))
                    .child(row().gap(px(4.)).children(fits));
            }
            LayerKind::Line { width, arrow, .. } => {
                let (width, arrow) = (*width, *arrow);
                kind_section = kind_section
                    .child(slider("line-width", width, 1.0, 40.0, cx, |this, v, _, cx| {
                        this.edit_layer(cx, false, |l| {
                            if let LayerKind::Line { width, .. } = &mut l.kind {
                                *width = v.round();
                            }
                        });
                    }))
                    .child(toggle("arrow", "Arrow head", arrow).on_click(cx.listener(|this, _, _, cx| {
                        this.edit_layer(cx, true, |l| {
                            if let LayerKind::Line { arrow, .. } = &mut l.kind {
                                *arrow = !*arrow;
                            }
                        });
                    })));
            }
            LayerKind::Legend { palette, .. } => {
                let chips = std::iter::once(("{{radar.palette}}", "Map palette"))
                    .chain(RADAR_PALETTES.iter().copied())
                    .enumerate()
                    .map(|(i, (code, name))| {
                        let code = code.to_string();
                        chip(("legend-pal", i), name, *palette == code).on_click(cx.listener(move |this, _, _, cx| {
                            let code = code.clone();
                            this.edit_layer(cx, true, |l| {
                                if let LayerKind::Legend { palette, .. } = &mut l.kind {
                                    *palette = code;
                                }
                            });
                        }))
                    })
                    .collect::<Vec<_>>();
                kind_section = kind_section.child(row().flex_wrap().gap(px(4.)).children(chips));
            }
            LayerKind::Ticker { speed, size, .. } => {
                let (speed, size) = (*speed, *size);
                kind_section = kind_section
                    .child(Input::new(&self.content))
                    .child(row().justify_between().child(muted("Speed")).child(muted(format!("{speed:.0} px/s"))))
                    .child(slider("crawl-speed", speed, 30.0, 400.0, cx, |this, v, _, cx| {
                        this.edit_layer(cx, false, |l| {
                            if let LayerKind::Ticker { speed, .. } = &mut l.kind {
                                *speed = v.round();
                            }
                        });
                    }))
                    .child(slider("crawl-size", size, 16.0, 120.0, cx, |this, v, _, cx| {
                        this.edit_layer(cx, false, |l| {
                            if let LayerKind::Ticker { size, .. } = &mut l.kind {
                                *size = v.round();
                            }
                        });
                    }));
            }
            LayerKind::Chart { chart: kind, series, series2, show_values, unit, highlight, .. } => {
                let (kind, series, series2, show_values, unit) = (*kind, series.clone(), series2.clone(), *show_values, unit.clone());
                let highlight_now = highlight.clone();
                let chart_anim = !layer.reveal.is_empty();
                let kinds = ChartKind::ALL
                    .iter()
                    .enumerate()
                    .map(|(i, k)| {
                        let k = *k;
                        chip(("chart-kind", i), k.label(), kind == k).on_click(cx.listener(move |this, _, _, cx| {
                            this.edit_layer(cx, true, |l| {
                                if let LayerKind::Chart { chart, .. } = &mut l.kind {
                                    *chart = k;
                                }
                            });
                        }))
                    })
                    .collect::<Vec<_>>();
                let series_chips = |which: u8, current: String, cx: &mut Context<Self>| {
                    std::iter::once(("", "None"))
                        .filter(|_| which == 2)
                        .chain(SERIES.iter().copied())
                        .enumerate()
                        .map(|(i, (key, desc))| {
                            let key_s = key.to_string();
                            chip(("chart-series", which as usize * 100 + i), if key.is_empty() { "None".to_string() } else { desc.to_string() }, current == key_s).on_click(cx.listener(move |this, _, _, cx| {
                                let key = key_s.clone();
                                this.edit_layer(cx, true, |l| {
                                    if let LayerKind::Chart { series, series2, unit, .. } = &mut l.kind {
                                        if which == 1 {
                                            *unit = match key.as_str() {
                                                k if k.contains("temp") || k.contains("feels") || k.starts_with("day.hi") || k.starts_with("day.lo") => "°".into(),
                                                k if k.contains("precip") || k.contains("cloud") => "%".into(),
                                                k if k.contains("wind") || k.contains("gust") => "".into(),
                                                k if k.starts_with("river") => " ft".into(),
                                                k if k.starts_with("daylight") => " h".into(),
                                                _ => String::new(),
                                            };
                                            *series = key;
                                        } else {
                                            *series2 = key;
                                        }
                                    }
                                });
                            }))
                        })
                        .collect::<Vec<_>>()
                };
                let first = series_chips(1, series.clone(), cx);
                let second = series_chips(2, series2.clone(), cx);
                kind_section = kind_section
                    .child(row().flex_wrap().gap(px(4.)).children(kinds))
                    .child(muted("Data"))
                    .child(row().flex_wrap().gap(px(4.)).children(first))
                    .child(muted(if kind == ChartKind::Gauge { "Gauges show the last value; for a live number type a binding like {{humidity}} into the data box" } else { "Second series (optional)" }))
                    .child(row().flex_wrap().gap(px(4.)).children(second))
                    .child(muted(format!("Shown: {series}{}{}", if series2.is_empty() { String::new() } else { format!(" + {series2}") }, if unit.is_empty() { String::new() } else { format!(" ({unit})") })))
                    .child(muted("Highlight a point (the note text box below the content input sets its label)"))
                    .child(row().flex_wrap().gap(px(4.)).children([("", "None"), ("max", "Highest"), ("min", "Lowest"), ("last", "Latest")].into_iter().enumerate().map(|(i, (key, text))| {
                        chip(("chart-hl", i), text, highlight_now == key).on_click(cx.listener(move |this, _, _, cx| {
                            this.edit_layer(cx, true, |l| {
                                if let LayerKind::Chart { highlight, .. } = &mut l.kind {
                                    *highlight = key.to_string();
                                }
                            });
                        }))
                    }).collect::<Vec<_>>()))
                    .child(button_small("chart-note", "Use the text box as the note").on_click(cx.listener(|this, _, _, cx| {
                        let v = this.content.read(cx).value().to_string();
                        this.edit_layer(cx, true, |l| {
                            if let LayerKind::Chart { note, .. } = &mut l.kind {
                                *note = v;
                            }
                        });
                    })))
                    .child(chip("chart-anim", "Draw in (animated)", chart_anim).on_click(cx.listener(|this, _, _, cx| {
                        this.edit_layer(cx, true, |l| {
                            if l.reveal.is_empty() {
                                l.reveal = "words".into();
                                l.reveal_secs = 2.0;
                            } else {
                                l.reveal.clear();
                            }
                        });
                        this.playing = Some(Instant::now());
                    })))
                    .child(toggle("chart-values", "Show values", show_values).on_click(cx.listener(|this, _, _, cx| {
                        this.edit_layer(cx, true, |l| {
                            if let LayerKind::Chart { show_values, .. } = &mut l.kind {
                                *show_values = !*show_values;
                            }
                        });
                    })));
            }
            LayerKind::Particles { effect, density, speed, .. } => {
                let (effect, density, speed) = (effect.clone(), *density, *speed);
                let effects = [("snow", "Snow"), ("rain", "Rain"), ("hail", "Hail"), ("sparkle", "Sparkle"), ("leaves", "Leaves")]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (key, text))| {
                        chip(("pt-effect", i), text, effect == key).on_click(cx.listener(move |this, _, _, cx| {
                            this.edit_layer(cx, true, |l| {
                                if let LayerKind::Particles { effect, .. } = &mut l.kind {
                                    *effect = key.to_string();
                                }
                            });
                        }))
                    })
                    .collect::<Vec<_>>();
                kind_section = kind_section
                    .child(row().flex_wrap().gap(px(4.)).children(effects))
                    .child(row().justify_between().child(muted("Amount")).child(muted(format!("{density:.1}"))))
                    .child(slider("pt-density", density, 0.2, 12.0, cx, |this, v, _, cx| {
                        this.edit_layer(cx, false, |l| {
                            if let LayerKind::Particles { density, .. } = &mut l.kind {
                                *density = (v * 10.0).round() / 10.0;
                            }
                        });
                    }))
                    .child(row().justify_between().child(muted("Speed")).child(muted(format!("{speed:.1}x"))))
                    .child(slider("pt-speed", speed, 0.2, 3.0, cx, |this, v, _, cx| {
                        this.edit_layer(cx, false, |l| {
                            if let LayerKind::Particles { speed, .. } = &mut l.kind {
                                *speed = (v * 10.0).round() / 10.0;
                            }
                        });
                    }));
            }
            LayerKind::Diagram { diagram } => {
                let current = diagram.clone();
                let chips = crate::gfx::diagrams::DIAGRAMS
                    .iter()
                    .enumerate()
                    .map(|(i, (key, label))| {
                        let key: &'static str = *key;
                        chip(("diagram", i), *label, current == key).on_click(cx.listener(move |this, _, _, cx| {
                            this.edit_layer(cx, true, |l| {
                                if let LayerKind::Diagram { diagram } = &mut l.kind {
                                    *diagram = key.to_string();
                                }
                            });
                            this.playing = Some(Instant::now());
                        }))
                    })
                    .collect::<Vec<_>>();
                kind_section = kind_section.child(row().flex_wrap().gap(px(3.)).children(chips));
            }
            LayerKind::Ellipse { .. } => {}
        }

        let anim_in = AnimKind::ALL
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let k = *k;
                chip(("anim-in", i), k.label(), layer.anim_in.kind == k).on_click(cx.listener(move |this, _, _, cx| {
                    this.edit_layer(cx, true, |l| l.anim_in.kind = k);
                    this.playing = Some(Instant::now());
                    this.request_render(cx);
                }))
            })
            .collect::<Vec<_>>();
        let anim_out = [AnimKind::Fade, AnimKind::SlideLeft, AnimKind::SlideRight, AnimKind::SlideDown, AnimKind::Wipe]
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let k = *k;
                chip(("anim-out", i), k.label(), layer.anim_out.kind == k).on_click(cx.listener(move |this, _, _, cx| {
                    this.edit_layer(cx, true, |l| l.anim_out.kind = k);
                }))
            })
            .collect::<Vec<_>>();

        let (visible, locked, rotation, opacity, delay, duration) = (
            layer.visible,
            layer.locked,
            layer.rotation,
            layer.opacity,
            layer.anim_in.delay,
            layer.anim_in.duration,
        );

        panel
            .child(
                section("Layer")
                    .child(Input::new(&self.layer_name))
                    .child(
                        row()
                            .gap(px(6.))
                            .child(button_small("up", "▲ Forward").on_click(cx.listener(|this, _, _, cx| this.move_layer(1, cx))))
                            .child(button_small("down", "▼ Back").on_click(cx.listener(|this, _, _, cx| this.move_layer(-1, cx))))
                            .child(button_small("dup", "Duplicate").on_click(cx.listener(|this, _, window, cx| this.duplicate_layer(window, cx))))
                            .child(button_small("del", "Delete").on_click(cx.listener(|this, _, window, cx| this.delete_layer(window, cx)))),
                    )
                    .child(toggle("visible", "Visible", visible).on_click(cx.listener(|this, _, _, cx| {
                        this.edit_layer(cx, true, |l| l.visible = !l.visible);
                    })))
                    .child(toggle("locked", "Locked", locked).on_click(cx.listener(|this, _, _, cx| {
                        this.edit_layer(cx, true, |l| l.locked = !l.locked);
                    }))),
            )
            .child(
                section("Position & size")
                    .child(
                        row()
                            .gap(px(6.))
                            .child(div().flex_1().child(Input::new(&self.pos[0])))
                            .child(div().flex_1().child(Input::new(&self.pos[1]))),
                    )
                    .child(
                        row()
                            .gap(px(6.))
                            .child(div().flex_1().child(Input::new(&self.pos[2])))
                            .child(div().flex_1().child(Input::new(&self.pos[3]))),
                    )
                    .child(row().justify_between().child(muted("Rotation")).child(muted(format!("{rotation:.0}°"))))
                    .child(slider("rotation", rotation, -180.0, 180.0, cx, |this, v, _, cx| {
                        this.edit_layer(cx, false, |l| l.rotation = v.round());
                    }))
                    .child(row().justify_between().child(muted("Opacity")).child(muted(format!("{:.0}%", opacity * 100.0))))
                    .child(slider("opacity", opacity, 0.0, 1.0, cx, |this, v, _, cx| {
                        this.edit_layer(cx, false, |l| l.opacity = v);
                    })),
            )
            .child(
                section("Color")
                    .child(row().flex_wrap().gap(px(5.)).children(color_swatches))
                    .child(muted("Palette from your brand colours"))
                    .child(row().flex_wrap().gap(px(5.)).children(palette_swatches))
                    .child(Input::new(&self.hex)),
            )
            .child(kind_section)
            .child(layout_section)
            .child(anim_section)
            .child(
                section("Animate in")
                    .child(row().flex_wrap().gap(px(4.)).children(anim_in))
                    .child(row().justify_between().child(muted("Delay")).child(muted(format!("{delay:.2}s"))))
                    .child(slider("anim-delay", delay, 0.0, 4.0, cx, |this, v, _, cx| {
                        this.edit_layer(cx, false, |l| l.anim_in.delay = (v * 20.0).round() / 20.0);
                    }))
                    .child(row().justify_between().child(muted("Duration")).child(muted(format!("{duration:.2}s"))))
                    .child(slider("anim-duration", duration, 0.1, 3.0, cx, |this, v, _, cx| {
                        this.edit_layer(cx, false, |l| l.anim_in.duration = (v * 20.0).round() / 20.0);
                    }))
                    .child(muted("Animate out"))
                    .child(row().flex_wrap().gap(px(4.)).children(anim_out)),
            )
            .child(self.layers_list(cx))
    }

    fn document_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let backgrounds: [(&str, Option<Fill>); 5] = [
            ("Transparent (overlay)", None),
            ("Brand gradient", Some(Fill::linear("#0C1626", "#173A5C", 115.0))),
            ("Brand dark", Some(Fill::solid("$panel"))),
            ("Alert red", Some(Fill::linear("#22070A", "#5C0E16", 115.0))),
            ("Black", Some(Fill::solid("#000000"))),
        ];
        let current = self.doc.background.clone();
        let bg_chips = backgrounds
            .into_iter()
            .enumerate()
            .map(|(i, (name, fill))| {
                let active = current == fill;
                chip(("bg", i), name, active).on_click(cx.listener(move |this, _, _, cx| {
                    this.snapshot();
                    this.doc.background = fill.clone();
                    this.changed(cx);
                }))
            })
            .collect::<Vec<_>>();
        let duration = self.doc.duration;
        let folders = {
            let mut f: Vec<String> = self.app.read(cx).graphics.iter().map(|g| g.folder.clone()).filter(|f| !f.is_empty()).collect();
            f.sort();
            f.dedup();
            f
        };
        let folder_chips = folders
            .into_iter()
            .chain(std::iter::once(String::new()))
            .enumerate()
            .map(|(i, f)| {
                let active = self.doc.folder == f;
                let label = if f.is_empty() { "No folder".to_string() } else { f.clone() };
                chip(("folder-chip", i), label, active).on_click(cx.listener(move |this, _, window, cx| {
                    this.doc.folder = f.clone();
                    Self::set_input(&this.folder_input.clone(), f.clone(), window, cx);
                    this.dirty = true;
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();
        // Location choices: home, watched spots, forecast regions.
        let place_options: Vec<(String, Option<crate::gfx::model::PlaceRef>)> = {
            let s = self.app.read(cx);
            std::iter::once(("Home".to_string(), None))
                .chain(s.settings.alert_zones.iter().map(|z| (z.name.clone(), Some(crate::gfx::model::PlaceRef { name: z.name.clone(), lat: z.lat, lon: z.lon }))))
                .chain(s.ext.edits.published.regions.iter().map(|r| {
                    let (lat, lon) = r.shape.center();
                    (r.name.clone(), Some(crate::gfx::model::PlaceRef { name: r.name.clone(), lat, lon }))
                }))
                .collect()
        };
        let place_chips = place_options
            .into_iter()
            .enumerate()
            .map(|(i, (label, place))| {
                let active = self.doc.place == place;
                chip(("place", i), label, active).on_click(cx.listener(move |this, _, _, cx| {
                    this.snapshot();
                    this.doc.place = place.clone();
                    this.changed(cx);
                }))
            })
            .collect::<Vec<_>>();
        // Readiness report for this graphic.
        let issues = {
            let state = self.app.read(cx);
            let bindings = crate::gfx::bind::Bindings::build(&state.data, &state.settings, None, &state.settings.radar_palette);
            let bindings = crate::gfx::check::preview_bindings(&bindings, self.preview_data);
            crate::gfx::check::check_doc(&self.doc, &bindings, &state.settings.brand, &crate::config::LibraryPaths::get().media)
        };
        let deps = crate::collections::dependencies(&self.doc);
        let compat = {
            let state = self.app.read(cx);
            let bindings = crate::gfx::bind::Bindings::build(&state.data, &state.settings, None, &state.settings.radar_palette);
            crate::collections::validate(&self.doc, &bindings)
        };
        let base_name = self.doc.inherits.as_ref().and_then(|id| self.app.read(cx).graphics.iter().find(|g| &g.id == id).map(|g| g.name.clone()));
        let dependents = self.doc.dependents();
        let station_templates = crate::gfx::model::GraphicDoc::templates();
        let overrides = self.doc.data_overrides.clone();
        let rules = self.doc.rules.clone();
        let template_section = section("Station templates")
            .child(muted(match &base_name {
                Some(n) => format!("Based on the template \"{n}\": its layers come first; a layer with the same name here overrides it."),
                None => "Not based on a template.".to_string(),
            }))
            .when(!dependents.is_empty(), |d| d.child(muted(format!("Used as the base of: {}", dependents.join(", ")))))
            .child(
                row()
                    .flex_wrap()
                    .gap(px(4.))
                    .child(button_small("tpl-publish", if self.doc.published { "Published ✓ (unpublish)" } else { "Publish as template" }).on_click(cx.listener(|this, _, _, cx| {
                        if !this.doc.published && this.doc.approved_by.is_empty() {
                            this.app.update(cx, |s, cx| {
                                s.toast(ToastKind::Info, "Approve the graphic first, then publish it as a template");
                                cx.notify();
                            });
                            return;
                        }
                        this.doc.published = !this.doc.published;
                        this.changed(cx);
                    })))
                    .when(base_name.is_some(), |d| {
                        d.child(button_small("tpl-detach", "Detach from template").on_click(cx.listener(|this, _, _, cx| {
                            this.doc = this.doc.resolved();
                            this.doc.inherits = None;
                            this.changed(cx);
                        })))
                    }),
            )
            .when(!station_templates.is_empty(), |d| {
                d.child(muted("New graphic based on a station template"))
                    .child(row().flex_wrap().gap(px(4.)).children(station_templates.into_iter().enumerate().map(|(i, (id, name))| {
                        button_small(("tpl-new", i), name.clone()).on_click(cx.listener(move |this, _, _, cx| {
                            let mut doc = crate::gfx::model::GraphicDoc::new(&format!("{name} copy"), "Graphics", None);
                            doc.inherits = Some(id.clone());
                            this.preview_template(doc, cx);
                        }))
                    }).collect::<Vec<_>>()))
            })
            .child(muted("Data sources and rules: \"binding = value\" points a binding elsewhere for this graphic; \"Layer: required | max_chars:N | fits\" adds a rule"))
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.template_input))).child(button_small("tpl-add", "Add").on_click(cx.listener(|this, _, window, cx| {
                let v = this.template_input.read(cx).value().trim().to_string();
                if let Some((k, val)) = v.split_once('=') {
                    this.doc.data_overrides.push((k.trim().to_string(), val.trim().to_string()));
                } else if let Some((layer, rule)) = v.split_once(':') {
                    this.doc.rules.push((layer.trim().to_string(), rule.trim().to_string()));
                } else {
                    return;
                }
                this.template_input.update(cx, |s, cx| s.set_value(String::new(), window, cx));
                this.changed(cx);
            }))))
            .children(overrides.iter().enumerate().map(|(i, (k, v))| {
                row().gap(px(6.)).child(div().flex_1().text_size(px(12.)).text_color(c(TEXT)).child(format!("{{{{{k}}}}} = {v}"))).child(button_small(("tpl-ovr-del", i), "✕").on_click(cx.listener(move |this, _, _, cx| {
                    if i < this.doc.data_overrides.len() {
                        this.doc.data_overrides.remove(i);
                    }
                    this.changed(cx);
                })))
            }).collect::<Vec<_>>())
            .children(rules.iter().enumerate().map(|(i, (l, r))| {
                row().gap(px(6.)).child(div().flex_1().text_size(px(12.)).text_color(c(TEXT)).child(format!("{l}: {r}"))).child(button_small(("tpl-rule-del", i), "✕").on_click(cx.listener(move |this, _, _, cx| {
                    if i < this.doc.rules.len() {
                        this.doc.rules.remove(i);
                    }
                    this.changed(cx);
                })))
            }).collect::<Vec<_>>());
        let test_rows = self
            .test_results
            .iter()
            .map(|t| {
                row()
                    .gap(px(6.))
                    .child(div().size(px(8.)).rounded(px(4.)).bg(c(if t.ok { 0x3DD68C } else { RED })))
                    .child(div().w(px(150.)).text_size(px(12.)).text_color(c(TEXT)).child(t.name))
                    .child(div().flex_1().min_w_0().text_size(px(12.)).text_color(c(MUTED)).child(t.detail.clone()))
            })
            .collect::<Vec<_>>();
        let test_section = section("Tests")
            .child(
                row()
                    .flex_wrap()
                    .gap(px(4.))
                    .child(button_small("run-tests", "Test this graphic").on_click(cx.listener(|this, _, _, cx| {
                        let doc = this.doc.clone();
                        let state = this.app.read(cx);
                        let (settings, data) = (state.settings.clone(), state.data.clone());
                        let task = cx.background_executor().spawn(async move { crate::gfx::testing::run(&doc, &settings, &data) });
                        cx.spawn(async move |this, cx| {
                            let results = task.await;
                            let _ = this.update(cx, |this, cx| {
                                this.test_results = results;
                                cx.notify();
                            });
                        })
                        .detach();
                    })))
                    .child(button_small("save-baseline", "Save baseline").on_click(cx.listener(|this, _, _, cx| {
                        let state = this.app.read(cx);
                        let result = crate::gfx::testing::save_baseline(&this.doc, &state.settings, &state.data);
                        this.app.update(cx, |s, cx| {
                            match result {
                                Ok(()) => s.toast(ToastKind::Success, "Baseline saved: later tests compare against it"),
                                Err(e) => s.toast(ToastKind::Error, format!("Could not save the baseline: {e}")),
                            }
                            cx.notify();
                        });
                    })))
                    .child(button_small("make-variants", "Make size variants").on_click(cx.listener(|this, _, _, cx| {
                        let base = this.doc.clone();
                        let made = this.app.update(cx, |s, cx| {
                            let mut n: usize = 0;
                            for (w, h, label) in [(1080u32, 1920u32, "Vertical"), (1080, 1080, "Square"), (1080, 1350, "Portrait 4:5")] {
                                let mut v = base.laid_out(w, h);
                                v.id = crate::gfx::model::new_id();
                                v.name = format!("{} ({label})", base.name);
                                v.folder = format!("{} variants", base.name);
                                v.approved_by.clear();
                                v.published = false;
                                if s.save_graphic(v).is_some() {
                                    n += 1;
                                }
                            }
                            cx.notify();
                            n
                        });
                        this.app.update(cx, |s, cx| {
                            s.toast(ToastKind::Success, format!("Made {made} variants in the \"{} variants\" folder", base.name));
                            cx.notify();
                        });
                    }))),
            )
            .children(test_rows);
        let deps_section = section("Dependencies and compatibility")
            .child(muted(format!(
                "Media: {} • Fonts: {} • Bindings: {}{}",
                if deps.media.is_empty() { "none".to_string() } else { deps.media.join(", ") },
                if deps.fonts.is_empty() { "brand fonts".to_string() } else { deps.fonts.join(", ") },
                deps.bindings.len(),
                deps.template.as_ref().map(|t| format!(" • Template: {t}")).unwrap_or_default()
            )))
            .when(compat.is_empty(), |d| d.child(muted("Compatible with this version")))
            .children(compat.into_iter().map(|t| div().text_size(px(12.)).text_color(c(crate::ui::AMBER)).child(t)).collect::<Vec<_>>())
            .child(button_small("sync-shared", "Sync shared collection").on_click(cx.listener(|this, _, _, cx| {
                let folder = this.app.read(cx).settings.shared_graphics_folder.clone();
                let result = crate::collections::sync_shared(&folder);
                this.app.update(cx, |s, cx| {
                    match result {
                        Ok((pulled, pushed)) => {
                            s.graphics = crate::library::list_graphics();
                            s.toast(ToastKind::Success, format!("Shared collection synced: {pulled} updated here, {pushed} shared"));
                        }
                        Err(e) => s.toast(ToastKind::Error, format!("Sync: {e}")),
                    }
                    cx.notify();
                });
            })));
        let issue_rows = issues
            .iter()
            .enumerate()
            .map(|(i, issue)| {
                let id = issue.layer_id.clone();
                row()
                    .id(("issue", i))
                    .gap(px(6.))
                    .cursor_pointer()
                    .child(div().size(px(8.)).rounded(px(4.)).bg(c(if issue.serious { RED } else { crate::ui::AMBER })))
                    .child(div().flex_1().min_w_0().text_size(px(12.)).text_color(c(TEXT)).child(format!("{}: {}", issue.layer, issue.text)))
                    .on_click(cx.listener(move |this, _, window, cx| this.select(Some(id.clone()), window, cx)))
            })
            .collect::<Vec<_>>();
        let preset_rows = self
            .doc
            .presets
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let name = p.name.clone();
                let del = p.name.clone();
                row()
                    .gap(px(6.))
                    .child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(p.name.clone()))
                    .child(button_small(("preset-use", i), "Use").on_click(cx.listener(move |this, _, window, cx| {
                        this.snapshot();
                        this.doc.apply_preset(&name);
                        this.load_inputs(window, cx);
                        this.changed(cx);
                    })))
                    .child(button_small(("preset-del", i), "Remove").on_click(cx.listener(move |this, _, _, cx| {
                        this.doc.presets.retain(|p| p.name != del);
                        this.dirty = true;
                        cx.notify();
                    })))
            })
            .collect::<Vec<_>>();
        let versions = if self.versions_open { library::list_versions(&self.doc.id) } else { Vec::new() };
        let version_rows = versions
            .into_iter()
            .take(12)
            .enumerate()
            .map(|(i, (t, path))| {
                let when = chrono::DateTime::from_timestamp(t, 0)
                    .map(|d| d.with_timezone(&chrono::Local).format("%b %-d, %-I:%M:%S %p").to_string())
                    .unwrap_or_default();
                row()
                    .gap(px(6.))
                    .child(div().flex_1().text_size(px(12.)).text_color(c(TEXT)).child(when))
                    .child(button_small(("version", i), "Restore").on_click(cx.listener(move |this, _, window, cx| {
                        match library::load_version(&path) {
                            Ok(old) => {
                                this.snapshot();
                                let keep_id = this.doc.id.clone();
                                this.doc = old;
                                this.doc.id = keep_id;
                                this.load_inputs(window, cx);
                                this.changed(cx);
                                this.app.update(cx, |s, _| s.toast(ToastKind::Info, "Earlier version restored (Save to keep it, Undo to go back)"));
                            }
                            Err(e) => this.app.update(cx, |s, _| s.toast(ToastKind::Error, format!("Could not open that version: {e}"))),
                        }
                    })))
            })
            .collect::<Vec<_>>();
        section("Graphic")
            .child(Input::new(&self.doc_name))
            .child(muted("Folder"))
            .child(Input::new(&self.folder_input))
            .child(row().flex_wrap().gap(px(4.)).children(folder_chips))
            .child(muted("Tags"))
            .child(Input::new(&self.tags_input))
            .child(muted(match &self.doc.place {
                Some(p) => format!("Location: {} (model values follow it)", p.name),
                None => "Location: home".to_string(),
            }))
            .child(row().flex_wrap().gap(px(4.)).children(place_chips))
            .child(button_small("place-batch", "Make a copy for each watched spot").on_click(cx.listener(|this, _, _, cx| {
                let zones = this.app.read(cx).settings.alert_zones.clone();
                if zones.is_empty() {
                    this.app.update(cx, |s, _| s.toast(ToastKind::Info, "Add watched spots first (click the map, then Watch this spot)"));
                    return;
                }
                let base = this.doc.clone();
                this.app.update(cx, |s, cx| {
                    for z in &zones {
                        let mut copy = base.duplicate(&format!("{} - {}", base.name, z.name));
                        copy.place = Some(crate::gfx::model::PlaceRef { name: z.name.clone(), lat: z.lat, lon: z.lon });
                        copy.folder = format!("{} by town", base.name);
                        s.save_graphic(copy);
                    }
                    s.toast(ToastKind::Success, format!("Made {} local copies in the \"{} by town\" folder", zones.len(), base.name));
                    cx.notify();
                });
            })))
            .child(muted(if issues.is_empty() { "Check: ready for air".to_string() } else { format!("Check: {} thing(s) to fix", issues.len()) }))
            .children(issue_rows)
            .child(deps_section)
            .child(template_section)
            .child(test_section)
            .child(muted("Wording presets"))
            .children(preset_rows)
            .child(
                row()
                    .gap(px(6.))
                    .child(div().flex_1().child(Input::new(&self.preset_input)))
                    .child(button_small("preset-save", "Save wording").on_click(cx.listener(|this, _, window, cx| {
                        let name = this.preset_input.read(cx).value().trim().to_string();
                        let name = if name.is_empty() { format!("Preset {}", this.doc.presets.len() + 1) } else { name };
                        this.doc.save_preset(&name);
                        Self::set_input(&this.preset_input.clone(), String::new(), window, cx);
                        this.dirty = true;
                        cx.notify();
                    }))),
            )
            .child(
                row()
                    .gap(px(6.))
                    .child(button_small("versions", if self.versions_open { "Hide versions" } else { "Versions" }).on_click(cx.listener(|this, _, _, cx| {
                        this.versions_open = !this.versions_open;
                        cx.notify();
                    })))
                    .child(button_small("as-template", "Save as my template").on_click(cx.listener(|this, _, _, cx| {
                        let mut copy = this.doc.duplicate(&this.doc.name);
                        copy.folder = "My templates".into();
                        copy.category = "My templates".into();
                        this.app.update(cx, |s, cx| {
                            s.save_graphic(copy);
                            s.toast(ToastKind::Success, "Saved to My templates");
                            cx.notify();
                        });
                    }))),
            )
            .children(version_rows)
            .child(muted("Background"))
            .child(row().flex_wrap().gap(px(4.)).children(bg_chips))
            .child(row().justify_between().child(muted("Show duration")).child(muted(if duration <= 0.0 { "Hold".to_string() } else { format!("{duration:.0}s") })))
            .child(slider("doc-duration", duration, 0.0, 60.0, cx, |this, v, _, cx| {
                this.doc.duration = v.round();
                this.dirty = true;
                cx.notify();
            }))
            .child(muted("Select a layer on the canvas or in the list to edit it. Drag to move, drag the corner handle to resize, arrows to nudge, Delete to remove, Ctrl+Z to undo."))
            .child(
                row()
                    .gap(px(6.))
                    .child(button_small("dup-doc", "Duplicate graphic").on_click(cx.listener(|this, _, window, cx| {
                        let copy = this.doc.duplicate(&format!("{} copy", this.doc.name));
                        let saved = this.app.update(cx, |s, cx| {
                            let saved = s.save_graphic(copy);
                            cx.notify();
                            saved
                        });
                        if let Some(doc) = saved {
                            this.open(doc, window, cx);
                        }
                    })))
                    .child(button_small("del-doc", "Delete graphic").on_click(cx.listener(|this, _, window, cx| {
                        let id = this.doc.id.clone();
                        let next = this.app.update(cx, |s, cx| {
                            s.delete_graphic(&id);
                            s.toast(ToastKind::Info, "Graphic moved to the library trash");
                            cx.notify();
                            s.graphics.first().cloned()
                        });
                        this.dirty = false;
                        let doc = next.unwrap_or_else(|| templates::blank(false));
                        this.open(doc, window, cx);
                    }))),
            )
    }

    fn layers_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected.clone();
        let items = self
            .doc
            .layers
            .iter()
            .rev()
            .enumerate()
            .map(|(i, l)| {
                let id = l.id.clone();
                let active = selected.as_deref() == Some(l.id.as_str());
                row()
                    .id(("layer", i))
                    .gap(px(6.))
                    .px(px(8.))
                    .py(px(5.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .bg(if active { c(0x163A47) } else { c(PANEL_2) })
                    .hover(|s| s.bg(c(RAISED)))
                    .child(div().text_size(px(10.)).text_color(c(MUTED)).w(px(64.)).child(l.kind.type_label()))
                    .child(div().flex_1().text_size(px(12.)).text_color(if l.visible { c(TEXT) } else { c(MUTED) }).truncate().child(l.name.clone()))
                    .when(l.locked, |d| d.child(badge("LOCK", 0x7A8599)))
                    .on_click(cx.listener(move |this, _, window, cx| this.select(Some(id.clone()), window, cx)))
            })
            .collect::<Vec<_>>();
        section(format!("Layers ({})", self.doc.layers.len())).gap(px(4.)).children(items)
    }
}

/// Plain tooltip view.
pub(crate) struct TooltipText(pub(crate) String);

impl Render for TooltipText {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(4.))
            .bg(c(0x0A0D14))
            .border_1()
            .border_color(c(BORDER))
            .text_size(px(12.))
            .text_color(c(TEXT))
            .child(self.0.clone())
    }
}

impl Render for GraphicsStudio {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.home {
            let landing = self.landing(cx);
            let menu = self.card_menu_popup(cx);
            return div()
                .size_full()
                .bg(c(BG))
                .track_focus(&self.focus)
                .child(landing)
                .children(menu)
                .into_any_element();
        }
        let preview = self.preview.clone();
        let slot = self.slot.clone();
        let browsing = self.template_preview.is_some();
        let selection = self
            .selected
            .as_ref()
            .and_then(|id| self.doc.layer(id))
            .filter(|_| !browsing)
            .map(|l| (l.x, l.y, l.w, l.h));
        // Baseline and alignment guides for the selected text layer.
        let text_guides: Vec<(bool, f32)> = self
            .selected
            .as_ref()
            .and_then(|id| self.doc.layer(id))
            .filter(|_| !browsing)
            .and_then(|l| match &l.kind {
                LayerKind::Text { text, font, size, weight, align, vcenter, wrap, .. } => {
                    let brand = self.app.read(cx).settings.brand.clone();
                    let style = crate::gfx::text::TextStyle {
                        family: crate::gfx::render::resolve_font(font, &brand),
                        weight: *weight,
                        size: *size,
                        color: tiny_skia::Color::WHITE,
                        letter_spacing: 0.0,
                    };
                    let mut g: Vec<(bool, f32)> = crate::gfx::text::baselines(text, (l.x, l.y, l.w, l.h), &style, *vcenter, *wrap).into_iter().map(|b| (false, b)).collect();
                    g.push((true, match align {
                        crate::gfx::text::Align::Left => l.x,
                        crate::gfx::text::Align::Center => l.x + l.w / 2.0,
                        crate::gfx::text::Align::Right => l.x + l.w,
                    }));
                    Some(g)
                }
                _ => None,
            })
            .unwrap_or_default();
        let doc_label = format!(
            "{}{} • {} layers",
            self.doc.name,
            if self.dirty { " •" } else { "" },
            self.doc.layers.len()
        );
        let displayed = self.template_preview.as_ref().unwrap_or(&self.doc);
        let doc_label = if browsing { displayed.name.clone() } else { doc_label };
        let overlay = displayed.is_overlay();
        let (doc_w, doc_h) = (displayed.width as f32, displayed.height as f32);
        let show_grid = self.show_grid && !browsing;
        let guides = if browsing { Vec::new() } else { self.guides.clone() };

        let library = self.library_panel(cx);
        let toolbar = if browsing {
            row().px(px(12.)).py(px(6.)).gap(px(8.))
                .child(button_small("add-template-workspace", "Add to Workspace").on_click(cx.listener(|this, _, window, cx| {
                    if let Some(template) = this.template_preview.clone() {
                        let saved = this.app.update(cx, |s, cx| {
                            let saved = s.save_graphic(template);
                            cx.notify();
                            saved
                        });
                        if let Some(doc) = saved { this.open(doc, window, cx); }
                    }
                })))
                .child(button_danger("template-take", "TAKE TO AIR").h(px(24.)).on_click(cx.listener(|this, _, _, cx| {
                    // Straight from the template: nothing is added to the workspace.
                    if let Some(doc) = this.template_preview.clone() {
                        this.app.update(cx, |s, cx| {
                            s.take_graphic(doc, Transition::Fade);
                            cx.notify();
                        });
                    }
                })))
                .child(button_small("template-to-show", "Add to show").on_click(cx.listener(|this, _, _, cx| {
                    if let Some(doc) = this.template_preview.clone() {
                        this.app.update(cx, |s, cx| {
                            s.show.scenes.push(Scene::graphic(&doc));
                            s.toast(ToastKind::Success, format!("Added \"{}\" to {}", doc.name, s.show.name));
                            cx.notify();
                        });
                    }
                })))
                .child(button_small("close-template-preview", "Back to all graphics").on_click(cx.listener(|this, _, _, cx| {
                    this.template_preview = None;
                    this.preview = None;
                    this.home = true;
                    this.request_render(cx);
                    cx.notify();
                })))
                .child(button_small("template-classic", "Switch Modern / Classic").on_click(cx.listener(|this, _, _, cx| {
                    if let Some(doc) = this.template_preview.as_mut() {
                        let next = if crate::gfx::look::of(doc) == "modern" { "classic" } else { "modern" };
                        crate::gfx::look::apply(doc, next);
                        this.request_render(cx);
                        cx.notify();
                    }
                })))
                .into_any_element()
        } else {
            self.toolbar(cx).into_any_element()
        };
        let inspector = (!browsing).then(|| self.inspector(cx).into_any_element());

        let paint_slot = slot.clone();
        let viewport = canvas(
            move |bounds, _, _| bounds,
            move |_, bounds, window, _| {
                window.paint_quad(fill(bounds, rgb(0x05070B)));
                let rect = fit_16_9(bounds, doc_w / doc_h.max(1.0));
                paint_slot.set_rect(rect);
                match &preview {
                    Some(image) => {
                        let _ = window.paint_image(rect, rect, Corners::default(), image.clone(), 0, false);
                        paint_slot.track(image.clone(), window);
                    }
                    None => window.paint_quad(fill(rect, rgb(0x0E1320))),
                }
                window.paint_quad(outline(rect, rgb(0x273046), BorderStyle::default()));
                let s = f32::from(rect.size.width) / doc_w.max(1.0);
                if show_grid {
                    // 60 px layout grid and the title-safe frame.
                    let mut gx = 60.0;
                    while gx < doc_w {
                        window.paint_quad(fill(Bounds::new(point(rect.origin.x + px(gx * s), rect.origin.y), size(px(1.), rect.size.height)), rgba(0xFFFFFF14)));
                        gx += 60.0;
                    }
                    let mut gy = 60.0;
                    while gy < doc_h {
                        window.paint_quad(fill(Bounds::new(point(rect.origin.x, rect.origin.y + px(gy * s)), size(rect.size.width, px(1.))), rgba(0xFFFFFF14)));
                        gy += 60.0;
                    }
                    let safe = Bounds::new(
                        point(rect.origin.x + px(doc_w * 0.05 * s), rect.origin.y + px(doc_h * 0.05 * s)),
                        size(px(doc_w * 0.9 * s), px(doc_h * 0.9 * s)),
                    );
                    window.paint_quad(outline(safe, rgba(0xFFD60A90), BorderStyle::default()));
                }
                for (vertical, pos) in &guides {
                    let b = if *vertical {
                        Bounds::new(point(rect.origin.x + px(pos * s), rect.origin.y), size(px(1.), rect.size.height))
                    } else {
                        Bounds::new(point(rect.origin.x, rect.origin.y + px(pos * s)), size(rect.size.width, px(1.)))
                    };
                    window.paint_quad(fill(b, rgb(0xFF3EA5)));
                }
                for (vertical, pos) in &text_guides {
                    let b = if *vertical {
                        Bounds::new(point(rect.origin.x + px(pos * s), rect.origin.y), size(px(1.), rect.size.height))
                    } else {
                        Bounds::new(point(rect.origin.x, rect.origin.y + px(pos * s)), size(rect.size.width, px(1.)))
                    };
                    window.paint_quad(fill(b, rgba(0x5EC8FFAA)));
                }
                if let Some((x, y, w, h)) = selection {
                    let sel = Bounds::new(
                        point(rect.origin.x + px(x * s), rect.origin.y + px(y * s)),
                        size(px((w * s).max(2.0)), px((h * s).max(2.0))),
                    );
                    window.paint_quad(outline(sel, rgb(CYAN), BorderStyle::default()));
                    let handle = Bounds::new(
                        point(sel.origin.x + sel.size.width - px(6.), sel.origin.y + sel.size.height - px(6.)),
                        size(px(12.), px(12.)),
                    );
                    window.paint_quad(fill(handle, rgb(CYAN)));
                }
            },
        )
        .size_full();

        row()
            .size_full()
            .items_start()
            .bg(c(BG))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .child(library)
            .child(
                col()
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .child(toolbar)
                    .child(
                        row()
                            .px(px(12.))
                            .py(px(6.))
                            .gap(px(8.))
                            .child(label(doc_label))
                            .when(browsing, |d| d.child(badge("TEMPLATE PREVIEW", CYAN)))
                            .child(badge(if overlay { "OVERLAY" } else { "FULL SCREEN" }, if overlay { 0xB48CFF } else { CYAN }))
                            .when(self.playing.is_some(), |d| d.child(badge("PREVIEWING ANIMATION", RED))),
                    )
                    .child(
                        div()
                            .id("gfx-canvas")
                            .flex_1()
                            .w_full()
                            .min_h_0()
                            .p(px(12.))
                            .child(viewport)
                            .when(!browsing, |d| d.on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
                                .on_mouse_move(cx.listener(Self::on_move))
                                .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up))),
                    ),
            )
            .children(inspector)
            .into_any_element()
    }
}

/// Preview size for a graphic: 1280 px on the long side, the graphic's own shape.
fn preview_size(doc: &GraphicDoc) -> (u32, u32) {
    let (w, h) = (doc.width.max(1) as f32, doc.height.max(1) as f32);
    if w >= h {
        (PREVIEW_W, (PREVIEW_W as f32 * h / w).round() as u32)
    } else {
        ((PREVIEW_H as f32 * w / h).round() as u32, PREVIEW_H)
    }
}

/// Colours generated from the brand: tints and shades of the primary, its complement and
/// neighbours on the colour wheel, and the secondary's tint.
fn brand_palette(brand: &crate::config::Brand) -> Vec<String> {
    let base = parse_color(&brand.primary, brand).to_color_u8();
    let (r, g, b) = (base.red() as f32 / 255.0, base.green() as f32 / 255.0, base.blue() as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    let s = if d == 0.0 { 0.0 } else { d / (1.0 - (2.0 * l - 1.0).abs()) };
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let hsl = |h: f32, s: f32, l: f32| -> String {
        let h = h.rem_euclid(360.0);
        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = l - c / 2.0;
        let (r, g, b) = match h as u32 / 60 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let to = |v: f32| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{:02X}{:02X}{:02X}", to(r), to(g), to(b))
    };
    vec![
        hsl(h, s, (l + 0.25).min(0.92)),
        hsl(h, s, (l + 0.12).min(0.85)),
        hsl(h, s, l),
        hsl(h, s, (l - 0.15).max(0.08)),
        hsl(h, s, (l - 0.3).max(0.05)),
        hsl(h + 180.0, s, l),
        hsl(h + 30.0, s, l),
        hsl(h - 30.0, s, l),
        hsl(h, s * 0.25, 0.14),
    ]
}
