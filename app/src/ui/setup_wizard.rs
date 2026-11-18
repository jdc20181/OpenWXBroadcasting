//! First-run setup (also Settings > General > Run setup again): station, coverage (state,
//! counties, main city), where the weather data comes from, and a check that every feed answers.
//! Everything can be changed later in Settings. The `openwx setup` CLI writes the same settings.

use gpui_kit::{
    component::input::{Input, InputState},
    prelude::FluentBuilder as _,
    *,
};
use openwx_relay::region::{self, County};

use crate::{
    app::{AppState, Modal, ToastKind},
    config::{Bookmark, CoverageArea, CoverageCounty, GeoView, Settings},
    net,
    ui::{button, button_primary, button_small, c, chip, col, icons::ri, muted, row, BORDER, CYAN, GREEN, MUTED, RED, TEXT},
};

const STEPS: [&str; 5] = ["Welcome", "Your station", "Where you cover", "Weather data", "Finish"];

pub struct SetupWizardView {
    app: Entity<AppState>,
    step: usize,
    name: Entity<InputState>,
    short: Entity<InputState>,
    tagline: Entity<InputState>,
    contact: Entity<InputState>,
    state: Entity<InputState>,
    filter: Entity<InputState>,
    city: Entity<InputState>,
    region_name: Entity<InputState>,
    remote: Entity<InputState>,
    /// Counties of the state that was looked up, and the state they belong to.
    counties: Vec<County>,
    counties_state: String,
    picked: Vec<String>,
    /// Main city found: (name, lat, lon), and the NWS county zone holding it.
    found_city: Option<(String, f64, f64)>,
    home_zone: Option<String>,
    remote_mode: bool,
    busy: Option<String>,
    error: Option<String>,
    checks: Vec<(String, bool, String)>,
}

fn input(window: &mut Window, cx: &mut Context<SetupWizardView>, placeholder: &str, value: &str) -> Entity<InputState> {
    let (placeholder, value) = (placeholder.to_owned(), value.to_owned());
    cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(value))
}

fn field(label: &str, help: &str, input: &Entity<InputState>) -> Div {
    col().gap(px(4.)).child(div().text_size(px(13.)).text_color(c(TEXT)).child(label.to_owned())).child(Input::new(input)).when(!help.is_empty(), |d| d.child(muted(help.to_owned())))
}

impl SetupWizardView {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let s = app.read(cx).settings.clone();
        let fresh = !s.setup_complete;
        let keep = |v: &str| if fresh && (v.starts_with("Your Station") || v.is_empty()) { String::new() } else { v.to_owned() };
        let state = s.coverage.states.first().cloned().unwrap_or_default();
        let city = if fresh { String::new() } else { s.home_label.split(',').next().unwrap_or_default().trim().to_owned() };
        Self {
            step: 0,
            name: input(window, cx, "e.g. Riverside Weather", &keep(&s.station_name)),
            short: input(window, cx, "e.g. Riverside WX", &keep(&s.station_short)),
            tagline: input(window, cx, "Your Weather. Your Community.", &s.tagline),
            contact: input(window, cx, "you@example.org or https://your-site.org", &s.contact),
            state: input(window, cx, "State, e.g. Indiana or IN", &state),
            filter: input(window, cx, "Search counties", ""),
            city: input(window, cx, "Main city or town", &city),
            region_name: input(window, cx, "e.g. Southern Indiana", &s.coverage.name),
            remote: input(window, cx, "https://your-server/t/your-access-token", &s.remote_api_base),
            counties: Vec::new(),
            counties_state: String::new(),
            picked: s.coverage.counties.iter().map(|c| c.fips.clone()).collect(),
            found_city: (!fresh).then(|| (city.clone(), s.home.lat, s.home.lon)),
            home_zone: None,
            remote_mode: s.backend_mode == "remote",
            busy: None,
            error: None,
            checks: Vec::new(),
            app,
        }
    }

    fn text(&self, input: &Entity<InputState>, cx: &App) -> String {
        input.read(cx).value().trim().to_owned()
    }

    /// An in-process relay with the station's contact, for lookups (works in every backend mode).
    fn relay(&self, cx: &App) -> Option<openwx_relay::AppState> {
        let mut s = self.app.read(cx).settings.clone();
        s.contact = self.text(&self.contact, cx);
        openwx_relay::AppState::new(crate::backend::relay_config(&s)).ok()
    }

    fn set_error(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.busy = None;
        self.error = Some(message.into());
        cx.notify();
    }

    fn load_counties(&mut self, cx: &mut Context<Self>) {
        let typed = self.text(&self.state, cx);
        let Some(st) = region::state_abbr(&typed) else {
            return self.set_error(format!("\"{typed}\" is not a U.S. state. Try the two-letter code, like IN."), cx);
        };
        if st == self.counties_state && !self.counties.is_empty() {
            return;
        }
        let Some(relay) = self.relay(cx) else { return self.set_error("Could not start the data lookup.", cx) };
        self.busy = Some(format!("Loading {st} counties from the National Weather Service…"));
        self.error = None;
        cx.notify();
        let st = st.to_owned();
        cx.spawn(async move |this, cx| {
            let result = net::spawn({
                let st = st.clone();
                async move { region::counties(&relay, &st).await }
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(list)) => {
                        this.picked.retain(|f| list.iter().any(|c| c.fips == *f));
                        this.counties = list;
                        this.counties_state = st;
                        this.busy = None;
                    }
                    Ok(Err(e)) => this.error = Some(format!("Could not load counties: {e}")),
                    Err(e) => this.error = Some(format!("Could not load counties: {e}")),
                }
                this.busy = None;
                cx.notify();
            });
        })
        .detach();
    }

    fn find_city(&mut self, cx: &mut Context<Self>) {
        let city = self.text(&self.city, cx);
        let st = region::state_abbr(&self.text(&self.state, cx)).unwrap_or_default().to_owned();
        if city.is_empty() || st.is_empty() {
            return self.set_error("Enter your state and main city first.", cx);
        }
        let Some(relay) = self.relay(cx) else { return self.set_error("Could not start the data lookup.", cx) };
        self.busy = Some(format!("Finding {city}…"));
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = net::spawn(async move {
                let found = region::find_city(&relay, &st, &city).await?;
                let zone = match &found {
                    Some((_, lat, lon)) => region::county_at(&relay, *lat, *lon).await.ok().flatten(),
                    None => None,
                };
                anyhow::Ok((found, zone, city))
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = None;
                match result {
                    Ok(Ok((Some(found), zone, _))) => {
                        // Add the city's own county to the coverage if it isn't there yet.
                        if let Some(z) = &zone {
                            if let Some(county) = this.counties.iter().find(|c| c.zone == *z) {
                                if !this.picked.contains(&county.fips) {
                                    this.picked.push(county.fips.clone());
                                }
                            }
                        }
                        this.found_city = Some(found);
                        this.home_zone = zone;
                    }
                    Ok(Ok((None, _, city))) => this.error = Some(format!("Could not find \"{city}\". Check the spelling, or try a nearby larger town.")),
                    Ok(Err(e)) => this.error = Some(format!("Lookup failed: {e}")),
                    Err(e) => this.error = Some(format!("Lookup failed: {e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The settings this wizard would save.
    fn result(&self, cx: &App) -> Settings {
        let mut s = self.app.read(cx).settings.clone();
        let name = self.text(&self.name, cx);
        let short = self.text(&self.short, cx);
        s.station_name = if name.is_empty() { "Your Station Weather".into() } else { name.clone() };
        s.station_short = if short.is_empty() { s.station_name.clone() } else { short };
        let tagline = self.text(&self.tagline, cx);
        if !tagline.is_empty() {
            s.tagline = tagline;
        }
        s.contact = self.text(&self.contact, cx);
        s.backend_mode = if self.remote_mode { "remote".into() } else { "local".into() };
        if self.remote_mode {
            s.remote_api_base = self.text(&self.remote, cx).trim_end_matches('/').to_owned();
        }
        let picked: Vec<&County> = self.picked.iter().filter_map(|f| self.counties.iter().find(|c| c.fips == *f)).collect();
        if let Some((city, lat, lon)) = self.found_city.clone() {
            let st = self.counties_state.clone();
            let home = self.home_zone.as_deref().and_then(|z| picked.iter().find(|c| c.zone == z)).or(picked.first()).map(|c| c.fips.clone()).unwrap_or_default();
            let region_name = self.text(&self.region_name, cx);
            if !picked.is_empty() {
                s.coverage = CoverageArea {
                    name: region_name.clone(),
                    states: vec![st.clone()],
                    home_fips: home.clone(),
                    counties: picked
                        .iter()
                        .map(|c| CoverageCounty { fips: c.fips.clone(), name: c.name.clone(), state: c.state.clone(), town: if c.fips == home { city.clone() } else { String::new() } })
                        .collect(),
                };
                s.alert_area = st.clone();
            }
            s.home = GeoView { lat, lon, zoom: 8.2 };
            s.home_label = format!("{city}, {st}");
            let mut marks = vec![Bookmark { name: city.clone(), view: GeoView { lat, lon, zoom: 9.5 } }];
            if !region_name.is_empty() {
                marks.push(Bookmark { name: region_name, view: GeoView { lat, lon, zoom: 7.2 } });
            }
            let extra: Vec<Bookmark> = s.bookmarks.iter().filter(|b| !marks.iter().any(|m| m.name == b.name)).cloned().collect();
            marks.extend(extra);
            s.bookmarks = marks;
            s.setup_complete = true;
        }
        s
    }

    fn check_feeds(&mut self, cx: &mut Context<Self>) {
        let settings = self.result(cx);
        let base = crate::backend::effective_base(&settings);
        if base.is_empty() {
            return self.set_error("Direct mode has no relay to check.", cx);
        }
        self.busy = Some("Checking each weather source…".into());
        self.error = None;
        self.checks.clear();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = net::spawn(async move { net::get_json(&format!("{base}/api/openwx/check")).await }).await;
            let _ = this.update(cx, |this, cx| {
                this.busy = None;
                match result {
                    Ok(Ok(v)) => {
                        this.checks = v["checks"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|c| (c["name"].as_str().unwrap_or("?").to_owned(), c["ok"] == true, c["error"].as_str().unwrap_or("").to_owned()))
                            .collect();
                    }
                    Ok(Err(e)) => this.error = Some(format!("The relay did not answer: {e}")),
                    Err(e) => this.error = Some(format!("The relay did not answer: {e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn can_continue(&self, cx: &App) -> Result<(), String> {
        match self.step {
            1 => {
                let contact = self.text(&self.contact, cx);
                if self.text(&self.name, cx).is_empty() {
                    Err("Give your station a name.".into())
                } else if !openwx_relay::prompt::looks_like_contact(&contact) {
                    Err("Enter a contact e-mail (name@example.org) or an https:// website.".into())
                } else {
                    Ok(())
                }
            }
            2 if self.picked.is_empty() => Err("Choose at least one county.".into()),
            2 if self.found_city.is_none() => Err("Find your main city so the map knows where home is.".into()),
            3 if self.remote_mode && !self.text(&self.remote, cx).starts_with("https://") => Err("Enter the https:// address of your relay server.".into()),
            _ => Ok(()),
        }
    }

    fn next(&mut self, cx: &mut Context<Self>) {
        if let Err(e) = self.can_continue(cx) {
            return self.set_error(e, cx);
        }
        self.error = None;
        if self.step == 1 && !self.text(&self.state, cx).is_empty() {
            self.load_counties(cx);
        }
        self.step = (self.step + 1).min(STEPS.len() - 1);
        if self.step == 3 && self.checks.is_empty() && !self.remote_mode {
            self.check_feeds(cx);
        }
        cx.notify();
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        let mut settings = self.result(cx);
        settings.setup_complete = true;
        let city = settings.home_label.clone();
        self.app.update(cx, |s, cx| {
            s.apply_settings(settings);
            s.modal = Modal::None;
            s.refresh_data(cx);
            s.toast(ToastKind::Success, format!("All set. Home is {city}. Change anything later in Settings."));
            cx.notify();
        });
    }

    fn skip(&mut self, cx: &mut Context<Self>) {
        let mut settings = self.app.read(cx).settings.clone();
        settings.setup_complete = true;
        self.app.update(cx, |s, cx| {
            s.apply_settings(settings);
            s.modal = Modal::None;
            s.toast(ToastKind::Info, "Setup skipped. Run it again any time from Settings > General.");
            cx.notify();
        });
    }

    fn rail(&self) -> Div {
        row().gap(px(6.)).children(STEPS.iter().enumerate().map(|(i, label)| {
            let (fg, bg) = if i == self.step { (TEXT, CYAN) } else if i < self.step { (TEXT, GREEN) } else { (MUTED, BORDER) };
            row()
                .gap(px(6.))
                .items_center()
                .child(div().size(px(20.)).rounded_full().bg(c(bg)).flex().items_center().justify_center().text_size(px(11.)).text_color(c(0xFFFFFF)).child((i + 1).to_string()))
                .child(div().text_size(px(12.)).text_color(c(fg)).child(label.to_string()))
                .when(i + 1 < STEPS.len(), |r| r.child(div().w(px(18.)).h(px(1.)).bg(c(BORDER))))
        }))
    }

    fn welcome(&self) -> Div {
        col()
            .gap(px(10.))
            .child(div().text_size(px(22.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).child("Welcome to OpenWX Broadcasting"))
            .child(muted("A broadcast weather studio for your community: live radar maps, on-air graphics, shows and alerts, built on free public data from NOAA and the National Weather Service."))
            .child(muted("This takes about two minutes. You'll name your station, choose the counties you cover and your main city, and we'll check that the weather data is flowing."))
            .child(row().gap(px(8.)).items_center().child(ri("error-warning-line", 16., MUTED)).child(muted("OpenWX is not an official warning source. For life-safety decisions, follow the National Weather Service.")))
    }

    fn station(&self) -> Div {
        col()
            .gap(px(12.))
            .child(field("Station name", "Shown on air and on your graphics.", &self.name))
            .child(field("Short name", "For tight spots like the corner bug.", &self.short))
            .child(field("Tagline", "", &self.tagline))
            .child(field(
                "Contact e-mail or website",
                "The National Weather Service asks every app that uses its data for a contact. It is sent with data requests, never shown on air.",
                &self.contact,
            ))
    }

    fn coverage(&self, cx: &mut Context<Self>) -> Div {
        let filter = self.text(&self.filter, cx).to_ascii_lowercase();
        let picked_names: Vec<String> = self.picked.iter().filter_map(|f| self.counties.iter().find(|c| c.fips == *f)).map(|c| c.name.clone()).collect();
        let list = self.counties.iter().filter(|c| filter.is_empty() || c.name.to_ascii_lowercase().contains(&filter)).map(|county| {
            let on = self.picked.contains(&county.fips);
            let fips = county.fips.clone();
            chip(SharedString::from(format!("wiz-county-{}", county.fips)), county.name.clone(), on).on_click(cx.listener(move |this, _, _, cx| {
                if let Some(i) = this.picked.iter().position(|f| *f == fips) {
                    this.picked.remove(i);
                } else {
                    this.picked.push(fips.clone());
                }
                cx.notify();
            }))
        });
        col()
            .gap(px(12.))
            .child(
                row()
                    .gap(px(8.))
                    .items_end()
                    .child(div().flex_1().child(field("State", "", &self.state)))
                    .child(button("wiz-load-counties", "Show counties").on_click(cx.listener(|this, _, _, cx| this.load_counties(cx)))),
            )
            .when(!self.counties.is_empty(), |d| {
                d.child(muted(format!("Click the counties you cover ({} chosen){}", self.picked.len(), if picked_names.is_empty() { String::new() } else { format!(": {}", picked_names.join(", ")) })))
                    .child(Input::new(&self.filter))
                    .child(div().id("wiz-county-list").max_h(px(200.)).overflow_y_scroll().child(row().flex_wrap().gap(px(6.)).children(list)))
            })
            .child(
                row()
                    .gap(px(8.))
                    .items_end()
                    .child(div().flex_1().child(field("Main city or town", "The map centres here and forecasts default to it.", &self.city)))
                    .child(button("wiz-find-city", "Find").on_click(cx.listener(|this, _, _, cx| this.find_city(cx)))),
            )
            .when_some(self.found_city.clone(), |d, (name, lat, lon)| {
                d.child(row().gap(px(6.)).items_center().child(ri("map-pin-2-line", 15., GREEN)).child(muted(format!("{name} at {lat:.4}, {lon:.4}"))))
            })
            .child(field("Coverage area name (optional)", "Used in titles, like \"Southern Indiana\".", &self.region_name))
    }

    fn backend(&self, cx: &mut Context<Self>) -> Div {
        let option = |id: &'static str, on: bool, title: &'static str, help: &'static str, remote: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(c(if on { CYAN } else { BORDER }))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.remote_mode = remote;
                    this.checks.clear();
                    cx.notify();
                }))
                .child(row().gap(px(8.)).items_center().child(ri(if on { "focus-3-line" } else { "checkbox-blank-circle-line" }, 16., if on { CYAN } else { MUTED })).child(div().text_color(c(TEXT)).child(title)))
                .child(div().pl(px(24.)).child(muted(help)))
        };
        col()
            .gap(px(10.))
            .child(muted("OpenWX reads NOAA and National Weather Service data through a small built-in server called OpenWX Relay."))
            .child(option("wiz-local", !self.remote_mode, "On this PC (recommended)", "Runs inside the app. Nothing else to install.", false, cx))
            .child(option("wiz-remote", self.remote_mode, "A hosted relay on my own server", "For several studio PCs sharing one relay (see server/README.md).", true, cx))
            .when(self.remote_mode, |d| d.child(field("Relay address", "The address `openwx-relay setup` printed on your server.", &self.remote)))
            .child(row().gap(px(8.)).child(button_small("wiz-check", "Check the weather feeds").on_click(cx.listener(|this, _, _, cx| this.check_feeds(cx)))))
            .children(self.checks.iter().map(|(name, ok, error)| {
                row()
                    .gap(px(8.))
                    .items_center()
                    .child(ri(if *ok { "focus-3-line" } else { "close-line" }, 15., if *ok { GREEN } else { RED }))
                    .child(div().w(px(220.)).text_color(c(TEXT)).text_size(px(13.)).child(name.clone()))
                    .child(muted(if *ok { "Answering".to_owned() } else { error.clone() }))
            }))
    }

    fn summary(&self, cx: &App) -> Div {
        let s = self.result(cx);
        let line = |label: &str, value: String| row().gap(px(10.)).child(div().w(px(150.)).text_color(c(MUTED)).child(label.to_owned())).child(div().text_color(c(TEXT)).child(value));
        col()
            .gap(px(8.))
            .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child("Ready to go"))
            .child(line("Station", format!("{} ({})", s.station_name, s.station_short)))
            .child(line("Home", s.home_label.clone()))
            .child(line("Counties", s.coverage.counties.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")))
            .child(line("Weather data", if s.backend_mode == "remote" { s.remote_api_base.clone() } else { "OpenWX Relay on this PC".into() }))
            .child(muted("Next: add your logo and colours in Settings > Branding, then build your first show."))
    }
}

impl Render for SetupWizardView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.step {
            0 => self.welcome(),
            1 => self.station(),
            2 => self.coverage(cx),
            3 => self.backend(cx),
            _ => self.summary(cx),
        };
        let last = self.step == STEPS.len() - 1;
        col()
            .p(px(18.))
            .gap(px(16.))
            .min_h(px(420.))
            .child(self.rail())
            .child(div().id("wiz-body").flex_1().overflow_y_scroll().child(body))
            .when_some(self.busy.clone(), |d, b| d.child(row().gap(px(8.)).items_center().child(ri("refresh-line", 15., CYAN)).child(muted(b))))
            .when_some(self.error.clone(), |d, e| d.child(row().gap(px(8.)).items_center().child(ri("error-warning-line", 15., RED)).child(div().text_color(c(RED)).text_size(px(13.)).child(e))))
            .child(
                row()
                    .gap(px(8.))
                    .child(button_small("wiz-skip", "Skip setup").on_click(cx.listener(|this, _, _, cx| this.skip(cx))))
                    .child(div().flex_1())
                    .when(self.step > 0, |r| {
                        r.child(button("wiz-back", "Back").on_click(cx.listener(|this, _, _, cx| {
                            this.step -= 1;
                            this.error = None;
                            cx.notify();
                        })))
                    })
                    .when(!last, |r| r.child(button_primary("wiz-next", if self.step == 0 { "Get started" } else { "Next" }).on_click(cx.listener(|this, _, _, cx| this.next(cx)))))
                    .when(last, |r| r.child(button_primary("wiz-finish", "Finish").on_click(cx.listener(|this, _, _, cx| this.finish(cx))))),
            )
    }
}
