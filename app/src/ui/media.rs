//! Media: import images (logos, photos, backgrounds) and browse exports/recordings.

use gpui_kit::{
    component::input::{Input, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::{AppState, ToastKind},
    config::LibraryPaths,
    library::{self, human_size, MediaKind, IMAGE_EXTS},
    ui::{badge, button_primary, button_small, c, chip, col, heading, label, muted, row, BG, BORDER, CYAN, MUTED, PANEL, PANEL_2, TEXT},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Library,
    Exports,
}

pub struct MediaView {
    app: Entity<AppState>,
    tab: Tab,
    /// Media file whose license notes are open.
    selected: Option<String>,
    /// Source, license, credit, license end date.
    meta_inputs: Vec<Entity<InputState>>,
    _subscriptions: Vec<Subscription>,
}

impl MediaView {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.observe(&app, |_, _, cx| cx.notify())];
        let meta_inputs = ["Source (where it came from)", "License (e.g. Licensed, CC BY 4.0, Station-owned)", "Credit line to show", "License ends (YYYY-MM-DD, blank = never)"]
            .into_iter()
            .map(|p| cx.new(|cx| InputState::new(window, cx).placeholder(p)))
            .collect();
        Self {
            app,
            tab: Tab::Library,
            selected: None,
            meta_inputs,
            _subscriptions: subscriptions,
        }
    }

    fn import(&mut self, cx: &mut Context<Self>) {
        let pick = cx.background_executor().spawn(async move {
            rfd::FileDialog::new()
                .set_title("Import images into OpenWX Broadcasting")
                .add_filter("Images", library::IMPORT_EXTS)
                .pick_files()
        });
        cx.spawn(async move |this, cx| {
            let files = pick.await;
            let Some(files) = files else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.app.update(cx, |s, cx| {
                    match library::import_media(&files) {
                        Ok(names) => s.toast(ToastKind::Success, format!("Imported {} file(s)", names.len())),
                        Err(e) => s.toast(ToastKind::Error, format!("Import failed: {e}")),
                    }
                    s.media = library::list_media();
                    cx.notify();
                });
            });
        })
        .detach();
    }
}

impl Render for MediaView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let media = state.media.clone();
        let exports = state.exports.clone();
        let logo = state.settings.brand.logo_media.clone();
        let sponsor = state.settings.sponsor_logo_media.clone();
        let paths = LibraryPaths::get();
        let all_meta = crate::collections::load_meta();
        let graphics = state.graphics.clone();
        let selected = self.selected.clone();

        let media_cards = media
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let name = item.name.clone();
                let is_logo = logo.as_deref() == Some(name.as_str());
                let logo_name = name.clone();
                let is_sponsor = sponsor.as_deref() == Some(name.as_str());
                let sponsor_name = name.clone();
                let delete_name = name.clone();
                let select_name = name.clone();
                let used = crate::collections::used_by(&name, &graphics).len();
                let meta = all_meta.get(&name).cloned();
                let expired = meta.as_ref().map(crate::collections::expired).unwrap_or(false);
                col()
                    .id(("media", i))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let meta = crate::collections::load_meta().get(&select_name).cloned().unwrap_or_default();
                        let values = [meta.source, meta.license, meta.credit, meta.expires];
                        for (input, v) in this.meta_inputs.clone().iter().zip(values) {
                            input.update(cx, |s, cx| s.set_value(v, window, cx));
                        }
                        this.selected = Some(select_name.clone());
                        cx.notify();
                    }))
                    .w(px(220.))
                    .rounded(px(8.))
                    .overflow_hidden()
                    .bg(c(PANEL_2))
                    .border_1()
                    .border_color(if is_logo { c(CYAN) } else { c(BORDER) })
                    .child(
                        div()
                            .w_full()
                            .h(px(124.))
                            .flex_shrink_0()
                            .overflow_hidden()
                            .bg(c(0x0A0D14))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(item.kind == MediaKind::Image, |d| {
                                d.child(img(item.path.as_path()).size_full().object_fit(ObjectFit::Contain))
                            })
                            .when(item.kind != MediaKind::Image, |d| d.child(muted(item.name.clone()))),
                    )
                    .child(
                        col()
                            .p(px(8.))
                            .gap(px(6.))
                            .child(div().text_size(px(12.)).text_color(c(TEXT)).truncate().child(item.name.clone()))
                            .child(
                                row()
                                    .flex_wrap()
                                    .gap(px(6.))
                                    .child(muted(human_size(item.bytes)))
                                    .child(muted(if used == 0 { "unused".to_string() } else { format!("in {used} graphic(s)") }))
                                    .when(expired, |d| d.child(badge("LICENSE ENDED", 0xE5383B)))
                                    .when(meta.as_ref().map(|m| !m.license.is_empty()).unwrap_or(false) && !expired, |d| d.child(badge("LICENSED", 0x3DD68C)))
                                    .when(is_logo, |d| d.child(badge("STATION LOGO", CYAN))),
                            )
                            .child(
                                // Wraps: "Use as sponsor logo" ran past the card edge.
                                row()
                                    .flex_wrap()
                                    .gap(px(6.))
                                    .when(item.kind == MediaKind::Image, |d| {
                                        d.child(button_small(("logo", i), if is_logo { "Clear logo" } else { "Use as logo" }).on_click(cx.listener(
                                            move |this, _, _, cx| {
                                                let name = logo_name.clone();
                                                this.app.update(cx, |s, cx| {
                                                    let mut settings = s.settings.clone();
                                                    settings.brand.logo_media = if is_logo { None } else { Some(name) };
                                                    s.apply_settings(settings);
                                                    cx.notify();
                                                });
                                            },
                                        )))
                                    })
                                    .when(item.kind == MediaKind::Image, |d| {
                                        let name = sponsor_name.clone();
                                        d.child(button_small(("sponsor", i), if is_sponsor { "Clear sponsor logo" } else { "Use as sponsor logo" }).on_click(cx.listener(
                                            move |this, _, _, cx| {
                                                let name = name.clone();
                                                this.app.update(cx, |s, cx| {
                                                    let mut settings = s.settings.clone();
                                                    settings.sponsor_logo_media = if is_sponsor { None } else { Some(name) };
                                                    s.apply_settings(settings);
                                                    cx.notify();
                                                });
                                            },
                                        )))
                                    })
                                    .child(button_small(("del", i), "Delete").on_click(cx.listener(move |this, _, _, cx| {
                                        let name = delete_name.clone();
                                        this.app.update(cx, |s, cx| {
                                            if let Err(e) = library::delete_media(&name) {
                                                s.toast(ToastKind::Error, format!("Could not delete: {e}"));
                                            }
                                            s.media = library::list_media();
                                            cx.notify();
                                        });
                                    }))),
                            ),
                    )
            })
            .collect::<Vec<_>>();

        let export_rows = exports
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let path = item.path.clone();
                let reveal = item.path.clone();
                let name = item.name.clone();
                let ext = item
                    .path
                    .extension()
                    .map(|e| e.to_string_lossy().to_uppercase())
                    .unwrap_or_default();
                row()
                    .id(("export", i))
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .bg(c(PANEL_2))
                    .child(badge(ext, if item.kind == MediaKind::Video { 0xE5383B } else { CYAN }))
                    .child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).truncate().child(item.name.clone()))
                    .child(muted(human_size(item.bytes)))
                    .child(button_small(("open", i), "Open").on_click(move |_, _, _| {
                        let _ = open::that(&path);
                    }))
                    .child(button_small(("reveal", i), "Show in folder").on_click(move |_, _, _| {
                        library::reveal(&reveal);
                    }))
                    .child(button_small(("trash", i), "Delete").on_click(cx.listener(move |this, _, _, cx| {
                        let name = name.clone();
                        this.app.update(cx, |s, cx| {
                            if let Err(e) = library::delete_export(&name) {
                                s.toast(ToastKind::Error, format!("Could not delete: {e}"));
                            }
                            s.exports = library::list_exports();
                            cx.notify();
                        });
                    })))
            })
            .collect::<Vec<_>>();

        let media_dir = paths.media.clone();
        let exports_dir = paths.exports.clone();

        col()
            .id("media-root")
            .size_full()
            .bg(c(BG))
            .overflow_y_scroll()
            .child(
                row()
                    .px(px(20.))
                    .py(px(14.))
                    .gap(px(10.))
                    .bg(c(PANEL))
                    .border_b_1()
                    .border_color(c(BORDER))
                    .child(chip("tab-lib", format!("Library ({})", media.len()), self.tab == Tab::Library).on_click(cx.listener(|this, _, _, cx| {
                        this.tab = Tab::Library;
                        cx.notify();
                    })))
                    .child(chip("tab-exp", format!("Exports & recordings ({})", exports.len()), self.tab == Tab::Exports).on_click(cx.listener(|this, _, _, cx| {
                        this.tab = Tab::Exports;
                        cx.notify();
                    })))
                    .child(div().flex_1())
                    .child(button_small("refresh-media", "Refresh").on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.reload_library();
                            cx.notify();
                        });
                    })))
                    .child(button_small("open-folder", "Open folder").on_click(move |_, _, _| {
                        let _ = open::that(&media_dir);
                    }))
                    .child(button_primary("import", "Import images…").on_click(cx.listener(|this, _, _, cx| this.import(cx)))),
            )
            .when(self.tab == Tab::Library, |d| {
                d.child(
                    col()
                        .p(px(20.))
                        .gap(px(12.))
                        .child(heading("Images for graphics, logos and backgrounds"))
                        .when_some(selected.clone(), |d, name| {
                            let users = crate::collections::used_by(&name, &graphics);
                            d.child(
                                col()
                                    .gap(px(6.))
                                    .p(px(12.))
                                    .rounded(px(8.))
                                    .bg(c(PANEL_2))
                                    .border_1()
                                    .border_color(c(BORDER))
                                    .child(label(format!("License notes: {name}")))
                                    .child(muted(if users.is_empty() { "Not used by any graphic".to_string() } else { format!("Used by: {}", users.join(", ")) }))
                                    .children(self.meta_inputs.iter().map(|i| Input::new(i)).collect::<Vec<_>>())
                                    .child(
                                        row()
                                            .gap(px(6.))
                                            .child(button_small("meta-save", "Save license notes").on_click(cx.listener(move |this, _, _, cx| {
                                                let v: Vec<String> = this.meta_inputs.iter().map(|i| i.read(cx).value().trim().to_string()).collect();
                                                let meta = crate::collections::AssetMeta { source: v[0].clone(), license: v[1].clone(), credit: v[2].clone(), expires: v[3].clone() };
                                                let result = crate::collections::save_meta(&name, meta);
                                                this.app.update(cx, |s, cx| {
                                                    match result {
                                                        Ok(()) => s.toast(ToastKind::Success, "License notes saved"),
                                                        Err(e) => s.toast(ToastKind::Error, format!("Could not save: {e}")),
                                                    }
                                                    cx.notify();
                                                });
                                            })))
                                            .child(button_small("meta-close", "Close").on_click(cx.listener(|this, _, _, cx| {
                                                this.selected = None;
                                                cx.notify();
                                            }))),
                                    ),
                            )
                        })
                        .when(media.is_empty(), |d| {
                            d.child(
                                col()
                                    .py(px(40.))
                                    .items_center()
                                    .gap(px(6.))
                                    .child(label("No media yet"))
                                    .child(muted("Import PNG, JPG or WebP files. Use one as the station logo bug, or place them in graphics with an Image layer.")),
                            )
                        })
                        .child(row().flex_wrap().gap(px(14.)).children(media_cards)),
                )
            })
            .when(self.tab == Tab::Exports, |d| {
                d.child(
                    col()
                        .p(px(20.))
                        .gap(px(8.))
                        .child(
                            row()
                                .child(heading("PNG stills, MP4 and GIF recordings"))
                                .child(div().flex_1())
                                .child(button_small("open-exports", "Open folder").on_click(move |_, _, _| {
                                    let _ = open::that(&exports_dir);
                                })),
                        )
                        .when(exports.is_empty(), |d| d.child(muted("Nothing exported yet. Export a graphic as PNG or press REC to record Program output.")))
                        .children(export_rows),
                )
            })
            .child(div().text_color(c(MUTED)))
    }
}
