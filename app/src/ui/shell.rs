//! Main window: title bar, workspaces, status bar, dialogs and notifications.

use gpui_kit::{component::TitleBar, prelude::FluentBuilder as _, *};

use crate::{
    app::{AppState, FrameStore, Modal, ToastKind, Workspace},
    config::APP_VERSION,
    updater::UpdateState,
    ui::{
        about::AboutView, badge, button_small, c, ca, col, data_discovery::DataDiscovery,
        graphics_studio::GraphicsStudio, map_studio::MapStudio, media::MediaView,
        output::OutputView, row, settings::SettingsView, show_builder::ShowBuilder, AMBER, BG,
        BORDER, CYAN, DISPLAY_FONT, GREEN, MUTED, PANEL, RED, TEXT, UI_FONT,
    },
};

/// gpui-component's title bar: left padding before our content, and the min/max/close buttons.
const TITLE_BAR_PADDING: f32 = 12.0;
const WINDOW_CONTROLS_W: f32 = 3.0 * 34.0 + 2.0;

pub struct MainView {
    app: Entity<AppState>,
    frames: Entity<FrameStore>,
    map: Entity<MapStudio>,
    severe: Entity<crate::ui::severe_center::SevereCenter>,
    winter: Entity<crate::ui::winter_studio::WinterStudio>,
    graphics: Entity<GraphicsStudio>,
    show: Entity<ShowBuilder>,
    data: Entity<DataDiscovery>,
    media: Entity<MediaView>,
    settings: Entity<SettingsView>,
    community: Entity<crate::ui::community_board::CommunityBoard>,
    forecast_editor: Entity<crate::ui::forecast_editor::ForecastEditor>,
    desk: Entity<crate::ui::desk::DeskView>,
    history: Entity<crate::ui::history::HistoryView>,
    viewers: Entity<crate::ui::viewers::ViewersView>,
    publishing: Entity<crate::ui::publishing::PublishingView>,
    eoc: Entity<crate::ui::eoc::EocView>,
    workflows: Entity<crate::ui::workflows::WorkflowsView>,
    lab: Entity<crate::ui::lab::LabView>,
    themes: Entity<crate::ui::themes::ThemesView>,
    about: Entity<AboutView>,
    bug_report: Entity<crate::ui::bug_report::BugReportView>,
    setup_wizard: Entity<crate::ui::setup_wizard::SetupWizardView>,
    dev_logs: Entity<crate::ui::dev_logs::DevLogs>,
    dev_open: bool,
    last_modal: Modal,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl MainView {
    pub fn new(app: Entity<AppState>, frames: Entity<FrameStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let map = cx.new(|cx| MapStudio::new(app.clone(), frames.clone(), window, cx));
        let severe = cx.new(|cx| crate::ui::severe_center::SevereCenter::new(app.clone(), frames.clone(), cx));
        let winter = cx.new(|cx| crate::ui::winter_studio::WinterStudio::new(app.clone(), window, cx));
        let graphics = cx.new(|cx| GraphicsStudio::new(app.clone(), window, cx));
        let show = cx.new(|cx| ShowBuilder::new(app.clone(), frames.clone(), window, cx));
        let data = cx.new(|cx| DataDiscovery::new(app.clone(), window, cx));
        let media = cx.new(|cx| MediaView::new(app.clone(), window, cx));
        let settings = cx.new(|cx| SettingsView::new(app.clone(), window, cx));
        let community = cx.new(|cx| crate::ui::community_board::CommunityBoard::new(app.clone(), window, cx));
        let forecast_editor = cx.new(|cx| crate::ui::forecast_editor::ForecastEditor::new(app.clone(), window, cx));
        let desk = cx.new(|cx| crate::ui::desk::DeskView::new(app.clone(), window, cx));
        let history = cx.new(|cx| crate::ui::history::HistoryView::new(app.clone(), window, cx));
        let viewers = cx.new(|cx| crate::ui::viewers::ViewersView::new(app.clone(), window, cx));
        let publishing = cx.new(|cx| crate::ui::publishing::PublishingView::new(app.clone(), window, cx));
        let eoc = cx.new(|cx| crate::ui::eoc::EocView::new(app.clone(), window, cx));
        let workflows = cx.new(|cx| crate::ui::workflows::WorkflowsView::new(app.clone(), window, cx));
        let lab = cx.new(|cx| crate::ui::lab::LabView::new(app.clone(), window, cx));
        let themes = cx.new(|cx| crate::ui::themes::ThemesView::new(app.clone(), window, cx));
        let about = cx.new(|cx| AboutView::new(app.clone(), cx));
        let bug_report = cx.new(|cx| crate::ui::bug_report::BugReportView::new(app.clone(), window, cx));
        let dev_logs = cx.new(|cx| crate::ui::dev_logs::DevLogs::new(window, cx));
        let setup_wizard = cx.new(|cx| crate::ui::setup_wizard::SetupWizardView::new(app.clone(), window, cx));
        // First run: the setup wizard opens over the studio.
        if !app.read(cx).settings.setup_complete {
            app.update(cx, |s, _| s.modal = Modal::Setup);
        }
        let focus = cx.focus_handle();
        window.focus(&focus, cx);

        let subscriptions = vec![
            cx.observe_in(&app, window, |this, app, window, cx| {
                // Find anything (Map Studio) asks for a window or a Settings page.
                if let Some(req) = app.update(cx, |s, _| s.open_request.take()) {
                    match req {
                        crate::app::OpenRequest::Modal(m) => this.open_modal(m, window, cx),
                        crate::app::OpenRequest::Settings(page) => {
                            this.open_modal(Modal::Settings, window, cx);
                            this.settings.update(cx, |v, cx| v.show_page(page, cx));
                        }
                    }
                }
                // "Present" in the Show workspace asks for the output window.
                let want = app.update(cx, |s, _| std::mem::take(&mut s.want_output));
                if want && app.read(cx).output_window.is_none() {
                    this.toggle_output(cx);
                }
                let close = app.update(cx, |s, _| std::mem::take(&mut s.want_close_output));
                if close && app.read(cx).output_window.is_some() {
                    this.toggle_output(cx);
                }
                // Report a bug: screenshot the app, then open the form.
                if let Some(new) = app.update(cx, |s, _| s.bug_capture.take()) {
                    this.capture_for_bug_report(new, cx);
                }
                cx.notify();
            }),
            // Closing the main window ends the app (and the output window with it).
            cx.on_release(|_, cx| {
                crate::ops::mark_clean_exit();
                cx.quit()
            }),
        ];
        Self {
            app,
            frames,
            map,
            severe,
            winter,
            graphics,
            show,
            data,
            media,
            settings,
            community,
            forecast_editor,
            desk,
            history,
            viewers,
            publishing,
            eoc,
            workflows,
            lab,
            themes,
            about,
            bug_report,
            setup_wizard,
            dev_logs,
            dev_open: false,
            last_modal: Modal::None,
            focus,
            _subscriptions: subscriptions,
        }
    }

    fn open_modal(&mut self, modal: Modal, window: &mut Window, cx: &mut Context<Self>) {
        if modal == Modal::Settings {
            self.settings.update(cx, |v, cx| v.load(window, cx));
        }
        if modal == Modal::Community {
            self.community.update(cx, |v, cx| v.load(cx));
        }
        if modal == Modal::Setup {
            // Start from the settings as they are now.
            let app = self.app.clone();
            self.setup_wizard = cx.new(|cx| crate::ui::setup_wizard::SetupWizardView::new(app, window, cx));
        }
        self.app.update(cx, |s, cx| {
            s.modal = modal;
            cx.notify();
        });
    }

    /// Screenshot the app window for a bug report once menus and the form have closed, then
    /// open the form (`new` starts a fresh report; otherwise the shot is added to the open one).
    fn capture_for_bug_report(&mut self, new: bool, cx: &mut Context<Self>) {
        cx.notify();
        cx.spawn(async move |this, cx| {
            // Let the frame without the menu / form reach the screen first.
            cx.background_executor().timer(std::time::Duration::from_millis(if new { 300 } else { 500 })).await;
            let shot = cx
                .background_executor()
                .spawn(async move {
                    // A new report starts clean: drop screenshots from an unsaved one first.
                    if new {
                        crate::bugreport::clear_pending();
                    }
                    crate::bugreport::capture_pending().map_err(|e| e.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.bug_report.update(cx, |view, cx| if new { view.begin(shot, cx) } else { view.add(shot, cx) });
                this.app.update(cx, |s, cx| {
                    s.modal = Modal::BugReport;
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn toggle_output(&mut self, cx: &mut Context<Self>) {
        let existing = self.app.read(cx).output_window;
        if let Some(handle) = existing {
            let closed = handle.update(cx, |_, window, _| window.remove_window()).is_ok();
            self.app.update(cx, |s, cx| {
                s.output_window = None;
                cx.notify();
            });
            if closed {
                return;
            }
        }
        let app = self.app.clone();
        let frames = self.frames.clone();
        let on_top = self.app.read(cx).settings.output_always_on_top;
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some("OpenWX • Program Output".into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1280.), px(720.)), cx))),
            kind: if on_top { WindowKind::Floating } else { WindowKind::Normal },
            app_id: Some("openwx-broadcaster-output".into()),
            ..Default::default()
        };
        match gpui_kit::open_window(options, cx, move |window, cx| cx.new(|cx| OutputView::new(app, frames, window, cx))) {
            Ok((handle, _)) => self.app.update(cx, |s, cx| {
                s.output_window = Some(handle);
                s.toast(ToastKind::Info, "Program output window open - press F there for fullscreen");
                cx.notify();
            }),
            Err(e) => self.app.update(cx, |s, cx| {
                s.toast(ToastKind::Error, format!("Could not open output window: {e}"));
                cx.notify();
            }),
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        let ctrl = ks.modifiers.control;
        if ctrl && ks.modifiers.shift && ks.key.eq_ignore_ascii_case("d") {
            self.dev_open = !self.dev_open;
            cx.notify();
            return;
        }
        if ctrl && ks.modifiers.shift && ks.key.eq_ignore_ascii_case("b") {
            self.app.update(cx, |s, cx| {
                s.report_bug();
                cx.notify();
            });
            return;
        }
        match (ctrl, ks.key.as_str()) {
            (true, "1") => self.set_workspace(Workspace::Map, cx),
            (true, "2") => self.set_workspace(Workspace::Graphics, cx),
            (true, "3") => self.set_workspace(Workspace::Show, cx),
            (true, "4") => self.set_workspace(Workspace::Data, cx),
            (true, "5") => self.set_workspace(Workspace::Media, cx),
            (true, "6") => self.set_workspace(Workspace::Severe, cx),
            (true, "7") => self.set_workspace(Workspace::Winter, cx),
            (true, ",") => self.open_modal(Modal::Settings, window, cx),
            (true, "r") => self.app.update(cx, |s, cx| {
                s.toggle_recording();
                cx.notify();
            }),
            (true, "b") => self.app.update(cx, |s, cx| {
                let b = !s.black;
                s.set_black(b);
                cx.notify();
            }),
            (false, "f10") => self.toggle_output(cx),
            (false, "f5") => self.app.update(cx, |s, cx| {
                s.refresh_radar(cx);
                cx.notify();
            }),
            (false, "escape") => self.app.update(cx, |s, cx| {
                s.modal = Modal::None;
                cx.notify();
            }),
            _ => {}
        }
    }

    fn set_workspace(&mut self, workspace: Workspace, cx: &mut Context<Self>) {
        self.app.update(cx, |s, cx| {
            s.workspace = workspace;
            cx.notify();
        });
    }

    fn title_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let workspace = state.workspace;
        let recording = state.recording.as_ref().map(|r| r.started.elapsed().as_secs());
        let black = state.black;
        let output_open = state.output_window.is_some();
        let update_ready = matches!(state.update, UpdateState::Available(_) | UpdateState::Ready { .. });

        // The window controls (min/max/close, 3 x 34 px) sit to the right of this content and the
        // title bar does not shrink its content, so size it to exactly what is left; anything
        // that does not fit is dropped or clipped instead of pushing the close button off-screen.
        let viewport_w = f32::from(window.viewport_size().width);
        let content_w = (viewport_w - TITLE_BAR_PADDING - WINDOW_CONTROLS_W).max(200.0);
        let wide = content_w >= 1500.0;
        let medium = content_w >= 1150.0;
        let clock_format = match (state.settings.clock_24h, wide) {
            (true, true) => "%H:%M:%S",
            (true, false) => "%H:%M",
            (false, true) => "%-I:%M:%S %p",
            (false, false) => "%-I:%M %p",
        };
        let clock = chrono::Local::now().format(clock_format).to_string();

        let tabs = Workspace::ALL
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let w = *w;
                let active = workspace == w;
                div()
                    .id(("ws", i))
                    .h_full()
                    .px(px(if wide { 14. } else { 9. }))
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .cursor_pointer()
                    .text_size(px(13.))
                    .border_b_2()
                    .border_color(if active { ca(CYAN, 1.0) } else { ca(CYAN, 0.0) })
                    .text_color(if active { c(0xFFFFFF) } else { c(MUTED) })
                    .hover(|s| s.text_color(c(TEXT)))
                    .child(if wide { w.label() } else { w.short_label() })
                    // Buttons sit inside the title bar's window-drag area; occluding keeps Windows
                    // from treating a click on them as a click on the caption.
                    .occlude()
                    .on_click(cx.listener(move |this, _, _, cx| this.set_workspace(w, cx)))
            })
            .collect::<Vec<_>>();

        TitleBar::new()
            .bg(c(0x070A10))
            .border_b_1()
            .border_color(c(BORDER))
            .child(
                row()
                    .h_full()
                    .w(px(content_w))
                    .flex_shrink_0()
                    .overflow_hidden()
                    .gap(px(if medium { 12. } else { 8. }))
                    .pl(px(6.))
                    .pr(px(8.))
                    .child(
                        row()
                            .flex_shrink_0()
                            .gap(px(8.))
                            .child(
                                row()
                                    .font_family(DISPLAY_FONT)
                                    .font_weight(FontWeight::BLACK)
                                    .text_size(px(17.))
                                    .child(div().text_color(c(0xFEFEFE)).child("OPEN"))
                                    .child(div().text_color(c(crate::ui::BRAND)).child("WX")),
                            )
                            .when(wide, |d| {
                                d.child(
                                    div()
                                        .font_family(DISPLAY_FONT)
                                        .text_size(px(11.))
                                        .text_color(c(0x9AA0AA))
                                        .child("BROADCASTING"),
                                )
                            }),
                    )
                    .child(row().h_full().flex_1().min_w_0().overflow_hidden().children(tabs))
                    .child(
                        row()
                            .flex_shrink_0()
                            .gap(px(6.))
                            .child(
                                button_small("rec", match recording {
                                    Some(secs) => format!("● REC {}:{:02}", secs / 60, secs % 60),
                                    None => "● REC".to_string(),
                                })
                                .when(recording.is_some(), |b| b.bg(c(RED)).border_color(c(RED)).text_color(c(0xFFFFFF)))
                                .occlude()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.app.update(cx, |s, cx| {
                                        s.toggle_recording();
                                        cx.notify();
                                    });
                                })),
                            )
                            .child(
                                button_small("output", if output_open { "▣ OUTPUT ON" } else { "▣ OUTPUT" })
                                    .when(output_open, |b| b.border_color(c(GREEN)).text_color(c(GREEN)))
                                    .occlude()
                                    .on_click(cx.listener(|this, _, _, cx| this.toggle_output(cx))),
                            )
                            .child(
                                button_small("black", if black { "■ BLACK ON" } else { "BLACK" })
                                    .when(black, |b| b.bg(c(0x3A2A05)).border_color(c(AMBER)).text_color(c(AMBER)))
                                    .occlude()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.app.update(cx, |s, cx| {
                                            let b = !s.black;
                                            s.set_black(b);
                                            cx.notify();
                                        });
                                    })),
                            )
                            .when(medium, |d| {
                                d.child(
                                    div()
                                        .font_family(DISPLAY_FONT)
                                        .text_size(px(15.))
                                        .text_color(c(TEXT))
                                        .px(px(6.))
                                        .child(clock),
                                )
                            })
                            .when(update_ready, |d| {
                                d.child(
                                    button_small("update-badge", "⬆ Update")
                                        .border_color(c(CYAN))
                                        .text_color(c(CYAN))
                                        .occlude()
                                        .on_click(cx.listener(|this, _, window, cx| this.open_modal(Modal::Updates, window, cx))),
                                )
                            })
                            .child(
                                button_small("branding", if medium { "Branding & sponsors" } else { "Brand" })
                                    .occlude()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.settings.update(cx, |v, cx| v.show_branding(cx));
                                        this.open_modal(Modal::Settings, window, cx);
                                    })),
                            )
                            .child(
                                // Report a bug (same as Ctrl+Shift+B): screenshots the app, then opens the form.
                                button_small("report-bug", if medium { "Report a bug" } else { "Bug" })
                                    .border_color(c(AMBER))
                                    .text_color(c(AMBER))
                                    .occlude()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.app.update(cx, |s, cx| {
                                            s.report_bug();
                                            cx.notify();
                                        });
                                    })),
                            )
                            .child(
                                button_small("settings", if medium { "⚙ Settings" } else { "⚙" })
                                    .occlude()
                                    .on_click(cx.listener(|this, _, window, cx| this.open_modal(Modal::Settings, window, cx))),
                            )
                            .when(wide, |d| {
                                d.child(
                                    button_small("whatsnew", "What's new")
                                        .occlude()
                                        .on_click(cx.listener(|this, _, window, cx| this.open_modal(Modal::Changelog, window, cx))),
                                )
                            })
                            .child(
                                button_small("about", if medium { "About" } else { "?" })
                                    .occlude()
                                    .on_click(cx.listener(|this, _, window, cx| this.open_modal(Modal::About, window, cx))),
                            ),
                    ),
            )
    }

    fn status_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let updated = state
            .data
            .updated_at
            .map(|t| format!("Data {}", t.format("%-I:%M %p")))
            .unwrap_or_else(|| "Loading data…".into());
        let radar_age = state
            .data
            .radar
            .as_ref()
            .and_then(|r| r.latest_past())
            .map(|f| {
                let minutes = (chrono::Utc::now().timestamp() - f.time) / 60;
                format!("Radar {minutes} min old")
            })
            .unwrap_or_else(|| "Radar —".into());
        let engine = state.engine.status();
        let alerts = state.data.local_alerts.len();
        let map_alerts = state.data.map_alerts.len();
        let show = format!(
            "{} • {} scenes{}",
            state.show.name,
            state.show.scenes.len(),
            state.show_pos.map(|p| format!(" • on scene {}", p + 1)).unwrap_or_default()
        );
        let refreshing = state.refreshing;

        row()
            .h(px(26.))
            .flex_shrink_0()
            .overflow_hidden()
            .w_full()
            .px(px(12.))
            .gap(px(16.))
            .bg(c(0x070A10))
            .border_t_1()
            .border_color(c(BORDER))
            .text_size(px(11.))
            .text_color(c(MUTED))
            .child(
                row()
                    .gap(px(6.))
                    .child(div().w(px(7.)).h(px(7.)).rounded(px(4.)).bg(if state.health.radar.ok { c(GREEN) } else { c(AMBER) }))
                    .child(if refreshing { "Refreshing…".to_string() } else { updated }),
            )
            .child(div().child(radar_age))
            .child(div().child(if engine.ready { format!("MapLibre {:.0} fps", engine.fps) } else { "MapLibre starting".into() }))
            .child(div().text_color(if alerts > 0 { c(RED) } else { c(MUTED) }).child(format!("{alerts} local alerts • {map_alerts} on map")))
            .child(div().child(show))
            .child(div().flex_1())
            .child(
                div()
                    .id("readiness-open")
                    .px(px(8.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .hover(|s| s.text_color(c(TEXT)))
                    .child("Ready for air?")
                    .on_click(cx.listener(|this, _, window, cx| this.open_modal(Modal::Readiness, window, cx))),
            )
            .child({
                let (errors, warnings) = crate::devlog::counts();
                let color = if errors > 0 { RED } else if warnings > 0 { AMBER } else { MUTED };
                div()
                    .id("devlog-open")
                    .px(px(8.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_color(c(color))
                    .when(self.dev_open, |d| d.bg(ca(CYAN, 0.18)).text_color(c(CYAN)))
                    .hover(|s| s.bg(ca(0x2A3449, 0.8)))
                    .child(if errors + warnings > 0 {
                        format!("⚠ {errors} error{} • {warnings} warning{} • Dev logs", if errors == 1 { "" } else { "s" }, if warnings == 1 { "" } else { "s" })
                    } else {
                        "Dev logs".to_string()
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.dev_open = !this.dev_open;
                        cx.notify();
                    }))
            })
            .child(div().child(state.settings.api_base.clone()))
            .child(div().text_color(c(MUTED)).child(format!("v{APP_VERSION} • build {}", crate::config::APP_BUILD)))
    }

    fn modal_layer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let modal = self.app.read(cx).modal;
        if modal == Modal::Sponsors && self.last_modal != modal {
            self.settings.update(cx, |view, cx| view.load(window, cx));
        }
        self.last_modal = modal;
        if modal == Modal::None {
            return None;
        }
        // The six tools share one size and spot (they opened at random-looking sizes).
        const TOOL_W: f32 = 1180.;
        let is_tool = matches!(modal, Modal::Desk | Modal::Viewers | Modal::Publishing | Modal::Eoc | Modal::Workflows | Modal::Lab);
        let (title, width) = match modal {
            Modal::Settings => ("Settings", 1080.),
            Modal::Sponsors => ("Sponsor settings", 760.),
            Modal::About => ("About OpenWX Broadcasting", 760.),
            Modal::Setup => ("Set up OpenWX Broadcasting", 860.),
            Modal::Changelog => ("What's new", 760.),
            Modal::Updates => ("Software updates", 760.),
            Modal::Readiness => ("Ready for air?", 720.),
            Modal::Community => ("Community board", 760.),
            Modal::ForecastEditor => ("Forecast editor", 980.),
            Modal::DataSources => ("Data sources", 980.),
            Modal::Desk => ("Forecaster desk", TOOL_W),
            Modal::History => ("Event history", 1080.),
            Modal::Viewers => ("Viewer submissions", TOOL_W),
            Modal::Publishing => ("Publishing", TOOL_W),
            Modal::Eoc => ("Emergency operations", TOOL_W),
            Modal::Workflows => ("Workflow automation", TOOL_W),
            Modal::Lab => ("Science lab", TOOL_W),
            Modal::Themes => ("Map themes", 1180.),
            Modal::BugReport => ("Report a bug", 760.),
            Modal::None => ("", 600.),
        };
        let body: AnyElement = match modal {
            Modal::Settings | Modal::Sponsors => self.settings.clone().into_any_element(),
            Modal::Community => self.community.clone().into_any_element(),
            Modal::ForecastEditor => self.forecast_editor.clone().into_any_element(),
            Modal::Desk => self.desk.clone().into_any_element(),
            Modal::History => self.history.clone().into_any_element(),
            Modal::Viewers => self.viewers.clone().into_any_element(),
            Modal::Publishing => self.publishing.clone().into_any_element(),
            Modal::Eoc => self.eoc.clone().into_any_element(),
            Modal::Workflows => self.workflows.clone().into_any_element(),
            Modal::Lab => self.lab.clone().into_any_element(),
            Modal::Themes => self.themes.clone().into_any_element(),
            Modal::BugReport => self.bug_report.clone().into_any_element(),
            Modal::Setup => self.setup_wizard.clone().into_any_element(),
            _ => self.about.clone().into_any_element(),
        };
        Some(
            div()
                .id("modal-scrim")
                .absolute()
                .inset_0()
                .bg(rgba(0x02040AC0))
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll(|_, _, cx| cx.stop_propagation())
                .occlude()
                .child(
                    col()
                        .id("modal")
                        .w(px(width))
                        .max_w(relative(0.96))
                        .max_h(relative(0.88))
                        // Settings and the six tools open at one fixed size, so they don't land in different spots (CWXB-70).
                        .when(modal == Modal::Settings || is_tool, |d| d.h(relative(0.88)))
                        .bg(c(PANEL))
                        .border_1()
                        .border_color(c(BORDER))
                        .rounded(px(10.))
                        .shadow_lg()
                        .child(
                            row()
                                .px(px(18.))
                                .py(px(12.))
                                .border_b_1()
                                .border_color(c(BORDER))
                                .child(div().font_family(DISPLAY_FONT).text_size(px(20.)).text_color(c(TEXT)).child(title))
                                .child(div().flex_1())
                                .when(!matches!(modal, Modal::Settings | Modal::Sponsors | Modal::Readiness | Modal::Community | Modal::ForecastEditor | Modal::DataSources | Modal::Desk | Modal::History | Modal::Viewers | Modal::Publishing | Modal::Eoc | Modal::Workflows | Modal::Lab | Modal::Themes | Modal::BugReport), |d| {
                                    d.child(button_small("to-updates", "Updates").on_click(cx.listener(|this, _, window, cx| this.open_modal(Modal::Updates, window, cx))))
                                        .child(button_small("to-changelog", "What's new").on_click(cx.listener(|this, _, window, cx| this.open_modal(Modal::Changelog, window, cx))))
                                })
                                .child(button_small("close-modal", "✕").on_click(cx.listener(|this, _, _, cx| {
                                    this.app.update(cx, |s, cx| {
                                        s.modal = Modal::None;
                                        cx.notify();
                                    });
                                }))),
                        )
                        .child(div().id("modal-body").w_full().min_w_0().p(px(18.))
                            .when(modal == Modal::Settings, |d| d.flex().flex_col().flex_1().min_h_0())
                            .when(modal != Modal::Settings, |d| d.overflow_y_scroll())
                            .when(is_tool, |d| d.flex_1().min_h_0())
                            .child(body)),
                ),
        )
    }

    fn toasts(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let toasts = self.app.read(cx).toasts.clone();
        col()
            .absolute()
            .bottom(px(40.))
            .right(px(16.))
            .gap(px(8.))
            .w(px(380.))
            .children(toasts.into_iter().map(|t| {
                let color = match t.kind {
                    ToastKind::Info => CYAN,
                    ToastKind::Success => GREEN,
                    ToastKind::Error => RED,
                };
                let id = t.id;
                row()
                    .id(("toast", id as usize))
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(10.))
                    .rounded(px(8.))
                    .bg(c(0x111724))
                    .border_1()
                    .border_color(ca(color, 0.6))
                    .border_l_4()
                    .shadow_lg()
                    .cursor_pointer()
                    // min_w_0 lets the text box narrower than its longest word so it wraps
                    // inside the card instead of running off it.
                    .items_start()
                    .overflow_hidden()
                    .child(div().flex_1().min_w_0().overflow_hidden().whitespace_normal().text_size(px(13.)).line_height(px(18.)).text_color(c(TEXT)).child({
                        // Very long messages (paths, errors) are shortened on screen; the
                        // log keeps the full text.
                        let mut text: String = t.text.chars().take(240).collect();
                        if t.text.chars().count() > 240 {
                            text.push('…');
                        }
                        text
                    }))
                    .child(div().flex_shrink_0().text_size(px(12.)).text_color(c(MUTED)).child("✕"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.dismiss_toast(id);
                            cx.notify();
                        });
                    }))
            }))
    }
}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = self.app.read(cx).workspace;
        let title_bar = self.title_bar(window, cx);
        let status = self.status_bar(cx);
        let modal = self.modal_layer(window, cx);
        let toasts = self.toasts(cx);
        let body: AnyElement = match workspace {
            Workspace::Map => self.map.clone().into_any_element(),
            Workspace::Severe => self.severe.clone().into_any_element(),
            Workspace::Winter => self.winter.clone().into_any_element(),
            Workspace::Graphics => self.graphics.clone().into_any_element(),
            Workspace::Show => self.show.clone().into_any_element(),
            Workspace::Data => self.data.clone().into_any_element(),
            Workspace::Media => self.media.clone().into_any_element(),
        };

        col()
            .size_full()
            .relative()
            .bg(c(BG))
            .font_family(UI_FONT)
            .text_color(c(TEXT))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .child(title_bar)
            .child(div().flex_1().min_h_0().w_full().child(body))
            .when(self.dev_open, |d| d.child(self.dev_logs.clone()))
            .child(status)
            .child(toasts)
            .children(modal)
    }
}
