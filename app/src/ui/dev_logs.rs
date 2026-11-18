//! Developer Logs: a drawer over the bottom of the window with everything the app logged, failed
//! requests (status, URL, response) and error toasts. Filter by kind, search, copy for sharing,
//! open the log folder. Ctrl+Shift+D or the counter in the status bar opens it.

use gpui_kit::{
    component::input::{Input, InputEvent, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    devlog::{self, Entry, Level},
    ui::{button_small, c, ca, chip, col, muted, row, AMBER, BORDER, CYAN, DIM, MUTED, PANEL, RED, TEXT},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Problems,
    Errors,
    Network,
    Toasts,
}

impl Filter {
    const ALL: [Filter; 5] = [Filter::Problems, Filter::Errors, Filter::Network, Filter::Toasts, Filter::All];

    fn label(self) -> &'static str {
        match self {
            Filter::All => "Everything",
            Filter::Problems => "Errors + warnings",
            Filter::Errors => "Errors",
            Filter::Network => "Network",
            Filter::Toasts => "Notifications",
        }
    }

    fn keep(self, e: &Entry) -> bool {
        match self {
            Filter::All => true,
            Filter::Problems => e.level <= Level::Warn,
            Filter::Errors => e.level == Level::Error,
            Filter::Network => e.is_network(),
            Filter::Toasts => e.source == "toast",
        }
    }
}

/// Sources that log a lot of routine detail (map engine, GPU).
fn is_engine(e: &Entry) -> bool {
    ["maplibre", "wgpu", "naga", "winit", "gpui", "blade"].iter().any(|p| e.source.starts_with(p))
}

const SHOWN: usize = 400;

pub struct DevLogs {
    filter: Filter,
    app_only: bool,
    search: Entity<InputState>,
    query: String,
    /// Height of the drawer in px.
    pub height: f32,
    copied: Option<std::time::Instant>,
    _subscriptions: Vec<Subscription>,
}

impl DevLogs {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search source or message (e.g. 429, open-meteo, radar)"));
        let subscriptions = vec![cx.subscribe_in(&search, window, |this, state, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.query = state.read(cx).value().to_string().to_lowercase();
                cx.notify();
            }
        })];
        Self {
            filter: Filter::Problems,
            app_only: false,
            search,
            query: String::new(),
            height: 320.0,
            copied: None,
            _subscriptions: subscriptions,
        }
    }

    fn visible(&self) -> Vec<Entry> {
        let mut list: Vec<Entry> = devlog::snapshot()
            .into_iter()
            .filter(|e| self.filter.keep(e))
            .filter(|e| !self.app_only || !is_engine(e))
            .filter(|e| {
                self.query.is_empty()
                    || e.message.to_lowercase().contains(&self.query)
                    || e.source.to_lowercase().contains(&self.query)
            })
            .collect();
        // Newest first.
        list.reverse();
        list
    }

    fn copy(&mut self, entries: &[Entry], cx: &mut Context<Self>) {
        let mut ordered = entries.to_vec();
        ordered.reverse();
        cx.write_to_clipboard(ClipboardItem::new_string(devlog::report(&ordered)));
        self.copied = Some(std::time::Instant::now());
        cx.notify();
    }
}

impl Render for DevLogs {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entries = self.visible();
        let total = entries.len();
        let (errors, warnings) = devlog::counts();
        let copied = self.copied.map(|t| t.elapsed().as_secs() < 3).unwrap_or(false);

        let filters = Filter::ALL
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let f = *f;
                chip(("devlog-filter", i), f.label(), self.filter == f).on_click(cx.listener(move |this, _, _, cx| {
                    this.filter = f;
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();

        let rows = entries
            .iter()
            .take(SHOWN)
            .enumerate()
            .map(|(i, e)| {
                let color = match e.level {
                    Level::Error => RED,
                    Level::Warn => AMBER,
                    Level::Info => CYAN,
                    Level::Debug => DIM,
                };
                let line = e.line();
                row()
                    .id(("devlog-row", i))
                    .items_start()
                    .gap(px(10.))
                    .px(px(10.))
                    .py(px(3.))
                    .border_b_1()
                    .border_color(ca(BORDER, 0.5))
                    .hover(|s| s.bg(ca(0x2A3449, 0.6)))
                    .cursor_pointer()
                    .text_size(px(12.))
                    .font_family("Cascadia Mono")
                    .child(div().w(px(84.)).flex_shrink_0().text_color(c(MUTED)).child(e.at.format("%H:%M:%S%.3f").to_string()))
                    .child(div().w(px(48.)).flex_shrink_0().text_color(c(color)).child(e.level.label()))
                    .child(div().w(px(190.)).flex_shrink_0().overflow_hidden().text_color(c(MUTED)).child(e.source.clone()))
                    .child(div().flex_1().min_w_0().text_color(c(TEXT)).child(e.message.clone()))
                    // Click a line to copy it.
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(line.clone()));
                        this.copied = Some(std::time::Instant::now());
                        cx.notify();
                    }))
            })
            .collect::<Vec<_>>();

        let copy_entries: Vec<Entry> = entries.iter().take(SHOWN).cloned().collect();
        let problem_entries: Vec<Entry> = devlog::snapshot().into_iter().rev().filter(|e| e.level <= Level::Warn).take(150).collect();

        col()
            .h(px(self.height))
            .w_full()
            .bg(c(PANEL))
            .border_t_2()
            .border_color(c(CYAN))
            .child(
                row()
                    .flex_wrap()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(c(BORDER))
                    .child(div().text_size(px(14.)).text_color(c(TEXT)).font_weight(FontWeight::SEMIBOLD).child("Developer Logs"))
                    .child(div().text_size(px(12.)).text_color(c(RED)).child(format!("{errors} error{}", if errors == 1 { "" } else { "s" })))
                    .child(div().text_size(px(12.)).text_color(c(AMBER)).child(format!("{warnings} warning{}", if warnings == 1 { "" } else { "s" })))
                    .child(row().gap(px(6.)).children(filters))
                    .child(chip("devlog-apponly", "Hide map engine", self.app_only).on_click(cx.listener(|this, _, _, cx| {
                        this.app_only = !this.app_only;
                        cx.notify();
                    })))
                    .child(div().w(px(320.)).child(Input::new(&self.search)))
                    .child(div().flex_1())
                    .when(copied, |d| d.child(div().text_size(px(12.)).text_color(c(0x3DD68C)).child("Copied")))
                    .child(button_small("devlog-copy", "Copy shown").on_click(cx.listener(move |this, _, _, cx| this.copy(&copy_entries, cx))))
                    .child(button_small("devlog-copy-problems", "Copy problems").on_click(cx.listener(move |this, _, _, cx| this.copy(&problem_entries, cx))))
                    .child(button_small("devlog-folder", "Open log folder").on_click(|_, _, _| {
                        crate::library::reveal(&devlog::path());
                    }))
                    .child(button_small("devlog-clear", "Clear").on_click(cx.listener(|_, _, _, cx| {
                        devlog::clear();
                        cx.notify();
                    })))
                    .child(button_small("devlog-size", if self.height > 400.0 { "Smaller" } else { "Taller" }).on_click(cx.listener(|this, _, _, cx| {
                        this.height = if this.height > 400.0 { 320.0 } else { 620.0 };
                        cx.notify();
                    }))),
            )
            .child(
                div()
                    .id("devlog-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .when(total == 0, |d| {
                        d.child(div().p(px(16.)).child(muted("Nothing here. Errors, warnings and failed requests show up as they happen.")))
                    })
                    .children(rows)
                    .when(total > SHOWN, |d| {
                        d.child(div().p(px(10.)).child(muted(format!("{} older entries not shown; narrow the filter or search.", total - SHOWN))))
                    }),
            )
            .child(
                row()
                    .px(px(12.))
                    .py(px(4.))
                    .border_t_1()
                    .border_color(c(BORDER))
                    .child(muted(format!(
                        "Click a line to copy it. Errors and warnings are also saved to {}",
                        devlog::path().display()
                    ))),
            )
    }
}
