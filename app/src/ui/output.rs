//! Program output window: the clean 16:9 feed for screen sharing, capture or a second monitor.

use std::rc::Rc;

use gpui_kit::{prelude::FluentBuilder as _, *};

use crate::{
    app::{AppState, FrameStore},
    ui::{
        c,
        frame_view::{frame_canvas, ImageSlot},
    },
};

pub struct OutputView {
    app: Entity<AppState>,
    frames: Entity<FrameStore>,
    slot: Rc<ImageSlot>,
    focus: FocusHandle,
    show_hint: bool,
    _subscriptions: Vec<Subscription>,
}

impl OutputView {
    pub fn new(app: Entity<AppState>, frames: Entity<FrameStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let subscriptions = vec![
            cx.observe(&frames, |_, _, cx| cx.notify()),
            // Closing the output window ends the presentation: the main window must not keep
            // showing OUTPUT ON with a show still "on air".
            cx.on_release(|this, cx| {
                this.app.update(cx, |s, cx| {
                    s.output_window = None;
                    if s.show_running || s.show_pos.is_some() {
                        s.show_running = false;
                        s.show_pos = None;
                        s.toast(crate::app::ToastKind::Info, "Presentation ended");
                    }
                    cx.notify();
                });
            }),
        ];
        Self {
            app,
            frames,
            slot: ImageSlot::new(),
            focus,
            show_hint: true,
            _subscriptions: subscriptions,
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "f" | "f11" => window.toggle_fullscreen(),
            "escape" => {
                if window.is_fullscreen() {
                    window.toggle_fullscreen();
                }
            }
            "space" | "right" | "pagedown" => self.app.update(cx, |s, cx| {
                s.next_scene();
                cx.notify();
            }),
            "left" | "pageup" => self.app.update(cx, |s, cx| {
                s.prev_scene();
                cx.notify();
            }),
            "b" => self.app.update(cx, |s, cx| {
                let b = !s.black;
                s.set_black(b);
                cx.notify();
            }),
            "h" => {
                self.show_hint = !self.show_hint;
                cx.notify();
            }
            _ => {}
        }
    }
}

impl Render for OutputView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let image = self.frames.read(cx).program.clone();
        div()
            .size_full()
            .bg(c(0x000000))
            .relative()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, window, cx| {
                window.focus(&this.focus, cx);
                this.show_hint = false;
                // Touchscreen / click: hotspots on the graphic on air.
                if let Some((x, y)) = this.slot.to_frame(e.position) {
                    this.app.update(cx, |s, cx| {
                        s.output_click(x as f32, y as f32);
                        cx.notify();
                    });
                }
                cx.notify();
            }))
            .child(frame_canvas(image, self.slot.clone()).size_full())
            .when(self.show_hint, |d| {
                d.child(
                    div()
                        .absolute()
                        .bottom(px(12.))
                        .left(px(12.))
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(px(6.))
                        .bg(rgba(0x000000B0))
                        .text_size(px(12.))
                        .text_color(c(0xC8CED8))
                        .child("F fullscreen • Space/→ next scene • ← previous • B black • H hide this"),
                )
            })
    }
}
