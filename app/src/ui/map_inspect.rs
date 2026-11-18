//! The map's point inspector: click a spot with the Pan tool to see the nearest town, current
//! conditions there, and full details of every alert covering it, with on-air actions.
//! This card is presenter UI only; it never appears on air.

use gpui_kit::{prelude::FluentBuilder as _, *};

use crate::{
    app::AppState,
    data::{Alert, Current},
    map::geo::{bearing_deg, compass, haversine_km, KM_PER_MILE},
    ui::{button_small, c, col, hex_color, muted, row, AMBER, BORDER, MUTED, PANEL, PANEL_2, TEXT},
};

#[derive(Clone)]
pub struct Inspect {
    pub lat: f64,
    pub lon: f64,
    /// "3 mi NE of Huntingburg, IN" or the town itself when very close.
    pub place: Option<String>,
    pub alerts: Vec<Alert>,
    pub current: Option<Current>,
    pub loading: bool,
    pub error: Option<String>,
    /// Matches the fetch that is allowed to fill this card (a newer click wins).
    pub serial: u64,
    /// Ground elevation, feet (filled in after the click).
    pub elevation_ft: Option<f64>,
    /// The storm cell clicked on (when storm cells are shown), for its details.
    pub cell: Option<crate::data::StormCell>,
}

impl Inspect {
    pub fn at(lat: f64, lon: f64, map_alerts: &[Alert], serial: u64) -> Self {
        let mut alerts: Vec<Alert> = map_alerts.iter().filter(|a| a.contains(lat, lon)).cloned().collect();
        alerts.sort_by_key(|a| a.priority());
        Self { lat, lon, place: nearest_place(lat, lon), alerts, current: None, loading: true, error: None, serial, elevation_ft: None, cell: None }
    }
}

/// "3 mi NE of Huntingburg, IN" (public for saved places).
pub fn place_name(lat: f64, lon: f64) -> Option<String> {
    nearest_place(lat, lon)
}

/// Nearest named place within about 25 miles, worded from the town's point of view.
fn nearest_place(lat: f64, lon: f64) -> Option<String> {
    let d = 0.45;
    let places = crate::places::in_box(lat - d, lat + d, lon - d * 1.3, lon + d * 1.3);
    let best = places
        .iter()
        .map(|p| (haversine_km(lat, lon, p.lat, p.lon), p))
        // Prefer real towns over tiny ones when they are about as close.
        .min_by(|a, b| {
            let score = |(km, p): &(f64, &crate::places::Place)| km - (p.population as f64).log10().max(0.0) * 0.6;
            score(a).total_cmp(&score(b))
        })?;
    let (km, p) = best;
    let name = if p.state.is_empty() { p.name.clone() } else { format!("{}, {}", p.name, p.state) };
    let miles = km / KM_PER_MILE;
    Some(if miles < 1.5 {
        name
    } else {
        format!("{:.0} mi {} of {name}", miles, compass(bearing_deg(p.lat, p.lon, lat, lon)))
    })
}

fn fmt_opt(v: Option<f64>, unit: &str) -> String {
    v.map(|v| format!("{v:.0}{unit}")).unwrap_or_else(|| "—".into())
}

fn trimmed(text: &str, max: usize) -> String {
    let t = text.trim().replace("\n\n", "\n").replace('\n', " ");
    if t.chars().count() > max {
        t.chars().take(max).collect::<String>() + "…"
    } else {
        t
    }
}

/// The card. `on_close` clears it; alert buttons act on the app directly.
pub fn card<V: 'static>(
    inspect: &Inspect,
    app: &Entity<AppState>,
    clock_24h: bool,
    selected: Option<String>,
    cx: &mut Context<V>,
    on_close: impl Fn(&mut V, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let kv = |k: &str, v: String| row().justify_between().child(muted(k.to_string())).child(div().text_size(px(13.)).text_color(c(TEXT)).child(v));

    let conditions: AnyElement = match (&inspect.current, inspect.loading, &inspect.error) {
        (Some(cur), _, _) => {
            let feels = cur.heat_index_f.or(cur.wind_chill_f);
            let wind = match (cur.wind_speed_mph, cur.wind_direction_cardinal.clone()) {
                (Some(s), Some(d)) if s >= 1.0 => format!("{d} {s:.0} mph"),
                (Some(_), _) => "Calm".into(),
                _ => "—".into(),
            };
            let gust = cur.wind_gust_mph.map(|g| format!(", gusts {g:.0}")).unwrap_or_default();
            col()
                .gap(px(4.))
                .child(
                    row()
                        .items_end()
                        .gap(px(10.))
                        .child(div().text_size(px(30.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(fmt_opt(cur.temperature_f, "°")))
                        .child(
                            col()
                                .child(div().text_size(px(13.)).text_color(c(TEXT)).child(cur.text_description.clone().unwrap_or_default()))
                                .when_some(feels, |d, f| d.child(muted(format!("Feels like {f:.0}°")))),
                        ),
                )
                .child(kv("Wind", format!("{wind}{gust}")))
                .child(kv("Humidity", fmt_opt(cur.relative_humidity, "%")))
                .child(kv("Dew point", fmt_opt(cur.dewpoint_f, "°")))
                .child(kv("Pressure", cur.barometric_pressure_in_hg.map(|p| format!("{p:.2} in")).unwrap_or_else(|| "—".into())))
                .child(kv("Visibility", cur.visibility_miles.map(|v| format!("{v:.0} mi")).unwrap_or_else(|| "—".into())))
                .when_some(cur.station_label.clone().or(cur.station_identifier.clone()), |d, s| d.child(muted(format!("Observed at {s}"))))
                .into_any_element()
        }
        (None, true, _) => muted("Loading conditions…").into_any_element(),
        (None, false, Some(e)) => muted(format!("Conditions unavailable: {e}")).into_any_element(),
        (None, false, None) => muted("No observation for this spot.").into_any_element(),
    };

    let alerts = inspect
        .alerts
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let id = a.id.clone();
            let is_on = selected.as_deref() == Some(a.id.as_str());
            let motion = a.motion();
            let towns: Vec<String> = a.towns().into_iter().take(8).collect();
            let call_app = app.clone();
            let track_app = app.clone();
            let lt_app = app.clone();
            let (call_id, track_id, lt_id) = (id.clone(), id.clone(), id.clone());
            col()
                .gap(px(5.))
                .p(px(10.))
                .rounded(px(6.))
                .bg(c(PANEL_2))
                .border_l_4()
                .border_color(hex_color(a.color()))
                .child(
                    row()
                        .gap(px(8.))
                        .child(div().flex_1().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(a.event.clone()))
                        .when_some(a.time_left_label(), |d, l| d.child(div().text_size(px(12.)).text_color(c(AMBER)).child(format!("{l} left")))),
                )
                .when_some(a.threat_tag(), |d, t| d.child(div().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(c(0xD27AF0)).child(t)))
                .when_some(a.expires_label(clock_24h), |d, l| d.child(muted(format!("In effect {l}"))))
                .when_some(motion, |d, m| d.child(muted(format!("Storm moving {} at {:.0} mph", compass(m.heading_deg), m.speed_mph))))
                .when(!towns.is_empty(), |d| d.child(div().text_size(px(12.)).text_color(c(TEXT)).child(format!("Includes {}", towns.join(", ")))))
                .child(div().text_size(px(12.)).text_color(c(MUTED)).child(trimmed(&a.area_desc, 220)))
                .when(!a.description.trim().is_empty(), |d| d.child(div().text_size(px(12.)).text_color(c(TEXT)).child(trimmed(&a.description, 700))))
                .when(!a.instruction.trim().is_empty(), |d| d.child(div().text_size(px(12.)).text_color(c(AMBER)).child(trimmed(&a.instruction, 300))))
                .child(muted(format!("{} • {}", a.sender, a.severity)))
                .child(
                    row()
                        .flex_wrap()
                        .gap(px(6.))
                        .child(button_small(("insp-call", i), if is_on { "Clear call-out" } else { "Call out on air" }).on_click(move |_, _, cx| {
                            call_app.update(cx, |s, cx| {
                                s.select_alert(if is_on { None } else { Some(call_id.clone()) });
                                cx.notify();
                            });
                        }))
                        .when(motion.is_some(), |d| {
                            d.child(button_small(("insp-track", i), "Track storm").on_click(move |_, _, cx| {
                                track_app.update(cx, |s, cx| {
                                    s.track_alert(&track_id);
                                    cx.notify();
                                });
                            }))
                        })
                        .child(button_small(("insp-lt", i), "Lower third").on_click(move |_, _, cx| {
                            lt_app.update(cx, |s, cx| {
                                s.warning_graphic(&lt_id, false);
                                cx.notify();
                            });
                        })),
                )
        })
        .collect::<Vec<_>>();

    col()
        .id("map-inspect")
        .absolute()
        .top(px(44.))
        .right(px(10.))
        .w(px(360.))
        .max_h(relative(0.86))
        .overflow_y_scroll()
        .p(px(12.))
        .gap(px(10.))
        .rounded(px(8.))
        .bg(c(PANEL))
        .border_1()
        .border_color(c(BORDER))
        .shadow_lg()
        // Clicks and scrolling inside the card stay in the card.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .child(
            row()
                .gap(px(8.))
                .child(
                    col()
                        .flex_1()
                        .min_w_0()
                        .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(inspect.place.clone().unwrap_or_else(|| "This spot".into())))
                        .child(muted(format!("{:.3}, {:.3}", inspect.lat, inspect.lon)))
                        .child(muted({
                            let (site, ft, mi) = crate::data::beam_height(inspect.lat, inspect.lon);
                            format!("Radar beam {:.0} ft up here ({site}, {mi:.0} mi away)", (ft / 100.0).round() * 100.0)
                        }))
                        .child(muted({
                            let home = app.read(cx).settings.home;
                            let mi = crate::map::geo::haversine_km(home.lat, home.lon, inspect.lat, inspect.lon) / 1.609_344;
                            let dir = crate::map::geo::compass(crate::map::geo::bearing_deg(home.lat, home.lon, inspect.lat, inspect.lon));
                            let elev = inspect.elevation_ft.map(|e| format!(" • elevation {e:.0} ft")).unwrap_or_default();
                            format!("{mi:.0} mi {dir} of home{elev}")
                        }))
                        .child(muted({
                            let town = crate::places::in_box(inspect.lat - 0.15, inspect.lat + 0.15, inspect.lon - 0.2, inspect.lon + 0.2)
                                .into_iter()
                                .min_by(|a, b| {
                                    let da = (a.lat - inspect.lat).powi(2) + (a.lon - inspect.lon).powi(2);
                                    let db = (b.lat - inspect.lat).powi(2) + (b.lon - inspect.lon).powi(2);
                                    da.total_cmp(&db)
                                });
                            let sun = crate::astro::sun_times(chrono::Local::now().date_naive(), inspect.lat, inspect.lon)
                                .map(|(r, s)| format!("Sunrise {} • sunset {}", r.format("%-I:%M %p"), s.format("%-I:%M %p")))
                                .unwrap_or_default();
                            match town {
                                Some(t) if t.population > 0 => format!("{} pop. {} • {sun}", t.name, t.population),
                                _ => sun,
                            }
                        })),
                )
                .child(button_small("insp-close", "✕").on_click(cx.listener(move |this, _, _, cx| on_close(this, cx)))),
        )
        .child(conditions)
        .child({
            // Model forecast for this exact spot (neighborhood level).
            let s = app.read(cx);
            let line = s.data.grid.as_deref().and_then(|g| {
                let temps: Vec<f32> = (0..=24).filter_map(|h| g.sample(crate::grid::Var::Temp, g.time_index(h as f32), inspect.lat, inspect.lon)).collect();
                let hi = temps.iter().copied().reduce(f32::max)?;
                let lo = temps.iter().copied().reduce(f32::min)?;
                let end = g.time_index(24.0);
                let rain = match (g.sample(crate::grid::Var::RainAcc, end, inspect.lat, inspect.lon), g.sample(crate::grid::Var::RainAcc, g.time_index(0.0), inspect.lat, inspect.lon)) {
                    (Some(a), Some(z)) => (a - z).max(0.0),
                    _ => 0.0,
                };
                Some(format!("Next 24 h here: high {hi:.0}°, low {lo:.0}°, rain {rain:.2}\""))
            });
            muted(line.unwrap_or_else(|| "Model forecast loads with the data maps".into()))
        })
        .child({
            let make_app = app.clone();
            let name = inspect.place.clone().unwrap_or_else(|| format!("{:.2}, {:.2}", inspect.lat, inspect.lon));
            let (lat, lon) = (inspect.lat, inspect.lon);
            button_small("insp-local", "Hour-by-hour graphic for this spot").on_click(move |_, _, cx| {
                make_app.update(cx, |s, cx| {
                    if let Some(mut doc) = crate::gfx::templates::build("hourly_strip") {
                        doc.name = format!("Hour by Hour - {name}");
                        doc.place = Some(crate::gfx::model::PlaceRef { name: name.clone(), lat, lon });
                        if let Some(saved) = s.save_graphic(doc) {
                            s.pending_graphic = Some(saved.id.clone());
                            s.workspace = crate::app::Workspace::Graphics;
                        }
                    }
                    cx.notify();
                });
            })
        })
        .child({
            let zone_app = app.clone();
            let name = inspect.place.clone().unwrap_or_else(|| format!("{:.2}, {:.2}", inspect.lat, inspect.lon));
            let (lat, lon) = (inspect.lat, inspect.lon);
            button_small("insp-watch", "Watch this spot for warnings").on_click(move |_, _, cx| {
                zone_app.update(cx, |s, cx| {
                    s.add_alert_zone(name.clone(), lat, lon);
                    cx.notify();
                });
            })
        })
        .when_some(inspect.cell.clone(), |d, cell| {
            let motion = cell.motion.as_ref().map(|m| format!("Moving {} at {:.0} mph", m.toward, m.speed_mph)).unwrap_or_else(|| "Nearly stationary".into());
            let lightning = if cell.lightning.strikes > 0 { format!("{} lightning strikes in {} min", cell.lightning.strikes, cell.lightning.minutes.max(1)) } else { "No recent lightning".into() };
            d.child(div().h(px(1.)).bg(c(BORDER)))
                .child(div().text_size(px(15.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).child(cell.calm_label()))
                .child(kv("Strength", format!("{:.0} dBZ", cell.max_dbz)))
                .child(kv("Movement", motion))
                .child(kv("Lightning", lightning))
                .when_some(cell.intensity.hail.clone().filter(|h| !h.is_empty()), |d, hail| d.child(kv("Hail", hail)))
                .child(kv("From home", format!("{:.0} mi {}", cell.distance_mi, cell.direction)))
        })
        .child(div().h(px(1.)).bg(c(BORDER)))
        .when(alerts.is_empty(), |d| d.child(muted("No alerts in effect here.")))
        .when(!alerts.is_empty(), |d| {
            d.child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(format!("{} alert(s) here", alerts.len())))
                .children(alerts)
        })
}
