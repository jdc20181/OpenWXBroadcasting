//! Forecaster desk (modal): projects, saved analysis sessions, notebook, decisions, event
//! bookmarks, quick calculations and workspace performance.

use gpui_kit::{
    component::input::{Input, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::{AppState, ToastKind},
    desk::{self, AnalysisSession, Bookmark, Desk, Entry, Project},
    ui::{button_small, c, chip, col, heading, muted, row, BORDER, MUTED, TEXT},
};

pub struct DeskView {
    app: Entity<AppState>,
    desk: Desk,
    entry: Entity<InputState>,
    due: Entity<InputState>,
    calc: Entity<InputState>,
    calc_result: String,
    _subscriptions: Vec<Subscription>,
}

fn when(t: i64) -> String {
    chrono::DateTime::from_timestamp(t, 0)
        .map(|d| d.with_timezone(&chrono::Local).format("%b %-d %-I:%M %p").to_string())
        .unwrap_or_default()
}

impl DeskView {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let entry = cx.new(|cx| InputState::new(window, cx).placeholder("Name, note, decision or bookmark label"));
        let due = cx.new(|cx| InputState::new(window, cx).placeholder("Due (e.g. Fri 5 PM)"));
        let calc = cx.new(|cx| InputState::new(window, cx).placeholder("hi 95 60 • wc 10 20 • td 80 65 • kt 40"));
        Self {
            app: app.clone(),
            desk: desk::load(),
            entry,
            due,
            calc,
            calc_result: String::new(),
            _subscriptions: vec![cx.observe(&app, |_, _, cx| cx.notify())],
        }
    }

    fn text(&self, cx: &App) -> String {
        self.entry.read(cx).value().trim().to_string()
    }

    fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.entry.update(cx, |s, cx| s.set_value(String::new(), window, cx));
        self.due.update(cx, |s, cx| s.set_value(String::new(), window, cx));
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        desk::save(&self.desk);
        cx.notify();
    }
}

impl Render for DeskView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Re-read so decisions logged elsewhere (forecast publishes) show up.
        let disk = desk::load();
        self.desk.decisions = disk.decisions;
        let d = self.desk.clone();
        let state = self.app.read(cx);
        let stats = state.compositor.stats();
        let mem = state.ext.memory_mb;
        let tick = state.ext.tick_ms;
        let sources = crate::broadcast_ops::source_stats();
        let avg_req = if sources.is_empty() { 0 } else { sources.iter().map(|(_, s)| s.avg_ms()).sum::<u64>() / sources.len() as u64 };

        let projects = d
            .projects
            .iter()
            .enumerate()
            .map(|(i, p)| {
                row()
                    .gap(px(6.))
                    .child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(format!("{}{}", p.name, if p.due.is_empty() { String::new() } else { format!(" • due {}", p.due) })))
                    .child(chip(("proj-status", i), if p.status.is_empty() { "Planned".to_string() } else { p.status.clone() }, p.status == "Done").on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(p) = this.desk.projects.get_mut(i) {
                            p.status = match p.status.as_str() { "" | "Planned" => "Working", "Working" => "Done", _ => "Planned" }.to_string();
                        }
                        this.commit(cx);
                    })))
                    .child(button_small(("proj-del", i), "✕").on_click(cx.listener(move |this, _, _, cx| {
                        if i < this.desk.projects.len() {
                            this.desk.projects.remove(i);
                        }
                        this.commit(cx);
                    })))
            })
            .collect::<Vec<_>>();
        let sessions = d
            .sessions
            .iter()
            .enumerate()
            .rev()
            .take(8)
            .map(|(i, s)| {
                row()
                    .gap(px(6.))
                    .child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(format!("{} • {}", s.name, when(s.saved))))
                    .child(button_small(("sess-open", i), "Open").on_click(cx.listener(move |this, _, _, cx| {
                        let Some(sess) = this.desk.sessions.get(i).cloned() else { return };
                        this.app.update(cx, |s, cx| {
                            let view = sess.map.view;
                            s.live = sess.map;
                            s.set_view(view);
                            s.sync_map();
                            s.toast(ToastKind::Info, format!("Analysis session \"{}\" restored", sess.name));
                            cx.notify();
                        });
                    })))
                    .child(button_small(("sess-del", i), "✕").on_click(cx.listener(move |this, _, _, cx| {
                        if i < this.desk.sessions.len() {
                            this.desk.sessions.remove(i);
                        }
                        this.commit(cx);
                    })))
            })
            .collect::<Vec<_>>();
        let entries = |list: &[Entry]| list.iter().rev().take(8).map(|e| muted(format!("{}  {}", when(e.time), e.text))).collect::<Vec<_>>();
        let bookmarks = d
            .bookmarks
            .iter()
            .enumerate()
            .rev()
            .take(10)
            .map(|(i, b)| {
                row()
                    .gap(px(6.))
                    .child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(format!("{} • {}", b.label, when(b.time))))
                    .child(button_small(("bm-go", i), "Go").on_click(cx.listener(move |this, _, _, cx| {
                        let Some(b) = this.desk.bookmarks.get(i).cloned() else { return };
                        this.app.update(cx, |s, cx| {
                            s.fly_to(b.view, 1.4);
                            if let Some(t) = b.radar_time {
                                s.jump_to_radar_time(t);
                            }
                            cx.notify();
                        });
                    })))
                    .child(button_small(("bm-del", i), "✕").on_click(cx.listener(move |this, _, _, cx| {
                        if i < this.desk.bookmarks.len() {
                            this.desk.bookmarks.remove(i);
                        }
                        this.commit(cx);
                    })))
            })
            .collect::<Vec<_>>();
        let block = |title: &str| col().gap(px(6.)).p(px(12.)).rounded(px(8.)).border_1().border_color(c(BORDER)).child(heading(title.to_string()));

        col()
            .gap(px(12.))
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.entry))).child(div().w(px(180.)).child(Input::new(&self.due))))
            .child(
                row()
                    .flex_wrap()
                    .gap(px(6.))
                    .child(button_small("desk-proj", "Add project").on_click(cx.listener(|this, _, window, cx| {
                        let name = this.text(cx);
                        if name.is_empty() {
                            return;
                        }
                        let due = this.due.read(cx).value().trim().to_string();
                        this.desk.projects.push(Project { name, due, status: "Planned".into() });
                        this.clear(window, cx);
                        this.commit(cx);
                    })))
                    .child(button_small("desk-session", "Save analysis session").on_click(cx.listener(|this, _, window, cx| {
                        let name = this.text(cx);
                        let (map, warnings) = {
                            let s = this.app.read(cx);
                            (s.live.clone(), s.coverage_warnings().iter().map(|a| a.id.clone()).collect::<Vec<_>>())
                        };
                        let name = if name.is_empty() { format!("Session {}", chrono::Local::now().format("%b %-d %-I:%M %p")) } else { name };
                        this.desk.sessions.push(AnalysisSession { name, saved: chrono::Utc::now().timestamp(), map, warnings });
                        this.clear(window, cx);
                        this.commit(cx);
                    })))
                    .child(button_small("desk-note", "Add to notebook").on_click(cx.listener(|this, _, window, cx| {
                        let text = this.text(cx);
                        if text.is_empty() {
                            return;
                        }
                        this.desk.notebook.push(Entry { time: chrono::Utc::now().timestamp(), text });
                        this.clear(window, cx);
                        this.commit(cx);
                    })))
                    .child(button_small("desk-decision", "Log a decision").on_click(cx.listener(|this, _, window, cx| {
                        let text = this.text(cx);
                        if text.is_empty() {
                            return;
                        }
                        desk::log_decision(&text);
                        this.clear(window, cx);
                        cx.notify();
                    })))
                    .child(button_small("desk-bookmark", "Bookmark this moment").on_click(cx.listener(|this, _, window, cx| {
                        let label = this.text(cx);
                        let (view, radar_time) = {
                            let s = this.app.read(cx);
                            (s.live.view, s.current_radar_time())
                        };
                        let label = if label.is_empty() { "Bookmark".to_string() } else { label };
                        this.desk.bookmarks.push(Bookmark { time: chrono::Utc::now().timestamp(), label, radar_time, view });
                        this.clear(window, cx);
                        this.commit(cx);
                    }))),
            )
            .child(
                row()
                    .items_start()
                    .gap(px(12.))
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(12.))
                            .child(block("Forecast projects").when(projects.is_empty(), |b| b.child(muted("No projects yet"))).children(projects))
                            .child(block("Saved analysis sessions").when(sessions.is_empty(), |b| b.child(muted("Save the map setup and warnings you are working with"))).children(sessions))
                            .child(block("Event bookmarks").when(bookmarks.is_empty(), |b| b.child(muted("Bookmark a radar frame and view to come back to"))).children(bookmarks)),
                    )
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(12.))
                            .child(block("Notebook").when(d.notebook.is_empty(), |b| b.child(muted("Timestamped forecaster notes"))).children(entries(&d.notebook)))
                            .child(block("Decision history").when(d.decisions.is_empty(), |b| b.child(muted("Published forecasts and logged decisions appear here"))).children(entries(&d.decisions)))
                            .child(
                                block("Quick calculations")
                                    .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.calc))).child(button_small("calc-go", "Calculate").on_click(cx.listener(|this, _, _, cx| {
                                        let q = this.calc.read(cx).value().to_string();
                                        this.calc_result = desk::calculate(&q);
                                        cx.notify();
                                    }))))
                                    .when(!self.calc_result.is_empty(), |b| b.child(div().text_size(px(15.)).text_color(c(TEXT)).child(self.calc_result.clone()))),
                            )
                            .child(
                                block("Workspace performance")
                                    .child(muted(format!("Program: {:.0} fps, {:.1} ms per frame (worst {:.0} ms), {} late frames", stats.fps, stats.render_ms, stats.worst_ms, stats.late_frames)))
                                    .child(muted(format!("App background work: {tick:.2} ms per tick")))
                                    .child(muted({
                                        let (waiting, running, done) = crate::jobs::status();
                                        format!("Background jobs: {waiting} waiting, {running} running, {done} done")
                                    }))
                                    .child(muted(match state.ext.quality_level {
                                        0 => "Radar detail: full".to_string(),
                                        l => format!("Radar detail: reduced (level {l}) to keep frames on time"),
                                    }))
                                    .child(muted(match mem { Some(m) => format!("Memory: {m:.0} MB"), None => "Memory: checking…".into() }))
                                    .child(muted(format!("Data requests: {avg_req} ms average across {} sources", sources.len())))
                                    .child(div().h(px(1.)).bg(c(BORDER)))
                                    .child(div().text_size(px(11.)).text_color(c(MUTED)).child("Find Low-resource mode and tile cache size in Settings → System → Performance.")),
                            ),
                    ),
            )
    }
}
