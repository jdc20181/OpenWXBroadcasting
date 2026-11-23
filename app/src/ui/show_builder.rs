//! Show Builder: a rundown of map scenes and graphics, played to Program with transitions.

use std::rc::Rc;

use gpui_kit::{
    component::input::{Input, InputEvent, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::{AppState, FrameStore, OnAir, ToastKind},
    library,
    show::{Scene, SceneKind, Show, Transition},
    ui::{
        badge, button, button_danger, button_primary, button_small, c, chip, col,
        frame_view::{frame_canvas, ImageSlot},
        heading, label, muted, row, section, slider, toggle, AMBER, BG, BORDER, CYAN, GREEN, MUTED,
        PANEL, PANEL_2, RAISED, RED, TEXT,
    },
};

// CC0 provenance and original downloads are listed in docs/SHOW-AUDIO-SOURCES.md.
const WEATHER_MUSIC: [(&str, &str, &str, &[u8]); 5] = [
    ("Mellow Cafe Vibe", "Guitar, upright bass and brushes", "Mellow Cafe Vibe - John Bartmann.mp3",
        include_bytes!("../../assets/music/Mellow Cafe Vibe - John Bartmann.mp3")),
    ("Busted Guitar 2", "Relaxed electric jazz guitar", "Busted Guitar 2 - HoliznaCC0.mp3",
        include_bytes!("../../assets/music/Busted Guitar 2 - HoliznaCC0.mp3")),
    ("Bossa Nova", "Light retro bossa", "Bossa Nova - Joth.mp3",
        include_bytes!("../../assets/music/Bossa Nova - Joth.mp3")),
    ("Moil", "Slow Rhodes and baritone sax", "Moil - Ruskerdax.mp3",
        include_bytes!("../../assets/music/Moil - Ruskerdax.mp3")),
    ("Catchy Swing", "Brighter sax, bass and drums", "Catchy Swing - Doge.mp3",
        include_bytes!("../../assets/music/Catchy Swing - Doge.mp3")),
];

/// A rundown row being dragged to a new position.
#[derive(Clone)]
struct DraggedScene {
    index: usize,
    title: String,
}

impl Render for DraggedScene {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(12.))
            .py(px(8.))
            .rounded(px(6.))
            .bg(c(RAISED))
            .border_1()
            .border_color(c(CYAN))
            .text_size(px(13.))
            .text_color(c(TEXT))
            .child(format!("{}  {}", self.index + 1, self.title))
    }
}

/// Rehearsal: scenes stepped through on the preview only, timing each against the plan.
struct Rehearsal {
    index: usize,
    started: std::time::Instant,
    /// (title, planned seconds, actual seconds)
    times: Vec<(String, f32, f32)>,
}

pub struct ShowBuilder {
    rehearsal: Option<Rehearsal>,
    rehearsal_report: Option<Vec<(String, f32, f32)>>,
    app: Entity<AppState>,
    frames: Entity<FrameStore>,
    slot: Rc<ImageSlot>,
    selected: Option<usize>,
    music_catalog_open: bool,
    /// The music picker is choosing the show soundtrack (not the selected scene's).
    bed_picker: bool,
    music_page: usize,
    music_importing: bool,
    music_search: Entity<InputState>,
    picking_graphic: bool,
    syncing: bool,
    show_name: Entity<InputState>,
    scene_title: Entity<InputState>,
    scene_notes: Entity<InputState>,
    caption: Entity<InputState>,
    sched_time: Entity<InputState>,
    sched_arg: Entity<InputState>,
    sched_action: &'static str,
    sched_daily: bool,
    /// OBS scenes used as camera buttons (loaded on demand).
    cameras: Vec<String>,
    focus: FocusHandle,
    /// Height of the big centre Program preview, from the window size (CWXB-104).
    preview_h: f32,
    _subscriptions: Vec<Subscription>,
}

impl ShowBuilder {
    pub fn new(app: Entity<AppState>, frames: Entity<FrameStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = app.read(cx).show.name.clone();
        let show_name = cx.new(|cx| InputState::new(window, cx).placeholder("Show name").default_value(name));
        let scene_title = cx.new(|cx| InputState::new(window, cx).placeholder("Scene title"));
        let scene_notes = cx.new(|cx| InputState::new(window, cx).placeholder("Talking points / notes"));
        let music_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search tracks, dates or bundles"));
        let caption = cx.new(|cx| InputState::new(window, cx).placeholder("Caption text"));
        let sched_time = cx.new(|cx| InputState::new(window, cx).placeholder("Time, e.g. 17:58"));
        let sched_arg = cx.new(|cx| InputState::new(window, cx).placeholder("Scene number / graphic name"));

        let mut subscriptions = vec![
            cx.observe(&app, |_, _, cx| cx.notify()),
            cx.observe(&frames, |_, _, cx| cx.notify()),
            cx.subscribe_in(&music_search, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.music_page = 0;
                    cx.notify();
                }
            }),
        ];
        subscriptions.push(cx.subscribe_in(&show_name, window, |this, state, e: &InputEvent, _, cx| {
            if matches!(e, InputEvent::Change) && !this.syncing {
                let v = state.read(cx).value().to_string();
                this.app.update(cx, |s, _| s.show.name = v);
            }
        }));
        subscriptions.push(cx.subscribe_in(&scene_title, window, |this, state, e: &InputEvent, _, cx| {
            if matches!(e, InputEvent::Change) && !this.syncing {
                let v = state.read(cx).value().to_string();
                this.edit_scene(cx, |s| s.title = v);
            }
        }));
        subscriptions.push(cx.subscribe_in(&scene_notes, window, |this, state, e: &InputEvent, _, cx| {
            if matches!(e, InputEvent::Change) && !this.syncing {
                let v = state.read(cx).value().to_string();
                this.edit_scene(cx, |s| s.notes = v);
            }
        }));

        Self {
            app,
            frames,
            slot: ImageSlot::new(),
            selected: None,
            music_catalog_open: false,
            bed_picker: false,
            music_page: 0,
            music_importing: false,
            music_search,
            picking_graphic: false,
            syncing: false,
            show_name,
            scene_title,
            scene_notes,
            caption,
            sched_time,
            sched_arg,
            rehearsal: None,
            rehearsal_report: None,
            sched_action: "take",
            sched_daily: true,
            cameras: Vec::new(),
            focus: cx.focus_handle(),
            preview_h: 360.0,
            _subscriptions: subscriptions,
        }
    }

    fn edit_scene(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Scene)) {
        let Some(i) = self.selected else {
            return;
        };
        self.app.update(cx, |s, cx| {
            if let Some(scene) = s.show.scenes.get_mut(i) {
                f(scene);
            }
            cx.notify();
        });
    }

    fn set_soundtrack(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        if self.bed_picker {
            self.app.update(cx, |s, cx| {
                s.show.soundtrack = name;
                cx.notify();
            });
        } else {
            self.edit_scene(cx, |scene| scene.soundtrack = name);
        }
    }

    fn choose_scene_soundtrack(&mut self, cx: &mut Context<Self>) {
        let media_dir = crate::config::LibraryPaths::get().media;
        crate::ui::pick_off_thread(cx, move || rfd::FileDialog::new()
            .set_title("Choose a soundtrack for this scene")
            .set_directory(&media_dir)
            .add_filter("Audio", crate::library::AUDIO_EXTS)
            .pick_file(), |this: &mut Self, path, cx| this.scene_soundtrack_picked(path, cx));
    }

    fn scene_soundtrack_picked(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        let media_dir = crate::config::LibraryPaths::get().media;
        if !crate::library::is_audio(&path) {
            self.app.update(cx, |s, cx| {
                s.toast(ToastKind::Error, "Choose a WAV, MP3, OGG or FLAC file");
                cx.notify();
            });
            return;
        }
        if path.parent() == Some(media_dir.as_path()) {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned());
            self.set_soundtrack(name, cx);
            return;
        }
        match crate::library::import_media(&[path]) {
            Ok(files) => {
                if let Some(name) = files.into_iter().next() {
                    self.attach_imported_soundtrack(Ok(name), cx);
                }
            }
            Err(error) => self.attach_imported_soundtrack(Err(error), cx),
        }
    }

    fn attach_imported_soundtrack(&mut self, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        match result {
            Ok(name) => {
                self.music_catalog_open = false;
                self.set_soundtrack(Some(name), cx);
                self.bed_picker = false;
                self.app.update(cx, |s, cx| {
                    s.media = library::list_media();
                    cx.notify();
                });
            }
            Err(error) => self.app.update(cx, |s, cx| {
                s.toast(ToastKind::Error, format!("Could not import soundtrack: {error}"));
                cx.notify();
            }),
        }
    }

    fn import_music_library(&mut self, cx: &mut Context<Self>) {
        if self.music_importing {
            return;
        }
        crate::ui::pick_off_thread(cx, || rfd::FileDialog::new()
            .set_title("Import music into the library")
            .add_filter("Audio", library::AUDIO_EXTS)
            .pick_files(), |this: &mut Self, paths, cx| this.music_files_picked(paths, cx));
    }

    fn music_files_picked(&mut self, paths: Vec<std::path::PathBuf>, cx: &mut Context<Self>) {
        let media_dir = crate::config::LibraryPaths::get().media;
        self.music_importing = true;
        let task = cx.background_executor().spawn(async move {
            let mut added = 0;
            let mut failed = 0;
            for path in paths {
                if !library::is_audio(&path) || path.parent() == Some(media_dir.as_path()) {
                    continue;
                }
                match library::import_media(&[path]) {
                    Ok(names) => added += names.len(),
                    Err(error) => {
                        failed += 1;
                        log::warn!("Music import: {error}");
                    }
                }
            }
            (added, failed, library::list_media())
        });
        cx.spawn(async move |this, cx| {
            let (added, failed, media) = task.await;
            let _ = this.update(cx, |this, cx| {
                this.music_importing = false;
                this.app.update(cx, |s, cx| {
                    s.media = media;
                    if failed == 0 {
                        s.toast(ToastKind::Success, format!("Imported {added} music file(s)"));
                    } else {
                        s.toast(ToastKind::Error, format!("Imported {added} music file(s). {failed} failed. See Developer Logs."));
                    }
                    cx.notify();
                });
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    fn library_music(&self, cx: &mut Context<Self>) -> Div {
        const PAGE_SIZE: usize = 20;
        let query = self.music_search.read(cx).value().trim().to_lowercase();
        let mut bundled = crate::audio_catalog::tracks().iter().filter(|track| {
            track.title.to_lowercase().contains(&query) || track.bundle().contains(&query)
        }).skip(self.music_page * PAGE_SIZE);
        let bundled_page: Vec<_> = bundled.by_ref().take(PAGE_SIZE).collect();
        let bundled_has_more = bundled.next().is_some();
        let mut results = self.app.read(cx).media.iter()
            .filter(|m| library::is_audio(std::path::Path::new(&m.name))
                && m.name.replace('_', " ").to_lowercase().contains(&query))
            .map(|m| m.name.clone());
        let visible: Vec<_> = results.by_ref().take(20).collect();
        let more = results.next().is_some();
        col().gap(px(5.))
            .child(Input::new(&self.music_search))
            .child(label("Bundled Abstraction loops · CC0"))
            .when(bundled_page.is_empty(), |d| d.child(muted("No matching bundled loops.")))
            .child(div().id("bundled-music-results").max_h(px(200.)).overflow_y_scroll().child(
                col().gap(px(3.)).children(bundled_page.into_iter().enumerate().map(|(i, track)| {
                    row().flex_wrap().gap(px(6.))
                        .child(button_small(("bundled-loop", i), track.title.clone()).on_click(cx.listener(move |this, _, _, cx| {
                            this.attach_imported_soundtrack(track.copy_to_media(), cx);
                        })))
                        .child(muted(track.bundle().to_string()))
                }).collect::<Vec<_>>()),
            ))
            .when(self.music_page > 0 || bundled_has_more, |d| d.child(row().flex_wrap().gap(px(6.))
                .when(self.music_page > 0, |r| r.child(button_small("bundled-prev", "Previous").on_click(cx.listener(|this, _, _, cx| {
                    this.music_page = this.music_page.saturating_sub(1);
                    cx.notify();
                }))))
                .child(muted(format!("Page {}", self.music_page + 1)))
                .when(bundled_has_more, |r| r.child(button_small("bundled-next", "Next").on_click(cx.listener(|this, _, _, cx| {
                    this.music_page += 1;
                    cx.notify();
                }))))))
            .child(row().flex_wrap().gap(px(6.))
                .child(label("Local music library"))
                .child(button_small("import-music-library", if self.music_importing { "Importing…" } else { "Import tracks…" })
                    .on_click(cx.listener(|this, _, _, cx| this.import_music_library(cx)))))
            .when(visible.is_empty(), |d| d.child(muted("No matching audio files in Media.")))
            .child(div().id("music-search-results").max_h(px(200.)).overflow_y_scroll().child(
                col().gap(px(3.)).children(visible.into_iter().enumerate().map(|(i, name)| {
                    button_small(("library-music", i), name.replace('_', " ")).on_click(cx.listener(move |this, _, _, cx| {
                        this.music_catalog_open = false;
                        this.set_soundtrack(Some(name.clone()), cx);
                        this.bed_picker = false;
                        cx.notify();
                    }))
                }).collect::<Vec<_>>()),
            ))
            .when(more, |d| d.child(muted("Showing the first 20 matches. Type a title or artist to narrow the list.")))
    }

    fn select(&mut self, index: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = index;
        let scene = index.and_then(|i| self.app.read(cx).show.scenes.get(i).cloned());
        self.syncing = true;
        let (title, notes) = scene
            .map(|s| (s.title, s.notes))
            .unwrap_or_default();
        self.scene_title.update(cx, |st, cx| st.set_value(title, window, cx));
        self.scene_notes.update(cx, |st, cx| st.set_value(notes, window, cx));
        self.syncing = false;
        cx.notify();
    }

    fn load_show(&mut self, show: Show, window: &mut Window, cx: &mut Context<Self>) {
        let name = show.name.clone();
        self.app.update(cx, |s, cx| {
            s.show = show;
            s.show_pos = None;
            s.show_running = false;
            let relinked = s.relink_show_graphics();
            if relinked > 0 {
                s.toast(crate::app::ToastKind::Info, format!("Relinked {relinked} scene(s) to the library graphic with the same name"));
            }
            cx.notify();
        });
        self.syncing = true;
        self.show_name.update(cx, |st, cx| st.set_value(name, window, cx));
        self.syncing = false;
        self.select(None, window, cx);
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if [&self.show_name, &self.scene_title, &self.scene_notes, &self.music_search,
            &self.caption, &self.sched_time, &self.sched_arg].into_iter()
            .any(|input| input.read(cx).focus_handle(cx).is_focused(window))
        {
            return;
        }
        match event.keystroke.key.as_str() {
            "space" | "right" | "pagedown" => self.app.update(cx, |s, cx| {
                s.next_scene();
                cx.notify();
            }),
            "left" | "pageup" => self.app.update(cx, |s, cx| {
                s.prev_scene();
                cx.notify();
            }),
            "enter" => {
                if let Some(i) = self.selected {
                    self.app.update(cx, |s, cx| {
                        s.go_to_scene(i);
                        cx.notify();
                    });
                }
            }
            "up" => {
                let i = self.selected.map(|i| i.saturating_sub(1)).unwrap_or(0);
                self.select(Some(i), window, cx);
            }
            "down" => {
                let len = self.app.read(cx).show.scenes.len();
                let i = self.selected.map(|i| (i + 1).min(len.saturating_sub(1))).unwrap_or(0);
                self.select(Some(i), window, cx);
            }
            _ => {}
        }
    }

    fn shows_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let shows = state.shows.clone();
        let current = state.show.id.clone();
        let loop_show = state.show.loop_show;
        let bed = state.show.soundtrack.clone();
        let bed_volume = state.show.soundtrack_volume;
        let total = state.show.total_seconds();
        let exporting = state.exporting_show;
        let presenting = state.show_running && state.output_window.is_some();

        let items = shows
            .into_iter()
            .enumerate()
            .map(|(i, show)| {
                let active = show.id == current;
                let count = show.scenes.len();
                row()
                    .id(("show", i))
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .bg(if active { c(0x163A47) } else { c(PANEL_2) })
                    .hover(|s| s.bg(c(RAISED)))
                    .child(
                        col()
                            .flex_1()
                            .child(div().text_size(px(13.)).text_color(c(TEXT)).child(show.name.clone()))
                            .child(div().text_size(px(11.)).text_color(c(MUTED)).child(format!("{count} scenes"))),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| this.load_show(show.clone(), window, cx)))
            })
            .collect::<Vec<_>>();

        col()
            .id("shows")
            .w(px(260.))
            .h_full()
            .flex_shrink_0()
            .bg(c(PANEL))
            .border_r_1()
            .border_color(c(BORDER))
            .overflow_y_scroll()
            .child(
                section("This show")
                    .child(Input::new(&self.show_name))
                    .child(muted(format!("Runtime {}:{:02}", (total as u32) / 60, (total as u32) % 60)))
                    .child(toggle("loop-show", "Loop when finished", loop_show).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.show.loop_show = !s.show.loop_show;
                            cx.notify();
                        });
                    })))
                    .child(muted("Show soundtrack · loops under every scene"))
                    .child(row().flex_wrap().gap(px(5.))
                        .child(chip("bed-catalog", "Weather music ▾", self.bed_picker).on_click(cx.listener(|this, _, _, cx| {
                            this.bed_picker = !this.bed_picker;
                            this.music_catalog_open = false;
                            cx.notify();
                        })))
                        .child(button_small("bed-file", "Choose file…").on_click(cx.listener(|this, _, _, cx| {
                            this.bed_picker = true;
                            this.choose_scene_soundtrack(cx);
                            this.bed_picker = false;
                            cx.notify();
                        })))
                        .when(bed.is_some(), |d| d.child(button_small("bed-clear", "Remove").on_click(cx.listener(|this, _, _, cx| {
                            this.app.update(cx, |s, cx| {
                                s.show.soundtrack = None;
                                cx.notify();
                            });
                        })))))
                    .child(muted(bed.clone().map(|name| name.replace('_', " ")).unwrap_or_else(|| "No show soundtrack".into())))
                    .when(bed.is_some(), |d| d
                        .child(row().justify_between().child(muted("Bed volume")).child(muted(format!("{:.0}%", bed_volume * 100.0))))
                        .child(slider("bed-volume", bed_volume, 0.0, 1.5, cx, |this, v, _, cx| {
                            this.app.update(cx, |s, cx| {
                                s.show.soundtrack_volume = v;
                                cx.notify();
                            });
                        })))
                    .when(self.bed_picker, |d| d.child(
                        col().gap(px(5.))
                            .child(muted("CC0 instrumental music"))
                            .children(WEATHER_MUSIC.into_iter().enumerate().map(|(i, (title, description, filename, bytes))| {
                                row().flex_wrap().gap(px(6.))
                                    .child(button_small(("bed-track", i), title).on_click(cx.listener(move |this, _, _, cx| {
                                        this.attach_imported_soundtrack(library::import_builtin_track(filename, bytes), cx);
                                    })))
                                    .child(muted(description))
                            }).collect::<Vec<_>>())
                            .child(self.library_music(cx)),
                    ))
                    .child(
                        row()
                            .gap(px(6.))
                            .child(button_primary("save-show", "Save show").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.save_show();
                                    cx.notify();
                                });
                            })))
                            .child(button_small("new-show", "+ New").on_click(cx.listener(|this, _, window, cx| {
                                let show = Show::new("New Show");
                                this.load_show(show, window, cx);
                            }))),
                    )
                    .child(
                        button("auto-show", "Build a show for me").on_click(cx.listener(|this, _, window, cx| {
                            let show = this.app.update(cx, |s, cx| {
                                let show = crate::autoshow::build(s);
                                s.toast(
                                    crate::app::ToastKind::Success,
                                    format!("Built \"{}\" with {} scenes from current conditions. Review it, then Save show.", show.name, show.scenes.len()),
                                );
                                cx.notify();
                                show
                            });
                            this.load_show(show, window, cx);
                        })),
                    )
                    .child(
                        row()
                            .gap(px(6.))
                            .flex_wrap()
                            .child(button_small("auto-fly", "Town flythrough").on_click(cx.listener(|this, _, window, cx| {
                                let show = this.app.update(cx, |s, cx| {
                                    let show = crate::autoshow::flythrough(s);
                                    s.toast(crate::app::ToastKind::Success, format!("Flythrough of {} places built", show.scenes.len()));
                                    cx.notify();
                                    show
                                });
                                this.load_show(show, window, cx);
                            })))
                            .child(button_small("auto-rotate", "Forecast map rotation").on_click(cx.listener(|this, _, window, cx| {
                                let show = this.app.update(cx, |s, cx| {
                                    let show = crate::autoshow::data_rotation(s);
                                    s.toast(crate::app::ToastKind::Success, format!("Looping rotation of {} forecast maps built", show.scenes.len()));
                                    cx.notify();
                                    show
                                });
                                this.load_show(show, window, cx);
                            }))),
                    )
                    .child(muted("Builds a full segment from what's happening now: ID, conditions, radar, any local warnings, severe outlook, Futurecast, forecast and sign-off."))
                    .child(button_small("del-show", "Delete show").on_click(cx.listener(|this, _, window, cx| {
                        let id = this.app.read(cx).show.id.clone();
                        let next = this.app.update(cx, |s, cx| {
                            if let Err(e) = library::delete_show(&id) {
                                s.toast(ToastKind::Error, format!("Could not delete: {e}"));
                            }
                            s.shows.retain(|sh| sh.id != id);
                            s.toast(ToastKind::Info, "Show moved to the library trash");
                            cx.notify();
                            s.shows.first().cloned()
                        });
                        this.load_show(next.unwrap_or_else(|| Show::new("New Show")), window, cx);
                    }))),
            )
            .child(
                section("Export & present")
                    .child(muted("Present: plays this show in the full-screen output window, for screen sharing or a second monitor."))
                    .child(muted("For a silent presentation, use Settings → Audio → Mute presentation audio."))
                    .when(presenting, |d| {
                        d.child(crate::ui::button_icon("stop-present", "stop-line", "Stop presenting", crate::ui::Btn::Danger).on_click(cx.listener(|this, _, _, cx| {
                            this.app.update(cx, |s, cx| {
                                s.show_running = false;
                                s.show_pos = None;
                                s.want_close_output = true;
                                s.take_live_map(crate::show::Transition::Fade);
                                cx.notify();
                            });
                        })))
                    })
                    .child((if presenting { crate::ui::button_icon("present-show", "restart-line", "Restart presentation", crate::ui::Btn::Primary) } else { crate::ui::button_icon("present-show", "play-line", "Present show", crate::ui::Btn::Primary) }).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.want_output = true;
                            s.switch_tab(None);
                            s.show_running = true;
                            s.show_pos = None;
                            s.next_scene();
                            s.toast(ToastKind::Info, "Presenting - press F in the output window for full screen, Space for next");
                            cx.notify();
                        });
                    })))
                    .child(muted("Video: records the show to Exports with its music as an MP4 (a GIF has no sound), using the format in Settings."))
                    .child(crate::ui::button_icon("export-video", "download-2-line", if exporting { "Exporting…" } else { "Export as video" }, crate::ui::Btn::Normal).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            if !s.exporting_show {
                                s.export_show_video();
                            }
                            cx.notify();
                        });
                    })))
                    .child(muted("Show file: share or back up the rundown with its maps and graphics."))
                    .child(
                        row()
                            .gap(px(6.))
                            .child(button_small("export-file", "Export .cwxshow").on_click(cx.listener(|this, _, _, cx| {
                                let dialog = this.app.read(cx).export_show_dialog();
                                crate::ui::pick_off_thread(cx, move || dialog.save_file(), |this: &mut Self, path, cx| {
                                    this.app.update(cx, |s, cx| {
                                        s.export_show_file(path);
                                        cx.notify();
                                    });
                                });
                            })))
                            .child(button_small("import-file", "Import…").on_click(cx.listener(|_this, _, window, cx| {
                                crate::ui::pick_off_thread_in(window, cx, || rfd::FileDialog::new()
                                    .set_title("Import show")
                                    .add_filter("OpenWX show", &["cwxshow", "json"])
                                    .pick_file(), |this: &mut Self, path, window, cx| {
                                    let show = this.app.update(cx, |s, cx| {
                                        let show = s.import_show_file(path);
                                        cx.notify();
                                        show
                                    });
                                    if let Some(show) = show {
                                        this.load_show(show, window, cx);
                                    }
                                });
                            }))),
                    )
                    .child(button_small("open-exports", "Open Exports folder").on_click(|_, _, _| {
                        let _ = open::that(crate::config::LibraryPaths::get().exports);
                    })),
            )
            .child(section("Saved shows").gap(px(5.)).children(items))
    }

    /// The rundown column (right side) and the scene timeline (under the big preview).
    fn rundown(&mut self, cx: &mut Context<Self>) -> (AnyElement, AnyElement) {
        let state = self.app.read(cx);
        let scenes = state.show.scenes.clone();
        let on_air = state.show_pos;
        let next = match on_air {
            Some(i) if i + 1 < scenes.len() => Some(i + 1),
            None if !scenes.is_empty() => Some(0),
            _ => None,
        };
        let graphics = state.graphics.clone();
        let elapsed = state.scene_elapsed();
        let selected = self.selected;
        // Running time: timed scenes count as set; held scenes count the time to read their
        // notes aloud (about 150 words a minute), or 10 s when they have none.
        let read_secs = |notes: &str| (notes.split_whitespace().count() as f32 / 150.0 * 60.0).round();
        let total: f32 = scenes
            .iter()
            .map(|s| if s.duration > 0.0 { s.duration } else { read_secs(&s.notes).max(10.0) })
            .sum();
        let held = scenes.iter().filter(|s| s.duration <= 0.0).count();
        let running_time = format!(
            "Running time about {}:{:02}{}",
            (total / 60.0) as u32,
            (total % 60.0) as u32,
            if held > 0 { format!(" ({held} held scene(s) estimated)") } else { String::new() }
        );

        // Timeline: one block per scene, width by running time; click to select.
        let secs_of = |s: &Scene| if s.duration > 0.0 { s.duration } else { read_secs(&s.notes).max(10.0) };
        let timeline = row()
            .w_full()
            .h(px(34.))
            .gap(px(2.))
            .children(scenes.iter().enumerate().map(|(i, s)| {
                let share = if total > 0.0 { secs_of(s) / total } else { 0.0 };
                let is_air = on_air == Some(i);
                let fill = if is_air && s.duration > 0.0 { (elapsed / s.duration).clamp(0.0, 1.0) } else { 0.0 };
                div()
                    .id(("tl", i))
                    .relative()
                    .h_full()
                    .w(relative(share.max(0.01)))
                    .rounded(px(4.))
                    .overflow_hidden()
                    .cursor_pointer()
                    .bg(c(if matches!(s.kind, SceneKind::Map { .. }) { 0x1E4A5A } else { 0x3A2E5A }))
                    .border_1()
                    .border_color(if is_air { c(RED) } else if selected == Some(i) { c(CYAN) } else { c(BORDER) })
                    .child(div().absolute().top_0().left_0().h_full().w(relative(fill)).bg(c(0x5A1A20)))
                    .child(div().absolute().top(px(8.)).left(px(6.)).text_size(px(11.)).text_color(c(TEXT)).truncate().child(format!("{} {}", i + 1, s.title)))
                    .on_click(cx.listener(move |this, _, window, cx| this.select(Some(i), window, cx)))
            }).collect::<Vec<_>>());

        // Rehearsal: step through on the preview only and time each scene.
        let rehearsal_bar = match &self.rehearsal {
            None => row()
                .flex_wrap()
                .gap(px(6.))
                .child(button_small("rehearse", "Rehearse (preview only)").on_click(cx.listener(|this, _, window, cx| {
                    if this.app.read(cx).show.scenes.is_empty() {
                        return;
                    }
                    this.rehearsal = Some(Rehearsal { index: 0, started: std::time::Instant::now(), times: Vec::new() });
                    this.rehearsal_report = None;
                    this.select(Some(0), window, cx);
                })))
                .when_some(self.rehearsal_report.clone(), |d, report| {
                    let planned: f32 = report.iter().map(|r| r.1).sum();
                    let actual: f32 = report.iter().map(|r| r.2).sum();
                    let slow: Vec<String> = report.iter().filter(|r| r.2 - r.1 > 5.0).map(|r| format!("{} (+{:.0}s)", r.0, r.2 - r.1)).collect();
                    d.child(muted(format!(
                        "Rehearsal: {:.0}:{:02.0} actual vs {:.0}:{:02.0} planned{}",
                        (actual / 60.0).floor(),
                        actual % 60.0,
                        (planned / 60.0).floor(),
                        planned % 60.0,
                        if slow.is_empty() { String::new() } else { format!(" • long: {}", slow.join(", ")) }
                    )))
                }),
            Some(r) => {
                let i = r.index;
                let secs = r.started.elapsed().as_secs_f32();
                let planned = scenes.get(i).map(|s| secs_of(s)).unwrap_or(0.0);
                row()
                    .flex_wrap()
                    .gap(px(6.))
                    .child(badge("REHEARSAL", AMBER))
                    // A talk timer: amber near the plan, red once over it (CWXB-81).
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(c(if planned > 0.0 && secs > planned { RED } else if planned > 0.0 && secs > planned * 0.8 { AMBER } else { MUTED }))
                            .child(format!(
                                "Scene {} of {} • {:.0}s of {:.0}s{}",
                                i + 1,
                                scenes.len(),
                                secs,
                                planned,
                                if planned > 0.0 && secs > planned { format!(" • {:.0}s over", secs - planned) } else { String::new() }
                            )),
                    )
                    .child(muted("Talk through the scene, then click Next scene."))
                    .child(button_small("rehearse-next", "Next scene").on_click(cx.listener(move |this, _, window, cx| {
                        let scenes = this.app.read(cx).show.scenes.clone();
                        let Some(r) = this.rehearsal.as_mut() else { return };
                        let actual = r.started.elapsed().as_secs_f32();
                        if let Some(s) = scenes.get(r.index) {
                            let plan = if s.duration > 0.0 { s.duration } else { (s.notes.split_whitespace().count() as f32 / 150.0 * 60.0).round().max(10.0) };
                            r.times.push((s.title.clone(), plan, actual));
                        }
                        r.index += 1;
                        r.started = std::time::Instant::now();
                        let next = r.index;
                        if next >= scenes.len() {
                            this.rehearsal_report = this.rehearsal.take().map(|r| r.times);
                            cx.notify();
                        } else {
                            this.select(Some(next), window, cx);
                        }
                    })))
                    .child(button_small("rehearse-stop", "Stop").on_click(cx.listener(|this, _, _, cx| {
                        this.rehearsal_report = this.rehearsal.take().map(|r| r.times);
                        cx.notify();
                    })))
            }
        };

        let rows = scenes
            .iter()
            .enumerate()
            .map(|(i, scene)| {
                let is_air = on_air == Some(i);
                let is_next = next == Some(i);
                let is_sel = selected == Some(i);
                let duration = if scene.duration <= 0.0 {
                    "HOLD".to_string()
                } else {
                    format!("{:.0}s", scene.duration)
                };
                let progress = if is_air && scene.duration > 0.0 {
                    (elapsed / scene.duration).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let mmss = |s: f32| format!("{}:{:02}", (s / 60.0) as u32, (s % 60.0) as u32);
                // On air: how far into the scene, and what is left (held scenes count up).
                let timer = is_air.then(|| if scene.duration > 0.0 {
                    format!("{} of {} • {} left", mmss(elapsed.min(scene.duration)), mmss(scene.duration), mmss((scene.duration - elapsed).max(0.0)))
                } else {
                    format!("{} on air • held until Next", mmss(elapsed))
                });
                // A card per scene, two lines so it fits the side column: picture, title and
                // notes on top; kind, transition, length and the order buttons below.
                col()
                    .id(("scene", i))
                    .relative()
                    // Never squeezed: in a long rundown the column shrank every card and cut off
                    // its second line (kind / transition / length / buttons).
                    .flex_shrink_0()
                    .gap(px(6.))
                    .px(px(10.))
                    .pt(px(8.))
                    .pb(px(10.))
                    .rounded(px(6.))
                    .overflow_hidden()
                    .cursor_pointer()
                    .border_1()
                    .border_color(if is_air { c(RED) } else if is_sel { c(CYAN) } else { c(BORDER) })
                    .bg(if is_air { c(0x2A1014) } else { c(PANEL_2) })
                    .hover(|s| s.bg(c(RAISED)))
                    .child(
                        row()
                            .gap(px(8.))
                            .items_center()
                            .child(div().w(px(20.)).text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(c(MUTED)).child(format!("{}", i + 1)))
                            .when_some(Some(map_thumb_path(&scene.id)).filter(|p| p.exists()), |d, p| {
                                d.child(img(p).w(px(64.)).h(px(36.)).flex_shrink_0().rounded(px(3.)).object_fit(ObjectFit::Cover))
                            })
                            .child(
                                col()
                                    .flex_1()
                                    .min_w_0()
                                    .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).truncate().child(scene.title.clone()))
                                    .when(!scene.notes.is_empty(), |d| {
                                        d.child(div().text_size(px(11.)).text_color(c(MUTED)).truncate().child(scene.notes.clone()))
                                    }),
                            )
                            .when(is_air, |d| d.child(badge("ON AIR", RED)))
                            .when(is_next && !is_air, |d| d.child(badge("NEXT", GREEN))),
                    )
                    .child(
                        row()
                            .gap(px(8.))
                            .items_center()
                            .pl(px(28.))
                            .child(badge(scene.kind_label(), if matches!(scene.kind, SceneKind::Map { .. }) { CYAN } else { 0xB48CFF }))
                            .when(matches!(&scene.kind, SceneKind::Graphic { graphic_id, .. } if !graphics.iter().any(|g| &g.id == graphic_id)), |d| {
                                d.child(badge("Saved copy", AMBER))
                            })
                            .child(div().text_size(px(12.)).text_color(c(MUTED)).child(scene.transition.label()))
                            .child(div().text_size(px(12.)).text_color(c(TEXT)).child(duration))
                            .child(div().flex_1())
                            .child(crate::ui::button_icon(("up", i), "arrow-up-line", "", crate::ui::Btn::Small).on_click(cx.listener(move |this, _, window, cx| {
                        if i > 0 {
                            this.app.update(cx, |s, cx| {
                                s.show.scenes.swap(i, i - 1);
                                cx.notify();
                            });
                            this.select(Some(i - 1), window, cx);
                        }
                    })))
                    .child(crate::ui::button_icon(("down", i), "arrow-down-line", "", crate::ui::Btn::Small).on_click(cx.listener(move |this, _, window, cx| {
                        let len = this.app.read(cx).show.scenes.len();
                        if i + 1 < len {
                            this.app.update(cx, |s, cx| {
                                s.show.scenes.swap(i, i + 1);
                                cx.notify();
                            });
                            this.select(Some(i + 1), window, cx);
                        }
                    })))
                    .child(crate::ui::button_icon(("remove", i), "close-line", "", crate::ui::Btn::Small).on_click(cx.listener(move |this, _, window, cx| {
                        this.app.update(cx, |s, cx| {
                            if i < s.show.scenes.len() {
                                s.show.scenes.remove(i);
                            }
                            s.show_pos = None;
                            cx.notify();
                        });
                        this.select(None, window, cx);
                    }))),
                    )
                    .when_some(timer, |d, t| d.child(div().pl(px(28.)).text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(t)))
                    // Progress along the bottom of the scene on air.
                    .when(is_air, |d| {
                        d.child(div().absolute().bottom_0().left_0().h(px(4.)).w_full().bg(c(0x3A1A20)))
                            .child(div().absolute().bottom_0().left_0().h(px(4.)).w(relative(progress)).bg(c(RED)))
                    })
                    .on_drag(DraggedScene { index: i, title: scene.title.clone() }, |d, _, _, cx| cx.new(|_| d.clone()))
                    .drag_over::<DraggedScene>(|s, _, _, _| s.border_color(c(CYAN)).bg(c(RAISED)))
                    .on_drop(cx.listener(move |this, d: &DraggedScene, window, cx| {
                        let from = d.index;
                        if from == i {
                            return;
                        }
                        this.app.update(cx, |s, cx| {
                            if from < s.show.scenes.len() && i < s.show.scenes.len() {
                                let scene = s.show.scenes.remove(from);
                                s.show.scenes.insert(i, scene);
                                // Keep the on-air marker on the same scene.
                                s.show_pos = s.show_pos.map(|p| {
                                    if p == from {
                                        i
                                    } else if from < p && i >= p {
                                        p - 1
                                    } else if from > p && i <= p {
                                        p + 1
                                    } else {
                                        p
                                    }
                                });
                            }
                            cx.notify();
                        });
                        this.select(Some(i), window, cx);
                    }))
                    .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                        window.focus(&this.focus, cx);
                        this.select(Some(i), window, cx);
                        if e.click_count() >= 2 {
                            this.app.update(cx, |s, cx| {
                                s.go_to_scene(i);
                                cx.notify();
                            });
                        }
                    }))
            })
            .collect::<Vec<_>>();

        let picker = graphics
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let doc = g.clone();
                chip(("pick", i), g.name.clone(), false).on_click(cx.listener(move |this, _, _, cx| {
                    let doc = doc.clone();
                    this.app.update(cx, |s, cx| {
                        s.show.scenes.push(Scene::graphic(&doc));
                        cx.notify();
                    });
                    this.picking_graphic = false;
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();

        let rundown = col()
            .id("rundown")
            .w(px(440.))
            .flex_shrink_0()
            .h_full()
            .p(px(12.))
            .gap(px(8.))
            .bg(c(PANEL))
            .border_l_1()
            .border_color(c(BORDER))
            .overflow_y_scroll()
            .child(heading("Rundown"))
            .child(
                row()
                    .flex_wrap()
                    .gap(px(6.))
                    .child(button_small("add-map", "+ Current map").on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.add_map_to_show(false);
                            if let Some(scene) = s.show.scenes.last() {
                                save_map_thumb(&scene.id, s.compositor.map_frame());
                            }
                            cx.notify();
                        });
                    })))
                    .child(button_small("add-clip", "+ GIF / MP4 clip").on_click(cx.listener(|_this, _, _, cx| {
                        crate::ui::pick_off_thread(cx, || rfd::FileDialog::new().set_title("Add a clip to the show")
                            .add_filter("Animated GIF or MP4", &["gif", "mp4"]).pick_file(), |this: &mut Self, path, cx| {
                            this.app.update(cx, |s, cx| { s.add_show_clip(path); cx.notify(); });
                        });
                    })))
                    .child(button_small("add-gfx", if self.picking_graphic { "Close" } else { "+ Graphic" }).on_click(cx.listener(|this, _, _, cx| {
                        this.picking_graphic = !this.picking_graphic;
                        cx.notify();
                    }))),
            )
            .when(self.picking_graphic, |d| {
                d.child(
                    col()
                        .gap(px(6.))
                        .p(px(10.))
                        .rounded(px(6.))
                        .bg(c(PANEL))
                        .border_1()
                        .border_color(c(BORDER))
                        .child(muted("Pick a graphic to add"))
                        .child(row().flex_wrap().gap(px(5.)).children(picker))
                        .when(graphics.is_empty(), |d| d.child(muted("No graphics yet - make one in Graphics."))),
                )
            })
            .when(scenes.is_empty(), |d| {
                d.child(
                    col()
                        .items_center()
                        .justify_center()
                        .py(px(60.))
                        .gap(px(6.))
                        .child(label("This show is empty"))
                        .child(muted("Add the current map view, or graphics from your library. Double-click a scene to take it to air.")),
                )
            })
            .child(muted(running_time.clone()))
            .child(rehearsal_bar)
            .children(rows);
        let timeline = if scenes.is_empty() { div().into_any_element() } else { timeline.into_any_element() };
        (rundown.into_any_element(), timeline)
    }

    /// Planned seconds per scene: timed scenes as set, held scenes estimated from their notes
    /// (about 150 words a minute, at least 10 s), the same rule as the running time.
    fn planned_secs(scenes: &[Scene]) -> Vec<f32> {
        scenes
            .iter()
            .map(|s| if s.duration > 0.0 { s.duration } else { (s.notes.split_whitespace().count() as f32 / 150.0 * 60.0).round().max(10.0) })
            .collect()
    }

    /// Scrub bar and transport under the Program preview (CWXB-104): drag anywhere on the bar
    /// to jump to that scene and time, scene markers and numbers on the bar, a time readout,
    /// and start / previous / play-pause / next / end / loop.
    fn transport(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.app.read(cx);
        let scenes = state.show.scenes.clone();
        let pos = state.show_pos;
        let running = state.show_running;
        let paused = state.scene_paused.is_some();
        let looping = state.show.loop_show;
        let elapsed = state.scene_elapsed();
        let secs = Self::planned_secs(&scenes);
        let total: f32 = secs.iter().sum::<f32>().max(0.001);
        let starts: Vec<f32> = secs.iter().scan(0.0, |acc, s| { let start = *acc; *acc += s; Some(start) }).collect();
        let now = pos.map(|i| starts[i.min(starts.len().saturating_sub(1))] + elapsed.min(secs.get(i).copied().unwrap_or(0.0))).unwrap_or(0.0);
        let frac = (now / total).clamp(0.0, 1.0);
        let clock = |t: f32| format!("{}:{:02}", (t / 60.0) as u32, (t % 60.0) as u32);
        let readout = format!("{} / {}", clock(now), clock(total));
        let scene_text = match pos {
            Some(i) => format!("Scene {} of {}: {}", i + 1, scenes.len(), scenes.get(i).map(|s| s.title.clone()).unwrap_or_default()),
            None if scenes.is_empty() => "No scenes yet".to_string(),
            None => format!("Ready: {} scenes", scenes.len()),
        };

        // Bar geometry is captured at paint time, like the sliders.
        let bounds: Rc<std::cell::Cell<Bounds<Pixels>>> = Rc::new(std::cell::Cell::new(Bounds::default()));
        let capture = bounds.clone();
        let (down_b, move_b) = (bounds.clone(), bounds.clone());
        let (secs_d, starts_d) = (secs.clone(), starts.clone());
        let seek_at = Rc::new(move |this: &mut Self, b: Bounds<Pixels>, x: Pixels, cx: &mut Context<Self>| {
            let width = f32::from(b.size.width).max(1.0);
            let t = (f32::from(x - b.origin.x) / width).clamp(0.0, 1.0) * total;
            let Some(i) = starts_d.iter().rposition(|s| *s <= t) else { return };
            let offset = (t - starts_d[i]).min(secs_d[i]);
            this.app.update(cx, |s, cx| {
                if s.show_pos != Some(i) {
                    let was_paused = s.scene_paused.is_some();
                    s.go_to_scene(i);
                    if was_paused || !s.show_running {
                        s.scene_paused = Some(offset);
                    }
                }
                s.set_scene_offset(offset);
                cx.notify();
            });
            this.selected = Some(i);
            cx.notify();
        });
        let (seek_down, seek_move) = (seek_at.clone(), seek_at.clone());

        let markers = starts.iter().enumerate().map(|(i, start)| {
            let left = start / total;
            let width = secs[i] / total;
            let on = pos == Some(i);
            div()
                .absolute()
                .top_0()
                .left(relative(left))
                .w(relative(width))
                .h_full()
                .border_l_2()
                .border_color(c(if on { RED } else { 0x5A6478 }))
                .when(i % 2 == 1, |d| d.bg(crate::ui::ca(0xFFFFFF, 0.04)))
                .child(div().pl(px(4.)).pt(px(2.)).text_size(px(10.)).text_color(c(if on { TEXT } else { MUTED })).truncate().child(format!("{}", i + 1)))
        }).collect::<Vec<_>>();

        let bar = div()
            .id("show-scrub")
            .relative()
            .w_full()
            .h(px(40.))
            .rounded(px(6.))
            .overflow_hidden()
            .bg(c(PANEL_2))
            .border_1()
            .border_color(c(BORDER))
            .cursor_pointer()
            .child(canvas(move |b, _, _| capture.set(b), |_, _, _, _| {}).absolute().size_full())
            .child(div().absolute().left_0().bottom_0().h(px(8.)).w(relative(frac)).bg(c(RED)))
            .children(markers)
            .child(div().absolute().top_0().left(relative(frac)).ml(px(-2.)).w(px(4.)).h_full().bg(c(0xFFFFFF)))
            .child(div().absolute().bottom(px(2.)).left(relative(frac)).ml(px(-7.)).w(px(14.)).h(px(14.)).rounded(px(7.)).bg(c(0xFFFFFF)).border_2().border_color(c(RED)))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                if !this.app.read(cx).show.scenes.is_empty() {
                    seek_down(this, down_b.get(), e.position.x, cx);
                }
            }))
            .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
                if e.pressed_button == Some(MouseButton::Left) && !this.app.read(cx).show.scenes.is_empty() {
                    seek_move(this, move_b.get(), e.position.x, cx);
                }
            }));

        let last = scenes.len().saturating_sub(1);
        let has = !scenes.is_empty();
        // One button shape for the whole transport: same height, icon then label, so the row
        // reads like a video player's controls (owner: "some of the buttons are irregular").
        let tbtn = |id: &'static str, icon: &'static str, text: &'static str, accent: Option<u32>| {
            let ink = accent.unwrap_or(TEXT);
            div()
                .id(id)
                .flex()
                .flex_row()
                .flex_shrink_0()
                .items_center()
                .gap(px(6.))
                .h(px(34.))
                .px(px(12.))
                .rounded(px(6.))
                .border_1()
                .border_color(accent.map(c).unwrap_or(c(BORDER)))
                .bg(match accent { Some(a) => crate::ui::ca(a, 0.16), None => Hsla::from(c(PANEL_2)) })
                .hover(|s| s.bg(c(RAISED)))
                .cursor_pointer()
                .child(crate::ui::icons::ri(icon, 16., ink))
                .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(c(ink)).child(text))
        };
        let controls = row()
            .w_full()
            .gap(px(6.))
            .items_center()
            .flex_wrap()
            .child(tbtn("tr-start", "skip-back-line", "Start", None).on_click(cx.listener(move |this, _, _, cx| {
                if has {
                    this.app.update(cx, |s, cx| {
                        s.go_to_scene(0);
                        cx.notify();
                    });
                }
            })))
            .child(tbtn("tr-prev", "arrow-left-s-line", "Previous", None).on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, cx| {
                    s.prev_scene();
                    cx.notify();
                });
            })))
            .child(
                if running { tbtn("tr-play", "pause-line", "Pause", Some(RED)) } else { tbtn("tr-play", "play-line", if paused { "Resume" } else { "Play" }, Some(GREEN)) }
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            if s.show_running {
                                s.pause_show();
                            } else {
                                s.resume_show();
                            }
                            cx.notify();
                        });
                    })),
            )
            .child(tbtn("tr-next", "arrow-right-s-line", "Next", None).on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, cx| {
                    s.next_scene();
                    cx.notify();
                });
            })))
            .child(tbtn("tr-end", "skip-forward-line", "End", None).on_click(cx.listener(move |this, _, _, cx| {
                if has {
                    this.app.update(cx, |s, cx| {
                        s.go_to_scene(last);
                        cx.notify();
                    });
                }
            })))
            .child(tbtn("tr-loop", "repeat-line", if looping { "Loop on" } else { "Loop off" }, looping.then_some(CYAN)).on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, cx| {
                    s.show.loop_show = !s.show.loop_show;
                    s.save_show();
                    cx.notify();
                });
            })))
            .child(div().flex_1())
            .child(div().text_size(px(12.)).text_color(c(MUTED)).truncate().max_w(px(360.)).child(scene_text))
            .child(div().px(px(10.)).py(px(4.)).rounded(px(6.)).bg(c(PANEL_2)).text_size(px(15.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).child(readout));

        col().w_full().gap(px(6.)).child(bar).child(controls).into_any_element()
    }

    /// The centre: a large Program preview with the take controls and the scene timeline
    /// under it, then the selected scene's settings and the broadcast tools (CWXB-104).
    fn program_panel(&mut self, timeline: AnyElement, cx: &mut Context<Self>) -> impl IntoElement {
        let outputs = self.outputs_panel(cx);
        let autocast = self.autocast_panel(cx);
        let captions = self.captions_panel(cx);
        let schedule = self.schedule_panel(cx);
        let transport = self.transport(cx);
        let image = self.frames.read(cx).program.clone();
        let state = self.app.read(cx);
        let on_air = state.on_air.clone();
        let black = state.black;
        let running = state.show_running;
        let has_switcher = state.has_switcher();
        let has_external_switcher = state.has_external_switcher();
        let scene = self.selected.and_then(|i| state.show.scenes.get(i).cloned());
        let air_label = match &on_air {
            OnAir::LiveMap => "Source: Map Studio".to_string(),
            OnAir::Graphic(name) => format!("Source: {name}"),
        };

        let transitions = Transition::ALL
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let t = *t;
                let active = scene.as_ref().map(|s| s.transition == t).unwrap_or(false);
                chip(("transition", i), t.label(), active).on_click(cx.listener(move |this, _, _, cx| {
                    this.edit_scene(cx, |s| s.transition = t);
                }))
            })
            .collect::<Vec<_>>();

        col()
            .id("program-panel")
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(c(BG))
            .overflow_y_scroll()
            .child(
                col()
                    .p(px(14.))
                    .gap(px(8.))
                    .child(
                        row()
                            .gap(px(8.))
                            .child(badge("PROGRAM PREVIEW", CYAN))
                            .child(label(air_label))
                            .when(black, |d| d.child(badge("BLACK", AMBER))),
                    )
                    .child(div().w_full().h(px(self.preview_h)).child(frame_canvas(image, self.slot.clone()).size_full()))
                    .child(transport)
                    .child(
                        row()
                            .gap(px(6.))
                            .child(crate::ui::button_icon("prev", "arrow-left-s-line", "Prev", crate::ui::Btn::Normal).on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.prev_scene();
                                    cx.notify();
                                });
                            })))
                            .child(button_danger("take", "TAKE").flex_1().on_click(cx.listener(|this, _, _, cx| {
                                if let Some(i) = this.selected {
                                    this.app.update(cx, |s, cx| {
                                        s.go_to_scene(i);
                                        cx.notify();
                                    });
                                }
                            })))
                            .child(crate::ui::button_icon("next", "arrow-right-s-line", "Next", crate::ui::Btn::Normal).on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.next_scene();
                                    cx.notify();
                                });
                            }))),
                    )
                    .child(
                        row()
                            .gap(px(6.))
                            .child(
                                if running { crate::ui::button_icon("auto", "stop-line", "Stop auto-run", crate::ui::Btn::Primary) } else { crate::ui::button_icon("auto", "play-line", "Auto-run show", crate::ui::Btn::Normal) }
                                    .flex_1()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.app.update(cx, |s, cx| {
                                            s.show_running = !s.show_running;
                                            if s.show_running && s.show_pos.is_none() {
                                                s.next_scene();
                                            }
                                            cx.notify();
                                        });
                                    })),
                            )
                            .child(button("live", "Live map").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.take_live_map(Transition::Fade);
                                    s.show_pos = None;
                                    cx.notify();
                                });
                            })))
                            .child(
                                if black { button_primary("black", "Unblack") } else { button("black", "Black") }.on_click(cx.listener(|this, _, _, cx| {
                                    this.app.update(cx, |s, cx| {
                                        let b = !s.black;
                                        s.set_black(b);
                                        cx.notify();
                                    });
                                })),
                            ),
                    )
                    .when(has_switcher, |d| {
                        d.child(
                            row()
                                .gap(px(6.))
                                .child(muted("Switcher"))
                                .when(has_external_switcher, |row| row.child(button_small("sw-cut", "Cut").on_click(cx.listener(|this, _, _, cx| {
                                    this.app.update(cx, |s, cx| {
                                        s.switcher_transition(false, cx);
                                        cx.notify();
                                    });
                                }))))
                                .child(button_small("sw-auto", if has_external_switcher { "Auto" } else { "OBS transition" }).on_click(cx.listener(|this, _, _, cx| {
                                    this.app.update(cx, |s, cx| {
                                        s.switcher_transition(true, cx);
                                        cx.notify();
                                    });
                                }))),
                        )
                    })
                    .child({
                        // Presenter controls for an interactive graphic on air.
                        let states: Vec<String> = match &on_air {
                            OnAir::Graphic(name) => state.graphics.iter().find(|g| &g.name == name).map(|g| g.states.clone()).unwrap_or_default(),
                            _ => Vec::new(),
                        };
                        row()
                            .flex_wrap()
                            .gap(px(4.))
                            .children(states.into_iter().enumerate().map(|(i, s)| {
                                let a = format!("state:{s}");
                                button_small(("pres-state", i), s).on_click(cx.listener(move |this, _, _, cx| {
                                    let a = a.clone();
                                    this.app.update(cx, |s, cx| {
                                        s.graphic_action(&a, "presenter");
                                        cx.notify();
                                    });
                                }))
                            }).collect::<Vec<_>>())
                            .child(button_small("pres-pause", "Pause").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.graphic_action("pause", "presenter");
                                    cx.notify();
                                });
                            })))
                            .child(button_small("pres-play", "Resume").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.graphic_action("play", "presenter");
                                    cx.notify();
                                });
                            })))
                    })
                    .child(muted("Keys: Space/→ next • ← previous • Enter take selected • ↑↓ select"))
                    .child(timeline),
            )
            .child(
                section("Scene")
                    .when(scene.is_none(), |d| d.child(muted("Select a scene in the rundown.")))
                    .when_some(scene.clone(), |d, scene| {
                        let duration = scene.duration;
                        d.child(Input::new(&self.scene_title))
                            .child(row().justify_between().child(muted("Duration")).child(muted(if duration <= 0.0 { "Hold until Next".to_string() } else { format!("{duration:.0} seconds") })))
                            .child(slider("scene-duration", duration, 0.0, 120.0, cx, |this, v, _, cx| {
                                this.edit_scene(cx, |s| s.duration = v.round());
                            }))
                            .child(muted("Transition"))
                            .child(row().gap(px(5.)).children(transitions))
                            .child(muted("Scene soundtrack"))
                            .child(row().flex_wrap().gap(px(5.))
                                .child(chip("scene-catalog", "Weather music ▾", self.music_catalog_open).on_click(cx.listener(|this, _, _, cx| {
                                    this.music_catalog_open = !this.music_catalog_open;
                                    this.bed_picker = false;
                                    cx.notify();
                                })))
                                .child(button_small("scene-soundtrack", "Choose file…").on_click(cx.listener(|this, _, _, cx| {
                                    this.bed_picker = false;
                                    this.choose_scene_soundtrack(cx);
                                })))
                                .when(scene.soundtrack.is_some(), |d| d.child(button_small("scene-soundtrack-clear", "Remove").on_click(cx.listener(|this, _, _, cx| {
                                    this.edit_scene(cx, |scene| scene.soundtrack = None);
                                })))))
                            .when(self.music_catalog_open, |d| d.child(
                                col().gap(px(5.))
                                    .child(muted("CC0 instrumental music"))
                                    .children(WEATHER_MUSIC.into_iter().enumerate().map(|(i, (title, description, filename, bytes))| {
                                        row().flex_wrap().gap(px(6.))
                                            .child(button_small(("catalog-track", i), title).on_click(cx.listener(move |this, _, _, cx| {
                                                this.attach_imported_soundtrack(library::import_builtin_track(filename, bytes), cx);
                                            })))
                                            .child(muted(description))
                                    }).collect::<Vec<_>>())
                                    .child(self.library_music(cx)),
                            ))
                            .child(muted(scene.soundtrack.clone().unwrap_or_else(|| "Use the graphic or clip’s own audio when available".into())))
                            .child(muted("A scene soundtrack replaces the scene’s own audio and plays once when taken. It also silences the show soundtrack while it plays. Save the show to keep it."))
                            .child(muted("Notes"))
                            .child(Input::new(&self.scene_notes))
                            .when_some(Some(scene.clone()).filter(|s| !s.notes.trim().is_empty()), |d, s| {
                                let words = s.notes.split_whitespace().count();
                                d.child(muted(format!("Script: {words} words, about {:.0} s to read aloud", words as f32 / 150.0 * 60.0)))
                                    .child(
                                        div()
                                            .p(px(10.))
                                            .rounded(px(6.))
                                            .bg(c(crate::ui::PANEL_2))
                                            .text_size(px(16.))
                                            .text_color(c(crate::ui::TEXT))
                                            .child(s.notes.clone()),
                                    )
                            })
                            .when(matches!(scene.kind, SceneKind::Map { .. }), |d| {
                                d.child(button_small("preview-move", "Preview the camera move").on_click(cx.listener(|this, _, _, cx| {
                                    let Some(i) = this.selected else { return };
                                    this.app.update(cx, |s, cx| {
                                        if matches!(s.on_air, OnAir::LiveMap) && !s.black {
                                            s.toast(ToastKind::Info, "Program is showing the map: take a graphic or Black first to preview off air");
                                        } else if let Some(Scene { kind: SceneKind::Map { map }, .. }) = s.show.scenes.get(i).cloned() {
                                            s.fly_to(map.view, Transition::Fly.seconds());
                                        }
                                        cx.notify();
                                    });
                                })))
                                .child(button_small("update-map", "Replace with current map").on_click(cx.listener(|this, _, _, cx| {
                                    let live = this.app.read(cx).live.clone();
                                    if let Some(id) = this.selected.and_then(|i| this.app.read(cx).show.scenes.get(i).map(|s| s.id.clone())) {
                                        save_map_thumb(&id, this.app.read(cx).compositor.map_frame());
                                    }
                                    this.edit_scene(cx, |s| s.kind = SceneKind::Map { map: live });
                                })))
                                .child(button_small("preview-map", "Load into Map Studio").on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(i) = this.selected {
                                        let scene = this.app.read(cx).show.scenes.get(i).cloned();
                                        if let Some(Scene { kind: SceneKind::Map { map }, .. }) = scene {
                                            this.app.update(cx, |s, cx| {
                                                let view = map.view;
                                                s.studio_map_before_show = None;
                                                s.live = map;
                                                s.set_view(view);
                                                let overlay = s.live.overlay_graphic.clone();
                                                s.set_map_overlay(overlay);
                                                s.sync_map();
                                                cx.notify();
                                            });
                                        }
                                    }
                                })))
                            })
                    }),
            )
            .child(autocast)
            .child(outputs)
            .child(captions)
            .child(schedule)
    }

    /// Automated broadcast: a continuous weather channel with interruptions for warnings.
    fn autocast_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let running = state.autocast_running();
        let s = state.settings.clone();
        const SEGMENTS: [(&str, &str); 9] = [
            ("current", "Current conditions"),
            ("radar", "Radar loop"),
            ("seven_day", "7-day forecast"),
            ("regional", "Regional temperatures"),
            ("hourly_strip", "Hour by hour"),
            ("severe_outlook", "Severe outlook"),
            ("travel_times", "Travel times"),
            ("aqi_today", "Air quality"),
            ("sun_moon", "Sun and moon"),
        ];
        section("Automated broadcast")
            .child(muted("A continuous weather channel: rotates segments, breaks in for new warnings, adds station IDs and a conditions ticker."))
            .child(if running { crate::ui::button_icon("autocast", "stop-line", "Stop automated broadcast", crate::ui::Btn::Primary) } else { crate::ui::button_icon("autocast", "play-line", "Start automated broadcast", crate::ui::Btn::Normal) }.on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, cx| {
                    s.toggle_autocast();
                    cx.notify();
                });
            })))
            .child(row().flex_wrap().gap(px(3.)).children(SEGMENTS.iter().enumerate().map(|(i, (key, text))| {
                let key: &'static str = key;
                let on = s.autocast_segments.iter().any(|k| k == key);
                chip(("ac-seg", i), *text, on).on_click(cx.listener(move |this, _, _, cx| {
                    this.app.update(cx, |s, cx| {
                        let mut settings = s.settings.clone();
                        if let Some(pos) = settings.autocast_segments.iter().position(|k| k == key) {
                            settings.autocast_segments.remove(pos);
                        } else {
                            settings.autocast_segments.push(key.to_string());
                        }
                        s.apply_settings(settings);
                        cx.notify();
                    });
                }))
            }).collect::<Vec<_>>()))
            .child(row().flex_wrap().gap(px(3.)).children([10.0f32, 20.0, 30.0, 60.0].into_iter().enumerate().map(|(i, v)| {
                chip(("ac-secs", i), format!("{v:.0} s each"), (s.autocast_seconds - v).abs() < 0.5).on_click(cx.listener(move |this, _, _, cx| {
                    this.app.update(cx, |s, cx| {
                        let mut settings = s.settings.clone();
                        settings.autocast_seconds = v;
                        s.apply_settings(settings);
                        cx.notify();
                    });
                }))
            }).collect::<Vec<_>>()))
            .child(row().flex_wrap().gap(px(3.)).children([0u32, 15, 30, 60].into_iter().enumerate().map(|(i, v)| {
                chip(("ac-id", i), if v == 0 { "No station ID".to_string() } else { format!("Station ID every {v} min") }, s.autocast_id_minutes == v).on_click(cx.listener(move |this, _, _, cx| {
                    this.app.update(cx, |s, cx| {
                        let mut settings = s.settings.clone();
                        settings.autocast_id_minutes = v;
                        s.apply_settings(settings);
                        cx.notify();
                    });
                }))
            }).collect::<Vec<_>>()))
            .child(
                row()
                    .gap(px(6.))
                    .child(toggle("ac-ticker", "Conditions ticker", s.autocast_ticker).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            let mut settings = s.settings.clone();
                            settings.autocast_ticker = !settings.autocast_ticker;
                            s.apply_settings(settings);
                            cx.notify();
                        });
                    })))
                    .child(toggle("ac-bulletin", "Spoken bulletin each cycle", s.autocast_bulletin).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            let mut settings = s.settings.clone();
                            settings.autocast_bulletin = !settings.autocast_bulletin;
                            s.apply_settings(settings);
                            cx.notify();
                        });
                    }))),
            )
    }

    /// Extra graphics channels and camera switching.
    fn outputs_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let selected_doc = self.selected.and_then(|i| state.show.scenes.get(i)).and_then(|s| match &s.kind {
            SceneKind::Graphic { graphic_id, doc } => Some(match state.scene_graphic(graphic_id, doc) {
                crate::app::SceneGraphic::Library(d) | crate::app::SceneGraphic::Relinked(d) | crate::app::SceneGraphic::Saved(d) => d,
            }),
            _ => None,
        });
        let port = state.settings.control_port;
        let obs = state.settings.obs_enabled;
        let channels = crate::channels::numbers()
            .into_iter()
            .map(|n| {
                let current = crate::channels::get(n).and_then(|c| c.current());
                let doc = selected_doc.clone();
                row()
                    .gap(px(6.))
                    .child(div().w(px(46.)).text_size(px(13.)).text_color(c(TEXT)).child(format!("Ch {n}")))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(12.)).text_color(c(MUTED)).child(current.unwrap_or_else(|| "Clear".into())))
                    .child(button_small(("ch-take", n), "Send selected").on_click(cx.listener(move |this, _, _, cx| {
                        let Some(doc) = doc.clone() else {
                            this.app.update(cx, |s, cx| {
                                s.toast(ToastKind::Info, "Select a graphic scene in the rundown first");
                                cx.notify();
                            });
                            return;
                        };
                        this.app.update(cx, |s, cx| {
                            if !s.approval_blocked(&doc) {
                                if let Some(ch) = crate::channels::get(n) {
                                    ch.take(doc);
                                }
                            }
                            cx.notify();
                        });
                    })))
                    .child(button_small(("ch-clear", n), "Clear").on_click(cx.listener(move |_, _, _, cx| {
                        if let Some(ch) = crate::channels::get(n) {
                            ch.clear();
                        }
                        cx.notify();
                    })))
            })
            .collect::<Vec<_>>();
        let cams = self
            .cameras
            .iter()
            .enumerate()
            .map(|(i, scene)| {
                let scene = scene.clone();
                button_small(("cam", i), scene.clone()).on_click(cx.listener(move |this, _, _, cx| {
                    let scene = scene.clone();
                    this.app.update(cx, |s, cx| {
                        s.obs_switch(scene, cx);
                        cx.notify();
                    });
                }))
            })
            .collect::<Vec<_>>();
        let webcams = self.app.read(cx).settings.webcams.clone();
        let media_dir = crate::config::LibraryPaths::get().media;
        let wall = webcams
            .iter()
            .enumerate()
            .map(|(i, cam)| {
                let st = crate::travel::cam_status(&cam.name);
                let age = st.last_ok.map(|t| (chrono::Utc::now().timestamp() - t) / 60);
                let path = media_dir.join(crate::travel::cam_file(&cam.name));
                let cam_ptz = cam.clone();
                let ptz_buttons = ["left", "up", "down", "right", "zoomin", "zoomout", "home"]
                    .into_iter()
                    .enumerate()
                    .map(|(j, m)| {
                        let c = cam_ptz.clone();
                        let icon = match m { "left" => "arrow-left-line", "right" => "arrow-right-line", "up" => "arrow-up-line", "down" => "arrow-down-line", "zoomin" => "zoom-in-line", "zoomout" => "zoom-out-line", _ => "home-4-line" };
                        crate::ui::button_icon(("ptz", i * 10 + j), icon, "", crate::ui::Btn::Small).on_click(move |_, _, _| crate::travel::ptz(&c, m))
                    })
                    .collect::<Vec<_>>();
                let name = cam.name.clone();
                col()
                    .w(px(200.))
                    .gap(px(3.))
                    .when(path.exists(), |d| d.child(crate::ui::thumb_box_path(path.clone(), 200., 112., 0x05070B, true).rounded(px(4.))))
                    .child(div().text_size(px(12.)).text_color(c(TEXT)).truncate().child(cam.name.clone()))
                    .child(div().text_size(px(11.)).text_color(c(match age { Some(a) if a <= 5 => GREEN, Some(_) => AMBER, None => RED })).child(match age {
                        Some(a) => format!("{} • {a} min old", st.condition),
                        None if !st.error.is_empty() => format!("Offline: {}", st.error.chars().take(40).collect::<String>()),
                        None => "Waiting for first image".into(),
                    }))
                    .when(!cam.ptz.is_empty(), |d| d.child(row().flex_wrap().gap(px(2.)).children(ptz_buttons)))
                    .child(button_small(("timelapse", i), "Time-lapse (24 h)").on_click(cx.listener(move |this, _, _, cx| {
                        let n = name.clone();
                        let task = cx.background_executor().spawn(async move { crate::travel::timelapse(&n, 24) });
                        cx.spawn(async move |this, cx| {
                            let result = task.await;
                            let _ = this.update(cx, |this, cx| {
                                this.app.update(cx, |s, cx| {
                                    match result {
                                        Ok(p) => s.toast(ToastKind::Success, format!("Time-lapse saved: {}", p.display())),
                                        Err(e) => s.toast(ToastKind::Error, format!("Time-lapse: {e}")),
                                    }
                                    cx.notify();
                                });
                            });
                        })
                        .detach();
                    })))
            })
            .collect::<Vec<_>>();
        section("Outputs and cameras")
            .when(!wall.is_empty(), |d| d.child(muted("Camera wall")).child(row().flex_wrap().gap(px(8.)).children(wall)))
            .child(muted(format!("Channels play graphics on their own, with transparency: http://127.0.0.1:{port}/channel2 (and 3). Producer page: /producer")))
            .children(channels)
            .when(obs, |d| {
                d.child(
                    row()
                        .gap(px(6.))
                        .child(muted("Cameras (OBS scenes)"))
                        .child(button_small("cam-load", "Load scenes").on_click(cx.listener(|this, _, _, cx| {
                            let (url, pw) = {
                                let s = &this.app.read(cx).settings;
                                (s.obs_url.clone(), s.obs_password.clone())
                            };
                            cx.spawn(async move |this, cx| {
                                let result = crate::net::spawn(crate::obs::scene_list(url, pw)).await;
                                let _ = this.update(cx, |this, cx| {
                                    if let Ok(Ok(scenes)) = result {
                                        this.cameras = scenes;
                                    }
                                    cx.notify();
                                });
                            })
                            .detach();
                        }))),
                )
                .child(row().flex_wrap().gap(px(4.)).children(cams))
                .child(row().flex_wrap().gap(px(6.))
                    .child(button_small("obs-save-replay", "Save OBS replay").on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |state, cx| state.obs_save_replay(cx));
                    })))
                    .child(muted("Start the Replay Buffer in OBS to save the last part of the broadcast.")))
            })
    }

    fn captions_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let enabled = state.settings.captions_enabled;
        let current = state.ops2.caption.clone();
        section("Captions")
            .when(!enabled, |d| d.child(muted("Captions are off. Open Settings → On-air & output → Captions.")))
            .child(Input::new(&self.caption))
            .child(
                row()
                    .gap(px(6.))
                    .child(button_small("cc-send", "Show caption").on_click(cx.listener(|this, _, window, cx| {
                        let text = this.caption.read(cx).value().to_string();
                        this.caption.update(cx, |s, cx| s.set_value(String::new(), window, cx));
                        this.app.update(cx, |s, cx| {
                            s.set_caption(&text);
                            cx.notify();
                        });
                    })))
                    .child(button_small("cc-clear", "Clear").on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.set_caption("");
                            cx.notify();
                        });
                    }))),
            )
            .when(!current.is_empty(), |d| d.child(muted(format!("On air: {current}"))))
    }

    fn schedule_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        const ACTIONS: [(&str, &str); 10] = [
            ("take", "Take scene"),
            ("next", "Next scene"),
            ("graphic", "Graphic"),
            ("live", "Live map"),
            ("severe", "Severe Mode"),
            ("black", "Black"),
            ("refresh", "Refresh radar"),
            ("channel2", "Graphic to Ch 2"),
            ("channel3", "Graphic to Ch 3"),
            ("audio", "Play audio file"),
        ];
        let items = self.app.read(cx).settings.schedule.clone();
        let action = self.sched_action;
        let daily = self.sched_daily;
        section("Schedule")
            .child(muted("Run a command at a clock time, e.g. take scene 1 at 5:58 PM every day."))
            .children(items.iter().enumerate().map(|(i, item)| {
                let label = ACTIONS.iter().find(|(k, _)| *k == item.action).map(|(_, l)| *l).unwrap_or(item.action.as_str());
                row()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.))
                            .text_color(c(TEXT))
                            .child(format!("{}  {label}{}{}", item.at, if item.arg.is_empty() { String::new() } else { format!(" {}", item.arg) }, if item.daily { " • daily" } else { " • once" })),
                    )
                    .child(button_small(("sched-del", i), "Remove").on_click(cx.listener(move |this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            let mut settings = s.settings.clone();
                            if i < settings.schedule.len() {
                                settings.schedule.remove(i);
                            }
                            s.apply_settings(settings);
                            cx.notify();
                        });
                    })))
            }).collect::<Vec<_>>())
            .child(row().flex_wrap().gap(px(4.)).children(ACTIONS.iter().enumerate().map(|(i, (key, text))| {
                let key: &'static str = *key;
                chip(("sched-act", i), *text, action == key).on_click(cx.listener(move |this, _, _, cx| {
                    this.sched_action = key;
                    cx.notify();
                }))
            }).collect::<Vec<_>>()))
            .child(row().gap(px(6.)).child(div().w(px(120.)).child(Input::new(&self.sched_time))).child(div().flex_1().child(Input::new(&self.sched_arg))))
            .child(toggle("sched-daily", "Every day", daily).on_click(cx.listener(|this, _, _, cx| {
                this.sched_daily = !this.sched_daily;
                cx.notify();
            })))
            .child(button_small("sched-add", "Add to schedule").on_click(cx.listener(|this, _, window, cx| {
                let raw = this.sched_time.read(cx).value().trim().to_string();
                let arg = this.sched_arg.read(cx).value().trim().to_string();
                let Some(at) = parse_clock(&raw) else {
                    this.app.update(cx, |s, cx| {
                        s.toast(ToastKind::Error, "Use a time like 17:58 or 5:58 PM");
                        cx.notify();
                    });
                    return;
                };
                let item = crate::broadcast_ops::Scheduled {
                    at,
                    action: this.sched_action.to_string(),
                    arg,
                    daily: this.sched_daily,
                    last_run: String::new(),
                };
                this.sched_time.update(cx, |s, cx| s.set_value(String::new(), window, cx));
                this.sched_arg.update(cx, |s, cx| s.set_value(String::new(), window, cx));
                this.app.update(cx, |s, cx| {
                    let mut settings = s.settings.clone();
                    settings.schedule.push(item);
                    settings.schedule.sort_by(|a, b| a.at.cmp(&b.at));
                    s.apply_settings(settings);
                    cx.notify();
                });
            })))
    }
}

/// "17:58", "5:58 PM", "558pm" -> "17:58"; "17:58:30" keeps the seconds.
fn parse_clock(raw: &str) -> Option<String> {
    let t = raw.trim().to_lowercase().replace(' ', "");
    let parts: Vec<&str> = t.trim_end_matches("pm").trim_end_matches("am").split(':').collect();
    if parts.len() == 3 {
        let secs = parts[2].parse::<u32>().ok().filter(|s| *s < 60)?;
        let suffix = if t.ends_with("pm") { "pm" } else if t.ends_with("am") { "am" } else { "" };
        let hm = parse_clock(&format!("{}:{}{suffix}", parts[0], parts[1]))?;
        return Some(format!("{hm}:{secs:02}"));
    }
    let (body, pm, am) = if let Some(b) = t.strip_suffix("pm") {
        (b.to_string(), true, false)
    } else if let Some(b) = t.strip_suffix("am") {
        (b.to_string(), false, true)
    } else {
        (t.clone(), false, false)
    };
    let (h, m) = match body.split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None if body.len() >= 3 => {
            let split = body.len() - 2;
            (body[..split].parse::<u32>().ok()?, body[split..].parse::<u32>().ok()?)
        }
        None => (body.parse::<u32>().ok()?, 0),
    };
    if m > 59 || h > 23 || ((pm || am) && (h == 0 || h > 12)) {
        return None;
    }
    let h = match (pm, am, h) {
        (true, _, 12) => 12,
        (true, _, h) => h + 12,
        (_, true, 12) => 0,
        (_, _, h) => h,
    };
    Some(format!("{h:02}:{m:02}"))
}

impl Render for ShowBuilder {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The centre preview fills the space between the show list (260) and the rundown
        // (440) at 16:9, but leaves room under it for the take controls.
        let vp = window.viewport_size();
        let centre_w = (f32::from(vp.width) - 260.0 - 440.0 - 28.0).max(320.0);
        self.preview_h = (centre_w * 9.0 / 16.0).min(f32::from(vp.height) * 0.56).max(200.0);
        let shows = self.shows_panel(cx);
        let (rundown, timeline) = self.rundown(cx);
        let program = self.program_panel(timeline, cx);
        row()
            .size_full()
            .items_start()
            .bg(c(BG))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .child(shows)
            .child(program)
            .child(rundown)
    }
}

/// Small picture of a map scene, captured from the Map bus when the scene is added or replaced.
fn map_thumb_path(scene_id: &str) -> std::path::PathBuf {
    crate::config::LibraryPaths::get().shows.join(".thumbs").join(format!("{}.png", scene_id.replace(['/', '\\', '.'], "_")))
}

fn save_map_thumb(scene_id: &str, frame: Option<crate::program::BusFrame>) {
    let Some(frame) = frame else { return };
    let path = map_thumb_path(scene_id);
    crate::jobs::submit(crate::jobs::Priority::Low, "map thumbnail", move || {
        let mut rgba = frame.bgra.as_ref().clone();
        for px in rgba.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        if let Some(img) = image::RgbaImage::from_raw(frame.width, frame.height, rgba) {
            let small = image::imageops::resize(&img, 256, 144, image::imageops::FilterType::Triangle);
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = small.save(&path);
        }
    });
}
