//! "Build a show for me": a complete weather segment from what is happening right now.
//!
//! Opening ID, current conditions, regional radar, any warnings in the coverage area (each
//! zoomed with an on-air call-out), the SPC outlook when there is a severe risk, Futurecast,
//! the near-term and 7-day forecasts, and the sign-off. Graphics it creates go into a folder
//! named for the show so they're easy to find or clear out later.

use crate::{
    app::AppState,
    config::GeoView,
    gfx::templates,
    map::scene::{DataField, MapScene, PanelLayout, RadarMode, RadarSource},
    show::{Scene, Show, Transition},
};

/// The live map with presenter-only state cleared: no drawings, call-out, data layer or panels.
fn clean_map(state: &AppState) -> MapScene {
    let mut map = state.live.clone();
    map.annotations.clear();
    map.storm_track = None;
    map.extra_tracks.clear();
    map.range_rings = None;
    map.selected_alert = None;
    map.data.field = None;
    map.precip_total = None;
    map.layout = PanelLayout::Single;
    map.satellite.enabled = false;
    map.radar.enabled = true;
    map.radar.source = RadarSource::Composite;
    map.radar.mode = RadarMode::Loop;
    map.spc_day = None;
    map
}

/// Build the show, saving the graphics it needs. Returns the new (unsaved) show.
pub fn build(state: &mut AppState) -> Show {
    let now = chrono::Local::now();
    let name = format!("Weather show {}", now.format("%a %b %-d, %-I:%M %p"));
    let folder = format!("Auto show {}", now.format("%b %-d %-I:%M %p"));
    let mut show = Show::new(&name);
    let home = state.settings.home;

    // Save a template as a graphic in this show's folder and add it as a scene.
    let mut add_graphic = |state: &mut AppState, show: &mut Show, key: &str, seconds: f32| {
        if let Some(mut doc) = templates::build(key) {
            doc.folder = folder.clone();
            if let Some(saved) = state.save_graphic(doc) {
                let mut scene = Scene::graphic(&saved);
                scene.duration = seconds;
                scene.transition = Transition::Fade;
                show.scenes.push(scene);
            }
        }
    };

    add_graphic(state, &mut show, "station_id", 5.0);
    add_graphic(state, &mut show, "current", 9.0);

    // Regional radar.
    let mut radar = clean_map(state);
    radar.view = GeoView { lat: home.lat, lon: home.lon, zoom: 6.4 };
    radar.title = "LIVE DOPPLER".into();
    let mut scene = Scene::map("Regional radar", radar.clone());
    scene.duration = 12.0;
    scene.notes = "Regional radar loop. Point out anything moving toward the area.".into();
    show.scenes.push(scene);

    // Local radar.
    let mut local = radar.clone();
    local.view = GeoView { lat: home.lat, lon: home.lon, zoom: 8.2 };
    let mut scene = Scene::map("Local radar", local);
    scene.duration = 10.0;
    show.scenes.push(scene);

    // Warnings in the coverage area, most serious first (up to three).
    let warnings = state.coverage_warnings();
    for alert in warnings.iter().take(3) {
        let mut map = clean_map(state);
        let (lat, lon) = alert.centroid().unwrap_or((home.lat, home.lon));
        map.view = GeoView { lat, lon, zoom: 8.6 };
        map.selected_alert = Some(alert.id.clone());
        map.title = alert.event.to_uppercase();
        let mut scene = Scene::map(&alert.event, map);
        scene.duration = 14.0;
        scene.notes = format!(
            "{} {}. {}",
            alert.event,
            alert.expires_label(state.settings.clock_24h).unwrap_or_default(),
            alert.towns().into_iter().take(6).collect::<Vec<_>>().join(", ")
        );
        show.scenes.push(scene);
    }

    // Does any polygon reach the box around home (degrees each way)? A risk on the far side
    // of the country leaves the scene's map empty (CWXB-81).
    let near = |rings: &[Vec<(f64, f64)>], dlon: f64, dlat: f64| {
        rings.iter().flatten().any(|(lon, lat)| (lon - home.lon).abs() <= dlon && (lat - home.lat).abs() <= dlat)
    };
    // Severe outlook when SPC has more than general thunder for today in the outlook view.
    let severe_risk = state
        .data
        .spc
        .iter()
        .find(|d| d.day == 1)
        .map(|d| d.categorical.iter().any(|a| !a.label.eq_ignore_ascii_case("TSTM") && near(&a.rings, 9.0, 5.5)))
        .unwrap_or(false);
    if severe_risk {
        let mut map = clean_map(state);
        map.view = GeoView { lat: home.lat, lon: home.lon, zoom: 5.0 };
        map.spc_day = Some(1);
        map.title = "SEVERE OUTLOOK".into();
        let mut scene = Scene::map("Severe outlook", map);
        scene.duration = 12.0;
        show.scenes.push(scene);
    }

    // Futurecast only when WPC expects rain or snow near home in the next day; on a dry day
    // the loop would show an empty map, so Hour by Hour takes its place. If the WPC feed
    // hasn't loaded, keep Futurecast (we can't tell).
    let qpf = &state.data.qpf[0];
    let dry = !qpf.is_empty() && !qpf.iter().any(|a| near(&a.rings, 3.0, 2.0));
    if dry {
        add_graphic(state, &mut show, "hourly_strip", 10.0);
        add_graphic(state, &mut show, "tonight_tomorrow", 9.0);
        add_graphic(state, &mut show, "seven_day", 10.0);
        add_graphic(state, &mut show, "sign_off", 6.0);
        return show;
    }
    let mut future = clean_map(state);
    future.view = GeoView { lat: home.lat, lon: home.lon, zoom: 6.6 };
    future.data.field = Some(DataField::Futurecast);
    future.data.play_on_take = true;
    future.data.hour = 0.0;
    future.data.loop_hours = 24;
    future.title = "FUTURECAST".into();
    let mut scene = Scene::map("Futurecast", future);
    scene.duration = 14.0;
    scene.transition = Transition::Fade;
    show.scenes.push(scene);

    add_graphic(state, &mut show, "tonight_tomorrow", 9.0);
    add_graphic(state, &mut show, "seven_day", 10.0);
    add_graphic(state, &mut show, "sign_off", 6.0);
    show
}

/// The largest towns within about 60 miles of home, largest first.
fn local_towns(state: &AppState, n: usize) -> Vec<crate::places::Place> {
    let home = state.settings.home;
    let mut towns = crate::places::in_box(home.lat - 0.9, home.lat + 0.9, home.lon - 1.2, home.lon + 1.2);
    towns.truncate(n);
    towns
}

/// A flythrough of the local towns: the camera flies from town to town with current
/// temperatures plotted, ending back over home.
pub fn flythrough(state: &mut AppState) -> Show {
    let now = chrono::Local::now();
    let mut show = Show::new(&format!("Town flythrough {}", now.format("%b %-d %-I:%M %p")));
    let home = state.settings.home;
    let mut start = clean_map(state);
    start.view = GeoView { lat: home.lat, lon: home.lon, zoom: 7.4 };
    start.title = state.settings.home_label.to_uppercase();
    start.data.field = Some(DataField::Temperature);
    start.data.city_plots = true;
    start.data.opacity = 0.35;
    let mut scene = Scene::map("Overview", start.clone());
    scene.duration = 6.0;
    show.scenes.push(scene);
    for town in local_towns(state, 6) {
        let mut map = start.clone();
        map.view = GeoView { lat: town.lat, lon: town.lon, zoom: 9.6 };
        map.title = town.name.to_uppercase();
        let mut scene = Scene::map(&town.name, map);
        scene.duration = 5.0;
        scene.transition = Transition::Fly;
        scene.notes = format!("Conditions in {}.", town.name);
        show.scenes.push(scene);
    }
    let mut back = start;
    back.view = GeoView { lat: home.lat, lon: home.lon, zoom: 7.4 };
    let mut scene = Scene::map("Back home", back);
    scene.duration = 5.0;
    scene.transition = Transition::Fly;
    show.scenes.push(scene);
    show
}

/// A looping rotation of forecast data maps for each part of the coverage area (for a weather
/// channel or a lobby screen): temperature, rain and wind over each of the largest towns.
pub fn data_rotation(state: &mut AppState) -> Show {
    let now = chrono::Local::now();
    let mut show = Show::new(&format!("Forecast map rotation {}", now.format("%b %-d")));
    show.loop_show = true;
    let fields = [
        (DataField::Temperature, "TEMPERATURES", 0.0),
        (DataField::RainTotal, "RAIN THROUGH TOMORROW", 36.0),
        (DataField::WindGust, "WIND GUSTS", 6.0),
    ];
    let mut areas = vec![(state.settings.home_label.clone(), state.settings.home.lat, state.settings.home.lon, 7.6)];
    areas.extend(local_towns(state, 3).into_iter().map(|t| (t.name, t.lat, t.lon, 8.8)));
    for (name, lat, lon, zoom) in areas {
        for (field, title, hour) in fields {
            let mut map = clean_map(state);
            map.radar.enabled = false;
            map.view = GeoView { lat, lon, zoom };
            map.data.field = Some(field);
            map.data.city_plots = true;
            map.data.hour = hour;
            map.title = format!("{} • {}", name.to_uppercase(), title);
            let mut scene = Scene::map(&format!("{name}: {}", field.label()), map);
            scene.duration = 8.0;
            scene.transition = Transition::Fade;
            show.scenes.push(scene);
        }
    }
    show
}

/// Several places framed at once: the view that fits all of them with a margin.
pub fn frame_points(points: &[(f64, f64)]) -> Option<GeoView> {
    if points.is_empty() {
        return None;
    }
    let (mut n, mut s, mut w, mut e) = (f64::MIN, f64::MAX, f64::MAX, f64::MIN);
    for (lat, lon) in points {
        n = n.max(*lat);
        s = s.min(*lat);
        w = w.min(*lon);
        e = e.max(*lon);
    }
    let span = (n - s).max((e - w) * 0.56).max(0.05) * 1.35;
    // 1080 px tall at zoom z covers about 1080 / 256 * 360 / 2^z degrees of longitude-ish.
    let zoom = (1080.0 / 256.0 * 180.0 / span).log2().clamp(3.0, 11.0);
    Some(GeoView { lat: (n + s) / 2.0, lon: (w + e) / 2.0, zoom })
}
