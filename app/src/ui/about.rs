//! About, What's New (changelog) and Updates.

use gpui_kit::{prelude::FluentBuilder as _, *};

use crate::{
    app::{AppState, Modal},
    config::{LibraryPaths, APP_VERSION},
    updater::UpdateState,
    ui::{badge, button, button_primary, c, col, heading, label, muted, row, wordmark, BORDER, CYAN, GREEN, MUTED, PANEL_2, TEXT},
};

pub const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

/// Render a small subset of Markdown (headings, bullets, paragraphs).
pub fn markdown(text: &str) -> Div {
    let mut out = col().w_full().min_w_0().gap(px(6.));
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            out = out.child(div().h(px(4.)));
        } else if let Some(h) = trimmed.strip_prefix("# ") {
            out = out.child(div().font_family("Bahnschrift").text_size(px(24.)).text_color(c(TEXT)).child(h.to_string()));
        } else if let Some(h) = trimmed.strip_prefix("## ") {
            out = out.child(
                div()
                    .mt(px(8.))
                    .font_family("Bahnschrift")
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(18.))
                    .text_color(c(CYAN))
                    .child(h.to_string()),
            );
        } else if let Some(h) = trimmed.strip_prefix("### ") {
            out = out.child(div().font_weight(FontWeight::SEMIBOLD).text_size(px(14.)).text_color(c(TEXT)).child(h.to_string()));
        } else if let Some(item) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            out = out.child(
                row()
                    .w_full()
                    .min_w_0()
                    .items_start()
                    .gap(px(8.))
                    .child(div().flex_shrink_0().text_color(c(CYAN)).child("•"))
                    .child(div().flex_1().min_w_0().whitespace_normal().text_size(px(13.)).text_color(c(TEXT)).child(item.replace("**", ""))),
            );
        } else {
            out = out.child(div().w_full().min_w_0().whitespace_normal().text_size(px(13.)).text_color(c(MUTED)).child(trimmed.replace("**", "")));
        }
    }
    out
}

pub struct AboutView {
    app: Entity<AppState>,
    _subscriptions: Vec<Subscription>,
}

impl AboutView {
    pub fn new(app: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.observe(&app, |_, _, cx| cx.notify())];
        Self {
            app,
            _subscriptions: subscriptions,
        }
    }

    fn about(&self) -> Div {
        let paths = LibraryPaths::get();
        let credit = |name: &str, what: &str| {
            row()
                .justify_between()
                .w_full()
                .py(px(4.))
                .border_b_1()
                .border_color(c(BORDER))
                .gap(px(16.))
                .child(div().flex_shrink_0().child(label(name.to_string())))
                .child(div().flex_1().min_w_0().whitespace_normal().text_right().child(muted(what.to_string())))
        };
        col()
            .gap(px(14.))
            .child(
                row()
                    .gap(px(18.))
                    .child(wordmark(46., Some("BROADCASTING")))
                    .child(div().flex_1())
                    .child(col().items_end().child(badge(format!("VERSION {}", crate::config::version_label()), CYAN)).child(muted("Community edition"))),
            )
            .child(muted("Broadcast weather radar, graphics and presentation studio for community weather stations. Not an official warning source: follow the National Weather Service for life-safety decisions."))
            .child(
                col()
                    .p(px(12.))
                    .rounded(px(8.))
                    .bg(c(PANEL_2))
                    .child(heading("Built with"))
                    .child(credit("Rust + GPUI / GPUI Kit", "Native, GPU-accelerated UI"))
                    .child(credit("maplibre-rs", "MapLibre map rendering (wgpu)"))
                    .child(credit("OpenWX Relay", "The built-in backend: NOAA/NWS, SPC, USGS, IEM, ACIS and more, fetched with your station contact"))
                    .child(credit("LibreWXR", "Radar & satellite composite, storm cells and GLM lightning (via OpenWX Relay)"))
                    .child(credit("National Weather Service", "Forecasts, observations, alerts"))
                    .child(credit("NOAA NOMADS", "GFS, HRRR, RAP and NBM model grids (GRIB2, via OpenWX Relay)"))
                    .child(credit("Storm Prediction Center", "Convective outlooks and mesoscale discussions"))
                    .child(credit("NOAA WPC • NWPS", "Rainfall outlooks, river gauges and flood stages"))
                    .child(credit("Iowa Environmental Mesonet", "Single-site NEXRAD, GOES satellite, MRMS rainfall, storm reports"))
                    .child(credit("Esri • HERE • Garmin • OpenStreetMap", "Basemaps and labels"))
                    .child(credit("tiny-skia • fontdue", "Broadcast graphics renderer"))
                    .child(credit("OpenH264", "MP4 recording (Cisco OpenH264, BSD)"))
                    .child(credit("Remix Icon", "Interface icons, remixicon.com (Apache License 2.0)"))
                    .child(credit("Meteocons", "Weather icons in the Modern graphics look, by Bas Milius, github.com/basmilius/weather-icons (MIT License)"))
                    .child(credit("NOAA NCEI", "1991-2020 station climate normals"))
                    .child(credit("Open-Meteo", "Other models (HRRR, NBM, ECMWF, ICON, GEM), air quality, climate history, place search (CC BY 4.0)")),
            )
            .child(
                col()
                    .gap(px(4.))
                    .child(heading("Library"))
                    .child(muted(paths.root.display().to_string())),
            )
    }

    fn updates(&self, cx: &mut Context<Self>) -> Div {
        let state = self.app.read(cx);
        let update = state.update.clone();
        let feed = state.settings.update_feed_for_channel();
        let older = crate::updater::previous_installers();
        let notes = match &update {
            UpdateState::Available(m) => Some(m.notes.clone()),
            _ => None,
        };
        let ready = matches!(update, UpdateState::Ready { .. });
        let available = matches!(update, UpdateState::Available(_));
        let busy = matches!(update, UpdateState::Checking | UpdateState::Downloading { .. });
        let progress = match &update {
            UpdateState::Downloading { downloaded, total: Some(total), .. } if *total > 0 => Some(*downloaded as f32 / *total as f32),
            _ => None,
        };

        col()
            .gap(px(12.))
            .child(
                row()
                    .gap(px(10.))
                    .child(badge(format!("INSTALLED {}", crate::config::version_label()), GREEN))
                    .child(label(update.summary())),
            )
            .when_some(progress, |d, p| {
                d.child(
                    div()
                        .w_full()
                        .h(px(6.))
                        .rounded(px(3.))
                        .bg(c(PANEL_2))
                        .child(div().h_full().w(relative(p)).rounded(px(3.)).bg(c(CYAN))),
                )
            })
            .child(
                row()
                    .gap(px(8.))
                    .child(button("check", if busy { "Working…" } else { "Check for updates" }).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| s.check_updates(true, cx));
                    })))
                    .when(available, |d| {
                        d.child(button_primary("download", if crate::updater::SELF_INSTALL { "Download & verify" } else { "Open download page" }).on_click(cx.listener(|this, _, _, cx| {
                            this.app.update(cx, |s, cx| s.download_update(cx));
                        })))
                        .child(button("skip", "Skip this version").on_click(cx.listener(|this, _, _, cx| {
                            this.app.update(cx, |s, cx| {
                                if let UpdateState::Available(m) = &s.update {
                                    let mut settings = s.settings.clone();
                                    settings.skipped_version = Some(m.label());
                                    s.apply_settings(settings);
                                }
                                cx.notify();
                            });
                        })))
                    })
                    .when(ready, |d| {
                        d.child(button_primary("install", "Install & restart").on_click(cx.listener(|this, _, _, cx| {
                            this.app.update(cx, |s, _| s.install_update());
                        })))
                    }),
            )
            .child(muted(format!("Feed: {feed}")))
            .when(crate::updater::SELF_INSTALL, |d| d.child(muted("Installers are verified with the OpenWX signing key and SHA-256 before they run.")))
            .when(older.len() > 1, |d| {
                // The newest staged installer is the current version; the next one is the rollback.
                let previous = older[1].clone();
                let name = previous.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                d.child(muted(format!("Previous version kept for rollback: {name}")))
                    .child(button("rollback", "Go back to the previous version").on_click(cx.listener(move |_, _, _, _| {
                        log::warn!("rolling back to {}", previous.display());
                        if let Err(e) = crate::updater::install_and_exit(&previous) {
                            log::error!("rollback failed: {e}");
                        }
                    })))
            })
            .when_some(notes, |d, notes| {
                d.child(
                    col()
                        .p(px(12.))
                        .rounded(px(8.))
                        .bg(c(PANEL_2))
                        .child(heading("Release notes"))
                        .child(markdown(&notes)),
                )
            })
    }
}

impl AboutView {
    /// Licensing registry, lineage, live quality, redistribution switches and the compliance report.
    fn data_sources(&mut self, cx: &mut Context<Self>) -> Div {
        let blocked = self.app.read(cx).settings.no_redistribute.clone();
        let rows = crate::sources::SOURCES
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let q = crate::sources::quality(s);
                let off = blocked.iter().any(|b| b == s.name);
                let name = s.name;
                col()
                    .gap(px(4.))
                    .py(px(10.))
                    .border_t_1()
                    .border_color(c(BORDER))
                    .child(
                        row()
                            .gap(px(10.))
                            .child(div().text_size(px(15.)).text_color(c(TEXT)).child(s.name))
                            .child(match &q {
                                Some((v, _)) => badge(format!("Quality {v}"), if *v >= 80 { GREEN } else { 0xE0A030 }),
                                None => badge("Not used yet", MUTED),
                            })
                            .child(div().flex_1())
                            .child(crate::ui::toggle(("src-web", i), "Allow in web exports", !off).on_click(cx.listener(move |this, _, _, cx| {
                                this.app.update(cx, |st, cx| {
                                    let mut settings = st.settings.clone();
                                    if let Some(pos) = settings.no_redistribute.iter().position(|b| b == name) {
                                        settings.no_redistribute.remove(pos);
                                    } else {
                                        settings.no_redistribute.push(name.to_string());
                                    }
                                    st.apply_settings(settings);
                                    cx.notify();
                                });
                            }))),
                    )
                    .child(muted(format!("Feeds: {}", s.feeds.join(", "))))
                    .child(muted(format!("License: {} • {}", s.license, s.redistribution)))
                    .when(!s.credit.is_empty(), |d| d.child(muted(format!("Credit: {}", s.credit))))
                    .when_some(q.map(|(_, st)| st), |d, st| {
                        d.child(muted(format!(
                            "This session: {} requests, {} failed, {} ms average{}",
                            st.requests,
                            st.failures,
                            st.avg_ms(),
                            st.last_ok
                                .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
                                .map(|t| format!(", last good {}", t.with_timezone(&chrono::Local).format("%-I:%M %p")))
                                .unwrap_or_default()
                        )))
                    })
            })
            .collect::<Vec<_>>();
        let lineage = crate::sources::SOURCES
            .iter()
            .flat_map(|s| s.feeds.iter().map(move |f| (*f, s.name)))
            .map(|(product, source)| row().gap(px(8.)).child(div().w(px(300.)).text_size(px(13.)).text_color(c(TEXT)).child(product)).child(muted(format!("← {source}"))))
            .collect::<Vec<_>>();
        col()
            .gap(px(12.))
            .child(muted("Where every product on air comes from, what its license allows, and how each source is doing this session."))
            .child(
                row()
                    .gap(px(8.))
                    .child(button_primary("src-report", "Write compliance report").on_click(cx.listener(|this, _, _, cx| {
                        let settings = this.app.read(cx).settings.clone();
                        let result = crate::sources::compliance_report(&settings);
                        this.app.update(cx, |s, cx| {
                            match result {
                                Ok(path) => {
                                    let _ = open::that(&path);
                                    s.toast(crate::app::ToastKind::Success, format!("Report saved: {}", path.display()));
                                }
                                Err(e) => s.toast(crate::app::ToastKind::Error, format!("Could not write the report: {e}")),
                            }
                            cx.notify();
                        });
                    })))
                    .child(button("src-log", "Open the request logs").on_click(cx.listener(|_, _, _, _| {
                        if let Some(dir) = crate::devlog::path().parent() {
                            let _ = open::that(dir);
                        }
                    })))
                    .child(button("src-back", "Back to Ready for air?").on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.modal = Modal::Readiness;
                            cx.notify();
                        });
                    }))),
            )
            .children(rows)
            .child(heading("Lineage: product ← source"))
            .children(lineage)
    }
}

impl Render for AboutView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let modal = self.app.read(cx).modal;
        match modal {
            // Full width, so long lines wrap inside the dialog instead of running off its sides.
            Modal::Changelog => col().w_full().min_w_0().child(markdown(CHANGELOG)),
            Modal::Updates => self.updates(cx),
            Modal::DataSources => self.data_sources(cx),
            Modal::Readiness => {
                let items = crate::ui::readiness::checks(self.app.read(cx));
                let system = crate::ui::readiness::system(self.app.read(cx));
                col()
                    .gap(px(14.))
                    .child(crate::ui::readiness::render(&items))
                    .child(heading("System"))
                    .child(crate::ui::readiness::render(&system))
                    .child(button("ready-sources", "Data sources and licenses…").on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.modal = Modal::DataSources;
                            cx.notify();
                        });
                    })))
                    .child(
                        row()
                            .gap(px(8.))
                            .child(button("ready-refresh", "Refresh radar (F5)").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.refresh_radar(cx);
                                    cx.notify();
                                });
                            })))
                            .child(button("ready-clear-tests", "Clear practice warnings").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.clear_test_warnings();
                                    cx.notify();
                                });
                            })))
                            .child(div().flex_1())
                            .child(muted("Updates live while open")),
                    )
            }
            _ => self.about(),
        }
    }
}
