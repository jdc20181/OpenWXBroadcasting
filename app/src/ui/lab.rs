//! Science lab (modal): forecast verification and the analysis / research tools, one tab each.

use gpui_kit::{
    component::input::{Input, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::{AppState, ToastKind},
    ui::{button_small, c, chip, col, heading, muted, row, BORDER, TEXT},
    verify,
};

pub const TABS: &[(&str, &str)] = &[("verify", "Verification"), ("nowcast", "Nowcast"), ("files", "Data files")];

pub struct LabView {
    pub app: Entity<AppState>,
    pub tab: &'static str,
    days: i64,
    report: Option<verify::Report>,
    pub input: Entity<InputState>,
    pub input2: Entity<InputState>,
    pub output: String,
    files: Entity<crate::ui::data_files::DataFilesView>,
    _subscriptions: Vec<Subscription>,
}

impl LabView {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(""));
        let input2 = cx.new(|cx| InputState::new(window, cx).placeholder(""));
        let files = cx.new(|cx| crate::ui::data_files::DataFilesView::new(app.clone(), window, cx));
        Self { files, app: app.clone(), tab: "verify", days: 30, report: None, input, input2, output: String::new(), _subscriptions: vec![cx.observe(&app, |_, _, cx| cx.notify())] }
    }

    pub fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        self.app.update(cx, |s, cx| {
            s.toast(kind, msg);
            cx.notify();
        });
    }

    fn verification(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.report.is_none() {
            self.report = Some(verify::compute(self.days));
        }
        let r = self.report.clone().unwrap_or_default();
        let days = self.days;
        let cell = |t: String, w: f32| div().w(px(w)).text_size(px(12.)).text_color(c(TEXT)).child(t);
        let header = row().gap(px(4.)).children(["Source", "Day", "n", "High bias", "High MAE", "High RMSE", "Low bias", "Low MAE", "Rain MAE", "Brier"].iter().map(|h| div().w(px(if *h == "Source" { 110. } else { 78. })).text_size(px(11.)).text_color(c(0x9AA3B5)).child(h.to_string())).collect::<Vec<_>>());
        let rows = r.by_lead.iter().filter(|((_, lead), _)| *lead >= 1 && *lead <= 7).map(|((src, lead), s)| {
            row()
                .gap(px(4.))
                .child(cell(src.clone(), 110.))
                .child(cell(lead.to_string(), 78.))
                .child(cell(s.hi.n.to_string(), 78.))
                .child(cell(format!("{:+.1}°", s.hi.bias()), 78.))
                .child(cell(format!("{:.1}°", s.hi.mae()), 78.))
                .child(cell(format!("{:.1}°", s.hi.rmse()), 78.))
                .child(cell(format!("{:+.1}°", s.lo.bias()), 78.))
                .child(cell(format!("{:.1}°", s.lo.mae()), 78.))
                .child(cell(format!("{:.2}\"", s.precip.mae()), 78.))
                .child(cell(s.brier().map(|b| format!("{b:.3}")).unwrap_or_else(|| "–".into()), 78.))
        }).collect::<Vec<_>>();
        let skill = r.by_source.keys().map(|src| {
            muted(format!(
                "{src}: day-1 high skill vs persistence {} • rain chance skill vs climatology {}",
                r.skill_vs_persistence(src).map(|v| format!("{:.0}%", v * 100.0)).unwrap_or_else(|| "–".into()),
                r.brier_skill(src).map(|v| format!("{:.0}%", v * 100.0)).unwrap_or_else(|| "–".into())
            ))
        }).collect::<Vec<_>>();
        let rank = |title: String, m: &std::collections::BTreeMap<String, verify::Errors>| {
            let list = verify::ranking(m).into_iter().enumerate().map(|(i, (s, mae, n))| format!("{}. {s} {mae:.1}° (n={n})", i + 1)).collect::<Vec<_>>().join("   ");
            row().gap(px(8.)).child(div().w(px(140.)).text_size(px(12.)).text_color(c(TEXT)).child(title)).child(muted(if list.is_empty() { "no pairs yet".to_string() } else { list }))
        };
        let by_place = r.by_place.iter().map(|(p, m)| rank(p.clone(), m)).collect::<Vec<_>>();
        let by_season = r.by_season.iter().map(|(p, m)| rank(p.clone(), m)).collect::<Vec<_>>();
        // Reliability: one bar row per bin for the station forecast (or the first source with rain chances).
        let rel_src = r.by_source.iter().find(|(s, x)| s.as_str() == "Station" && x.brier_n > 0).or_else(|| r.by_source.iter().find(|(_, x)| x.brier_n > 0));
        let reliability = rel_src.map(|(src, s)| {
            col().gap(px(3.)).child(muted(format!("Rain chance reliability ({src}): forecast chance vs how often it rained"))).children(s.reliability.iter().enumerate().filter(|(_, b)| b.0 > 0).map(|(i, (n, w, sum))| {
                let f = sum / *n as f32;
                let o = *w as f32 / *n as f32;
                row()
                    .gap(px(8.))
                    .child(div().w(px(90.)).text_size(px(12.)).text_color(c(TEXT)).child(format!("{}-{}%", i * 10, i * 10 + 10)))
                    .child(div().w(px(260.)).h(px(10.)).rounded(px(5.)).bg(c(0x1A2030)).relative().child(div().absolute().left(relative(f)).top_0().w(px(2.)).h_full().bg(c(0xFFFFFF))).child(div().h_full().w(relative(o)).rounded(px(5.)).bg(c(0x4F8FD6))))
                    .child(muted(format!("forecast {:.0}% • observed {:.0}% • {n} days", f * 100.0, o * 100.0)))
            }).collect::<Vec<_>>())
        });
        col()
            .gap(px(10.))
            .child(muted("Every day the station forecast (with your edits), the Open-Meteo blend and GFS / ECMWF / ICON are recorded for home and each forecast region, then checked against the nearest NWS station's measured highs, lows and rain (IEM daily summaries). Results need a few days to build up."))
            .child(row().gap(px(4.)).child(muted("Period")).children([7i64, 30, 90, 365].into_iter().enumerate().map(|(i, d)| {
                chip(("verif-days", i), format!("{d} days"), days == d).on_click(cx.listener(move |this, _, _, cx| {
                    this.days = d;
                    this.report = None;
                    cx.notify();
                }))
            }).collect::<Vec<_>>()).child(div().flex_1()).child(button_small("verif-refresh", "Recalculate").on_click(cx.listener(|this, _, _, cx| {
                this.report = None;
                cx.notify();
            }))).child(button_small("verif-report", "Export report").on_click(cx.listener(|this, _, _, cx| {
                let station = this.app.read(cx).settings.station_name.clone();
                match verify::export_report(this.days, &station) {
                    Ok(p) => {
                        let _ = open::that(&p);
                        this.toast(cx, ToastKind::Success, format!("Saved {}", p.display()));
                    }
                    Err(e) => this.toast(cx, ToastKind::Error, e.to_string()),
                }
            }))))
            .child(muted(if r.pairs == 0 {
                format!(
                    "No results yet: a forecast is checked once its day has passed. {} forecasts and {} observations recorded so far.",
                    r.recorded, r.observed
                )
            } else {
                format!("{} forecast / observation pairs", r.pairs)
            }))
            // Until something is checked, the empty table headings were just clutter.
            .when(r.pairs > 0, |d| {
                d.child(heading("Errors by source and lead day"))
                    .child(header)
                    .children(rows)
                    .child(heading("Skill"))
                    .children(skill)
                    .children(reliability)
                    .child(heading("Best source by place (high MAE)"))
                    .children(by_place)
                    .child(heading("Best source by season"))
                    .children(by_season)
            })
            .into_any_element()
    }
}

impl LabView {
    fn nowcast(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let data = self.app.read(cx).data.clone();
        let n = &data.nowcast;
        col()
            .gap(px(8.))
            .child(muted("Built every 30 seconds from tracked storms, the home sounding and the model grid. Show it on the map from the Map Studio side panel > Forecast > Storm nowcast."))
            .child(heading("Home, next hour"))
            .child(muted(format!(
                "Storm arrival chance {:.0}%{} • {}",
                n.home_arrival * 100.0,
                n.home_eta_min.map(|m| format!(" (about {m:.0} min)")).unwrap_or_default(),
                if n.initiation.is_empty() { "no new storms expected nearby".to_string() } else { format!("{} areas where new storms may form", n.initiation.len()) }
            )))
            .child(heading("Storms"))
            .when(n.cells.is_empty(), |d| d.child(muted("No storms tracked.")))
            .children(n.cells.iter().map(|cn| div().text_size(px(13.)).text_color(c(TEXT)).child(crate::nowcast::cell_line(cn, data.cells.iter().find(|s| s.id == cn.id)))).collect::<Vec<_>>())
            .child(heading("Nowcast vs what happened"))
            .child(muted(match n.verify_km {
                Some(km) => format!("30-minute positions were off by {:.1} mi on average ({} checked in the last 6 hours).", km / 1.609, n.verify_n),
                None => "Checks start 30 minutes after storms are first tracked.".to_string(),
            }))
            .into_any_element()
    }
}

impl Render for LabView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.tab;
        let tabs = row().gap(px(4.)).flex_wrap().pb(px(8.)).border_b_1().border_color(c(BORDER)).children(TABS.iter().enumerate().map(|(i, (k, t))| {
            let k: &'static str = k;
            chip(("lab-tab", i), *t, tab == k).on_click(cx.listener(move |this, _, window, cx| {
                this.tab = k;
                this.output.clear();
                this.input.update(cx, |s, cx| s.set_value(String::new(), window, cx));
                this.input2.update(cx, |s, cx| s.set_value(String::new(), window, cx));
                cx.notify();
            }))
        }).collect::<Vec<_>>());
        let body = match tab {
            "nowcast" => self.nowcast(cx),
            "files" => self.files.clone().into_any_element(),
            _ => self.verification(cx),
        };
        let _ = Input::new(&self.input);
        col().gap(px(10.)).child(tabs).child(body)
    }
}
