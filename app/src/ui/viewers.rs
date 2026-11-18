//! Viewer submissions (modal): moderate photos, video and reports, put questions on air, and run
//! live polls. Only approved items reach graphics and the map.

use gpui_kit::{
    component::input::{Input, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::{AppState, ToastKind},
    portal,
    ui::{badge, button_small, c, chip, col, heading, muted, row, BORDER, TEXT},
};

pub struct ViewersView {
    app: Entity<AppState>,
    filter: &'static str,
    poll_q: Entity<InputState>,
    poll_opts: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl ViewersView {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let poll_q = cx.new(|cx| InputState::new(window, cx).placeholder("Poll question, e.g. How much snow did you get?"));
        let poll_opts = cx.new(|cx| InputState::new(window, cx).placeholder("Options separated by ; e.g. Under 2\"; 2-4\"; 4-6\"; Over 6\""));
        Self { app: app.clone(), filter: "pending", poll_q, poll_opts, _subscriptions: vec![cx.observe(&app, |_, _, cx| cx.notify())] }
    }

    fn act(&mut self, cx: &mut Context<Self>, f: impl FnOnce() -> anyhow::Result<String>) {
        let result = f();
        self.app.update(cx, |s, cx| {
            match result {
                Ok(m) => s.toast(ToastKind::Success, m),
                Err(e) => s.toast(ToastKind::Error, e.to_string()),
            }
            // Pick the change up right away.
            let mut data = (*s.data).clone();
            data.viewer = portal::list();
            s.data = std::sync::Arc::new(data);
            s.reload_library();
            cx.notify();
        });
    }
}

impl Render for ViewersView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let subs = self.app.read(cx).data.viewer.clone();
        let filter = self.filter;
        let poll = portal::poll();
        let shown: Vec<portal::Submission> = subs.iter().filter(|s| filter == "all" || s.status == filter || (filter == "questions" && s.kind == "question")).cloned().collect();
        let rows = shown
            .into_iter()
            .enumerate()
            .map(|(i, s)| {
                let when = chrono::DateTime::from_timestamp(s.time, 0).map(|d| d.with_timezone(&chrono::Local).format("%b %-d %-I:%M %p").to_string()).unwrap_or_default();
                let file = portal::dir().join(&s.id).join(&s.file);
                let (id1, id2, id3, id_air) = (s.id.clone(), s.id.clone(), s.id.clone(), s.id.clone());
                let is_photo = s.kind == "photo" && file.exists();
                row()
                    .gap(px(10.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(c(BORDER))
                    .when(is_photo, |d| d.child(crate::ui::thumb_box_path(file.clone(), 120., 68., 0x05070B, true).rounded(px(4.))))
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(row().gap(px(6.)).child(badge(s.kind.to_uppercase(), 0x4F8FD6)).child(badge(s.status.to_uppercase(), if s.status == "approved" { 0x3DD68C } else if s.status == "rejected" { 0xE5383B } else { 0xE0A030 })).when(!s.consent, |d| d.child(badge("NO CONSENT", 0xE5383B))))
                            .child(div().text_size(px(13.)).text_color(c(TEXT)).child(format!("{}{} • {}{}", if s.name.is_empty() { "Anonymous" } else { &s.name }, if s.place.is_empty() { String::new() } else { format!(", {}", s.place) }, when, if s.report_type.is_empty() { String::new() } else { format!(" • {}", s.report_type) })))
                            .when(!s.text.is_empty(), |d| d.child(muted(s.text.clone())))
                            .when(s.kind == "video" && file.exists(), |d| {
                                let f = file.clone();
                                d.child(button_small(("sub-play", i), "Open video").on_click(move |_, _, _| {
                                    let _ = open::that(&f);
                                }))
                            }),
                    )
                    .child(
                        col()
                            .gap(px(4.))
                            .when(s.kind != "question", |d| {
                                d.child(button_small(("sub-ok", i), "Approve for air").on_click(cx.listener(move |this, _, _, cx| {
                                    let id = id1.clone();
                                    this.act(cx, || portal::approve(&id).map(|m| m.map(|n| format!("Approved: {n} is in Media")).unwrap_or_else(|| "Approved".into())));
                                })))
                            })
                            .when(s.kind == "question", |d| {
                                d.child(button_small(("sub-air", i), "Put on air").on_click(cx.listener(move |this, _, _, cx| {
                                    let id = id_air.clone();
                                    this.act(cx, || {
                                        // Only one question on air at a time.
                                        for q in portal::list().into_iter().filter(|q| q.status == "onair") {
                                            portal::set_status(&q.id, "answered")?;
                                        }
                                        portal::set_status(&id, "onair")?;
                                        Ok("Question on air: use the Viewer Question graphic".into())
                                    });
                                })))
                            })
                            .child(button_small(("sub-no", i), "Reject").on_click(cx.listener(move |this, _, _, cx| {
                                let id = id2.clone();
                                this.act(cx, || portal::set_status(&id, "rejected").map(|_| "Rejected".into()));
                            })))
                            .child(button_small(("sub-done", i), "Mark answered").on_click(cx.listener(move |this, _, _, cx| {
                                let id = id3.clone();
                                this.act(cx, || portal::set_status(&id, "answered").map(|_| "Done".into()));
                            }))),
                    )
            })
            .collect::<Vec<_>>();
        let total = poll.votes.iter().sum::<u32>().max(1) as f32;
        col()
            .gap(px(12.))
            .child(muted("Everything viewers send waits here. Approved photos go into Media and the Viewer Photo graphics; approved reports with a location show on the map (Viewer reports layer)."))
            .child(row().gap(px(4.)).children([("pending", "New"), ("approved", "Approved"), ("questions", "Questions"), ("rejected", "Rejected"), ("all", "All")].into_iter().enumerate().map(|(i, (k, t))| {
                let n = subs.iter().filter(|s| k == "all" || s.status == k || (k == "questions" && s.kind == "question")).count();
                chip(("sub-filter", i), format!("{t} ({n})"), filter == k).on_click(cx.listener(move |this, _, _, cx| {
                    this.filter = k;
                    cx.notify();
                }))
            }).collect::<Vec<_>>()))
            .when(rows.is_empty(), |d| d.child(muted("Nothing here.")))
            .children(rows)
            .child(heading("Live poll"))
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.poll_q))))
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.poll_opts))).child(button_small("poll-start", "Start poll").on_click(cx.listener(|this, _, _, cx| {
                let q = this.poll_q.read(cx).value().trim().to_string();
                let opts: Vec<String> = this.poll_opts.read(cx).value().split(';').map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).collect();
                if q.is_empty() || opts.len() < 2 {
                    return;
                }
                portal::set_poll(&q, opts);
                cx.notify();
            }))).child(button_small("poll-close", "Close poll").on_click(cx.listener(|_, _, _, cx| {
                portal::close_poll();
                cx.notify();
            }))))
            .when(!poll.question.is_empty(), |d| {
                d.child(muted(format!("{} ({}) • {} votes", poll.question, if poll.open { "open" } else { "closed" }, poll.votes.iter().sum::<u32>())))
                    .children(poll.options.iter().zip(&poll.votes).map(|(o, v)| {
                        let pct = *v as f32 / total;
                        row()
                            .gap(px(8.))
                            .child(div().w(px(160.)).text_size(px(13.)).text_color(c(TEXT)).child(o.clone()))
                            .child(div().flex_1().h(px(10.)).rounded(px(5.)).bg(c(0x1A2030)).child(div().h_full().w(relative(pct)).rounded(px(5.)).bg(c(0x4F8FD6))))
                            .child(muted(format!("{:.0}%", pct * 100.0)))
                    }).collect::<Vec<_>>())
            })
    }
}
