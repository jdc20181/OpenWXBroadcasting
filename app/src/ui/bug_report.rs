//! Report a bug (modal): a description and up to three screenshots, saved locally in the app
//! data folder for the developer to read (`crate::bugreport`). The first screenshot is the app
//! window, taken just before the form opens.

use std::path::PathBuf;

use gpui_kit::{
    component::input::{Textarea, TextareaState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::{AppState, Modal, ToastKind},
    bugreport,
    ui::{button_primary, button_small, c, col, muted, row, BORDER, PANEL_2, RED, TEXT},
};

pub struct BugReportView {
    app: Entity<AppState>,
    text: Entity<TextareaState>,
    shots: Vec<PathBuf>,
    error: Option<String>,
    /// Clear the description on the next render (needs the window).
    reset_text: bool,
}

impl BugReportView {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text = cx.new(|cx| TextareaState::new(window, cx).auto_grow(5, 14).placeholder("What went wrong, what you expected, and where (tab, map, graphic)."));
        Self { app, text, shots: Vec::new(), error: None, reset_text: false }
    }

    /// Start a new report with the screenshot just taken (if the capture worked). Old pending
    /// screenshots are cleared by the shell before it captures, never here (that deleted the
    /// new screenshot).
    pub fn begin(&mut self, shot: Result<PathBuf, String>, cx: &mut Context<Self>) {
        self.shots.clear();
        self.reset_text = true;
        self.error = None;
        self.add(shot, cx);
    }

    /// Add a screenshot (or note why it failed).
    pub fn add(&mut self, shot: Result<PathBuf, String>, cx: &mut Context<Self>) {
        match shot {
            Ok(path) if self.shots.len() < bugreport::MAX_SHOTS => self.shots.push(path),
            Ok(_) => self.error = Some(format!("A report holds up to {} screenshots.", bugreport::MAX_SHOTS)),
            Err(e) => self.error = Some(format!("Screenshot failed: {e}")),
        }
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let description = self.text.read(cx).value().to_string();
        let workspace = self.app.read(cx).workspace.label().to_string();
        match bugreport::save(&description, &self.shots, &workspace) {
            Ok((id, _)) => {
                bugreport::clear_pending();
                self.shots.clear();
                self.reset_text = true;
                self.error = None;
                self.app.update(cx, |s, cx| {
                    s.modal = Modal::None;
                    s.toast(ToastKind::Success, format!("Bug report {id} saved. Claude can read it when you ask."));
                    cx.notify();
                });
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        cx.notify();
    }
}

impl Render for BugReportView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.reset_text {
            self.reset_text = false;
            self.text.update(cx, |t, cx| t.set_value("", window, cx));
        }
        let full = self.shots.len() >= bugreport::MAX_SHOTS;
        let thumbs = self.shots.iter().enumerate().map(|(i, path)| {
            col()
                .gap(px(4.))
                .w(px(220.))
                .child(
                    div()
                        .w(px(220.))
                        .h(px(124.))
                        .bg(c(PANEL_2))
                        .border_1()
                        .border_color(c(BORDER))
                        .rounded(px(4.))
                        .child(crate::ui::thumb_box_path(path.clone(), 218., 122., 0x05070B, false)),
                )
                .child(
                    row()
                        .gap(px(6.))
                        .child(muted(format!("Screenshot {}", i + 1)))
                        .child(div().flex_1())
                        .child(button_small(("bug-remove", i), "Remove").on_click(cx.listener(move |this, _, _, cx| {
                            if i < this.shots.len() {
                                let _ = std::fs::remove_file(this.shots.remove(i));
                            }
                            cx.notify();
                        }))),
                )
                .into_any_element()
        });
        col()
            .gap(px(12.))
            .child(muted("Describe the problem. The report is saved on this PC with its screenshots, so it can be checked later without interrupting work."))
            .child(div().text_color(c(TEXT)).child(Textarea::new(&self.text)))
            .child(row().flex_wrap().gap(px(10.)).children(thumbs))
            .child(
                row()
                    .flex_wrap()
                    .gap(px(6.))
                    .when(!full, |r| {
                        r.child(button_small("bug-capture", "Capture the app again").on_click(cx.listener(|this, _, _, cx| {
                            // Close the form so it is not in the picture; the shell captures and reopens it.
                            this.app.update(cx, |s, cx| {
                                s.modal = Modal::None;
                                s.bug_capture = Some(false);
                                cx.notify();
                            });
                        })))
                        .child(button_small("bug-file", "Add image file…").on_click(cx.listener(|_, _, _, cx| {
                            crate::ui::pick_off_thread(
                                cx,
                                || rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "bmp", "webp"]).pick_file(),
                                |this: &mut Self, path, cx| this.add(Ok(path), cx),
                            );
                        })))
                    })
                    .child(muted(format!("{} of {} screenshots", self.shots.len(), bugreport::MAX_SHOTS)))
                    .child(div().flex_1())
                    .child(button_small("bug-folder", "Open reports folder").on_click(|_, _, _| {
                        let _ = open::that(bugreport::folder());
                    }))
                    .child(button_primary("bug-save", "Save report").on_click(cx.listener(|this, _, _, cx| this.save(cx)))),
            )
            .when_some(self.error.clone(), |d, e| d.child(div().text_size(px(13.)).text_color(c(RED)).child(e)))
    }
}
