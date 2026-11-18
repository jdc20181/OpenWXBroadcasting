//! Severe Weather Center: the live-coverage command post. One-click modes, the warning queue
//! (most dangerous and soonest-expiring first) with towns, motion and countdowns, one-click
//! zoom / call-out / storm track / lower third / full-screen per warning, safety cards, storm
//! tracks, storm reports, data health and the event log.

use std::rc::Rc;

use gpui_kit::{prelude::FluentBuilder as _, *};

use crate::{
    app::{AppState, FrameStore, SevereMode, Workspace},
    map::geo::{compass, haversine_km, KM_PER_MILE},
    show::Transition,
    ui::{
        badge, button, button_danger, button_primary, button_small, c, col,
        frame_view::{frame_canvas, ImageSlot},
        heading, hex_color, label, muted, row, section, toggle, AMBER, BG, BORDER, CYAN, GREEN, MUTED, PANEL, PANEL_2,
        RED, TEXT,
    },
};

fn group_thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

pub struct SevereCenter {
    app: Entity<AppState>,
    frames: Entity<FrameStore>,
    slot: Rc<ImageSlot>,
    /// An emergency mode waiting for its confirming second click (CWXB-50).
    armed: Option<(SevereMode, std::time::Instant)>,
    _subscriptions: Vec<Subscription>,
}

impl SevereCenter {
    pub fn new(app: Entity<AppState>, frames: Entity<FrameStore>, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.observe(&app, |_, _, cx| cx.notify()), cx.observe(&frames, |_, _, cx| cx.notify())];
        Self {
            app,
            frames,
            armed: None,
            slot: ImageSlot::new(),
            _subscriptions: subscriptions,
        }
    }

    fn act(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut AppState, &mut Context<AppState>)) {
        self.app.update(cx, |s, cx| {
            f(s, cx);
            cx.notify();
        });
    }

    fn mode_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_tests = !self.app.read(cx).test_alerts.is_empty();
        let state = self.app.read(cx);
        let mode = state.severe_mode;
        let radar_age = state.radar_age_minutes();
        let health = state.health.clone();
        let obs = state.settings.obs_enabled;
        let lightning = state.lightning_near();
        let buses = state.ext.buses.clone();
        let studio = state.settings.obs_studio_mode;
        // Emergency modes change the on-air look and sound the alert tone: they take a second
        // click within 5 seconds, so a slip next to "Severe Weather Mode" can't trigger one.
        let armed = self.armed.filter(|(_, at)| at.elapsed() < std::time::Duration::from_secs(5)).map(|(m, _)| m);
        let mode_button = |id: &'static str, text: &'static str, m: SevereMode, cx: &mut Context<Self>| {
            let active = mode == m;
            let emergency = m != SevereMode::Severe;
            let confirming = emergency && armed == Some(m) && !active;
            let label: SharedString = if confirming { format!("Click again: {text}").into() } else { text.into() };
            let b = if m == SevereMode::Severe { button_primary(id, label) } else { button_danger(id, label) };
            b.h(px(40.))
                .px(px(16.))
                .when(active || confirming, |b| b.border_2().border_color(c(0xFFFFFF)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    let armed_now = this.armed.filter(|(_, at)| at.elapsed() < std::time::Duration::from_secs(5)).map(|(m, _)| m);
                    if emergency && !active && armed_now != Some(m) {
                        this.armed = Some((m, std::time::Instant::now()));
                        // Drop the "Click again" label once the 5 seconds are up.
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(std::time::Duration::from_secs(5)).await;
                            let _ = this.update(cx, |_, cx| cx.notify());
                        })
                        .detach();
                        cx.notify();
                        return;
                    }
                    this.armed = None;
                    this.act(cx, |s, cx| s.set_severe_mode(m, cx));
                }))
        };
        let stale = radar_age.map(|m| m > 15).unwrap_or(true);
        let feed = |name: &str, ok: bool| {
            row()
                .gap(px(5.))
                .child(div().w(px(8.)).h(px(8.)).rounded(px(4.)).bg(c(if ok { GREEN } else { RED })))
                .child(div().text_size(px(12.)).text_color(c(TEXT)).child(name.to_string()))
        };

        col()
            .gap(px(10.))
            .p(px(14.))
            .bg(c(PANEL))
            .border_b_1()
            .border_color(c(BORDER))
            .child(
                row()
                    .gap(px(10.))
                    .flex_wrap()
                    .child(mode_button("mode-severe", "Severe Weather Mode", SevereMode::Severe, cx))
                    // A little room between the everyday mode and the emergencies.
                    .child(div().w(px(14.)))
                    .child(mode_button("mode-tor", "Tornado Emergency", SevereMode::TornadoEmergency, cx))
                    .child(mode_button("mode-ffe", "Flash Flood Emergency", SevereMode::FlashFloodEmergency, cx))
                    .child(
                        button("mode-off", "Return to normal")
                            .h(px(40.))
                            .on_click(cx.listener(|this, _, _, cx| this.act(cx, |s, cx| s.set_severe_mode(SevereMode::Off, cx)))),
                    )
                    .child(
                        button("rehearse", "Rehearse: practice warning")
                            .h(px(40.))
                            .on_click(cx.listener(|this, _, _, cx| this.act(cx, |s, _| s.start_test_warning()))),
                    )
                    .when(has_tests, |d| {
                        d.child(
                            button("clear-tests", "Clear practice warnings")
                                .h(px(40.))
                                .on_click(cx.listener(|this, _, _, cx| this.act(cx, |s, _| s.clear_test_warnings()))),
                        )
                    })
                    .child(
                        button("ready-check", "Ready for air?")
                            .h(px(40.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.modal = crate::app::Modal::Readiness;
                                    cx.notify();
                                });
                            })),
                    )
                    .child(
                        button("go-home", "Home area")
                            .h(px(40.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.act(cx, |s, _| {
                                    let home = s.settings.home;
                                    s.fly_to(crate::config::GeoView { zoom: 8.0, ..home }, 1.4);
                                })
                            })),
                    )
                    .child(
                        button("go-map", "Open Map Studio")
                            .h(px(40.))
                            .on_click(cx.listener(|this, _, _, cx| this.act(cx, |s, _| s.workspace = Workspace::Map))),
                    ),
            )
            .child(
                row()
                    .gap(px(16.))
                    .flex_wrap()
                    .child(badge(mode.label(), if mode == SevereMode::Off { MUTED } else { RED }))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(c(if stale { RED } else { MUTED }))
                            .child(match radar_age {
                                Some(m) if m > 15 => format!("RADAR STALE - {m} min old"),
                                Some(m) => format!("Radar {m} min old"),
                                None => "Radar: no data".into(),
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(c(if lightning.map(|l| l.all_clear_in > 0).unwrap_or(false) { AMBER } else { MUTED }))
                            .child(match lightning {
                                Some(l) => format!("{} • {}", l.summary(), l.all_clear_text()),
                                None => format!("No lightning within {:.0} mi", crate::data::LIGHTNING_RADIUS_MI),
                            }),
                    )
                    .child(feed("Alerts", health.alerts.ok))
                    .child(feed("Radar", health.radar.ok))
                    .child(feed("Reports", health.reports.ok))
                    .child(feed("Lightning", health.lightning.ok || health.lightning.message.is_empty()))
                    .child(feed("Roads", health.roads.ok))
                    .when_some(buses.clone(), |d, (program, preview)| {
                        d.child(badge(format!("Program: {program}"), RED))
                            .when(!preview.is_empty(), |d| d.child(badge(format!("Preview: {preview}"), GREEN)))
                    })
                    .when(obs && studio, |d| {
                        d.child(button_small("obs-trans", "OBS transition").on_click(cx.listener(|this, _, _, cx| {
                            this.act(cx, |s, cx| s.obs_transition(cx));
                        })))
                    })
                    .when(obs, |d| {
                        d.child(button_small("obs-severe", "OBS → severe scene").on_click(cx.listener(|this, _, _, cx| {
                            this.act(cx, |s, cx| {
                                let scene = s.settings.obs_severe_scene.clone();
                                s.obs_switch(scene, cx);
                            })
                        })))
                        .child(button_small("obs-normal", "OBS → normal scene").on_click(cx.listener(|this, _, _, cx| {
                            this.act(cx, |s, cx| {
                                let scene = s.settings.obs_normal_scene.clone();
                                s.obs_switch(scene, cx);
                            })
                        })))
                    }),
            )
    }

    fn queue(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let warnings = state.coverage_warnings();
        let selected = state.live.selected_alert.clone();
        let coverage = state.settings.coverage_miles;
        let home = state.settings.home;

        let cards = warnings
            .into_iter()
            .enumerate()
            .map(|(i, a)| {
                let id = a.id.clone();
                let color = hex_color(a.color());
                let minutes = a.minutes_left().unwrap_or(0).max(0);
                let until = a.expires_label(false).unwrap_or_default();
                let left = a.time_left_label().unwrap_or_else(|| "ended".into());
                let towns: Vec<String> = a.towns().into_iter().take(12).collect();
                let population = a.population();
                let motion = a.motion();
                let active = selected.as_deref() == Some(a.id.as_str());
                let dist = a
                    .centroid()
                    .map(|(la, lo)| haversine_km(home.lat, home.lon, la, lo) / KM_PER_MILE);
                let (id1, id2, id3, id4, id5) = (id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
                let (id6, id7) = (id5.clone(), id5.clone());
                let (id8, id9) = (id5.clone(), id5.clone());
                let hm = |t: i64| {
                    chrono::DateTime::from_timestamp(t, 0)
                        .map(|d| d.with_timezone(&chrono::Local).format("%-I:%M %p").to_string())
                        .unwrap_or_default()
                };
                let ack = state.is_acknowledged(&a.id);
                let aired = state.ops2.status.aired.get(&a.id).copied();
                let assigned = state.ops2.status.assigned.get(&a.id).cloned();
                let me = state.settings.operator_name.clone();
                let status_line = {
                    let mut parts = Vec::new();
                    match &ack {
                        Some((t, who)) => parts.push(format!("Acknowledged by {who} at {}", hm(*t))),
                        None => parts.push("Not acknowledged".to_string()),
                    }
                    if let Some(who) = &assigned {
                        parts.push(format!("Covering: {who}"));
                    }
                    match aired {
                        Some(t) => parts.push(format!("On air since {}", hm(t))),
                        None => parts.push("Not on air yet".to_string()),
                    }
                    parts.join(" • ")
                };
                let acked = ack.is_some();
                let mine = assigned.as_deref() == Some(me.as_str());
                col()
                    .id(("warn-card", i))
                    .gap(px(6.))
                    .p(px(12.))
                    .rounded(px(8.))
                    .bg(if active { c(0x163A47) } else { c(PANEL_2) })
                    .border_l_4()
                    .border_color(color)
                    .child(
                        row()
                            .gap(px(10.))
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c(TEXT))
                                    .child(a.event.to_uppercase()),
                            )
                            .when_some(a.threat_tag(), |d, tag| d.child(badge(tag, 0xC03CE6)))
                            .when(a.message_type.eq_ignore_ascii_case("Update"), |d| d.child(badge("UPDATED", AMBER)))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .text_size(px(26.))
                                    .font_weight(FontWeight::BLACK)
                                    .text_color(c(if minutes <= 10 { RED } else { AMBER }))
                                    .child(left),
                            ),
                    )
                    .child(muted(format!(
                        "{}{}{}{}",
                        capitalize(&until),
                        if population > 0 { format!(" • ~{} people", group_thousands(population)) } else { String::new() },
                        dist.map(|d| format!(" • {d:.0} mi from home")).unwrap_or_default(),
                        motion
                            .map(|m| format!(" • moving {} at {:.0} mph", compass(m.heading_deg), m.speed_mph))
                            .unwrap_or_default()
                    )))
                    .child(div().text_size(px(12.)).text_color(c(if acked { MUTED } else { AMBER })).child(status_line))
                    .child(
                        div().text_size(px(13.)).text_color(c(TEXT)).child(if towns.is_empty() {
                            a.area_desc.clone()
                        } else {
                            format!("Including {}", towns.join(", "))
                        }),
                    )
                    .child(
                        row()
                            .gap(px(6.))
                            .flex_wrap()
                            .when(!acked, |d| {
                                d.child(button_small(("w-ack", i), "Acknowledge").on_click(cx.listener({
                                    let id = id8;
                                    move |this, _, _, cx| {
                                        let id = id.clone();
                                        this.act(cx, |s, _| s.acknowledge_alert(&id));
                                    }
                                })))
                            })
                            .when(!mine, |d| {
                                d.child(button_small(("w-mine", i), "I'll cover it").on_click(cx.listener({
                                    let id = id9;
                                    let me = me.clone();
                                    move |this, _, _, cx| {
                                        let (id, me) = (id.clone(), me.clone());
                                        this.act(cx, |s, _| s.assign_alert(&id, &me));
                                    }
                                })))
                            })
                            .child(button_small(("w-zoom", i), "Zoom to").on_click(cx.listener(move |this, _, _, cx| {
                                let id = id1.clone();
                                this.act(cx, |s, _| s.zoom_to_alert(&id));
                            })))
                            .child(button_small(("w-call", i), if active { "Clear call-out" } else { "Call out" }).on_click(cx.listener(move |this, _, _, cx| {
                                let id = id2.clone();
                                this.act(cx, |s, _| {
                                    if active {
                                        s.select_alert(None);
                                    } else {
                                        s.zoom_to_alert(&id);
                                        s.select_alert(Some(id));
                                        s.take_live_map(Transition::Cut);
                                    }
                                });
                            })))
                            .when(motion.is_some(), |d| {
                                d.child(button_small(("w-track", i), "Track storm").on_click(cx.listener(move |this, _, _, cx| {
                                    let id = id3.clone();
                                    this.act(cx, |s, _| s.track_alert(&id));
                                })))
                            })
                            .child(button_small(("w-l3", i), "Lower third").on_click(cx.listener(move |this, _, _, cx| {
                                let id = id4.clone();
                                this.act(cx, |s, _| s.warning_graphic(&id, false));
                            })))
                            .child(button_small(("w-full", i), "Full-screen graphic").on_click(cx.listener(move |this, _, _, cx| {
                                let id = id5.clone();
                                this.act(cx, |s, _| s.warning_graphic(&id, true));
                            })))
                            .child(button_small(("w-break", i), "Break in").on_click(cx.listener({
                                let id = id6;
                                move |this, _, _, cx| {
                                    let id = id.clone();
                                    this.act(cx, |s, _| s.break_in(&id));
                                }
                            })))
                            .child(button_small(("w-speak", i), "Read aloud").on_click(cx.listener({
                                let id = id7;
                                move |this, _, _, cx| {
                                    let id = id.clone();
                                    this.act(cx, |s, _| s.speak_alert(&id));
                                }
                            }))),
                    )
            })
            .collect::<Vec<_>>();

        let watches = state
            .data
            .map_alerts
            .iter()
            .filter(|a| a.event.to_lowercase().contains("watch") && a.contains(home.lat, home.lon))
            .map(|a| a.event.clone())
            .collect::<Vec<_>>();

        let comparison = crate::travel::storm_comparison(&state.data, &state.settings);
        let snd = crate::sounding::latest();
        let analysis = state.data.analysis.clone();
        let mut live_tracks: Vec<crate::cells::Track> = analysis.tracks.iter().filter(|t| !t.ended).cloned().collect();
        live_tracks.sort_by(|a, b| {
            let da = a.points.last().map(|p| p.dbz).unwrap_or(0.0);
            let db = b.points.last().map(|p| p.dbz).unwrap_or(0.0);
            db.total_cmp(&da)
        });
        col()
            .gap(px(10.))
            .when_some(snd, |d, s| {
                d.child(muted(format!(
                    "Sounding at home: MLCAPE {:.0} • 0-1 km SRH {:.0} • 0-6 km shear {:.0} kt • LCL {:.0} ft • STP {:.1} • SCP {:.1}",
                    s.ml.cape,
                    s.srh01,
                    s.shear06 * 1.944,
                    s.ml.lcl_m * 3.281,
                    s.stp,
                    s.scp
                )))
            })
            .when(!comparison.is_empty(), |d| {
                d.child(heading("Storm intensity by source"))
                    .children(comparison.into_iter().map(|cmp| {
                        row()
                            .gap(px(10.))
                            .py(px(4.))
                            .border_b_1()
                            .border_color(c(BORDER))
                            .child(div().w(px(200.)).text_size(px(13.)).text_color(c(TEXT)).truncate().child(cmp.name))
                            .child(div().w(px(150.)).text_size(px(12.)).text_color(c(MUTED)).child(cmp.radar))
                            .child(div().w(px(150.)).text_size(px(12.)).text_color(c(MUTED)).child(cmp.lightning))
                            .child(div().w(px(130.)).text_size(px(12.)).text_color(c(MUTED)).child(cmp.reports))
                            .child(div().flex_1().text_size(px(12.)).text_color(c(MUTED)).truncate().child(cmp.warning))
                            .child(badge(format!("{} of 4", cmp.agreement), if cmp.agreement >= 3 { RED } else if cmp.agreement == 2 { AMBER } else { GREEN }))
                    }).collect::<Vec<_>>())
            })
            .when(!live_tracks.is_empty(), |d| {
                d.child(heading("Storm tracks"))
                    .children(live_tracks.into_iter().take(6).map(|t| {
                        let last = t.points.last().cloned().unwrap_or_default();
                        let minutes = t.points.first().map(|p| (last.time - p.time) / 60).unwrap_or(0);
                        row()
                            .gap(px(10.))
                            .child(div().w(px(40.)).text_size(px(13.)).text_color(c(TEXT)).child(t.id.clone()))
                            .child(div().w(px(150.)).text_size(px(12.)).text_color(c(if t.trend_score >= 2 { RED } else if t.trend_score == 1 { AMBER } else { MUTED })).child(t.trend_label.clone()))
                            .child(div().w(px(120.)).text_size(px(12.)).text_color(c(MUTED)).child(format!("{:.0} dBZ, {} fl/2min", last.dbz, last.flashes)))
                            .child(div().w(px(110.)).text_size(px(12.)).text_color(c(MUTED)).child(format!("tracked {minutes} min")))
                            .child(div().flex_1().text_size(px(12.)).text_color(c(MUTED)).child(match t.lightning_corr {
                                Some(r) if r >= 0.5 => "Lightning rising with intensity".to_string(),
                                Some(r) if r <= -0.5 => "Lightning falling as it strengthens".to_string(),
                                Some(_) => "Lightning not tied to intensity".to_string(),
                                None => String::new(),
                            }))
                            .when(t.jump, |d| d.child(badge("LIGHTNING JUMP", AMBER)))
                    }).collect::<Vec<_>>())
            })
            .children(analysis.events.iter().rev().take(4).map(|e| {
                let when = chrono::DateTime::from_timestamp(e.time, 0).map(|t| t.with_timezone(&chrono::Local).format("%-I:%M %p").to_string()).unwrap_or_default();
                muted(format!("{when}  {}", e.text))
            }).collect::<Vec<_>>())
            .children(analysis.mcs.iter().map(|m| {
                div().text_size(px(13.)).text_color(c(AMBER)).child(format!("{} detected: {:.0} miles long, {} storms", if m.squall_line { "Squall line" } else { "Storm complex (MCS)" }, m.length_mi, m.cells))
            }).collect::<Vec<_>>())
            .child(
                row()
                    .gap(px(6.))
                    .flex_wrap()
                    .child(button_small("auto-tour", if state.tour_running() { "Stop presenting" } else { "Present all warnings" }).on_click(cx.listener(|this, _, _, cx| {
                        this.act(cx, |s, _| if s.tour_running() { s.stop_tour() } else { s.warning_tour(8.0) });
                    })))
                    .child(button_small("auto-path", "Track the strongest storm").on_click(cx.listener(|this, _, _, cx| {
                        this.act(cx, |s, _| s.auto_storm_path());
                    })))
                    .child(button_small("cone-build", "Cone from storm history").on_click(cx.listener(|this, _, _, cx| {
                        this.act(cx, |s, _| s.cone_from_history());
                    })))
                    .child(toggle("auto-zoom", "Zoom to new tornado warnings automatically", state.settings.auto_zoom_threat).on_click(cx.listener(|this, _, _, cx| {
                        this.act(cx, |s, _| {
                            let mut settings = s.settings.clone();
                            settings.auto_zoom_threat = !settings.auto_zoom_threat;
                            s.apply_settings(settings);
                        });
                    }))),
            )
            .child(heading(format!("Warning queue • within {coverage:.0} mi of home")))
            .when(!watches.is_empty(), |d| d.child(div().text_size(px(13.)).text_color(c(AMBER)).child(format!("Home is in: {}", watches.join(", ")))))
            .when(cards.is_empty(), |d| {
                d.child(
                    col()
                        .p(px(20.))
                        .rounded(px(8.))
                        .bg(c(PANEL_2))
                        .child(label("No storm warnings in the coverage area"))
                        .child(muted("New tornado, severe thunderstorm and flash flood warnings appear here with a sound. Sounds, coverage radius and automatic Severe Mode are in Settings.")),
                )
            })
            .children(cards)
    }

    fn side(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let image = self.frames.read(cx).program.clone();
        let state = self.app.read(cx);
        let cell_trends = state.cell_trends();
        let backup_on = state.ext.backup_on;
        let lightning_line = {
            let home = state.settings.home;
            let rate = crate::map::extras::flash_rate(&state.data.lightning, home.lat, home.lon, 25.0);
            let bars: String = rate
                .iter()
                .map(|n| match n {
                    0 => '▁',
                    1..=2 => '▂',
                    3..=5 => '▄',
                    6..=10 => '▆',
                    _ => '█',
                })
                .collect();
            if rate.iter().all(|n| *n == 0) {
                "No lightning within 25 mi in the last hour".to_string()
            } else {
                format!("Lightning within 25 mi, last hour (5-min steps): {bars}")
            }
        };
        let tracks: Vec<(usize, String)> = state
            .live
            .storm_track
            .iter()
            .chain(state.live.extra_tracks.iter())
            .enumerate()
            .map(|(i, t)| (i, format!("{} • {} {:.0} mph", t.label, compass(t.heading_deg), t.speed_mph)))
            .collect();
        let home = state.settings.home;
        let cutoff = chrono::Utc::now().timestamp() - 6 * 3600;
        let mut reports: Vec<crate::data::StormReport> = state
            .data
            .reports
            .iter()
            .filter(|r| r.time >= cutoff && haversine_km(home.lat, home.lon, r.lat, r.lon) < state.settings.coverage_miles * 3.0 * KM_PER_MILE)
            .cloned()
            .collect();
        reports.sort_by(|a, b| b.time.cmp(&a.time));
        let log = state.event_log.clone();
        let lightning_on = state.live.lightning;
        let reports_on = state.live.reports;
        let roads_on = state.live.roads;

        let report_rows = reports
            .into_iter()
            .take(12)
            .enumerate()
            .map(|(i, r)| {
                let (letter, rgb) = r.marker();
                let color = (rgb[0] as u32) << 16 | (rgb[1] as u32) << 8 | rgb[2] as u32;
                let when = chrono::DateTime::from_timestamp(r.time, 0)
                    .map(|t| t.with_timezone(&chrono::Local).format("%-I:%M %p").to_string())
                    .unwrap_or_default();
                let (lat, lon) = (r.lat, r.lon);
                row()
                    .id(("lsr", i))
                    .gap(px(8.))
                    .p(px(6.))
                    .rounded(px(6.))
                    .bg(c(PANEL_2))
                    .cursor_pointer()
                    .hover(|s| s.bg(c(0x232C40)))
                    .child(div().w(px(22.)).h(px(22.)).rounded(px(11.)).bg(c(color)).flex().items_center().justify_center().text_size(px(12.)).font_weight(FontWeight::BLACK).text_color(c(0x0B0F18)).child(letter))
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_size(px(12.)).text_color(c(TEXT)).truncate().child(format!("{} {} • {}", r.text, if r.magnitude.is_empty() { String::new() } else { r.magnitude.clone() }, r.city)))
                            .child(div().text_size(px(11.)).text_color(c(MUTED)).child(format!("{when} • {}", r.source))),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.act(cx, |s, _| {
                            s.update_live(|l| l.reports = true);
                            s.fly_to(crate::config::GeoView { lat, lon, zoom: 9.5 }, 1.4);
                        })
                    }))
            })
            .collect::<Vec<_>>();

        let track_rows = tracks
            .into_iter()
            .map(|(i, text)| {
                row()
                    .gap(px(6.))
                    .child(div().flex_1().text_size(px(12.)).text_color(c(TEXT)).child(text))
                    .child(button_small(("rm-track", i), "Remove").on_click(cx.listener(move |this, _, _, cx| {
                        this.act(cx, |s, _| {
                            s.update_live(|l| {
                                if i == 0 {
                                    l.storm_track = if l.extra_tracks.is_empty() { None } else { Some(l.extra_tracks.remove(0)) };
                                } else if i - 1 < l.extra_tracks.len() {
                                    l.extra_tracks.remove(i - 1);
                                }
                            })
                        })
                    })))
            })
            .collect::<Vec<_>>();

        let safety = [("tornado", "Tornado"), ("flood", "Flood"), ("thunderstorm", "Severe storm"), ("winter", "Winter"), ("heat", "Heat")]
            .into_iter()
            .enumerate()
            .map(|(i, (key, text))| {
                button_small(("safety", i), text).on_click(cx.listener(move |this, _, _, cx| this.act(cx, |s, _| s.safety_graphic(key))))
            })
            .collect::<Vec<_>>();

        let log_rows = log
            .into_iter()
            .take(40)
            .map(|e| {
                row()
                    .gap(px(8.))
                    .items_start()
                    .child(div().w(px(62.)).flex_shrink_0().text_size(px(11.)).text_color(c(MUTED)).child(e.at.format("%-I:%M:%S").to_string()))
                    .child(div().flex_1().text_size(px(12.)).text_color(c(match e.level {
                        2 => RED,
                        1 => AMBER,
                        _ => TEXT,
                    })).child(e.text))
            })
            .collect::<Vec<_>>();

        col()
            .id("severe-side")
            .w(px(440.))
            .h_full()
            .flex_shrink_0()
            .bg(c(PANEL))
            .border_l_1()
            .border_color(c(BORDER))
            .overflow_y_scroll()
            .child(
                section("Program")
                    .child(div().w_full().h(px(236.)).child(frame_canvas(image, self.slot.clone()).size_full()))
                    .child(
                        row()
                            .gap(px(6.))
                            .child(button_small("to-map", "Radar to Program").on_click(cx.listener(|this, _, _, cx| this.act(cx, |s, _| s.take_live_map(Transition::Cut)))))
                            .child(button_small("clear-marks", "Clear drawings").on_click(cx.listener(|this, _, _, cx| {
                                this.act(cx, |s, _| s.update_live(|l| l.annotations.clear()))
                            })))
                            .child(button_small("want-output", "Output window").on_click(cx.listener(|this, _, _, cx| this.act(cx, |s, _| s.want_output = true)))),
                    ),
            )
            .child(
                section("Layers")
                    .child(toggle("sc-lightning", "Lightning", lightning_on).on_click(cx.listener(|this, _, _, cx| this.act(cx, |s, _| s.update_live(|l| l.lightning = !l.lightning)))))
                    .child(toggle("sc-reports", "Storm reports", reports_on).on_click(cx.listener(|this, _, _, cx| this.act(cx, |s, _| s.update_live(|l| l.reports = !l.reports)))))
                    .child(toggle("sc-roads", "INDOT road closures & incidents", roads_on).on_click(cx.listener(|this, _, _, cx| this.act(cx, |s, _| s.update_live(|l| l.roads = !l.roads))))),
            )
            .child(section("Safety cards (full screen)").child(row().flex_wrap().gap(px(6.)).children(safety)))
            .child(
                section("Strongest storms")
                    .gap(px(4.))
                    .when(cell_trends.is_empty(), |d| d.child(muted("No storm cells tracked right now")))
                    .children(cell_trends.into_iter().enumerate().map(|(i, (summary, trend, rising))| {
                        row()
                            .id(("cell-trend", i))
                            .gap(px(8.))
                            .child(div().flex_1().min_w_0().text_size(px(12.)).text_color(c(TEXT)).child(summary))
                            .child(div().text_size(px(12.)).text_color(c(match rising {
                                Some(true) => RED,
                                Some(false) => GREEN,
                                None => MUTED,
                            })).child(trend))
                    })),
            )
            .child(
                section("Continuity and tools")
                    .child(
                        row()
                            .flex_wrap()
                            .gap(px(6.))
                            .child(button_small("backup-scene", if backup_on { "End backup scene" } else { "Backup scene (stay with us)" }).on_click(cx.listener(move |this, _, _, cx| {
                                this.act(cx, |s, _| s.set_backup_scene(!backup_on));
                            })))
                            .child(button_small("speak-fcst", "Read forecast aloud").on_click(cx.listener(|this, _, _, cx| {
                                this.act(cx, |s, _| s.speak_forecast());
                            })))
                            .child(button_small("speak-stop", "Stop reading").on_click(cx.listener(|_, _, _, _| {
                                crate::ops::stop_speaking();
                            })))
                            .child(button_small("community", "Community board").on_click(cx.listener(|this, _, _, cx| {
                                this.act(cx, |s, _| s.modal = crate::app::Modal::Community);
                            })))
                            .child(button_small("fcst-edit", "Forecast editor").on_click(cx.listener(|this, _, _, cx| {
                                this.act(cx, |s, _| {
                                    s.ext.paint = None;
                                    s.modal = crate::app::Modal::ForecastEditor;
                                });
                            }))),
                    )
                    .child(muted(lightning_line)),
            )
            .child(
                section("Storm tracks")
                    .when(track_rows.is_empty(), |d| d.child(muted("None. Use \"Track storm\" on a warning, or S / F in Map Studio.")))
                    .children(track_rows),
            )
            .child(
                section("Storm reports • last 6 h")
                    .when(report_rows.is_empty(), |d| d.child(muted("No reports nearby.")))
                    .children(report_rows),
            )
            .child(section("Event log").gap(px(4.)).children(log_rows))
    }
}

impl Render for SevereCenter {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bar = self.mode_bar(cx);
        let queue = self.queue(cx);
        let side = self.side(cx);
        row()
            .size_full()
            .items_start()
            .bg(c(BG))
            .child(
                col()
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .child(bar)
                    .child(div().id("warning-queue").flex_1().min_h_0().p(px(14.)).overflow_y_scroll().child(queue)),
            )
            .child(side)
    }
}

/// First letter upper-case ("until 4 PM EDT" -> "Until 4 PM EDT").
fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
