//! SPC complete-year tornado rankings. State totals require state segments, not bbox hits.
use crate::{
    config::Settings,
    gfx::{
        bind::Bindings,
        model::{GraphicDoc, LayerKind},
    },
};
use chrono::{Datelike, NaiveDate};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

const HEADER: &str = "om,yr,mo,dy,date,time,tz,st,stf,stn,mag,inj,fat,loss,closs,slat,slon,elat,elon,len,wid,ns,sn,sg,f1,f2,f3,f4,fc";
#[derive(Default)]
struct Track {
    whole: Option<(u8, u8, NaiveDate)>,
    states: BTreeSet<u8>,
    ns: Option<u8>,
    dates: BTreeMap<u8, NaiveDate>,
}
#[derive(Debug)]
struct Archive {
    first: i32,
    last: i32,
    national: BTreeMap<i32, u32>,
    states: BTreeMap<u8, BTreeMap<i32, u32>>,
    national_months: BTreeMap<(i32, u32), u32>,
    state_months: BTreeMap<(u8, i32, u32), u32>,
}
impl Archive {
    fn ranked(&self, state: Option<u8>) -> Vec<(i32, u32)> {
        let mut years: Vec<_> = (self.first..=self.last)
            .map(|year| {
                let count = match state {
                    None => self.national.get(&year),
                    Some(s) => self.states.get(&s).and_then(|v| v.get(&year)),
                }
                .copied()
                .unwrap_or(0);
                (year, count)
            })
            .collect();
        years.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.0.cmp(&a.0)));
        years.truncate(5);
        years
    }
}
// Numeric SPC CSV fields contain no commas/quotes. Reject any unsupported CSV quoting,
// unknown schema, missing year, orphan county continuation or missing state segment.
fn valid_state(fips: u8, code: &str) -> bool {
    crate::winter::STATES
        .iter()
        .any(|(id, abbr, _)| id.parse::<u8>().ok() == Some(fips) && *abbr == code)
        || matches!(
            (fips, code),
            (2, "AK") | (15, "HI") | (72, "PR") | (78, "VI") | (66, "GU") | (60, "AS") | (69, "MP")
        )
}
fn decode(raw: &str, first: i32, last: i32) -> Option<Archive> {
    if first > last || raw.len() > 16 * 1024 * 1024 {
        return None;
    }
    let mut lines = raw.lines();
    let header = lines.next()?.trim_start_matches('\u{feff}');
    let extra = match header {
        h if h == HEADER => 0,
        h if h == format!("{HEADER},edat,etime") => 2,
        _ => return None,
    };
    let mut tracks: BTreeMap<(i32, u32), Track> = BTreeMap::new();
    for (i, line) in lines.enumerate() {
        if i >= 150_000 || line.is_empty() || line.contains('"') {
            return None;
        }
        let f: Vec<_> = line.split(',').collect();
        if f.len() != 29 + extra {
            return None;
        }
        let om = f[0].parse::<u32>().ok()?;
        let year = f[1].parse::<i32>().ok()?;
        let date = NaiveDate::parse_from_str(f[4], "%Y-%m-%d").ok()?;
        let st = f[8].parse::<u8>().ok()?;
        let ns = f[21].parse::<u8>().ok()?;
        let sn = f[22].parse::<u8>().ok()?;
        let sg = f[23].parse::<i8>().ok()?;
        if om == 0
            || !(first..=last).contains(&year)
            || date.year() != year
            || date.month() != f[2].parse::<u32>().ok()?
            || date.day() != f[3].parse::<u32>().ok()?
            || !valid_state(st, f[7])
            || f[7].len() != 2
            || !f[7].bytes().all(|b| b.is_ascii_uppercase())
            || !(1..=3).contains(&ns)
            || sn > 1
        {
            return None;
        }
        let t = tracks.entry((year, om)).or_default();
        if t.ns.is_some_and(|old| old != ns) {
            return None;
        }
        t.ns = Some(ns);
        if sn == 1 {
            if t.dates.insert(st, date).is_some_and(|old| old != date) {
                return None;
            }
        }
        match (ns, sn, sg) {
            (1, 1, 1) => {
                let whole = (ns, st, date);
                if t.whole.is_some_and(|old| old != whole) {
                    return None;
                }
                t.whole = Some(whole);
                t.states.insert(st);
            }
            (2..=3, 0, 1) => {
                let whole = (ns, st, date);
                if t.whole.is_some_and(|old| old != whole) {
                    return None;
                }
                t.whole = Some(whole);
            }
            (2..=3, 1, 2) => {
                t.states.insert(st);
            }
            (_, 0, -9) => {}
            _ => return None,
        }
    }
    let mut out = Archive {
        first,
        last,
        national: BTreeMap::new(),
        states: BTreeMap::new(),
        national_months: BTreeMap::new(),
        state_months: BTreeMap::new(),
    };
    for ((year, _), t) in tracks {
        let (ns, start, date) = t.whole?;
        if t.states.len() != ns as usize || !t.states.contains(&start) {
            return None;
        }
        *out.national.entry(year).or_default() += 1;
        *out.national_months.entry((year, date.month())).or_default() += 1;
        for st in t.states {
            let date = t.dates.get(&st)?;
            *out.state_months
                .entry((st, year, date.month()))
                .or_default() += 1;
            *out.states.entry(st).or_default().entry(year).or_default() += 1;
        }
    }
    if (first..=last).any(|year| !out.national.contains_key(&year)) {
        return None;
    }
    Some(out)
}
#[derive(Default)]
struct Store {
    generation: u64,
    attempted: Option<i64>,
    archive: Option<Arc<Archive>>,
}
fn store() -> &'static Mutex<Store> {
    static S: OnceLock<Mutex<Store>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Store::default()))
}
pub(super) async fn refresh() {
    let now = chrono::Utc::now();
    let ticket = {
        let Ok(mut s) = store().lock() else {
            return;
        };
        if s.attempted
            .is_some_and(|at| (0..86400).contains(&(now.timestamp() - at)))
        {
            return;
        }
        s.generation = s.generation.wrapping_add(1);
        s.attempted = Some(now.timestamp());
        s.generation
    };
    let mut result = None;
    for last in [now.year() - 1, now.year() - 2, now.year() - 3] {
        let url = format!("https://www.spc.noaa.gov/wcm/data/1950-{last}_all_tornadoes.csv");
        let Some(bytes) = super::report_feed::get_bytes(&url, Duration::from_secs(40)).await else {
            continue;
        };
        let Ok(raw) = std::str::from_utf8(&bytes) else {
            continue;
        };
        if let Some(archive) = decode(raw, 1950, last) {
            result = Some(Arc::new(archive));
            break;
        }
    }
    if let Ok(mut s) = store().lock() {
        if s.generation == ticket {
            if result.is_some() {
                s.archive = result;
            }
        }
    }
}
fn put(b: &mut Bindings, prefix: &str, archive: Option<&Archive>, state: Option<u8>, scope: &str) {
    b.set(&format!("{prefix}.scope"), scope);
    b.set(
        &format!("{prefix}.source"),
        "Source: NWS Storm Prediction Center • Complete-year tornado archive",
    );
    b.set(&format!("{prefix}.period"), super::reference::MISSING);
    b.set(
        &format!("{prefix}.note"),
        "Complete-year archive not available yet. Current-year reports are excluded.",
    );
    for i in 0..5 {
        for field in ["year", "count"] {
            b.set(&format!("{prefix}.{field}{i}"), "");
        }
    }
    b.set(&format!("{prefix}.status"), super::reference::MISSING);
    if let Some(a) = archive {
        b.set(&format!("{prefix}.status"), "");
        b.set(
            &format!("{prefix}.period"),
            &format!("Complete years {}–{}", a.first, a.last),
        );
        b.set(
            &format!("{prefix}.note"),
            "One tornado per year/ID in each affected state • Ties: most recent year first",
        );
        if state.is_none() {
            b.set(
                &format!("{prefix}.note"),
                "One whole-track record per year/ID • Ties: most recent year first",
            );
        }
        for (i, (year, count)) in a.ranked(state).iter().enumerate() {
            b.set(&format!("{prefix}.year{i}"), &year.to_string());
            b.set(&format!("{prefix}.count{i}"), &count.to_string());
        }
    }
}
fn state(settings: &Settings) -> Option<(u8, &'static str)> {
    let scope = super::observed::report_scope(settings, super::observed::Coverage::State);
    let fips = scope.states.first()?;
    let info = crate::winter::STATES.iter().find(|s| s.0 == fips)?;
    Some((fips.parse().ok()?, info.2))
}
pub(super) fn scope_label(settings: &Settings, national: bool) -> String {
    if national {
        "United States • SPC archive coverage".into()
    } else {
        state(settings)
            .map(|s| s.1)
            .unwrap_or("State not available yet")
            .into()
    }
}
impl Archive {
    fn monthly(&self, state: Option<u8>, year: Option<i32>) -> Option<([f64; 12], [f64; 12])> {
        if self.first > 1991
            || self.last < 2020
            || year.is_some_and(|y| y < self.first || y > self.last)
        {
            return None;
        }
        let count = |year, month| {
            match state {
                None => self.national_months.get(&(year, month)),
                Some(st) => self.state_months.get(&(st, year, month)),
            }
            .copied()
            .unwrap_or(0) as f64
        };
        let mut actual = [0.; 12];
        let mut average = [0.; 12];
        for m in 1..=12 {
            let i = (m - 1) as usize;
            actual[i] = year.map(|y| count(y, m)).unwrap_or(0.);
            average[i] = (1991..=2020).map(|y| count(y, m)).sum::<f64>() / 30.;
        }
        Some((actual, average))
    }
}
pub(super) fn monthly(
    settings: &Settings,
    national: bool,
    year: Option<i32>,
) -> Option<([f64; 12], [f64; 12])> {
    let state = if national {
        None
    } else {
        Some(state(settings)?.0)
    };
    let archive = store().lock().ok()?.archive.clone()?;
    archive.monthly(state, year)
}
/// 1991–2020 average yearly tornado count for one state (sum of monthly averages).
pub(super) fn annual_average(state: u8) -> Option<f64> {
    let archive = store().lock().ok()?.archive.clone()?;
    archive
        .monthly(Some(state), None)
        .map(|(_, avg)| avg.iter().sum())
}
pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let archive = store().lock().ok().and_then(|s| s.archive.clone());
    let scope = super::observed::report_scope(settings, super::observed::Coverage::State);
    let state = scope.states.first().and_then(|s| s.parse::<u8>().ok());
    let label = scope
        .states
        .first()
        .and_then(|s| crate::winter::STATES.iter().find(|x| x.0 == s))
        .map(|x| x.2)
        .unwrap_or("State not available yet");
    put(
        b,
        "set.history",
        archive.as_deref().filter(|_| state.is_some()),
        state,
        label,
    );
    put(
        b,
        "set.history.national",
        archive.as_deref(),
        None,
        "United States • SPC archive coverage",
    );
}
pub(super) fn build(national: bool, format: Option<bool>) -> GraphicDoc {
    let mut d = match format {
        None => {
            let mut doc = super::reference::ranked();
            for (state, width) in [("Table", 1760.), ("With photo", 1060.)] {
                let start = doc.layers.len();
                super::reference::label(
                    &mut doc,
                    &format!("{state} missing status"),
                    (230., 573., width - 180., 70.),
                    "{{set.history.status}}",
                    30.,
                    "$muted",
                    crate::gfx::text::Align::Center,
                );
                doc.layers[start].states = vec![state.into()];
            }
            doc
        }
        Some(portrait) => social(portrait),
    };
    // Separate the history source/scope from the current-year tally and survey bindings.
    for l in &mut d.layers {
        if let LayerKind::Text { text, .. } = &mut l.kind {
            *text = text
                .replace("set.tornado.source", "set.history.source")
                .replace("set.tornado.scope", "set.history.scope");
            if national {
                *text = text
                    .replace("set.history.", "set.history.national.")
                    .replace("set.history.national.photo", "set.history.photo")
                    .replace("set.history.national.credit", "set.history.credit");
            }
        }
    }
    d
}
fn social(portrait: bool) -> GraphicDoc {
    use crate::gfx::{templates::panel, text::Align};
    let mut d = super::reference::social_frame(
        "Top Tornado Years",
        "Tornado",
        "{{set.history.period}}",
        "{{set.history.source}}",
        portrait,
    );
    let label = super::reference::label;
    label(
        &mut d,
        "History scope",
        (54., 238., 972., 70.),
        "{{set.history.scope}}",
        36.,
        "$text",
        Align::Left,
    );
    d.states = vec!["Table".into(), "With photo".into()];
    for photo in [false, true] {
        let start = d.layers.len();
        let state = if photo { "With photo" } else { "Table" };
        let width = if photo && !portrait { 484. } else { 972. };
        let step = if portrait {
            if photo {
                132.
            } else {
                210.
            }
        } else {
            103.
        };
        let row_y = if portrait { 402. } else { 375. };
        let size = if photo && !portrait { 30. } else { 40. };
        label(
            &mut d,
            &format!("{state} year heading"),
            (134., 330., width * 0.47 - 80., 45.),
            "YEAR",
            24.,
            "$muted",
            Align::Left,
        );
        label(
            &mut d,
            &format!("{state} count heading"),
            (54. + width * 0.54, 330., width * 0.46 - 24., 45.),
            "TORNADOES",
            if photo && !portrait { 20. } else { 24. },
            "$muted",
            Align::Right,
        );
        for i in 0..5 {
            let y = row_y + step * i as f32;
            d.layers.push(panel(
                &format!("{state} row {i}"),
                54.,
                y,
                width,
                step - 14.,
            ));
            label(
                &mut d,
                &format!("{state} rank {i}"),
                (72., y + 10., 50., step - 34.),
                &(i + 1).to_string(),
                28.,
                "$primary",
                Align::Left,
            );
            label(
                &mut d,
                &format!("{state} year {i}"),
                (134., y + 10., width * 0.47 - 80., step - 34.),
                &format!("{{{{set.history.year{i}}}}}"),
                size,
                "$text",
                Align::Left,
            );
            label(
                &mut d,
                &format!("{state} count {i}"),
                (54. + width * 0.54, y + 10., width * 0.46 - 24., step - 34.),
                &format!("{{{{set.history.count{i}}}}}"),
                size,
                "$text",
                Align::Right,
            );
        }
        label(
            &mut d,
            &format!("{state} missing status"),
            (134., row_y + 2. * step + 10., width - 104., step - 34.),
            "{{set.history.status}}",
            28.,
            "$muted",
            Align::Center,
        );
        if photo {
            let (x, y, w, h) = if portrait {
                (54., 1090., 972., 500.)
            } else {
                (558., 375., 468., 440.)
            };
            d.layers.push(panel("Photo panel", x, y, w, h));
            label(
                &mut d,
                "Photo placeholder",
                (x + 24., y + 100., w - 48., 120.),
                "Photo not available yet",
                28.,
                "$muted",
                Align::Center,
            );
            d.layers.push(crate::gfx::model::Layer::new(
                "Optional licensed photo",
                x + 16.,
                y + 16.,
                w - 32.,
                h - 32.,
                LayerKind::Image {
                    media: "{{set.history.photo}}".into(),
                    fit: crate::gfx::model::ImageFit::Contain,
                },
            ));
            label(
                &mut d,
                "Photo caption",
                (x, y + h + 14., w, 70.),
                "{{set.history.credit}}",
                24.,
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
        (
            54.,
            if portrait { 1680. } else { 915. },
            972.,
            if portrait { 60. } else { 50. },
        ),
        "{{set.history.note}}",
        if portrait { 27. } else { 23. },
        "$muted",
        Align::Left,
    );
    d
}
#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    if !doc
        .template
        .as_deref()
        .is_some_and(|k| k.starts_with("set_tornado_top_years"))
    {
        return;
    }
    for prefix in ["set.history", "set.history.national"] {
        for (field, value) in [
            ("scope", "SAMPLE • Reviewed archive ranking"),
            ("status", ""),
            ("source", "SAMPLE ONLY • Not for broadcast"),
            ("period", "SAMPLE • Complete years 1950–2024"),
            (
                "note",
                "SAMPLE • Whole tornado IDs counted once • Ties: most recent year first",
            ),
        ] {
            doc.data_overrides
                .push((format!("{prefix}.{field}"), value.into()));
        }
        for (i, (year, count)) in [(2011, 72), (2023, 54), (1973, 49), (2004, 48), (2024, 47)]
            .iter()
            .enumerate()
        {
            doc.data_overrides
                .push((format!("{prefix}.year{i}"), year.to_string()));
            doc.data_overrides
                .push((format!("{prefix}.count{i}"), count.to_string()));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn row(id: u32, year: i32, state: u8, ns: u8, sn: u8, sg: i8) -> String {
        format!("{id},{year},01,02,{year}-01-02,12:00:00,3,{},{state},1,1,0,0,0,0,38,-87,39,-86,1,10,{ns},{sn},{sg},1,0,0,0,0",if state==18{"IN"}else{"KY"})
    }
    #[test]
    fn whole_tracks_state_crossings_and_county_continuations_count_once() {
        let raw = format!(
            "{HEADER}\n{}\n{}\n{}\n{}\n{}\n{}\n",
            row(1, 2023, 18, 2, 0, 1),
            row(1, 2023, 18, 2, 1, 2),
            row(1, 2023, 21, 2, 1, 2),
            row(1, 2023, 21, 2, 0, -9),
            row(1, 2023, 18, 2, 1, 2),
            row(1, 2024, 18, 1, 1, 1)
        );
        let a = decode(&raw, 2023, 2024).unwrap();
        assert_eq!(a.national[&2023], 1);
        assert_eq!(a.states[&18][&2023], 1);
        assert_eq!(a.states[&21][&2023], 1);
        assert_eq!(a.ranked(None), vec![(2024, 1), (2023, 1)]);
        assert!(decode(
            &raw.replace(&(row(1, 2023, 21, 2, 1, 2) + "\n"), ""),
            2023,
            2024
        )
        .is_none());
        assert!(decode(&raw, 2022, 2024).is_none());
        assert!(decode(&raw.replace("2023-01-02", "2023-02-30"), 2023, 2024).is_none());
    }
    #[test]
    fn monthly_baseline_includes_all_thirty_years_and_requires_target_coverage() {
        let mut raw = format!("{HEADER}\n");
        for year in 1991..=2024 {
            raw.push_str(&(row(1, year, 18, 1, 1, 1) + "\n"));
        }
        raw.push_str(&(row(2, 2024, 21, 1, 1, 1) + "\n"));
        let a = decode(&raw, 1991, 2024).unwrap();
        let (actual, average) = a.monthly(None, Some(2024)).unwrap();
        assert_eq!(actual[0], 2.);
        assert_eq!(average[0], 1.);
        assert_eq!(average[1], 0.);
        let (_, kentucky) = a.monthly(Some(21), None).unwrap();
        assert_eq!(kentucky, [0.; 12]);
        assert!(a.monthly(None, Some(2025)).is_none());
        let mut b = Bindings::default();
        put(&mut b, "set.history", None, Some(18), "Indiana");
        assert_eq!(b.get("set.history.count0"), Some(""));
        assert_eq!(
            b.get("set.history.status"),
            Some(super::super::reference::MISSING)
        );
    }
    #[tokio::test]
    #[ignore = "manual approved-relay archive contract check"]
    async fn live_spc_complete_archive_contract() {
        let url = "https://www.spc.noaa.gov/wcm/data/1950-2024_all_tornadoes.csv";
        let bytes = super::super::report_feed::get_bytes(url, Duration::from_secs(40))
            .await
            .expect("SPC relay archive unavailable");
        let raw = std::str::from_utf8(&bytes).unwrap();
        let a = decode(raw, 1950, 2024)
            .expect("SPC full archive did not satisfy complete-state-segment contract");
        println!(
            "SPC bytes={} years={}–{} national top={:?} Indiana top={:?}",
            bytes.len(),
            a.first,
            a.last,
            a.ranked(None),
            a.ranked(Some(18))
        );
    }
}
