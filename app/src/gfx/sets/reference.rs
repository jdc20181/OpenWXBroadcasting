//! Owner reference layouts. All weather values default to unavailable, never a sample.
use crate::gfx::{
    bind::Bindings,
    model::{AnimKind, Fill, GraphicDoc, ImageFit, Layer, LayerKind},
    templates::{bg_gradient, footer, header, panel, text, wrapped, DISPLAY},
    text::{Align, Weight},
};
use chrono::{Datelike, Local, NaiveDate, Weekday};

pub(super) const MISSING: &str = "Data not available yet";
pub(super) const EF: [(&str, &str); 7] = [
    ("EF0", "#36CC83"),
    ("EF1", "#FFE34A"),
    ("EF2", "#FF982F"),
    ("EF3", "#F34545"),
    ("EF4", "#CD4DE6"),
    ("EF5", "#854CFF"),
    ("Unrated", "#C8D1DE"),
];

pub(super) fn frame(title: &str, category: &str, subtitle: &str, source: &str) -> GraphicDoc {
    let mut d = GraphicDoc::new(title, category, bg_gradient());
    header(&mut d, title, subtitle);
    footer(&mut d, "{{station}}", source);
    d
}

/// Native social frame, made from the shared station header/footer. Renamed layers
/// retain their phone layout when the shared landscape look restyler is applied.
pub(super) fn social_frame(
    title: &str,
    category: &str,
    subtitle: &str,
    source: &str,
    portrait: bool,
) -> GraphicDoc {
    let mut d = frame(title, category, subtitle, source);
    d.width = 1080;
    d.height = if portrait { 1920 } else { 1080 };
    for l in &mut d.layers {
        let (x, y, w, h, size) = match (l.name.as_str(), portrait) {
            ("Header", true) => (0., 0., 1080., 210., 0.),
            ("Header", false) => (0., 0., 1080., 170., 0.),
            ("Header accent", true) => (0., 210., 1080., 8., 0.),
            ("Header accent", false) => (0., 170., 1080., 8., 0.),
            ("Title", true) => (54., 30., 972., 90., 57.),
            ("Title", false) => (54., 24., 972., 72., 54.),
            ("Header right", true) => (54., 132., 972., 54., 31.),
            ("Header right", false) => (54., 108., 972., 44., 28.),
            ("Footer", true) => (0., 1750., 1080., 170., 0.),
            ("Footer", false) => (0., 980., 1080., 100., 0.),
            ("Footer accent", true) => (0., 1746., 1080., 4., 0.),
            ("Footer accent", false) => (0., 976., 1080., 4., 0.),
            ("Footer left", true) => (54., 1822., 972., 66., 38.),
            ("Footer left", false) => (54., 1032., 972., 38., 29.),
            ("Footer right", true) => (54., 1768., 972., 42., 25.),
            ("Footer right", false) => (54., 994., 972., 30., 23.),
            _ => continue,
        };
        (l.x, l.y, l.w, l.h) = (x, y, w, h);
        l.name = format!("Social {}", l.name);
        if let LayerKind::Text {
            size: font,
            align,
            letter_spacing,
            auto_fit,
            ..
        } = &mut l.kind
        {
            *font = size;
            *align = Align::Left;
            *letter_spacing = 0.;
            *auto_fit = true;
        }
    }
    d
}

/// Shared station branding for transparent broadcast lower thirds.
pub(super) fn lowerthird_frame(
    title: &str,
    category: &str,
    subtitle: &str,
    source: &str,
) -> GraphicDoc {
    let mut d = frame(title, category, subtitle, source);
    d.background = None;
    for l in &mut d.layers {
        let (x, y, w, h, font) = match l.name.as_str() {
            "Header" => (0., 760., 1920., 100., 0.),
            "Header accent" => (0., 860., 1920., 5., 0.),
            "Title" => (80., 772., 1120., 70., 50.),
            "Header right" => (1220., 790., 620., 44., 28.),
            "Footer" => (0., 1000., 1920., 80., 0.),
            "Footer accent" => (0., 996., 1920., 4., 0.),
            "Footer left" => (80., 1010., 900., 58., 32.),
            "Footer right" => (960., 1010., 880., 58., 26.),
            _ => continue,
        };
        (l.x, l.y, l.w, l.h) = (x, y, w, h);
        l.name = format!("Lower third {}", l.name);
        if let LayerKind::Text { size, auto_fit, .. } = &mut l.kind {
            *size = font;
            *auto_fit = true;
        }
    }
    d
}

pub(super) fn label(
    d: &mut GraphicDoc,
    name: &str,
    box_: (f32, f32, f32, f32),
    value: &str,
    size: f32,
    color: &str,
    align: Align,
) {
    let mut layer = wrapped(text(
        name,
        box_,
        value,
        DISPLAY,
        size,
        Weight::Bold,
        color,
        align,
    ));
    if let LayerKind::Text { vcenter, .. } = &mut layer.kind {
        *vcenter = true;
    }
    d.layers.push(layer.with_anim(AnimKind::Rise, 0.25, 0.5));
}

fn map(d: &mut GraphicDoc, box_: (f32, f32, f32, f32), payload_spec: &str) {
    d.layers.push(
        panel("Map panel", box_.0, box_.1, box_.2, box_.3).with_anim(AnimKind::Fade, 0.15, 0.5),
    );
    d.layers.push(
        Layer::risk_map(
            "Tornado locations",
            box_.0 + 18.,
            box_.1 + 18.,
            box_.2 - 36.,
            box_.3 - 36.,
            payload_spec,
        )
        .with_anim(AnimKind::Fade, 0.3, 0.5),
    );
}

pub(super) fn tally() -> GraphicDoc {
    let mut d = frame(
        "Tornadoes This Year",
        "Tornado",
        "{{set.tornado.period}}",
        "{{set.tornado.source}}",
    );
    label(
        &mut d,
        "Scope",
        (80., 192., 1160., 56.),
        "{{set.tornado.scope}}",
        36.,
        "$text",
        Align::Left,
    );
    map(&mut d, (70., 265., 1170., 586.), "set:tornado:{{set.tornado.payload}}");
    d.layers
        .push(panel("Counts panel", 1270., 192., 580., 659.).with_anim(AnimKind::Rise, 0.2, 0.5));
    d.layers.push(
        Layer::risk_map(
            "Derived tornado counts",
            1270.,
            192.,
            580.,
            659.,
            "set:tornado-summary:{{set.tornado.payload}}",
        )
        .with_anim(AnimKind::Fade, 0.3, 0.5),
    );
    // One shared legend strip outside the map. No translucent category stacking.
    for (i, (ef, color)) in EF.iter().enumerate() {
        let x = 80. + i as f32 * 165.;
        d.layers.push(
            Layer::rect(
                &format!("{ef} legend"),
                x,
                875.,
                28.,
                28.,
                Fill::solid(color),
                0.,
            )
            .with_anim(AnimKind::Fade, 0.4, 0.4),
        );
        label(
            &mut d,
            &format!("{ef} legend label"),
            (x + 38., 867., 125., 45.),
            ef,
            25.,
            "$text",
            Align::Left,
        );
    }
    label(
        &mut d,
        "Tally note",
        (80., 923., 1760., 42.),
        "{{set.tornado.note}}",
        25.,
        "$muted",
        Align::Left,
    );
    d
}

pub(super) fn confirmed() -> GraphicDoc {
    // Confirmation is a binding: a fresh template must never declare a tornado confirmed.
    let mut d = frame(
        "Tornado Survey",
        "Tornado",
        "{{set.survey.status}}",
        "{{set.tornado.source}}",
    );
    map(&mut d, (70., 210., 1130., 685.), "set:tornado:{{set.survey.payload}}");
    d.layers
        .push(panel("Survey card", 1230., 210., 620., 685.).with_anim(AnimKind::Rise, 0.2, 0.5));
    label(
        &mut d,
        "Confirmation",
        (1265., 240., 550., 85.),
        "{{set.survey.status}}",
        44.,
        "$primary",
        Align::Left,
    );
    label(
        &mut d,
        "Survey location",
        (1265., 350., 550., 110.),
        "{{set.survey.location}}",
        43.,
        "$text",
        Align::Left,
    );
    for (i, (name, key)) in [
        ("NWS rating", "rating"),
        ("Peak wind", "wind"),
        ("Survey date", "date"),
    ]
    .iter()
    .enumerate()
    {
        let y = 490. + i as f32 * 116.;
        label(
            &mut d,
            &format!("{name} label"),
            (1265., y, 550., 36.),
            name,
            25.,
            "$muted",
            Align::Left,
        );
        label(
            &mut d,
            name,
            (1265., y + 37., 550., 65.),
            &format!("{{{{set.survey.{key}}}}}"),
            37.,
            "$text",
            Align::Left,
        );
    }
    label(
        &mut d,
        "Survey note",
        (80., 919., 1760., 45.),
        "{{set.survey.note}}",
        26.,
        "$muted",
        Align::Left,
    );
    d
}

pub(super) fn ranked() -> GraphicDoc {
    let mut d = frame(
        "Top Tornado Years",
        "Tornado",
        "{{set.history.period}}",
        "{{set.tornado.source}}",
    );
    label(
        &mut d,
        "History scope",
        (80., 199., 1760., 65.),
        "{{set.tornado.scope}}",
        38.,
        "$text",
        Align::Left,
    );
    // The optional photo is a second state, keeping the ordinary table full-width.
    d.states = vec!["Table".into(), "With photo".into()];
    for photo in [false, true] {
        let start = d.layers.len();
        let w = if photo { 1060. } else { 1760. };
        let state = if photo { "With photo" } else { "Table" };
        label(
            &mut d,
            &format!("{state} rank heading"),
            (110., 280., 95., 50.),
            "RANK",
            26.,
            "$muted",
            Align::Left,
        );
        label(
            &mut d,
            &format!("{state} year heading"),
            (230., 280., w - 620., 50.),
            "YEAR",
            26.,
            "$muted",
            Align::Left,
        );
        label(
            &mut d,
            &format!("{state} count heading"),
            (110. + w - 420., 280., 360., 50.),
            "TORNADOES",
            26.,
            "$muted",
            Align::Right,
        );
        for i in 0..5 {
            let y = 349. + i as f32 * 107.;
            d.layers.push(
                panel(&format!("{state} row {i}"), 80., y, w, 91.).with_anim(
                    AnimKind::Rise,
                    0.2 + i as f32 * 0.07,
                    0.45,
                ),
            );
            label(
                &mut d,
                &format!("{state} rank {i}"),
                (110., y + 10., 95., 70.),
                &(i + 1).to_string(),
                43.,
                "$primary",
                Align::Left,
            );
            label(
                &mut d,
                &format!("{state} year {i}"),
                (230., y + 10., w - 620., 70.),
                &format!("{{{{set.history.year{i}}}}}"),
                42.,
                "$text",
                Align::Left,
            );
            label(
                &mut d,
                &format!("{state} total {i}"),
                (80. + w - 360., y + 10., 330., 70.),
                &format!("{{{{set.history.count{i}}}}}"),
                42.,
                "$text",
                Align::Right,
            );
        }
        if photo {
            d.layers
                .push(panel("Photo panel", 1170., 280., 670., 604.).with_anim(
                    AnimKind::Fade,
                    0.3,
                    0.5,
                ));
            label(
                &mut d,
                "Photo placeholder",
                (1230., 445., 550., 120.),
                "Photo not available yet",
                30.,
                "$muted",
                Align::Center,
            );
            d.layers.push(
                Layer::new(
                    "Optional licensed photo",
                    1190.,
                    300.,
                    630.,
                    445.,
                    LayerKind::Image {
                        media: "{{set.history.photo}}".into(),
                        fit: ImageFit::Contain,
                    },
                )
                .with_anim(AnimKind::Fade, 0.4, 0.5),
            );
            label(
                &mut d,
                "Photo caption",
                (1200., 765., 610., 95.),
                "{{set.history.credit}}",
                27.,
                "$muted",
                Align::Left,
            );
        }
        for l in &mut d.layers[start..] {
            l.states = vec![state.into()];
        }
    }
    label(
        &mut d,
        "History note",
        (80., 919., 1760., 45.),
        "{{set.history.note}}",
        25.,
        "$muted",
        Align::Left,
    );
    d
}

pub(super) fn countdown(format: Option<bool>) -> GraphicDoc {
    if let Some(portrait) = format {
        return social_countdown(portrait);
    }
    let mut d = frame(
        "Holiday Countdown",
        "Seasons",
        "{{set.calendar.today}}",
        "Calendar dates • US holidays",
    );
    label(
        &mut d,
        "Calendar intro",
        (80., 205., 1760., 70.),
        "Dates to look forward to",
        42.,
        "$text",
        Align::Left,
    );
    for (name, x, w, align) in [
        ("HOLIDAY", 110., 850., Align::Left),
        ("DATE", 1000., 350., Align::Left),
        ("TIME TO GO", 1430., 380., Align::Right),
    ] {
        label(&mut d, name, (x, 290., w, 50.), name, 26., "$muted", align);
    }
    for i in 0..5 {
        let y = 358. + i as f32 * 110.;
        d.layers.push(
            panel(&format!("Holiday row {i}"), 80., y, 1760., 94.).with_anim(
                AnimKind::Rise,
                0.2 + i as f32 * 0.06,
                0.45,
            ),
        );
        d.layers.push(
            Layer::rect(
                &format!("Calendar accent {i}"),
                80.,
                y + 19.,
                5.,
                56.,
                Fill::solid("$primary"),
                0.,
            )
            .with_anim(AnimKind::Grow, 0.3, 0.4),
        );
        d.layers.push(
            Layer::risk_map(
                &format!("Remix calendar {i}"),
                110.,
                y + 25.,
                44.,
                44.,
                "set:calendar-icon",
            )
            .with_anim(AnimKind::Fade, 0.3, 0.4),
        );
        label(
            &mut d,
            &format!("Holiday {i}"),
            (180., y + 10., 770., 74.),
            &format!("{{{{set.calendar.name{i}}}}}"),
            43.,
            "$text",
            Align::Left,
        );
        label(
            &mut d,
            &format!("Holiday date {i}"),
            (1000., y + 10., 360., 74.),
            &format!("{{{{set.calendar.date{i}}}}}"),
            33.,
            "$muted",
            Align::Left,
        );
        label(
            &mut d,
            &format!("Holiday remaining {i}"),
            (1390., y + 10., 420., 74.),
            &format!("{{{{set.calendar.left{i}}}}}"),
            38.,
            "$primary",
            Align::Right,
        );
    }
    label(
        &mut d,
        "Calendar note",
        (80., 924., 1760., 40.),
        "Calendar days remaining • Actual holiday dates, not observed office closures",
        25.,
        "$muted",
        Align::Left,
    );
    d
}

fn social_countdown(portrait: bool) -> GraphicDoc {
    let mut d = social_frame(
        "Holiday Countdown",
        "Seasons",
        "{{set.calendar.today}}",
        "Calendar dates • US holidays",
        portrait,
    );
    label(
        &mut d,
        "Calendar intro",
        if portrait {
            (54., 260., 972., 80.)
        } else {
            (54., 196., 972., 58.)
        },
        "Dates to look forward to",
        if portrait { 44. } else { 34. },
        "$text",
        Align::Left,
    );
    let (start, step, height) = if portrait {
        (390., 224., 194.)
    } else {
        (280., 118., 104.)
    };
    for i in 0..5 {
        let y = start + i as f32 * step;
        d.layers.push(
            panel(&format!("Holiday row {i}"), 54., y, 972., height).with_anim(
                AnimKind::Rise,
                0.2 + i as f32 * 0.06,
                0.45,
            ),
        );
        d.layers.push(
            Layer::risk_map(
                &format!("Remix calendar {i}"),
                80.,
                y + if portrait { 36. } else { 22. },
                if portrait { 52. } else { 40. },
                if portrait { 52. } else { 40. },
                "set:calendar-icon",
            )
            .with_anim(AnimKind::Fade, 0.3, 0.4),
        );
        label(
            &mut d,
            &format!("Holiday {i}"),
            (155., y + 12., 847., if portrait { 64. } else { 44. }),
            &format!("{{{{set.calendar.name{i}}}}}"),
            if portrait { 44. } else { 34. },
            "$text",
            Align::Left,
        );
        label(
            &mut d,
            &format!("Holiday date {i}"),
            (
                155.,
                y + if portrait { 88. } else { 60. },
                340.,
                if portrait { 50. } else { 34. },
            ),
            &format!("{{{{set.calendar.date{i}}}}}"),
            if portrait { 32. } else { 26. },
            "$muted",
            Align::Left,
        );
        label(
            &mut d,
            &format!("Holiday remaining {i}"),
            (
                510.,
                y + if portrait { 88. } else { 60. },
                492.,
                if portrait { 62. } else { 34. },
            ),
            &format!("{{{{set.calendar.left{i}}}}}"),
            if portrait { 38. } else { 29. },
            "$primary",
            Align::Right,
        );
    }
    label(
        &mut d,
        "Calendar note",
        if portrait {
            (54., 1570., 972., 120.)
        } else {
            (54., 892., 972., 72.)
        },
        "Calendar days remaining • Actual holiday dates, not observed office closures",
        if portrait { 30. } else { 25. },
        "$muted",
        Align::Left,
    );
    d
}

pub(super) fn holidays(today: NaiveDate) -> Vec<(&'static str, NaiveDate)> {
    let mut dates = Vec::new();
    for year in today.year()..=today.year() + 1 {
        for (name, month, day) in [
            ("New Year's Day", 1, 1),
            ("Independence Day", 7, 4),
            ("Halloween", 10, 31),
            ("Veterans Day", 11, 11),
            ("Christmas Day", 12, 25),
        ] {
            dates.push((name, NaiveDate::from_ymd_opt(year, month, day).unwrap()));
        }
        let nov = NaiveDate::from_ymd_opt(year, 11, 1).unwrap();
        let delta =
            (Weekday::Thu.num_days_from_monday() + 7 - nov.weekday().num_days_from_monday()) % 7;
        dates.push((
            "Thanksgiving",
            nov + chrono::Duration::days((delta + 21) as i64),
        ));
        let sep = NaiveDate::from_ymd_opt(year, 9, 1).unwrap();
        let delta = (7 - sep.weekday().num_days_from_monday()) % 7;
        dates.push(("Labor Day", sep + chrono::Duration::days(delta as i64)));
        let may = NaiveDate::from_ymd_opt(year, 5, 31).unwrap();
        dates.push((
            "Memorial Day",
            may - chrono::Duration::days(may.weekday().num_days_from_monday() as i64),
        ));
    }
    dates.retain(|(_, date)| *date >= today);
    dates.sort_by_key(|(_, date)| *date);
    dates.truncate(5);
    dates
}

pub(super) fn remaining(days: i64) -> String {
    if days == 0 {
        return "Today".into();
    }
    let (weeks, days) = (days / 7, days % 7);
    let day = if days == 1 { "day" } else { "days" };
    match (weeks, days) {
        (0, _) => format!("{days} {day}"),
        (w, 0) => format!("{w} {}", if w == 1 { "week" } else { "weeks" }),
        (w, _) => format!("{w} {} {days} {day}", if w == 1 { "week" } else { "weeks" }),
    }
}

pub(super) fn bind(b: &mut Bindings) {
    let today = Local::now().date_naive();
    b.set(
        "set.calendar.today",
        &today.format("As of %B %-d, %Y").to_string(),
    );
    for (i, (name, date)) in holidays(today).iter().enumerate() {
        b.set(&format!("set.calendar.name{i}"), *name);
        b.set(
            &format!("set.calendar.date{i}"),
            &date.format("%b %-d, %Y").to_string(),
        );
        b.set(
            &format!("set.calendar.left{i}"),
            &remaining((*date - today).num_days()),
        );
    }
    b.set("set.tornado.scope", "Indiana");
    b.set(
        "set.tornado.period",
        &today.format("%Y • Year to date").to_string(),
    );
    b.set(
        "set.tornado.note",
        "Awaiting reviewed NWS / SPC tornado records.",
    );
    b.set(
        "set.tornado.source",
        "Source: NWS / SPC • Awaiting reviewed data",
    );
    b.set("set.tornado.payload", r#"{"state":"18","events":null}"#);
    b.set("set.survey.payload", r#"{"state":"18","events":null}"#);
    for key in ["status", "location", "rating", "wind", "date"] {
        b.set(&format!("set.survey.{key}"), MISSING);
    }
    b.set(
        "set.survey.note",
        "Survey ratings and peak winds may change after NWS review.",
    );
    for key in ["period", "note"] {
        b.set(&format!("set.history.{key}"), MISSING);
    }
    for i in 0..5 {
        b.set(&format!("set.history.year{i}"), MISSING);
        b.set(&format!("set.history.count{i}"), MISSING);
    }
    b.set("set.history.photo", "");
    b.set("set.history.credit", "Image credit not available yet");
}
