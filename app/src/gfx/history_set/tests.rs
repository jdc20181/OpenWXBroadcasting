use crate::{config::Settings, data::WxSnapshot};

#[test]
fn keys_unique_and_build_in_both_looks() {
    let mut keys = std::collections::HashSet::new();
    for t in crate::gfx::templates::all() {
        assert!(keys.insert(t.key), "duplicate template key {}", t.key);
    }
    for t in super::TEMPLATES {
        for look in ["modern", "classic"] {
            let doc = crate::gfx::templates::build_in(t.key, look).unwrap_or_else(|| panic!("{} did not build", t.key));
            assert_eq!(doc.template.as_deref(), Some(t.key));
            assert!(!doc.layers.is_empty());
        }
    }
}

#[test]
fn renders_without_data_in_both_looks() {
    let settings = Settings::default();
    let data = WxSnapshot::default();
    for t in super::TEMPLATES {
        for look in ["modern", "classic"] {
            let doc = crate::gfx::templates::build_in(t.key, look).unwrap();
            let _ = crate::program::render_graphic_mode(&doc, &settings, &data, 10_000., 480, 270, None, crate::gfx::check::PreviewData::Sample);
        }
    }
}

#[test]
fn bins_cover_every_count() {
    for max in [0, 1, 5, 9, 10, 23, 180] {
        let b = super::canvas::bins(max);
        for n in 0..=max {
            assert!(b.iter().any(|(lo, hi)| n >= *lo && n <= *hi), "max {max} misses {n}");
        }
        assert!(b.len() <= 11);
    }
}

#[test]
fn freeze_dates_from_dailies() {
    use chrono::NaiveDate;
    let mut h = super::data::Hist { year: 2026, ..Default::default() };
    for (d, lo) in [("2025-04-10", 30.), ("2025-04-20", 31.), ("2025-05-01", 40.), ("2025-10-20", 32.), ("2025-11-02", 25.)] {
        h.daily.push(super::data::Daily { date: NaiveDate::parse_from_str(d, "%Y-%m-%d").unwrap(), hi: Some(50.), lo: Some(lo), precip: Some(0.) });
    }
    // Fill the rest of 2025 so the year counts as complete.
    let mut day = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
    while day.year_ce().1 == 2025 {
        if !h.daily.iter().any(|x| x.date == day) {
            h.daily.push(super::data::Daily { date: day, hi: Some(60.), lo: Some(45.), precip: Some(0.) });
        }
        day = day.succ_opt().unwrap();
    }
    let f = super::freezes(&h);
    let y = f.iter().find(|f| f.0 == 2025).unwrap();
    assert_eq!(y.1, NaiveDate::from_ymd_opt(2025, 4, 20));
    assert_eq!(y.2, NaiveDate::from_ymd_opt(2025, 10, 20));
}

use chrono::Datelike;

/// Visual QA with real IEM data (network). Run with CWX_QA_NEW_DIR set:
/// `CWX_QA_NEW_DIR=docs/audit/2026-11-15/graphics cargo test history_set::tests::qa -- --ignored`
#[test]
#[ignore]
fn qa_render_history() {
    let Some(dir) = std::env::var_os("CWX_QA_NEW_DIR") else { return };
    let dir = std::path::PathBuf::from(dir).join("history");
    std::fs::create_dir_all(&dir).unwrap();
    if std::env::var_os("CWX_HISTORY_CACHE").is_none() {
        std::env::set_var("CWX_HISTORY_CACHE", std::env::temp_dir().join("cwx-history-cache"));
    }
    let settings = Settings::default();
    crate::net::runtime().block_on(super::refresh(settings.home.clone()));
    let data = WxSnapshot::default();
    let only = std::env::var("CWX_QA_ONLY").unwrap_or_default();
    for t in super::TEMPLATES.iter().filter(|t| only.is_empty() || t.key.contains(&only)) {
        for look in ["modern", "classic"] {
            let doc = crate::gfx::templates::build_in(t.key, look).unwrap();
            let img = crate::program::render_graphic_mode(&doc, &settings, &data, 10_000., 1920, 1080, None, crate::gfx::check::PreviewData::Live);
            img.save_png(dir.join(format!("{}-{look}.png", t.key))).unwrap();
        }
    }
}
