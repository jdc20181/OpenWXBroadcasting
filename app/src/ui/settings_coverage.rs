//! Settings > Coverage area: region name, state chips, a clickable county map, county search,
//! the supported counties (click one to make it home) and a town name per county. Modelled on
//! the website's Admin > Social Graphics "Regional support / Supported counties".

use std::sync::Arc;

use gpui_kit::{
    component::input::{Input, InputState},
    prelude::FluentBuilder as _,
    *,
};

use super::{group, SettingsView};
use crate::{
    coverage,
    ui::{c, chip, col, frame_view::frame_canvas, graphics_studio::TooltipText, icons::ri, muted, row, BORDER, MUTED, TEXT},
};

/// Size the county map is drawn at (16:9, mapped onto the 1920 x 1080 frame slot).
const MAP_W: u32 = 960;
const MAP_H: u32 = 540;

impl SettingsView {
    /// Keep one town field per supported county and redraw the county map when it changed.
    pub(super) fn sync_coverage(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let counties = self.draft.coverage.counties.clone();
        self.cov_towns.retain(|(f, _)| counties.iter().any(|c| c.fips == *f));
        for county in &counties {
            if !self.cov_towns.iter().any(|(f, _)| *f == county.fips) {
                let town = county.town.clone();
                let input = cx.new(|cx| InputState::new(window, cx).placeholder("Blank = NWS place nearest the centre").default_value(town));
                self.cov_towns.push((county.fips.clone(), input));
            }
        }
        self.cov_towns.sort_by_key(|(f, _)| counties.iter().position(|c| c.fips == *f));

        let Some(all) = crate::map::boundaries::counties_named() else {
            self.cov_image = None;
            return;
        };
        let states = coverage::map_states(&self.draft, &all);
        let home = coverage::home_fips(&self.draft, Some(&all));
        let key = format!(
            "{}|{}|{}",
            states.join(","),
            counties.iter().map(|c| c.fips.as_str()).collect::<Vec<_>>().join(","),
            home.clone().unwrap_or_default()
        );
        if self.cov_image.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        self.cov_image = coverage::render_map(&self.draft.coverage, home.as_deref(), &all, &states, MAP_W, MAP_H).and_then(|pm| {
            let mut px = pm.take();
            for p in px.chunks_exact_mut(4) {
                p.swap(0, 2);
                p[3] = 255;
            }
            image::RgbaImage::from_raw(MAP_W, MAP_H, px).map(|b| (key, Arc::new(RenderImage::new(vec![image::Frame::new(b)]))))
        });
    }

    /// A click on the county map adds or removes the county under it.
    fn coverage_map_click(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some((fx, fy)) = self.cov_slot.to_frame(position) else { return };
        let Some(all) = crate::map::boundaries::counties_named() else { return };
        let states = coverage::map_states(&self.draft, &all);
        let scale = MAP_W as f64 / crate::config::FRAME_W as f64;
        if let Some(county) = coverage::pick(&all, &states, MAP_W as f64, MAP_H as f64, fx * scale, fy * scale) {
            coverage::toggle_county(&mut self.draft.coverage, county);
            cx.notify();
        }
    }

    pub(super) fn coverage_group(&self, cx: &mut Context<Self>) -> Div {
        let cov = self.draft.coverage.clone();
        let all = crate::map::boundaries::counties_named();
        let home = coverage::home_fips(&self.draft, all.as_deref().map(|v| v.as_slice()));
        let shown_states: Vec<String> = all.as_deref().map(|a| coverage::map_states(&self.draft, a)).unwrap_or_default();

        let state_chips = crate::winter::STATES
            .iter()
            .enumerate()
            .map(|(i, (fips, abbr, name))| {
                let on = shown_states.iter().any(|s| s == fips);
                let abbr = abbr.to_string();
                chip(("cov-state", i), abbr.clone(), on).tooltip(move |_, cx| cx.new(|_| TooltipText(name.to_string())).into()).on_click(cx.listener(move |this, _, _, cx| {
                    // The first click keeps the states already shown (home + county states).
                    if this.draft.coverage.states.is_empty() {
                        if let Some(all) = crate::map::boundaries::counties_named() {
                            this.draft.coverage.states = coverage::map_states(&this.draft, &all)
                                .iter()
                                .filter_map(|f| coverage::state_abbr(f).map(str::to_string))
                                .collect();
                        }
                    }
                    coverage::toggle_state(&mut this.draft.coverage, &abbr);
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();

        let matches = all.as_deref().map(|a| coverage::search(&cov, a, &shown_states, &self.cov_query)).unwrap_or_default();
        let match_chips = matches
            .iter()
            .enumerate()
            .map(|(i, county)| {
                let fips = county.fips.clone();
                let text = format!("{}, {}", county.name, coverage::state_abbr(&county.state).unwrap_or_default());
                chip(("cov-match", i), text, false)
                    .child(div().ml(px(4.)).child(ri("add-line", 12., MUTED)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(all) = crate::map::boundaries::counties_named() {
                            if let Some(county) = all.iter().find(|c| c.fips == fips) {
                                coverage::toggle_county(&mut this.draft.coverage, county);
                            }
                        }
                        this.cov_query.clear();
                        this.cov_search.update(cx, |s, cx| s.set_value(String::new(), window, cx));
                        cx.notify();
                    }))
            })
            .collect::<Vec<_>>();

        let county_chips = cov
            .counties
            .iter()
            .enumerate()
            .map(|(i, county)| {
                let is_home = home.as_deref() == Some(county.fips.as_str());
                let (make_home, remove) = (county.fips.clone(), county.fips.clone());
                row()
                    .gap(px(2.))
                    .child(
                        chip(("cov-county", i), format!("{}, {}", county.name, county.state), is_home)
                            .when(is_home, |c| c.child(div().ml(px(4.)).child(ri("home-4-line", 12., TEXT))))
                            .tooltip(|_, cx| cx.new(|_| TooltipText("Make home county".into())).into())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.draft.coverage.home_fips = make_home.clone();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id(("cov-remove", i))
                            .px(px(3.))
                            .py(px(5.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .hover(|s| s.bg(c(BORDER)))
                            .tooltip({ let t = format!("Remove {}", county.name); move |_, cx| cx.new(|_| TooltipText(t.clone())).into() })
                            .child(ri("close-line", 12., MUTED))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(i) = this.draft.coverage.counties.iter().position(|c| c.fips == remove) {
                                    this.draft.coverage.counties.remove(i);
                                    if this.draft.coverage.home_fips == remove {
                                        this.draft.coverage.home_fips.clear();
                                    }
                                }
                                cx.notify();
                            })),
                    )
            })
            .collect::<Vec<_>>();

        let towns = self
            .cov_towns
            .iter()
            .filter_map(|(fips, input)| {
                let county = cov.counties.iter().find(|c| c.fips == *fips)?;
                Some(div().w(px(220.)).child(col().gap(px(4.)).child(muted(format!("{}, {}", county.name, county.state))).child(Input::new(input))))
            })
            .collect::<Vec<_>>();

        let image = self.cov_image.as_ref().map(|(_, img)| img.clone());
        let map = div()
            .id("cov-map")
            .w_full()
            .h(px(380.))
            .rounded(px(6.))
            .overflow_hidden()
            .border_1()
            .border_color(c(BORDER))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, _, cx| this.coverage_map_click(e.position, cx)))
            .child(frame_canvas(image, self.cov_slot.clone()).size_full());

        group("Coverage area")
            .child(muted("Click a county on the map to add or remove it. The supported counties are the coverage towns for the 7-Day Outlook town chips, the Whole region (coverage) table and the Regional Outlook graphic; watched spots and forecast regions come after them. None picked = the home county only, as before."))
            .child(col().gap(px(4.)).child(muted("Region name (shown as the area line)")).child(Input::new(&self.cov_name)))
            .child(muted("States on the map"))
            .child(row().flex_wrap().gap(px(4.)).children(state_chips))
            .when(all.is_none(), |g| g.child(muted("County outlines are still downloading; the map appears when they are ready.")))
            .child(map)
            .child(row().gap(px(6.)).items_center().child(ri("search-line", 14., MUTED)).child(div().flex_1().child(Input::new(&self.cov_search))))
            .when(!match_chips.is_empty(), |g| g.child(row().flex_wrap().gap(px(4.)).children(match_chips)))
            .child(muted(format!("Supported counties ({})", cov.counties.len())))
            .when(cov.counties.is_empty(), |g| g.child(muted("None picked: the home county only.")))
            .child(row().flex_wrap().gap(px(4.)).children(county_chips))
            .when(!cov.counties.is_empty(), |g| {
                g.child(muted("Click a county name to make it the home county (named first)."))
                    .child(muted("Town shown for each county (blank = the NWS place nearest the county's centre)"))
                    .child(row().flex_wrap().gap(px(8.)).children(towns))
            })
    }
}
