//! Winter hazard map variants, built only from the existing NWS alert snapshot.
use super::{
    board::{self, Band, Layout, Scene, Zone},
    comparison,
    observed::{self, Coverage},
};
use crate::{
    config::Settings,
    data::{Alert, WxSnapshot},
    gfx::{bind::Bindings, model::GraphicDoc},
};
use std::collections::BTreeMap;

pub(super) const SPECS: &[(&str, Coverage)] = &[
    ("set_winter_alerts__county_overlay", Coverage::County),
    ("set_winter_alerts__region_overlay", Coverage::Region),
    ("set_winter_alerts__state_overlay", Coverage::State),
    ("set_winter_alerts__multi_overlay", Coverage::Multi),
    ("set_winter_alerts__county_square", Coverage::County),
    ("set_winter_alerts__region_square", Coverage::Region),
    ("set_winter_alerts__state_square", Coverage::State),
    ("set_winter_alerts__multi_square", Coverage::Multi),
    ("set_winter_alerts__county_portrait", Coverage::County),
    ("set_winter_alerts__region_portrait", Coverage::Region),
    ("set_winter_alerts__state_portrait", Coverage::State),
    ("set_winter_alerts__multi_portrait", Coverage::Multi),
    ("set_winter_alerts__county", Coverage::County),
    ("set_winter_alerts__region", Coverage::Region),
    ("set_winter_alerts__state", Coverage::State),
    ("set_winter_alerts__multi", Coverage::Multi),
];
fn prefix(key: &str) -> String {
    key.strip_suffix("_portrait")
        .or_else(|| key.strip_suffix("_square"))
        .or_else(|| key.strip_suffix("_overlay"))
        .unwrap_or(key)
        .replace("set_winter_alerts__", "winteralerts.")
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    SPECS.iter().find(|s| s.0 == key)?;
    if key.ends_with("_overlay") {
        return Some(super::product_overlay::template("Winter Alerts", "Winter", &prefix(key)));
    }
    if key.ends_with("_portrait") {
        return Some(comparison::portrait_template(
            "Winter Alerts",
            "Winter",
            &prefix(key),
        ));
    }
    if key.ends_with("_square") {
        return Some(comparison::square_template(
            "Winter Alerts",
            "Winter",
            &prefix(key),
        ));
    }
    Some(comparison::template(
        "Winter Alerts",
        "Winter",
        &prefix(key),
        Layout::Single,
    ))
}
fn category(event: &str) -> Option<usize> {
    match event {
        "Winter Weather Advisory" | "Freezing Rain Advisory" | "Freezing Fog Advisory" => Some(0),
        "Winter Storm Watch" | "Blizzard Watch" | "Lake Effect Snow Watch" => Some(1),
        "Winter Storm Warning"
        | "Blizzard Warning"
        | "Ice Storm Warning"
        | "Lake Effect Snow Warning"
        | "Heavy Snow Warning" => Some(2),
        "Snow Squall Warning" => Some(3),
        _ => None,
    }
}
fn timestamp(value: &Option<String>) -> Option<i64> {
    value
        .as_ref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.timestamp())
}
/// Current snapshot versions. Each graphic filters its own official product names.
pub(super) fn active(data: &WxSnapshot, now: i64) -> Vec<&Alert> {
    let mut latest: BTreeMap<&str, &Alert> = BTreeMap::new();
    for a in data.map_alerts.iter().chain(&data.local_alerts) {
        if a.id.trim().is_empty() {
            continue;
        }
        match latest.get(a.id.as_str()) {
            Some(old)
                if timestamp(&old.sent) > timestamp(&a.sent)
                    || (timestamp(&old.sent) == timestamp(&a.sent)
                        && old.message_type.eq_ignore_ascii_case("Cancel")) => {}
            _ => {
                latest.insert(&a.id, a);
            }
        }
    }
    let mut alerts: Vec<_> = latest
        .into_values()
        .filter(|a| {
            !a.message_type.eq_ignore_ascii_case("Cancel")
                && timestamp(&a.expires).is_some_and(|t| t > now)
                && timestamp(&a.sent).is_some_and(|t| t <= now)
        })
        .collect();
    alerts.sort_by_key(|a| (category(&a.event), a.id.as_str()));
    alerts
}
fn scene(data: &WxSnapshot, scope: board::Scope, now: i64) -> Scene {
    let mut scene = board::empty(Layout::Single);
    scene.scope = scope;
    scene.legend_title =
        "NWS winter alert level • Highest available level shown in overlaps".into();
    scene.legend = [
        ("Winter advisory", "Winter Weather Advisory"),
        ("Winter watch", "Winter Storm Watch"),
        ("Winter warning", "Winter Storm Warning"),
        ("Snow squall warning", "Snow Squall Warning"),
    ]
    .into_iter()
    .map(|(label, event)| Band {
        label: label.into(),
        color: Alert {
            event: event.into(),
            ..Default::default()
        }
        .color()
        .into(),
    })
    .collect();
    let p = &mut scene.panels[0];
    p.title = "Available winter alerts".into();
    p.valid = chrono::DateTime::from_timestamp(now, 0)
        .unwrap()
        .format("As of %b %-d, %Y %H:%M UTC")
        .to_string();
    p.detail = "Winter warnings include snow, ice and blizzard products".into();
    let Some(all) = crate::map::boundaries::counties_named() else {
        return scene;
    };
    let counties = board::selected(&all, &scene.scope);
    for a in active(data, now) {
        let Some(i) = category(&a.event) else {
            continue;
        };
        let fips: Vec<_> = counties
            .iter()
            .filter(|c| a.fips.contains(&c.fips))
            .map(|c| c.fips.clone())
            .collect();
        let zones: Vec<_> = a
            .polygons
            .iter()
            .filter(|r| {
                r.len() >= 3
                    && r.len() <= 4096
                    && r.iter().all(|&(x, y)| {
                        x.is_finite()
                            && y.is_finite()
                            && (-180. ..=180.).contains(&x)
                            && (-85. ..=85.).contains(&y)
                    })
                    && counties.iter().any(|c| {
                        let (x, y) = board::centre(c);
                        crate::gfx::winter_map::inside_pub(r, x, y)
                            || r.iter().any(|&(x, y)| board::inside(c, x, y))
                    })
            })
            .map(|ring| Zone {
                category: i,
                ring: ring.clone(),
                label: String::new(),
                label_at: None,
            })
            .collect();
        if fips.is_empty() && zones.is_empty() {
            continue;
        }
        scene.reviewed = true;
        for f in fips {
            p.counties.get_or_insert_with(Default::default).insert(f, i);
        }
        p.zones.get_or_insert_with(Default::default).extend(zones);
    }
    scene
}
pub(super) fn bind(b: &mut Bindings, data: &WxSnapshot, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
    for &(key, coverage) in SPECS {
        if key.ends_with("_portrait") || key.ends_with("_square") || key.ends_with("_overlay") {
            continue;
        }
        let prefix = prefix(key);
        let scene = scene(data, observed::report_scope(settings, coverage), now);
        b.set(
            &format!("set.{prefix}.payload"),
            &serde_json::to_string(&scene).unwrap_or_default(),
        );
        b.set(&format!("set.{prefix}.subtitle"), &settings.home_label);
        b.set(
            &format!("set.{prefix}.source"),
            "Source: National Weather Service",
        );
        b.set(&format!("set.{prefix}.note"), "Available NWS alert areas • Unfilled counties do not establish an all-clear • Cold alerts use a separate graphic.");
    }
}
#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(&(key, coverage)) = SPECS.iter().find(|s| Some(s.0) == doc.template.as_deref()) else {
        return;
    };
    let now = chrono::Utc::now().timestamp();
    let scope = observed::report_scope(&Settings::default(), coverage);
    let mut scene = scene(&WxSnapshot::default(), scope, now);
    if let Some(all) = crate::map::boundaries::counties_named() {
        let values = board::selected(&all, &scene.scope)
            .iter()
            .enumerate()
            .map(|(i, c)| (c.fips.clone(), i % 4))
            .collect();
        scene.panels[0].counties = Some(values);
        scene.reviewed = true;
    }
    let prefix = prefix(key);
    doc.data_overrides.extend([
        (
            format!("set.{prefix}.payload"),
            serde_json::to_string(&scene).unwrap(),
        ),
        (
            format!("set.{prefix}.source"),
            "SAMPLE ONLY • Not for broadcast".into(),
        ),
        (
            format!("set.{prefix}.subtitle"),
            "SAMPLE • Synthetic winter alert categories".into(),
        ),
    ]);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_expiry_and_nonwinter_products_are_excluded() {
        let now = chrono::Utc::now();
        let alert = Alert {
            id: "test".into(),
            event: "Winter Storm Warning".into(),
            sent: Some((now - chrono::Duration::minutes(5)).to_rfc3339()),
            expires: Some((now + chrono::Duration::hours(1)).to_rfc3339()),
            ..Default::default()
        };
        let mut data = WxSnapshot::default();
        data.map_alerts.push(alert.clone());
        assert_eq!(active(&data, now.timestamp()).len(), 1);
        data.local_alerts.push(Alert {
            message_type: "Cancel".into(),
            sent: Some(now.to_rfc3339()),
            ..alert.clone()
        });
        assert!(active(&data, now.timestamp()).is_empty());
        data.local_alerts.clear();
        data.map_alerts[0].expires = Some((now - chrono::Duration::seconds(1)).to_rfc3339());
        assert!(active(&data, now.timestamp()).is_empty());
        assert!(category("Tornado Warning").is_none());
        // Cold comparisons share the same version reducer: a newer cancellation
        // must replace an older warning even when it came from the other feed.
        data.map_alerts = vec![Alert {
            id: "cold".into(),
            event: "Extreme Cold Warning".into(),
            ..alert.clone()
        }];
        assert_eq!(active(&data, now.timestamp()).len(), 1);
        data.local_alerts.push(Alert {
            id: "cold".into(),
            event: "Extreme Cold Warning".into(),
            message_type: "Cancel".into(),
            sent: Some(now.to_rfc3339()),
            ..alert
        });
        assert!(active(&data, now.timestamp()).is_empty());
    }
}
