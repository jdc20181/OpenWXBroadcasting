//! Winter Weather Studio: make snow-total, ice and winter-storm-potential maps for any state.
//! Pick a model and window (the map fills from the model), edit the legend bands, paint
//! counties by hand, then send the graphic to Graphics Studio or straight to air.

use std::{collections::HashMap, rc::Rc, sync::Arc};

use gpui_kit::{
    component::input::{Input, InputEvent, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::AppState,
    grid::GridModel,
    show::Transition,
    ui::{
        button_primary, button_small, c, chip, col, frame_view::{frame_canvas, ImageSlot}, label, muted, row, section, swatch, toggle, BG,
        BORDER, MUTED, PANEL, TEXT,
    },
    winter::{self, Band, WinterGrid, WinterKind, WinterMap},
};

/// Colours offered for legend bands: the winter ramp plus ice pinks and a few accents.
const PALETTE: [&str; 14] = [
    "#D6ECFF", "#A9D3FF", "#5C9EF2", "#2E62CF", "#1E3FA6", "#4B3AA6", "#B05CC8", "#F6CCF6", "#F6B8D0", "#E5679A", "#C2185B", "#FFF4A3",
    "#F2D21E", "#8C97A8",
];

const HOURS: [u32; 6] = [12, 24, 48, 72, 120, 168];

/// Models offered side by side in "Compare models" (CWXB-83 item 3).
/// The models Winter Studio reads from NOAA itself (NAM, NBM and ECMWF came from Open-Meteo,
/// which is no longer used; they would only repeat GFS).
const COMPARE: [GridModel; 2] = [GridModel::Gfs, GridModel::Hrrr];

/// Ready-made zone names (the owner's reference: "Flakes possible", "Accumulation possible").
const ZONE_LABELS: [&str; 5] = ["Flakes possible", "Accumulation possible", "Heavy snow possible", "Icing possible", "Mixed precipitation"];
/// Zone colours: snow blues, ice pink, a mix purple, a highlight yellow.
const ZONE_COLORS: [&str; 5] = ["#9FD8FF", "#3F8FFF", "#E5679A", "#B05CC8", "#F2D21E"];

/// Graphics in Graphics Studio that go with a winter storm (Codex's graphic sets): the
/// alerts map, reported totals, cold maps and multi-panel timelines (CWXB-83 items 2, 6, 7, 10).
pub(crate) const RELATED: [(&str, &str); 8] = [
    ("Winter alerts map", "set_winter_alerts__state"),
    ("Snow totals reported (48 h)", "set_snow_observed__48h_state"),
    ("Snow leaderboard (48 h)", "set_snow_leaderboard__48h_state"),
    ("4-panel timeline (12 h steps)", "set_four_panel_timeline__12h_state"),
    ("Wind chill tomorrow", "set_wind_chill__tomorrow_state"),
    ("Morning lows tomorrow", "set_morning_lows__tomorrow_state"),
    ("Advisory vs warning criteria", "set_winter_criteria_compare"),
    ("Cold advisory vs warning", "set_cold_alert_compare"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Brush {
    Off,
    Band(usize),
    NoSnow,
    Erase,
    /// Clicks add corners to the zone being drawn.
    Zone,
}

/// One model in the side-by-side comparison.
struct CompareCell {
    model: GridModel,
    grid: Option<WinterGrid>,
    status: String,
    image: Option<Arc<RenderImage>>,
    slot: Rc<ImageSlot>,
}

/// Cache key: model, start, hours, kind, region (states + home area).
type GridKey = (GridModel, u32, u32, WinterKind, String);

fn grid_key(m: &WinterMap, model: GridModel) -> GridKey {
    (model, m.start, m.hours, m.kind, format!("{}|{:?}", m.state, m.focus))
}

pub struct WinterStudio {
    app: Entity<AppState>,
    map: WinterMap,
    slot: Rc<ImageSlot>,
    preview: Option<Arc<RenderImage>>,
    render_gen: u64,
    dirty: bool,
    title: Entity<InputState>,
    subtitle: Entity<InputState>,
    period: Entity<InputState>,
    state_input: Entity<InputState>,
    bands: Vec<(Entity<InputState>, Entity<InputState>)>,
    band_subs: Vec<Subscription>,
    /// Inputs need refilling from `map` (done in render, which has the window).
    reload_inputs: bool,
    filling: bool,
    brush: Brush,
    loading: bool,
    status: String,
    cache: HashMap<GridKey, WinterGrid>,
    /// Side-by-side model maps while comparing (None: the normal preview).
    compare: Option<Vec<CompareCell>>,
    zone_label: Entity<InputState>,
    zone_color: usize,
    /// The forecaster has checked the drawn zones (needed before they go into a graphic).
    zones_reviewed: bool,
    _subscriptions: Vec<Subscription>,
}

impl WinterStudio {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let map = winter::all().first().cloned().unwrap_or_else(|| {
            let m = WinterMap::new(WinterKind::Snow, &home_state(&app, cx));
            winter::save(&m);
            m
        });
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("Title (e.g. Final Call Snow Totals)"));
        let subtitle = cx.new(|cx| InputState::new(window, cx).placeholder("Subtitle (optional)"));
        let period = cx.new(|cx| InputState::new(window, cx).placeholder("Legend heading (e.g. Through Friday 11/13)"));
        let state_input = cx.new(|cx| InputState::new(window, cx).placeholder("States, e.g. IN or IN, KY, IL"));
        let zone_label = cx.new(|cx| InputState::new(window, cx).placeholder("Zone label"));
        zone_label.update(cx, |s, cx| s.set_value(ZONE_LABELS[0].to_string(), window, cx));
        let mut subs = vec![cx.observe(&app, |_, _, cx| cx.notify())];
        for (input, field) in [(&title, 0u8), (&subtitle, 1), (&period, 2), (&state_input, 3)] {
            subs.push(cx.subscribe_in(input, window, move |this, state, e: &InputEvent, _, cx| {
                if !matches!(e, InputEvent::Change) || this.filling {
                    return;
                }
                let v = state.read(cx).value().to_string();
                match field {
                    0 => this.map.title = v,
                    1 => this.map.subtitle = v,
                    2 => this.map.period = v,
                    _ => {
                        // One state or several ("IN, KY, IL"); ignored until it parses.
                        let Some(states) = winter::parse_states(&v) else { return };
                        if states == this.map.state {
                            return;
                        }
                        this.set_region(states, this.map.focus, cx);
                        return;
                    }
                }
                this.changed(cx);
            }));
        }
        let mut s = Self {
            app,
            map,
            slot: ImageSlot::new(),
            preview: None,
            render_gen: 0,
            dirty: true,
            title,
            subtitle,
            period,
            state_input,
            bands: Vec::new(),
            band_subs: Vec::new(),
            reload_inputs: true,
            filling: false,
            brush: Brush::Off,
            loading: false,
            status: String::new(),
            cache: HashMap::new(),
            compare: None,
            zone_label,
            zone_color: 0,
            zones_reviewed: false,
            _subscriptions: subs,
        };
        if s.map.grid.is_none() {
            s.load_model(cx);
        }
        s
    }

    /// Save the map and redraw the preview.
    fn changed(&mut self, cx: &mut Context<Self>) {
        winter::save(&self.map);
        self.dirty = true;
        cx.notify();
    }

    fn open(&mut self, map: WinterMap, cx: &mut Context<Self>) {
        self.finish_zone();
        self.map = map;
        self.brush = Brush::Off;
        self.compare = None;
        self.reload_inputs = true;
        self.status.clear();
        if self.map.grid.is_none() {
            self.load_model(cx);
        }
        self.changed(cx);
    }

    fn new_map(&mut self, kind: WinterKind, cx: &mut Context<Self>) {
        let mut m = WinterMap::new(kind, &self.map.state);
        m.model = self.map.model;
        m.focus = self.map.focus;
        // Keep the legend set up last time for this kind (CWXB-83 item 12).
        if let Some(bands) = winter::last_bands(kind) {
            m.bands = bands;
        }
        self.open(m, cx);
        self.load_model(cx);
    }

    /// Show other states or only the home area; painting and zones belong to the old area.
    fn set_region(&mut self, state: String, focus: Option<winter::Focus>, cx: &mut Context<Self>) {
        self.finish_zone();
        self.map.state = state;
        self.map.focus = focus;
        self.map.overrides.clear();
        self.map.grid = None;
        self.compare = None;
        self.reload_inputs = true;
        self.load_model(cx);
        self.changed(cx);
    }

    /// The home state plus every state with a county within 150 km of home.
    fn nearby_states(&self, cx: &App) -> Option<String> {
        let home = self.app.read(cx).settings.home;
        let all = crate::map::boundaries::counties_named()?;
        let mut states = vec![home_state(&self.app, cx)];
        for c in all.iter() {
            if !states.contains(&c.state) && winter::km_between(winter::county_centre(&c.rings), (home.lon, home.lat)) <= 150.0 {
                states.push(c.state.clone());
            }
        }
        Some(states.join(","))
    }

    // --- Hand-drawn zones (CWXB-83 item 8) ---------------------------------------------------

    fn start_zone(&mut self, cx: &mut Context<Self>) {
        self.finish_zone();
        // New zones need a fresh check before they go on air.
        self.zones_reviewed = false;
        let label = self.zone_label.read(cx).value().trim().to_string();
        self.map.zones.push(winter::Zone { label, color: ZONE_COLORS[self.zone_color].to_string(), points: Vec::new(), open: true });
        self.brush = Brush::Zone;
        self.changed(cx);
    }

    /// Close the zone being drawn (one with fewer than 3 corners is dropped).
    fn finish_zone(&mut self) {
        if let Some(z) = self.map.zones.last_mut().filter(|z| z.open) {
            z.open = false;
            if z.points.len() < 3 {
                self.map.zones.pop();
            }
            winter::save(&self.map);
            self.dirty = true;
        }
        if self.brush == Brush::Zone {
            self.brush = Brush::Off;
        }
    }

    // --- Model comparison (CWXB-83 item 3) ---------------------------------------------------

    /// Load the same map from each model and show them side by side.
    fn start_compare(&mut self, cx: &mut Context<Self>) {
        self.finish_zone();
        self.brush = Brush::Off;
        let now = chrono::Utc::now().timestamp();
        let mut cells = Vec::new();
        for model in COMPARE {
            let cached = self.cache.get(&grid_key(&self.map, model)).filter(|g| now - g.fetched_at < 1800).cloned();
            let status = if cached.is_some() { String::new() } else { "Loading…".into() };
            cells.push(CompareCell { model, grid: cached, status, image: None, slot: ImageSlot::new() });
        }
        for cell in cells.iter().filter(|c| c.grid.is_none()) {
            let key = grid_key(&self.map, cell.model);
            let (state, focus, kind, start, hours, model) = (self.map.state.clone(), self.map.focus, self.map.kind, self.map.start, self.map.hours, cell.model);
            cx.spawn(async move |this, cx| {
                let result = crate::net::spawn(winter::fetch(state, focus, kind, model, start, hours)).await;
                let _ = this.update(cx, |this, cx| {
                    let grid = match result {
                        Ok(Ok(g)) => Ok(g),
                        Ok(Err(e)) => Err(e.to_string()),
                        Err(e) => Err(e.to_string()),
                    };
                    if let Ok(g) = &grid {
                        this.cache.insert(key.clone(), g.clone());
                    }
                    if grid_key(&this.map, model) != key {
                        return;
                    }
                    if let Some(cell) = this.compare.as_mut().and_then(|cells| cells.iter_mut().find(|c| c.model == model)) {
                        match grid {
                            Ok(g) => {
                                cell.grid = Some(g);
                                cell.status.clear();
                            }
                            Err(e) => cell.status = format!("Not loaded: {e}"),
                        }
                    }
                    this.render_compare(cx);
                });
            })
            .detach();
        }
        self.compare = Some(cells);
        self.render_compare(cx);
    }

    /// Draw each loaded model's map (map only, the legend is shared) as a thumbnail.
    fn render_compare(&mut self, cx: &mut Context<Self>) {
        let Some(cells) = &self.compare else { return };
        let font = self.app.read(cx).settings.brand.font_display.clone();
        let jobs: Vec<(GridModel, WinterMap)> = cells
            .iter()
            .filter(|c| c.grid.is_some() && c.image.is_none())
            .map(|c| {
                let mut m = self.map.clone();
                m.grid = c.grid.clone();
                m.zones.clear();
                m.towns = m.towns.min(8);
                (c.model, m)
            })
            .collect();
        for (model, map) in jobs {
            let font = font.clone();
            let task = cx.background_executor().spawn(async move { thumbnail(&map, &font) });
            cx.spawn(async move |this, cx| {
                let image = task.await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(cell) = this.compare.as_mut().and_then(|cells| cells.iter_mut().find(|c| c.model == model)) {
                        cell.image = image;
                    }
                    cx.notify();
                });
            })
            .detach();
        }
        cx.notify();
    }

    /// Use one model's map (or the average of all loaded ones) and leave the comparison.
    fn adopt(&mut self, grid: WinterGrid, cx: &mut Context<Self>) {
        self.map.model = grid.model;
        self.map.grid = Some(grid);
        self.compare = None;
        self.status = format!("Using {}", self.map.grid.as_ref().map(|g| if g.note.is_empty() { g.model.label().to_string() } else { g.note.clone() }).unwrap_or_default());
        self.changed(cx);
    }

    /// Send the finished zones to a new Forecast Zones graphic in Graphics Studio.
    fn zones_to_graphic(&mut self, cx: &mut Context<Self>) {
        let start = chrono::Local::now() + chrono::Duration::hours(self.map.start as i64);
        let day = (start.date_naive() - chrono::Local::now().date_naive()).num_days();
        let bbox = winter::region_bbox(&self.map.state, self.map.focus);
        let Some((key, overrides)) = self.map.zones_graphic(self.zones_reviewed, day, bbox) else { return };
        self.app.update(cx, |s, cx| {
            match crate::gfx::templates::build(&key) {
                Some(mut doc) => {
                    doc.data_overrides.extend(overrides);
                    if let Some(saved) = s.save_graphic(doc) {
                        s.toast(crate::app::ToastKind::Success, format!("Created \"{}\" from your zones - it's open in Graphics", saved.name));
                        s.open_graphic(saved.id);
                    }
                }
                None => s.toast(crate::app::ToastKind::Error, "The Forecast Zones graphic is not in this version".to_string()),
            }
            cx.notify();
        });
    }

    /// Open one of the related winter graphics (Graphics Studio templates) as a new graphic.
    fn open_related(&mut self, key: &'static str, cx: &mut Context<Self>) {
        self.app.update(cx, |s, cx| {
            match crate::gfx::templates::build(key) {
                Some(doc) => {
                    if let Some(saved) = s.save_graphic(doc) {
                        s.toast(crate::app::ToastKind::Success, format!("Created \"{}\" - it's open in Graphics", saved.name));
                        s.open_graphic(saved.id);
                    }
                }
                None => s.toast(crate::app::ToastKind::Error, "That graphic is not in this version".to_string()),
            }
            cx.notify();
        });
    }

    /// Move the window's start (hours from now), retitle the legend and reload.
    fn set_start(&mut self, start: u32, cx: &mut Context<Self>) {
        if start == self.map.start {
            return;
        }
        self.map.start = start;
        self.map.period = winter::window_text(start, self.map.hours);
        self.reload_inputs = true;
        self.load_model(cx);
        self.changed(cx);
    }

    /// Quick start (CWXB-83): a new map with its window, subtitle and legend heading set.
    fn new_preset(&mut self, kind: WinterKind, hours: u32, subtitle: &str, cx: &mut Context<Self>) {
        let mut m = WinterMap::new(kind, &self.map.state);
        m.model = self.map.model;
        m.hours = hours;
        m.subtitle = subtitle.to_string();
        m.period = winter::window_text(0, hours);
        m.focus = self.map.focus;
        if let Some(bands) = winter::last_bands(kind) {
            m.bands = bands;
        }
        self.open(m, cx);
        self.load_model(cx);
    }

    /// Save the county list (county, value, band) as CSV in Exports and show it in its folder.
    fn export_county_csv(&mut self, cx: &mut Context<Self>) {
        let Some(counties) = crate::map::boundaries::counties_named() else {
            self.status = "County outlines are still downloading; try again in a moment.".into();
            cx.notify();
            return;
        };
        let centres: Vec<(String, String, String, (f64, f64))> = counties.iter().filter(|c| c.state == self.map.state).filter_map(|c| {
            let pts: Vec<&(f64, f64)> = c.rings.iter().flatten().collect();
            let n = pts.len() as f64;
            (n > 0.0).then(|| (c.name.clone(), c.state.clone(), c.fips.clone(), (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n)))
        }).collect();
        let previous = (!self.map.based_on.is_empty()).then(|| winter::get(&self.map.based_on)).flatten();
        let csv = self.map.county_csv(centres.iter().map(|(n, s, f, p)| (n.as_str(), s.as_str(), f.as_str(), *p)), previous.as_ref());
        let dir = crate::config::LibraryPaths::get().exports.clone();
        let safe: String = self.map.title.chars().map(|ch| if ch.is_alphanumeric() || ch == ' ' || ch == '-' { ch } else { '_' }).collect();
        let path = dir.join(format!("{} counties {}.csv", safe.trim(), chrono::Local::now().format("%Y-%m-%d %H%M")));
        let result = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, csv));
        self.app.update(cx, |s, cx| {
            match result {
                Ok(()) => s.toast(crate::app::ToastKind::Success, format!("Saved {}", path.display())),
                Err(e) => s.toast(crate::app::ToastKind::Error, format!("Could not save the county list: {e}")),
            }
            cx.notify();
        });
    }

    /// Fill the map from the selected model and window (cached for 30 minutes per model).
    fn load_model(&mut self, cx: &mut Context<Self>) {
        let key = grid_key(&self.map, self.map.model);
        let now = chrono::Utc::now().timestamp();
        if let Some(g) = self.cache.get(&key).filter(|g| now - g.fetched_at < 1800) {
            self.map.grid = Some(g.clone());
            self.status = format!("{} loaded", self.map.model.label());
            self.changed(cx);
            return;
        }
        if self.loading {
            return;
        }
        self.loading = true;
        self.status = if crate::map::boundaries::counties_named().is_none() {
            "Waiting for county outlines to download…".to_string()
        } else if self.map.model.is_noaa() {
            format!("Loading NOAA {} (about 20 seconds)…", self.map.model.label())
        } else {
            format!("Loading {}…", self.map.model.label())
        };
        cx.notify();
        let (model, start, hours, kind) = (key.0, key.1, key.2, key.3);
        let (state, focus) = (self.map.state.clone(), self.map.focus);
        cx.spawn(async move |this, cx| {
            let result = crate::net::spawn(winter::fetch(state, focus, kind, model, start, hours)).await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(Ok(grid)) => {
                        this.cache.insert(key.clone(), grid.clone());
                        // Only fill if the map still wants this model (the user may have moved on).
                        if grid_key(&this.map, this.map.model) == key {
                            this.map.grid = Some(grid);
                            this.status = format!("{} loaded {}", model.label(), chrono::Local::now().format("%-I:%M %p"));
                            this.changed(cx);
                        } else {
                            this.load_model(cx);
                        }
                    }
                    Ok(Err(e)) => this.status = format!("Model not loaded: {e}"),
                    Err(e) => this.status = format!("Model not loaded: {e}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn fill_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reload_inputs = false;
        self.filling = true;
        let m = self.map.clone();
        self.title.update(cx, |s, cx| s.set_value(m.title.clone(), window, cx));
        self.subtitle.update(cx, |s, cx| s.set_value(m.subtitle.clone(), window, cx));
        self.period.update(cx, |s, cx| s.set_value(m.period.clone(), window, cx));
        self.state_input.update(cx, |s, cx| s.set_value(winter::region_codes(&m.state), window, cx));
        self.bands.clear();
        self.band_subs.clear();
        for (i, b) in m.bands.iter().enumerate() {
            let label = cx.new(|cx| InputState::new(window, cx).placeholder("Label"));
            let min = cx.new(|cx| InputState::new(window, cx).placeholder("From"));
            label.update(cx, |s, cx| s.set_value(b.label.clone(), window, cx));
            min.update(cx, |s, cx| s.set_value(trim_num(b.min), window, cx));
            self.band_subs.push(cx.subscribe_in(&label, window, move |this, state, e: &InputEvent, _, cx| {
                if matches!(e, InputEvent::Change) && !this.filling {
                    if let Some(b) = this.map.bands.get_mut(i) {
                        b.label = state.read(cx).value().to_string();
                        this.changed(cx);
                    }
                }
            }));
            self.band_subs.push(cx.subscribe_in(&min, window, move |this, state, e: &InputEvent, _, cx| {
                if matches!(e, InputEvent::Change) && !this.filling {
                    if let (Some(b), Ok(v)) = (this.map.bands.get_mut(i), state.read(cx).value().trim().trim_end_matches('"').parse::<f32>()) {
                        b.min = v;
                        this.changed(cx);
                    }
                }
            }));
            self.bands.push((label, min));
        }
        self.filling = false;
    }

    fn rerender(&mut self, cx: &mut Context<Self>) {
        self.dirty = false;
        self.render_gen += 1;
        let gen = self.render_gen;
        let doc = crate::gfx::templates::winter_map(&self.map);
        let state = self.app.read(cx);
        let settings = state.settings.clone();
        let data = state.data.clone();
        let task = cx.background_executor().spawn(async move {
            let pm = crate::program::render_graphic(&doc, &settings, &data, crate::gfx::render::SETTLED, 1280, 720, None);
            let (w, h) = (pm.width(), pm.height());
            let mut px = pm.take();
            for p in px.chunks_exact_mut(4) {
                p.swap(0, 2);
                p[3] = 255;
            }
            image::RgbaImage::from_raw(w, h, px).map(|b| Arc::new(RenderImage::new(vec![image::Frame::new(b)])))
        });
        cx.spawn(async move |this, cx| {
            let image = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.render_gen == gen {
                    this.preview = image;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// A click on the preview paints the county under it with the selected brush.
    fn paint_at(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if self.brush == Brush::Off {
            return;
        }
        let Some((fx, fy)) = self.slot.to_frame(position) else { return };
        let (mx, my, mw, mh) = crate::gfx::templates::WINTER_MAP_RECT;
        if self.brush == Brush::Zone {
            let Some(p) = crate::gfx::winter_map::to_lonlat(&self.map, mw as f64, mh as f64, fx - mx as f64, fy - my as f64) else { return };
            if let Some(z) = self.map.zones.last_mut().filter(|z| z.open) {
                z.points.push(p);
                self.changed(cx);
            }
            return;
        }
        let Some(fips) = crate::gfx::winter_map::pick(&self.map, mw as f64, mh as f64, fx - mx as f64, fy - my as f64) else { return };
        match self.brush {
            Brush::Band(i) => {
                let Some(v) = band_value(&self.map, i) else { return };
                self.map.overrides.insert(fips, v);
            }
            Brush::NoSnow => {
                self.map.overrides.insert(fips, 0.0);
            }
            Brush::Erase => {
                self.map.overrides.remove(&fips);
            }
            Brush::Off | Brush::Zone => {}
        }
        self.changed(cx);
    }

    fn take_to_air(&mut self, cx: &mut Context<Self>) {
        let doc = crate::gfx::templates::winter_map(&self.map);
        self.app.update(cx, |s, cx| {
            s.take_graphic(doc, Transition::Fade);
            cx.notify();
        });
    }

    fn open_in_graphics(&mut self, cx: &mut Context<Self>) {
        let doc = crate::gfx::templates::winter_map(&self.map);
        self.app.update(cx, |s, cx| {
            if let Some(saved) = s.save_graphic(doc) {
                s.open_graphic(saved.id);
            }
            cx.notify();
        });
    }
}

/// A 640 x 430 picture of just the map (no header or legend) for the model comparison.
fn thumbnail(map: &WinterMap, font: &str) -> Option<Arc<RenderImage>> {
    let (w, h) = (640u32, 430u32);
    let mut pm = tiny_skia::Pixmap::new(w, h)?;
    pm.fill(tiny_skia::Color::from_rgba8(8, 16, 32, 255));
    let mut glyphs = crate::gfx::text::GlyphCache::default();
    crate::gfx::winter_map::draw_map(&mut pm, &mut glyphs, tiny_skia::Transform::identity(), None, w as f32, h as f32, map, 1.0, font);
    let mut px = pm.take();
    for p in px.chunks_exact_mut(4) {
        p.swap(0, 2);
        p[3] = 255;
    }
    image::RgbaImage::from_raw(w, h, px).map(|b| Arc::new(RenderImage::new(vec![image::Frame::new(b)])))
}

/// The value a painted county gets for band `i`: halfway to the next band up (the top band
/// gets its own threshold plus a little).
fn band_value(map: &WinterMap, i: usize) -> Option<f32> {
    let b = map.bands.get(i)?;
    let next = map.bands.iter().map(|o| o.min).filter(|m| *m > b.min).fold(f32::MAX, f32::min);
    Some(if next == f32::MAX { b.min * 1.15 + 0.01 } else { ((b.min + next) / 2.0 * 10.0).round() / 10.0 })
}

fn trim_num(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn home_state(app: &Entity<AppState>, cx: &App) -> String {
    let home = app.read(cx).settings.home;
    crate::map::boundaries::counties_named()
        .and_then(|all| {
            all.iter()
                .find(|c| c.rings.iter().any(|r| crate::gfx::winter_map::inside_pub(r, home.lon, home.lat)))
                .map(|c| c.state.clone())
        })
        .unwrap_or_else(|| "18".into())
}

impl Render for WinterStudio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.reload_inputs {
            self.fill_inputs(window, cx);
        }
        if self.dirty {
            self.rerender(cx);
        }
        let m = self.map.clone();
        let maps = winter::all();

        // --- Maps -----------------------------------------------------------------------
        let map_chips = maps.iter().take(12).enumerate().map(|(i, saved)| {
            let saved = saved.clone();
            let active = saved.id == m.id;
            chip(("ws-map", i), format!("{} · {}{}", saved.title, winter::state_code(&saved.state), if saved.based_on.is_empty() { "" } else { " (update)" }), active)
                .on_click(cx.listener(move |this, _, _, cx| this.open(saved.clone(), cx)))
        });
        let maps_section = section("Winter maps")
            .child(row().flex_wrap().gap(px(6.)).children(map_chips))
            .child(
                row()
                    .flex_wrap()
                    .gap(px(6.))
                    .children(WinterKind::ALL.iter().enumerate().map(|(i, k)| {
                        let k = *k;
                        button_small(("ws-new", i), format!("New {}", k.label().to_lowercase())).on_click(cx.listener(move |this, _, _, cx| this.new_map(k, cx)))
                    }))
                    .child(button_small("ws-update", "Make an update (keeps this one)").on_click(cx.listener(|this, _, _, cx| {
                        let update = this.map.make_update();
                        this.open(update, cx);
                    })))
                    .child(button_small("ws-del", "Delete this map").on_click(cx.listener(|this, _, _, cx| {
                        winter::delete(&this.map.id);
                        let next = winter::all().first().cloned().unwrap_or_else(|| WinterMap::new(WinterKind::Snow, &this.map.state));
                        this.open(next, cx);
                    }))),
            )
            .child(label("Quick start"))
            .child(
                row()
                    .flex_wrap()
                    .gap(px(6.))
                    .child(button_small("ws-first-call", "First call (3 days)").on_click(cx.listener(|this, _, _, cx| this.new_preset(WinterKind::Snow, 72, "First call", cx))))
                    .child(button_small("ws-final-call", "Final call (2 days)").on_click(cx.listener(|this, _, _, cx| this.new_preset(WinterKind::Snow, 48, "Final call", cx))))
                    .child(button_small("ws-ice-storm", "Ice storm").on_click(cx.listener(|this, _, _, cx| this.new_preset(WinterKind::Ice, 48, "Ice storm", cx)))),
            );

        // --- Text -------------------------------------------------------------------------
        // Area: whole states (one or several) or only the home area (CWXB-83 item 9).
        let home = self.app.read(cx).settings.home;
        let focus_km = m.focus.map(|f| f.km);
        let area_chips = [(None, "Whole states"), (Some(97.0), "60 mi around home"), (Some(161.0), "100 mi around home")].into_iter().enumerate().map(|(i, (km, text))| {
            let on = match (km, focus_km) {
                (None, None) => true,
                (Some(a), Some(b)) => (a - b).abs() < 1.0,
                _ => false,
            };
            chip(("ws-area", i), text, on).on_click(cx.listener(move |this, _, _, cx| {
                let focus = km.map(|km| winter::Focus { lon: home.lon, lat: home.lat, km });
                if focus != this.map.focus {
                    let state = this.map.state.clone();
                    this.set_region(state, focus, cx);
                }
            }))
        });
        let text_section = section(format!("{} · {}", m.kind.label(), m.area_name()))
            .child(label("States"))
            .child(Input::new(&self.state_input))
            .child(
                row()
                    .flex_wrap()
                    .gap(px(6.))
                    .children(area_chips)
                    .child(button_small("ws-nearby", "Add nearby states").on_click(cx.listener(|this, _, _, cx| {
                        if let Some(states) = this.nearby_states(cx) {
                            let focus = this.map.focus;
                            this.set_region(states, focus, cx);
                        }
                    }))),
            )
            .child(label("Title"))
            .child(Input::new(&self.title))
            .child(label("Subtitle"))
            .child(Input::new(&self.subtitle))
            .child(label("Legend heading"))
            .child(Input::new(&self.period));

        // --- Model ------------------------------------------------------------------------
        let model_chips = GridModel::ALL.iter().enumerate().map(|(i, g)| {
            let g = *g;
            chip(("ws-model", i), g.label(), m.model == g).on_click(cx.listener(move |this, _, _, cx| {
                this.map.model = g;
                this.load_model(cx);
                this.changed(cx);
            }))
        });
        let hour_chips = HOURS.iter().enumerate().map(|(i, h)| {
            let h = *h;
            let reach = m.model.max_hours();
            chip(("ws-hours", i), if h < 48 { format!("{h} h") } else { format!("{} days", h / 24) }, m.hours == h)
                .when(h > reach, |d| d.opacity(0.4))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.map.hours = h;
                    this.map.period = winter::window_text(this.map.start, h);
                    this.reload_inputs = true;
                    this.load_model(cx);
                    this.changed(cx);
                }))
        });
        // When the window starts (CWXB-83): "Tonight", "Tomorrow"… are the next such times.
        let start_chips = winter::start_choices().into_iter().enumerate().map(|(i, (text, start))| {
            // The chip for the current start: same label as when it was picked, within an hour.
            let on = if start == 0 { m.start == 0 } else { m.start.abs_diff(start) <= 1 && m.start != 0 };
            chip(("ws-start", i), text, on).on_click(cx.listener(move |this, _, _, cx| {
                this.map.start = start;
                this.map.period = winter::window_text(start, this.map.hours);
                this.reload_inputs = true;
                this.load_model(cx);
                this.changed(cx);
            }))
        });
        let model_section = section("Forecast model")
            .child(row().flex_wrap().gap(px(6.)).children(model_chips))
            .child(label("Starts"))
            .child(row().flex_wrap().gap(px(6.)).children(start_chips))
            // Step through a storm one window at a time (Tonight → Tomorrow → Tomorrow night…).
            .child(
                row()
                    .gap(px(6.))
                    .child(button_small("ws-prev-period", "◀ Previous period").when(m.start == 0, |d| d.opacity(0.4)).on_click(cx.listener(|this, _, _, cx| {
                        let next = this.map.start.saturating_sub(this.map.hours);
                        this.set_start(next, cx);
                    })))
                    .child(button_small("ws-next-period", "Next period ▶").when(m.start + 2 * m.hours > m.model.max_hours(), |d| d.opacity(0.4)).on_click(cx.listener(|this, _, _, cx| {
                        let next = this.map.start + this.map.hours;
                        if next + this.map.hours <= this.map.model.max_hours() {
                            this.set_start(next, cx);
                        }
                    }))),
            )
            .child(label("Window"))
            .child(row().flex_wrap().gap(px(6.)).children(hour_chips))
            .child(muted(format!("{} reaches {} hours. {}", m.model.label(), m.model.max_hours(), m.model.about())))
            .child(
                row()
                    .gap(px(8.))
                    .child(button_small("ws-reload", if self.loading { "Loading…" } else { "Reload model" }).on_click(cx.listener(|this, _, _, cx| {
                        this.cache.clear();
                        this.load_model(cx);
                    })))
                    .child(button_small("ws-compare", if self.compare.is_some() { "Close comparison" } else { "Compare models" }).on_click(cx.listener(|this, _, _, cx| {
                        if this.compare.is_some() {
                            this.compare = None;
                            cx.notify();
                        } else {
                            this.start_compare(cx);
                        }
                    }))),
            )
            .child(div().text_size(px(12.)).text_color(c(MUTED)).child(self.status.clone()));

        // --- Legend -----------------------------------------------------------------------
        let band_rows = m.bands.iter().enumerate().map(|(i, b)| {
            let inputs = self.bands.get(i).cloned();
            let color = b.color.clone();
            row()
                .id(("ws-band", i))
                .gap(px(6.))
                .child(swatch(("ws-band-color", i), &color, false).on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(b) = this.map.bands.get_mut(i) {
                        let at = PALETTE.iter().position(|p| p.eq_ignore_ascii_case(&b.color)).map(|p| (p + 1) % PALETTE.len()).unwrap_or(0);
                        b.color = PALETTE[at].to_string();
                        this.changed(cx);
                    }
                })))
                .when_some(inputs, |r, (l, v)| r.child(div().flex_1().min_w_0().child(Input::new(&l))).child(div().w(px(64.)).child(Input::new(&v))))
                .child(button_small(("ws-band-del", i), "✕").on_click(cx.listener(move |this, _, _, cx| {
                    if this.map.bands.len() > 1 {
                        this.map.bands.remove(i);
                        this.reload_inputs = true;
                        this.changed(cx);
                    }
                })))
        });
        let legend_section = section("Legend (top band first)")
            .child(muted("Colour: click the swatch. Second box: the amount where the band starts."))
            .children(band_rows)
            .child(
                row()
                    .gap(px(6.))
                    .child(button_small("ws-band-add", "Add band").on_click(cx.listener(|this, _, _, cx| {
                        let low = this.map.bands.last().map(|b| b.min).unwrap_or(1.0);
                        this.map.bands.push(Band { label: "New band".into(), color: "#D6ECFF".into(), min: (low / 2.0 * 10.0).round() / 10.0 });
                        this.reload_inputs = true;
                        this.changed(cx);
                    })))
                    .child(button_small("ws-band-sort", "Sort by amount").on_click(cx.listener(|this, _, _, cx| {
                        this.map.sort_bands();
                        this.reload_inputs = true;
                        this.changed(cx);
                    })))
                    .child(button_small("ws-band-reset", "Reset legend").on_click(cx.listener(|this, _, _, cx| {
                        this.map.bands = this.map.kind.default_bands();
                        this.reload_inputs = true;
                        this.changed(cx);
                    }))),
            );

        // --- Paint ------------------------------------------------------------------------
        let brush = self.brush;
        let brushes = m.bands.iter().enumerate().map(|(i, b)| {
            chip(("ws-brush", i), b.label.clone(), brush == Brush::Band(i)).on_click(cx.listener(move |this, _, _, cx| {
                this.brush = if this.brush == Brush::Band(i) { Brush::Off } else { Brush::Band(i) };
                cx.notify();
            }))
        });
        let paint_section = section("Paint counties")
            .child(muted("Pick a band, then click counties on the preview. Painted counties override the model."))
            .child(
                row()
                    .flex_wrap()
                    .gap(px(6.))
                    .children(brushes)
                    .child(chip("ws-brush-none", "None", brush == Brush::NoSnow).on_click(cx.listener(|this, _, _, cx| {
                        this.brush = if this.brush == Brush::NoSnow { Brush::Off } else { Brush::NoSnow };
                        cx.notify();
                    })))
                    .child(chip("ws-brush-erase", "Back to model", brush == Brush::Erase).on_click(cx.listener(|this, _, _, cx| {
                        this.brush = if this.brush == Brush::Erase { Brush::Off } else { Brush::Erase };
                        cx.notify();
                    }))),
            )
            .child(
                row()
                    .gap(px(6.))
                    .child(muted(format!("{} counties painted", m.overrides.len())))
                    .child(button_small("ws-clear-paint", "Clear painting").on_click(cx.listener(|this, _, _, cx| {
                        this.map.overrides.clear();
                        this.changed(cx);
                    }))),
            );

        // --- Hand-drawn zones (CWXB-83 item 8) ------------------------------------------------
        let drawing = brush == Brush::Zone;
        let label_chips = ZONE_LABELS.iter().enumerate().map(|(i, text)| {
            let text = *text;
            chip(("ws-zone-label", i), text, false).on_click(cx.listener(move |this, _, window, cx| {
                this.zone_label.update(cx, |s, cx| s.set_value(text.to_string(), window, cx));
                // Renaming the zone being drawn too.
                if let Some(z) = this.map.zones.last_mut().filter(|z| z.open) {
                    z.label = text.to_string();
                    this.changed(cx);
                }
                cx.notify();
            }))
        });
        let color_chips = ZONE_COLORS.iter().enumerate().map(|(i, col)| {
            swatch(("ws-zone-color", i), col, self.zone_color == i).on_click(cx.listener(move |this, _, _, cx| {
                this.zone_color = i;
                if let Some(z) = this.map.zones.last_mut().filter(|z| z.open) {
                    z.color = ZONE_COLORS[i].to_string();
                    this.changed(cx);
                }
                cx.notify();
            }))
        });
        let zone_rows = m.zones.iter().enumerate().filter(|(_, z)| !z.open).map(|(i, z)| {
            row()
                .gap(px(6.))
                .items_center()
                .child(div().size(px(14.)).rounded(px(3.)).bg(crate::ui::hex_color(&z.color)))
                .child(div().flex_1().min_w_0().text_size(px(13.)).text_color(c(TEXT)).child(if z.label.trim().is_empty() { "(no label)".to_string() } else { z.label.clone() }))
                .child(button_small(("ws-zone-del", i), "✕").on_click(cx.listener(move |this, _, _, cx| {
                    if i < this.map.zones.len() {
                        this.map.zones.remove(i);
                        this.changed(cx);
                    }
                })))
        });
        let points = m.zones.last().filter(|z| z.open).map(|z| z.points.len()).unwrap_or(0);
        let zones_section = section("Draw zones")
            .child(muted("Outline an area by clicking around it on the map, then Finish. Drawn zones sit on top of the colours."))
            .child(row().flex_wrap().gap(px(6.)).children(label_chips))
            .child(Input::new(&self.zone_label))
            .child(row().gap(px(6.)).child(label("Colour")).children(color_chips))
            .child(
                row()
                    .flex_wrap()
                    .gap(px(6.))
                    .when(!drawing, |r| r.child(button_small("ws-zone-start", "Draw a zone").on_click(cx.listener(|this, _, _, cx| this.start_zone(cx)))))
                    .when(drawing, |r| {
                        r.child(button_primary("ws-zone-finish", if points >= 3 { "Finish zone" } else { "Cancel zone" }).on_click(cx.listener(|this, _, _, cx| {
                            this.finish_zone();
                            cx.notify();
                        })))
                        .child(button_small("ws-zone-undo", "Undo last point").when(points == 0, |d| d.opacity(0.4)).on_click(cx.listener(|this, _, _, cx| {
                            if let Some(z) = this.map.zones.last_mut().filter(|z| z.open) {
                                z.points.pop();
                                this.changed(cx);
                            }
                        })))
                    }),
            )
            .children(zone_rows)
            .when(m.zones.iter().any(|z| !z.open), |s| {
                let reviewed = self.zones_reviewed;
                s.child(toggle("ws-zones-reviewed", "I've checked these zones for air", reviewed).on_click(cx.listener(|this, _, _, cx| {
                    this.zones_reviewed = !this.zones_reviewed;
                    cx.notify();
                })))
                .child(button_small("ws-zones-graphic", "Make a Forecast Zones graphic").when(!reviewed, |d| d.opacity(0.4)).on_click(cx.listener(|this, _, _, cx| {
                    if this.zones_reviewed {
                        this.zones_to_graphic(cx);
                    } else {
                        this.app.update(cx, |s, cx| {
                            s.toast(crate::app::ToastKind::Info, "Check the zones first, then tick \"I've checked these zones for air\"".to_string());
                            cx.notify();
                        });
                    }
                })))
            });

        // --- Related graphics (Graphics Studio) -----------------------------------------------
        let related_section = section("More winter graphics")
            .child(muted("Opens a new graphic in Graphics Studio: alerts, reported totals, cold and timeline maps."))
            .child(row().flex_wrap().gap(px(6.)).children(RELATED.iter().enumerate().map(|(i, (text, key))| {
                let key = *key;
                button_small(("ws-related", i), *text).on_click(cx.listener(move |this, _, _, cx| this.open_related(key, cx)))
            })));

        // --- Display ----------------------------------------------------------------------
        let modern = crate::gfx::look::normalize(&m.look) == "modern";
        let display_section = section("On the map")
            .child(row().gap(px(6.)).child(label("Look")).children([("modern", "Modern"), ("classic", "Classic")].into_iter().enumerate().map(|(i, (key, name))| {
                chip(("ws-look", i), name, (key == "modern") == modern).on_click(cx.listener(move |this, _, _, cx| {
                    this.map.look = key.to_string();
                    this.changed(cx);
                }))
            })))
            .child(toggle("ws-smooth", "Smooth model contours (off: whole counties)", m.smooth).on_click(cx.listener(|this, _, _, cx| {
                this.map.smooth = !this.map.smooth;
                this.changed(cx);
            })))
            .child(toggle("ws-blips", "Town totals", m.blips).on_click(cx.listener(|this, _, _, cx| {
                this.map.blips = !this.map.blips;
                this.changed(cx);
            })))
            .child(toggle("ws-blip-bands", "Town placards show the band name instead of the number", m.blip_bands).on_click(cx.listener(|this, _, _, cx| {
                this.map.blip_bands = !this.map.blip_bands;
                this.changed(cx);
            })))
            .child(row().gap(px(6.)).child(label("Towns")).children([6u32, 10, 14, 20].iter().enumerate().map(|(i, n)| {
                let n = *n;
                chip(("ws-towns", i), n.to_string(), m.towns == n).on_click(cx.listener(move |this, _, _, cx| {
                    this.map.towns = n;
                    this.changed(cx);
                }))
            })));

        let sidebar = col()
            .id("ws-side")
            .w(px(400.))
            .h_full()
            .flex_shrink_0()
            .bg(c(PANEL))
            .border_r_1()
            .border_color(c(BORDER))
            .overflow_y_scroll()
            .child(maps_section)
            .child(text_section)
            .child(model_section)
            .child(legend_section)
            .child(paint_section)
            .child(zones_section)
            .child(display_section)
            .child(related_section);

        let painting = self.brush != Brush::Off;
        // Model comparison: thumbnails in a grid, click one to use it.
        let compare_view = self.compare.as_ref().map(|cells| {
            let loaded: Vec<WinterGrid> = cells.iter().filter_map(|c| c.grid.clone()).collect();
            let thumbs = cells.iter().enumerate().map(|(i, cell)| {
                let grid = cell.grid.clone();
                let reach = cell.model.max_hours();
                let note = if !cell.status.is_empty() {
                    cell.status.clone()
                } else if m.start + m.hours > reach {
                    format!("Reaches {reach} h: shorter window")
                } else {
                    "Click to use this model".to_string()
                };
                col()
                    .id(("ws-cmp", i))
                    .w(px(330.))
                    .gap(px(4.))
                    .p(px(6.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(BORDER))
                    .bg(c(PANEL))
                    .when(grid.is_some(), |d| d.cursor_pointer().hover(|s| s.border_color(c(TEXT))))
                    .child(div().text_size(px(14.)).text_color(c(TEXT)).child(cell.model.label()))
                    .child(div().w_full().h(px(212.)).child(frame_canvas(cell.image.clone(), cell.slot.clone()).size_full()))
                    .child(div().text_size(px(12.)).text_color(c(MUTED)).child(note))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(g) = grid.clone() {
                            this.adopt(g, cx);
                        }
                    }))
            });
            col()
                .flex_1()
                .min_h_0()
                .p(px(16.))
                .gap(px(10.))
                .id("ws-compare-view")
                .overflow_y_scroll()
                .child(
                    row()
                        .gap(px(8.))
                        .items_center()
                        .child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(format!("{} models, same window and legend. GFS is read from NOAA; the others come through Open-Meteo.", cells.len())))
                        .child(button_small("ws-cmp-avg", format!("Use the average of {} loaded", loaded.len())).when(loaded.len() < 2, |d| d.opacity(0.4)).on_click(cx.listener(move |this, _, _, cx| {
                            let refs: Vec<&WinterGrid> = loaded.iter().collect();
                            if refs.len() >= 2 {
                                if let Some(avg) = WinterGrid::average(&refs) {
                                    this.adopt(avg, cx);
                                }
                            }
                        }))),
                )
                .child(row().flex_wrap().gap(px(10.)).children(thumbs))
        });
        let stage = col()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(c(BG))
            .child(
                row()
                    .gap(px(8.))
                    .px(px(14.))
                    .py(px(10.))
                    .border_b_1()
                    .border_color(c(BORDER))
                    .child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(if self.brush == Brush::Zone {
                        "Drawing a zone: click around the area on the map, then Finish zone".to_string()
                    } else if painting {
                        "Painting: click counties on the map".to_string()
                    } else {
                        format!("{} · {}", m.title, m.source_text())
                    }))
                    .child(button_small("ws-csv", "County list (CSV)").on_click(cx.listener(|this, _, _, cx| this.export_county_csv(cx))))
                    .child(button_small("ws-open-gfx", "Edit in Graphics Studio").on_click(cx.listener(|this, _, _, cx| this.open_in_graphics(cx))))
                    .child(button_primary("ws-air", "Take to air").on_click(cx.listener(|this, _, _, cx| this.take_to_air(cx)))),
            )
            .when_some(compare_view, |s, v| s.child(v))
            .when(self.compare.is_none(), |s| {
                s.child(
                    div()
                        .id("ws-canvas")
                        .flex_1()
                        .min_h_0()
                        .p(px(16.))
                        .when(painting, |d| d.cursor_crosshair())
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, _, cx| this.paint_at(e.position, cx)))
                        .child(frame_canvas(self.preview.clone(), self.slot.clone()).size_full()),
                )
            });

        row().size_full().child(sidebar).child(stage)
    }
}

#[cfg(test)]
mod tests {
    /// Every "More winter graphics" button opens a template this version has.
    #[test]
    fn related_graphics_exist() {
        for (name, key) in super::RELATED {
            assert!(crate::gfx::templates::build(key).is_some(), "{name}: {key}");
        }
    }
}
