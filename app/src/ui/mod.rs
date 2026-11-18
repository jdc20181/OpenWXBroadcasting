//! UI: broadcast-console styling and shared widgets on plain GPUI elements.

pub mod about;
pub mod bug_report;
pub mod community_board;
pub mod data_discovery;
pub mod desk;
pub mod dev_logs;
pub mod forecast_editor;
pub mod frame_view;
pub mod icons;
pub mod graphics_studio;
pub mod history;
pub mod viewers;
pub mod publishing;
pub mod eoc;
pub mod workflows;
pub mod lab;
pub mod data_files;
pub mod themes;
pub mod map_inspect;
pub mod map_studio;
pub mod media;
pub mod readiness;
pub mod output;
pub mod settings;
pub mod setup_wizard;
pub mod severe_center;
pub mod winter_studio;
pub mod shell;
pub mod show_builder;

use std::{cell::Cell, rc::Rc};

use gpui_kit::{prelude::FluentBuilder as _, *};

// ---- Palette ----------------------------------------------------------------------------------
// Calm graphite neutrals and one soft blue accent, like a desktop production tool. The brand
// cyan is only for the OpenWX wordmark; on-air graphics use the broadcast theme instead.
pub const BG: u32 = 0x121418;
pub const PANEL: u32 = 0x181B20;
pub const PANEL_2: u32 = 0x1D2026;
pub const RAISED: u32 = 0x24282F;
pub const HOVER: u32 = 0x2C3139;
pub const BORDER: u32 = 0x2B2F37;
pub const TEXT: u32 = 0xE4E6EA;
pub const MUTED: u32 = 0x9AA0AB;
pub const DIM: u32 = 0x666C77;
/// UI accent (selection, primary actions). Named for history; it is a soft blue now.
pub const CYAN: u32 = 0x5B93EE;
pub const CYAN_DIM: u32 = 0x22324D;
/// Brand cyan for the wordmark only.
pub const BRAND: u32 = 0x2ED3F3;
/// Text on an accent fill.
pub const ON_ACCENT: u32 = 0xFFFFFF;
pub const RED: u32 = 0xE0484B;
pub const AMBER: u32 = 0xE9A23B;
pub const GREEN: u32 = 0x45C483;

pub const UI_FONT: &str = "Segoe UI";
pub const DISPLAY_FONT: &str = "Bahnschrift";

pub fn c(hex: u32) -> Rgba {
    rgb(hex)
}

pub fn ca(hex: u32, alpha: f32) -> Hsla {
    let mut color: Hsla = rgb(hex).into();
    color.a = alpha;
    color
}

/// Parse "#RRGGBB" for UI swatches.
pub fn hex_color(value: &str) -> Hsla {
    let v = value.trim().trim_start_matches('#');
    u32::from_str_radix(v.get(0..6).unwrap_or("FFFFFF"), 16)
        .map(|n| rgb(n).into())
        .unwrap_or_else(|_| rgb(0xFFFFFF).into())
}

/// gpui-component widgets (text inputs, popovers, scrollbars) default to a light theme, which
/// put white text on white fields. Switch them to dark and match the console palette.
pub fn apply_component_theme(cx: &mut App) {
    use gpui_kit::component::{Theme, ThemeMode};
    Theme::change(ThemeMode::Dark, None, cx);
    let h = |hex: u32| -> Hsla { rgb(hex).into() };
    let theme = Theme::global_mut(cx);
    theme.background = h(0x0E131E);
    theme.foreground = h(TEXT);
    theme.border = h(BORDER);
    theme.input = h(0x34405A);
    theme.ring = h(CYAN);
    theme.caret = h(CYAN);
    theme.primary = h(CYAN);
    theme.primary_foreground = h(ON_ACCENT);
    theme.selection = ca(CYAN, 0.35);
    theme.muted = h(PANEL_2);
    theme.muted_foreground = h(MUTED);
    theme.popover = h(PANEL);
    theme.popover_foreground = h(TEXT);
    theme.accent = h(HOVER);
    theme.accent_foreground = h(TEXT);




    theme.title_bar = h(0x0F1114);
    theme.title_bar_border = h(BORDER);
    theme.font_family = UI_FONT.into();
    theme.font_size = px(13.);
}

// ---- Text ------------------------------------------------------------------------------------

/// Section heading: sentence case, quiet.
pub fn heading(text: impl Into<SharedString>) -> Div {
    div()
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(12.))
        .text_color(c(MUTED))
        .child(text.into())
}

pub fn label(text: impl Into<SharedString>) -> Div {
    div().text_size(px(13.)).text_color(c(TEXT)).child(text.into())
}

/// Run a native file dialog (rfd) on a background thread, then continue on the UI entity.
/// A dialog opened straight from a click handler runs a nested Windows message loop while
/// the app is mid-update, and GPUI then panics with "RefCell already borrowed" (the crash
/// when importing an image into a graphic, build 9). Every file dialog goes through here.
pub fn pick_off_thread<T: 'static, R: Send + 'static>(
    cx: &mut Context<T>,
    dialog: impl FnOnce() -> Option<R> + Send + 'static,
    then: impl FnOnce(&mut T, R, &mut Context<T>) + 'static,
) {
    let task = cx.background_executor().spawn(async move { dialog() });
    cx.spawn(async move |this, cx| {
        if let Some(result) = task.await {
            let _ = this.update(cx, |t, cx| then(t, result, cx));
        }
    })
    .detach();
}

/// Same as [`pick_off_thread`], for continuations that need the window.
pub fn pick_off_thread_in<T: 'static, R: Send + 'static>(
    window: &mut Window,
    cx: &mut Context<T>,
    dialog: impl FnOnce() -> Option<R> + Send + 'static,
    then: impl FnOnce(&mut T, R, &mut Window, &mut Context<T>) + 'static,
) {
    let task = cx.background_executor().spawn(async move { dialog() });
    cx.spawn_in(window, async move |this, cx| {
        if let Some(result) = task.await {
            let _ = this.update_in(cx, |t, window, cx| then(t, result, window, cx));
        }
    })
    .detach();
}

pub fn muted(text: impl Into<SharedString>) -> Div {
    div().text_size(px(12.)).text_color(c(MUTED)).child(text.into())
}

// ---- Containers ------------------------------------------------------------------------------

pub fn row() -> Div {
    div().flex().flex_row().items_center()
}

pub fn col() -> Div {
    div().flex().flex_col()
}

pub fn panel() -> Div {
    div().bg(c(PANEL)).border_color(c(BORDER))
}

/// The size an image of `img_w`×`img_h` takes inside a `box_w`×`box_h` box, letterboxed, never larger than the box.
pub fn fit_size(img_w: f32, img_h: f32, box_w: f32, box_h: f32) -> (f32, f32) {
    if img_w <= 0. || img_h <= 0. {
        return (box_w, box_h);
    }
    let scale = (box_w / img_w).min(box_h / img_h);
    ((img_w * scale).min(box_w), (img_h * scale).min(box_h))
}

/// A fixed-size thumbnail box: the image is sized to fit (portrait, square or wide) and centred,
/// and the box clips, so a thumbnail can never spill over the text under it. Use this for every
/// card preview instead of a bare `img(..).w(..).h(..)`.
pub fn thumb_box(image: std::sync::Arc<RenderImage>, box_w: f32, box_h: f32, bg: u32) -> Div {
    let size = image.size(0);
    let (w, h) = fit_size(size.width.0 as f32, size.height.0 as f32, box_w, box_h);
    thumb_frame(box_w, box_h, bg).child(img(image).w(px(w)).h(px(h)).flex_shrink_0().object_fit(ObjectFit::Contain))
}

/// The same box for a file-backed image whose size isn't known up front (clipped, contained).
pub fn thumb_box_path(path: impl Into<std::path::PathBuf>, box_w: f32, box_h: f32, bg: u32, cover: bool) -> Div {
    thumb_frame(box_w, box_h, bg).child(
        img(path.into()).w(px(box_w)).h(px(box_h)).max_w(px(box_w)).max_h(px(box_h)).flex_shrink_0()
            .object_fit(if cover { ObjectFit::Cover } else { ObjectFit::Contain }),
    )
}

fn thumb_frame(box_w: f32, box_h: f32, bg: u32) -> Div {
    div()
        .w(px(box_w))
        .h(px(box_h))
        .min_h(px(box_h))
        .max_h(px(box_h))
        .flex_shrink_0()
        .overflow_hidden()
        .flex()
        .items_center()
        .justify_center()
        .bg(c(bg))
}

/// A titled group inside a side panel.
pub fn section(title: impl Into<SharedString>) -> Div {
    col()
        .gap(px(8.))
        .px(px(14.))
        .py(px(12.))
        .border_b_1()
        .border_color(c(BORDER))
        .child(heading(title))
}

pub fn divider() -> Div {
    div().h(px(1.)).w_full().bg(c(BORDER))
}

// ---- Buttons / chips / toggles ---------------------------------------------------------------

/// Base button. GPUI allows one hover style per element, so each variant sets it exactly once.
fn button_base(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    bg: u32,
    border: u32,
    fg: u32,
    hover_bg: u32,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .h(px(30.))
        .px(px(12.))
        // Labels stay on one line: a fixed-height button whose text wraps spills over its
        // neighbours. Rows that hold many buttons wrap the buttons instead.
        .flex_shrink_0()
        .whitespace_nowrap()
        .rounded(px(6.))
        .bg(c(bg))
        .border_1()
        .border_color(c(border))
        .text_size(px(13.))
        .text_color(c(fg))
        .cursor_pointer()
        .hover(move |s| s.bg(c(hover_bg)))
        .child(text.into())
}

pub fn button(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    button_base(id, text, RAISED, BORDER, TEXT, HOVER)
}

pub fn button_primary(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    button_base(id, text, CYAN, CYAN, ON_ACCENT, 0x74A5F2).font_weight(FontWeight::SEMIBOLD)
}

pub fn button_danger(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    button_base(id, text, RED, RED, 0xFFFFFF, 0xF05A5D).font_weight(FontWeight::SEMIBOLD)
}

pub fn button_small(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    button(id, text).h(px(24.)).px(px(8.)).text_size(px(12.))
}

/// Button look for [`button_icon`].
#[derive(Clone, Copy, PartialEq)]
pub enum Btn {
    Normal,
    Primary,
    Danger,
    Small,
}

/// A button with a Remix icon before its label (or the icon alone when `text` is empty).
/// Owner rule: icons are always Remix, never unicode symbols (▲ ▶ ✕ ...), which the UI font
/// draws unevenly.
pub fn button_icon(id: impl Into<ElementId>, icon: &'static str, text: &str, kind: Btn) -> Stateful<Div> {
    let (fg, size) = match kind {
        Btn::Primary => (ON_ACCENT, 15.),
        Btn::Danger => (0xFFFFFF, 15.),
        Btn::Small => (TEXT, 13.),
        Btn::Normal => (TEXT, 15.),
    };
    let base = match kind {
        Btn::Primary => button_primary(id, ""),
        Btn::Danger => button_danger(id, ""),
        Btn::Small => button_small(id, ""),
        Btn::Normal => button(id, ""),
    };
    // The base's empty label is zero-width; the icon and label follow it.
    base.child(crate::ui::icons::ri(icon, size, fg)).when(!text.is_empty(), |b| b.child(text.to_string()))
}

/// Selectable pill (segmented controls, filters).
pub fn chip(id: impl Into<ElementId>, text: impl Into<SharedString>, active: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(26.))
        .px(px(10.))
        .flex_shrink_0()
        .whitespace_nowrap()
        .rounded(px(5.))
        .text_size(px(12.))
        .cursor_pointer()
        .border_1()
        .when(active, |d| {
            d.bg(c(CYAN_DIM))
                .border_color(ca(CYAN, 0.7))
                .text_color(c(0xFFFFFF))
                .font_weight(FontWeight::SEMIBOLD)
        })
        .when(!active, |d| {
            d.bg(c(PANEL_2))
                .border_color(c(BORDER))
                .text_color(c(MUTED))
                .hover(|s| s.bg(c(HOVER)).text_color(c(TEXT)))
        })
        .child(text.into())
}

/// Labeled on/off switch.
pub fn toggle(id: impl Into<ElementId>, text: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(10.))
        .w_full()
        // Long labels wrap onto a second line and the row grows, instead of overlapping.
        .min_h(px(26.))
        .py(px(2.))
        .cursor_pointer()
        .child(div().flex_1().min_w_0().text_size(px(13.)).line_height(px(17.)).text_color(c(TEXT)).child(text.into()))
        .child(
            div()
                .flex_shrink_0()
                .w(px(34.))
                .h(px(18.))
                .rounded(px(9.))
                .bg(if on { c(CYAN) } else { c(RAISED) })
                .border_1()
                .border_color(if on { c(CYAN) } else { c(BORDER) })
                .relative()
                .child(
                    div()
                        .absolute()
                        .top(px(2.))
                        .left(if on { px(18.) } else { px(2.) })
                        .w(px(12.))
                        .h(px(12.))
                        .rounded(px(6.))
                        .bg(if on { c(ON_ACCENT) } else { c(MUTED) }),
                ),
        )
}

/// Color swatch button.
pub fn swatch(id: impl Into<ElementId>, color: &str, active: bool) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(22.))
        .h(px(22.))
        .rounded(px(4.))
        .cursor_pointer()
        .bg(hex_color(color))
        .border_2()
        .border_color(if active { c(0xFFFFFF) } else { c(BORDER) })
}

/// Small status badge.
pub fn badge(text: impl Into<SharedString>, color: u32) -> Div {
    div()
        .flex_shrink_0()
        .whitespace_nowrap()
        .px(px(6.))
        .py(px(1.))
        .rounded(px(4.))
        .bg(ca(color, 0.18))
        .border_1()
        .border_color(ca(color, 0.6))
        .text_size(px(10.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(color))
        .child(text.into())
}

/// Horizontal slider. `on_change` receives the new value while clicking/dragging.
pub fn slider<V: 'static>(
    id: impl Into<ElementId>,
    value: f32,
    min: f32,
    max: f32,
    cx: &mut Context<V>,
    on_change: impl Fn(&mut V, f32, &mut Window, &mut Context<V>) + 'static,
) -> Stateful<Div> {
    let bounds: Rc<Cell<Bounds<Pixels>>> = Rc::new(Cell::new(Bounds::default()));
    let frac = if max > min { ((value - min) / (max - min)).clamp(0.0, 1.0) } else { 0.0 };
    let on_change = Rc::new(on_change);

    let capture = bounds.clone();
    let down_bounds = bounds.clone();
    let move_bounds = bounds.clone();
    let down_change = on_change.clone();
    let move_change = on_change.clone();
    let value_at = move |b: Bounds<Pixels>, x: Pixels| -> f32 {
        let width = f32::from(b.size.width).max(1.0);
        let f = (f32::from(x - b.origin.x) / width).clamp(0.0, 1.0);
        min + f * (max - min)
    };

    div()
        .id(id)
        .relative()
        .w_full()
        .h(px(20.))
        .cursor_pointer()
        .child(
            canvas(move |b, _, _| capture.set(b), |_, _, _, _| {})
                .absolute()
                .size_full(),
        )
        .child(
            div()
                .absolute()
                .top(px(8.))
                .left_0()
                .w_full()
                .h(px(4.))
                .rounded(px(2.))
                .bg(c(RAISED)),
        )
        .child(
            div()
                .absolute()
                .top(px(8.))
                .left_0()
                .w(relative(frac))
                .h(px(4.))
                .rounded(px(2.))
                .bg(c(CYAN)),
        )
        .child(
            div()
                .absolute()
                .top(px(3.))
                .left(relative(frac))
                .ml(px(-7.))
                .w(px(14.))
                .h(px(14.))
                .rounded(px(7.))
                .bg(c(0xFFFFFF))
                .border_2()
                .border_color(c(CYAN)),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                let v = value_at(down_bounds.get(), e.position.x);
                down_change(this, v, window, cx);
            }),
        )
        .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, window, cx| {
            if e.pressed_button == Some(MouseButton::Left) {
                let v = value_at(move_bounds.get(), e.position.x);
                move_change(this, v, window, cx);
            }
        }))
}

/// "Label ........ value" row used in inspectors.
pub fn kv(label_text: impl Into<SharedString>, value: impl Into<SharedString>) -> Div {
    row()
        .justify_between()
        .w_full()
        .child(muted(label_text))
        .child(div().text_size(px(13.)).text_color(c(TEXT)).child(value.into()))
}

/// The OPEN|WX wordmark with a subtitle, following the OpenWX logo.
pub fn wordmark(size: f32, subtitle: Option<&str>) -> Div {
    col()
        .gap(px(0.))
        .child(
            row()
                .font_family(DISPLAY_FONT)
                .font_weight(FontWeight::BLACK)
                .text_size(px(size))
                .line_height(px(size * 1.05))
                .child(div().text_color(c(0xFEFEFE)).child("OPEN"))
                .child(div().text_color(c(0xEE7D14)).child("WX")),
        )
        .when_some(subtitle.map(|s| s.to_string()), |d, sub| {
            d.child(
                div()
                    .font_family(DISPLAY_FONT)
                    .font_weight(FontWeight::LIGHT)
                    .text_size(px(size * 0.36))
                    .text_color(c(0x9AA0AA))
                    .child(sub),
            )
        })
}
