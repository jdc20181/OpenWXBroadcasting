//! Map Studio navigation, as the owner asked for it (2026-11-11):
//! - a VS Code-style menu bar (File, Map, View, Layers, Tools, On air, Show, Help) whose menus
//!   are real vertical drop-down lists: named commands, checkmarks, separators, shortcuts and
//!   › submenus;
//! - a Word-style ribbon under it: category tabs (Home, Radar, Forecast, Outlooks, Draw,
//!   On air, Compare), each showing its controls in labelled groups.
//! Ribbon buttons with ▾ open the same vertical menus. The side panel follows the ribbon tab.

use std::{rc::Rc, time::{Duration, Instant}};

use super::*;
use crate::ui::icons::ri;

type Run = Rc<dyn Fn(&mut MapStudio, &mut Context<MapStudio>)>;

/// One entry in a drop-down menu.
pub(super) enum MItem {
    Act { label: String, key: &'static str, check: Option<bool>, run: Run },
    Sep,
    Head(&'static str),
    Sub { label: String, items: Vec<MItem> },
}

fn act(label: impl Into<String>, key: &'static str, f: impl Fn(&mut MapStudio, &mut Context<MapStudio>) + 'static) -> MItem {
    MItem::Act { label: label.into(), key, check: None, run: Rc::new(f) }
}

fn check(label: impl Into<String>, on: bool, f: impl Fn(&mut MapStudio, &mut Context<MapStudio>) + 'static) -> MItem {
    MItem::Act { label: label.into(), key: "", check: Some(on), run: Rc::new(f) }
}

fn sub(label: impl Into<String>, items: Vec<MItem>) -> MItem {
    MItem::Sub { label: label.into(), items }
}

/// Flip a map setting (and redraw).
fn live(f: impl Fn(&mut MapScene) + 'static) -> impl Fn(&mut MapStudio, &mut Context<MapStudio>) + 'static {
    move |this, cx| {
        this.app.update(cx, |s, cx| {
            s.update_live(&f);
            cx.notify();
        });
    }
}

// Wide enough for the longest items ("Reset tab to the on-air map", model descriptions).
const MENU_W: f32 = 320.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RibbonTab {
    Home,
    Radar,
    Forecast,
    Outlooks,
    Draw,
    OnAir,
    Compare,
}

impl RibbonTab {
    pub(super) const ALL: [RibbonTab; 7] = [RibbonTab::Home, RibbonTab::Radar, RibbonTab::Forecast, RibbonTab::Outlooks, RibbonTab::Draw, RibbonTab::OnAir, RibbonTab::Compare];

    fn label(self) -> &'static str {
        match self {
            RibbonTab::Home => "Home",
            RibbonTab::Radar => "Radar",
            RibbonTab::Forecast => "Forecast",
            RibbonTab::Outlooks => "Outlooks",
            RibbonTab::Draw => "Draw",
            RibbonTab::OnAir => "On air",
            RibbonTab::Compare => "Compare",
        }
    }

    /// The ribbon tab for a map mode (the ribbon follows the map; the operator can still pick
    /// any tab).
    pub(super) fn for_mode(mode: ViewMode) -> RibbonTab {
        match mode {
            ViewMode::LiveRadar | ViewMode::SingleSite | ViewMode::Satellite => RibbonTab::Radar,
            ViewMode::Futurecast | ViewMode::DataMaps => RibbonTab::Forecast,
            ViewMode::Outlook => RibbonTab::Outlooks,
        }
    }
}

// ---- Ribbon pieces ----------------------------------------------------------------------------

/// Large ribbon button: icon over a short label.
fn rbig(id: impl Into<ElementId>, icon: &'static str, text: impl Into<SharedString>, active: bool) -> Stateful<Div> {
    let color = if active { CYAN } else { TEXT };
    div()
        .id(id)
        .flex()
        .flex_col()
        .items_center()
        .justify_start()
        .gap(px(3.))
        // Grows a little for long words ("Ground Temperatur|e" wrapped onto a hidden line).
        .min_w(px(64.))
        .max_w(px(88.))
        .px(px(3.))
        .h(px(58.))
        .pt(px(5.))
        .flex_shrink_0()
        .rounded(px(5.))
        .cursor_pointer()
        .text_size(px(11.))
        .line_height(px(13.))
        .text_center()
        .text_color(c(color))
        .when(active, |d| d.bg(ca(CYAN, 0.16)).border_1().border_color(ca(CYAN, 0.55)))
        .when(!active, |d| d.border_1().border_color(transparent_black()).hover(|s| s.bg(c(PANEL_2))))
        .child(ri(icon, 20., color))
        // Two lines at most ("Dubois County Weather ▾" wrapped onto the group caption).
        .child(div().w_full().line_clamp(2).child(text.into()))
}

/// Small ribbon row with a checkbox.
fn rcheck(id: impl Into<ElementId>, text: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(19.))
        .px(px(4.))
        .rounded(px(3.))
        .cursor_pointer()
        .whitespace_nowrap()
        .text_size(px(12.))
        .text_color(c(TEXT))
        .hover(|s| s.bg(c(PANEL_2)))
        .child(
            div()
                .w(px(12.))
                .h(px(12.))
                .flex_shrink_0()
                .rounded(px(2.))
                .border_1()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(10.))
                .when(on, |d| d.bg(c(CYAN)).border_color(c(CYAN)).text_color(c(0xFFFFFF)).child("✓"))
                .when(!on, |d| d.border_color(c(MUTED))),
        )
        .child(text.into())
}

/// Small ribbon button with an icon.
/// Warnings / Watches / Advisories / Statements switches (each turns alerts on if needed).
fn alert_switches(prefix: &'static str, l: &MapScene, cx: &mut Context<MapStudio>) -> Div {
    let kinds = [
        ("warn", "Warnings", l.alert_kinds.warnings, 0u8),
        ("watch", "Watches", l.alert_kinds.watches, 1),
        ("adv", "Advisories", l.alert_kinds.advisories, 2),
        ("stmt", "Statements", l.alert_kinds.statements, 3),
    ];
    // Three rows fit a ribbon; the fourth wraps to a second column (it used to cover the
    // group's name).
    let mut row = div().flex().gap(px(6.));
    let mut col = stack();
    for (key, label, on, k) in kinds {
        if k == 3 {
            row = row.child(col);
            col = stack();
        }
        let get = move |m: &MapScene| match k { 0 => m.alert_kinds.warnings, 1 => m.alert_kinds.watches, 2 => m.alert_kinds.advisories, _ => m.alert_kinds.statements };
        col = col.child(rcheck(SharedString::from(format!("{prefix}-{key}")), label, l.warnings && on).on_click(cx.listener(move |this, _, _, cx| {
            this.app.update(cx, |s, cx| {
                s.update_live(|m| {
                    let next = !(m.warnings && get(m));
                    match k { 0 => m.alert_kinds.warnings = next, 1 => m.alert_kinds.watches = next, 2 => m.alert_kinds.advisories = next, _ => m.alert_kinds.statements = next }
                    m.warnings = m.warnings || next;
                });
                cx.notify();
            });
        })));
    }
    row.child(col)
}

fn rsmall(id: impl Into<ElementId>, icon: &'static str, text: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(19.))
        .px(px(4.))
        .rounded(px(3.))
        .cursor_pointer()
        .whitespace_nowrap()
        .text_size(px(12.))
        .text_color(c(TEXT))
        .hover(|s| s.bg(c(PANEL_2)))
        .child(ri(icon, 14., MUTED))
        .child(text.into())
}

/// A ribbon group: its controls with the group's name underneath, and a divider.
fn group(name: &'static str, body: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .h_full()
        .flex_shrink_0()
        .px(px(8.))
        .border_r_1()
        .border_color(c(BORDER))
        .child(div().flex().flex_1().items_start().gap(px(2.)).child(body))
        .child(div().text_size(px(11.)).text_color(c(DIM)).text_center().pb(px(3.)).child(name))
}

/// A column of small rows (three fit a ribbon).
fn stack() -> Div {
    div().flex().flex_col().gap(px(1.)).pt(px(2.))
}

impl MapStudio {
    // ---- Drop-down menus --------------------------------------------------------------------

    fn close_menus(&mut self) {
        self.menu_open = None;
        self.menu_sub = None;
    }

    /// Rows of a menu. Top-level rows close any open submenu when hovered; a submenu opens to
    /// the right of its row.
    fn menu_rows(&mut self, items: Vec<MItem>, nested: bool, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let open_sub = self.menu_sub;
        items
            .into_iter()
            .enumerate()
            .map(|(i, item)| match item {
                MItem::Sep => div().h(px(1.)).mx(px(8.)).my(px(4.)).bg(c(BORDER)).into_any_element(),
                MItem::Head(text) => div().px(px(12.)).pt(px(6.)).pb(px(2.)).text_size(px(11.)).text_color(c(DIM)).child(text).into_any_element(),
                MItem::Act { label, key, check, run } => {
                    let row_id = if nested { ElementId::from(("menu-sub-item", i)) } else { ElementId::from(("menu-item", i)) };
                    div()
                        .id(row_id)
                        .flex()
                        .items_center()
                        .h(px(26.))
                        .px(px(8.))
                        .mx(px(4.))
                        .rounded(px(4.))
                        .cursor_pointer()
                        .text_size(px(13.))
                        .text_color(c(TEXT))
                        .whitespace_nowrap()
                        .hover(|s| s.bg(ca(CYAN, 0.22)))
                        .child(div().w(px(18.)).flex_shrink_0().text_color(c(CYAN)).child(if check == Some(true) { "✓" } else { "" }))
                        .child(div().flex_1().min_w_0().overflow_hidden().child(label))
                        .child(div().pl(px(16.)).text_size(px(12.)).text_color(c(DIM)).child(key))
                        .when(!nested, |d| {
                            d.on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                                if *hovered && this.menu_sub.is_some() {
                                    this.menu_sub = None;
                                    cx.notify();
                                }
                            }))
                        })
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.close_menus();
                            this.menu_closed = None;
                            run(this, cx);
                            cx.notify();
                        }))
                        .into_any_element()
                }
                MItem::Sub { label, items } => {
                    let open = open_sub == Some(i) && !nested;
                    let mut row = div()
                        .id(("menu-sub", i))
                        .relative()
                        .flex()
                        .items_center()
                        .h(px(26.))
                        .px(px(8.))
                        .mx(px(4.))
                        .rounded(px(4.))
                        .cursor_pointer()
                        .text_size(px(13.))
                        .text_color(c(TEXT))
                        .whitespace_nowrap()
                        .when(open, |d| d.bg(ca(CYAN, 0.22)))
                        .when(!open, |d| d.hover(|s| s.bg(ca(CYAN, 0.22))))
                        .child(div().w(px(18.)).flex_shrink_0())
                        .child(div().flex_1().child(label))
                        .child(div().pl(px(16.)).text_color(c(DIM)).child("›"))
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            if *hovered && this.menu_sub != Some(i) {
                                this.menu_sub = Some(i);
                                cx.notify();
                            }
                        }))
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.menu_sub = Some(i);
                            cx.notify();
                        }));
                    if open {
                        let rows = self.menu_rows(items, true, cx);
                        row = row.child(
                            div()
                                .absolute()
                                .left(px(MENU_W - 14.))
                                .top(px(-5.))
                                .w(px(MENU_W))
                                .py(px(4.))
                                .rounded(px(6.))
                                .bg(c(PANEL))
                                .border_1()
                                .border_color(c(BORDER))
                                .shadow_lg()
                                .children(rows),
                        );
                    }
                    row.into_any_element()
                }
            })
            .collect()
    }

    /// A trigger with its drop-down menu (vertical list) floating under it when open.
    fn with_menu(&mut self, id: &'static str, trigger: Stateful<Div>, hover_switch: bool, cx: &mut Context<Self>, items: impl FnOnce(&mut Self, &mut Context<Self>) -> Vec<MItem>) -> Div {
        let open = self.menu_open == Some(id);
        let trigger = trigger
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                // The outside-click that just closed this menu also lands here: keep it closed.
                if this.menu_closed.is_some_and(|(m, at)| m == id && at.elapsed() < Duration::from_millis(300)) {
                    this.menu_closed = None;
                    return;
                }
                this.open_dd = None;
                this.menu_sub = None;
                this.menu_open = if this.menu_open == Some(id) { None } else { Some(id) };
                cx.notify();
            }))
            .when(hover_switch, |d| {
                d.on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    // Like a desktop menu bar: with one menu open, hovering another opens it.
                    if *hovered && this.menu_open.is_some() && this.menu_open != Some(id) && this.menu_open.is_some_and(|m| m.starts_with("mb-")) {
                        this.menu_open = Some(id);
                        this.menu_sub = None;
                        cx.notify();
                    }
                }))
            });
        let mut d = div().relative().flex_shrink_0().child(trigger);
        if open {
            let list = items(self, cx);
            // Long menus (Layers) scroll in a short window. Menus with › submenus don't: the
            // scroll would clip the submenu that opens to the side (they are short anyway).
            let scroll = !list.iter().any(|m| matches!(m, MItem::Sub { .. }));
            let max_h = (self.win_h - 90.).max(200.);
            let rows = self.menu_rows(list, false, cx);
            d = d.child(
                deferred(
                    anchored().snap_to_window_with_margin(px(8.)).child(
                        div()
                            .id(("menu-panel", id.len()))
                            .occlude()
                            .mt(px(2.))
                            .w(px(MENU_W))
                            .py(px(4.))
                            .rounded(px(6.))
                            .bg(c(PANEL))
                            .border_1()
                            .border_color(c(BORDER))
                            .shadow_lg()
                            .when(scroll, |d| d.max_h(px(max_h)).overflow_y_scroll())
                            .on_mouse_down_out(cx.listener(move |this, _, _, cx| {
                                if this.menu_open == Some(id) {
                                    this.close_menus();
                                    this.menu_closed = Some((id, Instant::now()));
                                    cx.notify();
                                }
                            }))
                            .children(rows),
                    ),
                )
                .with_priority(4),
            );
        }
        d
    }

    // ---- Menu bar ---------------------------------------------------------------------------

    pub(super) fn menu_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let names: [(&'static str, &'static str); 8] = [
            ("mb-file", "File"),
            ("mb-map", "Map"),
            ("mb-view", "View"),
            ("mb-layers", "Layers"),
            ("mb-tools", "Tools"),
            ("mb-onair", "On air"),
            ("mb-show", "Show"),
            ("mb-help", "Help"),
        ];
        let mut bar = div().flex().items_center().w_full().flex_shrink_0().h(px(30.)).px(px(6.)).gap(px(1.)).bg(c(BG)).border_b_1().border_color(c(BORDER));
        for (id, name) in names {
            let open = self.menu_open == Some(id);
            let trigger = div()
                .id(id)
                .px(px(10.))
                .h(px(24.))
                .flex()
                .items_center()
                .rounded(px(4.))
                .cursor_pointer()
                .text_size(px(13.))
                .text_color(c(TEXT))
                .when(open, |d| d.bg(c(PANEL_2)))
                .when(!open, |d| d.hover(|s| s.bg(c(PANEL_2))))
                .child(name);
            let menu = self.with_menu(id, trigger, true, cx, move |this, cx| this.top_menu(id, cx));
            bar = bar.child(menu);
        }
        let results = (self.find_query.trim().chars().count() >= 2).then(|| {
            // Pinned under the box's left edge; snapping keeps it inside the window.
            div().absolute().left_0().top(px(24.)).child(
                deferred(anchored().anchor(Anchor::TopLeft).snap_to_window_with_margin(px(8.)).child(self.find_results(cx))).with_priority(5),
            )
        });
        bar.child(div().flex_1())
            .child(
                div().relative().w(px(300.))
                    .child(div().flex().items_center().gap(px(6.)).w_full().child(ri("search-line", 14., MUTED)).child(div().flex_1().child(Input::new(&self.find_input))))
                    .children(results),
            )
            .child(div().w(px(8.)))
    }

    fn top_menu(&mut self, id: &'static str, cx: &mut Context<Self>) -> Vec<MItem> {
        let state = self.app.read(cx);
        let l = state.live.clone();
        let mode = Self::view_mode(&l);
        let work_tab = state.active_tab.is_some();
        let pending = state.preview_pending();
        let bookmarks = state.settings.bookmarks.clone();
        let overlays: Vec<(String, String)> = state.graphics.iter().filter(|g| g.is_overlay()).map(|g| (g.id.clone(), g.name.clone())).collect();
        let has_logo = state.settings.brand.logo_media.is_some();
        let loopable = l.data.field.is_some_and(|f| f.fixed_window().is_none()) || l.radar_replay.is_some() || l.site_replay.is_some();
        match id {
            "mb-file" => vec![
                act("New map tab", "", |this, cx| this.app.update(cx, |s, cx| { s.open_work_tab(); cx.notify(); })),
                MItem::Sep,
                act(if work_tab { "Send this tab to air" } else { "Send map to air" }, "", |this, cx| this.app.update(cx, |s, cx| { s.send_tab_to_air(); cx.notify(); })),
                act(if pending { "Reset tab to the on-air map" } else { "Tab matches the on-air map" }, "", |this, cx| this.app.update(cx, |s, cx| { s.revert_map_preview(); cx.notify(); })),
                MItem::Sep,
                MItem::Head("Export"),
                act("Still image (PNG)", "", |this, cx| this.app.update(cx, |s, cx| { s.save_map_snapshot(); cx.notify(); })),
                act("Animation (GIF)", "", |this, cx| this.export_motion(true, cx)),
                act("Video (MP4)", "", |this, cx| this.export_motion(false, cx)),
                MItem::Sep,
                act("Add still to the show", "", |this, cx| this.app.update(cx, |s, cx| { s.add_map_to_show(false); cx.notify(); })),
                act(if loopable { "Add loop to the show" } else { "Add loop (needs a forecast or replay)" }, "", move |this, cx| {
                    if loopable {
                        this.app.update(cx, |s, cx| { s.add_map_to_show(true); cx.notify(); });
                    }
                }),
                MItem::Sep,
                act("Save this view", "", |this, cx| this.app.update(cx, |s, cx| {
                    let n = s.settings.bookmarks.len() + 1;
                    let view = s.live.view;
                    s.settings.bookmarks.push(Bookmark { name: format!("View {n}"), view });
                    let settings = s.settings.clone();
                    s.apply_settings(settings);
                    s.toast(ToastKind::Success, format!("Saved this view as \"View {n}\""));
                    cx.notify();
                })),
            ],
            "mb-map" => {
                let views: Vec<MItem> = std::iter::once(act("Home", "Home", |this, cx| this.app.update(cx, |s, _| { let h = s.settings.home; s.fly_to(h, 1.8); })))
                    .chain(bookmarks.into_iter().map(|b| {
                        let view = b.view;
                        act(b.name.clone(), "", move |this, cx| this.app.update(cx, |s, _| s.fly_to(view, 1.8)))
                    }))
                    .collect();
                vec![
                    // First, per docs/MAP-STUDIO-NAVIGATION.md (Map: Home · Live radar …).
                    act("Home", "Home", |this, cx| this.app.update(cx, |s, _| { let h = s.settings.home; s.fly_to(h, 1.8); })),
                    MItem::Sep,
                    check("Live radar", mode == ViewMode::LiveRadar && l.radar.enabled, |this, cx| this.set_view_mode(ViewMode::LiveRadar, cx)),
                    check("Single site radar", mode == ViewMode::SingleSite, |this, cx| this.set_view_mode(ViewMode::SingleSite, cx)),
                    check("Satellite", mode == ViewMode::Satellite, |this, cx| this.set_view_mode(ViewMode::Satellite, cx)),
                    check("Forecast", matches!(mode, ViewMode::Futurecast | ViewMode::DataMaps), |this, cx| this.set_view_mode(ViewMode::Futurecast, cx)),
                    check("Outlooks", mode == ViewMode::Outlook, |this, cx| this.show_outlook(1, "categorical", cx)),
                    MItem::Sep,
                    sub("Radar history", crate::history::RADAR_REPLAY_HOURS.into_iter().map(|h| {
                        act(format!("Past {h} hours"), "", move |this, cx| this.app.update(cx, |s, cx| { s.start_radar_replay(h); cx.notify(); }))
                    }).chain([
                        MItem::Sep,
                        act("Past 3 hours of site velocity", "", |this, cx| this.app.update(cx, |s, cx| s.start_site_radar_replay(cx))),
                        act("Event history…", "", |this, cx| this.app.update(cx, |s, cx| { s.modal = crate::app::Modal::History; cx.notify(); })),
                    ]).collect()),
                    MItem::Sep,
                    act("Go to a place…", "", |this, cx| { this.ribbon = RibbonTab::Home; cx.notify(); }),
                    sub("Saved views", views),
                    check("Recent places", self.show_history, |this, cx| { this.show_history = !this.show_history; this.ribbon = RibbonTab::Home; cx.notify(); }),
                    MItem::Sep,
                    act("Refresh radar", "F5", |this, cx| this.app.update(cx, |s, cx| { s.refresh_radar(cx); cx.notify(); })),
                    act("Clear map", "", |this, cx| this.clear_map(cx)),
                ]
            }
            "mb-view" => vec![
                check("Side panel", self.layers_open, |this, cx| { this.layers_open = !this.layers_open; cx.notify(); }),
                check("Conditions panel", self.info_open, |this, cx| { this.info_open = !this.info_open; cx.notify(); }),
                check("Ribbon", self.ribbon_open, |this, cx| { this.ribbon_open = !this.ribbon_open; cx.notify(); }),
                MItem::Sep,
                MItem::Head("On the map"),
                check("Title", l.show_title, live(|m| m.show_title = !m.show_title)),
                check("Legend", l.show_legend, live(|m| m.show_legend = !m.show_legend)),
                check("Clock", l.show_clock, live(|m| m.show_clock = !m.show_clock)),
                check("Time box", l.show_timestamp, live(|m| m.show_timestamp = !m.show_timestamp)),
                check("OpenWX watermark", l.shows_watermark(), live(|m| m.watermark_visible = Some(!m.shows_watermark()))),
                check(if has_logo { "Station logo" } else { "Station logo (add one in Settings)" }, l.show_logo, live(|m| { m.watermark_visible = Some(m.shows_watermark()); m.show_logo = !m.show_logo; })),
                check("Sponsor badge", l.show_sponsor, |this, cx| this.app.update(cx, |s, cx| {
                    if !s.live.show_sponsor && s.current_sponsor().is_none() {
                        s.modal = crate::app::Modal::Sponsors;
                    } else {
                        s.update_live(|m| m.show_sponsor = !m.show_sponsor);
                    }
                    cx.notify();
                })),
                MItem::Sep,
                sub("Layout", PanelLayout::ALL.into_iter().map(|p| check(p.label(), l.layout == p, live(move |m| m.layout = p))).collect()),
                sub("Base map", Basemap::ALL.into_iter().map(|b| check(b.label(), l.basemap == b && l.theme.is_empty(), live(move |m| { m.basemap = b; m.theme.clear(); }))).collect()),
            ],
            "mb-layers" => self.layer_items(&l),
            "mb-tools" => {
                let tool = self.tool;
                let tools = |list: &[Tool]| -> Vec<MItem> {
                    list.iter().map(|t| {
                        let t = *t;
                        MItem::Act { label: t.label().to_string(), key: t.key(), check: Some(tool == t), run: Rc::new(move |this: &mut MapStudio, cx: &mut Context<MapStudio>| this.set_tool(t, cx)) }
                    }).collect()
                };
                let mut out = tools(&[Tool::Pan]);
                out.push(MItem::Sep);
                out.push(sub("Draw", tools(&[Tool::Pen, Tool::Arrow, Tool::Circle, Tool::Area, Tool::Text, Tool::Marker, Tool::Ruler])));
                out.push(sub("Weather symbols", tools(&Tool::WEATHER)));
                out.push(sub("Storm tools", tools(&[Tool::StormTrack, Tool::FreeTrack, Tool::Rings, Tool::Spotlight])));
                out.push(MItem::Sep);
                out.push(check("Raise an area (draw it)", tool == Tool::RaisedArea, |this, cx| this.set_tool(Tool::RaisedArea, cx)));
                out.push(check("Raise a county", tool == Tool::RaisedBoundary && !self.raise_state, |this, cx| { this.raise_state = false; this.set_tool(Tool::RaisedBoundary, cx); }));
                out.push(check("Raise a state", tool == Tool::RaisedBoundary && self.raise_state, |this, cx| { this.raise_state = true; this.set_tool(Tool::RaisedBoundary, cx); }));
                out.push(act("Remove raised area", "", |this, cx| {
                    this.cancel_raise(cx);
                    this.app.update(cx, |s, _| s.update_live(|m| m.annotations.retain(|a| a.kind != AnnotationKind::RaisedArea)));
                }));
                out.push(MItem::Sep);
                out.push(act("Undo last mark", "Backspace", |this, cx| this.app.update(cx, |s, _| s.update_live(|m| { m.annotations.pop(); }))));
                out.push(act("Remove all drawings", "", |this, cx| this.app.update(cx, |s, _| s.update_live(|m| m.annotations.retain(|a| a.kind == AnnotationKind::RaisedArea)))));
                out.push(MItem::Sep);
                out.push(check("Arrange on-air items", tool == Tool::Arrange, |this, cx| this.set_tool(Tool::Arrange, cx)));
                out
            }
            "mb-onair" => {
                let slot_items = |slot: usize, template: &'static str, new_label: &'static str, active: Option<String>| -> Vec<MItem> {
                    let mut v = vec![act(new_label, "", move |this, cx| { this.overlay_slot = slot; this.add_overlay_template(template, slot, cx); })];
                    if !overlays.is_empty() {
                        v.push(MItem::Sep);
                        v.push(MItem::Head("Saved overlays"));
                        for (gid, name) in &overlays {
                            let gid = gid.clone();
                            let on = active.as_deref() == Some(gid.as_str());
                            v.push(check(name.clone(), on, move |this, cx| this.app.update(cx, |s, cx| { s.set_map_overlay_slot(slot, (!on).then(|| gid.clone())); cx.notify(); })));
                        }
                    }
                    if active.is_some() {
                        v.push(MItem::Sep);
                        v.push(act("Take it off", "", move |this, cx| this.app.update(cx, |s, cx| { s.set_map_overlay_slot(slot, None); cx.notify(); })));
                    }
                    v
                };
                let brands = self.brand_items(cx);
                vec![
                    sub("Branding", brands),
                    MItem::Sep,
                    act("Title…", "", |this, cx| { this.ribbon = RibbonTab::OnAir; cx.notify(); }),
                    check("Show the title", l.show_title, live(|m| m.show_title = !m.show_title)),
                    act("Use the automatic title", "", live(|m| { m.title = "LIVE DOPPLER".into(); m.custom_title = false; })),
                    MItem::Sep,
                    sub("Lower third", slot_items(0, "lower_third", "New lower third", l.overlay_graphic.clone())),
                    sub("Crawl", slot_items(1, "alert_crawl", "New alert crawl", l.overlay_slots[0].clone())),
                    sub("Bug / sponsor", slot_items(2, "sponsor_lower_third", "New sponsor lower third", l.overlay_slots[1].clone())),
                    MItem::Sep,
                    act("Sponsors, logos and rotation…", "", |this, cx| this.app.update(cx, |s, cx| { s.modal = crate::app::Modal::Sponsors; cx.notify(); })),
                    check("Arrange on-air items", self.tool == Tool::Arrange, |this, cx| this.set_tool(Tool::Arrange, cx)),
                ]
            }
            "mb-show" => vec![
                act("Add still to the show", "", |this, cx| this.app.update(cx, |s, cx| { s.add_map_to_show(false); cx.notify(); })),
                act(if loopable { "Add loop to the show" } else { "Add loop (needs a forecast or replay)" }, "", move |this, cx| {
                    if loopable {
                        this.app.update(cx, |s, cx| { s.add_map_to_show(true); cx.notify(); });
                    }
                }),
                MItem::Sep,
                act("Open Show Builder", "", |this, cx| this.app.update(cx, |s, cx| { s.workspace = crate::app::Workspace::Show; cx.notify(); })),
                act("Open Graphics Studio", "", |this, cx| this.app.update(cx, |s, cx| { s.workspace = crate::app::Workspace::Graphics; cx.notify(); })),
            ],
            _ => vec![
                act("Find anything (box at the top right)", "", |_, _| {}),
                act("Report a bug…", "Ctrl+Shift+B", |this, cx| this.app.update(cx, |s, cx| { s.report_bug(); cx.notify(); })),
                MItem::Sep,
                MItem::Head("Keyboard"),
                act("Play / pause the loop", "Space", |_, _| {}),
                act("Step back / forward", "← →", |_, _| {}),
                act("Fly home", "Home", |_, _| {}),
                act("Undo last mark", "Backspace", |_, _| {}),
                act("Back to Pan, close menus", "Esc", |_, _| {}),
                act("Drawing tools", "letters in Tools", |_, _| {}),
            ],
        }
    }

    /// Saved brandings (Dubois County Weather, OpenWX…) to switch between.
    fn brand_items(&mut self, cx: &mut Context<Self>) -> Vec<MItem> {
        let s = &self.app.read(cx).settings;
        let active = s.current_brand_profile().name;
        s.brand_profiles_or_defaults().into_iter().map(|p| {
            let name = p.name.clone();
            check(p.name.clone(), p.name == active, move |this, cx| {
                let name = name.clone();
                this.app.update(cx, |s, cx| {
                    let mut next = s.settings.clone();
                    next.switch_brand(&name);
                    s.apply_settings(next);
                    s.toast(ToastKind::Success, format!("Now using the {name} branding"));
                    cx.notify();
                });
            })
        }).collect()
    }

    /// Layers menu: everything that can sit on the map, with checkmarks.
    fn layer_items(&self, l: &MapScene) -> Vec<MItem> {
        let mut v = vec![
            check("Radar", l.radar.enabled, live(|m| m.radar.enabled = !m.radar.enabled)),
            check("Satellite", l.satellite.enabled, live(|m| m.satellite.enabled = !m.satellite.enabled)),
            MItem::Sep,
            MItem::Head("Alerts"),
            check("Warnings", l.warnings && l.alert_kinds.warnings, live(|m| { m.alert_kinds.warnings = !(m.warnings && m.alert_kinds.warnings); m.warnings = m.warnings || m.alert_kinds.warnings; })),
            check("Watches", l.warnings && l.alert_kinds.watches, live(|m| { m.alert_kinds.watches = !(m.warnings && m.alert_kinds.watches); m.warnings = m.warnings || m.alert_kinds.watches; })),
            check("Advisories", l.warnings && l.alert_kinds.advisories, live(|m| { m.alert_kinds.advisories = !(m.warnings && m.alert_kinds.advisories); m.warnings = m.warnings || m.alert_kinds.advisories; })),
            check("Statements", l.warnings && l.alert_kinds.statements, live(|m| { m.alert_kinds.statements = !(m.warnings && m.alert_kinds.statements); m.warnings = m.warnings || m.alert_kinds.statements; })),
            check("Warning countdowns", l.warning_countdowns, live(|m| m.warning_countdowns = !m.warning_countdowns)),
            MItem::Sep,
            MItem::Head("Storms"),
            check("Lightning", l.lightning, live(|m| m.lightning = !m.lightning)),
            check("Storm icons", l.storm_cells, live(|m| m.storm_cells = !m.storm_cells)),
            check("Storm paths (30/60 min)", l.cell_paths, live(|m| m.cell_paths = !m.cell_paths)),
            check("Storm tracks (history)", l.storm_tracks, |this, cx| this.app.update(cx, |s, cx| {
                s.update_live(|m| m.storm_tracks = !m.storm_tracks);
                let building = !s.data.analysis.tracks.iter().any(|t| !t.ended && t.points.len() >= 2);
                if s.live.storm_tracks && building {
                    s.toast(crate::app::ToastKind::Info, "Storm tracks build up from each radar update: paths appear after a few minutes.");
                }
                cx.notify();
            })),
            check("Storm reports", l.reports, live(|m| m.reports = !m.reports)),
            act("Clear storm tracks", "", |this, cx| this.clear_storm_tracks(cx)),
            check("Distance rings", l.range_rings.is_some(), |this, cx| this.app.update(cx, |s, cx| {
                let home = s.settings.home;
                s.update_live(|m| {
                    m.range_rings = if m.range_rings.is_some() { None } else { Some(crate::map::scene::RangeRings { lat: home.lat, lon: home.lon, ..Default::default() }) };
                });
                cx.notify();
            })),
            MItem::Sep,
            MItem::Head("Map"),
            check("County lines", l.counties, live(|m| m.counties = !m.counties)),
            check("State lines", l.states, live(|m| m.states = !m.states)),
            check("Labels", l.labels, live(|m| m.labels = !m.labels)),
            check("Town labels", l.town_labels, live(|m| m.town_labels = !m.town_labels)),
            check("Highway shields", l.highways, live(|m| m.highways = !m.highways)),
            check("Roads (INDOT)", l.roads, live(|m| m.roads = !m.roads)),
            MItem::Sep,
            MItem::Head("Observations"),
            check("Airport obs", l.metar_plots, live(|m| m.metar_plots = !m.metar_plots)),
            check("River gauges", l.rivers, live(|m| m.rivers = !m.rivers)),
            check("Wind flow", l.data.wind_particles, live(|m| m.data.wind_particles = !m.data.wind_particles)),
        ];
        v.push(MItem::Sep);
        v.push(act("More layers in the side panel…", "", |this, cx| { this.layers_open = true; cx.notify(); }));
        v
    }

    // ---- Ribbon -----------------------------------------------------------------------------

    pub(super) fn ribbon(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.ribbon;
        let tabs = RibbonTab::ALL.iter().enumerate().map(|(i, t)| {
            let t = *t;
            let on = t == current;
            div()
                .id(("ribbon-tab", i))
                .px(px(14.))
                .h(px(30.))
                .flex()
                .items_center()
                .cursor_pointer()
                .text_size(px(13.))
                .border_b_2()
                .when(on, |d| d.text_color(c(CYAN)).border_color(c(CYAN)))
                .when(!on, |d| d.text_color(c(MUTED)).border_color(transparent_black()).hover(|s| s.text_color(c(TEXT))))
                .child(t.label())
                .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                    if this.ribbon == t && this.ribbon_open {
                        // Like Word: only a double-click on the open tab folds the ribbon. A
                        // single click (e.g. "back to Radar" mid-show) keeps the buttons.
                        if e.click_count() >= 2 {
                            this.ribbon_open = false;
                        }
                    } else {
                        this.ribbon = t;
                        this.ribbon_open = true;
                        this.follow_side_panel(t);
                    }
                    cx.notify();
                }))
        }).collect::<Vec<_>>();

        let body = if self.ribbon_open {
            let groups: Vec<AnyElement> = match current {
                RibbonTab::Home => self.ribbon_home(cx),
                RibbonTab::Radar => self.ribbon_radar(cx),
                RibbonTab::Forecast => self.ribbon_forecast(cx),
                RibbonTab::Outlooks => self.ribbon_outlooks(cx),
                RibbonTab::Draw => self.ribbon_draw(cx),
                RibbonTab::OnAir => self.ribbon_onair(cx),
                RibbonTab::Compare => self.ribbon_compare(cx),
            };
            // Narrow windows: the groups scroll sideways (mouse wheel works too), and a ‹ / ›
            // button on the cut-off edge shows that more is there and pages to it.
            let handle = self.ribbon_scroll.clone();
            let x = -f32::from(handle.offset().x);
            let max = f32::from(handle.max_offset().x);
            let edge = |id: &'static str, right: bool, step: f32| {
                let h = handle.clone();
                div()
                    .id(id)
                    // Block the click from reaching the ribbon button underneath.
                    .occlude()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .when(right, |d| d.right_0().border_l_1())
                    .when(!right, |d| d.left_0().border_r_1())
                    .w(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(c(PANEL_2))
                    .border_color(c(BORDER))
                    .text_size(px(16.))
                    .text_color(c(TEXT))
                    .hover(|s| s.bg(c(BORDER)))
                    .child(if right { "›" } else { "‹" })
                    .tooltip(move |_, cx| cx.new(|_| crate::ui::graphics_studio::TooltipText((if right { "More buttons" } else { "Back to the first buttons" }).into())).into())
                    .on_click(cx.listener(move |_, _, _, cx| {
                        let max = f32::from(h.max_offset().x);
                        let next = (-f32::from(h.offset().x) + step).clamp(0., max);
                        h.set_offset(point(px(-next), px(0.)));
                        cx.notify();
                    }))
            };
            Some(
                div()
                    .relative()
                    .w_full()
                    .h(px(84.))
                    .bg(c(PANEL))
                    .border_b_1()
                    .border_color(c(BORDER))
                    .child(
                        div()
                            .id("ribbon-body")
                            .flex()
                            .size_full()
                            .pt(px(3.))
                            .overflow_x_scroll()
                            .track_scroll(&self.ribbon_scroll)
                            .on_scroll(cx.listener(|_, _, _, cx| cx.notify()))
                            .children(groups),
                    )
                    .when(x > 1., |d| d.child(edge("ribbon-left", false, -360.)))
                    .when(max > 1. && x < max - 1., |d| d.child(edge("ribbon-right", true, 360.)))
                    .into_any_element(),
            )
        } else {
            None
        };

        div()
            .flex()
            .flex_col()
            .w_full()
            .flex_shrink_0()
            .child(div().flex().items_end().w_full().px(px(6.)).bg(c(BG)).border_b_1().border_color(c(BORDER)).children(tabs))
            .children(body)
    }

    /// The side panel shows the settings that belong to the ribbon tab.
    fn follow_side_panel(&mut self, t: RibbonTab) {
        self.tab = match t {
            RibbonTab::Radar => LeftTab::Radar,
            RibbonTab::Forecast => LeftTab::Data,
            RibbonTab::Outlooks => LeftTab::Severe,
            RibbonTab::OnAir => LeftTab::OnAir,
            RibbonTab::Compare => LeftTab::Layout,
            RibbonTab::Home | RibbonTab::Draw => self.tab,
        };
    }

    fn mode_buttons(&mut self, mode: ViewMode, live_on: bool, cx: &mut Context<Self>) -> Div {
        let modes = [
            (ViewMode::LiveRadar, "radar-line", "Live radar"),
            (ViewMode::SingleSite, "base-station-line", "Single site"),
            (ViewMode::Satellite, "earth-line", "Satellite"),
            (ViewMode::Futurecast, "sun-cloudy-line", "Forecast"),
            (ViewMode::Outlook, "thunderstorms-line", "Outlooks"),
        ];
        div().flex().gap(px(2.)).children(modes.into_iter().enumerate().map(|(i, (m, icon, text))| {
            let active = match m {
                ViewMode::Futurecast => matches!(mode, ViewMode::Futurecast | ViewMode::DataMaps),
                ViewMode::LiveRadar => mode == m && live_on,
                _ => mode == m,
            };
            rbig(("rb-mode", i), icon, text, active).on_click(cx.listener(move |this, _, _, cx| {
                match m {
                    ViewMode::Outlook => this.show_outlook(1, "categorical", cx),
                    ViewMode::Futurecast if active => {}
                    _ => this.set_view_mode(m, cx),
                }
                this.ribbon = RibbonTab::for_mode(m);
                this.follow_side_panel(this.ribbon);
                cx.notify();
            }))
        }).collect::<Vec<_>>())
    }

    fn ribbon_home(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let state = self.app.read(cx);
        let l = state.live.clone();
        let work_tab = state.active_tab.is_some();
        let pending = state.preview_pending();
        let mode = Self::view_mode(&l);
        let modes = self.mode_buttons(mode, l.radar.enabled, cx);
        let views = self.with_menu("rb-views", rsmall("rb-views-btn", "map-pin-line", "Saved views ▾"), false, cx, |this, cx| {
            let bookmarks = this.app.read(cx).settings.bookmarks.clone();
            let mut v: Vec<MItem> = bookmarks.into_iter().map(|b| {
                let view = b.view;
                act(b.name.clone(), "", move |this, cx| this.app.update(cx, |s, _| s.fly_to(view, 1.8)))
            }).collect();
            if v.is_empty() {
                v.push(act("No saved views yet: use Save view", "", |_, _| {}));
            }
            v
        });
        let place_results = self.place_results(cx);
        let place = div().flex().flex_col().gap(px(2.)).pt(px(2.))
            .child(div().w(px(230.)).relative().child(Input::new(&self.place_input))
                .when_some(place_results, |d, results| d.child(deferred(anchored().snap_to_window_with_margin(px(8.)).child(
                    div().id("rb-place-results").occlude().mt(px(2.)).w(px(320.)).max_h(px(360.)).overflow_y_scroll().p(px(6.)).rounded(px(6.)).bg(c(PANEL)).border_1().border_color(c(BORDER)).shadow_lg().child(results),
                )).with_priority(4))))
            .child(div().flex().gap(px(2.))
                .child(rsmall("rb-home", "home-4-line", "Home").on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| { let h = s.settings.home; s.fly_to(h, 1.8); }))))
                .child(views)
                .child(rsmall("rb-recent", "history-line", "Recent").on_click(cx.listener(|this, _, _, cx| { this.show_history = !this.show_history; cx.notify(); })))
                .child(rsmall("rb-save-view", "add-line", "Save view").on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, cx| {
                    let n = s.settings.bookmarks.len() + 1;
                    let view = s.live.view;
                    s.settings.bookmarks.push(Bookmark { name: format!("View {n}"), view });
                    let settings = s.settings.clone();
                    s.apply_settings(settings);
                    s.toast(ToastKind::Success, format!("Saved this view as \"View {n}\""));
                    cx.notify();
                })))));
        // The everyday map tools, one click away (they were on the old toolbar).
        let tool = self.tool;
        let tb = |id: &'static str, t: Tool, icon: &'static str, cx: &mut Context<Self>| {
            rbig(id, icon, t.label(), tool == t).on_click(cx.listener(move |this, _, _, cx| this.set_tool(t, cx)))
        };
        let tools = div().flex().gap(px(2.))
            .child(tb("rb-h-pan", Tool::Pan, "drag-move-2-line", cx))
            .child(tb("rb-h-pen", Tool::Pen, "mark-pen-line", cx))
            .child(tb("rb-h-arrow", Tool::Arrow, "arrow-right-up-line", cx))
            .child(tb("rb-h-text", Tool::Text, "text", cx))
            .child(tb("rb-h-raise", Tool::RaisedArea, "pencil-line", cx))
            .child(tb("rb-h-track", Tool::StormTrack, "route-line", cx))
            .child(rbig("rb-h-cleartrack", "eraser-line", "Clear tracks", false).on_click(cx.listener(|this, _, _, cx| this.clear_storm_tracks(cx))));
        vec![
            group("Map", modes).into_any_element(),
            group("Tools", tools).into_any_element(),
            group("Place", place).into_any_element(),
            group("Map tabs", div().flex().gap(px(2.))
                .child(rbig("rb-newtab", "add-line", "New tab", false).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, cx| { s.open_work_tab(); cx.notify(); }))))
                .child(rbig("rb-send", "send-plane-line", if work_tab { "Send to air" } else { "Send map to air" }, pending).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, cx| { s.send_tab_to_air(); cx.notify(); }))))
                .child(rbig("rb-reset", "arrow-go-back-line", "Reset tab", false).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, cx| { s.revert_map_preview(); cx.notify(); }))))).into_any_element(),
            group("Show", stack()
                .child(rcheck("rb-sidepanel", "Side panel", self.layers_open).on_click(cx.listener(|this, _, _, cx| { this.layers_open = !this.layers_open; cx.notify(); })))
                .child(rcheck("rb-conditions", "Conditions", self.info_open).on_click(cx.listener(|this, _, _, cx| { this.info_open = !this.info_open; cx.notify(); })))
                .child(rcheck("rb-legend", "Legend", l.show_legend).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| m.show_legend = !m.show_legend)))))).into_any_element(),
            group("Map", div().flex().gap(px(2.))
                .child(rbig("rb-refresh", "refresh-line", "Refresh radar", false).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, cx| { s.refresh_radar(cx); cx.notify(); }))))
                .child(rbig("rb-clear", "eraser-line", "Clear map", false).on_click(cx.listener(|this, _, _, cx| this.clear_map(cx))))).into_any_element(),
            group("Export", div().flex().gap(px(2.))
                .child(rbig("rb-png", "image-line", "Still PNG", false).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, cx| { s.save_map_snapshot(); cx.notify(); }))))
                .child(rbig("rb-gif", "film-line", "GIF", false).on_click(cx.listener(|this, _, _, cx| this.export_motion(true, cx))))
                .child(rbig("rb-mp4", "movie-2-line", "MP4", false).on_click(cx.listener(|this, _, _, cx| this.export_motion(false, cx))))
                .child(rbig("rb-toshow", "slideshow-line", "Add to show", false).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, cx| { s.add_map_to_show(false); cx.notify(); }))))).into_any_element(),
        ]
    }

    fn ribbon_radar(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let state = self.app.read(cx);
        let l = state.live.clone();
        let playing = state.playing;
        let mode = Self::view_mode(&l);
        let site = format!("K{}", l.radar.resolved_site(l.view.lat, l.view.lon));
        let modes = div().flex().gap(px(2.))
            .child(rbig("rb-r-refresh", "refresh-line", "Refresh radar", false).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, cx| { s.refresh_radar(cx); cx.notify(); }))))
            .child(rbig("rb-live", "radar-line", "Live radar", mode == ViewMode::LiveRadar && l.radar.enabled).on_click(cx.listener(|this, _, _, cx| this.set_view_mode(ViewMode::LiveRadar, cx))))
            .child(rbig("rb-site", "base-station-line", "Single site", mode == ViewMode::SingleSite).on_click(cx.listener(|this, _, _, cx| this.set_view_mode(ViewMode::SingleSite, cx))));
        let site_menu = self.with_menu("rb-site-pick", rbig("rb-site-pick-btn", "map-pin-2-line", format!("{site} ▾"), false), false, cx, |this, cx| {
            let l = this.app.read(cx).live.clone();
            let resolved = l.radar.resolved_site(l.view.lat, l.view.lon);
            let mut near: Vec<(&str, &str, f64)> = NEXRAD_SITES.iter().map(|(id, name, lat, lon)| (*id, *name, haversine_km(l.view.lat, l.view.lon, *lat, *lon))).collect();
            near.sort_by(|a, b| a.2.total_cmp(&b.2));
            let mut v = vec![check(format!("Nearest to the map (K{resolved})"), l.radar.site.eq_ignore_ascii_case("AUTO"), live(|m| { m.radar.site = "AUTO".into(); m.radar.enabled = true; m.radar.source = RadarSource::Site; })), MItem::Sep];
            for (id, name, km) in near.into_iter().take(12) {
                let id_owned = id.to_string();
                v.push(check(format!("K{id} {name} · {:.0} mi", km / KM_PER_MILE), l.radar.site.eq_ignore_ascii_case(id), live(move |m| { m.radar.site = id_owned.clone(); m.radar.enabled = true; m.radar.source = RadarSource::Site; })));
            }
            v
        });
        let product_menu = self.with_menu("rb-site-product", rbig("rb-site-product-btn", "list-check", format!("{} ▾", l.radar.product.short()), false), false, cx, |this, cx| {
            let current = this.app.read(cx).live.radar.product;
            SiteProduct::ALL.iter().map(|p| {
                let p = *p;
                check(p.label(), current == p, live(move |m| { m.radar.product = p; m.radar.enabled = true; m.radar.source = RadarSource::Site; }))
            }).collect()
        });
        // Single-site clear-air filter (CWXB-88): hides weak returns around the radar.
        let clear_air = self.with_menu("rb-clear-air", rbig("rb-clear-air-btn", "eraser-line", "Clear air ▾", l.radar.site_min_dbz > 0.0), false, cx, |this, cx| {
            let current = this.app.read(cx).live.radar.site_min_dbz;
            crate::map::site_radar::MIN_DBZ_CHOICES.iter().map(|v| {
                let v = *v;
                let text = if v <= 0.0 { "Show all returns".to_string() } else { format!("Hide under {v:.0} dBZ (single site)") };
                check(text, (current - v).abs() < 0.5, live(move |m| m.radar.site_min_dbz = v))
            }).collect()
        });
        let colors = self.with_menu("rb-colors", rbig("rb-colors-btn", "palette-line", "Colors ▾", false), false, cx, |this, cx| {
            let current = this.app.read(cx).live.radar.palette.clone();
            RADAR_PALETTES.iter().map(|(code, name)| {
                let code = code.to_string();
                check(*name, current == code, live(move |m| m.radar.palette = code.clone()))
            }).collect()
        });
        let history = self.with_menu("rb-history", rbig("rb-history-btn", "history-line", "Past radar ▾", l.is_radar_replay()), false, cx, |_, _| {
            let mut v: Vec<MItem> = crate::history::RADAR_REPLAY_HOURS.into_iter().map(|h| act(format!("Past {h} hours"), "", move |this, cx| this.app.update(cx, |s, cx| { s.start_radar_replay(h); cx.notify(); }))).collect();
            v.push(MItem::Sep);
            v.push(act("Past 3 hours of site velocity", "", |this, cx| this.app.update(cx, |s, cx| s.start_site_radar_replay(cx))));
            v.push(act("Event history…", "", |this, cx| this.app.update(cx, |s, cx| { s.modal = crate::app::Modal::History; cx.notify(); })));
            v.push(MItem::Sep);
            v.push(act("Back to live", "", |this, cx| this.set_view_mode(ViewMode::LiveRadar, cx)));
            v
        });
        let satellite = self.with_menu("rb-sat", rbig("rb-sat-btn", "earth-line", format!("{} ▾", l.satellite.channel.label()), mode == ViewMode::Satellite), false, cx, |this, cx| {
            let current = this.app.read(cx).live.satellite.channel;
            SatChannel::ALL.iter().map(|ch| {
                let ch = *ch;
                check(ch.label(), current == ch, move |this, cx| {
                    this.app.update(cx, |s, _| s.update_live(move |m| m.satellite.channel = ch));
                    this.set_view_mode(ViewMode::Satellite, cx);
                })
            }).collect()
        });
        let totals = self.with_menu("rb-totals", rbig("rb-totals-btn", "drop-line", "Rain totals ▾", l.precip_total.is_some()), false, cx, |this, cx| {
            let current = this.app.read(cx).live.precip_total;
            let mut v = vec![check("Off", current.is_none(), live(|m| m.precip_total = None))];
            v.extend(PrecipWindow::ALL.into_iter().map(|w| check(w.label(), current == Some(w), live(move |m| m.precip_total = Some(w)))));
            v
        });
        vec![
            group("Radar", modes).into_any_element(),
            group("Site", div().flex().gap(px(2.)).child(site_menu).child(product_menu)).into_any_element(),
            group("Look", div().flex().gap(px(2.)).child(colors).child(clear_air).child(totals)).into_any_element(),
            group("Loop", div().flex().gap(px(2.))
                .child(rbig("rb-play", if playing { "pause-line" } else { "play-line" }, if playing { "Pause" } else { "Play" }, playing).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, cx| {
                    s.playing = !s.playing;
                    s.live.radar.mode = RadarMode::Loop;
                    s.sync_map();
                    cx.notify();
                }))))
                .child(history)).into_any_element(),
            group("Satellite", satellite).into_any_element(),
            group("Alerts", div().flex().gap(px(6.)).child(alert_switches("rb-r", &l, cx)).child(stack()
                .child(rcheck("rb-r-count", "Countdowns", l.warning_countdowns).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| m.warning_countdowns = !m.warning_countdowns)))))
                .child(rcheck("rb-r-rings", "Distance rings", l.range_rings.is_some()).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| {
                    let home = s.settings.home;
                    s.update_live(|m| m.range_rings = if m.range_rings.is_some() { None } else { Some(crate::map::scene::RangeRings { lat: home.lat, lon: home.lon, ..Default::default() }) });
                }))))
                .child(rcheck("rb-r-paths", "Storm paths", l.cell_paths).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| m.cell_paths = !m.cell_paths))))))).into_any_element(),
            group("On the map", stack()
                .child(rcheck("rb-r-light", "Lightning", l.lightning).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| m.lightning = !m.lightning)))))
                .child(rcheck("rb-r-cells", "Storm icons", l.storm_cells).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| m.storm_cells = !m.storm_cells)))))).into_any_element(),
        ]
    }

    fn ribbon_forecast(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        use crate::futurecast::Product;
        let state = self.app.read(cx);
        let l = state.live.clone();
        let playing = state.data_playing;
        let noaa = l.data.field == Some(DataField::Futurecast);
        let model = l.data.grid_model;
        let product_text = match l.data.field {
            Some(DataField::Futurecast) => l.data.model_product.winter_choices().map_or(l.data.model_product, |c| c[0]).label().to_string(),
            Some(field) => field.label().to_string(),
            None => "Choose".into(),
        };
        let model_menu = self.with_menu("rb-model", rbig("rb-model-btn", "cpu-line", format!("{} ▾", model.label()), false), false, cx, |this, cx| {
            let model = this.app.read(cx).live.data.grid_model;
            crate::grid::GridModel::ALL.into_iter().map(|m| check(format!("{} · {}", m.label(), m.short_about()), model == m, move |this, cx| this.select_forecast_model(m, cx))).collect()
        });
        let product_menu = self.with_menu("rb-product", rbig("rb-product-btn", "list-check", format!("{product_text} ▾"), l.data.field.is_some()), false, cx, |this, cx| {
            let l = this.app.read(cx).live.clone();
            let model = l.data.grid_model;
            let noaa = l.data.field == Some(DataField::Futurecast);
            let current_noaa = l.data.model_product.winter_choices().map_or(l.data.model_product, |c| c[0]);
            FieldGroup::ALL.into_iter().map(|group| {
                let mut items = Vec::new();
                if let Some(source) = model.noaa_source() {
                    for option in Self::noaa_products(group).iter().copied().filter(|p| p.source() == source) {
                        let title = match option {
                            Product::Precipitation => "Rain, mix and snow".to_string(),
                            Product::GfsReflectivity | Product::HrrrReflectivity => "Simulated radar".to_string(),
                            other => other.label().to_string(),
                        };
                        // "· NOAA" only matters when other items in the list come from elsewhere.
                        let label = if model.is_noaa() { title } else { format!("{title} · NOAA") };
                        items.push(check(label, noaa && current_noaa == option, move |this, cx| { this.forecast_group = group; this.select_forecast_product(option, cx); }));
                    }
                    if !items.is_empty() {
                        items.push(MItem::Sep);
                    }
                }
                // With a NOAA model (GFS) the grid field and the NOAA product are the same data:
                // name the difference: the smooth NOAA map vs the grid map with town values (CWXB-98).
                let covered: Vec<DataField> = if model.is_noaa() {
                    model.noaa_source().map(|source| Self::noaa_products(group).iter().filter(|p| p.source() == source).filter_map(|p| Self::noaa_field(*p)).collect()).unwrap_or_default()
                } else {
                    Vec::new()
                };
                for field in DataField::ALL.iter().copied().filter(|f| f.group() == group && *f != DataField::Futurecast) {
                    let label = if covered.contains(&field) { format!("{}, town values", field.label()) } else { field.label().to_string() };
                    items.push(check(label, l.data.field == Some(field), move |this, cx| { this.forecast_group = group; this.select_data_field(Some(field), cx); }));
                }
                sub(group.label(), items)
            }).collect()
        });
        let display = l.data.model_product.winter_choices().filter(|_| noaa).map(|options| {
            let names = ["Most likely", "Low end (10%)", "High end (90%)", "Chance"];
            let product = l.data.model_product;
            self.with_menu("rb-display", rbig("rb-display-btn", "contrast-2-line", "Display ▾", false), false, cx, move |_, _| {
                options.into_iter().zip(names).map(|(o, n)| check(n, product == o, move |this, cx| this.select_forecast_product(o, cx))).collect()
            })
        });
        let fixed = l.data.field.and_then(DataField::fixed_window);
        let span = l.data.loop_hours;
        let range = (l.data.field.is_some() && fixed.is_none()).then(|| {
            let text = if span >= 72 { format!("{} days ▾", span / 24) } else { format!("{span} hours ▾") };
            self.with_menu("rb-range", rbig("rb-range-btn", "time-line", text, false), false, cx, |this, cx| {
                let l = this.app.read(cx).live.clone();
                let noaa = l.data.field == Some(DataField::Futurecast);
                let options: Vec<u32> = if noaa { l.data.model_product.spans().to_vec() } else { crate::grid::FORECAST_SPANS.iter().copied().filter(|h| *h <= l.data.grid_model.max_hours()).collect() };
                let last = options.last().copied();
                options.into_iter().map(|h| {
                    let name = if noaa && Some(h) == last { "Full run".to_string() } else if h >= 72 { format!("{} days", h / 24) } else { format!("{h} hours") };
                    check(name, l.data.loop_hours == h, move |this, cx| this.app.update(cx, |s, cx| { s.set_data_hour(0.0); s.update_live(|m| m.data.loop_hours = h); cx.notify(); }))
                }).collect()
            })
        });
        let compare = noaa.then(|| self.with_menu("rb-compare", rbig("rb-compare-btn", "layout-column-line", "Compare ▾", l.layout != PanelLayout::Single), false, cx, |this, cx| {
            let pinned = this.app.read(cx).live.data.model_pins.iter().any(Option::is_some);
            let mut v = vec![
                act("GFS vs HRRR simulated radar", "", |this, cx| this.app.update(cx, |s, _| {
                    s.set_data_hour(0.0);
                    s.update_live(|map| {
                        map.layout = PanelLayout::Dual;
                        map.data.model_product = Product::GfsReflectivity;
                        map.data.loop_hours = 12;
                        let mut panel = map.panel_spec(1);
                        panel.product = PanelProduct::HrrrReflectivity;
                        if let Some(first) = map.panels.first_mut() { *first = panel; } else { map.panels.push(panel); }
                    });
                })),
                act("GFS precipitation vs temperature", "", |this, cx| this.app.update(cx, |s, _| s.update_live(|map| {
                    map.layout = PanelLayout::Dual;
                    let mut panel = map.panel_spec(1);
                    panel.product = if map.data.model_product == Product::Temperature { PanelProduct::Futurecast } else { PanelProduct::GfsTemperature };
                    if let Some(first) = map.panels.first_mut() { *first = panel; } else { map.panels.push(panel); }
                }))),
                act("Back to one map", "", live(|m| m.layout = PanelLayout::Single)),
            ];
            if pinned {
                v.push(MItem::Sep);
                v.push(act("Use the latest model run", "", live(|m| m.data.model_pins = [None; 3])));
            }
            v
        }));
        let mut pick = div().flex().gap(px(2.)).child(model_menu).child(product_menu);
        if let Some(d) = display {
            pick = pick.child(d);
        }
        let mut time = div().flex().gap(px(2.));
        match (fixed, range) {
            (Some(window), _) => time = time.child(div().pt(px(8.)).px(px(4.)).text_size(px(12.)).text_color(c(MUTED)).child(window)),
            (None, Some(r)) => time = time.child(r),
            _ => {}
        }
        time = time.child(rbig("rb-fplay", if playing { "pause-line" } else { "play-line" }, if playing { "Pause" } else { "Play" }, playing).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, cx| {
            if crate::program::forecast_span(&s.live, &s.data) > 0 && crate::program::forecast_ready(&s.live, &s.data) {
                s.data_playing = !s.data_playing;
            }
            cx.notify();
        }))));
        let mut out = vec![
            group("Map", self.mode_buttons(Self::view_mode(&l), l.radar.enabled, cx)).into_any_element(),
            group("Model and product", pick).into_any_element(),
            group("Time", time).into_any_element(),
        ];
        if let Some(c2) = compare {
            out.push(group("Compare", c2).into_any_element());
        }
        if l.data.field.is_some() {
            out.push(group("Forecast", div().flex().gap(px(2.)).child(rbig("rb-foff", "close-line", "Turn off", false).on_click(cx.listener(|this, _, _, cx| this.select_data_field(None, cx))))).into_any_element());
        }
        out
    }

    fn ribbon_outlooks(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let l = self.app.read(cx).live.clone();
        let today = chrono::Local::now().date_naive();
        let day_name = |d: u8| match d {
            1 => "Today".to_string(),
            2 => "Tomorrow".to_string(),
            _ => (today + chrono::Duration::days(i64::from(d) - 1)).format("%A").to_string(),
        };
        let spc_on = l.spc_day.is_some();
        let current_day = l.spc_day.or(l.ero_day).unwrap_or(1);
        let days = div().flex().gap(px(2.)).children((1..=3u8).map(|d| {
            rbig(("rb-day", d as usize), "calendar-line", format!("Day {d} {}", day_name(d)), current_day == d && (spc_on || l.ero_day.is_some()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    let layer: &'static str = {
                        let l = &this.app.read(cx).live;
                        if l.ero_day.is_some() && l.spc_day.is_none() { "ero" } else { match l.spc_layer.as_str() { "tornado" => "tornado", "wind" => "wind", "hail" => "hail", "significant" => "significant", _ => "categorical" } }
                    };
                    if layer == "ero" {
                        this.app.update(cx, |s, _| s.update_live(|m| m.ero_day = Some(d)));
                    } else {
                        let layer = if d >= 3 && layer != "categorical" { "severe" } else { layer };
                        this.show_outlook(d, layer, cx);
                    }
                    cx.notify();
                }))
        }).collect::<Vec<_>>());
        let layers: &[(&'static str, &'static str, &'static str)] = if current_day >= 3 {
            &[("categorical", "error-warning-line", "Overall risk"), ("severe", "thunderstorms-line", "Any severe")]
        } else {
            &[("categorical", "error-warning-line", "Overall risk"), ("tornado", "tornado-line", "Tornado"), ("wind", "windy-line", "Wind"), ("hail", "hail-line", "Hail"), ("significant", "alarm-warning-line", "Significant")]
        };
        let layer_now = if l.spc_layer.is_empty() { "categorical".to_string() } else { l.spc_layer.clone() };
        let show = div().flex().gap(px(2.)).children(layers.iter().enumerate().map(|(i, (key, icon, text))| {
            let key: &'static str = key;
            rbig(("rb-ol", i), icon, *text, spc_on && layer_now == key).on_click(cx.listener(move |this, _, _, cx| {
                let day = this.app.read(cx).live.spc_day.unwrap_or(1);
                this.show_outlook(day, key, cx);
            }))
        }).collect::<Vec<_>>());
        let graphics = self.with_menu("rb-ol-gfx", rbig("rb-ol-gfx-btn", "image-line", "Make a graphic ▾", false), false, cx, |_, _| {
            [
                ("severe_threats", "Severe Threats (bars)"),
                ("severe_threats_blocks", "Severe Threats (blocks)"),
                ("severe_threats_tomorrow", "Severe Threats Tomorrow"),
                ("severe_day", "Storms Possible Today"),
                ("severe_day_tomorrow", "Storms Possible Tomorrow"),
                ("risk_levels", "Severe Risk Levels"),
                ("risk_meaning", "What the Risk Levels Mean"),
                ("severe_outlook", "Severe Outlook, Days 1-3"),
            ].into_iter().map(|(key, text)| act(text, "", move |this, cx| this.app.update(cx, |s, cx| {
                if let Some(doc) = crate::gfx::templates::build(key) {
                    if let Some(saved) = s.save_graphic(doc) {
                        s.open_graphic(saved.id);
                    }
                }
                cx.notify();
            }))).collect()
        });
        vec![
            group("Day", days).into_any_element(),
            group("Show", show).into_any_element(),
            group("Also show", div().flex().gap(px(6.))
                .child(stack()
                    .child(rcheck("rb-ol-ero", "Flooding rain", l.ero_day.is_some()).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| m.ero_day = if m.ero_day.is_some() { None } else { Some(m.spc_day.unwrap_or(1)) })))))
                    .child(rcheck("rb-ol-fire", "Fire weather", l.fire_outlook).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| m.fire_outlook = !m.fire_outlook)))))
                    .child(rcheck("rb-ol-md", "Mesoscale discussions", l.mesoscale).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| m.mesoscale = !m.mesoscale))))))
                .child(stack()
                    .child(rcheck("rb-ol-warn", "Warnings", l.warnings && l.alert_kinds.warnings).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| { m.alert_kinds.warnings = !(m.warnings && m.alert_kinds.warnings); m.warnings = m.warnings || m.alert_kinds.warnings; })))))
                    .child(rcheck("rb-ol-watch", "Watches", l.warnings && l.alert_kinds.watches).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| { m.alert_kinds.watches = !(m.warnings && m.alert_kinds.watches); m.warnings = m.warnings || m.alert_kinds.watches; })))))
                    .child(rcheck("rb-ol-counties", "County lines", l.counties).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| m.counties = !m.counties))))))
                // Fourth row wrapped to its own column so the group name stays visible.
                .child(stack()
                    .child(rcheck("rb-ol-legend", "Legend", l.show_legend).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| m.show_legend = !m.show_legend))))))).into_any_element(),
            group("Graphics", div().flex().gap(px(2.)).child(graphics)
                .child(rbig("rb-ol-bug", "alarm-warning-line", "Risk bug", false).on_click(cx.listener(|this, _, _, cx| { this.overlay_slot = 2; this.add_overlay_template("severe_risk_bug", 2, cx); })))).into_any_element(),
        ]
    }

    fn ribbon_draw(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let tool = self.tool;
        let raise_state = self.raise_state;
        let style = self.app.read(cx).live.raise_style;
        let tb = |id: &'static str, t: Tool, icon: &'static str, cx: &mut Context<Self>| {
            rbig(id, icon, t.label(), tool == t).on_click(cx.listener(move |this, _, _, cx| this.set_tool(t, cx)))
        };
        let appearance = self.with_menu("rb-raise-style", rbig("rb-raise-style-btn", "contrast-2-line", "Appearance ▾", false), false, cx, move |_, _| {
            [(RaiseStyle::Lift, "Lifted"), (RaiseStyle::Spotlight, "Spotlight"), (RaiseStyle::Outline, "Outline")]
                .into_iter().map(|(choice, name)| check(name, style == choice, live(move |m| m.raise_style = choice))).collect()
        });
        vec![
            group("Select", tb("rb-t-pan", Tool::Pan, "drag-move-2-line", cx)).into_any_element(),
            group("Draw", div().flex().gap(px(2.))
                .child(tb("rb-t-pen", Tool::Pen, "mark-pen-line", cx))
                .child(tb("rb-t-arrow", Tool::Arrow, "arrow-right-up-line", cx))
                .child(tb("rb-t-circle", Tool::Circle, "checkbox-blank-circle-line", cx))
                .child(tb("rb-t-area", Tool::Area, "shape-2-line", cx))
                .child(tb("rb-t-text", Tool::Text, "text", cx))
                .child(tb("rb-t-marker", Tool::Marker, "map-pin-2-line", cx))
                .child(tb("rb-t-ruler", Tool::Ruler, "ruler-line", cx))).into_any_element(),
            group("Label for text and markers", div().w(px(180.)).pt(px(4.)).child(Input::new(&self.text_input))).into_any_element(),
            group("Weather", div().flex().gap(px(2.))
                .child(tb("rb-t-front", Tool::Front, "route-line", cx))
                .child(tb("rb-t-high", Tool::High, "arrow-up-line", cx))
                .child(tb("rb-t-low", Tool::Low, "arrow-down-line", cx))).into_any_element(),
            group("Storms", div().flex().gap(px(2.))
                .child(tb("rb-t-track", Tool::StormTrack, "route-line", cx))
                .child(tb("rb-t-free", Tool::FreeTrack, "road-map-line", cx))
                .child(tb("rb-t-rings", Tool::Rings, "compass-3-line", cx))
                .child(tb("rb-t-spot", Tool::Spotlight, "focus-3-line", cx))
                .child(rbig("rb-t-clear-track", "eraser-line", "Clear tracks", false).on_click(cx.listener(|this, _, _, cx| this.clear_storm_tracks(cx))))).into_any_element(),
            group("Raise", div().flex().gap(px(2.))
                .child(rbig("rb-raise-draw", "pencil-line", "Draw area", tool == Tool::RaisedArea).on_click(cx.listener(|this, _, _, cx| this.set_tool(Tool::RaisedArea, cx))))
                .child(rbig("rb-raise-county", "map-2-line", "County", tool == Tool::RaisedBoundary && !raise_state).on_click(cx.listener(|this, _, _, cx| { this.raise_state = false; this.set_tool(Tool::RaisedBoundary, cx); })))
                .child(rbig("rb-raise-state", "map-2-line", "State", tool == Tool::RaisedBoundary && raise_state).on_click(cx.listener(|this, _, _, cx| { this.raise_state = true; this.set_tool(Tool::RaisedBoundary, cx); })))
                .child(appearance)
                .child(rbig("rb-raise-remove", "eraser-line", "Remove raise", false).on_click(cx.listener(|this, _, _, cx| {
                    this.cancel_raise(cx);
                    this.app.update(cx, |s, _| s.update_live(|m| m.annotations.retain(|a| a.kind != AnnotationKind::RaisedArea)));
                })))).into_any_element(),
            group("Edit", stack()
                .child(rsmall("rb-undo", "arrow-go-back-line", "Undo last mark").on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| { m.annotations.pop(); })))))
                .child(rsmall("rb-clear-draw", "eraser-line", "Remove drawings").on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| m.annotations.retain(|a| a.kind == AnnotationKind::RaisedArea))))))
                .child(rsmall("rb-arrange", "drag-move-2-line", "Arrange on-air items").on_click(cx.listener(|this, _, _, cx| this.set_tool(Tool::Arrange, cx))))).into_any_element(),
        ]
    }

    fn ribbon_onair(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let state = self.app.read(cx);
        let l = state.live.clone();
        let has_logo = state.settings.brand.logo_media.is_some();
        let automatic = l.automatic_title();
        let slot_menu = |this: &mut Self, id: &'static str, icon: &'static str, label: &'static str, slot: usize, template: &'static str, new_label: &'static str, cx: &mut Context<Self>| {
            let active_now = { let l = &this.app.read(cx).live; if slot == 0 { l.overlay_graphic.clone() } else { l.overlay_slots[slot - 1].clone() } };
            this.with_menu(id, rbig(id, icon, format!("{label} ▾"), active_now.is_some()), false, cx, move |this, cx| {
                let state = this.app.read(cx);
                let active = if slot == 0 { state.live.overlay_graphic.clone() } else { state.live.overlay_slots[slot - 1].clone() };
                let overlays: Vec<(String, String)> = state.graphics.iter().filter(|g| g.is_overlay()).map(|g| (g.id.clone(), g.name.clone())).collect();
                let mut v = vec![act(new_label, "", move |this, cx| { this.overlay_slot = slot; this.add_overlay_template(template, slot, cx); })];
                if !overlays.is_empty() {
                    v.push(MItem::Sep);
                    v.push(MItem::Head("Saved overlays"));
                    for (gid, name) in overlays {
                        let on = active.as_deref() == Some(gid.as_str());
                        v.push(check(name, on, move |this, cx| this.app.update(cx, |s, cx| { s.set_map_overlay_slot(slot, (!on).then(|| gid.clone())); cx.notify(); })));
                    }
                }
                if let Some(id) = active {
                    v.push(MItem::Sep);
                    let edit = id.clone();
                    v.push(act("Edit in Graphics Studio", "", move |this, cx| this.app.update(cx, |s, cx| { s.open_graphic(edit.clone()); cx.notify(); })));
                    v.push(act("Take it off", "", move |this, cx| this.app.update(cx, |s, cx| { s.set_map_overlay_slot(slot, None); cx.notify(); })));
                }
                v
            })
        };
        let lower = slot_menu(self, "rb-l3", "layout-row-line", "Lower third", 0, "lower_third", "New lower third", cx);
        let crawl = slot_menu(self, "rb-crawl", "arrow-left-right-line", "Crawl", 1, "alert_crawl", "New alert crawl", cx);
        let bug = slot_menu(self, "rb-bug", "stack-line", "Bug", 2, "sponsor_lower_third", "New sponsor lower third", cx);
        let brand_now = {
            let profile = self.app.read(cx).settings.current_brand_profile();
            if profile.station_short.trim().is_empty() { profile.name } else { profile.station_short }
        };
        let brand_menu = self.with_menu("rb-brand", rbig("rb-brand-btn", "brush-line", format!("{brand_now} ▾"), false), false, cx, |this, cx| this.brand_items(cx));
        let size = l.sponsor_size.unwrap_or(100);
        let sponsor_size = self.with_menu("rb-sp-size", rsmall("rb-sp-size-btn", "contrast-2-line", "Badge size ▾"), false, cx, move |_, _| {
            [(75u8, "Small"), (100, "Medium"), (125, "Large")].into_iter().map(|(v, n)| check(n, size == v, live(move |m| m.sponsor_size = Some(v)))).collect()
        });
        vec![
            group("Branding", brand_menu).into_any_element(),
            group("Title", div().flex().flex_col().gap(px(2.)).pt(px(2.))
                .child(div().w(px(240.)).child(Input::new(&self.title_input)))
                .child(div().flex().gap(px(4.))
                    .child(rcheck("rb-title-show", "Show title", l.show_title).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| m.show_title = !m.show_title)))))
                    .child(rsmall("rb-title-auto", "magic-line", if automatic { "Automatic" } else { "Use automatic" }).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| { m.title = "LIVE DOPPLER".into(); m.custom_title = false; }))))))).into_any_element(),
            group("On the map", div().flex().gap(px(6.))
                .child(stack()
                    .child(rcheck("rb-oa-wm", "Watermark", l.shows_watermark()).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| m.watermark_visible = Some(!m.shows_watermark()))))))
                    .child(rcheck("rb-oa-logo", if has_logo { "Station logo" } else { "Station logo (none set)" }, l.show_logo).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| { m.watermark_visible = Some(m.shows_watermark()); m.show_logo = !m.show_logo; })))))
                    .child(rcheck("rb-oa-legend", "Legend", l.show_legend).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| m.show_legend = !m.show_legend))))))
                .child(stack()
                    .child(rcheck("rb-oa-clock", "Clock", l.show_clock).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| m.show_clock = !m.show_clock)))))
                    .child(rcheck("rb-oa-time", "Time box", l.show_timestamp).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, _| s.update_live(|m| m.show_timestamp = !m.show_timestamp)))))
                    .child(rsmall("rb-oa-arrange", "drag-move-2-line", "Arrange").on_click(cx.listener(|this, _, _, cx| this.set_tool(Tool::Arrange, cx)))))).into_any_element(),
            group("Graphics", div().flex().gap(px(2.)).child(lower).child(crawl).child(bug)).into_any_element(),
            group("Sponsor", stack()
                .child(rcheck("rb-sp-badge", "Sponsor badge", l.show_sponsor).on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, cx| {
                    if !s.live.show_sponsor && s.current_sponsor().is_none() {
                        s.modal = crate::app::Modal::Sponsors;
                    } else {
                        s.update_live(|m| m.show_sponsor = !m.show_sponsor);
                    }
                    cx.notify();
                }))))
                .child(sponsor_size)
                .child(rsmall("rb-sp-settings", "hand-heart-line", "Sponsors…").on_click(cx.listener(|this, _, _, cx| this.app.update(cx, |s, cx| { s.modal = crate::app::Modal::Sponsors; cx.notify(); }))))).into_any_element(),
        ]
    }

    fn ribbon_compare(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let l = self.app.read(cx).live.clone();
        let icons = |p: PanelLayout| match p {
            PanelLayout::Single => "checkbox-blank-circle-line",
            PanelLayout::Dual => "layout-column-line",
            PanelLayout::Quad => "layout-4-line",
            PanelLayout::Wipe => "arrow-left-right-line",
        };
        vec![
            group("Layout", div().flex().gap(px(2.)).children(PanelLayout::ALL.into_iter().enumerate().map(|(i, p)| {
                rbig(("rb-layout", i), icons(p), p.label(), l.layout == p).on_click(cx.listener(move |this, _, _, cx| this.app.update(cx, |s, _| s.update_live(move |m| m.layout = p))))
            }).collect::<Vec<_>>())).into_any_element(),
            group("Panels", stack()
                .child(rsmall("rb-panels", "layout-left-line", "Pick what each panel shows in the side panel").on_click(cx.listener(|this, _, _, cx| { this.layers_open = true; this.tab = LeftTab::Layout; cx.notify(); })))).into_any_element(),
        ]
    }
}
