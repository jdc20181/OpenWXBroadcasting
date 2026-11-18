//! Native social alert posts, with official product text and conservative county relevance.
use super::{
    board,
    observed::{self, Coverage},
    reference::{frame, label, lowerthird_frame, social_frame, MISSING},
};
use crate::{
    config::Settings,
    data::{Alert, WxSnapshot},
    gfx::{
        bind::Bindings,
        model::{AnimKind, Fill, GraphicDoc, Layer},
        text::Align,
    },
};
#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    coverage: Coverage,
    format: u8,
}
const SPECS: &[Spec] = &[
    Spec {
        key: "set_heat_alert__county",
        coverage: Coverage::County,
        format: 0,
    },
    Spec {
        key: "set_heat_alert__region",
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_heat_alert__state",
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_heat_alert__county_portrait",
        coverage: Coverage::County,
        format: 1,
    },
    Spec {
        key: "set_heat_alert__region_portrait",
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_heat_alert__state_portrait",
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_heat_alert__county_square",
        coverage: Coverage::County,
        format: 2,
    },
    Spec {
        key: "set_heat_alert__region_square",
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_heat_alert__state_square",
        coverage: Coverage::State,
        format: 2,
    },
    Spec {
        key: "set_heat_alert__county_lowerthird",
        coverage: Coverage::County,
        format: 3,
    },
    Spec {
        key: "set_heat_alert__region_lowerthird",
        coverage: Coverage::Region,
        format: 3,
    },
    Spec {
        key: "set_heat_alert__state_lowerthird",
        coverage: Coverage::State,
        format: 3,
    },
    Spec {
        key: "set_social_alert__county",
        coverage: Coverage::County,
        format: 0,
    },
    Spec {
        key: "set_social_alert__region",
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_social_alert__state",
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_social_alert__county_portrait",
        coverage: Coverage::County,
        format: 1,
    },
    Spec {
        key: "set_social_alert__region_portrait",
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_social_alert__state_portrait",
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_social_alert__county_square",
        coverage: Coverage::County,
        format: 2,
    },
    Spec {
        key: "set_social_alert__region_square",
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_social_alert__state_square",
        coverage: Coverage::State,
        format: 2,
    },
];
fn rank(a: &Alert) -> (u8, u8) {
    let severity = match a.severity.as_str() {
        "Extreme" => 0,
        "Severe" => 1,
        "Moderate" => 2,
        "Minor" => 3,
        _ => 4,
    };
    let urgency = match a.urgency.as_str() {
        "Immediate" => 0,
        "Expected" => 1,
        "Future" => 2,
        _ => 3,
    };
    (severity, urgency)
}
fn stamp(s: &Option<String>) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s.as_deref()?)
        .ok()
        .map(|t| t.timestamp())
}
fn selected<'a>(data: &'a WxSnapshot, counties: &[String], now: i64) -> Option<&'a Alert> {
    selected_kind(data, counties, now, false)
}
fn selected_kind<'a>(
    data: &'a WxSnapshot,
    counties: &[String],
    now: i64,
    heat: bool,
) -> Option<&'a Alert> {
    super::winter_alerts::active(data, now)
        .into_iter()
        .filter(|a| {
            data.map_alerts.iter().any(|m| m.id == a.id)
                && matches!(a.message_type.as_str(), "Alert" | "Update")
                && !a.event.trim().is_empty()
                && a.event.len() <= 160
                && a.fips.iter().any(|f| counties.contains(f))
                && (!heat
                    || matches!(
                        a.event.as_str(),
                        "Heat Advisory" | "Extreme Heat Warning" | "Excessive Heat Warning"
                    ))
        })
        .min_by(|a, b| {
            rank(a)
                .cmp(&rank(b))
                .then_with(|| stamp(&b.sent).cmp(&stamp(&a.sent)))
                .then_with(|| a.id.cmp(&b.id))
        })
}
fn clean(s: &str, max: usize) -> Option<String> {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    (!s.is_empty() && s.len() <= max).then_some(s)
}
fn values(a: Option<&Alert>, area: &str) -> Vec<(&'static str, String)> {
    let mut v = vec![
        ("source", "Source: National Weather Service".into()),
        ("event", MISSING.into()),
        ("headline", String::new()),
        ("area", String::new()),
        ("expires", "Current product not available yet".into()),
        ("action", String::new()),
        ("note", "Unavailable does not establish an all-clear".into()),
    ];
    let mut put = |k, value: String| {
        v.iter_mut().find(|(f, _)| *f == k).unwrap().1 = value;
    };
    let Some(a) = a else {
        return v;
    };
    put("event", a.event.clone());
    put("headline", clean(&a.headline, 700).unwrap_or_default());
    put("area", area.into());
    if let Some(t) = stamp(&a.expires).and_then(|t| chrono::DateTime::from_timestamp(t, 0)) {
        put(
            "expires",
            format!("Expires {} UTC", t.format("%b %d • %H:%M")),
        );
    }
    put("action", "Read the full NWS alert for instructions.".into());
    put(
        "note",
        "One available product • Additional alerts may apply".into(),
    );
    v
}
fn prefix(s: Spec) -> String {
    format!(
        "set.{}.{}",
        if is_heat(s) {
            "heatalert"
        } else {
            "socialalert"
        },
        match s.coverage {
            Coverage::County => "county",
            Coverage::Region => "region",
            _ => "state",
        }
    )
}
fn is_heat(s: Spec) -> bool {
    s.key.starts_with("set_heat_alert__")
}
pub(super) fn bind(b: &mut Bindings, data: &WxSnapshot, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
    let all = crate::map::boundaries::counties_named();
    for s in SPECS.iter().filter(|s| s.format == 0) {
        let scope = observed::report_scope(settings, s.coverage);
        let counties = all.as_ref().map(|all| board::selected(all, &scope));
        let ids: Vec<_> = counties
            .as_ref()
            .map(|cs| cs.iter().map(|c| c.fips.clone()).collect())
            .unwrap_or_default();
        let a = if is_heat(*s) {
            selected_kind(data, &ids, now, true)
        } else {
            selected(data, &ids, now)
        };
        let names: Vec<_> = counties
            .as_ref()
            .map(|cs| {
                cs.iter()
                    .filter(|c| a.is_some_and(|a| a.fips.contains(&c.fips)))
                    .map(|c| {
                        format!(
                            "{}, {}",
                            c.name,
                            crate::winter::STATES
                                .iter()
                                .find(|s| s.0 == c.state)
                                .map(|s| s.1)
                                .unwrap_or("US")
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let area = if names.is_empty() {
            String::new()
        } else {
            format!(
                "Includes parts of {}{}",
                names.iter().take(3).cloned().collect::<Vec<_>>().join(", "),
                if names.len() > 3 {
                    format!(" and {} other selected counties", names.len() - 3)
                } else {
                    "".into()
                }
            )
        };
        let p = prefix(*s);
        for (k, v) in values(a, &area) {
            b.set(&format!("{p}.{k}"), &v);
        }
        b.set(
            &format!("{p}.scope"),
            &match s.coverage {
                Coverage::County => format!("{} • County coverage", settings.home_label),
                Coverage::Region => "Regional coverage".into(),
                _ => "State coverage".into(),
            },
        );
    }
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let s = *SPECS.iter().find(|s| s.key == key)?;
    let p = prefix(s);
    let token = |f: &str| format!("{{{{{p}.{f}}}}}");
    let title = if is_heat(s) {
        "Available NWS Heat Alert"
    } else {
        "Available NWS Alert"
    };
    let category = if is_heat(s) { "Heat" } else { "Severe Weather" };
    if s.format == 3 {
        let mut d = lowerthird_frame(title, category, &token("expires"), &token("source"));
        d.layers.push(
            Layer::rect(
                "Heat alert content band",
                0.,
                865.,
                1920.,
                131.,
                Fill::solid("$panel"),
                0.,
            )
            .with_anim(AnimKind::Rise, 0.12, 0.4),
        );
        for (name, field, box_, size, color) in [
            (
                "Compact official event",
                "event",
                (80., 872., 1200., 55.),
                44.,
                "$text",
            ),
            (
                "Compact included counties",
                "area",
                (80., 931., 1200., 34.),
                24.,
                "$muted",
            ),
            (
                "Compact source instructions",
                "action",
                (1340., 873., 500., 88.),
                27.,
                "$text",
            ),
            (
                "Compact coverage caveat",
                "note",
                (80., 969., 1760., 23.),
                20.,
                "$muted",
            ),
        ] {
            label(&mut d, name, box_, &token(field), size, color, Align::Left);
        }
        return Some(d);
    }
    let mut d = if s.format == 0 {
        frame(title, category, &token("expires"), &token("source"))
    } else {
        social_frame(
            title,
            category,
            &token("expires"),
            &token("source"),
            s.format == 1,
        )
    };
    let (x, w, scope_y, ey, eh, hy, hh, ay, ah, cy, ch, ny) = match s.format {
        1 => (
            54., 972., 270., 380., 250., 710., 340., 1130., 210., 1450., 120., 1670.,
        ),
        2 => (
            54., 972., 210., 290., 125., 435., 190., 665., 105., 790., 90., 920.,
        ),
        _ => (
            80., 1760., 210., 305., 135., 475., 190., 710., 85., 825., 65., 930.,
        ),
    };
    d.layers.push(
        crate::gfx::templates::panel(
            "Alert product panel",
            x - 12.,
            ey - 20.,
            w + 24.,
            ay + ah - ey + 40.,
        )
        .with_anim(AnimKind::Rise, 0.12, 0.45),
    );
    for (name, field, box_, size, color) in [
        (
            "Alert scope",
            "scope",
            (x, scope_y, w, 50.),
            if s.format == 0 { 35. } else { 29. },
            "$muted",
        ),
        (
            "Official event",
            "event",
            (x + 20., ey, w - 40., eh),
            if s.format == 1 {
                80.
            } else if s.format == 2 {
                58.
            } else {
                93.
            },
            "$text",
        ),
        (
            "Official headline",
            "headline",
            (x + 20., hy, w - 40., hh),
            if s.format == 1 {
                45.
            } else if s.format == 2 {
                34.
            } else {
                50.
            },
            "$text",
        ),
        (
            "Included counties",
            "area",
            (x + 20., ay, w - 40., ah),
            if s.format == 1 {
                37.
            } else if s.format == 2 {
                27.
            } else {
                34.
            },
            "$muted",
        ),
        (
            "Full instructions",
            "action",
            (x, cy, w, ch),
            if s.format == 1 {
                37.
            } else if s.format == 2 {
                27.
            } else {
                35.
            },
            "$text",
        ),
        (
            "Coverage caveat",
            "note",
            (x, ny, w, 40.),
            if s.format == 2 { 22. } else { 25. },
            "$muted",
        ),
    ] {
        label(&mut d, name, box_, &token(field), size, color, Align::Left);
    }
    Some(d)
}
#[cfg(test)]
pub(super) fn sample(d: &mut GraphicDoc) {
    let Some(s) = SPECS.iter().find(|s| Some(s.key) == d.template.as_deref()) else {
        return;
    };
    let p = prefix(*s);
    let event = if is_heat(*s) {
        "Extreme Heat Warning"
    } else {
        "Severe Thunderstorm Warning"
    };
    let headline = if is_heat(*s) {
        "SAMPLE • An extreme heat warning includes Jeffersonville and surrounding communities."
    } else {
        "SAMPLE • A severe thunderstorm warning includes Jeffersonville and surrounding communities."
    };
    for (k, v) in [
        ("source", "SAMPLE ONLY • Not for broadcast"),
        ("scope", "SAMPLE • Selected coverage"),
        ("event", event),
        ("headline", headline),
        (
            "area",
            "SAMPLE • Includes parts of Clark, Floyd and Harrison counties",
        ),
        ("expires", "SAMPLE • Expires Nov 14 • 22:30 UTC"),
        ("action", "Read the full NWS alert for instructions."),
        (
            "note",
            "SAMPLE • One available product • Additional alerts may apply",
        ),
    ] {
        d.data_overrides.push((format!("{p}.{k}"), v.into()));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn alert(id: &str, event: &str, severity: &str) -> Alert {
        Alert {
            id: id.into(),
            event: event.into(),
            severity: severity.into(),
            urgency: "Immediate".into(),
            sent: Some("2026-11-13T12:00:00Z".into()),
            expires: Some("2026-11-13T14:00:00Z".into()),
            message_type: "Alert".into(),
            fips: vec!["18019".into()],
            ..Default::default()
        }
    }
    #[test]
    fn social_card_requires_actual_map_id_and_selected_county() {
        let a = alert("sample1", "Tornado Warning", "Extreme");
        let mut d = WxSnapshot::default();
        d.local_alerts.push(a.clone());
        let now = 1794574800;
        assert!(selected(&d, &["18019".into()], now).is_none());
        d.map_alerts.push(a);
        assert!(selected(&d, &["18019".into()], now).is_some());
        assert!(selected(&d, &["18037".into()], now).is_none());
        assert!(selected(&d, &["18019".into()], now + 7200).is_none());
    }
    #[test]
    fn priority_and_cancellation_preserve_official_product_status() {
        let mut d = WxSnapshot::default();
        d.map_alerts = vec![
            alert("minor", "Flood Advisory", "Minor"),
            alert("severe", "Severe Thunderstorm Warning", "Severe"),
        ];
        let now = 1794574800;
        assert_eq!(selected(&d, &["18019".into()], now).unwrap().id, "severe");
        let mut cancel = d.map_alerts[1].clone();
        cancel.message_type = "Cancel".into();
        cancel.sent = Some("2026-11-13T12:30:00Z".into());
        d.local_alerts.push(cancel);
        assert_eq!(selected(&d, &["18019".into()], now).unwrap().id, "minor");
        assert!(values(None, "").contains(&("event", MISSING.into())));
    }
    #[test]
    fn heat_selection_filters_before_ranking_and_never_infers_from_temperature() {
        let now = 1794574800;
        let mut d = WxSnapshot::default();
        d.map_alerts = vec![
            alert("tornado", "Tornado Warning", "Extreme"),
            alert("heat", "Heat Advisory", "Moderate"),
        ];
        assert_eq!(
            selected_kind(&d, &["18019".into()], now, true).unwrap().id,
            "heat"
        );
        d.map_alerts[1].event = "Extreme Heat Watch".into();
        assert!(selected_kind(&d, &["18019".into()], now, true).is_none());
        d.map_alerts[1].event = "Extreme Heat Warning".into();
        assert!(selected_kind(&d, &["18019".into()], now, true).is_some());
        d.map_alerts[1].event = "Excessive Heat Warning".into();
        assert!(selected_kind(&d, &["18019".into()], now, true).is_some());
        assert!(selected_kind(&d, &["18037".into()], now, true).is_none());
        assert!(selected_kind(&d, &["18019".into()], now + 7200, true).is_none());
    }
}
