use super::*;
use crate::gfx::{check::PreviewData, model::LayerKind};
use chrono::NaiveDate;

pub(super) fn sample(doc: &mut GraphicDoc) {
    comparison::sample(doc);
    mesoscale::sample(doc);
    river_crest::sample(doc);
    show::sample(doc);
    show_ident::sample(doc);
    show_schedule::sample(doc);
    sponsor_cards::sample(doc);
    catalog_completion::sample(doc);
    catalog_model_graphics::sample(doc);
    community_feature_cards::sample(doc);
    producer_story_cards::sample(doc);
    multi_day_forecast_cards::sample(doc);
    season_countdown_cards::sample(doc);
    forecast_editor_cards::sample(doc);
    viewer_cards::sample(doc);

    social_forecast::sample(doc);
    holiday_forecast::sample(doc);
    five_day::sample(doc);
    city_forecast::sample(doc);
    weather_word::sample(doc);
    town_forecast_maps::sample(doc);
    model_maps::sample(doc);
    spc_outlook::sample(doc);
    qpf::sample(doc);
    hourly_charts::sample(doc);
    weather_ticker::sample(doc);
    road_closures::sample(doc);
    point_report_overlays::sample(doc);
    nwps_gauges::sample(doc);
    zone_overlays::sample(doc);
    forecast_packaging::sample(doc);
    relief::sample(doc);
    social_alert::sample(doc);
    heat_risk::sample(doc);
    cpc::sample(doc);
    fire_weather::sample(doc);
    tornado_history::sample(doc);
    tornado_monthly::sample(doc);
    observed::sample(doc);
    winter_alerts::sample(doc);
    cold::sample(doc);
    shelter::sample(doc);
    quakes::sample(doc);
    let mut put = |key: &str, value: &str| doc.data_overrides.push((key.into(), value.into()));
    put("set.tornado.period", "SAMPLE • Jan 1–Nov 13, 2026");
    put("set.tornado.scope", "Indiana • SAMPLE DATA");
    put("set.tornado.source", "SAMPLE ONLY • Not for broadcast");
    put(
        "set.tornado.note",
        "SAMPLE • One marker per unique tornado • Unrated events shown separately",
    );
    put(
        "set.tornado.payload",
        r#"{"state":"18","reviewed":true,"annual_average":24.0,"baseline":"SAMPLE baseline: 1991–2020","events":[{"id":"sample0","lon":-86.9,"lat":38.4,"ef":0},{"id":"sample1","lon":-86.5,"lat":39.2,"ef":1},{"id":"sample2","lon":-86.2,"lat":39.8,"ef":2},{"id":"sample3","lon":-85.4,"lat":40.5,"ef":3},{"id":"sample4","lon":-86.4,"lat":41.2,"ef":4},{"id":"sample5","lon":-85.2,"lat":41.2,"ef":5},{"id":"sample6","lon":-87.1,"lat":40.5,"ef":null}]}"#,
    );
    for (key, value) in [
        ("status", "Tornado confirmed • SAMPLE"),
        ("location", "Jeffersonville, Indiana"),
        ("rating", "EF2 • Preliminary"),
        ("wind", "120 mph • Estimated"),
        ("date", "November 13, 2026"),
        (
            "note",
            "SAMPLE survey • Map markers are synthetic layout fixtures, not actual events.",
        ),
    ] {
        put(&format!("set.survey.{key}"), value);
    }
    put("set.history.period", "SAMPLE • 1991–2025");
    put(
        "set.history.note",
        "SAMPLE • Full calendar years • Ties ranked by most recent year",
    );
    for (i, (year, total)) in [
        ("2011", "72"),
        ("2004", "65"),
        ("2017", "49"),
        ("2008", "42"),
        ("2023", "36"),
    ]
    .iter()
    .enumerate()
    {
        put(&format!("set.history.year{i}"), year);
        put(&format!("set.history.count{i}"), total);
    }
    {
        put(
            "set.survey.payload",
            r#"{"state":"18","reviewed":true,"events":[{"id":"sample-survey","lon":-85.74,"lat":38.28,"ef":2}]}"#,
        );
    }
}

#[test]
fn product_overlays_preserve_transparency_over_a_program_picture() {
    let settings = crate::config::Settings::default();
    let data = crate::data::WxSnapshot::default();
    let keys: Vec<_> = TEMPLATES
        .iter()
        .filter(|t| {
            (t.key.ends_with("_overlay")
                || t.key.starts_with("set_snow_point_overlay__")
                || t.key.starts_with("set_hail_point_overlay__")
                || t.key.starts_with("set_wind_point_overlay__"))
                && (t.key.starts_with("set_winter_alerts__")
                    || t.key.starts_with("set_heat_risk__")
                    || t.key.starts_with("set_cpc_")
                    || t.key.starts_with("set_spc_prob__")
                    || t.key.starts_with("set_wpc_qpf__")
                    || t.key.starts_with("set_wind_chill__")
                    || t.key.starts_with("set_morning_lows__")
                    || t.key.starts_with("set_heat_index__")
                    || t.key.starts_with("set_snow_point_overlay__")
                    || t.key.starts_with("set_hail_point_overlay__")
                    || t.key.starts_with("set_wind_point_overlay__")
                    || t.key.starts_with("set_nwps_flood_map__")
                    || t.key.starts_with("set_town_forecast_maps__")
                    || t.key.starts_with("set_road_closures__")
                    || t.key.starts_with("set_forecast_zones__"))
        })
        .collect();
    assert!(!keys.is_empty());
    for info in keys {
        for look in ["modern", "classic"] {
            let mut doc = super::build(info.key).unwrap();
            doc.template = Some(info.key.into());
            sample(&mut doc);
            crate::gfx::look::apply(&mut doc, look);
            let layer = crate::program::render_graphic_mode(
                &doc,
                &settings,
                &data,
                10_000.,
                1920,
                1080,
                None,
                PreviewData::Sample,
            );
            let clear = layer.pixels().iter().filter(|p| p.alpha() == 0).count();
            assert!(
                clear > 1920 * 1080 / 4,
                "{} {look} hides too much program picture",
                info.key
            );
            assert_eq!(
                layer.pixel(1919, 1079).unwrap().alpha(),
                0,
                "Overlay corner must remain clear"
            );
            assert!(layer.pixels().iter().any(|p| p.alpha() > 0));
            let mut program = tiny_skia::Pixmap::new(1920, 1080).unwrap();
            program.fill(tiny_skia::Color::from_rgba8(28, 58, 82, 255));
            program.draw_pixmap(
                0,
                0,
                layer.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                tiny_skia::Transform::identity(),
                None,
            );
            let corner = program.pixel(1919, 1079).unwrap();
            assert_eq!(
                [corner.red(), corner.green(), corner.blue(), corner.alpha()],
                [28, 58, 82, 255]
            );
            // Supplemental underlay views exercise each scope and both looks. They are
            // reviewed separately from the four normal PNGs for each registered key.
            if info.key.starts_with("set_heat_risk__today_")
                || matches!(
                    info.key,
                    "set_spc_prob__tornado_day1_state_overlay"
                        | "set_wpc_qpf__day1_state_overlay"
                        | "set_wind_chill__today_state_overlay"
                        | "set_morning_lows__tomorrow_county_overlay"
                        | "set_heat_index__day3_region_overlay"
                        | "set_snow_point_overlay__county"
                        | "set_nwps_flood_map__county_overlay"
                        | "set_town_forecast_maps__highlow_today_county_overlay"
                        | "set_road_closures__county_overlay"
                        | "set_forecast_zones__today_county_overlay"
                        | "set_winter_alerts__state_overlay"
                )
            {
                // Keep every transparency assertion above, but export only the selected
                // batch so unchanged reviewed originals retain their exact hashes.
                if let Some(dir) = std::env::var_os("CWX_QA_NEW_DIR").filter(|_| {
                    std::env::var("CWX_QA_SET_KEYS").ok().map_or(true, |keys| {
                        keys.split(',').any(|key| key.trim() == info.key)
                    })
                }) {
                    let dir = std::path::PathBuf::from(dir).join("product-overlay-underlays");
                    std::fs::create_dir_all(&dir).unwrap();
                    program
                        .save_png(dir.join(format!("{}-{look}-underlay.png", info.key)))
                        .unwrap();
                }
            }
        }
    }
}

#[test]
fn registry_builds_unique_templates_and_renders_both_looks() {
    let mut keys = std::collections::HashSet::new();
    for info in crate::gfx::templates::all() {
        assert!(keys.insert(info.key), "Duplicate key {}", info.key);
    }
    // This exhaustive panic/identity/bounds smoke test uses a proportional preview.
    // qa_reference_graphics retains full native-canvas PNGs for every newly reviewed
    // key, look, fixture and state; the full-resolution visual gate is unchanged.
    let settings = Settings::default();
    let data = WxSnapshot::default();
    for (index, info) in TEMPLATES.iter().enumerate() {
        if index % 100 == 0 {
            eprintln!("Registry render: {index}/{} keys complete; next {}", TEMPLATES.len(), info.key);
        }
        for look in ["modern", "classic"] {
            let mut doc = crate::gfx::templates::build_in(info.key, look).unwrap();
            assert_eq!(doc.template.as_deref(), Some(info.key));
            sample(&mut doc);
            if info.key.ends_with("_portrait") {
                assert_eq!((doc.width, doc.height), (1080, 1920));
                let converted = doc.to_portrait();
                assert!(
                    doc.layers
                        .iter()
                        .zip(&converted.layers)
                        .all(|(a, b)| (a.x, a.y, a.w, a.h) == (b.x, b.y, b.w, b.h)),
                    "Native portrait must survive the existing portrait export"
                );
                let preview = crate::program::render_graphic_mode(
                    &doc,
                    &settings,
                    &data,
                    10_000.,
                    480,
                    270,
                    None,
                    PreviewData::Sample,
                );
                assert!(preview.data().iter().any(|v| *v != 0));
            }
            if info.key.ends_with("_square") {
                assert_eq!((doc.width, doc.height), (1080, 1080));
                let preview = crate::program::render_graphic_mode(
                    &doc,
                    &settings,
                    &data,
                    10_000.,
                    480,
                    270,
                    None,
                    PreviewData::Sample,
                );
                assert!(preview.data().iter().any(|v| *v != 0));
            }
            let states = if doc.states.is_empty() {
                vec![String::new()]
            } else {
                doc.states.clone()
            };
            for state in states {
                let doc = doc.in_state(&state, &[]);
                let pm = crate::program::render_graphic_mode(
                    &doc,
                    &settings,
                    &data,
                    10_000.,
                    480,
                    (doc.height as f64 * 480. / doc.width as f64).round() as u32,
                    None,
                    PreviewData::Sample,
                );
                assert!(pm.data().iter().any(|v| *v != 0));
                if info.key.ends_with("_lowerthird") || info.key.starts_with("set_weather_ticker__")
                {
                    assert!(doc.background.is_none());
                    assert_eq!(
                        pm.pixel(100, 100).unwrap().alpha(),
                        0,
                        "Lower third must leave the weather map above it visible"
                    );
                }
                assert!(
                    doc.layers.iter().all(|l| l.x >= 0.
                        && l.y >= 0.
                        && l.x + l.w <= doc.width as f32 + 0.5
                        && l.y + l.h <= doc.height as f32 + 0.5),
                    "{} {look}",
                    info.key
                );
            }
        }
    }
    eprintln!("Registry render complete: {} keys in both looks", TEMPLATES.len());
    assert!(build("not_a_set").is_none());
}

#[test]
fn missing_weather_never_becomes_zero_or_confirmed() {
    let mut b = Bindings::default();
    bind(&mut b, &WxSnapshot::default(), &Settings::default());
    assert_eq!(b.get("set.survey.status"), Some(reference::MISSING));
    assert!(tornado::parse(b.get("set.tornado.payload").unwrap())
        .unwrap()
        .events
        .is_none());
    for info in TEMPLATES {
        let doc = build(info.key).unwrap();
        for l in doc.layers {
            match l.kind {
                LayerKind::Text { text, .. } => {
                    assert!(b.unknown_keys(&text).iter().all(|s| !s.starts_with("set.")))
                }
                LayerKind::RiskMap { spec } => assert!(!b.apply(&spec).contains("{{")),
                _ => {}
            }
        }
    }
}

#[test]
fn payload_requires_review_and_deduplicates_tornado_ids() {
    let good = r#"{"state":"18","reviewed":true,"events":[{"id":"a","lat":38.4,"lon":-86.9,"ef":2},{"id":"a","lat":38.4,"lon":-86.9,"ef":2}]}"#;
    assert_eq!(tornado::parse(good).unwrap().events.unwrap().len(), 1);
    assert_eq!(
        tornado::category_counts(&tornado::parse(good).unwrap().events.unwrap()),
        [0, 0, 1, 0, 0, 0, 0]
    );
    assert!(tornado::parse(&good.replace(
        "\"reviewed\":true",
        "\"reviewed\":true,\"annual_average\":-1"
    ))
    .is_none());
    assert!(tornado::parse(&good.replacen("\"ef\":2", "\"ef\":3", 1)).is_none());
    assert!(tornado::parse(&good.replace("true", "false")).is_none());
    assert!(tornado::parse(&good.replace("\"ef\":2", "\"ef\":6")).is_none());
    assert!(tornado::parse(&good.replace("38.4", "138.4")).is_none());
    assert!(tornado::parse("malformed").is_none());
    assert!(
        tornado::parse(r#"{"state":"18","reviewed":true,"events":[]}"#)
            .unwrap()
            .events
            .unwrap()
            .is_empty()
    );
}

#[test]
fn calendar_handles_today_leap_year_and_rollover() {
    let date = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    let thanksgiving = reference::holidays(date(2026, 11, 13));
    assert_eq!(thanksgiving[0], ("Thanksgiving", date(2026, 11, 26)));
    assert_eq!(
        reference::holidays(date(2026, 12, 26))[0],
        ("New Year's Day", date(2027, 1, 1))
    );
    assert_eq!(
        reference::holidays(date(2028, 2, 29))[0],
        ("Memorial Day", date(2028, 5, 29))
    );
    assert_eq!(reference::remaining(0), "Today");
    assert_eq!(reference::remaining(1), "1 day");
    assert_eq!(reference::remaining(7), "1 week");
    assert_eq!(reference::remaining(13), "1 week 6 days");
}

/// Required full-resolution visual QA. No fixtures enter the live bindings or store.
#[test]
fn qa_reference_graphics() {
    let Some(dir) = std::env::var_os("CWX_QA_NEW_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    assert!(
        crate::map::boundaries::counties_named().is_some_and(|v| v.iter().any(|c| c.state == "18")),
        "QA requires the existing Indiana county geometry cache; do not substitute invented outlines"
    );
    // Synthetic image fixture exercises contain/credit without copying licensed photos.
    let fixture_path = dir.join("sample-image-fixture.png");
    let mut image = Pixmap::new(640, 400).unwrap();
    image.fill(tiny_skia::Color::from_rgba8(37, 69, 102, 255));
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(60, 118, 161, 255);
    image.fill_rect(
        tiny_skia::Rect::from_xywh(0., 220., 640., 180.).unwrap(),
        &paint,
        Transform::identity(),
        None,
    );
    image.save_png(&fixture_path).unwrap();
    let portable = crate::config::portable_root().expect(
        "Run QA through docs/codex/Validate-Graphics.ps1 to isolate the test media library",
    );
    let media = portable.join("Library").join("Media");
    std::fs::create_dir_all(&media).unwrap();
    std::fs::copy(&fixture_path, media.join("sets-qa-image.png")).unwrap();
    let settings = Settings::default();
    let data = WxSnapshot::default();
    let selected_keys=super::qa_selection::selected();
    for info in TEMPLATES {
        if selected_keys.as_ref().is_some_and(|keys| !keys.contains(info.key)) { continue; }
        for look in ["modern", "classic"] {
            for fixture in ["sample", "unavailable"] {
                let mut doc = crate::gfx::templates::build_in(info.key, look).unwrap();
                if fixture == "sample" {
                    sample(&mut doc);
                    doc.data_overrides
                        .push(("set.history.photo".into(), "sets-qa-image.png".into()));
                    doc.data_overrides.push((
                        "set.history.credit".into(),
                        "SAMPLE IMAGE • Synthetic layout fixture".into(),
                    ));
                 } else if info.key.starts_with("set_season_countdown__") {
                    doc.data_overrides.extend([
                        ("set.season.next".into(), "Season date unavailable".into()),
                        ("set.season.days".into(), "Unavailable".into()),
                        ("set.season.sample_note".into(), "".into()),
                    ]);
                } else if info.key.starts_with("set_cold_criteria_compare") {
                    let mut scene = board::empty(board::Layout::Compare);
                    scene.scope = comparison::scope(&settings, board::Layout::Zones);
                    scene.panels[0].title = "Cold Weather Advisory".into();
                    scene.panels[1].title = "Extreme Cold Warning".into();
                    doc.data_overrides.push((
                        "set.coldcriteria.payload".into(),
                        serde_json::to_string(&scene).unwrap(),
                    ));
                    doc.data_overrides.push((
                        "set.coldcriteria.source".into(),
                        "NWS office criteria • Data not available yet".into(),
                    ));
                }
                let states = if doc.states.is_empty() {
                    vec![String::new()]
                } else {
                    doc.states.clone()
                };
                for state in states {
                    let doc = doc.in_state(&state, &[]);
                    let pm = crate::program::render_graphic_mode(
                        &doc,
                        &settings,
                        &data,
                        10_000.,
                        doc.width,
                        doc.height,
                        None,
                        PreviewData::Sample,
                    );
                    if info.key == "set_tornado_top_years"
                        && state == "With photo"
                        && fixture == "sample"
                    {
                        let px = pm.pixel(1500, 450).unwrap().demultiply();
                        assert_eq!(
                            [px.red(), px.green(), px.blue()],
                            [37, 69, 102],
                            "Photo must actually load from its filename binding"
                        );
                    }
                    let suffix = match state.as_str() {
                        "Today" if info.key.starts_with("set_forecast_story") => "-today",
                        "Tomorrow" if info.key.starts_with("set_forecast_story") => "-tomorrow",
                        "Day 3" if info.key.starts_with("set_forecast_story") => "-day3",
                        "Question" if info.key.starts_with("set_weather_quiz") => "-question",
                        "Answer" if info.key.starts_with("set_weather_quiz") => "-answer",
                        "With photo" => "-photo",
                        "Map" => "-map",
                        "Table"
                            if !info.key.ends_with("_portrait")
                                && (info.key.starts_with("set_peak_gusts")
                                    || info.key.starts_with("set_largest_hail")) =>
                        {
                            "-table"
                        }
                        _ => "",
                    };
                    pm.save_png(dir.join(format!("{}-{look}-{fixture}{suffix}.png", info.key)))
                        .unwrap();
                }
            }
        }
    }
}
