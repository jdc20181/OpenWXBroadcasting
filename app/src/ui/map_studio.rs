//! Map Studio: the live map with radar (composite or single-site), satellite, data layers and
//! Futurecast, severe-weather tools, telestrator, comparison layouts, timeline and bookmarks.
//!
//! The left panel is split into tabs so each job has one place: Radar, Satellite, Data, Severe,
//! On-Air and Layout. The toolbar above the map holds every drawing tool, with a one-line hint
//! for the tool in use.

use std::rc::Rc;

use gpui_kit::{
    component::input::{Input, InputEvent, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::{AppState, FrameStore, ToastKind},
    config::{Basemap, Bookmark, FRAME_H, FRAME_W},
    data::{palette_label, NEXRAD_SITES, RADAR_PALETTES},
    gfx::{model::new_id, templates},
    map::{
        geo::{bearing_deg, haversine_km, Viewport, KM_PER_MILE},
        scene::{
            default_panels, Annotation, AnnotationKind, DataField, FieldGroup, MapScene, PanelLayout, PanelProduct, PanelSpec, RaiseStyle,
            PrecipWindow, RadarMode, RadarSource, RangeRings, SatChannel, SiteProduct, StormTrack,
        },
    },
    ui::{
        badge, button_primary, button_small, c, ca, chip, col, frame_view::{frame_canvas_with_trace, ImageSlot},
        hex_color, kv, label, muted, row, section, slider, swatch, toggle, AMBER, BG, BORDER, CYAN, DIM, GREEN,
        MUTED, PANEL, PANEL_2, RED, TEXT,
    },
};

#[path = "map_studio_chrome.rs"]
mod chrome;
use chrome::Dd;
#[path = "map_studio_ribbon.rs"]
mod ribbon;
use ribbon::RibbonTab;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Pan,
    Pen,
    Arrow,
    Circle,
    Area,
    RaisedArea,
    RaisedBoundary,
    Text,
    Marker,
    Spotlight,
    Ruler,
    Front,
    High,
    Low,
    StormTrack,
    FreeTrack,
    Rings,
    Arrange,
}

impl Tool {
    const DRAW: [Tool; 11] = [
        Tool::Pan,
        Tool::Pen,
        Tool::Arrow,
        Tool::Circle,
        Tool::Area,
        Tool::RaisedArea,
        Tool::RaisedBoundary,
        Tool::Text,
        Tool::Marker,
        Tool::Spotlight,
        Tool::Ruler,
    ];
    const WEATHER: [Tool; 3] = [Tool::Front, Tool::High, Tool::Low];
    const SEVERE: [Tool; 3] = [Tool::StormTrack, Tool::FreeTrack, Tool::Rings];
    const ONAIR: [Tool; 1] = [Tool::Arrange];

    fn all() -> impl Iterator<Item = Tool> {
        Self::DRAW.into_iter().chain(Self::WEATHER).chain(Self::SEVERE).chain(Self::ONAIR)
    }

    fn label(self) -> &'static str {
        match self {
            Tool::Pan => "Pan",
            Tool::Pen => "Pen",
            Tool::Arrow => "Arrow",
            Tool::Circle => "Circle",
            Tool::Area => "Area",
            Tool::RaisedArea => "Raise area",
            Tool::RaisedBoundary => "Raise boundary",
            Tool::Text => "Text",
            Tool::Marker => "Marker",
            Tool::Spotlight => "Spotlight",
            Tool::Ruler => "Ruler",
            Tool::Front => "Front",
            Tool::High => "High",
            Tool::Low => "Low",
            Tool::StormTrack => "Storm track",
            Tool::FreeTrack => "Freeform track",
            Tool::Rings => "Range rings",
            Tool::Arrange => "Arrange on-air",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Tool::Pan => "V",
            Tool::Pen => "P",
            Tool::Arrow => "A",
            Tool::Circle => "C",
            Tool::Area => "G",
            Tool::RaisedArea | Tool::RaisedBoundary => "",
            Tool::Text => "T",
            Tool::Marker => "M",
            Tool::Spotlight => "O",
            Tool::Ruler => "U",
            Tool::Front => "N",
            Tool::High => "H",
            Tool::Low => "L",
            Tool::StormTrack => "S",
            Tool::FreeTrack => "F",
            Tool::Rings => "R",
            Tool::Arrange => "E",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Tool::Pan => "Drag to pan, scroll to zoom, double-click to zoom in. Click a warning polygon to call it out on air; click empty map to clear it.",
            Tool::Pen => "Drag to draw freehand. Undo removes the last mark.",
            Tool::Arrow => "Drag from the tail to the point of the arrow.",
            Tool::Circle => "Drag from the centre outward.",
            Tool::Area => "Drag around an area to shade it.",
            Tool::RaisedArea => "Trace around the area. Release at the starting dot, or confirm the closing edge. Esc cancels.",
            Tool::RaisedBoundary => "Click inside a county or state to raise its outline.",
            Tool::Text => "Click to place the label typed in the box on the right.",
            Tool::Marker => "Click to drop a pin with the label typed in the box on the right.",
            Tool::Spotlight => "Drag a circle around a storm: everything else dims.",
            Tool::Ruler => "Drag between two points for distance and drive time.",
            Tool::Front => "Drag along the front. Symbols go on your left - draw with the cold air behind you.",
            Tool::High => "Click to place a High. Shows model pressure when Data is loaded.",
            Tool::Low => "Click to place a Low. Shows model pressure when Data is loaded.",
            Tool::StormTrack => "Press on the storm and drag toward where it will be in one hour.",
            Tool::FreeTrack => "Press on the storm and draw the path it will take. Towns along it get ETAs.",
            Tool::Rings => "Click to move the range rings' centre.",
            Tool::Arrange => "Drag the title, legend, time box, clock, watermark, sponsor badge or other on-air graphic so nothing overlaps. Reset in the On-Air tab.",
        }
    }
}

/// The main things the map can show (the switch at the top of Map Studio).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewMode {
    LiveRadar,
    SingleSite,
    Satellite,
    Futurecast,
    DataMaps,
    /// SPC convective outlook map (overall risk and tornado / wind / hail chances), with the
    /// WPC excessive rainfall and SPC fire outlooks alongside.
    Outlook,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MapMenu { Map, Forecast, PastRadar, Draw, OnAir, Export, Show }

/// One hit in "Find anything" above the map.
#[derive(Clone)]
enum FindAction {
    Field(DataField),
    Noaa(crate::futurecast::Product),
    Model(crate::grid::GridModel),
    Menu(MapMenu),
    Tool(Tool),
    Tab(LeftTab),
    /// SPC outlook map: (day 1-3, layer), or day 0 with "ero" / "fire".
    Outlook(u8, &'static str),
    /// A one-off command (Refresh radar, Clear storm tracks, …).
    Command(&'static str),
    /// Another workspace (Severe Center, Winter Studio, …).
    Workspace(crate::app::Workspace),
    /// A tool window or Settings page (opened by the shell).
    Open(crate::app::OpenRequest),
    /// A new graphic from this template, opened in Graphics Studio.
    Template(&'static str),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ForecastPicker { Model, Product, Range, Winter }

impl MapMenu {
    fn label(self) -> &'static str {
        match self {
            Self::Map => "Map",
            Self::Forecast => "Forecast",
            Self::PastRadar => "Past radar",
            Self::Draw => "Draw & raise",
            Self::OnAir => "On air",
            Self::Export => "Export",
            Self::Show => "Add to show",
        }
    }

    /// What the menu holds, for Find.
    fn keywords(self) -> &'static str {
        match self {
            Self::Map => "live radar single site satellite layers warnings watches lightning cells reports roads counties states labels basemap airport metar observations station plots",
            Self::Forecast => "model gfs nam hrrr nbm ecmwf icon gem futurecast precipitation snow ice temperature wind severe",
            Self::PastRadar => "radar history replay archive loop yesterday past",
            Self::Draw => "draw pen arrow circle text marker raise lift spotlight boundary county state ruler front",
            Self::OnAir => "banner title sponsor logo watermark lower third crawl clock legend arrange",
            Self::Export => "png gif mp4 video image snapshot export save",
            Self::Show => "show scene rundown add",
        }
    }
}

impl ViewMode {
    const ALL: [ViewMode; 6] = [ViewMode::LiveRadar, ViewMode::SingleSite, ViewMode::Satellite, ViewMode::Futurecast, ViewMode::DataMaps, ViewMode::Outlook];

    fn label(self) -> &'static str {
        match self {
            ViewMode::LiveRadar => "Live radar",
            ViewMode::SingleSite => "Single site",
            ViewMode::Satellite => "Satellite",
            ViewMode::Futurecast => "Futurecast",
            ViewMode::DataMaps => "Data maps",
            ViewMode::Outlook => "Outlooks",
        }
    }
}

/// Quick-jump offsets (hours from `now`) for a map grid of `span` hours: whole local clock hours
/// at a fixed time-of-day step (3 h for a day, 6 h for two, 12 h up to four days, then noon each
/// day), so the chips land on the same times every day instead of "12:19 AM, 3:19 PM, 6:19 AM".
fn grid_marks(now: i64, span: u32) -> Vec<f32> {
    use chrono::Timelike;
    let step: u32 = match span {
        0..=24 => 3,
        25..=48 => 6,
        49..=96 => 12,
        _ => 24,
    };
    let first = (now / 3600 + 1) * 3600;
    let end = now + span as i64 * 3600;
    (0..)
        .map(|k| first + k * 3600)
        .take_while(|t| *t <= end)
        .filter(|t| {
            let hour = chrono::DateTime::from_timestamp(*t, 0).map(|d| d.with_timezone(&chrono::Local).hour()).unwrap_or(0);
            if step == 24 { hour == 12 } else { hour % step == 0 }
        })
        .map(|t| (t - now) as f32 / 3600.0)
        .collect()
}

/// The regular map: composite radar loop, no data layer, totals, satellite or comparison panels.
fn back_to_radar(l: &mut MapScene) {
    l.data.field = None;
    l.radar_replay = None;
    l.site_replay = None;
    l.history_time = None;
    l.precip_total = None;
    l.satellite.enabled = false;
    l.radar.enabled = true;
    l.radar.source = RadarSource::Composite;
    l.layout = PanelLayout::Single;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LeftTab {
    Radar,
    Satellite,
    Data,
    Severe,
    OnAir,
    Layout,
}

impl LeftTab {
    const ALL: [LeftTab; 6] = [LeftTab::Radar, LeftTab::Satellite, LeftTab::Data, LeftTab::Severe, LeftTab::OnAir, LeftTab::Layout];

    fn label(self) -> &'static str {
        match self {
            LeftTab::Radar => "Radar",
            LeftTab::Satellite => "Satellite",
            LeftTab::Data => "Forecast",
            LeftTab::Severe => "Warnings",
            LeftTab::OnAir => "On-Air",
            LeftTab::Layout => "Compare",
        }
    }
}

const FRONT_KINDS: [AnnotationKind; 5] = [
    AnnotationKind::ColdFront,
    AnnotationKind::WarmFront,
    AnnotationKind::StationaryFront,
    AnnotationKind::OccludedFront,
    AnnotationKind::Trough,
];

/// Edit comparison panel `p` (1-3), creating default specs for older scenes as needed.
fn edit_panel(scene: &mut MapScene, p: usize, f: impl FnOnce(&mut PanelSpec)) {
    while scene.panels.len() < p {
        let next = default_panels().get(scene.panels.len()).cloned().unwrap_or_default();
        scene.panels.push(next);
    }
    if let Some(spec) = scene.panels.get_mut(p - 1) {
        f(spec);
    }
}

pub const PEN_COLORS: &[&str] = &["#FF3B3B", "#FFD60A", "#2ED3F3", "#3DD68C", "#FFFFFF", "#FF7AE0", "#000000"];

enum Drag {
    Pan {
        start: (f64, f64),
        view: crate::config::GeoView,
    },
    Draw {
        id: String,
    },
    Raise {
        viewport: Viewport,
        layout: PanelLayout,
        wipe: f32,
        tracing: bool,
        trace: Vec<(f64, f64)>,
    },
    Track,
    FreeTrack,
    Element {
        key: &'static str,
        start: (f64, f64),
        orig: (f32, f32),
    },
}

// A deliberate loop can be traced in either direction. Use on-screen pixels for
// the closure and area tolerances so they stay consistent as the preview resizes.
fn closed_raise_trace(trace: &[(f64, f64)], end: (f64, f64), scale: f64) -> bool {
    let [start, _, _, ..] = trace else { return false };
    if (end.0 - start.0).hypot(end.1 - start.1) * scale > 16.0 {
        return false;
    }
    let double_area: f64 = trace.windows(2).map(|edge| {
        let (ax, ay) = (edge[0].0 - start.0, edge[0].1 - start.1);
        let (bx, by) = (edge[1].0 - start.0, edge[1].1 - start.1);
        ax * by - ay * bx
    }).sum();
    double_area.abs() * scale * scale >= 128.0
}

#[cfg(test)]
mod raise_trace_tests {
    use super::closed_raise_trace;

    #[test]
    fn accepts_either_direction_and_rejects_open_or_flat_traces() {
        let clockwise = [(20.0, 20.0), (100.0, 20.0), (100.0, 100.0), (20.0, 100.0)];
        let counterclockwise = [clockwise[0], clockwise[3], clockwise[2], clockwise[1]];
        assert!(closed_raise_trace(&clockwise, (21.0, 19.0), 0.5));
        assert!(closed_raise_trace(&counterclockwise, (21.0, 19.0), 0.5));
        assert!(!closed_raise_trace(&clockwise, (20.0, 100.0), 0.5));
        assert!(!closed_raise_trace(&[(0.0, 0.0), (30.0, 0.0), (60.0, 0.0)], (0.0, 0.0), 1.0));
    }
}

pub struct MapStudio {
    /// Next map click sets where the last mark moves to.
    set_motion: bool,
    /// Pen and Area marks measure (length / area).
    measure: bool,
    /// Show recent places under the search box.
    show_history: bool,
    /// GIS tools: layers A and B, buffer distance, export projection, last results.
    gis_ab: (usize, usize),
    gis_miles: f64,
    gis_projection: crate::gis::Projection,
    gis_result: Vec<String>,
    app: Entity<AppState>,
    frames: Entity<FrameStore>,
    slot: Rc<ImageSlot>,
    pub tool: Tool,
    tab: LeftTab,
    front_kind: AnnotationKind,
    color: String,
    width: f32,
    drag: Option<Drag>,
    /// Where the pointer went down (frame px), to tell a click from a drag.
    press: Option<(f64, f64)>,
    title_input: Entity<InputState>,
    text_input: Entity<InputState>,
    sponsor_input: Entity<InputState>,
    /// Right-hand side panel (conditions, alerts, cells, feeds) shown.
    info_open: bool,
    layers_open: bool,
    exporting_map: bool,
    open_menu: Option<MapMenu>,
    forecast_picker: Option<ForecastPicker>,
    /// Category open in the Forecast picker.
    forecast_group: FieldGroup,
    overlay_slot: usize,
    raise_state: bool,
    /// Point inspector card (click the map with Pan).
    inspect: Option<crate::ui::map_inspect::Inspect>,
    inspect_serial: u64,
    /// Latitude / longitude under the mouse, shown on the map.
    cursor_text: Option<String>,
    /// Online lookup results (ZIP codes, counties) for the current search.
    online_places: Vec<(String, f64, f64)>,
    online_serial: u64,
    /// "Go to a place" search box and its current text.
    place_input: Entity<InputState>,
    /// "Find anything" above the map: products, layers, tools and menus by name.
    find_input: Entity<InputState>,
    find_query: String,
    /// Open dropdown in the toolbars, and the one an outside click just closed.
    open_dd: Option<Dd>,
    /// Open drop-down menu (menu bar "mb-…" or a ribbon button "rb-…"), and its open submenu.
    menu_open: Option<&'static str>,
    menu_sub: Option<usize>,
    menu_closed: Option<(&'static str, std::time::Instant)>,
    /// Ribbon tab showing, and whether the ribbon is unfolded.
    ribbon: RibbonTab,
    ribbon_open: bool,
    /// Horizontal scroll of the ribbon body (narrow windows): drives the ‹ › edge buttons.
    ribbon_scroll: ScrollHandle,
    /// Window height at the last render: long drop-down menus scroll instead of running off it.
    win_h: f32,
    place_query: String,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl MapStudio {
    pub fn new(app: Entity<AppState>, frames: Entity<FrameStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let live = &app.read(cx).live;
        let title = live.banner_text().into_owned();
        let title_input = cx.new(|cx| InputState::new(window, cx).placeholder("Custom banner text").default_value(title));
        let place_input = cx.new(|cx| InputState::new(window, cx).placeholder("Find a town or lat, lon"));
        let find_input = cx.new(|cx| InputState::new(window, cx).placeholder("Find anything: snow, NAM, lightning…"));
        let text_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Label for Text / Marker")
                .default_value("Heavy rain")
        });
        let sponsor = app.read(cx).live.storm_track.as_ref().map(|t| t.sponsor.clone()).unwrap_or_default();
        let sponsor_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Sponsor line (optional)")
                .default_value(sponsor)
        });

        let mut subscriptions = vec![
            cx.observe_in(&app, window, |this, app, window, cx| {
                let live = &app.read(cx).live;
                let title = live.banner_text().into_owned();
                if this.title_input.read(cx).value().to_string() != title {
                    this.title_input.update(cx, |input, cx| input.set_value(title, window, cx));
                }
                cx.notify();
            }),
            cx.observe(&frames, |_, _, cx| cx.notify()),
        ];
        subscriptions.push(cx.subscribe_in(&find_input, window, |this, state, event: &InputEvent, window, cx| {
            match event {
                InputEvent::Change => {
                    this.find_query = state.read(cx).value().to_string();
                    cx.notify();
                }
                // Enter takes the first match.
                InputEvent::PressEnter { .. } => {
                    if let Some((_, _, action)) = this.find_hits().into_iter().next() {
                        this.run_find(action, window, cx);
                    }
                }
                _ => {}
            }
        }));
        subscriptions.push(cx.subscribe_in(&place_input, window, |this, state, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.place_query = state.read(cx).value().to_string();
                this.online_places.clear();
                // ZIP codes, counties and places missing from the town list: look them up online
                // after a short pause in typing.
                let q = this.place_query.trim().to_string();
                let zip = q.len() == 5 && q.chars().all(|c| c.is_ascii_digit());
                let wants_online = zip || q.to_lowercase().contains("county") || (q.len() >= 4 && crate::places::search(&q, 1).is_empty() && crate::places::parse_lat_lon(&q).is_none());
                if wants_online {
                    this.online_serial += 1;
                    let serial = this.online_serial;
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(std::time::Duration::from_millis(400)).await;
                        let still = this.update(cx, |t, _| t.online_serial == serial).unwrap_or(false);
                        if !still {
                            return;
                        }
                        let result = crate::net::spawn(crate::net::geocode(q)).await;
                        let _ = this.update(cx, |t, cx| {
                            if t.online_serial == serial {
                                t.online_places = match result {
                                    Ok(Ok(v)) => v,
                                    _ => Vec::new(),
                                };
                                cx.notify();
                            }
                        });
                    })
                    .detach();
                }
                cx.notify();
            }
        }));
        subscriptions.push(cx.subscribe_in(&title_input, window, |this, state, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                let value = state.read(cx).value().to_string();
                this.app.update(cx, |s, cx| {
                    s.update_live(|live| {
                        live.title = value;
                        live.custom_title = true;
                    });
                    cx.notify();
                });
            }
        }));
        subscriptions.push(cx.subscribe_in(&sponsor_input, window, |this, state, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                let value = state.read(cx).value().to_string();
                this.app.update(cx, |s, _| {
                    s.update_live(|live| {
                        if let Some(track) = &mut live.storm_track {
                            track.sponsor = value;
                        }
                    })
                });
            }
        }));

        Self {
            app,
            frames,
            slot: ImageSlot::new(),
            tool: Tool::Pan,
            set_motion: false,
            measure: false,
            show_history: false,
            gis_ab: (0, 1),
            gis_miles: 10.0,
            gis_projection: crate::gis::Projection::Wgs84,
            gis_result: Vec::new(),
            tab: LeftTab::Radar,
            front_kind: AnnotationKind::ColdFront,
            color: PEN_COLORS[0].to_string(),
            width: 6.0,
            drag: None,
            press: None,
            title_input,
            text_input,
            sponsor_input,
            info_open: true,
            layers_open: true,
            exporting_map: false,
            open_menu: None,
            forecast_picker: None,
            forecast_group: FieldGroup::Precipitation,
            overlay_slot: 0,
            raise_state: false,
            inspect: None,
            inspect_serial: 0,
            place_input,
            find_input,
            find_query: String::new(),
            open_dd: None,
            menu_open: None,
            menu_sub: None,
            menu_closed: None,
            ribbon: RibbonTab::Home,
            ribbon_open: true,
            ribbon_scroll: ScrollHandle::new(),
            win_h: 900.,
            place_query: String::new(),
            cursor_text: None,
            online_places: Vec::new(),
            online_serial: 0,
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    fn data_mode(&self, cx: &App) -> bool {
        self.app.read(cx).live.data.field.is_some()
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if [&self.title_input, &self.place_input, &self.text_input, &self.sponsor_input, &self.find_input].iter()
            .any(|input| input.read(cx).focus_handle(cx).is_focused(window))
        {
            return;
        }
        if event.keystroke.modifiers.control || event.keystroke.modifiers.alt {
            return;
        }
        let data_mode = self.data_mode(cx);
        let key = event.keystroke.key.as_str();
        match key {
            "space" => self.app.update(cx, |s, cx| {
                if data_mode {
                    if crate::program::forecast_span(&s.live, &s.data) > 0 && crate::program::forecast_ready(&s.live, &s.data) {
                        s.data_playing = !s.data_playing;
                    }
                } else {
                    s.playing = !s.playing;
                    s.live.radar.mode = RadarMode::Loop;
                    s.sync_map();
                }
                cx.notify();
            }),
            "left" => self.app.update(cx, |s, cx| {
                if data_mode {
                    let h = (s.live.data.hour - 1.0).max(0.0).round();
                    s.set_data_hour(h);
                } else {
                    s.playing = false;
                    s.step_radar(-1);
                }
                cx.notify();
            }),
            "right" => self.app.update(cx, |s, cx| {
                if data_mode {
                    let h = (s.live.data.hour + 1.0).round();
                    s.set_data_hour(h);
                } else {
                    s.playing = false;
                    s.step_radar(1);
                }
                cx.notify();
            }),
            "enter" if matches!(self.drag, Some(Drag::Raise { tracing: false, .. })) => {
                self.finish_raise(None, cx);
            }
            "escape" if matches!(self.drag, Some(Drag::Raise { .. })) => {
                self.cancel_raise(cx);
            }
            "escape" if self.menu_open.is_some() => {
                self.menu_open = None;
                self.menu_sub = None;
                cx.notify();
            }
            "escape" if self.open_dd.is_some() => {
                self.open_dd = None;
                cx.notify();
            }
            "escape" if self.open_menu == Some(MapMenu::Forecast) && self.forecast_picker.is_some() => {
                self.forecast_picker = None;
                cx.notify();
            }
            "escape" if self.open_menu.is_some() => {
                self.open_menu = None;
                cx.notify();
            }
            "escape" => {
                self.set_tool(Tool::Pan, cx);
                self.app.update(cx, |s, _| s.select_alert(None));
            }
            "backspace" | "delete" => self.app.update(cx, |s, _| {
                s.update_live(|l| {
                    l.annotations.pop();
                })
            }),
            "home" => {
                self.cancel_raise(cx);
                self.app.update(cx, |s, _| s.fly_to(s.settings.home, 1.8));
            }
            other => {
                if let Some(tool) = Tool::all().find(|t| t.key().eq_ignore_ascii_case(other)) {
                    self.set_tool(tool, cx);
                }
            }
        }
    }

    pub fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        self.cancel_raise(cx);
        self.tool = tool;
        if matches!(tool, Tool::RaisedArea | Tool::RaisedBoundary) {
            self.set_motion = false;
            self.ribbon = RibbonTab::Draw;
            self.ribbon_open = true;
        }
        cx.notify();
    }

    fn cancel_raise(&mut self, cx: &mut Context<Self>) {
        if matches!(self.drag, Some(Drag::Raise { .. })) {
            self.drag = None;
            self.press = None;
            cx.notify();
        }
    }

    fn primary_raise_point(viewport: &Viewport, layout: PanelLayout, wipe: f32, point: (f64, f64)) -> bool {
        let width = if layout == PanelLayout::Wipe { (viewport.width * f64::from(wipe.clamp(0.02, 0.98))).round() } else { viewport.width };
        point.0 < width && point.1 < viewport.height
    }

    /// The viewport of the panel under frame point (fx, fy), and the point in that panel.
    /// In the single layout this is the whole frame.
    fn locate(&self, fx: f64, fy: f64, cx: &App) -> (Viewport, f64, f64) {
        let live = &self.app.read(cx).live;
        let (_, lx, ly) = live.layout.locate(fx, fy);
        let (pw, ph) = live.layout.panel_size();
        (Viewport::new(live.view, pw, ph), lx, ly)
    }

    // ---- Pointer -----------------------------------------------------------------------------

    /// The draggable on-air element under a frame point, with its current offset.
    fn element_at(&self, fx: f64, fy: f64, cx: &App) -> Option<(&'static str, (f32, f32))> {
        let state = self.app.read(cx);
        let live = &state.live;
        let mut boxes = crate::map::overlays::onair_boxes(
            live, FRAME_W as f32, FRAME_H as f32,
            state.settings.brand.logo_media.is_some(), state.current_sponsor().is_some(),
        );
        // Test graphic layers in the same stacking order as the compositor.
        for (key, id) in MapScene::OVERLAY_POSITIONS.into_iter()
            .zip(std::iter::once(&live.overlay_graphic).chain(&live.overlay_slots))
        {
            let Some(doc) = id.as_ref().and_then(|id| state.graphics.iter().find(|doc| &doc.id == id)) else { continue };
            let (dx, dy) = live.onair_offsets.get(key).copied().unwrap_or_default();
            for layer in doc.layers.iter().filter(|layer| layer.visible) {
                boxes.push((key, (layer.x + dx, layer.y + dy, layer.w, layer.h)));
            }
        }
        let (x, y) = (fx as f32, fy as f32);
        boxes
            .iter()
            .rev()
            .find(|(_, (bx, by, bw, bh))| x >= *bx && y >= *by && x <= bx + bw && y <= by + bh)
            .map(|(key, _)| (*key, if *key == "watermark" { live.watermark_offset() } else { live.onair_offsets.get(*key).copied().unwrap_or_default() }))
    }

    fn new_annotation(&self, kind: AnnotationKind, lat: f64, lon: f64, points: usize, text: String) -> Annotation {
        Annotation {
            id: new_id(),
            kind,
            points: vec![(lat, lon); points],
            area_rings: Vec::new(),
            color: self.color.clone(),
            width: self.width,
            text,
            fill: matches!(kind, AnnotationKind::Polygon | AnnotationKind::Circle),
            motion: None,
            loop_secs: 0.0,
            reveal: false,
            show_from: None,
            show_until: None,
            measure: self.measure && matches!(kind, AnnotationKind::Pen | AnnotationKind::Polygon),
            elevation_ft: None,
        }
    }

    fn on_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_raise(cx);
        window.focus(&self.focus, cx);
        let Some((fx, fy)) = self.slot.to_frame(event.position) else {
            return;
        };
        self.press = Some((fx, fy));
        let (vp, px_, py_) = self.locate(fx, fy, cx);
        let (lat, lon) = vp.to_geo(px_, py_);
        let text = self.text_input.read(cx).value().to_string();

        // "Set movement" mode: this click is where the last mark travels to.
        if self.set_motion && self.app.read(cx).live.annotations.last().is_some_and(|a| a.kind != AnnotationKind::RaisedArea) {
            self.set_motion = false;
            self.press = None;
            self.app.update(cx, |s, cx| {
                s.update_live(|l| {
                    if let Some(a) = l.annotations.last_mut() {
                        let n = a.points.len().max(1) as f64;
                        let (clat, clon) = a.points.iter().fold((0.0, 0.0), |acc, p| (acc.0 + p.0 / n, acc.1 + p.1 / n));
                        a.motion = Some((lat - clat, lon - clon));
                    }
                });
                s.toast(crate::app::ToastKind::Success, "Movement set: the mark now travels there on a loop");
                cx.notify();
            });
            return;
        }

        match self.tool {
            Tool::RaisedArea | Tool::RaisedBoundary => {
                let (viewport, layout, wipe) = {
                    let state = self.app.read(cx);
                    let map = &state.live;
                    let view = state.editor_engine().latest().map_or(map.view, |frame| frame.view);
                    let (width, height) = map.layout.panel_size();
                    (Viewport::new(view, width, height), map.layout, map.wipe)
                };
                if !Self::primary_raise_point(&viewport, layout, wipe, (fx, fy)) {
                    self.app.update(cx, |s, cx| {
                        s.toast(ToastKind::Info, "Draw or select the area on the first map panel.");
                        cx.notify();
                    });
                    return;
                }
                self.app.update(cx, |s, _| s.interrupt_fly());
                if self.tool == Tool::RaisedArea {
                    self.drag = Some(Drag::Raise { viewport, layout, wipe, tracing: true, trace: vec![(fx, fy)] });
                    cx.notify();
                    return;
                }
                let (lat, lon) = viewport.to_geo(fx, fy);
                let Some(region) = crate::map::boundaries::region_at(lat, lon, self.raise_state) else {
                    self.app.update(cx, |s, cx| {
                        s.toast(ToastKind::Info, "No loaded boundary here. Click inside a US county or state, or use Raise area.");
                        cx.notify();
                    });
                    return;
                };
                let mut mark = self.new_annotation(AnnotationKind::RaisedArea, lat, lon, 0, String::new());
                mark.text = region.name;
                mark.area_rings = region.rings;
                self.app.update(cx, |s, _| s.update_live(|map| {
                    map.raise_visible = true;
                    map.annotations.push(mark);
                }));
            }
            Tool::Pan => {
                if event.click_count >= 2 {
                    let view = vp.zoomed_at(1.0, px_, py_);
                    self.app.update(cx, |s, _| s.fly_to(view, 0.45));
                } else {
                    // Grabbing the map stops a camera move (it can be resumed).
                    self.app.update(cx, |s, _| s.interrupt_fly());
                    self.drag = Some(Drag::Pan {
                        start: (fx, fy),
                        view: vp.view,
                    });
                }
            }
            Tool::Pen | Tool::Area | Tool::Arrow | Tool::Circle | Tool::Spotlight | Tool::Ruler | Tool::Front => {
                let kind = match self.tool {
                    Tool::Pen => AnnotationKind::Pen,
                    Tool::Area => AnnotationKind::Polygon,
                    Tool::Arrow => AnnotationKind::Arrow,
                    Tool::Spotlight => AnnotationKind::Spotlight,
                    Tool::Ruler => AnnotationKind::Ruler,
                    Tool::Front => self.front_kind,
                    _ => AnnotationKind::Circle,
                };
                let annotation = self.new_annotation(kind, lat, lon, 2, String::new());
                let id = annotation.id.clone();
                self.app.update(cx, |s, _| s.update_live(|l| l.annotations.push(annotation)));
                self.drag = Some(Drag::Draw { id });
            }
            Tool::Text if text.trim().is_empty() => {
                self.app.update(cx, |s, cx| {
                    s.toast(ToastKind::Info, "Type the label first (Label box under the ribbon, or on the Draw tab), then click the map.");
                    cx.notify();
                });
            }
            Tool::Text | Tool::Marker => {
                let kind = if self.tool == Tool::Text { AnnotationKind::Text } else { AnnotationKind::Marker };
                let annotation = self.new_annotation(kind, lat, lon, 1, text);
                self.app.update(cx, |s, _| s.update_live(|l| l.annotations.push(annotation)));
            }
            Tool::High | Tool::Low => {
                let kind = if self.tool == Tool::High { AnnotationKind::High } else { AnnotationKind::Low };
                // Model pressure at the spot, when the data grid is loaded.
                let value = {
                    let state = self.app.read(cx);
                    state
                        .data
                        .grid
                        .as_ref()
                        .and_then(|g| g.value(DataField::Pressure, g.time_index(state.live.data.hour), lat, lon))
                        .map(|p| format!("{p:.0} mb"))
                        .unwrap_or_default()
                };
                let annotation = self.new_annotation(kind, lat, lon, 1, value);
                self.app.update(cx, |s, _| s.update_live(|l| l.annotations.push(annotation)));
            }
            Tool::StormTrack => {
                self.app.update(cx, |s, _| {
                    s.update_live(|l| {
                        let mut track = l.storm_track.clone().unwrap_or_default();
                        track.lat = lat;
                        track.lon = lon;
                        track.path.clear();
                        track.placed_at = chrono::Utc::now().timestamp();
                        l.storm_track = Some(track);
                    })
                });
                self.drag = Some(Drag::Track);
            }
            Tool::FreeTrack => {
                self.app.update(cx, |s, _| {
                    s.update_live(|l| {
                        let mut track = l.storm_track.clone().unwrap_or_default();
                        track.lat = lat;
                        track.lon = lon;
                        track.path.clear();
                        track.placed_at = chrono::Utc::now().timestamp();
                        l.storm_track = Some(track);
                    })
                });
                self.drag = Some(Drag::FreeTrack);
            }
            Tool::Arrange => {
                if let Some((key, orig)) = self.element_at(fx, fy, cx) {
                    self.drag = Some(Drag::Element { key, start: (fx, fy), orig });
                }
            }
            Tool::Rings => {
                self.app.update(cx, |s, _| {
                    s.update_live(|l| {
                        let mut rings = l.range_rings.clone().unwrap_or_default();
                        rings.lat = lat;
                        rings.lon = lon;
                        l.range_rings = Some(rings);
                    })
                });
            }
        }
        cx.notify();
    }

    fn on_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        // The left press starts a trace and mouse-up ends it. Move events may
        // arrive without button metadata or briefly cross a comparison divider.
        if let Some(Drag::Raise { viewport, layout, wipe, tracing: true, trace }) = &mut self.drag {
            if let Some(next) = self.slot.to_frame(event.position).filter(|&point| Self::primary_raise_point(viewport, *layout, *wipe, point)) {
                let last = trace.last().expect("trace starts at mouse down");
                let scale = f64::from(self.slot.scale().max(0.01));
                if (next.0 - last.0).hypot(next.1 - last.1) * scale >= 2.0 {
                    trace.push(next);
                    cx.notify();
                }
            }
            return;
        }
        if event.pressed_button != Some(MouseButton::Left) {
            if !matches!(self.drag, Some(Drag::Raise { .. })) { self.drag = None; }
            // Coordinate readout (bottom-left of the map).
            if let Some((fx, fy)) = self.slot.to_frame(event.position) {
                let (vp, lx, ly) = self.locate(fx, fy, cx);
                let (lat, lon) = vp.to_geo(lx, ly);
                let home = self.app.read(cx).settings.home;
                let mi = crate::map::geo::haversine_km(home.lat, home.lon, lat, lon) / crate::map::geo::KM_PER_MILE;
                let brg = crate::map::geo::bearing_deg(home.lat, home.lon, lat, lon);
                self.cursor_text = Some(format!(
                    "{lat:.3}, {lon:.3}  •  {mi:.0} mi {} of home",
                    crate::map::geo::compass(brg)
                ));
                cx.notify();
            }
            return;
        }
        let scale = f64::from(self.slot.scale().max(0.01));
        let (fx, fy) = self.slot.to_frame_clamped(event.position);
        match &self.drag {
            Some(Drag::Pan { start, view }) => {
                // Pan deltas are in frame pixels at 1:1 scale, the same in every panel size.
                let vp = Viewport::new(*view, FRAME_W, FRAME_H);
                let new_view = vp.panned(fx - start.0, fy - start.1);
                self.app.update(cx, |s, _| s.set_view(new_view));
            }
            Some(Drag::Draw { id }) => {
                let id = id.clone();
                let (vp, lx, ly) = self.locate(fx, fy, cx);
                let (lat, lon) = vp.to_geo(lx, ly);
                self.app.update(cx, |s, _| {
                    s.update_live(|l| {
                        if let Some(a) = l.annotations.iter_mut().find(|a| a.id == id) {
                            let freehand = matches!(a.kind, AnnotationKind::Pen | AnnotationKind::Polygon) || a.kind.is_front();
                            if freehand {
                                // Add a point once the pointer moved a few screen pixels.
                                let (px, py) = a.points.last().map(|(la, lo)| vp.to_screen(*la, *lo)).unwrap_or((0.0, 0.0));
                                let moved = ((lx as f32 - px).powi(2) + (ly as f32 - py).powi(2)).sqrt();
                                if moved as f64 * scale > 3.0 {
                                    a.points.push((lat, lon));
                                }
                            } else if a.points.len() >= 2 {
                                let last = a.points.len() - 1;
                                a.points[last] = (lat, lon);
                            }
                        }
                    })
                });
            }
            Some(Drag::Track) => {
                let (vp, lx, ly) = self.locate(fx, fy, cx);
                let (lat, lon) = vp.to_geo(lx, ly);
                self.app.update(cx, |s, _| {
                    s.update_live(|l| {
                        if let Some(track) = &mut l.storm_track {
                            let km = haversine_km(track.lat, track.lon, lat, lon);
                            if km > 2.0 {
                                track.heading_deg = bearing_deg(track.lat, track.lon, lat, lon);
                                // Dragged distance = one hour of travel.
                                track.speed_mph = (km / KM_PER_MILE).clamp(5.0, 95.0).round();
                            }
                        }
                    })
                });
            }
            Some(Drag::Element { key, start, orig }) => {
                let key = key.to_string();
                let (dx, dy) = ((fx - start.0) as f32 + orig.0, (fy - start.1) as f32 + orig.1);
                self.app.update(cx, |s, _| {
                    s.update_live(|l| {
                        l.onair_offsets.insert(key, (dx.round(), dy.round()));
                    })
                });
            }
            Some(Drag::FreeTrack) => {
                let (vp, lx, ly) = self.locate(fx, fy, cx);
                let (lat, lon) = vp.to_geo(lx, ly);
                self.app.update(cx, |s, _| {
                    s.update_live(|l| {
                        if let Some(track) = &mut l.storm_track {
                            let (px, py) = track
                                .path
                                .last()
                                .map(|(la, lo)| vp.to_screen(*la, *lo))
                                .unwrap_or_else(|| vp.to_screen(track.lat, track.lon));
                            let moved = ((lx as f32 - px).powi(2) + (ly as f32 - py).powi(2)).sqrt();
                            if moved as f64 * scale > 8.0 {
                                track.path.push((lat, lon));
                                if track.path.len() >= 2 {
                                    let first = track.path[0];
                                    track.heading_deg = bearing_deg(track.lat, track.lon, first.0, first.1);
                                }
                            }
                        }
                    })
                });
            }
            Some(Drag::Raise { .. }) | None => {}
        }
        cx.notify();
    }

    fn finish_raise(&mut self, position: Option<Point<Pixels>>, cx: &mut Context<Self>) {
        let Some(Drag::Raise { viewport, layout, wipe, mut trace, .. }) = self.drag.take() else { return };
        self.press = None;
        cx.notify();

        let map = &self.app.read(cx).live;
        if map.layout != layout || (layout == PanelLayout::Wipe && map.wipe != wipe) {
            return;
        }
        let scale = f64::from(self.slot.scale());
        let Some(&start) = trace.first() else { return };
        let end = position.and_then(|position| self.slot.to_frame(position))
            .filter(|&point| Self::primary_raise_point(&viewport, layout, wipe, point));
        if let Some(point) = end.filter(|point| trace.last() != Some(point)) {
            trace.push(point);
        }
        if !closed_raise_trace(&trace, start, scale) {
            self.app.update(cx, |s, cx| {
                s.toast(ToastKind::Info, "Draw around an area with enough room inside the outline.");
                cx.notify();
            });
            return;
        }
        if position.is_some() && !end.is_some_and(|end| closed_raise_trace(&trace, end, scale)) {
            self.drag = Some(Drag::Raise { viewport, layout, wipe, tracing: false, trace });
            return;
        }
        let mut mark = self.new_annotation(AnnotationKind::RaisedArea, 0.0, 0.0, 0, String::new());
        mark.points = trace.into_iter().map(|(x, y)| viewport.to_geo(x, y)).collect();
        self.app.update(cx, |s, _| s.update_live(|map| {
                    map.raise_visible = true;
                    map.annotations.push(mark);
                }));
    }

    fn on_up(&mut self, event: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.drag, Some(Drag::Raise { tracing: true, .. })) {
            self.finish_raise(Some(event.position), cx);
            return;
        }
        let was_pan = matches!(self.drag, Some(Drag::Pan { .. }));
        // A finished ruler looks up the ground elevation at both ends.
        if let Some(Drag::Draw { id }) = &self.drag {
            let id = id.clone();
            let ends = self.app.read(cx).live.annotations.iter().find(|a| a.id == id && a.kind == AnnotationKind::Ruler && a.points.len() >= 2).map(|a| vec![a.points[0], a.points[a.points.len() - 1]]);
            if let Some(points) = ends {
                cx.spawn(async move |this, cx| {
                    let result = crate::net::spawn(crate::map::geo::elevations_ft(points)).await;
                    if let Ok(Ok(e)) = result {
                        if e.len() >= 2 {
                            let _ = this.update(cx, |this, cx| {
                                this.app.update(cx, |s, _| {
                                    s.update_live(|l| {
                                        if let Some(a) = l.annotations.iter_mut().find(|a| a.id == id) {
                                            a.elevation_ft = Some((e[0], e[1]));
                                        }
                                    })
                                });
                                cx.notify();
                            });
                        }
                    }
                })
                .detach();
            }
        }
        self.drag = None;
        let press = self.press.take();
        // A click (not a drag) with the Pan tool calls out the warning under the pointer.
        if let (true, Some((x0, y0)), Some((fx, fy))) = (was_pan, press, self.slot.to_frame(event.position)) {
            let moved = ((fx - x0).powi(2) + (fy - y0).powi(2)).sqrt() * self.slot.scale().max(0.01) as f64;
            if moved < 4.0 {
                let (vp, lx, ly) = self.locate(fx, fy, cx);
                let (lat, lon) = vp.to_geo(lx, ly);
                if let Some((var, delta, radius)) = self.app.read(cx).ext.paint {
                    self.app.update(cx, |s, cx| {
                        s.paint_adjustment(lat, lon, var, delta, radius);
                        s.toast(crate::app::ToastKind::Info, format!("Painted {} {delta:+.0} (draft; publish in the Forecast editor)", var.label()));
                        cx.notify();
                    });
                    return;
                }
                self.open_inspect(lat, lon, cx);
                self.app.update(cx, |s, cx| {
                    let mut hits: Vec<&crate::data::Alert> = s.data.map_alerts.iter().filter(|a| a.contains(lat, lon)).collect();
                    hits.sort_by_key(|a| a.priority());
                    let hit = hits.first().map(|a| (a.id.clone(), a.event.clone()));
                    match hit {
                        Some((id, event)) => {
                            let same = s.live.selected_alert.as_deref() == Some(id.as_str());
                            s.select_alert(if same { None } else { Some(id) });
                            if !same {
                                s.toast(ToastKind::Info, format!("Calling out: {event} (Esc or click it again to clear)"));
                            }
                        }
                        None => {
                            if s.live.selected_alert.is_some() {
                                s.select_alert(None);
                            }
                        }
                    }
                    cx.notify();
                });
            }
        }
        cx.notify();
    }

    /// Open the point inspector and fetch current conditions for the spot.
    fn open_inspect(&mut self, lat: f64, lon: f64, cx: &mut Context<Self>) {
        self.inspect_serial += 1;
        let serial = self.inspect_serial;
        let (alerts, api) = {
            let s = self.app.read(cx);
            (s.data.map_alerts.clone(), s.settings.api_base.clone())
        };
        self.inspect = Some(crate::ui::map_inspect::Inspect::at(lat, lon, &alerts, serial));
        {
            // A click on (or near) a storm cell shows that storm in the card.
            let s = self.app.read(cx);
            if s.live.storm_cells {
                let km_per_px = 40_075.0 * lat.to_radians().cos() / (256.0 * 2f64.powf(s.live.view.zoom));
                let reach = (km_per_px * 40.0).clamp(3.0, 40.0);
                let cell = s
                    .data
                    .cells
                    .iter()
                    .map(|c| (crate::map::geo::haversine_km(lat, lon, c.lat, c.lon), c))
                    .filter(|(km, _)| *km <= reach)
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, c)| c.clone());
                if let Some(card) = self.inspect.as_mut() {
                    card.cell = cell;
                }
            }
        }
        cx.spawn(async move |this, cx| {
            let result = crate::net::spawn(crate::map::geo::elevations_ft(vec![(lat, lon)])).await;
            let _ = this.update(cx, |this, cx| {
                if let (Some(card), Ok(Ok(e))) = (this.inspect.as_mut().filter(|c| c.serial == serial), result) {
                    card.elevation_ft = e.first().copied();
                    cx.notify();
                }
            });
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let result = crate::net::spawn(crate::net::overview(api, lat, lon)).await;
            let _ = this.update(cx, |this, cx| {
                let Some(card) = this.inspect.as_mut().filter(|c| c.serial == serial) else { return };
                card.loading = false;
                match result {
                    Ok(Ok(o)) => card.current = Some(o.current),
                    Ok(Err(e)) => card.error = Some(e.to_string()),
                    Err(e) => card.error = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn on_scroll(&mut self, event: &ScrollEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.drag, Some(Drag::Raise { tracing: true, .. })) { return; }
        self.cancel_raise(cx);
        // A dialog (Settings, About...) sits over the map: its scrolling must not zoom the map.
        if self.app.read(cx).modal != crate::app::Modal::None {
            return;
        }
        let Some((fx, fy)) = self.slot.to_frame(event.position) else {
            return;
        };
        let lines = match event.delta {
            ScrollDelta::Lines(p) => p.y,
            ScrollDelta::Pixels(p) => f32::from(p.y) / 60.0,
        };
        if lines.abs() < 0.001 {
            return;
        }
        let (vp, lx, ly) = self.locate(fx, fy, cx);
        let view = vp.zoomed_at((lines * 0.35) as f64, lx, ly);
        self.app.update(cx, |s, _| s.set_view(view));
        cx.notify();
    }

    // ---- Left panel --------------------------------------------------------------------------

    fn left_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        // The ribbon picks what the side panel shows; the panel just names it.
        let title = match self.tab {
            LeftTab::Radar => "Radar settings",
            LeftTab::Satellite => "Satellite settings",
            LeftTab::Data => "Forecast settings",
            LeftTab::Severe => "Warnings and outlooks",
            LeftTab::OnAir => "On-air settings",
            LeftTab::Layout => "Compare panels",
        };
        let tabs = row()
            .w_full()
            .flex_shrink_0()
            .h(px(30.))
            .px(px(10.))
            .border_b_1()
            .border_color(c(BORDER))
            .child(div().flex_1().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(title))
            .child(div().id("side-close").px(px(4.)).rounded(px(4.)).cursor_pointer().hover(|s| s.bg(c(PANEL_2))).child(crate::ui::icons::ri("close-line", 14., MUTED)).on_click(cx.listener(|this, _, _, cx| {
                this.layers_open = false;
                cx.notify();
            })));

        let body: AnyElement = match self.tab {
            LeftTab::Radar => self.radar_tab(cx).into_any_element(),
            LeftTab::Satellite => self.satellite_tab(cx).into_any_element(),
            LeftTab::Data => self.data_tab(cx).into_any_element(),
            LeftTab::Severe => self.severe_tab(cx).into_any_element(),
            LeftTab::OnAir => self.onair_tab(cx).into_any_element(),
            LeftTab::Layout => self.layout_tab(cx).into_any_element(),
        };

        col()
            .w(px(310.))
            .h_full()
            .flex_shrink_0()
            .bg(c(PANEL))
            .border_r_1()
            .border_color(c(BORDER))
            .child(tabs)
            .child(div().id("map-left-body").flex_1().min_h_0().overflow_y_scroll().child(body))
    }

    fn radar_tab(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let live = state.live.clone();
        let loop_ms = state.settings.loop_frame_ms as f32;
        let site_mode = live.radar.source == RadarSource::Site;
        let resolved = live.radar.resolved_site(live.view.lat, live.view.lon);

        let mut basemaps = Basemap::ALL
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let b = *b;
                chip(("basemap", i), b.label(), live.basemap == b && live.theme.is_empty()).on_click(cx.listener(move |this, _, _, cx| {
                    this.app.update(cx, |s, _| s.update_live(|l| {
                        l.basemap = b;
                        l.theme.clear();
                    }));
                }))
            })
            .collect::<Vec<_>>();
        // Custom map themes (basemap designer).
        for (i, t) in crate::maptheme::list().into_iter().enumerate() {
            let name = t.name.clone();
            basemaps.push(chip(("map-theme", i), t.name.clone(), live.theme == t.name).on_click(cx.listener(move |this, _, _, cx| {
                let n = name.clone();
                this.app.update(cx, |s, _| s.update_live(|l| l.theme = n));
            })));
        }
        basemaps.push(chip("map-theme-edit", "Design a map theme…", false).on_click(cx.listener(|this, _, _, cx| {
            this.app.update(cx, |s, cx| {
                s.modal = crate::app::Modal::Themes;
                cx.notify();
            });
        })));

        let sources = [(RadarSource::Composite, "Composite loop"), (RadarSource::Site, "Single site (NEXRAD)")]
            .into_iter()
            .enumerate()
            .map(|(i, (source, text))| {
                chip(("radar-src", i), text, live.radar.source == source).on_click(cx.listener(move |this, _, _, cx| {
                    this.app.update(cx, |s, _| {
                        s.update_live(|l| {
                            l.radar.source = source;
                            l.radar.enabled = true;
                        })
                    });
                }))
            })
            .collect::<Vec<_>>();

        let palettes = RADAR_PALETTES
            .iter()
            .enumerate()
            .map(|(i, (code, name))| {
                let code = code.to_string();
                let active = live.radar.palette == code;
                chip(("palette", i), *name, active).on_click(cx.listener(move |this, _, _, cx| {
                    let code = code.clone();
                    this.app.update(cx, |s, _| s.update_live(|l| l.radar.palette = code));
                }))
            })
            .collect::<Vec<_>>();

        let products = SiteProduct::ALL
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let p = *p;
                chip(("site-prod", i), p.label(), live.radar.product == p).on_click(cx.listener(move |this, _, _, cx| {
                    this.app.update(cx, |s, _| s.update_live(|l| l.radar.product = p));
                }))
            })
            .collect::<Vec<_>>();

        // Sites nearest the map centre, plus Auto.
        let mut near: Vec<(&str, &str, f64)> = NEXRAD_SITES
            .iter()
            .map(|(id, name, lat, lon)| (*id, *name, haversine_km(live.view.lat, live.view.lon, *lat, *lon)))
            .collect();
        near.sort_by(|a, b| a.2.total_cmp(&b.2));
        let mut site_chips = vec![chip("site-auto", format!("Auto (K{resolved})"), live.radar.site.eq_ignore_ascii_case("AUTO"))
            .on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, _| s.update_live(|l| l.radar.site = "AUTO".into()));
            }))];
        for (i, (id, name, km)) in near.into_iter().take(9).enumerate() {
            let id_owned = id.to_string();
            site_chips.push(
                chip(("site", i), format!("K{id} {} • {:.0} mi", name.split_whitespace().next().unwrap_or(""), km / KM_PER_MILE), live.radar.site.eq_ignore_ascii_case(id))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let id = id_owned.clone();
                        this.app.update(cx, |s, _| s.update_live(|l| l.radar.site = id));
                    })),
            );
        }

        let totals = std::iter::once(None)
            .chain(PrecipWindow::ALL.iter().copied().map(Some))
            .enumerate()
            .map(|(i, w)| {
                let text = w.map(|w| w.label()).unwrap_or("Off");
                chip(("mrms", i), text, live.precip_total == w).on_click(cx.listener(move |this, _, _, cx| {
                    this.app.update(cx, |s, _| s.update_live(|l| l.precip_total = w));
                }))
            })
            .collect::<Vec<_>>();

        col()
            .child(
                section("Map")
                    .child(row().flex_wrap().gap(px(6.)).children(basemaps))
                    .child(toggle("labels", "Place labels", live.labels).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.labels = !l.labels));
                    })))
                    .child(toggle("counties", "County outlines", live.counties).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.counties = !l.counties));
                    })))
                    .child(toggle("states", "State outlines", live.states).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.states = !l.states));
                    })))
                    .when(live.counties || live.states, |d| {
                        d.child(row().justify_between().child(muted("Outline opacity")).child(muted(format!("{:.0}%", live.boundary_opacity * 100.0))))
                            .child(slider("boundary-opacity", live.boundary_opacity, 0.1, 1.0, cx, |this, v, _, cx| {
                                this.app.update(cx, |s, _| s.update_live(|l| l.boundary_opacity = v));
                            }))
                    }),
            )
            .child(
                section("Radar")
                    .child(toggle("radar-on", "Show radar", live.radar.enabled).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.radar.enabled = !l.radar.enabled));
                    })))
                    .child(row().flex_wrap().gap(px(6.)).children(sources))
                    .child(row().justify_between().child(muted("Opacity")).child(muted(format!("{:.0}%", live.radar.opacity * 100.0))))
                    .child(slider("radar-opacity", live.radar.opacity, 0.1, 1.0, cx, |this, v, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.radar.opacity = v));
                    })),
            )
            .when(!site_mode, |d| {
                d.child(
                    section("Composite • LibreWXR via OpenWX Relay")
                        .child(muted("Palette"))
                        .child(row().flex_wrap().gap(px(6.)).children(palettes))
                        .child(toggle("smooth", "Smoothing", live.radar.smoothing).on_click(cx.listener(|this, _, _, cx| {
                            this.app.update(cx, |s, _| s.update_live(|l| l.radar.smoothing = !l.radar.smoothing));
                        })))
                        .child(toggle("nowcast", "Future radar (nowcast)", live.radar.include_nowcast).on_click(cx.listener(|this, _, _, cx| {
                            this.app.update(cx, |s, _| s.update_live(|l| l.radar.include_nowcast = !l.radar.include_nowcast));
                        })))
                        .child(
                            row()
                                .gap(px(6.))
                                .child(chip("mode-loop", "Loop", live.radar.mode == RadarMode::Loop).on_click(cx.listener(|this, _, _, cx| {
                                    this.app.update(cx, |s, _| s.update_live(|l| l.radar.mode = RadarMode::Loop));
                                })))
                                .child(chip("mode-latest", "Latest", live.radar.mode == RadarMode::Latest).on_click(cx.listener(|this, _, _, cx| {
                                    this.app.update(cx, |s, _| s.update_live(|l| l.radar.mode = RadarMode::Latest));
                                }))),
                        )
                        .child(row().justify_between().child(muted("Loop speed")).child(muted(format!("{loop_ms:.0} ms/frame"))))
                        .child(slider("loop-speed", 1350.0 - loop_ms, 150.0, 1200.0, cx, |this, v, _, cx| {
                            this.app.update(cx, |s, cx| {
                                s.settings.loop_frame_ms = (1350.0 - v).round() as u64;
                                cx.notify();
                            });
                        })),
                )
            })
            .when(site_mode, |d| {
                d.child(
                    section("Single-site NEXRAD (Level III)")
                        .child(muted("Product"))
                        .child(row().flex_wrap().gap(px(6.)).children(products))
                        .child(muted("Radar site"))
                        .child(row().flex_wrap().gap(px(6.)).children(site_chips))
                        .child(muted("Latest scan from the National Weather Service radar, refreshed with each data update. Velocity: green = toward the radar, red = away. Tight red/green pairs mean rotation.")),
                )
            })
            .child(
                section("Rainfall totals (MRMS)")
                    .child(row().flex_wrap().gap(px(6.)).children(totals))
                    .child(muted("Radar + gauge estimated rainfall over the window.")),
            )
    }

    fn satellite_tab(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let live = self.app.read(cx).live.clone();
        let channels = SatChannel::ALL
            .iter()
            .enumerate()
            .map(|(i, ch)| {
                let ch = *ch;
                chip(("sat-ch", i), ch.label(), live.satellite.channel == ch).on_click(cx.listener(move |this, _, _, cx| {
                    this.app.update(cx, |s, _| {
                        s.update_live(|l| {
                            l.satellite.channel = ch;
                            l.satellite.enabled = true;
                        })
                    });
                }))
            })
            .collect::<Vec<_>>();
        col().child(
            section("Satellite")
                .child(toggle("sat", "Show satellite", live.satellite.enabled).on_click(cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |s, _| s.update_live(|l| l.satellite.enabled = !l.satellite.enabled));
                })))
                .child(row().flex_wrap().gap(px(6.)).children(channels))
                .child(row().justify_between().child(muted("Opacity")).child(muted(format!("{:.0}%", live.satellite.opacity * 100.0))))
                .child(slider("sat-opacity", live.satellite.opacity, 0.1, 1.0, cx, |this, v, _, cx| {
                    this.app.update(cx, |s, _| s.update_live(|l| l.satellite.opacity = v));
                }))
                .child(muted("Infrared comes through your site's proxy. GOES Visible (daytime), Clean IR and Water Vapor are the latest GOES-East images.")),
        )
    }

    fn data_tab(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let live = state.live.clone();
        let health = state.health.grid.clone();
        let model_mode = live.data.field == Some(DataField::Futurecast);
        let loading = state.data.grid.is_none() && live.data.field.is_some() && !model_mode;

        let group = self.forecast_group;
        let models = crate::grid::GridModel::ALL.into_iter().enumerate().map(|(i, option)| {
            chip(("data-model", i), option.label(), live.data.grid_model == option)
                .on_click(cx.listener(move |this, _, _, cx| this.select_forecast_model(option, cx)))
        }).collect::<Vec<_>>();
        let groups = FieldGroup::ALL.into_iter().enumerate().map(|(i, option)| {
            chip(("data-group", i), option.label(), group == option).on_click(cx.listener(move |this, _, _, cx| {
                this.forecast_group = option;
                cx.notify();
            }))
        }).collect::<Vec<_>>();
        let fields = DataField::ALL.iter().copied().enumerate()
            .filter(|(_, f)| f.group() == group && *f != DataField::Futurecast)
            .map(|(i, f)| {
                chip(("field", i), f.viewer_label(), live.data.field == Some(f))
                    .on_click(cx.listener(move |this, _, _, cx| this.select_data_field(Some(f), cx)))
            })
            .collect::<Vec<_>>();

        col()
            .child(
                section("Forecast map")
                    .child(muted("Model"))
                    .child(row().flex_wrap().gap(px(5.)).children(models))
                    .child(muted("Category"))
                    .child(row().flex_wrap().gap(px(5.)).children(groups))
                    .child(row().flex_wrap().gap(px(5.)).p(px(6.)).rounded(px(6.)).bg(c(PANEL_2)).children(fields))
                    .when(live.data.field.is_some(), |d| d.child(button_small("data-off", "Turn off forecast map").on_click(cx.listener(|this, _, _, cx| this.select_data_field(None, cx)))))
                    .child(muted("NOAA radar-style products (GFS, HRRR, NBM) are in the Forecast menu above the map. Range is on the timeline."))
                    .child(row().justify_between().child(muted("Opacity")).child(muted(format!("{:.0}%", live.data.opacity * 100.0))))
                    .child(slider("data-opacity", live.data.opacity, 0.2, 1.0, cx, |this, v, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.data.opacity = v));
                    })),
            )
            .child(
                section("On the forecast map")
                    .child(toggle("barbs", "Wind barbs", live.data.wind_barbs).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.data.wind_barbs = !l.data.wind_barbs));
                    })))
                    .child(toggle("isobars", "Isobars with H / L", live.data.isobars).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.data.isobars = !l.data.isobars));
                    })))
                    .child(toggle("city-plots", "City value plots", live.data.city_plots).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.data.city_plots = !l.data.city_plots));
                    })))
                    .child(toggle("particles", "Animated wind flow", live.data.wind_particles).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.data.wind_particles = !l.data.wind_particles));
                    })))
                    .child(toggle("impacts", "Impact labels at the worst spots", live.impact_callouts).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.impact_callouts = !l.impact_callouts));
                    })))
                ,
            )
            .child(
                section("Hazards and events")
                    .child(toggle("urban-flood", "Urban flooding risk (towns, next 3 h)", live.urban_flood).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.urban_flood = !l.urban_flood));
                    })))
                    .child(toggle("eoc-layer", "Emergency management (shelters, zones, closures)", live.eoc).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.eoc = !l.eoc));
                    })))
                    .child(toggle("nowcast-layer", "Storm nowcast", live.nowcast).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.nowcast = !l.nowcast));
                    })))
                    .when(live.nowcast, |d| {
                        d.child(row().gap(px(4.)).flex_wrap().children([("arrival", "Arrival chance (1 h)"), ("cones", "Motion uncertainty"), ("initiation", "New storms"), ("threats", "Hail / wind / rain"), ("verify", "Nowcast vs actual")].into_iter().enumerate().map(|(i, (k, t))| {
                            chip(("nowcast-mode", i), t, live.nowcast_mode == k).on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, _| s.update_live(|l| l.nowcast_mode = k.into()));
                            }))
                        }).collect::<Vec<_>>()))
                    })
                    .child(toggle("outage-layer", "Power outages by county", live.outages).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.outages = !l.outages));
                    })))
                    .when(live.outages, |d| {
                        d.child(row().gap(px(4.)).children([("now", "Customers out"), ("restoration", "Restoration progress"), ("infrastructure", "Critical facilities")].into_iter().enumerate().map(|(i, (k, t))| {
                            chip(("outage-mode", i), t, live.outage_mode == k).on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, _| s.update_live(|l| l.outage_mode = k.into()));
                            }))
                        }).collect::<Vec<_>>()))
                    })
                    .child(toggle("viewer-reports", "Viewer reports (approved)", live.viewer_reports).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.viewer_reports = !l.viewer_reports));
                    })))
                    .child(toggle("tor-hist", "Tornado history (SPC, 1950 on)", live.tornado_history).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.tornado_history = !l.tornado_history));
                        crate::net::spawn(async {
                            if let Err(e) = crate::history::ensure_tornado_db().await {
                                log::warn!("{e}");
                            }
                        });
                    })))
                    .when(live.tornado_history, |d| {
                        d.child(row().gap(px(4.)).children([1950, 1980, 2000, 2010, 2020].into_iter().enumerate().map(|(i, y)| {
                            chip(("tor-since", i), format!("Since {y}"), live.tornado_since == y).on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, _| s.update_live(|l| l.tornado_since = y));
                            }))
                        }).collect::<Vec<_>>()))
                    })
                    .child(button_small("open-history", "Event history…").on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.modal = crate::app::Modal::History;
                            cx.notify();
                        });
                    })))
                    .child(toggle("tropical", "Tropical (NHC)", live.tropical).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.tropical = !l.tropical));
                    })))
                    .when(live.tropical, |d| {
                        d.child(row().flex_wrap().gap(px(4.)).children([("cone", "Cone"), ("wind_prob", "Wind chances"), ("arrival", "Wind arrival"), ("surge", "Storm surge"), ("models", "Spaghetti models")].into_iter().enumerate().map(|(i, (key, text))| {
                            chip(("trop", i), text, live.tropical_layer == key).on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, _| s.update_live(|l| l.tropical_layer = key.to_string()));
                            }))
                        }).collect::<Vec<_>>()))
                    })
                    .child(toggle("fire-outlook", "Fire weather outlook (SPC)", live.fire_outlook).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.fire_outlook = !l.fire_outlook));
                    })))
                    .child(toggle("wildfires", "Wildfire perimeters", live.wildfires).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.wildfires = !l.wildfires));
                    })))
                ,
            )
            .child(self.drought_section(cx))
            .child(self.quake_section(cx))
            .child(
                section("Ensemble at towns")
                    .child(row().flex_wrap().gap(px(4.)).children([("", "Off"), ("mean_high", "Avg high"), ("spread_high", "Spread"), ("p10_snow", "Low-end snow"), ("p90_snow", "High-end snow"), ("prob_snow1", "1\" snow chance"), ("prob_rain1", "1\" rain chance"), ("prob_freeze", "Freeze chance")].into_iter().enumerate().map(|(i, (key, text))| {
                        chip(("ensmap", i), text, live.ens_map == key).on_click(cx.listener(move |this, _, _, cx| {
                            this.app.update(cx, |s, _| s.update_live(|l| l.ens_map = key.to_string()));
                        }))
                    }).collect::<Vec<_>>()))
                    .child(toggle("snowband", "Heaviest snow band track (12 h)", live.snow_band).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.snow_band = !l.snow_band));
                    }))),
            )
            .child(
                section("Forecast editor")
                    .child(muted("Your forecast on top of the model: day overrides, painted adjustments, regions, scenarios."))
                    .child(toggle("show-regions", "Show forecast regions on the map", live.regions).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.regions = !l.regions));
                    })))
                    .child(button_small("open-fcst-edit", "Open the Forecast editor").on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.ext.paint = None;
                            s.modal = crate::app::Modal::ForecastEditor;
                            cx.notify();
                        });
                    }))),
            )
            .child(
                section("Model data")
                    .child(row().gap(px(6.)).child(div().w(px(8.)).h(px(8.)).rounded(px(4.)).bg(c(if health.ok { GREEN } else if loading { AMBER } else { MUTED }))).child(muted(if health.message.is_empty() {
                        if loading { "Loading…".to_string() } else { "Loads when a data layer is on".to_string() }
                    } else {
                        health.message.clone()
                    })))
                    .child(muted(if crate::grid::grid_from_noaa(live.data.grid_model, live.view) {
                        format!("{} read directly from NOAA's model files on a grid over this view. It reloads when you move far, change model, or after an hour.", live.data.grid_model.label())
                    } else {
                        format!("{} point forecasts (Open-Meteo) on a grid over this view. It reloads when you move far, change model, or after an hour.", live.data.grid_model.label())
                    })),
            )
    }

    /// Earthquakes (USGS): map layer settings, the latest quakes near home and the biggest
    /// anywhere this week. Clicking a quake flies the map there with the layer on.
    fn quake_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let live = state.live.clone();
        let home = state.settings.home;
        let now = chrono::Utc::now().timestamp();
        let quakes = state.data.quakes.clone();
        let near: Vec<crate::quakes::Quake> = crate::quakes::select(&quakes, now, 30, 0.0, Some((home.lat, home.lon, 350.0))).into_iter().take(5).cloned().collect();
        let mut biggest: Vec<crate::quakes::Quake> = crate::quakes::select(&quakes, now, 7, 4.5, None).into_iter().cloned().collect();
        biggest.sort_by(|a, b| b.mag.total_cmp(&a.mag));
        biggest.truncate(3);
        let days_chips = [(1u32, "Day"), (7, "Week"), (30, "Month")].into_iter().enumerate().map(|(i, (days, text))| {
            chip(("quake-days", i), text, live.quake_days == days).on_click(cx.listener(move |this, _, _, cx| {
                this.app.update(cx, |s, _| s.update_live(|l| {
                    l.quakes = true;
                    l.quake_days = days;
                }));
            }))
        }).collect::<Vec<_>>();
        let mag_chips = [(0.0f32, "All"), (2.5, "2.5+"), (4.5, "4.5+"), (6.0, "6+")].into_iter().enumerate().map(|(i, (m, text))| {
            chip(("quake-min", i), text, (live.quake_min - m).abs() < 0.01).on_click(cx.listener(move |this, _, _, cx| {
                this.app.update(cx, |s, _| s.update_live(|l| {
                    l.quakes = true;
                    l.quake_min = m;
                }));
            }))
        }).collect::<Vec<_>>();
        let row_for = |id: (&'static str, usize), q: crate::quakes::Quake, distance: bool, cx: &mut Context<Self>| {
            let (r, g, b) = crate::quakes::age_rgb(q.age_secs(now));
            let detail = if distance {
                format!("{} · {:.0} mi away · {}", crate::quakes::age_label(q.age_secs(now)), q.miles_from(home.lat, home.lon), q.place)
            } else {
                format!("{} · {}", crate::quakes::age_label(q.age_secs(now)), q.place)
            };
            row().id(id).gap(px(8.)).py(px(4.)).px(px(6.)).rounded(px(6.)).cursor_pointer().hover(|s| s.bg(c(PANEL_2)))
                .child(div().w(px(10.)).h(px(10.)).flex_shrink_0().rounded(px(5.)).bg(c(((r as u32) << 16) | ((g as u32) << 8) | b as u32)))
                .child(div().w(px(46.)).flex_shrink_0().text_size(px(13.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).child(format!("M{:.1}", q.mag)))
                .child(div().flex_1().min_w_0().text_size(px(12.)).text_color(c(MUTED)).truncate().child(detail))
                .on_click(cx.listener(move |this, _, _, cx| {
                    let (lat, lon, mag) = (q.lat, q.lon, q.mag);
                    this.app.update(cx, |s, _| {
                        s.fly_to(crate::config::GeoView { lat, lon, zoom: if mag >= 6.0 { 5.0 } else { 6.5 } }, 1.4);
                        s.update_live(|l| {
                            l.quakes = true;
                            if l.quake_min > mag {
                                l.quake_min = 0.0;
                            }
                            if l.quake_days < 30 {
                                l.quake_days = 30;
                            }
                        });
                    });
                }))
        };
        let near_rows = near.into_iter().enumerate().map(|(i, q)| row_for(("quake-near", i), q, true, cx).into_any_element()).collect::<Vec<_>>();
        let big_rows = biggest.into_iter().enumerate().map(|(i, q)| row_for(("quake-big", i), q, false, cx).into_any_element()).collect::<Vec<_>>();
        let loaded = !quakes.is_empty();
        section("Earthquakes")
            .child(toggle("quakes", "Show earthquakes on the map", live.quakes).on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, _| s.update_live(|l| l.quakes = !l.quakes));
            })))
            .child(row().flex_wrap().gap(px(4.)).child(muted("Period")).children(days_chips))
            .child(row().flex_wrap().gap(px(4.)).child(muted("Size")).children(mag_chips))
            .child(div().pt(px(6.)).text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).child("Near home (350 mi, 30 days)"))
            .when(!loaded, |d| d.child(muted("Loading earthquakes from USGS…")))
            .when(loaded && near_rows.is_empty(), |d| d.child(muted("None in the past 30 days.")))
            .children(near_rows)
            .child(div().pt(px(6.)).text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).child("Biggest this week (M4.5+, anywhere)"))
            .when(loaded && big_rows.is_empty(), |d| d.child(muted("None this week.")))
            .children(big_rows)
            .child(muted("USGS Earthquake Hazards Program, updated every 5 minutes. Click a quake to fly there. Turn the radar off to show the earthquake legend on air."))
    }

    /// Drought: the US Drought Monitor map (this week) and NOAA CPC's drought outlooks for next
    /// month and the season, with what each says at home and quick views.
    fn drought_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let live = state.live.clone();
        let hz = &state.data.hazards;
        let home = state.settings.home;
        let valid = crate::hazards::drought_valid_date(chrono::Local::now().date_naive());
        let here = crate::hazards::drought_label(crate::hazards::drought_at(&hz.drought, home.lat, home.lon));
        let month = crate::hazards::drought_trend_at(&hz.drought_month, home.lat, home.lon);
        let season = crate::hazards::drought_trend_at(&hz.drought_season, home.lat, home.lon);
        let (loaded_now, loaded_month, loaded_season) = (!hz.drought.is_empty(), !hz.drought_month.is_empty() || !hz.drought_month_target.is_empty(), !hz.drought_season.is_empty() || !hz.drought_season_target.is_empty());
        let month_target = if hz.drought_month_target.is_empty() { "next month".to_string() } else { hz.drought_month_target.clone() };
        let season_target = if hz.drought_season_target.is_empty() { "the season".to_string() } else { format!("through {}", hz.drought_season_target) };
        let issued = hz.drought_outlook_issued.clone();
        let views = [("now", "This week"), ("month", "Next month"), ("season", "Season")];
        let view_chips = views.into_iter().enumerate().map(|(i, (key, text))| {
            chip(("drought-view", i), text, live.drought && live.drought_view == key).on_click(cx.listener(move |this, _, _, cx| {
                this.app.update(cx, |s, _| s.update_live(|l| {
                    l.drought = true;
                    l.drought_view = key.to_string();
                }));
            }))
        }).collect::<Vec<_>>();
        let legend: Vec<(String, (u8, u8, u8))> = if live.drought_view == "now" {
            ["D0 Abnormally dry", "D1 Moderate", "D2 Severe", "D3 Extreme", "D4 Exceptional"].iter().enumerate()
                .map(|(i, t)| (t.to_string(), crate::map::extras::drought_rgb(i as u8))).collect()
        } else {
            [(1u8, "Continues"), (2, "Improves"), (3, "Ends"), (4, "Develops")].iter()
                .map(|(code, t)| (t.to_string(), crate::map::extras::drought_trend_rgb(*code))).collect()
        };
        let legend_row = row().flex_wrap().gap(px(8.)).children(legend.into_iter().map(|(text, (r, g, b))| {
            row().gap(px(4.))
                .child(div().w(px(12.)).h(px(12.)).rounded(px(3.)).bg(c(((r as u32) << 16) | ((g as u32) << 8) | b as u32)))
                .child(muted(text))
        }).collect::<Vec<_>>());
        let views_for = [("Home", 7.0), ("Region", 5.6), ("U.S.", 3.6)];
        let zoom_chips = views_for.into_iter().enumerate().map(|(i, (text, zoom))| {
            chip(("drought-zoom", i), text, false).on_click(cx.listener(move |this, _, _, cx| {
                this.app.update(cx, |s, _| {
                    let home = s.settings.home;
                    let to = if zoom < 4.0 { crate::config::GeoView { lat: 39.0, lon: -96.5, zoom } } else { crate::config::GeoView { lat: home.lat, lon: home.lon, zoom } };
                    s.fly_to(to, 1.2);
                    s.update_live(|l| l.drought = true);
                });
            }))
        }).collect::<Vec<_>>();
        section("Drought")
            .child(toggle("drought", "Show drought on the map", live.drought).on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, _| s.update_live(|l| l.drought = !l.drought));
            })))
            .child(row().flex_wrap().gap(px(4.)).children(view_chips))
            .child(legend_row)
            .child(kv("Here now", if loaded_now { here.to_string() } else { "Loading the Drought Monitor…".into() }))
            .child(kv("Map valid", valid.format("%a %b %-d (new maps Thursdays)").to_string()))
            .child(kv(format!("Outlook, {month_target}"), if loaded_month { month.to_string() } else { "Loading…".into() }))
            .child(kv(format!("Outlook, {season_target}"), if loaded_season { season.to_string() } else { "Loading…".into() }))
            .child(row().flex_wrap().gap(px(4.)).child(muted("View")).children(zoom_chips))
            .child(muted(if issued.is_empty() {
                "U.S. Drought Monitor (NDMC, USDA, NOAA) and NOAA Climate Prediction Center outlooks.".to_string()
            } else {
                format!("U.S. Drought Monitor (NDMC, USDA, NOAA); outlooks from NOAA CPC issued {issued}.")
            }))
    }

    fn severe_tab(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let live_clock_24h = self.app.read(cx).settings.clock_24h;
        let (state_callout_l3, state_return) = { let s = &self.app.read(cx).settings; (s.callout_lower_third, s.return_after_warning) };
        let state = self.app.read(cx);
        let live = state.live.clone();
        let reports = self.app.read(cx).data.reports.clone();
        let mut warnings: Vec<crate::data::Alert> = state.data.map_alerts.iter().filter(|a| a.is_warning()).cloned().collect();
        warnings.sort_by_key(|a| a.priority());
        let view = live.view;
        warnings.sort_by(|a, b| {
            let da = a.centroid().map(|(la, lo)| haversine_km(view.lat, view.lon, la, lo)).unwrap_or(f64::MAX);
            let db = b.centroid().map(|(la, lo)| haversine_km(view.lat, view.lon, la, lo)).unwrap_or(f64::MAX);
            a.priority().cmp(&b.priority()).then(da.total_cmp(&db))
        });

        // Days 1-2 have separate tornado / hail / wind chances; day 3 has one combined chance.
        let layer_options: &[(&str, &str)] = if live.spc_day == Some(3) {
            &[("categorical", "Risk levels"), ("severe", "Severe chance")]
        } else {
            &[("categorical", "Risk levels"), ("tornado", "Tornado"), ("hail", "Hail"), ("wind", "Wind"), ("significant", "Significant severe")]
        };
        let spc_layers = layer_options
            .iter()
            .enumerate()
            .map(|(i, (key, text))| {
                let key = key.to_string();
                let on = live.spc_layer == key || (live.spc_layer.is_empty() && key == "categorical");
                chip(("spc-layer", i), *text, on).on_click(cx.listener(move |this, _, _, cx| {
                    let key = key.clone();
                    this.app.update(cx, |s, _| s.update_live(|l| l.spc_layer = key));
                }))
            })
            .collect::<Vec<_>>();
        let spc_days = [None, Some(1u8), Some(2), Some(3)]
            .into_iter()
            .enumerate()
            .map(|(i, day)| {
                let text = match day {
                    None => "Off".to_string(),
                    Some(d) => format!("Day {d}"),
                };
                chip(("spc", i), text, live.spc_day == day).on_click(cx.listener(move |this, _, _, cx| {
                    this.app.update(cx, |s, _| s.update_live(|l| l.spc_day = day));
                }))
            })
            .collect::<Vec<_>>();

        let callouts = warnings
            .into_iter()
            .take(10)
            .enumerate()
            .map(|(i, a)| {
                let active = live.selected_alert.as_deref() == Some(a.id.as_str());
                let id = a.id.clone();
                let center = a.centroid();
                let color = hex_color(a.color());
                let ends = a.expires_label(live_clock_24h).unwrap_or_default();
                row()
                    .id(("callout", i))
                    .gap(px(8.))
                    .p(px(6.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .bg(if active { c(0x163A47) } else { c(PANEL_2) })
                    .border_l_4()
                    .border_color(color)
                    .hover(|s| s.bg(c(0x232C40)))
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).child(a.event.clone()))
                            .child(div().text_size(px(11.)).text_color(c(MUTED)).truncate().child(a.area_desc.clone())),
                    )
                    .child(div().text_size(px(11.)).text_color(c(AMBER)).flex_shrink_0().child(ends))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let id = id.clone();
                        this.app.update(cx, |s, cx| {
                            if active {
                                s.select_alert(None);
                            } else {
                                s.select_alert(Some(id));
                                if let Some((lat, lon)) = center {
                                    let zoom = s.live.view.zoom.max(8.0);
                                    s.fly_to(crate::config::GeoView { lat, lon, zoom }, 1.4);
                                }
                            }
                            cx.notify();
                        });
                    }))
            })
            .collect::<Vec<_>>();

        col()
            .child(
                section("Warnings & watches")
                    .child(toggle("warn", "NWS warnings & watches", live.warnings).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.warnings = !l.warnings));
                    })))
                    .child(toggle("warn-fill", "Fill polygons", live.warnings_fill).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.warnings_fill = !l.warnings_fill));
                    })))
                    .child(toggle("warn-pattern", "Patterns by type (tornado ✕, severe ╱, flood ═, winter dots)", live.warning_patterns).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.warning_patterns = !l.warning_patterns));
                    })))
                    .child(muted("Show these alert types"))
                    .child(
                        row()
                            .flex_wrap()
                            .gap(px(6.))
                            .child(chip("ak-warn", "Warnings", live.alert_kinds.warnings).on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, _| s.update_live(|l| l.alert_kinds.warnings = !l.alert_kinds.warnings));
                            })))
                            .child(chip("ak-watch", "Watches", live.alert_kinds.watches).on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, _| s.update_live(|l| l.alert_kinds.watches = !l.alert_kinds.watches));
                            })))
                            .child(chip("ak-adv", "Advisories", live.alert_kinds.advisories).on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, _| s.update_live(|l| l.alert_kinds.advisories = !l.alert_kinds.advisories));
                            })))
                            .child(chip("ak-stmt", "Statements", live.alert_kinds.statements).on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, _| s.update_live(|l| l.alert_kinds.statements = !l.alert_kinds.statements));
                            }))),
                    )
                    .child(toggle("warn-count", "Countdown tags (TOR • 14 MIN)", live.warning_countdowns).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.warning_countdowns = !l.warning_countdowns));
                    })))
                    .child(muted("SPC convective outlook"))
                    .child(row().gap(px(6.)).children(spc_days))
                    .when(live.spc_day.is_some(), |d| d.child(row().gap(px(6.)).flex_wrap().children(spc_layers)))
                    .child(toggle("spc-md", "Mesoscale discussions", live.mesoscale).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.mesoscale = !l.mesoscale));
                    })))
                    .child(toggle("warn-towns", "Name the towns in a called-out warning", live.warning_towns).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.warning_towns = !l.warning_towns));
                    }))),
            )
            .child(
                section("Rain, flooding and rivers")
                    .child(muted("WPC rainfall forecast"))
                    .child(row().gap(px(6.)).flex_wrap().children(
                        [None, Some(crate::feeds::QpfPeriod::Day1), Some(crate::feeds::QpfPeriod::Days1to3), Some(crate::feeds::QpfPeriod::Days1to7)]
                            .into_iter()
                            .enumerate()
                            .map(|(i, p)| {
                                chip(("qpf", i), p.map(|p| p.label()).unwrap_or("Off"), live.qpf == p).on_click(cx.listener(move |this, _, _, cx| {
                                    this.app.update(cx, |s, _| s.update_live(|l| l.qpf = p));
                                }))
                            })
                            .collect::<Vec<_>>(),
                    ))
                    .child(muted("Excessive rainfall outlook"))
                    .child(row().gap(px(6.)).flex_wrap().children(
                        [None, Some(1u8), Some(2), Some(3)]
                            .into_iter()
                            .enumerate()
                            .map(|(i, d)| {
                                let text = d.map(|d| format!("Day {d}")).unwrap_or_else(|| "Off".into());
                                chip(("ero", i), text, live.ero_day == d).on_click(cx.listener(move |this, _, _, cx| {
                                    this.app.update(cx, |s, _| s.update_live(|l| l.ero_day = d));
                                }))
                            })
                            .collect::<Vec<_>>(),
                    ))
                    .child(toggle("rivers", "River gauges (flood category)", live.rivers).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.rivers = !l.rivers));
                    })))
                    .child(toggle("centers", "Warming and cooling centers", live.centers).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.centers = !l.centers));
                    })))
                    .child(toggle("town-labels", "Town labels (crisp, size by zoom)", live.town_labels).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.town_labels = !l.town_labels));
                    })))
                    .when(live.town_labels, |d| {
                        let style_now = live.label_style.clone();
                        let scale = live.label_scale;
                        d.child(row().flex_wrap().gap(px(4.)).children([("classic", "Classic"), ("bold", "Bold broadcast"), ("minimal", "Minimal"), ("dark", "Dark on light")].into_iter().enumerate().map(|(i, (key, text))| {
                            chip(("lbl-style", i), text, style_now == key).on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, _| s.update_live(|l| l.label_style = key.to_string()));
                            }))
                        }).collect::<Vec<_>>()))
                        .child(row().flex_wrap().gap(px(4.)).children([0.85f32, 1.0, 1.2, 1.4].into_iter().enumerate().map(|(i, v)| {
                            chip(("lbl-scale", i), format!("{:.0}%", v * 100.0), (scale - v).abs() < 0.01).on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, _| s.update_live(|l| l.label_scale = v));
                            }))
                        }).collect::<Vec<_>>()))
                        .child(muted("Type a town in the text box (top bar), then:"))
                        .child(row().flex_wrap().gap(px(4.)).children([("Hide", 0.0f32, 0.0f32, 1u8), ("Always show", 0.0, 0.0, 2), ("◀", -12.0, 0.0, 0), ("▶", 12.0, 0.0, 0), ("▲", 0.0, -12.0, 0), ("▼", 0.0, 12.0, 0), ("Reset", 0.0, 0.0, 3)].into_iter().enumerate().map(|(i, (text, dx, dy, mode))| {
                            button_small(("lbl-ovr", i), text).on_click(cx.listener(move |this, _, _, cx| {
                                let name = this.text_input.read(cx).value().trim().to_string();
                                if name.is_empty() {
                                    return;
                                }
                                this.app.update(cx, |s, _| {
                                    s.update_live(|l| {
                                        if mode == 3 {
                                            l.label_overrides.retain(|o| !o.name.eq_ignore_ascii_case(&name));
                                            return;
                                        }
                                        if !l.label_overrides.iter().any(|o| o.name.eq_ignore_ascii_case(&name)) {
                                            l.label_overrides.push(crate::map::scene::LabelOverride { name: name.clone(), ..Default::default() });
                                        }
                                        if let Some(o) = l.label_overrides.iter_mut().find(|o| o.name.eq_ignore_ascii_case(&name)) {
                                            match mode {
                                                1 => o.hidden = !o.hidden,
                                                2 => o.always = !o.always,
                                                _ => {
                                                    o.dx += dx;
                                                    o.dy += dy;
                                                }
                                            }
                                        }
                                    })
                                });
                            }))
                        }).collect::<Vec<_>>()))
                    })
                    .child(toggle("highways", "Highway shields", live.highways).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.highways = !l.highways));
                    })))
                    .child(toggle("tracks", "Storm tracks (ids, trends, lightning jumps)", live.storm_tracks).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| {
                            s.update_live(|l| l.storm_tracks = !l.storm_tracks);
                            if s.live.storm_tracks && !s.data.analysis.tracks.iter().any(|t| !t.ended && t.points.len() >= 2) {
                                s.toast(ToastKind::Info, "Storm tracks build up from each radar update: paths appear after a few minutes.");
                            }
                        });
                    })))
                    .child(toggle("lclusters", "Lightning clusters with motion", live.lightning_clusters).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.lightning_clusters = !l.lightning_clusters));
                    })))
                    .child(toggle("lanim", "Lightning density, last hour animated", live.lightning_anim).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.lightning_anim = !l.lightning_anim));
                    })))
                    .child(toggle("mcs", "Storm complexes and squall lines", live.mcs).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.mcs = !l.mcs));
                    })))
                    .child(toggle("traffic", "Traffic: slowdowns, crashes, closures", live.traffic).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.traffic = !l.traffic));
                    })))
                    .child(toggle("cameras", "Camera locations", live.cameras).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.cameras = !l.cameras));
                    })))
                    .child(toggle("flood-map", "Flood extent and flooded roads", live.flood_map).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.flood_map = !l.flood_map));
                    })))
                    .child(toggle("damage-map", "Storm damage reports (post-storm survey)", live.damage_map).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.damage_map = !l.damage_map));
                    })))
                    .when(live.damage_map, |d| {
                        d.child(row().gap(px(5.)).children([24u32, 48, 72].into_iter().enumerate().map(|(i, h)| {
                            chip(("dmg-h", i), format!("{h} hr"), live.damage_hours == h).on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, _| s.update_live(|l| l.damage_hours = h));
                            }))
                        }).collect::<Vec<_>>()))
                    }),
            )
            .child(
                section("Call out a warning on air")
                    .child(muted("Click one (or click its polygon on the map with Pan) for the pop-up call-out with countdown."))
                    .child(toggle("callout-l3", "Add the warning's lower third with a call-out", state_callout_l3).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.settings.callout_lower_third = !s.settings.callout_lower_third;
                            let _ = s.settings.save();
                            cx.notify();
                        });
                    })))
                    .child(toggle("warning-return", "When warnings end, clear the call-out and leave severe mode", state_return).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.settings.return_after_warning = !s.settings.return_after_warning;
                            let _ = s.settings.save();
                            cx.notify();
                        });
                    })))
                    .when(callouts.is_empty(), |d| d.child(muted("No active warnings on the map.")))
                    .children(callouts)
                    .when(live.selected_alert.is_some(), |d| {
                        d.child(button_small("clear-callout", "Clear call-out").on_click(cx.listener(|this, _, _, cx| {
                            this.app.update(cx, |s, cx| {
                                s.select_alert(None);
                                cx.notify();
                            });
                        })))
                    }),
            )
            .child(
                section("Storms & lightning")
                    .child(toggle("lightning", "Lightning (last 15 min)", live.lightning).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.lightning = !l.lightning));
                    })))
                    .child(toggle("lightning-density", "Lightning density (30 min)", live.lightning_density).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.lightning_density = !l.lightning_density));
                    })))
                    .child(toggle("reports", "NWS storm reports", live.reports).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.reports = !l.reports));
                    })))
                    .when(live.reports, |d| {
                        d.child(row().gap(px(6.)).children([1u32, 3, 6, 12, 24].into_iter().enumerate().map(|(i, h)| {
                            chip(("lsr-h", i), format!("{h} hr"), live.report_hours == h).on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, _| s.update_live(|l| l.report_hours = h));
                            }))
                        }).collect::<Vec<_>>()))
                        .child(row().gap(px(6.)).flex_wrap().children(
                            [("T", "Tornado"), ("H", "Hail"), ("W", "Wind"), ("F", "Flood"), ("S", "Snow / ice"), ("L", "Lightning")]
                                .into_iter()
                                .enumerate()
                                .map(|(i, (letter, text))| {
                                    let on = !live.report_hidden.contains(letter);
                                    chip(("lsr-kind", i), text, on).on_click(cx.listener(move |this, _, _, cx| {
                                        this.app.update(cx, |s, _| {
                                            s.update_live(|l| {
                                                if l.report_hidden.contains(letter) {
                                                    l.report_hidden = l.report_hidden.replace(letter, "");
                                                } else {
                                                    l.report_hidden.push_str(letter);
                                                }
                                            })
                                        });
                                    }))
                                })
                                .collect::<Vec<_>>(),
                        ))
                        .child({
                            let (kinds, counties) = crate::data::report_counts(&reports, live.report_hours, &live.report_hidden);
                            let name = |k: &str| match k {
                                "T" => "tornado",
                                "H" => "hail",
                                "W" => "wind",
                                "F" => "flood",
                                "S" => "snow/ice",
                                "L" => "lightning",
                                _ => "other",
                            };
                            let total: usize = kinds.iter().map(|(_, n)| n).sum();
                            let by_kind = kinds.iter().map(|(k, n)| format!("{n} {}", name(k))).collect::<Vec<_>>().join(", ");
                            let top = counties
                                .iter()
                                .take(3)
                                .map(|(c, n)| format!("{c} ({n})"))
                                .collect::<Vec<_>>()
                                .join(", ");
                            col()
                                .gap(px(2.))
                                .child(muted(if total == 0 {
                                    format!("No reports in the last {} hr", live.report_hours)
                                } else {
                                    format!("{total} reports in {} hr: {by_kind}", live.report_hours)
                                }))
                                .when(!top.is_empty(), |d| d.child(muted(format!("Most reports: {top}"))))
                        })
                    })
                    .child(toggle("roads", "INDOT closures & incidents", live.roads).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.roads = !l.roads));
                    })))
                    .when(live.roads, |d| {
                        d.child(toggle("roads-all", "Include roadwork & restrictions", live.roads_all).on_click(cx.listener(|this, _, _, cx| {
                            this.app.update(cx, |s, _| s.update_live(|l| l.roads_all = !l.roads_all));
                        })))
                    })
                    .child(toggle("cells", "Storm icons", live.storm_cells).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.storm_cells = !l.storm_cells));
                    })))
                    .child(toggle("cell-tracks", "Storm paths (30/60 min)", live.cell_paths).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.cell_paths = !l.cell_paths));
                    }))),
            )
            .child(
                section("Storm track")
                    .child(toggle("track", "Storm track", live.storm_track.is_some()).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| {
                            let center = s.live.view;
                            s.update_live(|l| {
                                l.storm_track = if l.storm_track.is_some() {
                                    None
                                } else {
                                    Some(StormTrack {
                                        lat: center.lat,
                                        lon: center.lon,
                                        ..StormTrack::default()
                                    })
                                }
                            })
                        });
                    })))
                    .child(
                        row()
                            .gap(px(6.))
                            .child(button_small("tool-track", "Draw cone (S)").on_click(cx.listener(|this, _, _, cx| this.set_tool(Tool::StormTrack, cx))))
                            .child(button_small("tool-free", "Draw freeform (F)").on_click(cx.listener(|this, _, _, cx| this.set_tool(Tool::FreeTrack, cx)))),
                    )
                    .when_some(live.storm_track.clone(), |d, track| {
                        let freeform = track.is_freeform();
                        d.child(row().justify_between().child(muted("Storm speed")).child(muted(format!("{:.0} mph", track.speed_mph))))
                            .child(slider("track-speed", track.speed_mph as f32, 5.0, 95.0, cx, |this, v, _, cx| {
                                this.app.update(cx, |s, _| {
                                    s.update_live(|l| {
                                        if let Some(t) = &mut l.storm_track {
                                            t.speed_mph = v.round() as f64;
                                        }
                                    })
                                });
                            }))
                            .when(!freeform, |d| {
                                d.child(row().justify_between().child(muted("Heading")).child(muted(format!("{:.0}°", track.heading_deg))))
                                    .child(slider("track-heading", track.heading_deg as f32, 0.0, 359.0, cx, |this, v, _, cx| {
                                        this.app.update(cx, |s, _| {
                                            s.update_live(|l| {
                                                if let Some(t) = &mut l.storm_track {
                                                    t.heading_deg = v.round() as f64;
                                                }
                                            })
                                        });
                                    }))
                                    .child(row().justify_between().child(muted("Cone width")).child(muted(format!("±{:.0}°", track.cone_deg))))
                                    .child(slider("track-cone", track.cone_deg as f32, 5.0, 45.0, cx, |this, v, _, cx| {
                                        this.app.update(cx, |s, _| {
                                            s.update_live(|l| {
                                                if let Some(t) = &mut l.storm_track {
                                                    t.cone_deg = v.round() as f64;
                                                }
                                            })
                                        });
                                    }))
                            })
                            .when(freeform, |d| {
                                d.child(row().justify_between().child(muted("Corridor half-width")).child(muted(format!("{:.0} mi", track.corridor_mi))))
                                    .child(slider("track-corridor", track.corridor_mi as f32, 1.0, 25.0, cx, |this, v, _, cx| {
                                        this.app.update(cx, |s, _| {
                                            s.update_live(|l| {
                                                if let Some(t) = &mut l.storm_track {
                                                    t.corridor_mi = v.round() as f64;
                                                }
                                            })
                                        });
                                    }))
                            })
                            .child(row().justify_between().child(muted("Time span")).child(muted(format!("{} min", track.minutes))))
                            .child(slider("track-minutes", track.minutes as f32, 15.0, 120.0, cx, |this, v, _, cx| {
                                this.app.update(cx, |s, _| {
                                    s.update_live(|l| {
                                        if let Some(t) = &mut l.storm_track {
                                            t.minutes = ((v / 15.0).round() * 15.0) as u32;
                                        }
                                    })
                                });
                            }))
                            .child(Input::new(&self.sponsor_input))
                    }),
            )
            .child(
                section("Range rings")
                    .child(toggle("rings", "Range rings", live.range_rings.is_some()).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| {
                            let home = s.settings.home;
                            s.update_live(|l| {
                                l.range_rings = if l.range_rings.is_some() {
                                    None
                                } else {
                                    Some(RangeRings {
                                        lat: home.lat,
                                        lon: home.lon,
                                        ..RangeRings::default()
                                    })
                                }
                            })
                        });
                    }))),
            )
    }

    fn onair_drawer(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let live = &self.app.read(cx).live;
        let slot = self.overlay_slot;
        let active = if slot == 0 { live.overlay_graphic.clone() } else { live.overlay_slots[slot - 1].clone() };
        let automatic = live.automatic_title();
        let banner_visible = live.show_title;
        let watermark = live.shows_watermark();
        let sponsor = live.show_sponsor;
        let sponsor_size = live.sponsor_size.unwrap_or(100);
        let station_logo = live.show_logo;
        let has_station_logo = self.app.read(cx).settings.brand.logo_media.is_some();
        let slots = ["Lower third", "Crawl", "Sponsor / bug"].into_iter().enumerate().map(|(index, name)| {
            chip(("overlay-slot", index), name, slot == index).on_click(cx.listener(move |this, _, _, cx| {
                this.overlay_slot = index;
                cx.notify();
            }))
        }).collect::<Vec<_>>();
        col().w_full().gap(px(6.)).px(px(10.)).py(px(6.)).bg(c(PANEL))
            .child(row().gap(px(6.)).flex_wrap().child(label("On-air graphics")).children(slots))
            .child(row().gap(px(6.)).flex_wrap()
                .child(button_small("new-map-lower-third", "+ Lower third").on_click(cx.listener(|this, _, _, cx| {
                    this.overlay_slot = 0;
                    this.add_overlay_template("lower_third", 0, cx);
                })))
                .child(button_small("new-map-sponsor-third", "+ Sponsor lower third").on_click(cx.listener(|this, _, _, cx| {
                    this.overlay_slot = 2;
                    this.add_overlay_template("sponsor_lower_third", 2, cx);
                })))
                .child(button_small("new-map-crawl", "+ Alert crawl").on_click(cx.listener(|this, _, _, cx| {
                    this.overlay_slot = 1;
                    this.add_overlay_template("alert_crawl", 1, cx);
                })))
                .when_some(active, |row, id| row
                    .child(button_small("edit-map-overlay", "Edit active graphic").on_click(cx.listener(move |this, _, _, cx| {
                        this.app.update(cx, |s, cx| { s.open_graphic(id.clone()); cx.notify(); });
                    })))
                    .child(button_small("remove-map-overlay", "Take graphic off").on_click(cx.listener(move |this, _, _, cx| {
                        this.app.update(cx, |s, cx| { s.set_map_overlay_slot(slot, None); cx.notify(); });
                    })))))
            .child(div().id("map-overlay-library").max_h(px(80.)).overflow_y_scroll()
                .child(self.overlay_picker("drawer-overlay", slot, cx)))
            .child(row().items_center().gap(px(6.)).flex_wrap()
                .child(toggle("watermark", "OpenWX watermark", watermark).on_click(cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |s, _| s.update_live(|map| map.watermark_visible = Some(!map.shows_watermark())));
                })))
                .when(has_station_logo, |row| row.child(toggle("station-logo", "Station logo", station_logo).on_click(cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |s, _| s.update_live(|map| {
                        map.watermark_visible = Some(map.shows_watermark());
                        map.show_logo = !map.show_logo;
                    }));
                }))))
                .child(toggle("sponsor-badge", "Sponsor badge", sponsor).on_click(cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |s, cx| {
                        if !s.live.show_sponsor && s.current_sponsor().is_none() {
                            s.modal = crate::app::Modal::Sponsors;
                            cx.notify();
                        } else {
                            s.update_live(|map| map.show_sponsor = !map.show_sponsor);
                        }
                    });
                })))
                .child(button_small("map-sponsor-settings", "Sponsor settings…").on_click(cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |s, cx| { s.modal = crate::app::Modal::Sponsors; cx.notify(); });
                })))
                .child(button_small("map-arrange", "Arrange on map").on_click(cx.listener(|this, _, _, cx| {
                    this.open_menu = None;
                    this.set_tool(Tool::Arrange, cx);
                }))))
            .when(sponsor, |d| d.child(row().items_center().gap(px(6.)).flex_wrap().child(label("Sponsor badge size"))
                .children([(75u8, "Small"), (100, "Medium"), (125, "Large")].into_iter().map(|(size, name)| {
                    chip(("sponsor-badge-size", usize::from(size)), name, sponsor_size == size).on_click(cx.listener(move |this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|map| map.sponsor_size = Some(size)));
                    }))
                }).collect::<Vec<_>>())))
            .child(row().items_center().gap(px(6.)).flex_wrap()
                .child(label("Banner text"))
                .child(div().w(px(220.)).child(Input::new(&self.title_input)))
                .child(chip("banner-visibility", if banner_visible { "Hide banner" } else { "Show banner" }, banner_visible).on_click(cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |s, _| s.update_live(|map| map.show_title = !map.show_title));
                })))
                .child(muted(if automatic { "Automatic title" } else { "Custom title" }))
                .child(button_small("auto-banner", "Reset to automatic").on_click(cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |s, _| s.update_live(|map| {
                        map.title = "LIVE DOPPLER".into();
                        map.custom_title = false;
                    }));
                }))))
    }

    fn add_overlay_template(&mut self, template: &str, slot: usize, cx: &mut Context<Self>) {
        if let Some(doc) = templates::build(template) {
            self.app.update(cx, |s, cx| {
                if let Some(saved) = s.save_graphic(doc) {
                    s.set_map_overlay_slot(slot, Some(saved.id.clone()));
                    s.toast(ToastKind::Success, format!("{} is on the map", saved.name));
                }
                cx.notify();
            });
        }
    }

    fn overlay_picker(&mut self, key: &'static str, slot: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let active = if slot == 0 { state.live.overlay_graphic.clone() } else { state.live.overlay_slots[slot - 1].clone() };
        let overlays = state.graphics.iter().filter(|g| g.is_overlay()).map(|g| (g.id.clone(), g.name.clone())).collect::<Vec<_>>();
        let empty = overlays.is_empty();
        let choices = overlays.into_iter().enumerate().map(|(i, (id, name))| {
            let selected = active.as_deref() == Some(id.as_str());
            chip((key, i), name, selected).on_click(cx.listener(move |this, _, _, cx| {
                this.app.update(cx, |s, cx| {
                    let active = if slot == 0 { &s.live.overlay_graphic } else { &s.live.overlay_slots[slot - 1] };
                    let selected = active.as_deref() == Some(id.as_str());
                    s.set_map_overlay_slot(slot, (!selected).then(|| id.clone()));
                    cx.notify();
                });
            }))
        }).collect::<Vec<_>>();
        row().flex_wrap().gap(px(6.)).children(choices)
            .when(empty, |d| d.child(muted("No saved overlay graphics yet.")))
    }

    fn onair_tab(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let live = state.live.clone();
        let overlay_choices = self.overlay_picker("sidebar-overlay", 0, cx);

        col()
            .child(
                section("On-air elements")
                    .child(button_small("edit-banner", "Banner settings…").on_click(cx.listener(|this, _, _, cx| {
                        this.open_dd = Some(Dd::Title);
                        cx.notify();
                    })))
                    .child(toggle("show-legend", "Legend", live.show_legend).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.show_legend = !l.show_legend));
                    })))
                    .child(toggle("show-time", "Time box", live.show_timestamp).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.show_timestamp = !l.show_timestamp));
                    })))
                    .child(toggle("show-clock", "Clock", live.show_clock).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.show_clock = !l.show_clock));
                    })))
                    .child(muted("Colors and fonts follow Settings → General → Brand & on-air theme."))
                    .child(
                        row()
                            .gap(px(6.))
                            .child(button_small("arrange", "Arrange (E)").on_click(cx.listener(|this, _, _, cx| this.set_tool(Tool::Arrange, cx))))
                            .child(button_small("reset-positions", "Reset positions").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, _| s.update_live(|l| l.onair_offsets.clear()));
                            }))),
                    ),
            )
            .child(
                section("Camera")
                    .child(muted(format!(
                        "Camera path: {} stop(s). Move the map, then Record stop. A map with a path plays it when taken in a show.",
                        live.camera_path.len()
                    )))
                    .child(
                        row()
                            .gap(px(6.))
                            .flex_wrap()
                            .child(button_small("cam-rec", "Record stop").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.record_camera_key();
                                    cx.notify();
                                });
                            })))
                            .child(button_small("cam-play", "Play path").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.play_camera_path();
                                    cx.notify();
                                });
                            })))
                            .child(button_small("cam-stop", "Stop").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.stop_camera_path();
                                    cx.notify();
                                });
                            })))
                            .child(button_small("cam-clear", "Clear path").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, _| s.update_live(|l| l.camera_path.clear()));
                            }))),
                    )
                    .children(live.camera_path.iter().enumerate().map(|(i, key)| {
                        let (fly, hold) = (key.fly, key.hold);
                        row()
                            .gap(px(6.))
                            .child(div().flex_1().text_size(px(12.)).text_color(c(TEXT)).child(format!("Stop {}: {:.2}, {:.2} at zoom {:.1}", i + 1, key.view.lat, key.view.lon, key.view.zoom)))
                            .child(chip(("cam-fly", i), format!("fly {fly:.0}s"), false).on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, _| {
                                    s.update_live(|l| {
                                        if let Some(k) = l.camera_path.get_mut(i) {
                                            k.fly = if k.fly >= 6.0 { 1.0 } else { k.fly + 1.0 };
                                        }
                                    })
                                });
                            })))
                            .child(chip(("cam-hold", i), format!("hold {hold:.0}s"), false).on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, _| {
                                    s.update_live(|l| {
                                        if let Some(k) = l.camera_path.get_mut(i) {
                                            k.hold = if k.hold >= 10.0 { 0.0 } else { k.hold + 2.0 };
                                        }
                                    })
                                });
                            })))
                            .child(button_small(("cam-del", i), "✕").on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, _| {
                                    s.update_live(|l| {
                                        if i < l.camera_path.len() {
                                            l.camera_path.remove(i);
                                        }
                                    })
                                });
                            })))
                    }).collect::<Vec<_>>())
                    .child(
                        row()
                            .gap(px(6.))
                            .flex_wrap()
                            .child(button_small("cam-frame", "Frame home, spots and warnings").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.frame_coverage();
                                    cx.notify();
                                });
                            })))
                            .child(button_small("map-snap", "Save map snapshot").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.save_map_snapshot();
                                    cx.notify();
                                });
                            }))),
                    )
                    .child(toggle("follow-storm", "Follow the storm (called-out warning, else the strongest cell)", live.follow_storm).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| l.follow_storm = !l.follow_storm));
                    }))),
            )
            .child(
                section("Overlay graphic on the map")
                    .child(overlay_choices)
                    .child(button_small("make-l3", "+ New lower third").on_click(cx.listener(|this, _, _, cx| {
                        this.overlay_slot = 0;
                    this.add_overlay_template("lower_third", 0, cx);
                    }))),
            )
    }

    fn layout_tab(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let layers = self.layers_section(cx);
        let places = self.places_section(cx);
        let custom = self.custom_data_section(cx);
        let live = self.app.read(cx).live.clone();
        let layouts = PanelLayout::ALL
            .iter()
            .enumerate()
            .map(|(i, layout)| {
                let layout = *layout;
                chip(("layout", i), layout.label(), live.layout == layout).on_click(cx.listener(move |this, _, _, cx| {
                    this.app.update(cx, |s, cx| {
                        s.update_live(|l| {
                            l.layout = layout;
                            while l.panels.len() < 3 {
                                let next = default_panels()[l.panels.len()].clone();
                                l.panels.push(next);
                            }
                        });
                        cx.notify();
                    });
                }))
            })
            .collect::<Vec<_>>();

        let mut panel_editors = Vec::new();
        for p in 1..live.layout.count() {
            let spec = live.panel_spec(p);
            let products = PanelProduct::ALL
                .iter()
                .enumerate()
                .map(|(j, product)| {
                    let product = *product;
                    chip(("p-prod", p * 100 + j), product.label(), spec.product == product).on_click(cx.listener(move |this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| edit_panel(l, p, |sp| sp.product = product)));
                    }))
                })
                .collect::<Vec<_>>();
            let bases = Basemap::ALL
                .iter()
                .enumerate()
                .map(|(j, b)| {
                    let b = *b;
                    chip(("p-base", p * 10 + j), b.label(), spec.basemap == b).on_click(cx.listener(move |this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|l| edit_panel(l, p, |sp| sp.basemap = b)));
                    }))
                })
                .collect::<Vec<_>>();
            let mut editor = col()
                .gap(px(6.))
                .p(px(8.))
                .rounded(px(6.))
                .bg(c(PANEL_2))
                .child(label(format!("Panel {} • {}", p + 1, live.panel_title(p))))
                .child(row().flex_wrap().gap(px(4.)).children(products));
            if spec.product.uses_radar() {
                let palettes = RADAR_PALETTES
                    .iter()
                    .enumerate()
                    .map(|(j, (code, name))| {
                        let code = code.to_string();
                        chip(("p-pal", p * 100 + j), *name, spec.palette == code).on_click(cx.listener(move |this, _, _, cx| {
                            let code = code.clone();
                            this.app.update(cx, |s, _| s.update_live(|l| edit_panel(l, p, |sp| sp.palette = code)));
                        }))
                    })
                    .collect::<Vec<_>>();
                editor = editor.child(muted("Palette")).child(row().flex_wrap().gap(px(4.)).children(palettes));
            }
            if spec.product == PanelProduct::Outlook {
                let days = (1u8..=3)
                    .map(|d| {
                        chip(("p-day", p * 10 + d as usize), format!("Day {d}"), spec.spc_day == d).on_click(cx.listener(move |this, _, _, cx| {
                            this.app.update(cx, |s, _| s.update_live(|l| edit_panel(l, p, |sp| sp.spc_day = d)));
                        }))
                    })
                    .collect::<Vec<_>>();
                editor = editor.child(row().gap(px(4.)).children(days));
            }
            editor = editor.child(muted("Basemap")).child(row().flex_wrap().gap(px(4.)).children(bases));
            panel_editors.push(editor);
        }

        col().child(
            section("Layout • compare radars")
                .child(row().gap(px(6.)).children(layouts))
                .child(muted("2-Up / 4-Up show different products side by side - e.g. reflectivity next to velocity. Slider lays panel 2 over panel 1 with a divider you move: radar now vs earlier, radar vs Futurecast, radar vs rainfall. Panel 1 uses the Radar/Data tabs. Cameras are linked."))
                .when(live.layout == PanelLayout::Wipe, |d| {
                    let wipe = live.wipe;
                    d.child(row().justify_between().child(muted("Slider position")).child(muted(format!("{:.0}%", wipe * 100.0))))
                        .child(slider("wipe", wipe, 0.0, 1.0, cx, |this, v, _, cx| {
                            this.app.update(cx, |s, _| s.update_live(|l| l.wipe = v));
                        }))
                })
                .children(panel_editors),
        )
        .child(layers)
        .child(places)
        .child(custom)
    }

    /// Custom data layers: load CSV / GeoJSON / shapefile, pick how to show it, build the scale.
    fn custom_data_section(&mut self, cx: &mut Context<Self>) -> Div {
        let layers = self.app.read(cx).live.custom_layers.clone();
        let rows = layers
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let data = crate::customdata::load(std::path::Path::new(&l.path));
                let numeric = data.as_ref().map(|d| d.numeric_fields.clone()).unwrap_or_default();
                let set = move |this: &mut Self, cx: &mut Context<Self>, f: Box<dyn Fn(&mut crate::customdata::CustomLayer)>| {
                    this.app.update(cx, |s, _| {
                        s.update_live(|lv| {
                            if let Some(layer) = lv.custom_layers.get_mut(i) {
                                f(layer);
                            }
                        })
                    });
                };
                let modes = [("points", "Points"), ("heatmap", "Heatmap"), ("choropleth", "Shaded areas"), ("interpolate", "Interpolated"), ("contours", "Contours"), ("compare", "Difference")]
                    .into_iter()
                    .enumerate()
                    .map(|(k, (key, text))| {
                        chip(("cd-mode", i * 10 + k), text, l.mode == key).on_click(cx.listener(move |this, _, _, cx| {
                            set(this, cx, Box::new(move |layer| layer.mode = key.to_string()));
                        }))
                    })
                    .collect::<Vec<_>>();
                let fields = numeric
                    .iter()
                    .enumerate()
                    .take(12)
                    .map(|(k, name)| {
                        let (n1, n2) = (name.clone(), name.clone());
                        let on1 = l.field == *name;
                        let on2 = l.field2 == *name;
                        row()
                            .gap(px(2.))
                            .child(chip(("cd-field", i * 100 + k), name.clone(), on1).on_click(cx.listener(move |this, _, _, cx| {
                                let n = n1.clone();
                                set(this, cx, Box::new(move |layer| layer.field = n.clone()));
                            })))
                            .when(l.mode == "compare", |d| {
                                d.child(chip(("cd-field2", i * 100 + k), "minus", on2).on_click(cx.listener(move |this, _, _, cx| {
                                    let n = n2.clone();
                                    set(this, cx, Box::new(move |layer| layer.field2 = n.clone()));
                                })))
                            })
                    })
                    .collect::<Vec<_>>();
                let scales = crate::customdata::PRESET_SCALES
                    .iter()
                    .enumerate()
                    .map(|(k, (name, stops))| {
                        let stops: Vec<(f32, String)> = stops.iter().map(|(v, c)| (*v, c.to_string())).collect();
                        let on = l.scale == stops;
                        chip(("cd-scale", i * 10 + k), *name, on).on_click(cx.listener(move |this, _, _, cx| {
                            let s = stops.clone();
                            set(this, cx, Box::new(move |layer| layer.scale = s.clone()));
                        }))
                    })
                    .collect::<Vec<_>>();
                col()
                    .gap(px(4.))
                    .py(px(6.))
                    .border_b_1()
                    .border_color(c(BORDER))
                    .child(
                        row()
                            .gap(px(6.))
                            .child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(format!("{} ({} features)", l.name, data.as_ref().map(|d| d.features.len()).unwrap_or(0))))
                            .child(chip(("cd-vis", i), "Show", l.visible).on_click(cx.listener(move |this, _, _, cx| {
                                set(this, cx, Box::new(|layer| layer.visible = !layer.visible));
                            })))
                            .child(chip(("cd-legend", i), "Legend", l.legend).on_click(cx.listener(move |this, _, _, cx| {
                                set(this, cx, Box::new(|layer| layer.legend = !layer.legend));
                            })))
                            .child(button_small(("cd-del", i), "✕").on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, _| {
                                    s.update_live(|lv| {
                                        if i < lv.custom_layers.len() {
                                            lv.custom_layers.remove(i);
                                        }
                                    })
                                });
                            }))),
                    )
                    .child(row().flex_wrap().gap(px(3.)).children(modes))
                    .child(row().flex_wrap().gap(px(3.)).children(fields))
                    .children(self.app.read(cx).data.grid.as_deref().map(|g| {
                        let basins = crate::customdata::basin_rain(l, g, 24.0);
                        col()
                            .gap(px(2.))
                            .when(!basins.is_empty(), |d| d.child(muted("Basin-average rain, next 24 h (watersheds over 2\" flagged)")))
                            .children(basins.into_iter().take(8).map(|(n, v)| {
                                div().text_size(px(12.)).text_color(c(if v >= 2.0 { 0xE5383B } else { TEXT })).child(format!("{n}: {v:.2}\""))
                            }).collect::<Vec<_>>())
                    }))
                    .child(muted("Colour scale"))
                    .child(row().flex_wrap().gap(px(3.)).children(scales))
            })
            .collect::<Vec<_>>();
        section("Custom data")
            .child(muted("CSV with lat/lon columns, GeoJSON, or a shapefile (.shp with its .dbf). Evacuation zones, school districts, station surveys…"))
            .child(button_small("cd-load", "Load a data file…").on_click(cx.listener(|_this, _, _, cx| {
                crate::ui::pick_off_thread(cx, || rfd::FileDialog::new().add_filter("Data", &["csv", "json", "geojson", "shp", "png", "jpg"]).pick_file(), |this: &mut Self, path, cx| {
                match crate::customdata::new_layer(&path) {
                    Some(layer) => this.app.update(cx, |s, cx| {
                        s.update_live(|l| l.custom_layers.push(layer));
                        cx.notify();
                    }),
                    None => this.app.update(cx, |s, cx| {
                        s.toast(crate::app::ToastKind::Error, "Couldn't read that file (CSV needs lat and lon columns; images need a .pgw / .jgw world file)");
                        cx.notify();
                    }),
                }
                });
            })))
            .children(rows)
            .child(self.gis_tools(cx))
    }

    /// Buffers, intersection, clip, union, dissolve, validation, projected export and the
    /// coordinate converter.
    fn gis_tools(&mut self, cx: &mut Context<Self>) -> Div {
        let layers = self.app.read(cx).live.custom_layers.clone();
        let (a, b) = self.gis_ab;
        let pick = |which: u8, current: usize, cx: &mut Context<Self>| {
            layers
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    chip(("gis-pick", which as usize * 100 + i), l.name.clone(), current == i).on_click(cx.listener(move |this, _, _, cx| {
                        if which == 0 {
                            this.gis_ab.0 = i;
                        } else {
                            this.gis_ab.1 = i;
                        }
                        cx.notify();
                    }))
                })
                .collect::<Vec<_>>()
        };
        let pa = pick(0, a, cx);
        let pb = pick(1, b, cx);
        let proj = self.gis_projection;
        let run = |label: &'static str, id: &'static str, op: u8, cx: &mut Context<Self>| {
            button_small(id, label).on_click(cx.listener(move |this, _, _, cx| {
                let (layers, alerts) = {
                    let s = this.app.read(cx);
                    (s.live.custom_layers.clone(), s.data.map_alerts.clone())
                };
                let (a, b) = this.gis_ab;
                let la = layers.get(a).map(crate::gis::layer_polygons).unwrap_or_else(|| geo::MultiPolygon::new(Vec::new()));
                let lb = layers.get(b).map(crate::gis::layer_polygons).unwrap_or_else(|| geo::MultiPolygon::new(Vec::new()));
                let miles = this.gis_miles;
                let (name, shapes) = match op {
                    0 => (format!("{} + {miles:.0} mi", layers.get(a).map(|l| l.name.clone()).unwrap_or_default()), crate::gis::buffer(&la, miles)),
                    1 => (format!("Warnings + {miles:.0} mi"), crate::gis::buffer(&crate::gis::warning_polygons(&alerts), miles)),
                    2 => ("Intersection".to_string(), crate::gis::intersection(&la, &lb)),
                    3 => ("Clipped to warnings".to_string(), crate::gis::intersection(&la, &crate::gis::warning_polygons(&alerts))),
                    4 => ("Union".to_string(), crate::gis::union(&la, &lb)),
                    _ => ("Dissolved".to_string(), crate::gis::dissolve(&la)),
                };
                let result = crate::gis::save(&name, &shapes, this.gis_projection);
                this.app.update(cx, |s, cx| {
                    match result {
                        Ok(layer) => {
                            s.toast(crate::app::ToastKind::Success, format!("{name}: {:.0} sq mi", crate::gis::area_sq_mi(&shapes)));
                            s.update_live(|l| l.custom_layers.push(layer));
                        }
                        Err(e) => s.toast(crate::app::ToastKind::Error, format!("GIS: {e}")),
                    }
                    cx.notify();
                });
            }))
        };
        let buttons = vec![
            run("Buffer A", "gis-buffer", 0, cx),
            run("Buffer warnings", "gis-buffer-w", 1, cx),
            run("A ∩ B", "gis-int", 2, cx),
            run("Clip A to warnings", "gis-clip", 3, cx),
            run("A ∪ B", "gis-union", 4, cx),
            run("Dissolve A", "gis-dissolve", 5, cx),
        ];
        let results = self.gis_result.clone();
        section("GIS tools")
            .child(muted("A:"))
            .child(row().flex_wrap().gap(px(3.)).children(pa))
            .child(muted("B:"))
            .child(row().flex_wrap().gap(px(3.)).children(pb))
            .child(row().flex_wrap().gap(px(3.)).children([5.0f64, 10.0, 25.0, 50.0].into_iter().enumerate().map(|(i, m)| {
                chip(("gis-mi", i), format!("{m:.0} mi"), (self.gis_miles - m).abs() < 0.1).on_click(cx.listener(move |this, _, _, cx| {
                    this.gis_miles = m;
                    cx.notify();
                }))
            }).collect::<Vec<_>>()))
            .child(row().flex_wrap().gap(px(3.)).children(buttons))
            .child(muted("Also save results as"))
            .child(row().gap(px(3.)).children([(crate::gis::Projection::Wgs84, "WGS84 only"), (crate::gis::Projection::WebMercator, "+ Web Mercator"), (crate::gis::Projection::Utm, "+ UTM")].into_iter().enumerate().map(|(i, (p, t))| {
                chip(("gis-proj", i), t, proj == p).on_click(cx.listener(move |this, _, _, cx| {
                    this.gis_projection = p;
                    cx.notify();
                }))
            }).collect::<Vec<_>>()))
            .child(
                row()
                    .gap(px(4.))
                    .child(button_small("gis-validate", "Validate A").on_click(cx.listener(|this, _, _, cx| {
                        let layers = this.app.read(cx).live.custom_layers.clone();
                        this.gis_result = layers.get(this.gis_ab.0).map(crate::gis::validate).unwrap_or_default();
                        cx.notify();
                    })))
                    .child(button_small("gis-convert", "Convert coordinates (text box)").on_click(cx.listener(|this, _, _, cx| {
                        let q = this.text_input.read(cx).value().to_string();
                        this.gis_result = crate::gis::convert(&q).lines().map(String::from).collect();
                        cx.notify();
                    }))),
            )
            .children(results.into_iter().map(|r| div().text_size(px(12.)).text_color(c(TEXT)).child(r)).collect::<Vec<_>>())
    }

    /// Saved place collections and custom aliases.
    fn places_section(&mut self, cx: &mut Context<Self>) -> Div {
        let settings = self.app.read(cx).settings.clone();
        let mut groups: Vec<String> = settings.saved_places.iter().map(|p| p.group.clone()).collect();
        groups.sort();
        groups.dedup();
        let collections = groups
            .into_iter()
            .map(|g| {
                let places = settings
                    .saved_places
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.group == g)
                    .map(|(i, p)| {
                        let (lat, lon, label) = (p.lat, p.lon, p.label.clone());
                        row()
                            .gap(px(2.))
                            .child(chip(("saved-place", i), label, false).on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    let zoom = s.live.view.zoom.max(8.5);
                                    s.fly_to(crate::config::GeoView { lat, lon, zoom }, 1.6);
                                    cx.notify();
                                });
                                this.open_inspect(lat, lon, cx);
                            })))
                            .child(button_small(("saved-del", i), "✕").on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    let mut settings = s.settings.clone();
                                    if i < settings.saved_places.len() {
                                        settings.saved_places.remove(i);
                                    }
                                    s.apply_settings(settings);
                                    cx.notify();
                                });
                            })))
                    })
                    .collect::<Vec<_>>();
                col().gap(px(4.)).child(muted(g)).child(row().flex_wrap().gap(px(4.)).children(places))
            })
            .collect::<Vec<_>>();
        let aliases = settings
            .place_aliases
            .iter()
            .enumerate()
            .map(|(i, a)| {
                row()
                    .gap(px(6.))
                    .child(div().flex_1().text_size(px(12.)).text_color(c(TEXT)).child(format!("\"{}\" → {}", a.group, a.label)))
                    .child(button_small(("alias-del", i), "✕").on_click(cx.listener(move |this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            let mut settings = s.settings.clone();
                            if i < settings.place_aliases.len() {
                                settings.place_aliases.remove(i);
                            }
                            s.apply_settings(settings);
                            cx.notify();
                        });
                    })))
            })
            .collect::<Vec<_>>();
        section("Places")
            .child(muted("Type a name in the text box (top bar), then save the map centre to a collection or as an alias for search."))
            .child(
                row()
                    .gap(px(6.))
                    .child(button_small("save-place", "Save centre to collection").on_click(cx.listener(|this, _, _, cx| {
                        let group = this.text_input.read(cx).value().trim().to_string();
                        this.app.update(cx, |s, cx| {
                            let v = s.live.view;
                            let near = crate::ui::map_inspect::place_name(v.lat, v.lon).unwrap_or_else(|| format!("{:.3}, {:.3}", v.lat, v.lon));
                            let mut settings = s.settings.clone();
                            settings.saved_places.push(crate::config::SavedPlace { group: if group.is_empty() { "Saved".into() } else { group }, label: near, lat: v.lat, lon: v.lon });
                            s.apply_settings(settings);
                            s.toast(crate::app::ToastKind::Success, "Place saved");
                            cx.notify();
                        });
                    })))
                    .child(button_small("alias-place", "Make alias for centre").on_click(cx.listener(|this, _, _, cx| {
                        let alias = this.text_input.read(cx).value().trim().to_string();
                        if alias.is_empty() {
                            return;
                        }
                        this.app.update(cx, |s, cx| {
                            let v = s.live.view;
                            let near = crate::ui::map_inspect::place_name(v.lat, v.lon).unwrap_or_else(|| format!("{:.3}, {:.3}", v.lat, v.lon));
                            let mut settings = s.settings.clone();
                            settings.place_aliases.retain(|a| !a.group.eq_ignore_ascii_case(&alias));
                            settings.place_aliases.push(crate::config::SavedPlace { group: alias.clone(), label: near, lat: v.lat, lon: v.lon });
                            s.apply_settings(settings);
                            s.toast(crate::app::ToastKind::Success, format!("Search \"{alias}\" now finds this spot"));
                            cx.notify();
                        });
                    }))),
            )
            .children(collections)
            .when(!settings.place_aliases.is_empty(), |d| d.child(muted("Aliases")))
            .children(aliases)
    }

    /// Layer manager: order, look (opacity, blend, pulse, clip, colour), groups, rules, freshness
    /// and presets for every overlay that is switched on.
    fn layers_section(&mut self, cx: &mut Context<Self>) -> Div {
        use crate::map::scene::{LayerFx, OVERLAY_LAYERS};
        let state = self.app.read(cx);
        let live = state.live.clone();
        let health = state.health.clone();
        let data = state.data.clone();
        let now_ms = chrono::Utc::now().timestamp_millis();
        let age = |key: &str| -> Option<i64> {
            let at = |s: &crate::data::FeedStatus| s.at.map(|t| (chrono::Local::now() - t).num_minutes());
            match key {
                "warnings" | "countdowns" => at(&health.alerts),
                "cells" | "storm_tracks" | "mcs" => at(&health.cells),
                "lightning" | "lightning_density" | "lightning_anim" | "lightning_clusters" => data.lightning.iter().map(|s| s.time_ms).max().map(|t| (now_ms - t) / 60_000),
                "reports" | "damage_map" => at(&health.reports),
                "spc" | "mesoscale" => at(&health.spc),
                "roads" | "traffic" => at(&health.roads),
                "rivers" | "flood_map" | "qpf" | "ero" => at(&health.hydro),
                _ => None,
            }
        };
        let order = live.overlay_order();
        let on: Vec<&'static str> = order.iter().copied().filter(|k| live.overlay_switch(k)).collect();
        let groups = ["Outlooks", "Outlooks/Hydro", "Severe", "Severe/Storms", "Severe/Lightning", "Hydro", "Travel", "Community", "Reference"];
        let group_chips = groups
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let hidden = live.hidden_groups.iter().any(|h| h == g);
                let g = g.to_string();
                chip(("lyr-group", i), g.replace('/', " ▸ "), !hidden).on_click(cx.listener(move |this, _, _, cx| {
                    let g = g.clone();
                    this.app.update(cx, |s, _| {
                        s.update_live(|l| {
                            if let Some(pos) = l.hidden_groups.iter().position(|h| *h == g) {
                                l.hidden_groups.remove(pos);
                            } else {
                                l.hidden_groups.push(g);
                            }
                        })
                    });
                }))
            })
            .collect::<Vec<_>>();
        let rows = on
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let key: &'static str = key;
                let label = OVERLAY_LAYERS.iter().find(|(k, _, _)| *k == key).map(|(_, l, _)| *l).unwrap_or(key);
                let fx = live.layer_fx.get(key).cloned().unwrap_or_default();
                let visible = live.overlay_visible(key);
                let need = live.layer_rules.iter().find(|(k, _)| k == key).map(|(_, n)| n.clone());
                let fresh = age(key);
                let set = move |this: &mut Self, cx: &mut Context<Self>, f: Box<dyn Fn(&mut LayerFx)>| {
                    this.app.update(cx, |s, _| {
                        s.update_live(|l| {
                            let mut fx = l.layer_fx.get(key).cloned().unwrap_or_default();
                            f(&mut fx);
                            if fx.is_plain() {
                                l.layer_fx.remove(key);
                            } else {
                                l.layer_fx.insert(key.to_string(), fx);
                            }
                        })
                    });
                };
                col()
                    .gap(px(4.))
                    .py(px(6.))
                    .border_b_1()
                    .border_color(c(BORDER))
                    .child(
                        row()
                            .gap(px(6.))
                            .child(div().flex_1().min_w_0().truncate().text_size(px(13.)).text_color(c(if visible { TEXT } else { MUTED })).child(if visible { label.to_string() } else { format!("{label} (hidden)") }))
                            .when_some(fresh, |d, m| d.child(muted(if m <= 0 { "now".to_string() } else { format!("{m} min old") })))
                            .child(button_small(("lyr-up", i), "▲").on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, _| {
                                    s.update_live(|l| {
                                        let mut order: Vec<String> = l.overlay_order().iter().map(|k| k.to_string()).collect();
                                        if let Some(pos) = order.iter().position(|k| k == key) {
                                            // Up = drawn later = on top.
                                            if pos + 1 < order.len() {
                                                order.swap(pos, pos + 1);
                                            }
                                        }
                                        l.layer_order = order;
                                    })
                                });
                            })))
                            .child(button_small(("lyr-down", i), "▼").on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, _| {
                                    s.update_live(|l| {
                                        let mut order: Vec<String> = l.overlay_order().iter().map(|k| k.to_string()).collect();
                                        if let Some(pos) = order.iter().position(|k| k == key) {
                                            if pos > 0 {
                                                order.swap(pos, pos - 1);
                                            }
                                        }
                                        l.layer_order = order;
                                    })
                                });
                            }))),
                    )
                    .child(
                        row()
                            .flex_wrap()
                            .gap(px(4.))
                            .child(button_small(("lyr-op", i), format!("{:.0}%", fx.opacity * 100.0)).on_click(cx.listener(move |this, _, _, cx| {
                                set(this, cx, Box::new(|fx| fx.opacity = match (fx.opacity * 100.0).round() as i32 { 100 => 0.75, 75 => 0.5, 50 => 0.25, _ => 1.0 }));
                            })))
                            .child(button_small(("lyr-blend", i), if fx.blend.is_empty() { "normal".to_string() } else { fx.blend.clone() }).on_click(cx.listener(move |this, _, _, cx| {
                                set(this, cx, Box::new(|fx| {
                                    fx.blend = match fx.blend.as_str() { "normal" | "" => "multiply", "multiply" => "screen", "screen" => "overlay", "overlay" => "lighten", "lighten" => "darken", _ => "normal" }.to_string();
                                }));
                            })))
                            .child(chip(("lyr-pulse", i), "Pulse", fx.pulse).on_click(cx.listener(move |this, _, _, cx| {
                                set(this, cx, Box::new(|fx| fx.pulse = !fx.pulse));
                            })))
                            .child(chip(("lyr-clip", i), "Only near home", fx.clip.is_some()).on_click(cx.listener(move |this, _, _, cx| {
                                let home = this.app.read(cx).settings.home;
                                let miles = this.app.read(cx).settings.coverage_miles.max(25.0);
                                set(this, cx, Box::new(move |fx| fx.clip = if fx.clip.is_some() { None } else { Some((home.lat, home.lon, miles)) }));
                            })))
                            .child(button_small(("lyr-tint", i), match fx.tint.as_deref() { None => "Theme colours".to_string(), Some(t) => format!("Colour {t}") }).on_click(cx.listener(move |this, _, _, cx| {
                                set(this, cx, Box::new(|fx| {
                                    fx.tint = match fx.tint.as_deref() { None => Some("#FFFFFF".into()), Some("#FFFFFF") => Some("#FFD60A".into()), Some("#FFD60A") => Some("#FF3B3B".into()), Some("#FF3B3B") => Some("#38BDF8".into()), _ => None };
                                }));
                            })))
                            .child(button_small(("lyr-need", i), match &need { None => "Always".to_string(), Some(n) => format!("Only with {n}") }).on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, _| {
                                    s.update_live(|l| {
                                        let current = l.layer_rules.iter().find(|(k, _)| k == key).map(|(_, n)| n.clone());
                                        let next = match current.as_deref() { None => Some("warnings"), Some("warnings") => Some("lightning"), Some("lightning") => Some("cells"), _ => None };
                                        l.layer_rules.retain(|(k, _)| k != key);
                                        if let Some(n) = next {
                                            if n != key {
                                                l.layer_rules.push((key.to_string(), n.to_string()));
                                            }
                                        }
                                    })
                                });
                            }))),
                    )
            })
            .collect::<Vec<_>>();
        section("Layers")
            .child(muted("Groups (click to hide a whole group):"))
            .child(row().flex_wrap().gap(px(4.)).children(group_chips))
            .when(on.is_empty(), |d| d.child(muted("Switch on overlays (Severe, Data tabs) to manage them here.")))
            .children(rows)
            .child(
                row()
                    .gap(px(6.))
                    .child(button_small("lyr-export", "Save layer preset…").on_click(cx.listener(|_this, _, _, cx| {
                        let dir = crate::config::LibraryPaths::get().root.join("Layer presets");
                        let _ = std::fs::create_dir_all(&dir);
                        crate::ui::pick_off_thread(cx, move || rfd::FileDialog::new().set_directory(&dir).add_filter("Layer preset", &["json"]).set_file_name("Layer preset.json").save_file(), |this: &mut Self, path, cx| {
                            let preset = this.app.read(cx).live.layer_preset();
                            let result = serde_json::to_string_pretty(&preset).map_err(anyhow::Error::from).and_then(|t| std::fs::write(&path, t).map_err(anyhow::Error::from));
                            this.app.update(cx, |s, cx| {
                                match result {
                                    Ok(()) => s.toast(crate::app::ToastKind::Success, "Layer preset saved"),
                                    Err(e) => s.toast(crate::app::ToastKind::Error, format!("Could not save: {e}")),
                                }
                                cx.notify();
                            });
                        });
                    })))
                    .child(button_small("lyr-import", "Load layer preset…").on_click(cx.listener(|_this, _, _, cx| {
                        let dir = crate::config::LibraryPaths::get().root.join("Layer presets");
                        crate::ui::pick_off_thread(cx, move || rfd::FileDialog::new().set_directory(&dir).add_filter("Layer preset", &["json"]).pick_file(), |this: &mut Self, path, cx| {
                            match std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
                                Some(preset) => this.app.update(cx, |s, cx| {
                                    s.update_live(|l| l.apply_layer_preset(&preset));
                                    s.toast(crate::app::ToastKind::Success, "Layer preset loaded");
                                    cx.notify();
                                }),
                                None => this.app.update(cx, |s, cx| {
                                    s.toast(crate::app::ToastKind::Error, "Not a layer preset");
                                    cx.notify();
                                }),
                            }
                        });
                    }))),
            )
    }

    // ---- Right panel -------------------------------------------------------------------------

    fn info_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let data = state.data.clone();
        let health = state.health.clone();
        let map_alert_count = data.map_alerts.len();
        let selected = state.live.selected_alert.clone();

        let alerts = data
            .local_alerts
            .iter()
            .take(8)
            .enumerate()
            .map(|(i, a)| {
                let color = hex_color(a.color());
                let id = a.id.clone();
                let has_polygon = data.map_alerts.iter().any(|m| m.id == a.id);
                let active = selected.as_deref() == Some(a.id.as_str());
                col()
                    .id(("alert", i))
                    .gap(px(2.))
                    .p(px(8.))
                    .rounded(px(6.))
                    .bg(if active { c(0x163A47) } else { c(PANEL_2) })
                    .border_l_4()
                    .border_color(color)
                    .when(has_polygon, |d| d.cursor_pointer().hover(|s| s.bg(c(0x232C40))))
                    .child(div().text_size(px(13.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).child(a.event.clone()))
                    .child(div().text_size(px(11.)).text_color(c(MUTED)).child(a.area_desc.clone()))
                    .when(has_polygon, |d| d.child(div().text_size(px(10.)).text_color(c(CYAN)).child(if active { "ON AIR CALL-OUT • click to clear" } else { "Click to call out on air" })))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !has_polygon {
                            return;
                        }
                        let id = id.clone();
                        this.app.update(cx, |s, cx| {
                            s.select_alert(if active { None } else { Some(id) });
                            cx.notify();
                        });
                    }))
            })
            .collect::<Vec<_>>();

        let cells = data
            .cells
            .iter()
            .take(10)
            .enumerate()
            .map(|(i, cell)| {
                let cell = cell.clone();
                let summary = format!(
                    "{:.0} mi {} • {}",
                    cell.distance_mi,
                    cell.direction,
                    cell.motion
                        .as_ref()
                        .map(|m| format!("→ {} {:.0} mph", m.toward, m.speed_mph))
                        .unwrap_or_else(|| "stationary".into())
                );
                let level_color = match cell.intensity.level {
                    0..=1 => GREEN,
                    2..=3 => AMBER,
                    _ => RED,
                };
                row()
                    .id(("cell", i))
                    .gap(px(8.))
                    .p(px(8.))
                    .rounded(px(6.))
                    .bg(c(PANEL_2))
                    .cursor_pointer()
                    .hover(|s| s.bg(c(0x232C40)))
                    .child(div().w(px(8.)).h(px(28.)).rounded(px(3.)).bg(c(level_color)))
                    .child(
                        col()
                            .flex_1()
                            .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(format!("{} • {:.0} dBZ", cell.calm_label(), cell.max_dbz)))
                            .child(div().text_size(px(11.)).text_color(c(MUTED)).child(summary)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let cell = cell.clone();
                        this.app.update(cx, |s, _| {
                            let calm = s.settings.calm_storm_labels;
                            let view = crate::config::GeoView {
                                lat: cell.lat,
                                lon: cell.lon,
                                zoom: s.live.view.zoom.max(8.0),
                            };
                            s.fly_to(view, 1.4);
                            s.update_live(|l| {
                                l.storm_cells = true;
                                if let Some(m) = &cell.motion {
                                    l.storm_track = Some(StormTrack {
                                        lat: cell.lat,
                                        lon: cell.lon,
                                        heading_deg: m.heading_deg,
                                        speed_mph: m.speed_mph.max(5.0),
                                        label: cell.on_air_label(calm).to_uppercase(),
                                        ..StormTrack::default()
                                    });
                                }
                            });
                        });
                    }))
            })
            .collect::<Vec<_>>();

        let current = data.overview.as_ref().map(|o| o.current.clone());
        let feed = |name: &str, ok: bool, msg: String| {
            row()
                .gap(px(6.))
                .child(div().w(px(8.)).h(px(8.)).rounded(px(4.)).bg(c(if ok { GREEN } else if msg.is_empty() { MUTED } else { RED })))
                .child(div().text_size(px(12.)).text_color(c(TEXT)).w(px(70.)).child(name.to_string()))
                .child(div().text_size(px(11.)).text_color(c(MUTED)).truncate().child(if msg.is_empty() { "off".to_string() } else { msg }))
        };

        col()
            .id("map-info")
            .w(px(290.))
            .h_full()
            .flex_shrink_0()
            .bg(c(PANEL))
            .border_l_1()
            .border_color(c(BORDER))
            .overflow_y_scroll()
            .child(
                section("Conditions").when_some(current, |d, cur| {
                    d.child(
                        row()
                            .gap(px(10.))
                            .child(
                                div()
                                    .font_family("Bahnschrift")
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(40.))
                                    .text_color(c(TEXT))
                                    .child(cur.temperature_f.map(|t| format!("{t:.0}°")).unwrap_or_else(|| "--".into())),
                            )
                            .child(
                                col()
                                    .child(label(cur.text_description.clone().unwrap_or_default()))
                                    .child(muted(cur.station_label.clone().unwrap_or_default())),
                            ),
                    )
                    .child(kv("Wind", format!("{} {:.0} mph", cur.wind_direction_cardinal.clone().unwrap_or_default(), cur.wind_speed_mph.unwrap_or(0.0))))
                    .child(kv("Humidity", cur.relative_humidity.map(|h| format!("{h:.0}%")).unwrap_or_default()))
                    .child(kv("Dew point", cur.dewpoint_f.map(|d| format!("{d:.0}°")).unwrap_or_default()))
                }),
            )
            .child(
                section(format!("Local alerts • {} on map", map_alert_count))
                    .when(data.local_alerts.is_empty(), |d| d.child(muted("No active alerts for the home area.")))
                    .children(alerts),
            )
            .child(
                section("Storm cells (click to track)")
                    .when(data.cells.is_empty(), |d| d.child(muted("No storm cells detected nearby.")))
                    .children(cells),
            )
            .child(
                section("Data feeds")
                    .child(feed("Radar", health.radar.ok, health.radar.message.clone()))
                    .child(feed("Forecast", health.overview.ok, health.overview.message.clone()))
                    .child(feed("Alerts", health.alerts.ok, health.alerts.message.clone()))
                    .child(feed("Cells", health.cells.ok, health.cells.message.clone()))
                    .child(feed("SPC", health.spc.ok, health.spc.message.clone()))
                    .child(feed("Lightning", health.lightning.ok, health.lightning.message.clone()))
                    .child(feed("Model", health.grid.ok, health.grid.message.clone()))
                    .child(button_small("refresh", "Refresh now").on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| s.refresh_data(cx));
                    }))),
            )
    }

    /// Quick layer switches in the Layers menu above the map.
    fn layers_menu(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let l = self.app.read(cx).live.clone();
        type Flip = fn(&mut MapScene);
        let items: Vec<(&'static str, bool, Flip)> = vec![
            ("Radar", l.radar.enabled, |l| l.radar.enabled = !l.radar.enabled),
            ("Satellite", l.satellite.enabled, |l| l.satellite.enabled = !l.satellite.enabled),
            ("Warnings", l.warnings && l.alert_kinds.warnings, |l| {
                l.alert_kinds.warnings = !(l.warnings && l.alert_kinds.warnings);
                l.warnings = l.warnings || l.alert_kinds.warnings;
            }),
            ("Watches", l.warnings && l.alert_kinds.watches, |l| {
                l.alert_kinds.watches = !(l.warnings && l.alert_kinds.watches);
                l.warnings = l.warnings || l.alert_kinds.watches;
            }),
            ("Advisories", l.warnings && l.alert_kinds.advisories, |l| {
                l.alert_kinds.advisories = !(l.warnings && l.alert_kinds.advisories);
                l.warnings = l.warnings || l.alert_kinds.advisories;
            }),
            ("Countdowns", l.warning_countdowns, |l| l.warning_countdowns = !l.warning_countdowns),
            ("Lightning", l.lightning, |l| l.lightning = !l.lightning),
            ("Cells", l.storm_cells, |l| l.storm_cells = !l.storm_cells),
            ("Reports", l.reports, |l| l.reports = !l.reports),
            ("Airport obs", l.metar_plots, |l| l.metar_plots = !l.metar_plots),
            ("Roads", l.roads, |l| l.roads = !l.roads),
            ("Counties", l.counties, |l| l.counties = !l.counties),
            ("States", l.states, |l| l.states = !l.states),
            ("Labels", l.labels, |l| l.labels = !l.labels),
            ("Wind flow", l.data.wind_particles, |l| l.data.wind_particles = !l.data.wind_particles),
        ];
        let chips = items
            .into_iter()
            .enumerate()
            .map(|(i, (text, on, flip))| {
                div()
                    .id(("quick", i))
                    .px(px(8.))
                    .py(px(3.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .border_1()
                    .when(on, |d| d.bg(ca(CYAN, 0.9)).border_color(c(CYAN)).text_color(c(0xFFFFFF)))
                    .when(!on, |d| d.bg(rgba(0x0B0F17D8)).border_color(c(BORDER)).text_color(c(MUTED)))
                    .child(text)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(flip));
                    }))
            })
            .collect::<Vec<_>>();
        col().gap(px(6.)).px(px(10.)).py(px(6.))
            .child(button_small("layer-toggle", if self.layers_open { "Hide layers sidebar" } else { "Show layers sidebar" }).on_click(cx.listener(|this, _, _, cx| {
                this.layers_open = !this.layers_open;
                cx.notify();
            })))
            .child(row().flex_wrap().gap(px(4.)).children(chips))
    }

    // ---- Toolbar, bookmarks, timeline --------------------------------------------------------

    /// Which of the main map modes is showing.
    fn view_mode(live: &MapScene) -> ViewMode {
        match live.data.field {
            Some(DataField::Futurecast) => ViewMode::Futurecast,
            Some(_) => ViewMode::DataMaps,
            None if !live.radar.enabled && (live.spc_day.is_some() || live.ero_day.is_some() || live.fire_outlook) => ViewMode::Outlook,
            None if live.radar.enabled && live.radar.source == RadarSource::Site => ViewMode::SingleSite,
            None if !live.radar.enabled && live.satellite.enabled => ViewMode::Satellite,
            None => ViewMode::LiveRadar,
        }
    }

    fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        // The ribbon follows the map, like Word's contextual tabs.
        if self.ribbon != RibbonTab::Home {
            self.ribbon = RibbonTab::for_mode(mode);
        }
        self.tab = match mode {
            ViewMode::LiveRadar | ViewMode::SingleSite => LeftTab::Radar,
            ViewMode::Satellite => LeftTab::Satellite,
            ViewMode::Futurecast | ViewMode::DataMaps => LeftTab::Data,
            ViewMode::Outlook => LeftTab::Severe,
        };
        self.app.update(cx, |s, cx| {
            s.data_playing = false;
            s.update_live(|l| {
                let was_outlook = Self::view_mode(l) == ViewMode::Outlook;
                l.radar_replay = None;
                l.site_replay = None;
                l.history_time = None;
                // Leaving the outlook map takes its outlook layers with it.
                if was_outlook && mode != ViewMode::Outlook {
                    l.spc_day = None;
                    l.ero_day = None;
                    l.fire_outlook = false;
                }
                match mode {
                ViewMode::Outlook => {
                    l.data.field = None;
                    l.precip_total = None;
                    l.radar.enabled = false;
                    l.satellite.enabled = false;
                    l.layout = PanelLayout::Single;
                    if l.spc_day.is_none() && l.ero_day.is_none() && !l.fire_outlook {
                        l.spc_day = Some(1);
                    }
                    if l.spc_layer.is_empty() {
                        l.spc_layer = "categorical".into();
                    }
                    l.warnings = false;
                    l.counties = true;
                }
                ViewMode::LiveRadar => {
                    back_to_radar(l);
                    l.warnings = true;
                }
                ViewMode::SingleSite => {
                    l.data.field = None;
                    l.radar.enabled = true;
                    l.radar.source = RadarSource::Site;
                }
                ViewMode::Satellite => {
                    l.data.field = None;
                    l.precip_total = None;
                    l.satellite.enabled = true;
                    l.radar.enabled = false;
                }
                ViewMode::Futurecast => {
                    l.data.field = Some(DataField::Futurecast);
                    l.data.hour = 0.0;
                    l.data.opacity = l.data.opacity.max(0.85);
                    if !l.data.model_product.spans().contains(&l.data.loop_hours) { l.data.loop_hours = 24; }
                    l.radar.enabled = false;
                    l.satellite.enabled = false;
                }
                ViewMode::DataMaps => {
                    if matches!(l.data.field, None | Some(DataField::Futurecast)) {
                        l.data.field = Some(DataField::Temperature);
                        l.data.hour = 0.0;
                        l.data.loop_hours = 72;
                        // The forecast map replaces the radar / satellite map (CWXB-69).
                        l.radar.enabled = false;
                        l.satellite.enabled = false;
                        l.precip_total = None;
                    }
                }
            }});
            // Coming back to Live radar resumes the loop (it may have been paused earlier).
            if mode == ViewMode::LiveRadar {
                s.resume_live_loop();
            }
            cx.notify();
        });
        cx.notify();
    }

    fn select_forecast_product(&mut self, product: crate::futurecast::Product, cx: &mut Context<Self>) {
        use crate::futurecast::Source;

        self.forecast_picker = None;
        self.app.update(cx, |s, cx| {
            s.set_data_hour(0.0);
            s.update_live(|map| {
                let switching_model = map.data.field != Some(DataField::Futurecast)
                    || map.data.model_product.source() != product.source();
                map.data.field = Some(DataField::Futurecast);
                map.data.model_product = product;
                map.data.grid_model = match product.source() {
                    Source::Gfs => crate::grid::GridModel::Gfs,
                    Source::Hrrr => crate::grid::GridModel::Hrrr,
                    Source::Nbm => crate::grid::GridModel::Nbm,
                };
                if switching_model || !product.spans().contains(&map.data.loop_hours) {
                    map.data.loop_hours = match product.source() {
                        Source::Gfs => 72,
                        Source::Hrrr => 24,
                        Source::Nbm => 48,
                    };
                }
                map.data.opacity = map.data.opacity.max(0.85);
                map.radar.enabled = false;
                map.satellite.enabled = false;
                map.precip_total = None;
                map.history_time = None;
                map.radar_replay = None;
                map.site_replay = None;
            });
            cx.notify();
        });
        cx.notify();
    }

    /// The grid field a NOAA product shows the same data as (hidden from the menu with GFS).
    pub(super) fn noaa_field(product: crate::futurecast::Product) -> Option<DataField> {
        use crate::futurecast::Product;
        Some(match product {
            Product::Temperature => DataField::Temperature,
            Product::DewPoint => DataField::DewPoint,
            Product::Humidity => DataField::Humidity,
            Product::Wind => DataField::WindSpeed,
            Product::Gust => DataField::WindGust,
            Product::Pressure => DataField::Pressure,
            Product::TotalPrecip => DataField::RainTotal,
            Product::Snowfall => DataField::SnowTotal,
            Product::Ice => DataField::IceTotal,
            _ => return None,
        })
    }

    /// NOAA full-resolution products shown under a picker category.
    fn noaa_products(group: FieldGroup) -> &'static [crate::futurecast::Product] {
        use crate::futurecast::Product;
        match group {
            FieldGroup::Precipitation => &[Product::Precipitation, Product::PrecipitationRate, Product::TotalPrecip, Product::GfsReflectivity, Product::HrrrReflectivity],
            FieldGroup::Temperature => &[Product::Temperature, Product::DewPoint, Product::Humidity],
            FieldGroup::Wind => &[Product::Wind, Product::Gust, Product::Pressure],
            FieldGroup::Winter => &[Product::Snowfall, Product::Ice],
            _ => &[],
        }
    }

    /// Pick a gridded data map (or turn the data layer off).
    fn select_data_field(&mut self, f: Option<DataField>, cx: &mut Context<Self>) {
        self.forecast_picker = None;
        if let Some(field) = f {
            self.forecast_group = field.group();
        }
        self.app.update(cx, |s, cx| {
            s.update_live(|l| {
                let previous = l.data.field;
                l.data.field = f;
                if previous.is_none() || previous == Some(DataField::Futurecast) {
                    l.data.loop_hours = 72;
                }
                let model_hours = l.data.grid_model.max_hours();
                l.data.loop_hours = l.data.loop_hours.min(crate::grid::MAX_FORECAST_HOURS).min(model_hours);
                if f.is_some() {
                    l.data.opacity = l.data.opacity.max(0.7);
                }
                // Coming from a radar or satellite map, the forecast map replaces it (like
                // NOAA products do): a single-site scan stayed under CAPE on air (CWXB-69).
                // Radar switched back on afterwards, over a forecast map, stays.
                if f.is_some() && (previous.is_none() || previous == Some(DataField::Futurecast)) {
                    l.radar.enabled = false;
                    l.satellite.enabled = false;
                    l.precip_total = None;
                    l.radar_replay = None;
                    l.site_replay = None;
                    l.history_time = None;
                }
            });
            if f.is_some() {
                s.set_data_hour(0.0);
            }
            cx.notify();
        });
        cx.notify();
    }

    /// Switch the forecast model. NOAA products stay on NOAA data when the model has them;
    /// otherwise the gridded map of the same kind is shown from the new model.
    fn select_forecast_model(&mut self, model: crate::grid::GridModel, cx: &mut Context<Self>) {
        use crate::futurecast::{Product, Source};
        let live = self.app.read(cx).live.clone();
        let on_noaa = live.data.field == Some(DataField::Futurecast);
        self.app.update(cx, |s, _| s.update_live(|l| {
            l.data.grid_model = model;
            l.data.loop_hours = l.data.loop_hours.min(model.max_hours());
        }));
        match (on_noaa, model.noaa_source()) {
            (true, Some(source)) => {
                let current = live.data.model_product;
                let same_kind = Self::noaa_products(self.forecast_group).iter().copied()
                    .find(|p| p.source() == source && (p.label() == current.label() || matches!((p, current), (Product::GfsReflectivity, Product::HrrrReflectivity) | (Product::HrrrReflectivity, Product::GfsReflectivity))));
                let fallback = match source {
                    Source::Gfs => Product::Precipitation,
                    Source::Hrrr => Product::HrrrReflectivity,
                    Source::Nbm => Product::Snowfall,
                };
                self.select_forecast_product(same_kind.unwrap_or(fallback), cx);
            }
            (true, None) => {
                // This model has no NOAA files here: show its gridded equivalent.
                let field = match live.data.model_product {
                    Product::Temperature => DataField::Temperature,
                    Product::Snowfall | Product::SnowfallLow | Product::SnowfallHigh | Product::SnowfallChance => DataField::Snow10,
                    Product::Ice | Product::IceLow | Product::IceHigh | Product::IceChance => DataField::IceTotal,
                    _ => DataField::PrecipType,
                };
                self.select_data_field(Some(field), cx);
            }
            _ => cx.notify(),
        }
    }

    fn forecast_menu(&mut self, live: &MapScene, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::futurecast::Product;
        use crate::grid::GridModel;

        let noaa = live.data.field == Some(DataField::Futurecast);
        let product = live.data.model_product;
        let model = live.data.grid_model;
        let noaa_source = model.noaa_source();
        let group = self.forecast_group;
        let span = live.data.loop_hours;
        let fixed = live.data.field.and_then(DataField::fixed_window);
        let range_options: Vec<u32> = if noaa {
            product.spans().to_vec()
        } else {
            crate::grid::FORECAST_SPANS.iter().copied().filter(|h| *h <= model.max_hours()).collect()
        };
        let range_name = |hours: u32| {
            if noaa && range_options.last() == Some(&hours) {
                "Full run".to_string()
            } else if hours >= 72 {
                format!("{} days", hours / 24)
            } else {
                format!("{hours} hours")
            }
        };
        let winter = product.winter_choices().filter(|_| noaa);
        let winter_labels = ["Most likely", "Low end (10%)", "High end (90%)", "Chance"];
        let heading_row = |text: &'static str| div().w(px(86.)).flex_shrink_0().text_size(px(12.)).text_color(c(DIM)).child(text);

        // Model row.
        let models = GridModel::ALL.into_iter().enumerate().map(|(i, option)| {
            chip(("fc-model", i), option.label(), model == option)
                .on_click(cx.listener(move |this, _, _, cx| this.select_forecast_model(option, cx)))
        }).collect::<Vec<_>>();

        // Category tabs.
        let groups = FieldGroup::ALL.into_iter().enumerate().map(|(i, option)| {
            chip(("fc-group", i), option.label(), group == option).on_click(cx.listener(move |this, _, _, cx| {
                this.forecast_group = option;
                cx.notify();
            }))
        }).collect::<Vec<_>>();

        // Products in the category: NOAA full-resolution products first (for GFS, HRRR, NBM),
        // then the gridded maps, which every model offers.
        let current_noaa = winter.map_or(product, |choices| choices[0]);
        let mut products = Vec::new();
        if let Some(source) = noaa_source {
            for (i, option) in Self::noaa_products(group).iter().copied().enumerate().filter(|(_, p)| p.source() == source) {
                let title = match option {
                    Product::Precipitation => "Rain, mix and snow",
                    Product::GfsReflectivity | Product::HrrrReflectivity => "Simulated radar",
                    other => other.label(),
                };
                products.push(chip(("fc-noaa", i), format!("{title} · NOAA"), noaa && current_noaa == option)
                    .on_click(cx.listener(move |this, _, _, cx| this.select_forecast_product(option, cx))));
            }
        }
        for (i, field) in DataField::ALL.iter().copied().enumerate().filter(|(_, f)| f.group() == group && *f != DataField::Futurecast) {
            products.push(chip(("fc-field", i), field.label(), live.data.field == Some(field))
                .on_click(cx.listener(move |this, _, _, cx| this.select_data_field(Some(field), cx))));
        }
        let showing = match live.data.field {
            Some(DataField::Futurecast) => format!("{} {} · NOAA", model.label(), product.label()),
            Some(field) => format!("{} · {}", model.label(), field.label()),
            None => "Nothing shown".into(),
        };

        col().w_full().gap(px(6.)).px(px(10.)).py(px(8.)).bg(c(PANEL))
            .child(row().w_full().items_center().gap(px(8.))
                .child(div().text_size(px(13.)).text_color(c(TEXT)).child(format!("Showing: {showing}")))
                .child(div().flex_1())
                .when(live.data.field.is_some(), |d| d.child(button_small("fc-off", "Turn off forecast").on_click(cx.listener(|this, _, _, cx| this.select_data_field(None, cx))))))
            .child(row().w_full().items_center().gap(px(5.)).flex_wrap().child(heading_row("Model")).children(models))
            .child(row().w_full().gap(px(5.)).child(heading_row("")).child(muted(model.about())))
            .child(row().w_full().items_center().gap(px(5.)).flex_wrap().child(heading_row("Category")).children(groups))
            .child(row().w_full().items_start().gap(px(5.)).flex_wrap().p(px(6.)).rounded(px(6.)).bg(c(PANEL_2))
                .child(heading_row("Product"))
                .children(products))
            .when_some(winter, |d, options| d.child(row().w_full().items_center().gap(px(5.)).flex_wrap().child(heading_row("Display"))
                .children(options.into_iter().zip(winter_labels).enumerate().map(|(i, (option, name))| {
                    chip(("winter-percentile", i), name, product == option)
                        .on_click(cx.listener(move |this, _, _, cx| this.select_forecast_product(option, cx)))
                }).collect::<Vec<_>>())))
            .when(live.data.field.is_some(), |d| match fixed {
                Some(range) => d.child(row().w_full().gap(px(5.)).child(heading_row("Range")).child(muted(range))),
                None => d.child(row().w_full().items_center().gap(px(5.)).flex_wrap().child(heading_row("Range"))
                    .children(range_options.iter().copied().map(|hours| {
                        chip(("forecast-span", hours), range_name(hours), span == hours)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.set_data_hour(0.0);
                                    s.update_live(|map| map.data.loop_hours = hours);
                                    cx.notify();
                                });
                                cx.notify();
                            }))
                    }).collect::<Vec<_>>())),
            })
            .when(noaa, |d| d.child(row().w_full().items_center().gap(px(5.)).flex_wrap().child(heading_row("Compare"))
                .child(button_small("hrrr-compare", "GFS vs HRRR radar").on_click(cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |s, _| {
                        s.set_data_hour(0.0);
                        s.update_live(|map| {
                            map.layout = PanelLayout::Dual;
                            map.data.model_product = crate::futurecast::Product::GfsReflectivity;
                            map.data.loop_hours = 12;
                            let mut panel = map.panel_spec(1);
                            panel.product = PanelProduct::HrrrReflectivity;
                            if let Some(first) = map.panels.first_mut() { *first = panel; } else { map.panels.push(panel); }
                        });
                    });
                })))
                .child(button_small("gfs-compare", "GFS precip vs temperature").on_click(cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |s, _| s.update_live(|map| {
                        if map.layout == PanelLayout::Dual && matches!(map.panel_spec(1).product, PanelProduct::Futurecast | PanelProduct::GfsTemperature) {
                            map.layout = PanelLayout::Single;
                            return;
                        }
                        map.layout = PanelLayout::Dual;
                        let mut panel = map.panel_spec(1);
                        panel.product = if map.data.model_product == crate::futurecast::Product::Temperature {
                            PanelProduct::Futurecast
                        } else {
                            PanelProduct::GfsTemperature
                        };
                        if let Some(first) = map.panels.first_mut() { *first = panel; } else { map.panels.push(panel); }
                    }));
                })))
                .when(live.data.model_pins.iter().any(Option::is_some), |d| d
                    .child(button_small("latest-model-run", "Use latest run").on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, _| s.update_live(|map| map.data.model_pins = [None; 3]));
                    }))))))
            .child(muted("NOAA products come straight from NOAA's model files. The other products are point forecasts on a grid over this view (Open-Meteo); switching model reloads them."))
    }


    /// Everything Find can reach, as (title, where it lives, extra words, action).
    fn find_catalog() -> Vec<(String, String, String, FindAction)> {
        use crate::futurecast::Product;
        let mut out = Vec::new();
        for model in crate::grid::GridModel::ALL {
            out.push((format!("{} model", model.label()), "Forecast › Model".to_string(), model.about().to_string(), FindAction::Model(model)));
        }
        for field in DataField::ALL.into_iter().filter(|f| *f != DataField::Futurecast) {
            out.push((field.label().to_string(), format!("Forecast › {}", field.group().label()), format!("{} {}", field.viewer_label(), field.broadcast_title()), FindAction::Field(field)));
        }
        for product in [Product::Precipitation, Product::PrecipitationRate, Product::TotalPrecip, Product::Temperature, Product::DewPoint, Product::Humidity, Product::Wind, Product::Gust, Product::Pressure, Product::GfsReflectivity, Product::HrrrReflectivity, Product::Snowfall, Product::Ice] {
            out.push((format!("{} ({})", product.label(), match product.source() {
                crate::futurecast::Source::Gfs => "GFS",
                crate::futurecast::Source::Hrrr => "HRRR",
                crate::futurecast::Source::Nbm => "NBM",
            }), "Forecast › NOAA".to_string(), format!("{} futurecast noaa simulated radar", product.broadcast_title()), FindAction::Noaa(product)));
        }
        for tool in Tool::DRAW.iter().chain(Tool::WEATHER.iter()).chain(Tool::SEVERE.iter()).chain(Tool::ONAIR.iter()) {
            out.push((tool.label().to_string(), "Draw & raise".to_string(), tool.hint().to_string(), FindAction::Tool(*tool)));
        }
        out.push(("Raise area / lift a county".into(), "Draw & raise".into(), "raise lift spotlight county state boundary".into(), FindAction::Tool(Tool::RaisedArea)));
        for (title, extra, day, layer) in [
            ("SPC Outlook map", "spc severe convective outlook risk level marginal slight enhanced moderate high day 1", 1u8, "categorical"),
            ("Tornado outlook (SPC)", "spc tornado chance probability outlook", 1, "tornado"),
            ("Damaging wind outlook (SPC)", "spc wind chance probability outlook", 1, "wind"),
            ("Large hail outlook (SPC)", "spc hail chance probability outlook", 1, "hail"),
            ("Significant severe outlook (SPC)", "spc hatched significant sig severe outlook", 1, "significant"),
            ("Tomorrow's SPC outlook (Day 2)", "spc severe outlook day 2 tomorrow", 2, "categorical"),
            ("Day 3 SPC outlook", "spc severe outlook day 3", 3, "categorical"),
        ] {
            out.push((title.to_string(), "Outlooks".to_string(), extra.to_string(), FindAction::Outlook(day, layer)));
        }
        out.push(("Excessive rainfall outlook (WPC)".into(), "Outlooks".into(), "flood flooding rain excessive ero wpc outlook".into(), FindAction::Outlook(0, "ero")));
        out.push(("Fire weather outlook (SPC)".into(), "Outlooks".into(), "fire weather red flag dry outlook spc".into(), FindAction::Outlook(0, "fire")));
        for (title, extra, key) in [
            ("Refresh radar (F5)", "refresh reload radar update stuck blank map", "refresh"),
            ("Clear storm tracks", "storm track cone clear remove delete", "clear_tracks"),
            ("Warning countdowns on / off", "countdown svr tor min minutes warning labels", "countdowns"),
            ("Storm icons on / off", "storm cell icons blips badges", "storm_icons"),
            ("Storm paths (30/60 min) on / off", "storm cell path projection arrows 30m 60m", "storm_paths"),
            ("Distance rings on / off", "range rings miles circles distance", "rings"),
            ("Watches on / off", "watch watches severe thunderstorm tornado watch", "watches"),
        ] {
            out.push((title.to_string(), "Command".to_string(), extra.to_string(), FindAction::Command(key)));
        }
        for menu in [MapMenu::Map, MapMenu::Forecast, MapMenu::PastRadar, MapMenu::Draw, MapMenu::OnAir, MapMenu::Export, MapMenu::Show] {
            out.push((format!("{} menu", menu.label()), "Menu".to_string(), menu.keywords().to_string(), FindAction::Menu(menu)));
        }
        for tab in LeftTab::ALL {
            out.push((format!("{} panel", tab.label()), "Sidebar".to_string(), match tab {
                LeftTab::Radar => "radar palette nexrad site mrms rainfall totals",
                LeftTab::Satellite => "goes infrared visible water vapor",
                LeftTab::Data => "overlays city plots wind barbs isobars tropical drought earthquakes quakes fire outages tornado history ensemble",
                LeftTab::Severe => "warnings watches storm track range rings lightning cells call out",
                LeftTab::OnAir => "banner sponsor clock time box timestamp legend overlay graphic camera",
                LeftTab::Layout => "compare panels split dual quad wipe",
            }.to_string(), FindAction::Tab(tab)));
        }
        // Beyond Map Studio: workspaces, tool windows, Settings pages and graphic templates.
        use crate::app::{Modal, OpenRequest, Workspace};
        for ws in Workspace::ALL.into_iter().filter(|w| *w != Workspace::Map) {
            out.push((ws.label().to_string(), "Workspace".to_string(), match ws {
                Workspace::Severe => "severe center warnings queue tornado emergency practice rehearse",
                Workspace::Winter => "winter studio snow ice snowfall forecast map storm potential",
                Workspace::Graphics => "graphics studio editor templates design lower third",
                Workspace::Show => "show builder rundown scenes take automated broadcast present",
                Workspace::Data => "data discovery products layers palettes",
                Workspace::Media => "media library images logo photos import exports recordings",
                Workspace::Map => "",
            }.to_string(), FindAction::Workspace(ws)));
        }
        for (title, extra, modal) in [
            ("Ready for air? check", "readiness checklist preflight status", Modal::Readiness),
            ("Community: closings and centers", "closings delays warming cooling centers schools", Modal::Community),
            ("Forecast editor", "station forecast edits highs lows override seven day 7-day", Modal::ForecastEditor),
            ("Data sources", "licenses lineage quality compliance credits noaa nws", Modal::DataSources),
            ("Forecaster desk", "projects notebook decisions bookmarks calculations performance", Modal::Desk),
            ("History and case studies", "historical events past storms anniversary", Modal::History),
            ("Viewer submissions", "photos reports questions poll viewers moderation", Modal::Viewers),
            ("Publishing", "publish website social facebook discord slack schedule upload", Modal::Publishing),
            ("Emergency operations", "eoc shelters evacuation resources incident closures", Modal::Eoc),
            ("Workflow automation", "workflows automation rules triggers", Modal::Workflows),
            ("Science lab", "verification nowcast data files accuracy", Modal::Lab),
            ("Map themes", "theme basemap design colors style", Modal::Themes),
            ("Sponsors", "sponsor rotation logos ads", Modal::Sponsors),
            ("What's new", "changelog release notes", Modal::Changelog),
            ("Updates", "update version download", Modal::Updates),
            ("About", "version credits license", Modal::About),
        ] {
            out.push((title.to_string(), "Window".to_string(), extra.to_string(), FindAction::Open(OpenRequest::Modal(modal))));
        }
        out.push(("Report a bug".into(), "Help".into(), "bug problem screenshot feedback ctrl+shift+b".into(), FindAction::Command("report_bug")));
        for page in crate::ui::settings::SettingsView::page_labels() {
            let extra = match page {
                "General" => "station name home location accessibility text size",
                "Branding & sponsors" => "brand logo colors fonts sponsors station",
                "Weather & maps" => "api relay alerts area refresh map defaults severe winter ingredients",
                "Audio" => "sound volume devices tones mute microphone",
                "On-air & output" => "bug clock legend recording mp4 gif output window captions",
                "Integrations" => "obs websocket midi stream deck",
                "System" => "updates performance memory cache low resource",
                _ => "",
            };
            out.push((format!("Settings: {page}"), "Settings".to_string(), format!("settings options preferences {extra}"), FindAction::Open(OpenRequest::Settings(page))));
        }
        for info in crate::gfx::templates::all() {
            out.push((info.name.to_string(), format!("Graphic › {}", info.category), format!("graphic template {}", info.description), FindAction::Template(info.key)));
        }
        out
    }

    /// Find matches for the typed words, best first: title starts with the query, then title
    /// contains it, then any keyword matches.
    pub(super) fn find_hits(&self) -> Vec<(String, String, FindAction)> {
        Self::find_matches(&self.find_query)
    }

    fn find_matches(query: &str) -> Vec<(String, String, FindAction)> {
        let query = query.trim().to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        if words.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<(usize, String, String, FindAction)> = Self::find_catalog().into_iter().filter_map(|(title, place, extra, action)| {
            let t = title.to_lowercase();
            let hay = format!("{t} {} {}", place.to_lowercase(), extra.to_lowercase());
            words.iter().all(|w| hay.contains(w)).then(|| {
                // A whole-word keyword hit on a window, setting or tool ("obs" → Integrations)
                // outranks the many templates that merely contain the letters ("Observed").
                let padded = format!(" {} ", hay.chars().map(|ch| if ch.is_alphanumeric() { ch } else { ' ' }).collect::<String>());
                let whole = padded.contains(&format!(" {query} "));
                let template = matches!(action, FindAction::Template(_));
                let rank = if t.starts_with(&query) { 0 } else if whole && !template { 1 } else if t.contains(&query) { 2 } else { 3 };
                (rank, title, place, action)
            })
        }).collect();
        hits.sort_by_key(|(rank, title, ..)| (*rank, title.len()));
        hits.into_iter().map(|(_, t, p, a)| (t, p, a)).collect()
    }

    /// The drop-down list under the Find box (it floats over the map instead of pushing it down).
    pub(super) fn find_results(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let hits = self.find_hits();
        let total = hits.len();
        let rows = hits.into_iter().take(40).enumerate().map(|(i, (title, place, action))| {
            div().id(("find-hit", i)).flex().items_center().gap(px(8.)).h(px(28.)).px(px(8.)).mx(px(4.)).rounded(px(4.)).cursor_pointer()
                .when(i == 0, |d| d.bg(ca(CYAN, 0.14)))
                .hover(|d| d.bg(ca(CYAN, 0.22)))
                .child(div().flex_1().min_w_0().truncate().text_size(px(13.)).text_color(c(TEXT)).child(title))
                .child(div().flex_shrink_0().text_size(px(11.)).text_color(c(DIM)).child(place))
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.run_find(action.clone(), window, cx);
                }))
        }).collect::<Vec<_>>();
        div().id("find-results").occlude().mt(px(4.)).w(px(420.)).max_h(px((self.win_h - 120.).clamp(160., 520.))).overflow_y_scroll()
            .py(px(4.)).rounded(px(6.)).bg(c(PANEL)).border_1().border_color(c(BORDER)).shadow_lg()
            .child(div().px(px(12.)).py(px(4.)).text_size(px(11.)).text_color(c(DIM)).child(if total == 0 {
                "Nothing matches. Try another word.".to_string()
            } else {
                format!("{total} match{} • Enter opens the first", if total == 1 { "" } else { "es" })
            }))
            .children(rows)
    }

    fn run_find(&mut self, action: FindAction, window: &mut Window, cx: &mut Context<Self>) {
        self.find_query.clear();
        self.find_input.update(cx, |s, cx| s.set_value(String::new(), window, cx));
        match action {
            FindAction::Field(field) => {
                self.select_data_field(Some(field), cx);
                self.open_menu = None;
            }
            FindAction::Noaa(product) => {
                self.forecast_group = match product {
                    crate::futurecast::Product::Temperature | crate::futurecast::Product::DewPoint | crate::futurecast::Product::Humidity => FieldGroup::Temperature,
                    crate::futurecast::Product::Wind | crate::futurecast::Product::Gust | crate::futurecast::Product::Pressure => FieldGroup::Wind,
                    p if p.winter_choices().is_some() => FieldGroup::Winter,
                    _ => FieldGroup::Precipitation,
                };
                self.select_forecast_product(product, cx);
                self.open_menu = None;
            }
            FindAction::Model(model) => {
                self.select_forecast_model(model, cx);
                self.open_menu = None;
            }
            FindAction::Menu(menu) => {
                self.open_menu = None;
                self.ribbon_open = true;
                self.ribbon = match menu {
                    MapMenu::Map | MapMenu::Export | MapMenu::Show => RibbonTab::Home,
                    MapMenu::Forecast => RibbonTab::Forecast,
                    MapMenu::PastRadar => RibbonTab::Radar,
                    MapMenu::Draw => RibbonTab::Draw,
                    MapMenu::OnAir => RibbonTab::OnAir,
                };
            }
            FindAction::Tool(tool) => {
                self.set_tool(tool, cx);
                self.open_menu = None;
            }
            FindAction::Tab(tab) => {
                self.tab = tab;
                self.layers_open = true;
                self.open_menu = None;
            }
            FindAction::Outlook(day, layer) => {
                self.open_menu = None;
                self.show_outlook(day, layer, cx);
            }
            FindAction::Command(key) => {
                self.open_menu = None;
                match key {
                    "clear_tracks" => self.clear_storm_tracks(cx),
                    _ => self.app.update(cx, |s, cx| {
                        let home = s.settings.home;
                        match key {
                            "refresh" => s.refresh_radar(cx),
                            "countdowns" => s.update_live(|m| m.warning_countdowns = !m.warning_countdowns),
                            "storm_icons" => s.update_live(|m| m.storm_cells = !m.storm_cells),
                            "storm_paths" => s.update_live(|m| m.cell_paths = !m.cell_paths),
                            "rings" => s.update_live(|m| m.range_rings = if m.range_rings.is_some() { None } else { Some(RangeRings { lat: home.lat, lon: home.lon, ..RangeRings::default() }) }),
                            "watches" => s.update_live(|m| { m.alert_kinds.watches = !(m.warnings && m.alert_kinds.watches); m.warnings = m.warnings || m.alert_kinds.watches; }),
                            "report_bug" => s.report_bug(),
                            _ => {}
                        }
                        cx.notify();
                    }),
                }
            }
            FindAction::Workspace(ws) => {
                self.open_menu = None;
                self.app.update(cx, |s, cx| {
                    s.workspace = ws;
                    cx.notify();
                });
            }
            FindAction::Open(req) => {
                self.open_menu = None;
                self.app.update(cx, |s, cx| {
                    s.open_request = Some(req);
                    cx.notify();
                });
            }
            FindAction::Template(key) => {
                self.open_menu = None;
                self.app.update(cx, |s, cx| {
                    if let Some(doc) = crate::gfx::templates::build(key) {
                        if let Some(saved) = s.save_graphic(doc) {
                            s.toast(ToastKind::Success, format!("Created \"{}\" - it's open in Graphics", saved.name));
                            s.open_graphic(saved.id);
                        }
                    }
                    cx.notify();
                });
            }
        }
        cx.notify();
    }

    /// Switch the map to the outlook view with a given SPC day and layer ("categorical",
    /// "tornado", "wind", "hail", "significant", "severe"), or day 0 with "ero" / "fire".
    pub(super) fn show_outlook(&mut self, day: u8, layer: &'static str, cx: &mut Context<Self>) {
        self.app.update(cx, |s, _| s.update_live(|l| {
            match layer {
                "ero" => {
                    l.spc_day = None;
                    l.fire_outlook = false;
                    l.ero_day = Some(l.ero_day.unwrap_or(1));
                }
                "fire" => {
                    l.spc_day = None;
                    l.ero_day = None;
                    l.fire_outlook = true;
                }
                _ => {
                    l.ero_day = None;
                    l.fire_outlook = false;
                    l.spc_day = Some(day.clamp(1, 3));
                    l.spc_layer = if day >= 3 && matches!(layer, "tornado" | "wind" | "hail") { "severe".into() } else { layer.into() };
                }
            }
        }));
        self.set_view_mode(ViewMode::Outlook, cx);
    }

    fn raise_menu(&mut self, live: &MapScene, cx: &mut Context<Self>) -> impl IntoElement {
        let raised = live.annotations.iter().rev().find(|mark| mark.kind == AnnotationKind::RaisedArea);
        let style = live.raise_style;
        col().gap(px(6.)).px(px(10.)).py(px(6.))
            .child(row().gap(px(5.)).flex_wrap()
                .child(chip("raise-trace", "Draw area", self.tool == Tool::RaisedArea).on_click(cx.listener(|this, _, _, cx| this.set_tool(Tool::RaisedArea, cx))))
                .child(chip("raise-county-mode", "Select county", self.tool == Tool::RaisedBoundary && !self.raise_state).on_click(cx.listener(|this, _, _, cx| {
                    this.raise_state = false;
                    this.set_tool(Tool::RaisedBoundary, cx);
                })))
                .child(chip("raise-state-mode", "Select state", self.tool == Tool::RaisedBoundary && self.raise_state).on_click(cx.listener(|this, _, _, cx| {
                    this.raise_state = true;
                    this.set_tool(Tool::RaisedBoundary, cx);
                })))
                .child(muted(raised.map_or("No area selected", |mark| if mark.text.is_empty() { "Drawn area" } else { &mark.text }).to_owned())))
            .child(row().gap(px(5.)).flex_wrap().child(label("Appearance"))
                .children([(RaiseStyle::Lift, "Lifted"), (RaiseStyle::Spotlight, "Spotlight"), (RaiseStyle::Outline, "Outline")]
                    .into_iter().enumerate().map(|(i, (choice, name))| {
                        chip(("raise-style", i), name, style == choice).on_click(cx.listener(move |this, _, _, cx| {
                            this.app.update(cx, |s, _| s.update_live(|map| map.raise_style = choice));
                        }))
                    }).collect::<Vec<_>>())
                .when(raised.is_some(), |d| d.child(toggle("raise-visible", "Show raised area", live.raise_visible).on_click(cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |s, _| s.update_live(|map| map.raise_visible = !map.raise_visible));
                }))).child(button_small("clear-raise", "Remove raised area").on_click(cx.listener(|this, _, _, cx| {
                    this.cancel_raise(cx);
                    this.app.update(cx, |s, _| s.update_live(|map| map.annotations.retain(|mark| mark.kind != AnnotationKind::RaisedArea)));
                })))))
    }

    fn show_menu(&mut self, live: &MapScene, cx: &mut Context<Self>) -> impl IntoElement {
        row().gap(px(6.)).flex_wrap().px(px(10.)).py(px(6.))
            .child(button_small("add-scene", "Add still to show").on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, cx| { s.add_map_to_show(false); cx.notify(); });
            })))
            .when(live.data.field.is_some_and(|field| field.fixed_window().is_none()) || live.radar_replay.is_some() || live.site_replay.is_some(), |d| d
                .child(button_small("add-loop-to-show", "Add loop to show").on_click(cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |s, cx| { s.add_map_to_show(true); cx.notify(); });
                }))))
    }
    fn export_drawer(&mut self, live: &MapScene, cx: &mut Context<Self>) -> impl IntoElement {
        let forecast = live.data.field.is_some_and(|field| field.fixed_window().is_none());
        let replay = !forecast && live.is_radar_replay() && self.app.read(cx).radar_frames.len() > 1;
        let busy = self.exporting_map || self.app.read(cx).is_exporting_replay();
        row().gap(px(6.)).flex_wrap().px(px(10.)).py(px(6.)).bg(c(PANEL))
            .child(label("Export map"))
            .child(button_small("export-png", "Still PNG").on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, cx| { s.save_map_snapshot(); cx.notify(); });
            })))
            .when(!busy, |d| d
                .child(button_small("export-gif", if forecast { "Forecast GIF" } else if replay { "Record replay GIF" } else { "Still GIF" }).on_click(cx.listener(|this, _, _, cx| this.export_motion(true, cx))))
                .child(button_small("export-mp4", if forecast { "Forecast MP4" } else if replay { "Record replay MP4" } else { "Still MP4" }).on_click(cx.listener(|this, _, _, cx| this.export_motion(false, cx)))))
            .when(busy, |d| d.child(muted("Exporting to Media → Exports…")))
    }

    fn replay_drawer(&mut self, live: &MapScene, cx: &mut Context<Self>) -> impl IntoElement {
        let windows = crate::history::RADAR_REPLAY_HOURS.into_iter().map(|hours| {
            chip(("radar-history-hours", hours), format!("Past {hours}h"), live.radar_replay.is_some_and(|r| r.hours == hours))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.app.update(cx, |s, cx| { s.start_radar_replay(hours); cx.notify(); });
                }))
        }).collect::<Vec<_>>();
        row().gap(px(6.)).flex_wrap().px(px(10.)).py(px(6.)).bg(c(PANEL))
            .child(label("Observed radar replay"))
            .children(windows)
            .child(button_small("site-velocity-replay", if self.app.read(cx).site_replay_loading { "Loading velocity…" } else { "Past 3h site velocity" }).on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, cx| s.start_site_radar_replay(cx));
            })))
            .child(button_small("radar-history-cases", "Event history…").on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, cx| { s.modal = crate::app::Modal::History; cx.notify(); });
            })))
            .when(live.is_radar_replay(), |d| d
                .when((live.radar_replay.is_some() || live.site_replay.is_some()) && live.data.field.is_none(), |d| d
                    .child(button_small("add-radar-replay-to-show", "+ Add replay to show").on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| { s.add_map_to_show(true); cx.notify(); });
                    }))))
                .child(button_small("radar-history-live", "Back to live").on_click(cx.listener(|this, _, _, cx| {
                    this.set_view_mode(ViewMode::LiveRadar, cx);
                }))))
    }

    /// Remove every storm track (drawn or picked from a storm cell) and its ETA card.
    pub(crate) fn clear_storm_tracks(&mut self, cx: &mut Context<Self>) {
        if matches!(self.tool, Tool::StormTrack | Tool::FreeTrack) {
            self.tool = Tool::Pan;
        }
        self.drag = None;
        self.app.update(cx, |s, cx| {
            s.update_live(|l| {
                l.storm_track = None;
                l.extra_tracks.clear();
                l.follow_storm = false;
            });
            cx.notify();
        });
        cx.notify();
    }

    fn clear_map(&mut self, cx: &mut Context<Self>) {
        self.open_menu = None;
        self.tool = Tool::Pan;
        self.inspect = None;
        self.drag = None;
        self.app.update(cx, |s, cx| {
            s.data_playing = false;
            s.playing = false;
            s.interrupt_fly();
            s.set_map_overlay(None);
            // Clears drawings and the extra layers, but leaves the radar, warnings and the on-air
            // elements (title, logo, clock) as they are (CWXB-107: Clear map used to turn off
            // radar, the title and the clock, leaving Program on a bare basemap).
            let mut clean = MapScene::from_settings(&s.settings);
            let live = &s.live;
            clean.view = live.view;
            clean.basemap = live.basemap;
            clean.labels = live.labels;
            clean.theme = live.theme.clone();
            clean.radar = live.radar.clone();
            clean.warnings = live.warnings;
            clean.alert_kinds = live.alert_kinds.clone();
            clean.title = live.title.clone();
            clean.custom_title = live.custom_title;
            clean.show_title = live.show_title;
            clean.show_logo = live.show_logo;
            clean.watermark_visible = live.watermark_visible;
            clean.show_clock = live.show_clock;
            clean.show_timestamp = live.show_timestamp;
            clean.show_legend = live.show_legend;
            clean.counties = live.counties;
            clean.states = live.states;
            clean.boundary_opacity = live.boundary_opacity;
            s.update_live(|map| *map = clean);
            s.compositor.update(|program| program.map_overlay_out = None);
            cx.notify();
        });
        cx.notify();
    }

    fn export_motion(&mut self, gif: bool, cx: &mut Context<Self>) {
        if self.exporting_map { return; }
        let state = self.app.read(cx);
        if state.live.is_radar_replay() && state.live.data.field.is_none() && state.radar_frames.len() > 1 {
            let format = if gif { crate::config::RecordFormat::Gif } else { crate::config::RecordFormat::Mp4 };
            self.app.update(cx, |s, cx| { s.export_radar_replay(format); cx.notify(); });
            return;
        }
        self.exporting_map = true;
        let compositor = self.app.read(cx).compositor.clone();
        let task = cx.background_executor().spawn(async move { compositor.export_map_motion(gif) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.exporting_map = false;
                this.app.update(cx, |s, cx| {
                    match result {
                        Ok(path) => {
                            s.exports = crate::library::list_exports();
                            s.toast(ToastKind::Success, format!("Map saved to {}", path.display()));
                        }
                        Err(e) => s.toast(ToastKind::Error, format!("Could not export map: {e}")),
                    }
                    cx.notify();
                });
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    /// Map modes, location search, and saved views.
    fn mode_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let current = Self::view_mode(&state.live);
        let field = state.live.data.field;
        let bookmarks = state.settings.bookmarks.clone();

        let modes = ViewMode::ALL
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let m = *m;
                let active = current == m && (m != ViewMode::LiveRadar || state.live.radar.enabled);
                let text = if m == ViewMode::DataMaps && active {
                    format!("Data: {}", field.map(|f| f.label()).unwrap_or("Temperature"))
                } else {
                    m.label().to_string()
                };
                div()
                    .id(("mode", i))
                    .h_full()
                    .px(px(14.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_size(px(13.))
                    .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                    .when(i > 0, |d| d.border_l_1().border_color(c(BORDER)))
                    .when(active, |d| d.bg(ca(CYAN, 0.16)).text_color(c(CYAN)))
                    .when(!active, |d| d.text_color(c(TEXT)).hover(|s| s.bg(c(0x212A3C))))
                    .child(div().w(px(7.)).h(px(7.)).rounded(px(4.)).bg(if active { c(CYAN) } else { c(0x3A4458) }))
                    .child(text)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_view_mode(m, cx)))
            })
            .collect::<Vec<_>>();

        let views = bookmarks
            .into_iter()
            .enumerate()
            .map(|(i, b)| {
                let view = b.view;
                button_small(("bm", i), b.name.clone()).on_click(cx.listener(move |this, _, _, cx| {
                    this.app.update(cx, |s, _| s.fly_to(view, 1.8));
                }))
            })
            .collect::<Vec<_>>();

        row()
            .w_full()
            .gap(px(10.))
            .px(px(10.))
            .py(px(7.))
            .bg(c(PANEL))
            .border_b_1()
            .border_color(c(BORDER))
            .flex_wrap()
            .child(
                row()
                    .h(px(32.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(BORDER))
                    .bg(c(BG))
                    .overflow_hidden()
                    .children(modes),
            )
            .child(div().w(px(1.)).h(px(24.)).bg(c(BORDER)))
            .child(muted("Go to"))
            .child(
                div()
                    .relative()
                    .w(px(220.))
                    .child(Input::new(&self.place_input)),
            )
            .child(button_small("recent-places", "Recent").on_click(cx.listener(|this, _, _, cx| {
                this.show_history = !this.show_history;
                cx.notify();
            })))
            .child(button_small("home", "⌂ Home").on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, _| {
                    let home = s.settings.home;
                    s.fly_to(home, 1.8);
                });
            })))
            .children(views)
            .child(button_small("save-view", "+ Save view").on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, cx| {
                    let n = s.settings.bookmarks.len() + 1;
                    let view = s.live.view;
                    s.settings.bookmarks.push(Bookmark {
                        name: format!("View {n}"),
                        view,
                    });
                    let settings = s.settings.clone();
                    s.apply_settings(settings);
                    s.toast(ToastKind::Success, format!("Saved this view as \"View {n}\""));
                    cx.notify();
                });
            })))
            .child(div().flex_1())
            .child(
                button_small("refresh-radar", "Refresh radar (F5)").on_click(cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |s, cx| {
                        s.refresh_radar(cx);
                        cx.notify();
                    });
                })),
            )
            .child(
                button_small("toggle-info", if self.info_open { "Hide side panel ›" } else { "‹ Show side panel" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.info_open = !this.info_open;
                        cx.notify();
                    })),
            )
            .children(self.place_results(cx))
    }
    /// Results under the "Go to" search box: matching towns, or the typed coordinates.
    fn place_results(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let q = self.place_query.trim().to_string();
        let settings = self.app.read(cx).settings.clone();
        if q.len() < 2 && !self.show_history {
            return None;
        }
        let mut options: Vec<(String, f64, f64)> = Vec::new();
        if q.len() < 2 {
            options.extend(settings.search_history.iter().map(|p| (format!("↺ {}", p.label), p.lat, p.lon)));
        }
        let ql = q.to_lowercase();
        if q.len() >= 2 {
            options.extend(settings.place_aliases.iter().filter(|a| a.group.to_lowercase().contains(&ql)).map(|a| (format!("{} → {}", a.group, a.label), a.lat, a.lon)));
            options.extend(settings.saved_places.iter().filter(|p| p.label.to_lowercase().contains(&ql) || p.group.to_lowercase().contains(&ql)).map(|p| (format!("★ {} ({})", p.label, p.group), p.lat, p.lon)));
        }
        if let Some((lat, lon)) = crate::places::parse_lat_lon(&q).filter(|_| q.len() >= 2) {
            options.push((format!("{lat:.3}, {lon:.3}"), lat, lon));
        }
        for p in crate::places::search(&q, 8).into_iter().filter(|_| q.len() >= 2) {
            let label = if p.state.is_empty() { p.name.clone() } else { format!("{}, {}", p.name, p.state) };
            options.push((label, p.lat, p.lon));
        }
        options.extend(self.online_places.iter().cloned());
        let empty = options.is_empty();
        let rows = options
            .into_iter()
            .enumerate()
            .map(|(i, (label, lat, lon))| {
                div()
                    .id(("place-hit", i))
                    .px(px(10.))
                    .py(px(6.))
                    .text_size(px(13.))
                    .text_color(c(TEXT))
                    .cursor_pointer()
                    .hover(|s| s.bg(c(crate::ui::HOVER)))
                    .child(label.clone())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.place_query.clear();
                        this.show_history = false;
                        this.place_input.update(cx, |s, cx| s.set_value(String::new(), window, cx));
                        let clean = label.trim_start_matches("↺ ").trim_start_matches("★ ").to_string();
                        this.app.update(cx, |s, cx| {
                            let zoom = s.live.view.zoom.max(8.5);
                            s.fly_to(crate::config::GeoView { lat, lon, zoom }, 1.6);
                            // Search history: newest first, no repeats, 15 kept.
                            let mut settings = s.settings.clone();
                            settings.search_history.retain(|p| p.label != clean);
                            settings.search_history.insert(0, crate::config::SavedPlace { group: String::new(), label: clean.clone(), lat, lon });
                            settings.search_history.truncate(15);
                            s.apply_settings(settings);
                            cx.notify();
                        });
                        this.open_inspect(lat, lon, cx);
                    }))
            })
            .collect::<Vec<_>>();
        Some(
            col()
                .w(px(280.))
                .py(px(4.))
                .rounded(px(6.))
                .bg(c(PANEL))
                .border_1()
                .border_color(c(BORDER))
                .shadow_lg()
                .when(empty, |d| d.child(div().px(px(10.)).py(px(6.)).child(muted("Searching towns, ZIP codes and counties…"))))
                .children(rows)
                .into_any_element(),
        )
    }

    fn tool_chip(&self, i: usize, tool: Tool, cx: &mut Context<Self>) -> Stateful<Div> {
        let active = self.tool == tool;
        div()
            .id(("tool", i))
            .h(px(28.))
            .px(px(9.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(px(5.))
            .cursor_pointer()
            .text_size(px(12.))
            .border_1()
            .when(active, |d| d.bg(ca(CYAN, 0.9)).border_color(c(CYAN)).text_color(c(0xFFFFFF)))
            .when(!active, |d| d.border_color(c(BORDER)).text_color(c(TEXT)).hover(|s| s.bg(c(0x212A3C))))
            .child(tool.label())
            .child(
                div()
                    .text_size(px(10.))
                    .px(px(4.))
                    .rounded(px(3.))
                    .when(active, |d| d.bg(rgba(0xFFFFFF30)).text_color(c(0xFFFFFF)))
                    .when(!active, |d| d.bg(c(0x212A3C)).text_color(c(MUTED)))
                    .child(tool.key()),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.set_tool(tool, cx)))
    }

    /// Top bar, row 2: tools in labelled groups, then the options of the active tool.
    /// The tool palette and, under it, the active tool's options (label, colour, width, front
    /// type). `options_only` draws just the options strip (shown under the ribbon).
    fn toolbar(&mut self, options_only: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let draw = Tool::DRAW.iter().enumerate().map(|(i, t)| self.tool_chip(i, *t, cx)).collect::<Vec<_>>();
        let weather = Tool::WEATHER.iter().enumerate().map(|(i, t)| self.tool_chip(20 + i, *t, cx)).collect::<Vec<_>>();
        let severe = Tool::SEVERE.iter().enumerate().map(|(i, t)| self.tool_chip(40 + i, *t, cx)).collect::<Vec<_>>();
        let onair = Tool::ONAIR.iter().enumerate().map(|(i, t)| self.tool_chip(60 + i, *t, cx)).collect::<Vec<_>>();
        let fronts = FRONT_KINDS
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let k = *k;
                chip(("front-kind", i), k.label().replace(" front", ""), self.front_kind == k).on_click(cx.listener(move |this, _, _, cx| {
                    this.front_kind = k;
                    this.tool = Tool::Front;
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();
        let colors = PEN_COLORS
            .iter()
            .enumerate()
            .map(|(i, color)| {
                let color = color.to_string();
                swatch(("pen", i), &color, self.color == color).on_click(cx.listener(move |this, _, _, cx| {
                    this.color = color.clone();
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();
        let width = self.width;
        let marks = matches!(self.tool, Tool::Pen | Tool::Arrow | Tool::Circle | Tool::Area | Tool::Text | Tool::Marker);
        let wants_text = matches!(self.tool, Tool::Text | Tool::Marker);
        let can_animate = self.app.read(cx).live.annotations.last().is_some_and(|mark| mark.kind != AnnotationKind::RaisedArea);
        let group = |name: &'static str, items: Vec<Stateful<Div>>| {
            col()
                .gap(px(3.))
                .child(div().text_size(px(11.)).text_color(c(DIM)).child(name))
                .child(row().gap(px(4.)).flex_wrap().children(items))
        };
        let sep = || div().w(px(1.)).h(px(40.)).bg(c(BORDER));

        col()
            .w_full()
            .bg(c(PANEL))
            .border_b_1()
            .border_color(c(BORDER))
            .when(!options_only, |d| d.child(
                row()
                    .w_full()
                    .items_end()
                    .gap(px(12.))
                    .px(px(10.))
                    .py(px(7.))
                    .flex_wrap()
                    .child(group("Draw", draw))
                    .child(sep())
                    .child(group("Weather", weather))
                    .child(sep())
                    .child(group("Storms", severe))
                    .child(sep())
                    .child(group("On-air", onair))
                    .child(div().flex_1())
                    .child(
                        row()
                            .gap(px(6.))
                            .when(self.app.read(cx).fly_interrupted.is_some(), |d| {
                                d.child(button_small("resume-fly", "Resume camera move").on_click(cx.listener(|this, _, _, cx| {
                                    this.app.update(cx, |s, cx| {
                                        s.resume_fly();
                                        cx.notify();
                                    });
                                })))
                            })
                            .child(button_small("undo", "↶ Undo").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, _| {
                                    s.update_live(|l| {
                                        l.annotations.pop();
                                    })
                                });
                            })))
                            .child(button_small("regional-tour", if self.app.read(cx).tour_running() { "Stop tour" } else { "Regional tour" }).on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    if s.tour_running() {
                                        s.stop_tour();
                                    } else {
                                        s.regional_tour(5.0);
                                    }
                                    cx.notify();
                                });
                            })))
                            .child(chip("measure", "Measure", self.measure).on_click(cx.listener(|this, _, _, cx| {
                                this.measure = !this.measure;
                                if this.measure {
                                    this.app.update(cx, |s, cx| {
                                        s.toast(crate::app::ToastKind::Info, "Measure: draw with Pen for distance, Area for size; Ruler adds elevation");
                                        cx.notify();
                                    });
                                }
                                cx.notify();
                            })))
                            .child(button_small("measure-export", "Export measurements").on_click(cx.listener(|this, _, _, cx| {
                                let marks: Vec<crate::map::scene::Annotation> = this.app.read(cx).live.annotations.iter().filter(|a| a.measure || a.kind == AnnotationKind::Ruler).cloned().collect();
                                let result = export_measurements(&marks);
                                this.app.update(cx, |s, cx| {
                                    match result {
                                        Ok(path) => {
                                            s.toast(crate::app::ToastKind::Success, format!("Measurements saved: {}", path.display()));
                                            s.exports = crate::library::list_exports();
                                        }
                                        Err(e) => s.toast(crate::app::ToastKind::Error, format!("Export: {e}")),
                                    }
                                    cx.notify();
                                });
                            })))
                            .when(can_animate, |d| d.child(button_small("anim-move", if self.set_motion { "Click where it moves to…" } else { "Move last mark" }).on_click(cx.listener(|this, _, _, cx| {
                                this.set_motion = !this.set_motion;
                                cx.notify();
                            })))
                            .child(button_small("anim-reveal", "Draw on").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, _| {
                                    s.update_live(|l| {
                                        if let Some(a) = l.annotations.last_mut() {
                                            a.reveal = !a.reveal;
                                        }
                                    })
                                });
                            })))
                            )
                            .child(button_small("anim-sync", "Show from this radar frame").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    let t = s.current_radar_time();
                                    s.update_live(|l| {
                                        if let Some(a) = l.annotations.last_mut() {
                                            a.show_from = t;
                                        }
                                    });
                                    s.toast(crate::app::ToastKind::Info, "The last mark now appears when the radar loop reaches this frame");
                                    cx.notify();
                                });
                            })))
                            .child(button_small("anim-sync-end", "Hide after this frame").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    let t = s.current_radar_time();
                                    s.update_live(|l| {
                                        if let Some(a) = l.annotations.last_mut() {
                                            a.show_until = t;
                                        }
                                    });
                                    cx.notify();
                                });
                            })))
                            .child(button_small("reset-to-radar", "Reset to live radar").on_click(cx.listener(|this, _, _, cx| {
                                this.tool = Tool::Pan;
                                this.tab = LeftTab::Radar;
                                this.app.update(cx, |s, cx| {
                                    s.data_playing = false;
                                    s.update_live(|l| {
                                        l.annotations.clear();
                                        l.storm_track = None;
                                        l.extra_tracks.clear();
                                        l.range_rings = None;
                                        l.selected_alert = None;
                                        back_to_radar(l);
                                    });
                                    s.toast(ToastKind::Info, "Cleared: drawings removed, back to live radar");
                                    cx.notify();
                                });
                            }))),
                    ),
            ))
            // Options for the active tool, only when it has some.
            .when(marks || self.tool == Tool::Front, |d| {
                d.child(
                    row()
                        .w_full()
                        .gap(px(10.))
                        .px(px(10.))
                        .py(px(6.))
                        .border_t_1()
                        .border_color(c(BORDER))
                        .bg(c(0x0F141F))
                        .flex_wrap()
                        .child(div().text_size(px(11.)).text_color(c(CYAN)).child(format!("{} options", self.tool.label())))
                        .when(self.tool == Tool::Front, |d| d.child(row().gap(px(4.)).children(fronts)))
                        .when(marks, |d| {
                            d.child(row().gap(px(4.)).children(colors))
                                .child(muted("Width"))
                                .child(div().w(px(110.)).child(slider("pen-width", width, 2.0, 16.0, cx, |this, v, _, cx| {
                                    this.width = v;
                                    cx.notify();
                                })))
                        })
                        // On the Draw tab the ribbon has its own Label box (one input on screen).
                        .when(wants_text && !(options_only && self.ribbon == RibbonTab::Draw), |d| d.child(muted("Label")).child(div().w(px(220.)).child(Input::new(&self.text_input)))),
                )
            })
    }

    fn timeline(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.app.read(cx);
        if state.live.data.field.is_some() {
            return self.data_timeline(cx).into_any_element();
        }
        if !state.live.radar.enabled {
            return div().into_any_element();
        }
        if state.live.radar.source == RadarSource::Site && state.live.site_replay.is_none() {
            let site = state.live.radar.resolved_site(state.live.view.lat, state.live.view.lon);
            let product = state.live.radar.product;
            return row()
                .w_full()
                .gap(px(10.))
                .px(px(12.))
                .py(px(10.))
                .bg(c(PANEL))
                .border_t_1()
                .border_color(c(BORDER))
                .child(badge(format!("K{site}"), CYAN))
                .child(label(product.label()))
                .child(muted("Latest scan • refreshes with each data update"))
                .child(div().flex_1())
                .child(button_small("site-refresh", "Refresh now").on_click(cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |s, cx| s.refresh_data(cx));
                })))
                .child(button_small("to-composite", "Back to composite loop").on_click(cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |s, _| s.update_live(|l| l.radar.source = RadarSource::Composite));
                })))
                .into_any_element();
        }
        self.radar_timeline(cx).into_any_element()
    }

    fn data_timeline(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let field = state.live.data.field.unwrap_or(DataField::Futurecast);
        let fixed = field.fixed_window();
        let model_mode = field == DataField::Futurecast;
        let product = state.live.data.model_product;
        let source = product.source();
        let model = if model_mode { crate::program::forecast_model(&state.live, &state.data) } else { None };
        let grid = if model_mode { None } else { state.data.grid.as_ref().filter(|grid| grid.covers(state.live.view)) };
        let span = crate::program::forecast_span(&state.live, &state.data);
        let hour = state.live.data.hour.clamp(0.0, span as f32);
        let playing = state.data_playing;
        let clock_24h = state.settings.clock_24h;
        let ready = span > 0 && crate::program::forecast_ready(&state.live, &state.data);
        let status = if model_mode {
            state.model_error[source.index()].clone().or_else(|| match model {
                Some(m) if m.frame(0.0, state.live.data.loop_hours, product).is_none() => Some("Forecast field unavailable for this range".into()),
                Some(m) if m.skipped > 0 => Some(format!("{} forecast frames unavailable", m.skipped)),
                None => Some(format!("Loading {} forecast…", source.label())),
                _ => None,
            })
        } else if grid.is_none() {
            Some(match crate::grid::noaa_progress() {
                Some((done, total)) => format!("Loading NOAA GFS: {done} of {total} files…"),
                None if state.health.grid.ok => "Loading forecast…".into(),
                None => state.health.grid.message.clone(),
            })
        } else if span == 0 && fixed.is_none() {
            Some("No forecast times available for this layer".into())
        } else {
            None
        };
        let date_label = |time: i64, full: bool| {
            chrono::DateTime::from_timestamp(time, 0).map(|date| {
                let format = match (full, clock_24h) {
                    (true, true) => "%a %b %-d, %H:%M",
                    (true, false) => "%a %b %-d, %-I:%M %p",
                    (false, true) => "%a %H:%M",
                    (false, false) => "%a %-I:%M %p",
                };
                date.with_timezone(&chrono::Local).format(format).to_string()
            })
        };
        let time_at = |offset: f32| {
            model.and_then(|m| m.frame(offset, state.live.data.loop_hours, product)).map(|frame| frame.time)
                .or_else(|| grid.map(|g| g.time_at(g.time_index(offset))))
        };
        let current = ready.then(|| time_at(hour).and_then(|time| date_label(time, true))).flatten();
        let end = ready.then(|| time_at(span as f32).and_then(|time| date_label(time, true))).flatten();
        let run_label = model.and_then(|m| chrono::DateTime::from_timestamp(m.run, 0))
            .map(|t| format!("{} run · {}", source.label(), t.format("%b %-d %HZ")));
        let marks: Vec<f32> = if !ready {
            Vec::new()
        } else if let Some(model) = model {
            let hours: Vec<_> = model.timeline_hours(span, product).collect();
            let stride = hours.len().saturating_sub(1).div_ceil(5).max(1);
            hours.iter().step_by(stride).copied()
                .chain(hours.last().copied().filter(|_| (hours.len() - 1) % stride != 0)).map(|h| h as f32).collect()
        } else {
            // Map grids: "Now", then whole clock hours on the same times every day.
            let now = chrono::Utc::now().timestamp();
            std::iter::once(0.0).chain(grid_marks(now, span)).collect()
        };
        let selected_time = time_at(hour);
        let ticks = marks.into_iter().enumerate().filter_map(|(i, h)| {
            let time = time_at(h)?;
            let label = if i == 0 && model.is_none() { "Now".to_string() } else { date_label(time, model.is_some() && span > 24)? };
            let on = selected_time.is_some_and(|t| (t - time).abs() < 120);
            Some(chip(("forecast-tick", i), label, on).on_click(cx.listener(move |this, _, _, cx| {
                this.app.update(cx, |s, cx| { s.set_data_hour(h); cx.notify(); });
            })))
        }).collect::<Vec<_>>();
        // Day steps (map grids): the same time of day, one day earlier or later.
        let day_steps = ready && model.is_none() && span > 24;

        col().w_full().flex_shrink_0().gap(px(5.)).px(px(10.)).py(px(6.))
            .bg(c(PANEL)).border_t_1().border_color(c(BORDER))
            .child(row().items_center().gap(px(6.)).flex_wrap()
                .child(badge(if model_mode { product.broadcast_title() } else { field.broadcast_title() }, CYAN))
                .when_some(fixed, |d, range| d.child(muted(range)))
                .when_some(run_label, |d, label| d.child(muted(label)))
                .when_some(current, |d, label| d.child(badge(label, CYAN)))
                .when_some(status, |d, label| d.child(muted(label)))
                .when(ready, |d| d.child(button_primary("data-play", if playing { "Pause" } else { "Play" }).on_click(cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |s, cx| { s.data_playing = !s.data_playing; cx.notify(); });
                }))))
                .when(fixed.is_none(), |d| d.child(chip("forecast-range-menu", "Range ▾", self.open_dd == Some(Dd::Range)).on_click(cx.listener(|this, _, _, cx| {
                    let range_open = this.open_dd == Some(Dd::Range);
                    this.open_menu = None;
                    this.open_dd = (!range_open).then_some(Dd::Range);
                    cx.notify();
                }))))
                .child(button_small("data-off", "Data off").on_click(cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |s, _| { s.data_playing = false; s.update_live(|l| l.data.field = None); });
                }))))
            .when(ready, |d| d
                .child(row().items_center().gap(px(8.))
                    .child(div().flex_1().min_w_0().child(slider("forecast-scrub", hour, 0.0, span as f32, cx, |this, value, _, cx| {
                        this.app.update(cx, |s, cx| { s.set_data_hour(value.round()); cx.notify(); });
                    })))
                    .when_some(end, |d, end| d
                        .child(muted(format!("Through {end}")))
                        .child(button_small("forecast-last", "Last frame").on_click(cx.listener(move |this, _, _, cx| {
                            this.app.update(cx, |s, cx| {
                                s.set_data_hour(span as f32);
                                cx.notify();
                            });
                        })))))
                .child(row().gap(px(4.)).flex_wrap()
                    .when(day_steps, |d| d.child(button_small("forecast-day-back", "◀ Day").on_click(cx.listener(move |this, _, _, cx| {
                        this.app.update(cx, |s, cx| { s.set_data_hour((hour - 24.0).max(0.0)); cx.notify(); });
                    }))))
                    .children(ticks)
                    .when(day_steps, |d| d.child(button_small("forecast-day-next", "Day ▶").on_click(cx.listener(move |this, _, _, cx| {
                        this.app.update(cx, |s, cx| { s.set_data_hour((hour + 24.0).min(span as f32)); cx.notify(); });
                    }))))))
    }


    fn radar_timeline(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let frames = state.radar_frames.clone();
        let index = state.radar_index;
        let playing = state.playing;
        let latest_mode = state.live.radar.mode == RadarMode::Latest;
        let replay = state.live.is_radar_replay();
        let nowcast_start = if replay { usize::MAX } else { state.data.radar.as_ref().map(|r| r.radar.past.len()).unwrap_or(usize::MAX) };
        let stride = if replay { frames.len().div_ceil(12).max(1) } else { 1 };
        let clock_24h = state.settings.clock_24h;

        let ticks = frames
            .iter()
            .enumerate()
            .filter(|(i, _)| i % stride == 0 || *i == index || *i + 1 == frames.len())
            .map(|(i, t)| {
                let label = chrono::DateTime::from_timestamp(*t, 0)
                    .map(|d| {
                        let local = d.with_timezone(&chrono::Local);
                        if clock_24h {
                            local.format("%H:%M").to_string()
                        } else {
                            local.format("%-I:%M").to_string()
                        }
                    })
                    .unwrap_or_default();
                let active = !latest_mode && i == index;
                let future = i >= nowcast_start;
                div()
                    .id(("frame", i))
                    .flex_1()
                    .min_w(px(36.))
                    .h(px(34.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .bg(if active { ca(CYAN, 1.0) } else if future { ca(AMBER, 0.15) } else { ca(PANEL_2, 1.0) })
                    .text_size(px(11.))
                    .text_color(if active { c(0xFFFFFF) } else if future { c(AMBER) } else { c(MUTED) })
                    .hover(|s| s.bg(c(0x2A3449)))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.playing = false;
                            s.live.radar.mode = RadarMode::Loop;
                            s.radar_index = i;
                            s.sync_map();
                            cx.notify();
                        });
                    }))
            })
            .collect::<Vec<_>>();

        row()
            .w_full()
            .gap(px(8.))
            .px(px(10.))
            .py(px(8.))
            .bg(c(PANEL))
            .border_t_1()
            .border_color(c(BORDER))
            .child(button_small("prev-frame", "◀").on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, cx| {
                    s.playing = false;
                    s.step_radar(-1);
                    cx.notify();
                });
            })))
            .child(
                if playing { button_primary("play", "❚❚ Pause") } else { button_primary("play", "▶ Play") }
                    .h(px(28.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.playing = !s.playing;
                            s.live.radar.mode = RadarMode::Loop;
                            s.sync_map();
                            cx.notify();
                        });
                    })),
            )
            .child(button_small("next-frame", "▶").on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, cx| {
                    s.playing = false;
                    s.step_radar(1);
                    cx.notify();
                });
            })))
            .child(row().flex_1().gap(px(3.)).children(ticks).when(frames.is_empty(), |d| {
                d.child(muted("Waiting for radar frames from OpenWX Relay…"))
            }))
            .child(chip("latest", "Latest", latest_mode).on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, cx| {
                    s.update_live(|l| {
                        l.radar.mode = if l.radar.mode == RadarMode::Latest { RadarMode::Loop } else { RadarMode::Latest }
                    });
                    cx.notify();
                });
            })))
    }
}

impl Render for MapStudio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.win_h = f32::from(window.viewport_size().height);
        let image = self.frames.read(cx).map.clone();
        let state = self.app.read(cx);
        let live = state.live.clone();
        let zoom = live.view.zoom;
        let engine = state.engine.status();
        let product_badge = if let Some(field) = live.data.field {
            field.label().to_string()
        } else if live.radar.enabled && live.radar.source == RadarSource::Site {
            format!("K{} {}", live.radar.resolved_site(live.view.lat, live.view.lon), live.radar.product.short())
        } else {
            if live.radar.enabled { palette_label(&live.radar.palette).to_string() } else { "Basemap".to_string() }
        };
        let callout = live.selected_alert.is_some();
        let cursor = match self.tool {
            Tool::Pan => CursorStyle::OpenHand,
            Tool::Arrange => CursorStyle::PointingHand,
            _ => CursorStyle::Crosshair,
        };

        let tab_strip = self.tab_strip(cx);
        // The active tool's options (label text, colour, width, front type) sit right under the
        // ribbon whenever the tool has any.
        let tool_options = (matches!(self.tool, Tool::Pen | Tool::Arrow | Tool::Circle | Tool::Area | Tool::Text | Tool::Marker | Tool::Front)
            && self.open_menu != Some(MapMenu::Draw))
            .then(|| self.toolbar(true, cx).into_any_element());
        let menu_bar = self.menu_bar(cx);
        let ribbon = self.ribbon(cx);
        let menu_content = match self.open_menu {
            Some(MapMenu::Map) => Some(col().w_full().child(self.mode_bar(cx)).child(self.layers_menu(cx)).into_any_element()),
            Some(MapMenu::Draw) => Some(col().w_full().child(self.toolbar(false, cx)).child(self.raise_menu(&live, cx)).into_any_element()),
            Some(MapMenu::Forecast) => Some(self.forecast_menu(&live, cx).into_any_element()),
            Some(MapMenu::OnAir) => Some(self.onair_drawer(cx).into_any_element()),
            Some(MapMenu::PastRadar) => Some(self.replay_drawer(&live, cx).into_any_element()),
            Some(MapMenu::Export) => Some(self.export_drawer(&live, cx).into_any_element()),
            Some(MapMenu::Show) => Some(self.show_menu(&live, cx).into_any_element()),
            None => None,
        };
        let menu_height = (f32::from(window.viewport_size().height) * 0.38).clamp(80., 300.);
        let show_info = self.info_open && f32::from(window.viewport_size().width) >= 1100.0;
        let hint = (self.tool != Tool::Pan).then(|| self.tool.hint());
        let timeline = self.timeline(cx);
        let pending_raise = matches!(self.drag, Some(Drag::Raise { tracing: false, .. }));
        let status = row().w_full().flex_shrink_0().gap(px(6.)).flex_wrap().px(px(10.)).py(px(3.))
            .bg(c(PANEL)).border_t_1().border_color(c(BORDER))
            .child(badge(format!("{product_badge} · zoom {zoom:.1}"), MUTED))
            .when(callout, |d| d.child(badge("Call-out on", AMBER)))
            .when_some(engine.error.clone(), |d, error| d.child(badge(format!("Map error: {error}"), RED)))
            .when(!engine.ready && engine.error.is_none(), |d| d.child(badge("Map starting…", AMBER)))
            .when_some(hint.filter(|_| !pending_raise), |d, text| d.child(muted(text)))
            .when(pending_raise, |d| d.child(muted("The dotted edge will close the outline."))
                .child(button_small("use-raise-outline", "Use outline").on_click(cx.listener(|this, _, _, cx| this.finish_raise(None, cx))))
                .child(button_small("discard-raise-outline", "Discard outline").on_click(cx.listener(|this, _, _, cx| this.cancel_raise(cx)))))
            .when_some(self.cursor_text.clone(), |d, text| d.child(muted(text)));
        let inspect_card = self.inspect.clone().map(|card| {
            let app = self.app.clone();
            let (clock, selected) = {
                let s = self.app.read(cx);
                (s.settings.clock_24h, s.live.selected_alert.clone())
            };
            crate::ui::map_inspect::card(&card, &app, clock, selected, cx, |this: &mut Self, cx| {
                this.inspect = None;
                cx.notify();
            })
        });

        let left = self.layers_open.then(|| self.left_panel(cx));
        let info = show_info.then(|| self.info_panel(cx));
        col()
            .size_full()
            .bg(c(BG))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .child(menu_bar)
            .child(ribbon)
            .children(tool_options)
            .child(tab_strip)
            .children(menu_content.map(|content| {
                div().id("map-menu-content").w_full().flex_shrink_0().max_h(px(menu_height))
                    .overflow_y_scroll().bg(c(PANEL)).border_b_1().border_color(c(BORDER)).child(content)
            }))
            .child(row().w_full().flex_1().min_h_0().items_start()
            .children(left)
            .child(
                col()
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .child(
                        div()
                            .id("map-viewport")
                            .relative()
                            .flex_1()
                            .w_full()
                            .min_h_0()
                            .cursor(cursor)
                            .child(frame_canvas_with_trace(image, self.slot.clone(), match &self.drag {
                                Some(Drag::Raise { trace, .. }) => trace.clone(),
                                _ => Vec::new(),
                            }, pending_raise).size_full())
                            .children(inspect_card)
                            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
                            .on_mouse_move(cx.listener(Self::on_move))
                            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up))
                            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, event: &MouseUpEvent, _, cx| {
                                if matches!(this.drag, Some(Drag::Raise { tracing: true, .. })) { this.finish_raise(Some(event.position), cx); }
                            }))
                            .on_scroll(cx.listener(Self::on_scroll)),
                    )
                    .child(status)
                    .child(timeline),
            )
            .children(info))
    }
}

/// Measurements as CSV (one row per mark) plus GeoJSON (the shapes) in Exports.
fn export_measurements(marks: &[crate::map::scene::Annotation]) -> anyhow::Result<std::path::PathBuf> {
    anyhow::ensure!(!marks.is_empty(), "draw a ruler, or Pen / Area with Measure on, first");
    let dir = crate::library::exports_dir();
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let mut csv = String::from("kind,points,length_mi,area_sq_mi,elevation_start_ft,elevation_end_ft\n");
    let mut features = Vec::new();
    for a in marks {
        let polygon = a.kind == AnnotationKind::Polygon;
        let length = crate::map::geo::path_miles(&a.points);
        let area = if polygon { crate::map::geo::polygon_sq_mi(&a.points) } else { 0.0 };
        let (e0, e1) = a.elevation_ft.map(|(a, b)| (format!("{a:.0}"), format!("{b:.0}"))).unwrap_or_default();
        csv.push_str(&format!("{:?},{},{length:.2},{area:.1},{e0},{e1}\n", a.kind, a.points.len()));
        let coords: Vec<[f64; 2]> = a.points.iter().map(|(lat, lon)| [*lon, *lat]).collect();
        let geometry = if polygon {
            let mut ring = coords.clone();
            if let Some(first) = coords.first() {
                ring.push(*first);
            }
            serde_json::json!({ "type": "Polygon", "coordinates": [ring] })
        } else {
            serde_json::json!({ "type": "LineString", "coordinates": coords })
        };
        features.push(serde_json::json!({ "type": "Feature", "geometry": geometry, "properties": { "kind": format!("{:?}", a.kind), "length_mi": length, "area_sq_mi": area } }));
    }
    std::fs::write(dir.join(format!("Measurements_{stamp}.geojson")), serde_json::json!({ "type": "FeatureCollection", "features": features }).to_string())?;
    let path = dir.join(format!("Measurements_{stamp}.csv"));
    std::fs::write(&path, csv)?;
    Ok(path)
}

#[cfg(test)]
mod timeline_tests {
    use chrono::Timelike;

    /// Grid quick-jump chips land on whole local hours at a fixed step, never "x:19".
    #[test]
    fn grid_marks_on_whole_hours() {
        let now = 1_794_532_740; // some minute past the hour
        for (span, step) in [(24u32, 3u32), (48, 6), (72, 12), (168, 24)] {
            let marks = super::grid_marks(now, span);
            assert!(!marks.is_empty(), "span {span}");
            for m in marks {
                let t = now + (m * 3600.0).round() as i64;
                let local = chrono::DateTime::from_timestamp(t, 0).unwrap().with_timezone(&chrono::Local);
                assert_eq!(local.minute(), 0, "span {span}");
                if step == 24 { assert_eq!(local.hour(), 12) } else { assert_eq!(local.hour() % step, 0, "span {span}") }
                assert!(m > 0.0 && m <= span as f32);
            }
        }
    }
}

#[cfg(test)]
mod find_tests {
    /// CWXB-68: the words the owner tried all find something beyond Map Studio.
    #[test]
    fn find_reaches_windows_settings_and_templates() {
        for word in ["theme", "sponsor", "settings", "winter studio", "7-day", "report a bug", "obs", "publishing", "community", "forecast editor", "history", "audio"] {
            let hits = super::MapStudio::find_matches(word);
            assert!(!hits.is_empty(), "nothing found for {word:?}");
        }
        let first = super::MapStudio::find_matches("forecast editor");
        assert_eq!(first[0].0, "Forecast editor");
        // OBS lives on the Integrations page; it must be near the top, not under 180 templates.
        let obs = super::MapStudio::find_matches("obs");
        assert!(obs.iter().take(3).any(|h| h.0 == "Settings: Integrations"), "{:?}", obs.iter().take(5).map(|h| &h.0).collect::<Vec<_>>());
    }
}
