//! Transparent observed-report point overlays built only from the existing NWS LSR scene payloads.
use super::observed::{Coverage, Kind};
#[cfg(test)]
use super::board;
use crate::gfx::model::GraphicDoc;

#[derive(Clone, Copy)]
pub(super) struct Spec {
    pub catalog_id: &'static str,
    pub key: &'static str,
    kind: Kind,
    coverage: Coverage,
    hours: u16,
}

pub(super) const SPECS: &[Spec] = &[
    Spec {
        catalog_id: "G01289",
        key: "set_rain_point_overlay__county",
        kind: Kind::Rain,
        coverage: Coverage::County,
        hours: 24,
    },
    Spec {
        catalog_id: "G01290",
        key: "set_rain_point_overlay__region",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        hours: 24,
    },
    Spec {
        catalog_id: "G01291",
        key: "set_rain_point_overlay__state",
        kind: Kind::Rain,
        coverage: Coverage::State,
        hours: 24,
    },
    Spec {
        catalog_id: "G01292",
        key: "set_rain_point_overlay__48h_county",
        kind: Kind::Rain,
        coverage: Coverage::County,
        hours: 48,
    },
    Spec {
        catalog_id: "G01293",
        key: "set_rain_point_overlay__48h_region",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        hours: 48,
    },
    Spec {
        catalog_id: "G01294",
        key: "set_rain_point_overlay__48h_state",
        kind: Kind::Rain,
        coverage: Coverage::State,
        hours: 48,
    },
    Spec {
        catalog_id: "G01295",
        key: "set_rain_point_overlay__72h_county",
        kind: Kind::Rain,
        coverage: Coverage::County,
        hours: 72,
    },
    Spec {
        catalog_id: "G01296",
        key: "set_rain_point_overlay__72h_region",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        hours: 72,
    },
    Spec {
        catalog_id: "G01297",
        key: "set_rain_point_overlay__72h_state",
        kind: Kind::Rain,
        coverage: Coverage::State,
        hours: 72,
    },
    Spec {
        catalog_id: "G01298",
        key: "set_rain_point_overlay__168h_county",
        kind: Kind::Rain,
        coverage: Coverage::County,
        hours: 168,
    },
    Spec {
        catalog_id: "G01299",
        key: "set_rain_point_overlay__168h_region",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        hours: 168,
    },
    Spec {
        catalog_id: "G01300",
        key: "set_rain_point_overlay__168h_state",
        kind: Kind::Rain,
        coverage: Coverage::State,
        hours: 168,
    },
    Spec {
        catalog_id: "G03468",
        key: "set_snow_point_overlay__county",
        kind: Kind::Snow,
        coverage: Coverage::County,
        hours: 24,
    },
    Spec {
        catalog_id: "G03469",
        key: "set_snow_point_overlay__region",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        hours: 24,
    },
    Spec {
        catalog_id: "G03470",
        key: "set_snow_point_overlay__state",
        kind: Kind::Snow,
        coverage: Coverage::State,
        hours: 24,
    },
    Spec {
        catalog_id: "G03471",
        key: "set_snow_point_overlay__multi",
        kind: Kind::Snow,
        coverage: Coverage::Multi,
        hours: 24,
    },
    Spec {
        catalog_id: "G03472",
        key: "set_snow_point_overlay__48h_county",
        kind: Kind::Snow,
        coverage: Coverage::County,
        hours: 48,
    },
    Spec {
        catalog_id: "G03473",
        key: "set_snow_point_overlay__48h_region",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        hours: 48,
    },
    Spec {
        catalog_id: "G03474",
        key: "set_snow_point_overlay__48h_state",
        kind: Kind::Snow,
        coverage: Coverage::State,
        hours: 48,
    },
    Spec {
        catalog_id: "G03475",
        key: "set_snow_point_overlay__48h_multi",
        kind: Kind::Snow,
        coverage: Coverage::Multi,
        hours: 48,
    },
    Spec {
        catalog_id: "G03476",
        key: "set_snow_point_overlay__72h_county",
        kind: Kind::Snow,
        coverage: Coverage::County,
        hours: 72,
    },
    Spec {
        catalog_id: "G03477",
        key: "set_snow_point_overlay__72h_region",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        hours: 72,
    },
    Spec {
        catalog_id: "G03478",
        key: "set_snow_point_overlay__72h_state",
        kind: Kind::Snow,
        coverage: Coverage::State,
        hours: 72,
    },
    Spec {
        catalog_id: "G03479",
        key: "set_snow_point_overlay__72h_multi",
        kind: Kind::Snow,
        coverage: Coverage::Multi,
        hours: 72,
    },
    Spec {
        catalog_id: "G03480",
        key: "set_snow_point_overlay__168h_county",
        kind: Kind::Snow,
        coverage: Coverage::County,
        hours: 168,
    },
    Spec {
        catalog_id: "G03481",
        key: "set_snow_point_overlay__168h_region",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        hours: 168,
    },
    Spec {
        catalog_id: "G03482",
        key: "set_snow_point_overlay__168h_state",
        kind: Kind::Snow,
        coverage: Coverage::State,
        hours: 168,
    },
    Spec {
        catalog_id: "G03483",
        key: "set_snow_point_overlay__168h_multi",
        kind: Kind::Snow,
        coverage: Coverage::Multi,
        hours: 168,
    },
    Spec {
        catalog_id: "G02524",
        key: "set_wind_point_overlay__county",
        kind: Kind::Wind,
        coverage: Coverage::County,
        hours: 24,
    },
    Spec {
        catalog_id: "G02525",
        key: "set_wind_point_overlay__region",
        kind: Kind::Wind,
        coverage: Coverage::Region,
        hours: 24,
    },
    Spec {
        catalog_id: "G02526",
        key: "set_wind_point_overlay__state",
        kind: Kind::Wind,
        coverage: Coverage::State,
        hours: 24,
    },
    Spec {
        catalog_id: "G02527",
        key: "set_wind_point_overlay__48h_county",
        kind: Kind::Wind,
        coverage: Coverage::County,
        hours: 48,
    },
    Spec {
        catalog_id: "G02528",
        key: "set_wind_point_overlay__48h_region",
        kind: Kind::Wind,
        coverage: Coverage::Region,
        hours: 48,
    },
    Spec {
        catalog_id: "G02529",
        key: "set_wind_point_overlay__48h_state",
        kind: Kind::Wind,
        coverage: Coverage::State,
        hours: 48,
    },
    Spec {
        catalog_id: "G02530",
        key: "set_wind_point_overlay__72h_county",
        kind: Kind::Wind,
        coverage: Coverage::County,
        hours: 72,
    },
    Spec {
        catalog_id: "G02531",
        key: "set_wind_point_overlay__72h_region",
        kind: Kind::Wind,
        coverage: Coverage::Region,
        hours: 72,
    },
    Spec {
        catalog_id: "G02532",
        key: "set_wind_point_overlay__72h_state",
        kind: Kind::Wind,
        coverage: Coverage::State,
        hours: 72,
    },
    Spec {
        catalog_id: "G02533",
        key: "set_wind_point_overlay__168h_county",
        kind: Kind::Wind,
        coverage: Coverage::County,
        hours: 168,
    },
    Spec {
        catalog_id: "G02534",
        key: "set_wind_point_overlay__168h_region",
        kind: Kind::Wind,
        coverage: Coverage::Region,
        hours: 168,
    },
    Spec {
        catalog_id: "G02535",
        key: "set_wind_point_overlay__168h_state",
        kind: Kind::Wind,
        coverage: Coverage::State,
        hours: 168,
    },
    Spec {
        catalog_id: "G02608",
        key: "set_hail_point_overlay__county",
        kind: Kind::Hail,
        coverage: Coverage::County,
        hours: 24,
    },
    Spec {
        catalog_id: "G02609",
        key: "set_hail_point_overlay__region",
        kind: Kind::Hail,
        coverage: Coverage::Region,
        hours: 24,
    },
    Spec {
        catalog_id: "G02610",
        key: "set_hail_point_overlay__state",
        kind: Kind::Hail,
        coverage: Coverage::State,
        hours: 24,
    },
    Spec {
        catalog_id: "G02611",
        key: "set_hail_point_overlay__48h_county",
        kind: Kind::Hail,
        coverage: Coverage::County,
        hours: 48,
    },
    Spec {
        catalog_id: "G02612",
        key: "set_hail_point_overlay__48h_region",
        kind: Kind::Hail,
        coverage: Coverage::Region,
        hours: 48,
    },
    Spec {
        catalog_id: "G02613",
        key: "set_hail_point_overlay__48h_state",
        kind: Kind::Hail,
        coverage: Coverage::State,
        hours: 48,
    },
    Spec {
        catalog_id: "G02614",
        key: "set_hail_point_overlay__72h_county",
        kind: Kind::Hail,
        coverage: Coverage::County,
        hours: 72,
    },
    Spec {
        catalog_id: "G02615",
        key: "set_hail_point_overlay__72h_region",
        kind: Kind::Hail,
        coverage: Coverage::Region,
        hours: 72,
    },
    Spec {
        catalog_id: "G02616",
        key: "set_hail_point_overlay__72h_state",
        kind: Kind::Hail,
        coverage: Coverage::State,
        hours: 72,
    },
    Spec {
        catalog_id: "G02617",
        key: "set_hail_point_overlay__168h_county",
        kind: Kind::Hail,
        coverage: Coverage::County,
        hours: 168,
    },
    Spec {
        catalog_id: "G02618",
        key: "set_hail_point_overlay__168h_region",
        kind: Kind::Hail,
        coverage: Coverage::Region,
        hours: 168,
    },
    Spec {
        catalog_id: "G02619",
        key: "set_hail_point_overlay__168h_state",
        kind: Kind::Hail,
        coverage: Coverage::State,
        hours: 168,
    },
];

fn kind_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Rain => "rain",
        Kind::Snow => "snow",
        Kind::Wind => "wind",
        Kind::Hail => "hail",
    }
}
fn coverage_key(coverage: Coverage) -> &'static str {
    match coverage {
        Coverage::County => "county",
        Coverage::Region => "region",
        Coverage::State => "state",
        Coverage::Multi => "multi",
    }
}
fn prefix(spec: Spec) -> String {
    let base = format!(
        "obs.{}.{}",
        kind_key(spec.kind),
        coverage_key(spec.coverage)
    );
    if spec.hours == 24 {
        base
    } else {
        format!("{base}.{}h", spec.hours)
    }
}
fn title(kind: Kind, hours: u16) -> String {
    let product = match kind {
        Kind::Rain => "NWS LSR Rain Reports",
        Kind::Snow => "NWS LSR Snow Reports",
        Kind::Wind => "NWS LSR Wind Gust Reports",
        Kind::Hail => "NWS LSR Hail Reports",
    };
    format!(
        "{product} • Past {}h",
        if hours == 168 { 168 } else { hours }
    )
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let spec = *SPECS.iter().find(|spec| spec.key == key)?;
    let prefix = prefix(spec);
    let mut doc = super::product_overlay::observed_points_template(
        &title(spec.kind, spec.hours),
        "Observed",
        &prefix,
    );
    doc.template = Some(key.to_owned());
    Some(doc)
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(spec) = SPECS
        .iter()
        .find(|spec| Some(spec.key) == doc.template.as_deref())
    else {
        return;
    };
    // Reuse the existing observed-report sample adapter so point values,
    // geometry and source labeling follow the production scene schema.
    let stem = match spec.kind {
        Kind::Rain => "set_rain_leaderboard",
        Kind::Snow => "set_snow_observed",
        Kind::Wind => "set_peak_gusts",
        Kind::Hail => "set_largest_hail",
    };
    let period = if spec.hours == 24 {
        "__".to_owned()
    } else {
        format!("__{}h_", spec.hours)
    };
    let wanted = format!("{stem}{period}{}", coverage_key(spec.coverage));
    let wanted_prefix = format!("{wanted}_");
    let Some(alias) = super::observed::SPECS
        .iter()
        .map(|s| s.key)
        .find(|key| *key == wanted.as_str() || key.starts_with(&wanted_prefix))
    else {
        return;
    };
    let requested = doc.template.clone();
    doc.template = Some(alias.to_owned());
    super::observed::sample(doc);
    doc.template = requested;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snow_catalog_ids_are_unique_and_contiguous_by_period() {
        let snow: Vec<_> = SPECS.iter().filter(|s| s.kind == Kind::Snow).collect();
        assert_eq!(snow.len(), 16);
        let ids: Vec<_> = snow
            .iter()
            .map(|s| {
                s.catalog_id[1..]
                    .parse::<u32>()
                    .expect("numeric catalog id")
            })
            .collect();
        assert_eq!(ids, (3468..=3483).collect::<Vec<_>>());
    }

    #[test]
    fn sample_adapter_produces_real_point_scene_payload_and_sample_source_for_every_key() {
        let _ = crate::map::boundaries::counties_for_tests();
        for spec in SPECS {
            let mut doc = build(spec.key).expect("registered template");
            sample(&mut doc);
            let values: std::collections::HashMap<_, _> =
                doc.data_overrides.iter().cloned().collect();
            let p = prefix(*spec);
            let raw = values
                .get(&format!("set.{p}.payload"))
                .expect("sample payload");
            let scene =
                super::board::parse(raw, super::board::Layout::Single).expect("sample scene");
            assert!(
                scene.panels[0]
                    .points
                    .as_ref()
                    .is_some_and(|points| !points.is_empty()),
                "{}",
                spec.key
            );
            assert_eq!(
                values.get("set.obs.source").map(String::as_str),
                Some("SAMPLE ONLY • Not for broadcast")
            );
        }
    }
}
