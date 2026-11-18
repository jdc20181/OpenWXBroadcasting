//! Community board dialog: school / business closings and warming / cooling centers. They feed
//! the Closings graphics and crawl, the Centers graphic and the centers map layer.

use gpui_kit::{
    component::input::{Input, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::{AppState, ToastKind},
    community::{Closing, Community},
    ui::{button_primary, button_small, c, chip, col, muted, row, section, BORDER, PANEL_2, TEXT},
};

pub struct CommunityBoard {
    app: Entity<AppState>,
    board: Community,
    closing_name: Entity<InputState>,
    closing_status: Entity<InputState>,
    center_name: Entity<InputState>,
    center_address: Entity<InputState>,
    center_hours: Entity<InputState>,
    #[allow(dead_code)]
    center_cooling: bool,
    /// Category for new closings.
    category: &'static str,
    /// Kind for new centers: warming, cooling or shelter.
    center_kind: &'static str,
}

const STATUSES: &[&str] = &["Closed", "2 hour delay", "1 hour delay", "Early dismissal", "E-learning day", "Virtual", "Canceled"];

impl CommunityBoard {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let make = |window: &mut Window, cx: &mut Context<Self>, p: &str| {
            let p = p.to_string();
            cx.new(|cx| InputState::new(window, cx).placeholder(p))
        };
        Self {
            app,
            board: crate::community::load(),
            closing_name: make(window, cx, "School, business or event"),
            closing_status: make(window, cx, "Status (e.g. 2 hour delay)"),
            center_name: make(window, cx, "Center name"),
            center_address: make(window, cx, "Address with town, e.g. 123 Main St, Jasper, IN"),
            center_hours: make(window, cx, "Hours (e.g. Open 24 hours)"),
            center_cooling: false,
            category: "schools",
            center_kind: "warming",
        }
    }

    pub fn load(&mut self, cx: &mut Context<Self>) {
        self.board = crate::community::load();
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        self.board.updated = chrono::Utc::now().timestamp();
        let result = crate::community::save(&self.board);
        self.app.update(cx, |s, cx| {
            match result {
                Ok(()) => {
                    s.reload_community();
                    s.toast(ToastKind::Success, "Community board saved; graphics update right away");
                }
                Err(e) => s.toast(ToastKind::Error, format!("Could not save the community board: {e}")),
            }
            cx.notify();
        });
        cx.notify();
    }

    fn take(input: &Entity<InputState>, window: &mut Window, cx: &mut Context<Self>) -> String {
        let v = input.read(cx).value().trim().to_string();
        input.update(cx, |s, cx| s.set_value(String::new(), window, cx));
        v
    }
}

impl Render for CommunityBoard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let closings = self
            .board
            .closings
            .iter()
            .enumerate()
            .map(|(i, c_)| {
                row()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .bg(c(PANEL_2))
                    .child(div().flex_1().min_w_0().text_size(px(13.)).text_color(c(TEXT)).child(c_.name.clone()))
                    .child(muted(c_.status.clone()))
                    .child(button_small(("cl-del", i), "Remove").on_click(cx.listener(move |this, _, _, cx| {
                        if i < this.board.closings.len() {
                            this.board.closings.remove(i);
                        }
                        this.save(cx);
                    })))
            })
            .collect::<Vec<_>>();
        let centers = self
            .board
            .centers
            .iter()
            .enumerate()
            .map(|(i, ce)| {
                let placed = ce.lat != 0.0;
                row()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .bg(c(PANEL_2))
                    .child(div().text_size(px(12.)).text_color(c(match ce.kind.as_str() { "cooling" => 0x5B93EE, "shelter" => 0x45C483, _ => 0xE9A23B })).child(match ce.kind.as_str() { "cooling" => "Cooling", "shelter" => "Shelter", _ => "Warming" }))
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_size(px(13.)).text_color(c(TEXT)).child(ce.name.clone()))
                            .child(muted(format!("{} • {}{}", ce.address, ce.hours, if placed { "" } else { " • not found on the map" }))),
                    )
                    .child(button_small(("ce-del", i), "Remove").on_click(cx.listener(move |this, _, _, cx| {
                        if i < this.board.centers.len() {
                            this.board.centers.remove(i);
                        }
                        this.save(cx);
                    })))
            })
            .collect::<Vec<_>>();
        let status_chips = STATUSES
            .iter()
            .enumerate()
            .map(|(i, s)| {
                chip(("cl-status", i), *s, false).on_click(cx.listener(move |this, _, window, cx| {
                    let st = s.to_string();
                    this.closing_status.update(cx, |state, cx| state.set_value(st, window, cx));
                }))
            })
            .collect::<Vec<_>>();
        let cooling = self.center_cooling;
        col()
            .gap(px(14.))
            .child(muted("Closings feed the Closings graphic and crawl; centers feed the Centers graphic and the map (Map Studio side panel > Warnings > Rain, flooding and rivers > Warming / cooling centers)."))
            .child(
                section(format!("Closings and delays ({})", self.board.closings.len()))
                    .gap(px(6.))
                    .children(closings)
                    .child(row().gap(px(4.)).children(crate::community::CATEGORIES.iter().enumerate().map(|(i, (key, label))| {
                        chip(("cl-cat", i), *label, self.category == *key).on_click(cx.listener(move |this, _, _, cx| {
                            this.category = *key;
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.closing_name))).child(div().w(px(220.)).child(Input::new(&self.closing_status))))
                    .child(row().flex_wrap().gap(px(4.)).children(status_chips))
                    .child(
                        row()
                            .gap(px(6.))
                            .child(button_primary("cl-add", "Add closing").on_click(cx.listener(|this, _, window, cx| {
                                let name = Self::take(&this.closing_name.clone(), window, cx);
                                let status = Self::take(&this.closing_status.clone(), window, cx);
                                if name.is_empty() {
                                    return;
                                }
                                let category = this.category.to_string();
                                this.board.closings.push(Closing { name, status: if status.is_empty() { "Closed".into() } else { status }, category });
                                this.save(cx);
                            })))
                            .child(button_small("cl-clear", "Clear all closings").on_click(cx.listener(|this, _, _, cx| {
                                this.board.closings.clear();
                                this.save(cx);
                            }))),
                    ),
            )
            .child(div().h(px(1.)).bg(c(BORDER)))
            .child(
                section(format!("Warming and cooling centers ({})", self.board.centers.len()))
                    .gap(px(6.))
                    .children(centers)
                    .child(row().gap(px(6.)).children([("warming", "Warming"), ("cooling", "Cooling"), ("shelter", "Emergency shelter")].into_iter().enumerate().map(|(i, (k, l))| {
                        chip(("ce-kind", i), l, self.center_kind == k).on_click(cx.listener(move |this, _, _, cx| {
                            this.center_kind = k;
                            this.center_cooling = k == "cooling";
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(Input::new(&self.center_name))
                    .child(Input::new(&self.center_address))
                    .child(Input::new(&self.center_hours))
                    .child(button_primary("ce-add", "Add center").on_click(cx.listener(|this, _, window, cx| {
                        let name = Self::take(&this.center_name.clone(), window, cx);
                        let address = Self::take(&this.center_address.clone(), window, cx);
                        let hours = Self::take(&this.center_hours.clone(), window, cx);
                        if name.is_empty() {
                            return;
                        }
                        let kind = this.center_kind;
                        let line = format!("{kind} | {name} | {address} | {hours}");
                        let mut parsed = Community::parse_centers(&line, &[]);
                        this.board.centers.append(&mut parsed);
                        this.save(cx);
                    }))),
            )
    }
}
