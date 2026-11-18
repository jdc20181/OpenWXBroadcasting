//! Map Studio chrome: the browser-style map tabs (On air plus work tabs). The pre-revamp
//! universal toolbar, context bar and their dropdowns were removed; the menu bar and ribbon
//! (map_studio_ribbon.rs) replace them.

use super::*;
use crate::ui::icons::ri;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Dd {
    Title,
    Range,
}

/// A navigation button: icon plus label, highlighted when `active`.
pub(super) fn nav_btn(id: impl Into<ElementId>, icon: &'static str, text: impl Into<SharedString>, active: bool) -> Stateful<Div> {
    let color = if active { CYAN } else { TEXT };
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(28.))
        .px(px(9.))
        .flex_shrink_0()
        .whitespace_nowrap()
        .rounded(px(6.))
        .cursor_pointer()
        .text_size(px(12.))
        .border_1()
        .when(active, |d| d.bg(ca(CYAN, 0.14)).border_color(ca(CYAN, 0.6)))
        .when(!active, |d| d.border_color(c(BORDER)).hover(|s| s.bg(c(PANEL_2))))
        .text_color(c(color))
        .child(ri(icon, 15., color))
        .child(text.into())
}

impl MapStudio {
    pub(super) fn tab_strip(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let active = state.active_tab;
        let on_air_title = state.map_preview.as_ref().map_or_else(|| state.live.banner_text().into_owned(), |p| p.program.banner_text().into_owned());
        let map_on_air = matches!(state.on_air, crate::app::OnAir::LiveMap) && !state.black;
        let pending = state.preview_pending();
        let titles: Vec<String> = state.work_tabs.iter().enumerate()
            .map(|(i, tab)| if active == Some(i) { state.live.banner_text().trim().to_string() } else { tab.title() })
            .collect();
        let can_add = state.work_tabs.len() < crate::app::MAX_WORK_TABS;
        let show_running = state.show_running;

        let tab = |id: ElementId, selected: bool| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(30.))
                .px(px(10.))
                .max_w(px(240.))
                .rounded_t(px(7.))
                .cursor_pointer()
                .text_size(px(12.))
                .when(selected, |d| d.bg(c(PANEL)).border_t_1().border_l_1().border_r_1().border_color(c(BORDER)).text_color(c(TEXT)))
                .when(!selected, |d| d.text_color(c(MUTED)).hover(|s| s.bg(c(PANEL_2))))
        };

        let air = tab("tab-air".into(), active.is_none())
            .child(ri("broadcast-line", 14., if map_on_air { RED } else { MUTED }))
            .child(div().text_color(c(if map_on_air { RED } else { MUTED })).child(if map_on_air { "On air" } else { "Program map" }))
            .child(div().overflow_hidden().whitespace_nowrap().child(format!("· {}", on_air_title.trim())))
            .on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, cx| { s.switch_tab(None); cx.notify(); });
            }));

        let work = titles.into_iter().enumerate().map(|(i, title)| {
            let selected = active == Some(i);
            tab(("tab-work", i).into(), selected)
                .child(ri("map-2-line", 14., if selected { CYAN } else { MUTED }))
                .child(div().overflow_hidden().whitespace_nowrap().child(if title.is_empty() { format!("Tab {}", i + 1) } else { title }))
                .child(div().id(("tab-close", i)).rounded(px(4.)).hover(|s| s.bg(c(BORDER)))
                    .child(ri("close-line", 13., MUTED))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.app.update(cx, |s, cx| { s.close_work_tab(i); cx.notify(); });
                    })))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.app.update(cx, |s, cx| { s.switch_tab(Some(i)); cx.notify(); });
                }))
        }).collect::<Vec<_>>();

        row()
            .w_full()
            .flex_shrink_0()
            .items_end()
            .gap(px(2.))
            .px(px(8.))
            .pt(px(6.))
            .bg(c(BG))
            .border_b_1()
            .border_color(c(BORDER))
            .child(air)
            .children(work)
            .when(can_add && !show_running, |d| d.child(
                div().id("tab-new").flex().items_center().gap(px(4.)).h(px(30.)).px(px(8.)).cursor_pointer()
                    .text_size(px(12.)).text_color(c(MUTED)).hover(|s| s.text_color(c(TEXT)))
                    .child(ri("add-line", 14., MUTED)).child("New tab")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| { s.open_work_tab(); cx.notify(); });
                    })),
            ))
            .child(div().flex_1())
            .child(row().items_center().gap(px(6.)).pb(px(4.))
                .when(show_running, |d| d.child(muted("Show running: the map plays straight to air")))
                .when(active.is_none() && !show_running && map_on_air, |d| d.child(muted("Edits here go straight to air. Open a tab to prepare a map first.")))
                .when(active.is_some(), |d| d
                    .child(div().text_size(px(12.)).text_color(c(if pending { AMBER } else { GREEN }))
                        .child(if pending { "Not on air yet" } else { "Same as on air" }))
                    .when(pending, |d| d
                        .child(nav_btn("tab-revert", "arrow-go-back-line", "Reset to on-air map", false).on_click(cx.listener(|this, _, _, cx| {
                            this.app.update(cx, |s, cx| { s.revert_map_preview(); cx.notify(); });
                        })))
                        .child(button_primary("tab-take", if map_on_air { "Send to air" } else { "Send map to air" }).on_click(cx.listener(|this, _, _, cx| {
                            this.app.update(cx, |s, cx| { s.send_tab_to_air(); cx.notify(); });
                        }))))))
    }

    // ---- Universal toolbar -------------------------------------------------------------------

}
