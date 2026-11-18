//! Event history (modal): search a date and time, load what happened (warnings, reports,
//! observations, the archived forecast), replay it on the map, save it as a case study and export
//! it as a show.

use gpui_kit::{
    component::{
        date_picker::{DatePicker, DatePickerEvent, DatePickerState},
        input::{Input, InputState},
        time_field::TimePrecision,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::{AppState, ToastKind},
    history::{self, CaseStudy},
    ui::{button_primary, button_small, c, chip, col, heading, muted, row, BORDER, TEXT},
};

/// A typed start time: "2024-05-07 16:00", "2024-05-07 4:00 PM", "5/7/2024 16:00",
/// "5/7/2024 4 PM" or a date alone (midnight).
fn parse_start(raw: &str) -> Option<chrono::NaiveDateTime> {
    let mut parts: Vec<String> = raw.split_whitespace().map(str::to_uppercase).collect();
    // "4 PM" → "4:00 PM" (chrono needs the minutes).
    if parts.len() == 3 && matches!(parts[2].as_str(), "AM" | "PM") && !parts[1].contains(':') {
        parts[1].push_str(":00");
    }
    let raw = parts.join(" ");
    for f in ["%Y-%m-%d %H:%M", "%Y-%m-%d %I:%M %p", "%m/%d/%Y %H:%M", "%m/%d/%Y %I:%M %p"] {
        if let Ok(t) = chrono::NaiveDateTime::parse_from_str(&raw, f) {
            return Some(t);
        }
    }
    ["%Y-%m-%d", "%m/%d/%Y"].iter().find_map(|f| chrono::NaiveDate::parse_from_str(&raw, f).ok()).and_then(|d| d.and_hms_opt(0, 0, 0))
}

pub struct HistoryView {
    app: Entity<AppState>,
    when: Entity<InputState>,
    /// Calendar + time picker beside the start box: picking fills the box (typing still works).
    picker: Entity<DatePickerState>,
    name: Entity<InputState>,
    hours: i64,
    loading: bool,
    cases: Vec<CaseStudy>,
    _subscriptions: Vec<Subscription>,
}

impl HistoryView {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let when = cx.new(|cx| InputState::new(window, cx).placeholder("Start: 2024-05-07 16:00 (local time)"));
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Name for this event, e.g. May 7 tornado outbreak"));
        let picker = cx.new(|cx| {
            let mut state = DatePickerState::new(window, cx).time_precision(TimePrecision::Minute).date_format("%Y-%m-%d");
            // Archives go back to the 1990s (IEM warnings from 2002, NEXRAD from the 1990s).
            state.set_year_range((1990, chrono::Datelike::year(&chrono::Local::now())), cx);
            state
        });
        let when_box = when.clone();
        let picked = cx.subscribe_in(&picker, window, move |_, _, event: &DatePickerEvent, window, cx| {
            let DatePickerEvent::Change(value) = event;
            if let Some(start) = value.start() {
                let text = start.format("%Y-%m-%d %H:%M").to_string();
                when_box.update(cx, |box_, cx| box_.set_value(text, window, cx));
            }
        });
        Self {
            app: app.clone(),
            when,
            picker,
            name,
            hours: 12,
            loading: false,
            cases: history::load_cases(),
            _subscriptions: vec![cx.observe(&app, |_, _, cx| cx.notify()), picked],
        }
    }

    fn case_from_inputs(&self, cx: &App) -> Option<CaseStudy> {
        let raw = self.when.read(cx).value().trim().to_string();
        let local = parse_start(&raw)?;
        let start = local.and_local_timezone(chrono::Local).single()?.timestamp();
        let home = self.app.read(cx).settings.home;
        let d = self.app.read(cx).settings.coverage_miles.max(50.0) * 1.5 / 69.0;
        let name = self.name.read(cx).value().trim().to_string();
        Some(CaseStudy {
            name: if name.is_empty() { local.format("%b %-d, %Y").to_string() } else { name },
            start,
            end: start + self.hours * 3600,
            bbox: (home.lat - d, home.lat + d, home.lon - d * 1.3, home.lon + d * 1.3),
            notes: String::new(),
        })
    }

    fn load(&mut self, case: CaseStudy, cx: &mut Context<Self>) {
        self.loading = true;
        cx.notify();
        let home = self.app.read(cx).settings.home;
        cx.spawn(async move |this, cx| {
            let result = crate::net::spawn(history::load_event(case, (home.lat, home.lon))).await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                this.app.update(cx, |s, cx| {
                    match result {
                        Ok(Ok(ev)) => {
                            let start = ev.case.start;
                            let mut data = (*s.data).clone();
                            s.toast(ToastKind::Success, format!("Loaded {}: {} warnings, {} reports", ev.case.name, ev.warnings.len(), ev.reports.len()));
                            data.history_event = Some(std::sync::Arc::new(ev));
                            s.data = std::sync::Arc::new(data);
                            let snapshot = s.data.clone();
                            s.compositor.update(|p| p.data = snapshot);
                            s.update_live(|l| { l.radar_replay = None; l.site_replay = None; l.history_time = Some(start); });
                        }
                        Ok(Err(e)) => s.toast(ToastKind::Error, format!("Couldn't load the event: {e}")),
                        Err(e) => s.toast(ToastKind::Error, format!("Event worker failed: {e}")),
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn step(&mut self, minutes: i64, cx: &mut Context<Self>) {
        self.app.update(cx, |s, cx| {
            let (start, end) = s.data.history_event.as_ref().map(|e| (e.case.start, e.case.end)).unwrap_or((0, i64::MAX));
            s.update_live(|l| {
                if let Some(t) = l.history_time {
                    l.history_time = Some((t + minutes * 60).clamp(start, end));
                }
            });
            cx.notify();
        });
    }
}

impl Render for HistoryView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let event = state.data.history_event.clone();
        let replay = state.live.history_time;
        let hours = self.hours;
        let cases = self.cases.clone();
        col()
            .gap(px(12.))
            .child(muted("Find a past event: warnings and storm reports come from the Iowa Environmental Mesonet archive, radar from the NEXRAD archive, satellite from NASA, observations from the nearest airport station (ASOS) and the forecast made at the time from NOAA GFS MOS, both through the Iowa Environmental Mesonet."))
            .child(row().gap(px(6.))
                .child(div().w(px(320.)).child(Input::new(&self.when)))
                .child(div().w(px(220.)).child(DatePicker::new(&self.picker).placeholder("Pick date and time")))
                .child(div().flex_1().child(Input::new(&self.name))))
            .child(
                row()
                    .gap(px(4.))
                    .flex_wrap()
                    .children([3i64, 6, 12, 24, 48].into_iter().enumerate().map(|(i, h)| {
                        chip(("hist-hours", i), format!("{h} hours"), hours == h).on_click(cx.listener(move |this, _, _, cx| {
                            this.hours = h;
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>())
                    .child(button_primary("hist-load", if self.loading { "Loading…" } else { "Load event" }).on_click(cx.listener(|this, _, _, cx| {
                        match this.case_from_inputs(cx) {
                            Some(case) => this.load(case, cx),
                            None => this.app.update(cx, |s, cx| {
                                s.toast(ToastKind::Error, "Pick a date, or type a start like 2024-05-07 16:00 or 5/7/2024 4 PM");
                                cx.notify();
                            }),
                        }
                    }))),
            )
            .when_some(event, |d, ev| {
                let timeline = ev.timeline.iter().take(40).map(|(t, text)| {
                    let when = chrono::DateTime::from_timestamp(*t, 0).map(|x| x.with_timezone(&chrono::Local).format("%-I:%M %p").to_string()).unwrap_or_default();
                    muted(format!("{when}  {text}"))
                }).collect::<Vec<_>>();
                let label = replay.and_then(|t| chrono::DateTime::from_timestamp(t, 0)).map(|x| x.with_timezone(&chrono::Local).format("%b %-d %-I:%M %p").to_string()).unwrap_or_else(|| "off".into());
                d.child(
                    col()
                        .gap(px(6.))
                        .p(px(12.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(c(BORDER))
                        .child(heading(format!("{} • {} warnings • {} reports", ev.case.name, ev.warnings.len(), ev.reports.len())))
                        .child(
                            row()
                                .gap(px(6.))
                                .flex_wrap()
                                .child(div().text_size(px(13.)).text_color(c(TEXT)).child(format!("Map replay: {label}")))
                                .child(button_small("hist-back", "-30 min").on_click(cx.listener(|this, _, _, cx| this.step(-30, cx))))
                                .child(button_small("hist-back5", "-5 min").on_click(cx.listener(|this, _, _, cx| this.step(-5, cx))))
                                .child(button_small("hist-fwd5", "+5 min").on_click(cx.listener(|this, _, _, cx| this.step(5, cx))))
                                .child(button_small("hist-fwd", "+30 min").on_click(cx.listener(|this, _, _, cx| this.step(30, cx))))
                                .child(button_small("hist-live", "Back to live").on_click(cx.listener(|this, _, _, cx| {
                                    this.app.update(cx, |s, cx| {
                                        s.update_live(|l| l.history_time = None);
                                        cx.notify();
                                    });
                                })))
                                .child(button_small("hist-save", "Save case study").on_click(cx.listener(|this, _, _, cx| {
                                    let case = this.app.read(cx).data.history_event.as_ref().map(|e| e.case.clone());
                                    if let Some(case) = case {
                                        this.cases.retain(|c| c.name != case.name);
                                        this.cases.push(case);
                                        history::save_cases(&this.cases);
                                        cx.notify();
                                    }
                                })))
                                .child(button_small("hist-show", "Export as a show").on_click(cx.listener(|this, _, _, cx| {
                                    this.app.update(cx, |s, cx| {
                                        let Some(ev) = s.data.history_event.clone() else { return };
                                        // One map scene per hour of the event, replaying the archive, then the timeline graphic.
                                        let mut show = crate::show::Show::new(&format!("Case study: {}", ev.case.name));
                                        let mut t = ev.case.start;
                                        while t <= ev.case.end {
                                            let mut map = s.live.clone();
                                            map.radar_replay = None;
                                            map.site_replay = None;
                                            map.history_time = Some(t);
                                            map.warnings = true;
                                            let title = chrono::DateTime::from_timestamp(t, 0).map(|x| x.with_timezone(&chrono::Local).format("%-I %p").to_string()).unwrap_or_default();
                                            let mut scene = crate::show::Scene::map(&title, map);
                                            scene.duration = 6.0;
                                            show.scenes.push(scene);
                                            t += 3600;
                                        }
                                        if let Some(doc) = crate::gfx::templates::build("hist_timeline").and_then(|d| s.save_graphic(d)) {
                                            show.scenes.push(crate::show::Scene::graphic(&doc));
                                        }
                                        let name = show.name.clone();
                                        s.show = show;
                                        s.workspace = crate::app::Workspace::Show;
                                        s.modal = crate::app::Modal::None;
                                        s.toast(ToastKind::Success, format!("\"{name}\" is in the Show workspace"));
                                        cx.notify();
                                    });
                                }))),
                        )
                        .children(timeline),
                )
            })
            .when(!cases.is_empty(), |d| {
                d.child(heading("Case studies")).children(cases.into_iter().enumerate().map(|(i, case)| {
                    let when = chrono::DateTime::from_timestamp(case.start, 0).map(|x| x.with_timezone(&chrono::Local).format("%b %-d, %Y").to_string()).unwrap_or_default();
                    let open = case.clone();
                    row()
                        .gap(px(6.))
                        .child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(format!("{} • {when}", case.name)))
                        .child(button_small(("case-open", i), "Open").on_click(cx.listener(move |this, _, _, cx| this.load(open.clone(), cx))))
                        .child(button_small(("case-del", i), "✕").on_click(cx.listener(move |this, _, _, cx| {
                            if i < this.cases.len() {
                                this.cases.remove(i);
                                history::save_cases(&this.cases);
                            }
                            cx.notify();
                        })))
                }).collect::<Vec<_>>())
            })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn typed_starts() {
        let want = chrono::NaiveDate::from_ymd_opt(2024, 5, 7).unwrap().and_hms_opt(16, 0, 0).unwrap();
        for raw in ["2024-05-07 16:00", "2024-05-07 4:00 PM", "2024-05-07 4 pm", "5/7/2024 16:00", "05/07/2024 4:00 PM", " 5/7/2024 4 PM "] {
            assert_eq!(super::parse_start(raw), Some(want), "{raw}");
        }
        assert_eq!(super::parse_start("2024-05-07"), Some(want.date().and_hms_opt(0, 0, 0).unwrap()));
        assert_eq!(super::parse_start("yesterday"), None);
    }
}
