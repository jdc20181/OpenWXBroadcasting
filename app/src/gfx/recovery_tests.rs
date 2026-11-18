use super::{check::PreviewData, model::LayerKind, templates};
use crate::{config::Settings, data::WxSnapshot};

#[test]
fn every_catalog_template_builds_and_renders_sample_data() {
    let settings = Settings::default();
    let data = WxSnapshot::default();
    let dir = std::env::var_os("CWX_QA_OUTPUT_DIR").map(std::path::PathBuf::from);
    if let Some(dir) = &dir {
        std::fs::create_dir_all(dir).unwrap();
    }
    let mut count = 0;
    let mut names = Vec::new();
    for info in templates::all() {
        let doc = templates::build(info.key)
            .unwrap_or_else(|| panic!("catalog entry {} has no builder", info.key));
        assert!(
            doc.width > 0 && doc.height > 0,
            "{} has no canvas",
            info.key
        );
        assert!(
            doc.layers
                .iter()
                .all(|l| l.x.is_finite() && l.y.is_finite() && l.w.is_finite() && l.h.is_finite()),
            "{} has invalid geometry",
            info.key
        );
        let pm = crate::program::render_graphic_mode(
            &doc,
            &settings,
            &data,
            10_000.,
            640,
            360,
            None,
            PreviewData::Sample,
        );
        assert_eq!((pm.width(), pm.height()), (640, 360));
        assert!(
            pm.data().chunks_exact(4).any(|p| p[3] > 0),
            "{} rendered empty",
            info.key
        );
        if let Some(dir) = &dir {
            if count % 8 == 0
                || [
                    "current",
                    "seven_day",
                    "sounding",
                    "ensemble_plume",
                    "storm_reports",
                ]
                .contains(&info.key)
            {
                pm.save_png(dir.join(format!("{}.png", info.key))).unwrap();
            }
        }
        names.push(info.key);
        count += 1;
    }
    assert!(count > 30, "template catalog unexpectedly small");
    if let Some(dir) = &dir {
        std::fs::write(dir.join("rendered-templates.txt"), names.join("\n")).unwrap();
    }
    eprintln!("Rendered {count} catalog templates with sample data");
}

#[test]
fn browser_export_has_alpha_layers_and_native_motion() {
    let mut doc = templates::blank(true);
    doc.width = 320;
    doc.height = 180;
    doc.name = "Browser QA".into();
    let mut layer = super::model::Layer::text(
        "Test",
        20.,
        20.,
        260.,
        80.,
        "Weather 72°",
        "Segoe UI",
        40.,
        super::text::Weight::Bold,
        "#FFFFFF",
        super::text::Align::Left,
    );
    layer.anim_in = super::model::Anim::new(super::model::AnimKind::Fade, 0., 0.5);
    doc.layers.push(layer);
    let path = super::browser::export(&doc, &Settings::default(), &WxSnapshot::default()).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let start =
        text.find("type=\"application/json\">").unwrap() + "type=\"application/json\">".len();
    let end = text[start..].find("</script>").unwrap() + start;
    let payload: serde_json::Value = serde_json::from_str(&text[start..end]).unwrap();
    assert_eq!(payload["layers"].as_array().unwrap().len(), 1);
    assert_eq!(payload["layers"][0]["poses"][0][0], 0.);
    assert_eq!(payload["layers"][0]["poses"][30][0], 1.);
    assert!(text.contains("getContext('webgl2'"));
    assert!(!text.contains("https://"));
    if let Some(dir) = std::env::var_os("CWX_QA_OUTPUT_DIR") {
        std::fs::copy(&path, std::path::Path::new(&dir).join("browser-qa.html")).unwrap();
    }
    let _ = std::fs::remove_file(path);
}

#[test]
#[cfg(windows)]
fn variable_weight_changes_glyph_coverage() {
    let file = std::path::Path::new("C:/Windows/Fonts/bahnschrift.ttf");
    let Ok(bytes) = std::fs::read(file) else {
        eprintln!("Bahnschrift unavailable; variable-font fixture skipped");
        return;
    };
    let font = super::variable_font::LoadedFont::from_bytes(bytes).expect("system variable font");
    assert!(font.variable);
    let id = font.lookup_glyph_index('W');
    let (_, light) = font.raster(id, 64., super::text::Weight::Light);
    let (_, bold) = font.raster(id, 64., super::text::Weight::Bold);
    let area = |p: &[u8]| p.iter().map(|v| *v as u64).sum::<u64>();
    assert!(
        area(&bold) > area(&light),
        "variable weight did not change coverage"
    );
}
