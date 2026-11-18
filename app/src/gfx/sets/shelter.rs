//! County tornado-warning callouts. Only IDs present in the actual-alert map feed
//! can activate the graphic; local point alerts alone cannot trigger it.
use super::{
    comparison,
    reference::{frame, label, lowerthird_frame, social_frame, MISSING},
    winter_alerts,
};
use crate::{
    config::Settings,
    data::{Alert, WxSnapshot},
    gfx::{
        bind::Bindings,
        model::{AnimKind, Fill, GraphicDoc, Layer},
        templates::panel,
        text::Align,
    },
};

const SPECS: &[(&str, u8)] = &[
    ("set_shelter_now", 0),
    ("set_shelter_now__portrait", 1),
    ("set_shelter_now__square", 2),
    ("set_shelter_now__lowerthird", 3),
];
const GUIDANCE: &str =
    "Go to an interior room on the lowest floor of a sturdy building. Avoid windows.";

fn selected<'a>(data: &'a WxSnapshot, county: &str, now: i64) -> Option<&'a Alert> {
    // net::map_alerts explicitly requests status=actual. The point feed lacks that
    // explicit filter and Alert drops CAP status, so it cannot introduce new IDs.
    winter_alerts::active(data, now)
        .into_iter()
        .filter(|a| {
            data.map_alerts.iter().any(|m| m.id == a.id)
                && a.event == "Tornado Warning"
                && matches!(a.message_type.as_str(), "Alert" | "Update")
                && !a.county_based
                && a.fips.iter().any(|f| f == county)
                && a.polygons.iter().any(|r| {
                    r.len() >= 3
                        && r.len() <= 4096
                        && r.iter().all(|&(x, y)| {
                            x.is_finite()
                                && y.is_finite()
                                && (-180. ..=180.).contains(&x)
                                && (-85. ..=85.).contains(&y)
                        })
                })
        })
        .min_by_key(|a| a.expires.as_deref())
}

fn put(b: &mut Bindings, alert: Option<&Alert>, area: &str) {
    let active = alert.is_some();
    b.set(
        "set.shelter.title",
        if active {
            "Tornado Warning"
        } else {
            "Warning Status"
        },
    );
    b.set(
        "set.shelter.action",
        if active { "Take shelter now" } else { MISSING },
    );
    b.set(
        "set.shelter.area",
        if active {
            area
        } else {
            "Current county warning status"
        },
    );
    let valid = alert
        .and_then(|a| a.expires.as_ref())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| {
            format!(
                "Warning expires {}",
                t.with_timezone(&chrono::Utc).format("%b %-d, %Y %H:%M UTC")
            )
        })
        .unwrap_or_else(|| "Current warning not available yet".into());
    b.set("set.shelter.valid", &valid);
    b.set("set.shelter.guide", if active { GUIDANCE } else { "" });
    b.set(
        "set.shelter.note",
        if active {
            "For the warned part of the county. Mobile homes and vehicles: use a sturdy shelter."
        } else {
            "Check current NWS warnings for your location."
        },
    );
    b.set("set.shelter.source", "Source: National Weather Service");
}

pub(super) fn bind(b: &mut Bindings, data: &WxSnapshot, settings: &Settings) {
    let county = comparison::scope(settings, super::board::Layout::Zones).county;
    let name = county.as_ref().and_then(|f| {
        crate::map::boundaries::counties_named()
            .and_then(|all| all.iter().find(|c| &c.fips == f).map(|c| c.name.clone()))
    });
    let area = format!(
        "Warned parts of {}",
        name.as_deref().unwrap_or("the selected county")
    );
    let alert = county
        .as_ref()
        .and_then(|f| selected(data, f, chrono::Utc::now().timestamp()));
    put(b, alert, &area);
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let format = SPECS.iter().find(|s| s.0 == key)?.1;
    let (title, source) = ("{{set.shelter.title}}", "{{set.shelter.source}}");
    let mut d = match format {
        1 | 2 => social_frame(title, "Severe", "{{set.shelter.area}}", source, format == 1),
        3 => lowerthird_frame(title, "Severe", "{{set.shelter.area}}", source),
        _ => frame(title, "Severe", "{{set.shelter.area}}", source),
    };
    if format == 3 {
        d.layers.push(
            Layer::rect(
                "Shelter instructions band",
                0.,
                865.,
                1920.,
                131.,
                Fill::solid("$panel"),
                0.,
            )
            .with_anim(AnimKind::Rise, 0.2, 0.4),
        );
        label(
            &mut d,
            "Shelter action",
            (80., 875., 730., 70.),
            "{{set.shelter.action}}",
            56.,
            "$text",
            Align::Left,
        );
        label(
            &mut d,
            "Shelter guidance",
            (850., 875., 990., 70.),
            "{{set.shelter.guide}}",
            30.,
            "$text",
            Align::Left,
        );
        label(
            &mut d,
            "Warning valid time",
            (80., 950., 1760., 38.),
            "{{set.shelter.valid}}",
            26.,
            "$muted",
            Align::Left,
        );
        return Some(d);
    }
    let (panel_box, action, area, valid, guide, note) = match format {
        1 => (
            (54., 270., 972., 1310.),
            (80., 320., 920., 230.),
            (80., 610., 920., 180.),
            (80., 820., 920., 110.),
            (80., 1000., 920., 380.),
            (54., 1598., 972., 120.),
        ),
        2 => (
            (54., 230., 972., 620.),
            (80., 250., 920., 130.),
            (80., 420., 920., 100.),
            (80., 530., 920., 60.),
            (80., 620., 920., 180.),
            (54., 880., 972., 80.),
        ),
        _ => (
            (80., 230., 1760., 640.),
            (130., 270., 1660., 150.),
            (130., 440., 1660., 90.),
            (130., 550., 1660., 60.),
            (130., 645., 1660., 150.),
            (80., 910., 1760., 55.),
        ),
    };
    d.layers.push(
        panel(
            "Shelter instructions",
            panel_box.0,
            panel_box.1,
            panel_box.2,
            panel_box.3,
        )
        .with_anim(AnimKind::Rise, 0.2, 0.5),
    );
    for (name, box_, field, size, color) in [
        (
            "Shelter action",
            action,
            "action",
            if format == 2 { 91. } else { 118. },
            "$text",
        ),
        (
            "Warned area",
            area,
            "area",
            if format == 1 { 49. } else { 40. },
            "$primary",
        ),
        (
            "Warning expires",
            valid,
            "valid",
            if format == 2 { 28. } else { 34. },
            "$muted",
        ),
        (
            "Shelter guidance",
            guide,
            "guide",
            if format == 1 { 54. } else { 40. },
            "$text",
        ),
        (
            "Shelter note",
            note,
            "note",
            if format == 1 { 31. } else { 27. },
            "$muted",
        ),
    ] {
        label(
            &mut d,
            name,
            box_,
            &format!("{{{{set.shelter.{field}}}}}"),
            size,
            color,
            Align::Left,
        );
    }
    Some(d)
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    if !SPECS.iter().any(|s| Some(s.0) == doc.template.as_deref()) {
        return;
    }
    let now = chrono::Utc::now();
    let alert = Alert {
        expires: Some((now + chrono::Duration::minutes(30)).to_rfc3339()),
        ..Default::default()
    };
    let mut b = Bindings::default();
    put(
        &mut b,
        Some(&alert),
        "SAMPLE • Warned parts of Jeffersonville",
    );
    b.set("set.shelter.title", "Tornado Warning");
    b.set("set.shelter.source", "SAMPLE ONLY • Not for broadcast");
    b.set(
        "set.shelter.note",
        "SAMPLE • Synthetic warning callout for layout review.",
    );
    for (k, v) in b.values() {
        doc.data_overrides.push((k.to_string(), v.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callout_requires_actual_map_id_current_warning_and_county_geometry() {
        let now = chrono::Utc::now();
        let a = Alert {
            id: "warning".into(),
            event: "Tornado Warning".into(),
            message_type: "Alert".into(),
            sent: Some((now - chrono::Duration::minutes(1)).to_rfc3339()),
            expires: Some((now + chrono::Duration::minutes(30)).to_rfc3339()),
            fips: vec!["18037".into()],
            polygons: vec![vec![(-87., 38.), (-86., 38.), (-86., 39.), (-87., 38.)]],
            ..Default::default()
        };
        let mut data = WxSnapshot::default();
        data.local_alerts.push(a.clone());
        assert!(selected(&data, "18037", now.timestamp()).is_none());
        data.map_alerts.push(a.clone());
        assert!(selected(&data, "18037", now.timestamp()).is_some());
        assert!(selected(&data, "18019", now.timestamp()).is_none());
        data.local_alerts[0].message_type = "Cancel".into();
        data.local_alerts[0].sent = Some(now.to_rfc3339());
        assert!(selected(&data, "18037", now.timestamp()).is_none());
        data.local_alerts.clear();
        data.map_alerts[0].county_based = true;
        assert!(selected(&data, "18037", now.timestamp()).is_none());
        data.map_alerts[0].county_based = false;
        data.map_alerts[0].expires = Some((now - chrono::Duration::seconds(1)).to_rfc3339());
        assert!(selected(&data, "18037", now.timestamp()).is_none());
    }
}
