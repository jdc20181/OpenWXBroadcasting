//! Graphics Studio > Social Graphics: make the website's alert cards and daily forecast posts by
//! hand (for something special, or to correct an automatic post), live or from the archive.
//! Same renderer and designs as the automatic posts (`crate::social_graphics`).

use std::sync::Arc;

use chrono::{Datelike, Local, NaiveDate};
use gpui_kit::{
    component::input::{Input, InputState},
    prelude::FluentBuilder as _,
    *,
};
use serde_json::Value;

use super::GraphicsStudio;
use crate::{
    app::ToastKind,
    net,
    social_graphics::{self as sg, AlertPick, ForecastData, Home, SponsorPick, Station},
    ui::{button_primary, button_small, c, chip, col, muted, row, toggle, BORDER, CYAN, MUTED, PANEL_2, RED, TEXT},
};

/// The landing-page category that opens this panel.
pub const CATEGORY: &str = "Social Graphics";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Alert,
    Forecast,
}

pub struct SocialState {
    kind: Kind,
    /// Archive (IEM) instead of live.
    past: bool,
    from: Entity<InputState>,
    to: Entity<InputState>,
    day: Entity<InputState>,
    caption: Entity<InputState>,
    home: Option<Home>,
    alerts: Vec<AlertPick>,
    picked: Option<usize>,
    forecast: Option<ForecastData>,
    /// Index into [`sponsors`]; None = no sponsor block.
    sponsor: Option<usize>,
    /// The last alert job (map and all), so a design or sponsor change redraws without refetching.
    alert_job: Option<Value>,
    busy: Option<String>,
    msg: Option<(bool, String)>,
    png: Option<Vec<u8>>,
    image: Option<Arc<RenderImage>>,
    gen: u64,
}

impl SocialState {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        let input = |window: &mut Window, cx: &mut App, placeholder: &str, value: String| {
            let placeholder = placeholder.to_string();
            cx.new(|cx| {
                let mut s = InputState::new(window, cx).placeholder(placeholder);
                s.set_value(value, window, cx);
                s
            })
        };
        let today = Local::now().date_naive();
        let jan1 = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap_or(today);
        SocialState {
            kind: Kind::Alert,
            past: false,
            from: input(window, cx, "From (YYYY-MM-DD)", jan1.format("%Y-%m-%d").to_string()),
            to: input(window, cx, "To (YYYY-MM-DD)", today.format("%Y-%m-%d").to_string()),
            day: input(window, cx, "Day (YYYY-MM-DD)", (today - chrono::Duration::days(365)).format("%Y-%m-%d").to_string()),
            caption: input(window, cx, "Facebook caption", String::new()),
            home: None,
            alerts: Vec::new(),
            picked: None,
            forecast: None,
            sponsor: None,
            alert_job: None,
            busy: None,
            msg: None,
            png: None,
            image: None,
            gen: 0,
        }
    }
}

/// Main sponsor first, then the rotation (Settings > Branding & sponsors).
fn sponsors(s: &crate::config::Settings) -> Vec<SponsorPick> {
    let label = "Brought to you by".to_string();
    let mut out = Vec::new();
    if !s.sponsor_name.trim().is_empty() {
        out.push(SponsorPick { label: label.clone(), name: s.sponsor_name.clone(), logo_media: s.sponsor_logo_media.clone() });
    }
    for sp in &s.sponsors {
        if !sp.name.trim().is_empty() && !out.iter().any(|o| o.name == sp.name) {
            out.push(SponsorPick { label: label.clone(), name: sp.name.clone(), logo_media: sp.logo_media.clone() });
        }
    }
    out
}

fn png_to_image(png: &[u8]) -> Option<Arc<RenderImage>> {
    let mut img = image::load_from_memory(png).ok()?.to_rgba8();
    for px in img.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(img)])))
}

/// "SOUTHERN INDIANA" -> "Southern Indiana".
fn title_case_words(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut ch = w.chars();
            ch.next().map(|f| f.to_uppercase().chain(ch.flat_map(char::to_lowercase)).collect::<String>()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_day(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d").ok()
}

impl GraphicsStudio {
    fn social_settings(&self, cx: &App) -> crate::config::Settings {
        self.app.read(cx).settings.clone()
    }

    fn social_prefs(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut sg::Prefs)) {
        self.app.update(cx, |s, cx| {
            let mut next = s.settings.clone();
            change(&mut next.social);
            s.apply_settings(next);
            cx.notify();
        });
    }

    /// Looks up the home county / zone / office once.
    fn social_with_home(&mut self, cx: &mut Context<Self>, then: impl FnOnce(&mut Self, Home, &mut Context<Self>) + 'static) {
        if let Some(h) = self.social.home.clone() {
            then(self, h, cx);
            return;
        }
        let home = self.social_settings(cx).home;
        self.social.busy = Some("Finding the home county and forecast zone…".into());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = net::spawn(sg::home(home.lat, home.lon)).await;
            let _ = this.update(cx, |this, cx| match r {
                Ok(Ok(h)) if !h.county_ugc.is_empty() => {
                    this.social.home = Some(h.clone());
                    then(this, h, cx);
                }
                Ok(Err(e)) => this.social_fail(format!("Couldn't look up the home area: {e}"), cx),
                _ => this.social_fail("Couldn't look up the home area (NWS points).".into(), cx),
            });
        })
        .detach();
    }

    fn social_fail(&mut self, text: String, cx: &mut Context<Self>) {
        self.social.busy = None;
        self.social.msg = Some((false, text));
        cx.notify();
    }

    fn social_load_alerts(&mut self, cx: &mut Context<Self>) {
        self.social.alerts.clear();
        self.social.picked = None;
        self.social.msg = None;
        if self.social.past {
            let (Some(from), Some(to)) = (parse_day(&self.social.from.read(cx).value()), parse_day(&self.social.to.read(cx).value())) else {
                return self.social_fail("Dates are YYYY-MM-DD, e.g. 2025-03-15.".into(), cx);
            };
            self.social_with_home(cx, move |this, home, cx| {
                this.social.busy = Some(format!("Searching the NWS archive for {} County…", home.county_name));
                cx.notify();
                let ugc = home.county_ugc.clone();
                cx.spawn(async move |this, cx| {
                    let r = net::spawn(async move { sg::past_alerts(&ugc, from, to).await }).await;
                    let _ = this.update(cx, |this, cx| this.social_alerts_loaded(r, cx));
                })
                .detach();
            });
        } else {
            let home = self.social_settings(cx).home;
            self.social.busy = Some("Loading active NWS alerts…".into());
            cx.notify();
            cx.spawn(async move |this, cx| {
                let r = net::spawn(sg::live_alerts(home.lat, home.lon)).await;
                let _ = this.update(cx, |this, cx| this.social_alerts_loaded(r, cx));
            })
            .detach();
        }
    }

    fn social_alerts_loaded(&mut self, r: Result<anyhow::Result<Vec<AlertPick>>, tokio::task::JoinError>, cx: &mut Context<Self>) {
        self.social.busy = None;
        match r {
            Ok(Ok(list)) => {
                self.social.msg = list.is_empty().then(|| (true, if self.social.past { "No alerts in that window.".to_string() } else { "No active alerts for home right now. Switch to Past to use the archive.".to_string() }));
                self.social.alerts = list;
            }
            Ok(Err(e)) => self.social.msg = Some((false, format!("Couldn't load alerts: {e}"))),
            Err(e) => self.social.msg = Some((false, format!("Couldn't load alerts: {e}"))),
        }
        cx.notify();
    }

    fn social_pick_alert(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(pick) = self.social.alerts.get(i).cloned() else { return };
        self.social.picked = Some(i);
        self.social.alert_job = None;
        let settings = self.social_settings(cx);
        let sponsor = self.social.sponsor.and_then(|n| sponsors(&settings).get(n).cloned());
        let design = settings.social.alert_design.clone();
        let station = Station::from_settings(&settings);
        let api = settings.api_base.clone();
        // County / state lines styled like Map Studio's live map (its theme and opacity).
        let lines = sg::lines_from_scene(&self.app.read(cx).live);
        self.social_with_home(cx, move |this, home, cx| {
            this.social.busy = Some(if pick.is_past() { "Reading the archived warning, polygon and radar…".into() } else { "Fetching radar and map…".into() });
            cx.notify();
            cx.spawn(async move |this, cx| {
                let r = net::spawn(sg::alert_job(pick, station, home.fips, api, design, sponsor, lines)).await;
                let _ = this.update(cx, |this, cx| match r {
                    Ok(Ok(job)) => {
                        this.social.alert_job = Some(job.clone());
                        this.social_render(job, cx);
                    }
                    Ok(Err(e)) => this.social_fail(format!("Couldn't build the card: {e}"), cx),
                    Err(e) => this.social_fail(format!("Couldn't build the card: {e}"), cx),
                });
            })
            .detach();
        });
    }

    fn social_load_forecast(&mut self, cx: &mut Context<Self>) {
        self.social.msg = None;
        let settings = self.social_settings(cx);
        let prefs = settings.social.clone();
        let regional = prefs.forecast_scope == "regional";
        let coverage = prefs.forecast_scope == "coverage";
        // Coverage towns: home plus the alert spots (Settings). One town for the outlook /
        // home-county forecast, or the ticked towns as the coverage table.
        let places = sg::coverage_places(&settings, &self.app.read(cx).data.forecast_edits.regions);
        let town = places.iter().find(|p| !p.home && p.name == prefs.forecast_place).cloned();
        let table: Vec<sg::Place> = places.into_iter().filter(|p| !prefs.coverage_skip.contains(&p.name)).collect();
        // Regional also reads the state forecast's city table (live, or archived for that morning).
        let attach = move |home: sg::Home, day: Option<chrono::NaiveDate>, f: anyhow::Result<ForecastData>| async move {
            let mut f = f?;
            if regional {
                match sg::regional_forecast(&home, day, &prefs.regional_section, &prefs.regional_home_city).await {
                    Ok(r) => f.regional = Some(r),
                    Err(e) => f.note = format!("{} · Regional table unavailable ({e}); showing the home forecast", f.note),
                }
            } else if coverage && day.is_some() {
                f.note = format!("{} · The coverage table is live only; showing the home forecast", f.note);
            } else if coverage {
                match sg::coverage_forecast(table).await {
                    Ok(r) => f.regional = Some(r),
                    Err(e) => f.note = format!("{} · Coverage table unavailable ({e}); showing the home forecast", f.note),
                }
            }
            Ok(f)
        };
        if self.social.past {
            let Some(day) = parse_day(&self.social.day.read(cx).value()) else {
                return self.social_fail("The day is YYYY-MM-DD, e.g. 2025-03-14.".into(), cx);
            };
            self.social_with_home(cx, move |this, home, cx| {
                this.social.busy = Some(if regional { "Reading the archived zone and state forecasts…".into() } else { "Reading the archived zone forecast…".into() });
                cx.notify();
                cx.spawn(async move |this, cx| {
                    let r = net::spawn(async move {
                        let f = sg::past_forecast(&home, day).await;
                        attach(home, Some(day), f).await
                    })
                    .await;
                    let _ = this.update(cx, |this, cx| this.social_forecast_loaded(r, cx));
                })
                .detach();
            });
        } else {
            let (lat, lon) = town.as_ref().filter(|_| !coverage && !regional).map(|t| (t.lat, t.lon)).unwrap_or((settings.home.lat, settings.home.lon));
            let place = town.filter(|_| !coverage && !regional).map(|t| t.name).unwrap_or_default();
            self.social_with_home(cx, move |this, home, cx| {
                this.social.busy = Some(if regional {
                    "Loading the NWS forecast and the state forecast…".into()
                } else if coverage {
                    "Loading each coverage town's NWS forecast…".into()
                } else {
                    "Loading the NWS forecast…".into()
                });
                cx.notify();
                cx.spawn(async move |this, cx| {
                    let r = net::spawn(async move {
                        let f = sg::live_forecast(lat, lon).await.map(|mut f| {
                            f.place = place;
                            f
                        });
                        attach(home, None, f).await
                    })
                    .await;
                    let _ = this.update(cx, |this, cx| this.social_forecast_loaded(r, cx));
                })
                .detach();
            });
        }
    }

    fn social_forecast_loaded(&mut self, r: Result<anyhow::Result<ForecastData>, tokio::task::JoinError>, cx: &mut Context<Self>) {
        match r {
            Ok(Ok(f)) => {
                self.social.forecast = Some(f);
                self.social_redraw(cx);
            }
            Ok(Err(e)) => self.social_fail(format!("Couldn't load the forecast: {e}"), cx),
            Err(e) => self.social_fail(format!("Couldn't load the forecast: {e}"), cx),
        }
    }

    /// Redraws the current graphic after a design / theme / size / sponsor change.
    fn social_redraw(&mut self, cx: &mut Context<Self>) {
        let settings = self.social_settings(cx);
        let sponsor = self.social.sponsor.and_then(|n| sponsors(&settings).get(n).cloned());
        match self.social.kind {
            Kind::Alert => {
                if let Some(mut job) = self.social.alert_job.clone() {
                    // Each design's map panel has its own size, and the map is fetched at full
                    // detail for it: a different design fetches the map again.
                    if job["design"].as_str() != Some(settings.social.alert_design.as_str()) {
                        if let Some(i) = self.social.picked {
                            self.social_pick_alert(i, cx);
                            return;
                        }
                    }
                    job["design"] = settings.social.alert_design.clone().into();
                    job["sponsor"] = sponsor.as_ref().map(sg::sponsor_json).unwrap_or(Value::Null);
                    self.social.alert_job = Some(job.clone());
                    self.social_render(job, cx);
                }
            }
            Kind::Forecast => {
                if let Some(f) = &self.social.forecast {
                    let job = sg::forecast_job(f, &Station::from_settings(&settings), &settings.social, sponsor.as_slice());
                    self.social_render(job, cx);
                }
            }
        }
    }

    fn social_render(&mut self, job: Value, cx: &mut Context<Self>) {
        self.social.gen += 1;
        let gen = self.social.gen;
        self.social.busy = Some("Drawing…".into());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = net::spawn(async move { tokio::task::spawn_blocking(move || sg::render(&job)).await }).await;
            let _ = this.update(cx, |this, cx| {
                if this.social.gen != gen {
                    return;
                }
                this.social.busy = None;
                match r {
                    Ok(Ok(Ok(png))) => {
                        this.social.image = png_to_image(&png);
                        this.social.png = Some(png);
                    }
                    Ok(Ok(Err(e))) => this.social.msg = Some((false, format!("Drawing failed: {e}"))),
                    _ => this.social.msg = Some((false, "Drawing failed.".into())),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn social_file_name(&self) -> String {
        match self.social.kind {
            Kind::Alert => self.social.picked.and_then(|i| self.social.alerts.get(i)).map(|a| a.event.clone()).unwrap_or_else(|| "Alert card".into()),
            Kind::Forecast => "Forecast post".into(),
        }
    }

    fn social_save(&mut self, cx: &mut Context<Self>) {
        let Some(png) = self.social.png.clone() else { return };
        let name = self.social_file_name();
        self.app.update(cx, |s, cx| {
            match sg::save_png(&png, &name) {
                Ok(path) => s.toast(ToastKind::Success, format!("Saved {} to Exports", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())),
                Err(e) => s.toast(ToastKind::Error, format!("Couldn't save the PNG: {e}")),
            }
            cx.notify();
        });
    }

    fn social_post(&mut self, cx: &mut Context<Self>) {
        let Some(png) = self.social.png.clone() else { return };
        let base = self.social_settings(cx).api_base.clone();
        if !crate::dcweather::has_token(&base) {
            self.social.msg = Some((false, "Sign in first: Settings > Integrations > DCWeather account.".into()));
            cx.notify();
            return;
        }
        let caption = self.social.caption.read(cx).value().trim().to_string();
        let file = format!("{}.png", self.social_file_name().replace(' ', "-").to_lowercase());
        self.social.busy = Some("Posting to the DCWeather Facebook page…".into());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = net::spawn(crate::dcweather::post_image(base, png, file, caption, None)).await;
            let _ = this.update(cx, |this, cx| {
                this.social.busy = None;
                this.social.msg = Some(match r {
                    Ok(Ok(id)) => (true, format!("Posted (Facebook post {id}).")),
                    Ok(Err(e)) => (false, format!("Not posted: {e}")),
                    Err(e) => (false, format!("Not posted: {e}")),
                });
                cx.notify();
            });
        })
        .detach();
    }

    /// The panel shown on the landing page when the Social Graphics category is open.
    pub(super) fn social_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let settings = self.social_settings(cx);
        let prefs = settings.social.clone();
        let st = &self.social;
        let kind = st.kind;
        let past = st.past;

        let kind_chips = [(Kind::Alert, "Alert card"), (Kind::Forecast, "Daily forecast")].into_iter().enumerate().map(|(i, (k, text))| {
            chip(("sg-kind", i), text, kind == k).on_click(cx.listener(move |this, _, _, cx| {
                this.social.kind = k;
                this.social.png = None;
                this.social.image = None;
                this.social.msg = None;
                cx.notify();
            }))
        });
        let source_chips = [(false, "Live"), (true, "Past (NWS archive)")].into_iter().enumerate().map(|(i, (p, text))| {
            chip(("sg-src", i), text, past == p).on_click(cx.listener(move |this, _, _, cx| {
                this.social.past = p;
                this.social.alerts.clear();
                this.social.picked = None;
                this.social.msg = None;
                cx.notify();
            }))
        });
        let designs: Vec<&'static str> = if kind == Kind::Alert { sg::ALERT_DESIGNS.to_vec() } else { sg::FORECAST_DESIGNS.to_vec() };
        let current_design = if kind == Kind::Alert { prefs.alert_design.clone() } else { prefs.forecast_design.clone() };
        let design_chips = designs.into_iter().enumerate().map(|(i, d)| {
            let label = if d == "modern" { "Modern (production)".to_string() } else { sg::design_label(d).to_string() };
            chip(("sg-design", i), label, current_design == d).on_click(cx.listener(move |this, _, _, cx| {
                let k = this.social.kind;
                this.social_prefs(cx, |p| if k == Kind::Alert { p.alert_design = d.into() } else { p.forecast_design = d.into() });
                this.social_redraw(cx);
            }))
        });
        let theme_chips = sg::THEMES.iter().enumerate().map(|(i, (key, label, _))| {
            let key: &'static str = key;
            chip(("sg-theme", i), *label, prefs.forecast_theme == key).on_click(cx.listener(move |this, _, _, cx| {
                this.social_prefs(cx, |p| p.forecast_theme = key.into());
                this.social_redraw(cx);
            }))
        });
        let ratio_chips = sg::FORECAST_RATIOS.iter().enumerate().map(|(i, r)| {
            let r: &'static str = r;
            chip(("sg-ratio", i), sg::ratio_label(r), prefs.forecast_ratio == r).on_click(cx.listener(move |this, _, _, cx| {
                this.social_prefs(cx, |p| p.forecast_ratio = r.into());
                this.social_redraw(cx);
            }))
        });
        // Forecast area: the TV-style outlook, the home 7-day, or the state forecast's city table.
        let scope_chips = [("outlook", "7-Day Outlook"), ("county", "Single forecast"), ("coverage", "Whole region (coverage)"), ("regional", "NWS state table")].into_iter().enumerate().map(|(i, (key, text))| {
            chip(("sg-scope", i), text, prefs.forecast_scope == key).on_click(cx.listener(move |this, _, _, cx| {
                this.social_prefs(cx, |p| p.forecast_scope = key.into());
                if this.social.forecast.is_some() {
                    this.social_load_forecast(cx);
                }
            }))
        });
        let regional = st.forecast.as_ref().and_then(|f| f.regional.clone());
        let section_chips = regional.iter().flat_map(|r| r.sections.clone()).enumerate().map(|(i, s)| {
            let on = prefs.regional_section.eq_ignore_ascii_case(&s) || (prefs.regional_section.is_empty() && regional.as_ref().is_some_and(|r| r.section == s));
            let key = s.clone();
            chip(("sg-section", i), title_case_words(&s), on).on_click(cx.listener(move |this, _, _, cx| {
                let key = key.clone();
                this.social_prefs(cx, |p| p.regional_section = key);
                this.social_load_forecast(cx);
            }))
        });
        let city_chips = std::iter::once(String::new()).chain(regional.iter().flat_map(|r| r.cities.iter().map(|c| c.0.clone()))).enumerate().map(|(i, name)| {
            let on = prefs.regional_home_city.eq_ignore_ascii_case(&name);
            let label = if name.is_empty() { "None".to_string() } else { name.clone() };
            chip(("sg-city", i), label, on).on_click(cx.listener(move |this, _, _, cx| {
                let name = name.clone();
                this.social_prefs(cx, |p| p.regional_home_city = name);
                this.social_load_forecast(cx);
            }))
        });
        // Coverage towns (home + alert spots): one town, or which ones go in the coverage table.
        let places = sg::coverage_places(&settings, &self.app.read(cx).data.forecast_edits.regions);
        let place_chips = places.iter().enumerate().map(|(i, p)| {
            let key = if p.home { String::new() } else { p.name.clone() };
            let on = prefs.forecast_place == key || (key.is_empty() && !places.iter().any(|q| !q.home && q.name == prefs.forecast_place));
            chip(("sg-place", i), p.name.clone(), on).on_click(cx.listener(move |this, _, _, cx| {
                let key = key.clone();
                this.social_prefs(cx, |p| p.forecast_place = key);
                this.social_load_forecast(cx);
            }))
        });
        let table_chips = places.iter().enumerate().map(|(i, p)| {
            let name = p.name.clone();
            chip(("sg-cover", i), name.clone(), !prefs.coverage_skip.contains(&name)).on_click(cx.listener(move |this, _, _, cx| {
                let name = name.clone();
                this.social_prefs(cx, |p| {
                    if let Some(i) = p.coverage_skip.iter().position(|n| *n == name) {
                        p.coverage_skip.remove(i);
                    } else {
                        p.coverage_skip.push(name);
                    }
                });
                this.social_load_forecast(cx);
            }))
        });
        let few_places = places.len() < 2;
        let sponsor_list = sponsors(&settings);
        let sponsor_chips = std::iter::once((None, "No sponsor".to_string()))
            .chain(sponsor_list.iter().enumerate().map(|(i, s)| (Some(i), s.name.clone())))
            .enumerate()
            .map(|(n, (idx, name))| {
                chip(("sg-sponsor", n), name, st.sponsor == idx).on_click(cx.listener(move |this, _, _, cx| {
                    this.social.sponsor = idx;
                    this.social_redraw(cx);
                }))
            });

        let alert_rows = st.alerts.iter().enumerate().take(200).map(|(i, a)| {
            let active = st.picked == Some(i);
            let color = crate::data::alert_color(&a.event);
            let hex = u32::from_str_radix(color.trim_start_matches('#'), 16).unwrap_or(0x888888);
            row()
                .id(("sg-alert", i))
                .gap(px(8.))
                .px(px(8.))
                .py(px(5.))
                .rounded(px(5.))
                .cursor_pointer()
                .border_1()
                .when(active, |d| d.border_color(c(CYAN)).bg(crate::ui::ca(CYAN, 0.12)))
                .when(!active, |d| d.border_color(c(BORDER)).hover(|s| s.bg(c(PANEL_2))))
                .child(div().w(px(10.)).h(px(10.)).rounded(px(2.)).bg(c(hex)).flex_shrink_0())
                .child(div().text_size(px(12.)).text_color(c(TEXT)).truncate().child(a.label()))
                .on_click(cx.listener(move |this, _, _, cx| this.social_pick_alert(i, cx)))
        });

        // Preview size: the PNG's own shape, fitted in about 760 px.
        let (pw, ph) = match kind {
            Kind::Alert => (760.0, 760.0 * 630.0 / 1200.0),
            Kind::Forecast => {
                let (w, h) = social_render::forecast_size(&prefs.forecast_ratio);
                if h > w {
                    (560.0 * w / h, 560.0)
                } else if (h - w).abs() < 1.0 {
                    (520.0, 520.0)
                } else {
                    (760.0, 760.0 * h / w)
                }
            }
        };
        let preview = match &st.image {
            Some(image) => div().w(px(pw)).h(px(ph)).flex_shrink_0().rounded(px(6.)).overflow_hidden().bg(c(0x05070B)).child(img(image.clone()).w(px(pw)).h(px(ph)).object_fit(ObjectFit::Contain)).into_any_element(),
            None => div()
                .w(px(pw))
                .h(px(ph))
                .flex_shrink_0()
                .rounded(px(6.))
                .border_1()
                .border_color(c(BORDER))
                .bg(c(0x0E1320))
                .flex()
                .items_center()
                .justify_center()
                .child(muted(if kind == Kind::Alert { "Pick an alert to see its card." } else { "Load a forecast to see the post." }))
                .into_any_element(),
        };

        let note = match kind {
            Kind::Forecast => st.forecast.as_ref().map(|f| f.note.clone()).unwrap_or_default(),
            Kind::Alert => {
                if past {
                    "Past alerts come from the Iowa Environmental Mesonet NWS archive: the original text, polygon and archived radar (IEM colours).".into()
                } else {
                    "Live alerts and radar, exactly as the automatic posts draw them.".into()
                }
            }
        };

        let controls = col()
            .gap(px(10.))
            .w(px(380.))
            .flex_shrink_0()
            .child(row().gap(px(6.)).flex_wrap().children(kind_chips))
            .child(row().gap(px(6.)).flex_wrap().children(source_chips))
            .when(kind == Kind::Alert && past, |d| {
                d.child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&st.from))).child(div().flex_1().child(Input::new(&st.to))))
            })
            .when(kind == Kind::Forecast, |d| {
                d.child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child("Forecast area"))
                    .child(row().gap(px(6.)).flex_wrap().children(scope_chips))
                    .when(matches!(prefs.forecast_scope.as_str(), "outlook" | "county") && !few_places, |d| {
                        d.child(muted("Town")).child(row().gap(px(6.)).flex_wrap().children(place_chips))
                    })
                    .when(prefs.forecast_scope == "coverage", |d| {
                        d.child(muted("Towns in the table (click to leave one out)")).child(row().gap(px(6.)).flex_wrap().children(table_chips))
                    })
                    .when(few_places && matches!(prefs.forecast_scope.as_str(), "outlook" | "county" | "coverage"), |d| {
                        d.child(muted("Add extra alert spots in Settings > Alerts to pick other towns."))
                    })
                    .when(prefs.forecast_scope == "regional" && regional.is_some(), |d| {
                        d.child(muted("Section of the state"))
                            .child(row().gap(px(6.)).flex_wrap().children(section_chips))
                            .child(muted("Highlighted city"))
                            .child(row().gap(px(6.)).flex_wrap().children(city_chips))
                    })
            })
            .when(kind == Kind::Forecast && past, |d| d.child(Input::new(&st.day)))
            .child(match kind {
                Kind::Alert => button_primary("sg-load", if past { "Find past alerts for home" } else { "Load active alerts" }).on_click(cx.listener(|this, _, _, cx| this.social_load_alerts(cx))),
                Kind::Forecast => button_primary("sg-load", if past { "Load that day's forecast" } else { "Load today's forecast" }).on_click(cx.listener(|this, _, _, cx| this.social_load_forecast(cx))),
            })
            .when(kind == Kind::Alert && !st.alerts.is_empty(), |d| {
                d.child(muted(format!("{} alert(s). Click one to make its card.", st.alerts.len())))
                    .child(col().id("sg-alert-list").gap(px(4.)).max_h(px(260.)).overflow_y_scroll().children(alert_rows))
            })
            .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child("Design"))
            .child(row().gap(px(6.)).flex_wrap().children(design_chips))
            .when(kind == Kind::Forecast, |d| {
                d.child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child("Theme"))
                    .child(row().gap(px(6.)).flex_wrap().children(theme_chips))
                    .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child("Size"))
                    .child(row().gap(px(6.)).flex_wrap().children(ratio_chips))
                    .child(toggle("sg-graph", "Temperature trend", prefs.show_graph).on_click(cx.listener(|this, _, _, cx| {
                        this.social_prefs(cx, |p| p.show_graph = !p.show_graph);
                        this.social_redraw(cx);
                    })))
            })
            .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child("Sponsor"))
            .child(row().gap(px(6.)).flex_wrap().children(sponsor_chips));

        let actions = row()
            .gap(px(8.))
            .flex_wrap()
            .child(div().w(px(320.)).child(Input::new(&st.caption)))
            .child(button_small("sg-save", "Save PNG to Exports").on_click(cx.listener(|this, _, _, cx| this.social_save(cx))))
            .child(button_small("sg-post", "Post to DCWeather Facebook").on_click(cx.listener(|this, _, _, cx| this.social_post(cx))));

        col()
            .w_full()
            .gap(px(10.))
            .child(muted("The same alert cards and forecast posts the website makes automatically. Modern is what the automatic posts use; the other designs are here for special posts and corrections."))
            .child(
                row()
                    .w_full()
                    .gap(px(18.))
                    .items_start()
                    .child(controls)
                    .child(
                        col()
                            .gap(px(8.))
                            .min_w_0()
                            .child(preview)
                            .when_some(st.busy.clone(), |d, b| d.child(row().gap(px(6.)).child(crate::ui::icons::ri("time-line", 14., CYAN)).child(muted(b))))
                            .when_some(st.msg.clone(), |d, (ok, m)| d.child(div().text_size(px(12.)).text_color(c(if ok { MUTED } else { RED })).child(m)))
                            .when(!note.is_empty(), |d| d.child(muted(note)))
                            .when(st.png.is_some(), |d| d.child(actions)),
                    ),
            )
            .into_any_element()
    }
}
