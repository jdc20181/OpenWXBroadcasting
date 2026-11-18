use crate::{config::Settings, data::WxSnapshot};

#[test]
fn every_explainer_builds_with_a_unique_key_and_known_scene() {
    let mut keys = std::collections::HashSet::new();
    let mut scenes = std::collections::HashSet::new();
    for e in super::EXPLAINERS {
        assert!(keys.insert(e.key), "duplicate key {}", e.key);
        // CWXB-76: one scene per topic, never reused for an unrelated explainer.
        assert!(scenes.insert(e.scene), "scene {} used twice", e.scene);
        for look in ["modern", "classic"] {
            let doc = crate::gfx::templates::build_in(e.key, look).unwrap_or_else(|| panic!("{} did not build", e.key));
            assert_eq!(doc.template.as_deref(), Some(e.key));
        }
        assert!(super::subcategory(e.key).is_some());
    }
    assert_eq!(super::TEMPLATES.len(), super::EXPLAINERS.len());
}

#[test]
fn renders_in_both_looks() {
    let settings = Settings::default();
    let data = WxSnapshot::default();
    for e in super::EXPLAINERS {
        for look in ["modern", "classic"] {
            let doc = crate::gfx::templates::build_in(e.key, look).unwrap();
            let _ = crate::program::render_graphic_mode(&doc, &settings, &data, 10_000., 480, 270, None, crate::gfx::check::PreviewData::Sample);
        }
    }
}

#[test]
fn nws_formulas_match_published_values() {
    // NWS wind chill chart: 0°F at 15 mph = -19; 10°F at 30 mph = -12.
    assert_eq!(super::scenes_b::wind_chill(0., 15.).round(), -19.);
    assert_eq!(super::scenes_b::wind_chill(10., 30.).round(), -12.);
    // NWS heat index chart: 90°F / 60% = 100; 100°F / 40% = 109.
    assert_eq!(super::scenes_b::heat_index(90., 60.).round(), 100.);
    assert_eq!(super::scenes_b::heat_index(100., 40.).round(), 109.);
}

/// Visual QA: `CWX_QA_NEW_DIR=docs/audit/2026-11-15/graphics cargo test explain_set::tests::qa -- --ignored`
#[test]
#[ignore]
fn qa_render_explainers() {
    let Some(dir) = std::env::var_os("CWX_QA_NEW_DIR") else { return };
    let dir = std::path::PathBuf::from(dir).join("explain");
    std::fs::create_dir_all(&dir).unwrap();
    let settings = Settings::default();
    let data = WxSnapshot::default();
    let only = std::env::var("CWX_QA_ONLY").unwrap_or_default();
    let looks: Vec<&str> = if std::env::var_os("CWX_QA_BOTH").is_some() { vec!["modern", "classic"] } else { vec!["modern"] };
    for e in super::EXPLAINERS.iter().filter(|e| only.is_empty() || e.key.contains(&only)) {
        for look in &looks {
            let doc = crate::gfx::templates::build_in(e.key, look).unwrap();
            let img = crate::program::render_graphic_mode(&doc, &settings, &data, 10_000., 1920, 1080, None, crate::gfx::check::PreviewData::Live);
            img.save_png(dir.join(format!("{}-{look}.png", e.key))).unwrap();
        }
    }
}
