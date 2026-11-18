//! Historical data graphics (Claude's lane; Codex owns `gfx::sets`). Everything is real data
//! from the Iowa Environmental Mesonet (NWS warnings, storm-based polygons, Local Storm
//! Reports, ASOS daily summaries) and the SPC tornado database; see `data.rs`. The home state
//! and county come from the home point in Settings, the brand from `{{station}}`.
//!
//! Hooks (one line each): `templates::all` / `build_classic` (TEMPLATES, build),
//! `bind_ext::extend` (bind), `riskmap::draw` (`history:` specs → draw_map), and
//! `app_ext` hazard refresh (refresh).

pub mod canvas;
pub mod data;
pub mod paint;
#[cfg(test)]
mod tests;

use chrono::{Datelike, NaiveDate};
use tiny_skia::{Mask, Pixmap, Transform};

use crate::{
    config::Settings,
    data::WxSnapshot,
    gfx::{
        bind::Bindings,
        model::{AnimKind, Fill, GraphicDoc, Layer},
        templates::{bg_gradient, caps, footer, header, panel, text, wrapped, TemplateInfo, BODY, DISPLAY},
        text::{Align, GlyphCache, Weight},
    },
};

/// Shares the existing History category so all history graphics sit together.
pub const CATEGORY: &str = "History";
const SOURCE: &str = "Source: Iowa Environmental Mesonet / NWS";
const SOURCE_SPC: &str = "Source: NOAA SPC tornado database";
const SOURCE_ASOS: &str = "Source: Iowa Environmental Mesonet / NWS ASOS";

macro_rules! t {
    ($k:expr, $n:expr, $d:expr) => {
        TemplateInfo { key: $k, name: $n, category: CATEGORY, description: $d }
    };
}

pub const TEMPLATES: &[TemplateInfo] = &[
    t!("hist_warnings_ytd", "Warnings To Date", "Every tornado and severe thunderstorm warning polygon this year on the state map, with count boxes."),
    t!("hist_county_to", "Tornado Warnings by County", "County map shaded by the number of tornado warnings this year, numbered legend."),
    t!("hist_county_sv", "Severe Warnings by County", "County map shaded by severe thunderstorm warnings this year, numbered legend."),
    t!("hist_county_ff", "Flash Flood Warnings by County", "County map shaded by flash flood warnings this year, numbered legend."),
    t!("hist_months_to", "Tornado Warnings by Month", "This year's tornado warnings per month against the 10-year average."),
    t!("hist_months_sv", "Severe Warnings by Month", "This year's severe thunderstorm warnings per month against the 10-year average."),
    t!("hist_months_ff", "Flash Flood Warnings by Month", "This year's flash flood warnings per month against the 10-year average."),
    t!("hist_years_to", "Tornado Warnings by Year", "Statewide tornado warnings each year for 11 years, with the average."),
    t!("hist_years_sv", "Severe Warnings by Year", "Statewide severe thunderstorm warnings each year for 11 years, with the average."),
    t!("hist_years_ff", "Flash Flood Warnings by Year", "Statewide flash flood warnings each year for 11 years, with the average."),
    t!("hist_county_years", "Home County Warning History", "Tornado, severe and flash flood warnings for the home county each year since 2005."),
    t!("hist_on_this_day", "On This Day", "Warnings and advisories issued for the home county on today's date in past years."),
    t!("hist_biggest_reports", "Biggest Hail and Wind This Year", "The largest hail and strongest wind gusts reported in the state this year, with a map."),
    t!("hist_report_day", "Busiest Storm Report Day", "Map of every storm report on the year's busiest report day, by type."),
    t!("hist_busiest_day", "Busiest Warning Day", "The day with the most warnings statewide this year: its polygons and counts."),
    t!("hist_freeze", "First and Last Freeze", "Last spring and first fall freeze at the nearest NWS station: this year, average and each year."),
    t!("hist_wettest_months", "Wettest Months on Record", "The ten wettest months at the nearest NWS station in the last 21 years."),
    t!("hist_tornado_tracks", "Tornado Tracks Since 1950", "Every recorded tornado path in the state since 1950, coloured by rating."),
    t!("hist_strongest_tornado", "Strongest Tornado Near Home", "The highest-rated tornado on record within 25 miles of home: its path and facts."),
    t!("hist_warning_calendar", "Warning Calendar", "Each day of this year shaded by the number of warnings issued statewide."),
    t!("hist_last_tornado_warning", "Last Tornado Warning Here", "When the home county last had a tornado warning, and how many each year."),
];

/// Sub-group for the Graphics Studio.
pub fn subcategory(key: &str) -> Option<&'static str> {
    TEMPLATES.iter().find(|t| t.key == key)?;
    Some(match key {
        "hist_warnings_ytd" | "hist_county_to" | "hist_county_sv" | "hist_county_ff" | "hist_months_to" | "hist_months_sv" | "hist_months_ff" | "hist_busiest_day" | "hist_warning_calendar" | "hist_biggest_reports" | "hist_report_day" => "This year",
        "hist_freeze" | "hist_wettest_months" => "Station climate",
        "hist_tornado_tracks" | "hist_strongest_tornado" => "Tornado database",
        _ => "Past years",
    })
}

/// Owner-style frame: header with the state, a yellow subtitle line, the branded footer.
fn frame(title: &str, subtitle: &str, source: &str) -> GraphicDoc {
    let mut d = GraphicDoc::new(title, CATEGORY, bg_gradient());
    header(&mut d, title, "{{hist.state_name}}");
    d.layers.push(
        text("Subtitle", (80., 160., 1760., 54.), subtitle, BODY, 36., Weight::SemiBold, "#FFD34D", Align::Left).with_anim(AnimKind::Fade, 0.35, 0.4),
    );
    footer(&mut d, "{{station}}", source);
    d
}

fn map(d: &mut GraphicDoc, name: &str, rect: (f32, f32, f32, f32), spec: &str) {
    d.layers.push(Layer::risk_map(name, rect.0, rect.1, rect.2, rect.3, spec).with_anim(AnimKind::Fade, 0.3, 0.6));
}

/// Rounded count box: coloured edge, small caps label, big value.
fn count_box(d: &mut GraphicDoc, i: usize, rect: (f32, f32, f32, f32), color: &str, label: &str, value: &str) {
    let (x, y, w, h) = rect;
    let delay = 0.25 + i as f32 * 0.1;
    d.layers.push(Layer::rect(&format!("Count {i} edge"), x, y, w, h, Fill::solid(color), 26.).with_anim(AnimKind::Rise, delay, 0.4));
    d.layers.push(Layer::rect(&format!("Count {i} face"), x + 6., y + 6., w - 12., h - 12., Fill::linear("#101A2A", "#1C2C44", 90.), 22.).with_anim(AnimKind::Rise, delay, 0.4));
    d.layers.push(caps(text(&format!("Count {i} label"), (x + 24., y + 16., w - 48., 44.), label, BODY, 28., Weight::Bold, color, Align::Center), 1.5).with_anim(AnimKind::Fade, delay + 0.1, 0.3));
    d.layers.push(text(&format!("Count {i} value"), (x + 24., y + 62., w - 48., h - 80.), value, DISPLAY, 110., Weight::Bold, "$text", Align::Center).with_anim(AnimKind::Pop, delay + 0.15, 0.4));
}

fn line(d: &mut GraphicDoc, name: &str, rect: (f32, f32, f32, f32), content: &str, size: f32, color: &str, align: Align) {
    d.layers.push(text(name, rect, content, BODY, size, Weight::SemiBold, color, align).with_anim(AnimKind::Fade, 0.5, 0.4));
}

fn phen_of(key: &str) -> &'static str {
    match key.rsplit('_').next() {
        Some("to") => "TO",
        Some("sv") => "SV",
        _ => "FF",
    }
}

fn pkey(p: &str) -> &'static str {
    match p {
        "TO" => "to",
        "SV" => "sv",
        _ => "ff",
    }
}

fn plural(p: &str) -> &'static str {
    match p {
        "TO" => "tornado warnings",
        "SV" => "severe thunderstorm warnings",
        _ => "flash flood warnings",
    }
}

pub fn build(key: &str) -> Option<GraphicDoc> {
    let info = TEMPLATES.iter().find(|t| t.key == key)?;
    let mut d = match key {
        "hist_warnings_ytd" => {
            let mut d = frame("# of Warnings To Date", "# of tornado and severe thunderstorm warnings from {{hist.range}}", SOURCE);
            count_box(&mut d, 0, (70., 240., 560., 230.), "#FF3B30", "# of tornado warnings", "{{hist.to.count}}");
            count_box(&mut d, 1, (70., 494., 560., 230.), "#FFD60A", "# of severe warnings", "{{hist.sv.count}}");
            count_box(&mut d, 2, (70., 748., 560., 206.), "#34C759", "# of flash flood warnings", "{{hist.ff.count}}");
            map(&mut d, "Warning polygons", (660., 228., 1200., 740.), "history:polys:ytd");
            d
        }
        "hist_county_to" | "hist_county_sv" | "hist_county_ff" => {
            let p = phen_of(key);
            let mut d = frame(&format!("{} by County", data::phen_name(p).to_string() + " Warnings"), &format!("Number of {} by county, {{{{hist.range}}}}", plural(p)), SOURCE);
            map(&mut d, "County counts", (60., 225., 1800., 745.), &format!("history:county:{p}"));
            d
        }
        "hist_months_to" | "hist_months_sv" | "hist_months_ff" => {
            let p = phen_of(key);
            let k = pkey(p);
            let mut d = frame(&format!("{} Warnings by Month", data::phen_name(p)), &format!("{{{{hist.year}}}} {} each month vs. the {{{{hist.avg_years}}}} average", plural(p)), SOURCE);
            map(&mut d, "Monthly chart", (60., 225., 1800., 650.), &format!("history:months:{p}"));
            line(&mut d, "Chart note", (80., 890., 1760., 70.), &format!("{{{{hist.{k}.count}}}} so far this year • about {{{{hist.{k}.avg}}}} in an average year"), 38., "$text", Align::Center);
            d
        }
        "hist_years_to" | "hist_years_sv" | "hist_years_ff" => {
            let p = phen_of(key);
            let k = pkey(p);
            let mut d = frame(&format!("{} Warnings by Year", data::phen_name(p)), &format!("Statewide {} each year, {{{{hist.first_year}}}} to {{{{hist.year}}}} (this year to date)", plural(p)), SOURCE);
            map(&mut d, "Yearly chart", (60., 225., 1800., 650.), &format!("history:years:{p}"));
            line(&mut d, "Chart note", (80., 890., 1760., 70.), &format!("Busiest year: {{{{hist.{k}.max_year}}}} • Average: {{{{hist.{k}.avg}}}} a year"), 38., "$text", Align::Center);
            d
        }
        "hist_county_years" => {
            let mut d = frame("Warnings in {{hist.county}} County", "Tornado, severe thunderstorm and flash flood warnings each year since 2005", SOURCE);
            map(&mut d, "County chart", (60., 225., 1800., 650.), "history:countyyears");
            line(&mut d, "Chart note", (80., 890., 1760., 70.), "Since 2005: {{hist.cty.to}} tornado • {{hist.cty.sv}} severe • {{hist.cty.ff}} flash flood warnings", 36., "$text", Align::Center);
            d
        }
        "hist_on_this_day" => {
            let mut d = frame("On This Day", "{{hist.today}} and the days around it in {{hist.county}} County, past years", SOURCE);
            d.layers.push(Layer::rect("Date tile", 80., 250., 420., 420., Fill::linear("$primary", "$secondary", 135.), 30.).with_anim(AnimKind::Pop, 0.2, 0.4));
            d.layers.push(caps(text("Date month", (80., 280., 420., 90.), "{{hist.today_month}}", DISPLAY, 70., Weight::Bold, "$text", Align::Center), 4.).with_anim(AnimKind::Fade, 0.3, 0.3));
            d.layers.push(text("Date day", (80., 360., 420., 240.), "{{hist.today_day}}", DISPLAY, 220., Weight::Bold, "$text", Align::Center).with_anim(AnimKind::Pop, 0.35, 0.4));
            line(&mut d, "On this day count", (80., 700., 420., 200.), "{{hist.otd.count}}", 34., "$muted", Align::Center);
            if let Some(l) = d.layers.last_mut() {
                *l = wrapped(l.clone());
            }
            for i in 0..7 {
                let y = 250. + i as f32 * 100.;
                d.layers.push(panel(&format!("Row {i}"), 560., y, 1290., 86.).with_anim(AnimKind::SlideLeft, 0.3 + i as f32 * 0.06, 0.35));
                d.layers.push(text(&format!("Row {i} year"), (585., y + 8., 170., 70.), &format!("{{{{hist.otd.{i}.year}}}}"), DISPLAY, 50., Weight::Bold, "#FFD34D", Align::Left).with_anim(AnimKind::Fade, 0.4 + i as f32 * 0.06, 0.3));
                d.layers.push(text(&format!("Row {i} text"), (770., y + 8., 1060., 70.), &format!("{{{{hist.otd.{i}.text}}}}"), BODY, 36., Weight::SemiBold, "$text", Align::Left).with_anim(AnimKind::Fade, 0.42 + i as f32 * 0.06, 0.3));
            }
            d
        }
        "hist_biggest_reports" => {
            let mut d = frame("Biggest Hail and Wind", "Largest reports in {{hist.state_name}}, {{hist.range}}", SOURCE);
            map(&mut d, "Report map", (50., 225., 820., 745.), "history:reports:big");
            for (j, (title, k, color)) in [("Largest hail", "hail", "#7FD1FF"), ("Strongest wind gusts", "wind", "#FFB340")].iter().enumerate() {
                let y0 = 232. + j as f32 * 372.;
                d.layers.push(caps(text(&format!("{title} title"), (900., y0, 950., 50.), title, DISPLAY, 40., Weight::Bold, color, Align::Left), 2.).with_anim(AnimKind::Fade, 0.3, 0.3));
                for i in 0..5 {
                    let y = y0 + 58. + i as f32 * 62.;
                    d.layers.push(panel(&format!("{k} row {i}"), 900., y, 950., 54.).with_anim(AnimKind::SlideLeft, 0.35 + i as f32 * 0.05, 0.3));
                    d.layers.push(text(&format!("{k} row {i} value"), (918., y + 4., 200., 46.), &format!("{{{{hist.{k}.{i}.value}}}}"), DISPLAY, 36., Weight::Bold, color, Align::Left).with_anim(AnimKind::Fade, 0.4, 0.3));
                    d.layers.push(text(&format!("{k} row {i} where"), (1125., y + 4., 710., 46.), &format!("{{{{hist.{k}.{i}.where}}}}"), BODY, 30., Weight::SemiBold, "$text", Align::Left).with_anim(AnimKind::Fade, 0.42, 0.3));
                }
            }
            d
        }
        "hist_report_day" => {
            let mut d = frame("Storm Reports: {{hist.rday.date}}", "{{hist.rday.count}} reports on the busiest storm report day of {{hist.year}} in {{hist.state_name}}", SOURCE);
            map(&mut d, "Report day map", (60., 225., 1800., 745.), "history:reports:day");
            d
        }
        "hist_busiest_day" => {
            let mut d = frame("Busiest Warning Day", "{{hist.bday.date}}: the most warnings in one day in {{hist.state_name}} this year", SOURCE);
            count_box(&mut d, 0, (70., 240., 560., 220.), "#FFFFFF", "warnings that day", "{{hist.bday.count}}");
            count_box(&mut d, 1, (70., 484., 560., 220.), "#FF3B30", "tornado warnings", "{{hist.bday.to}}");
            count_box(&mut d, 2, (70., 728., 560., 226.), "#FFD60A", "severe warnings", "{{hist.bday.sv}}");
            map(&mut d, "Busiest day polygons", (660., 228., 1200., 740.), "history:polys:busiest");
            d
        }
        "hist_freeze" => {
            let mut d = frame("First and Last Freeze", "32°F or colder at {{hist.station_name}}, {{hist.daily_years}}", SOURCE_ASOS);
            count_box(&mut d, 0, (70., 240., 520., 220.), "#7FD1FF", "last spring freeze {{hist.year}}", "{{hist.frz.last}}");
            count_box(&mut d, 1, (70., 484., 520., 220.), "#FFB340", "first fall freeze {{hist.year}}", "{{hist.frz.first}}");
            line(&mut d, "Freeze averages", (70., 728., 520., 230.), "Average last spring freeze: {{hist.frz.avg_last}}\nAverage first fall freeze: {{hist.frz.avg_first}}\nAverage growing season: {{hist.frz.avg_days}}", 30., "$text", Align::Left);
            if let Some(l) = d.layers.last_mut() {
                *l = wrapped(l.clone());
            }
            map(&mut d, "Freeze timeline", (620., 228., 1250., 740.), "history:freeze");
            d
        }
        "hist_wettest_months" => {
            let mut d = frame("Wettest Months on Record", "Total rain at {{hist.station_name}}, {{hist.daily_years}}", SOURCE_ASOS);
            map(&mut d, "Wettest chart", (60., 225., 1800., 745.), "history:wettest");
            d
        }
        "hist_tornado_tracks" => {
            let mut d = frame("Tornado Tracks Since 1950", "{{hist.trk.count}} recorded tornado paths in {{hist.state_name}}, 1950 to {{hist.trk.last_year}}", SOURCE_SPC);
            map(&mut d, "Track map", (60., 225., 1800., 745.), "history:tracks");
            d
        }
        "hist_strongest_tornado" => {
            let mut d = frame("Strongest Tornado Near Home", "Highest-rated tornado within 25 miles of {{hist.county}} County since 1950", SOURCE_SPC);
            map(&mut d, "Strongest track map", (640., 228., 1220., 740.), "history:strongest");
            count_box(&mut d, 0, (70., 240., 540., 220.), "#FF3B30", "rating", "{{hist.str.ef}}");
            line(&mut d, "Strongest facts", (80., 490., 520., 470.), "Date: {{hist.str.date}}\nPath length: {{hist.str.miles}}\nDeaths: {{hist.str.fat}}\nTornadoes this strong nearby: {{hist.str.same}}", 34., "$text", Align::Left);
            if let Some(l) = d.layers.last_mut() {
                *l = wrapped(l.clone());
            }
            d
        }
        "hist_warning_calendar" => {
            let mut d = frame("{{hist.year}} Warning Calendar", "Warnings issued statewide each day (tornado, severe and flash flood), {{hist.range}}", SOURCE);
            map(&mut d, "Calendar", (60., 225., 1800., 745.), "history:calendar");
            d
        }
        "hist_last_tornado_warning" => {
            let mut d = frame("Last Tornado Warning Here", "Tornado warnings for {{hist.county}} County since 2005", SOURCE);
            count_box(&mut d, 0, (70., 240., 600., 240.), "#FF3B30", "last tornado warning", "{{hist.last_to.date}}");
            line(&mut d, "Last warning ago", (70., 500., 600., 120.), "{{hist.last_to.ago}}", 40., "$text", Align::Center);
            line(&mut d, "Last warning total", (70., 640., 600., 300.), "{{hist.cty.to}} tornado warnings since 2005. Busiest year: {{hist.cty.to_max}}.", 34., "$muted", Align::Center);
            if let Some(l) = d.layers.last_mut() {
                *l = wrapped(l.clone());
            }
            map(&mut d, "Tornado warnings by year", (700., 228., 1160., 740.), "history:countyto");
            d
        }
        _ => return None,
    };
    d.name = info.name.to_string();
    d.category = info.category.to_string();
    d.template = Some(key.to_string());
    Some(d)
}

pub async fn refresh(home: crate::config::GeoView) {
    data::refresh(home).await;
}

pub fn draw_map(pm: &mut Pixmap, glyphs: &mut GlyphCache, local: Transform, mask: Option<&Mask>, w: f32, h: f32, spec: &str, opacity: f32, font: &str) -> bool {
    let Some(rest) = spec.trim().strip_prefix("history:") else { return false };
    canvas::draw(pm, glyphs, local, mask, w, h, rest, opacity, font);
    true
}

fn fmt_day(t: i64) -> String {
    chrono::DateTime::from_timestamp(t, 0).map(|d| d.with_timezone(&chrono::Local).format("%b %-d, %Y").to_string()).unwrap_or_default()
}

fn fmt_date(d: NaiveDate) -> String {
    d.format("%b %-d").to_string()
}

/// Local calendar date of a unix time.
pub fn local_date(t: i64) -> Option<NaiveDate> {
    chrono::DateTime::from_timestamp(t, 0).map(|d| d.with_timezone(&chrono::Local).date_naive())
}

/// Busiest day (local date, warnings) this year, ties go to the earlier day.
pub fn busiest_day(h: &data::Hist) -> Option<(NaiveDate, usize)> {
    let mut by: std::collections::BTreeMap<NaiveDate, usize> = Default::default();
    for w in &h.warns {
        if let Some(d) = local_date(w.issue) {
            *by.entry(d).or_default() += 1;
        }
    }
    let max = *by.values().max()?;
    by.into_iter().find(|(_, n)| *n == max)
}

/// Busiest storm-report day this year (local date, reports), not counting snow and rain totals.
pub fn report_day(h: &data::Hist) -> Option<(NaiveDate, usize)> {
    let mut by: std::collections::BTreeMap<NaiveDate, usize> = Default::default();
    for r in h.reports.iter().filter(|r| is_storm_report(&r.kind)) {
        if let Some(d) = local_date(r.valid) {
            *by.entry(d).or_default() += 1;
        }
    }
    let max = *by.values().max()?;
    by.into_iter().find(|(_, n)| *n == max)
}

pub fn is_storm_report(kind: &str) -> bool {
    matches!(kind, "TORNADO" | "FUNNEL CLOUD" | "HAIL" | "TSTM WND DMG" | "TSTM WND GST" | "FLASH FLOOD" | "NON-TSTM WND DMG" | "NON-TSTM WND GST" | "WALL CLOUD")
}

/// Last spring and first fall freeze (≤ 32 °F) for each year with enough data.
pub fn freezes(h: &data::Hist) -> Vec<(i32, Option<NaiveDate>, Option<NaiveDate>)> {
    let mut out = Vec::new();
    let years: std::collections::BTreeSet<i32> = h.daily.iter().map(|d| d.date.year()).collect();
    for y in years {
        let days: Vec<&data::Daily> = h.daily.iter().filter(|d| d.date.year() == y && d.lo.is_some()).collect();
        if days.len() < 250 && y != h.year {
            continue;
        }
        let last = days.iter().filter(|d| d.date.month() < 7 && d.lo.unwrap_or(99.) <= 32.).map(|d| d.date).max();
        let first = days.iter().filter(|d| d.date.month() >= 7 && d.lo.unwrap_or(99.) <= 32.).map(|d| d.date).min();
        out.push((y, last, first));
    }
    out
}

fn avg_doy(dates: &[NaiveDate]) -> Option<NaiveDate> {
    if dates.is_empty() {
        return None;
    }
    let mean = dates.iter().map(|d| d.ordinal() as f64).sum::<f64>() / dates.len() as f64;
    NaiveDate::from_yo_opt(2001, mean.round() as u32)
}

/// Monthly rain totals (year, month, inches) for months with at least 27 days of data.
pub fn month_rain(h: &data::Hist) -> Vec<(i32, u32, f32)> {
    let mut by: std::collections::BTreeMap<(i32, u32), (f32, u32)> = Default::default();
    for d in &h.daily {
        if let Some(p) = d.precip {
            let e = by.entry((d.date.year(), d.date.month())).or_default();
            e.0 += p;
            e.1 += 1;
        }
    }
    by.into_iter().filter(|(_, (_, n))| *n >= 27).map(|((y, m), (p, _))| (y, m, p)).collect()
}

/// Miles between two (lat, lon) points.
pub fn miles(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, la2) = (a.0.to_radians(), b.0.to_radians());
    let dl = (b.1 - a.1).to_radians();
    let dp = la2 - la1;
    let x = (dp / 2.).sin().powi(2) + la1.cos() * la2.cos() * (dl / 2.).sin().powi(2);
    3958.8 * 2. * x.sqrt().asin()
}

/// The strongest tornado (highest rating, then longest path) within 25 miles of home.
pub fn strongest(h: &data::Hist, home: (f64, f64)) -> Option<(crate::history::Tornado, usize)> {
    let near: Vec<&crate::history::Tornado> = h
        .near
        .iter()
        .filter(|t| t.ef >= 0 && (miles(t.start, home) <= 25. || miles(t.end, home) <= 25. || miles(((t.start.0 + t.end.0) / 2., (t.start.1 + t.end.1) / 2.), home) <= 25.))
        .collect();
    let best = near.iter().max_by(|a, b| (a.ef, (miles(a.start, a.end) * 10.) as i64).cmp(&(b.ef, (miles(b.start, b.end) * 10.) as i64)))?;
    let same = near.iter().filter(|t| t.ef == best.ef).count();
    Some(((*best).clone(), same))
}

/// Computed binding pairs (the history data changes at most hourly; bind runs every frame).
#[derive(Default)]
struct Out(Vec<(String, String)>);

impl Out {
    fn set(&mut self, k: &str, v: &str) {
        self.0.push((k.to_string(), v.to_string()));
    }
}

fn home_slot() -> &'static std::sync::RwLock<(f64, f64)> {
    static H: std::sync::OnceLock<std::sync::RwLock<(f64, f64)>> = std::sync::OnceLock::new();
    H.get_or_init(|| std::sync::RwLock::new((f64::NAN, f64::NAN)))
}

/// The home point (lat, lon) from the last bind, else the one the data was fetched for.
pub fn home() -> (f64, f64) {
    let h = home_slot().read().map(|h| *h).unwrap_or((f64::NAN, f64::NAN));
    if h.0.is_finite() {
        return h;
    }
    data::snapshot().map(|s| s.home).unwrap_or((39.0, -86.0))
}

pub fn bind(b: &mut Bindings, _data: &WxSnapshot, settings: &Settings) {
    if let Ok(mut h) = home_slot().write() {
        *h = (settings.home.lat, settings.home.lon);
    }
    type Cache = Option<((i64, NaiveDate, u64, u64), std::sync::Arc<Vec<(String, String)>>)>;
    static CACHE: std::sync::OnceLock<std::sync::RwLock<Cache>> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let today = chrono::Local::now().date_naive();
    let fetched = data::store().read().ok().and_then(|s| s.as_ref().map(|h| h.fetched)).unwrap_or(0);
    let key = (fetched, today, settings.home.lat.to_bits(), settings.home.lon.to_bits());
    let hit = cache.read().ok().and_then(|c| c.as_ref().filter(|(k, _)| *k == key).map(|(_, v)| v.clone()));
    let pairs = match hit {
        Some(v) => v,
        None => {
            let mut o = Out::default();
            compute(&mut o, settings, today);
            let v = std::sync::Arc::new(o.0);
            if let Ok(mut c) = cache.write() {
                *c = Some((key, v.clone()));
            }
            v
        }
    };
    for (k, v) in pairs.iter() {
        b.set(k, v);
    }
}

fn compute(b: &mut Out, settings: &Settings, today: NaiveDate) {
    b.set("hist.today", &today.format("%B %-d").to_string());
    b.set("hist.today_month", &today.format("%B").to_string());
    b.set("hist.today_day", &today.day().to_string());
    b.set("hist.year", &today.year().to_string());
    b.set("hist.range", &format!("Jan 1 – {}", today.format("%b %-d, %Y")));
    let missing = crate::gfx::history_set::MISSING;
    let Some(h) = data::snapshot() else {
        for k in [
            "hist.state_name", "hist.county", "hist.to.count", "hist.sv.count", "hist.ff.count", "hist.to.avg", "hist.sv.avg", "hist.ff.avg", "hist.station_name",
            "hist.daily_years", "hist.avg_years", "hist.first_year", "hist.bday.date", "hist.rday.date", "hist.last_to.date", "hist.str.ef", "hist.frz.last", "hist.frz.first",
        ] {
            b.set(k, if k.ends_with("state_name") { "" } else if k.ends_with("county") { "Home" } else { missing });
        }
        // Everything else a template shows: never "--" while the first fetch is running.
        for k in ["hist.bday.count", "hist.bday.to", "hist.bday.sv", "hist.rday.count", "hist.cty.to", "hist.cty.sv", "hist.cty.ff", "hist.cty.to_max", "hist.frz.avg_last", "hist.frz.avg_first", "hist.frz.avg_days", "hist.trk.count", "hist.str.date", "hist.str.miles", "hist.str.fat", "hist.str.same", "hist.to.max_year", "hist.sv.max_year", "hist.ff.max_year", "hist.first_year"] {
            b.set(k, missing);
        }
        b.set("hist.trk.last_year", "");
        b.set("hist.last_to.ago", "");
        for i in 0..7 {
            b.set(&format!("hist.otd.{i}.year"), "");
            b.set(&format!("hist.otd.{i}.text"), "");
        }
        for k in ["hail", "wind"] {
            for i in 0..5 {
                b.set(&format!("hist.{k}.{i}.value"), if i == 0 { missing } else { "" });
                b.set(&format!("hist.{k}.{i}.where"), "");
            }
        }
        b.set("hist.otd.count", missing);
        return;
    };
    b.set("hist.state_name", data::state_name(&h.state));
    b.set("hist.county", &h.county);
    let past_n = h.past.len().max(1) as f64;
    if let (Some(f), Some(l)) = (h.past.first(), h.past.last()) {
        b.set("hist.avg_years", &format!("{}–{}", f.year, l.year));
        b.set("hist.first_year", &f.year.to_string());
    } else {
        b.set("hist.avg_years", "10-year");
        b.set("hist.first_year", &h.year.to_string());
    }
    for (i, p) in data::PHENS.iter().enumerate() {
        let k = pkey(p);
        let n = h.warns.iter().filter(|w| w.phen == *p).count();
        b.set(&format!("hist.{k}.count"), &n.to_string());
        let avg = h.past.iter().map(|y| y.total(i) as f64).sum::<f64>() / past_n;
        b.set(&format!("hist.{k}.avg"), &format!("{}", avg.round() as i64));
        let mut years: Vec<(i32, u32)> = h.past.iter().map(|y| (y.year, y.total(i))).collect();
        years.push((h.year, n as u32));
        if let Some((y, c)) = years.iter().max_by_key(|(y, c)| (*c, -*y)) {
            b.set(&format!("hist.{k}.max_year"), &format!("{y} ({c})"));
        }
    }
    // Busiest warning day.
    match busiest_day(&h) {
        Some((d, n)) => {
            b.set("hist.bday.date", &d.format("%B %-d, %Y").to_string());
            b.set("hist.bday.count", &n.to_string());
            for p in ["TO", "SV"] {
                let c = h.warns.iter().filter(|w| w.phen == p && local_date(w.issue) == Some(d)).count();
                b.set(&format!("hist.bday.{}", pkey(p)), &c.to_string());
            }
        }
        None => {
            b.set("hist.bday.date", missing);
            for k in ["count", "to", "sv"] {
                b.set(&format!("hist.bday.{k}"), "0");
            }
        }
    }
    match report_day(&h) {
        Some((d, n)) => {
            b.set("hist.rday.date", &d.format("%B %-d, %Y").to_string());
            b.set("hist.rday.count", &n.to_string());
        }
        None => {
            b.set("hist.rday.date", missing);
            b.set("hist.rday.count", "0");
        }
    }
    // Biggest hail and wind.
    let mut hail: Vec<&data::Report> = h.reports.iter().filter(|r| r.kind == "HAIL" && r.mag.is_some()).collect();
    hail.sort_by(|a, b| b.mag.partial_cmp(&a.mag).unwrap_or(std::cmp::Ordering::Equal).then(a.valid.cmp(&b.valid)));
    let mut wind: Vec<&data::Report> = h.reports.iter().filter(|r| r.kind == "TSTM WND GST" && r.mag.is_some()).collect();
    wind.sort_by(|a, b| b.mag.partial_cmp(&a.mag).unwrap_or(std::cmp::Ordering::Equal).then(a.valid.cmp(&b.valid)));
    for (k, list, unit) in [("hail", &hail, "in"), ("wind", &wind, "mph")] {
        for i in 0..5 {
            let (v, w) = match list.get(i) {
                Some(r) => (
                    if unit == "in" { format!("{:.2}\"", r.mag.unwrap_or(0.)) } else { format!("{:.0} mph", r.mag.unwrap_or(0.)) },
                    format!("{} ({} Co.) • {}", title_case(&r.city), r.county, chrono::DateTime::from_timestamp(r.valid, 0).map(|d| d.with_timezone(&chrono::Local).format("%b %-d").to_string()).unwrap_or_default()),
                ),
                None if i == 0 => ("None".into(), "No reports this year".into()),
                None => (String::new(), String::new()),
            };
            b.set(&format!("hist.{k}.{i}.value"), &v);
            b.set(&format!("hist.{k}.{i}.where"), &w);
        }
    }
    // On this day (home county, past years).
    // Past years, today's date first, then the three days either side (labelled with the date).
    let mut by_day: std::collections::BTreeMap<NaiveDate, Vec<String>> = Default::default();
    for e in &h.county_events {
        let Some(d) = local_date(e.issue) else { continue };
        if d.year() >= today.year() {
            continue;
        }
        let Some(anniv) = NaiveDate::from_ymd_opt(d.year(), today.month(), today.day()) else { continue };
        if (d - anniv).num_days().abs() > 3 {
            continue;
        }
        let names = by_day.entry(d).or_default();
        if !names.contains(&e.name) {
            names.push(e.name.clone());
        }
    }
    let offset = |d: &NaiveDate| NaiveDate::from_ymd_opt(d.year(), today.month(), today.day()).map(|a| (*d - a).num_days().abs()).unwrap_or(9);
    let mut rows: Vec<(NaiveDate, Vec<String>)> = by_day.into_iter().collect();
    rows.sort_by(|a, b| offset(&a.0).cmp(&offset(&b.0)).then(b.0.year().cmp(&a.0.year())));
    let exact = rows.iter().filter(|r| offset(&r.0) == 0).count();
    b.set(
        "hist.otd.count",
        &match exact {
            0 => "No warnings or advisories on this exact date since 2005; nearby days shown".to_string(),
            1 => "1 year with alerts on this date since 2005, plus nearby days".to_string(),
            n => format!("{n} years with alerts on this date since 2005, plus nearby days"),
        },
    );
    for i in 0..7 {
        let (y, t) = rows.get(i).map(|(d, n)| (d.year().to_string(), format!("{}: {}", d.format("%b %-d"), n.join(", ")))).unwrap_or_default();
        b.set(&format!("hist.otd.{i}.year"), &y);
        b.set(&format!("hist.otd.{i}.text"), &t);
    }    // Home county totals.
    for (k, p) in [("to", "TO"), ("sv", "SV"), ("ff", "FF")] {
        let n = h.county_events.iter().filter(|e| e.phen == p && e.sig == "W").count();
        b.set(&format!("hist.cty.{k}"), &n.to_string());
    }
    let mut by_year: std::collections::BTreeMap<i32, usize> = Default::default();
    for e in h.county_events.iter().filter(|e| e.phen == "TO" && e.sig == "W") {
        if let Some(d) = local_date(e.issue) {
            *by_year.entry(d.year()).or_default() += 1;
        }
    }
    b.set("hist.cty.to_max", &by_year.iter().max_by_key(|(y, n)| (**n, -**y)).map(|(y, n)| format!("{y} ({n})")).unwrap_or_else(|| "none".into()));
    match h.county_events.iter().filter(|e| e.phen == "TO" && e.sig == "W").map(|e| e.issue).max() {
        Some(t) => {
            b.set("hist.last_to.date", &fmt_day(t));
            let days = (chrono::Utc::now().timestamp() - t) / 86400;
            b.set("hist.last_to.ago", &match days {
                0 => "Today".to_string(),
                1 => "1 day ago".to_string(),
                n if n < 60 => format!("{n} days ago"),
                n if n < 730 => format!("About {} months ago", n / 30),
                n => format!("About {} years ago", n / 365),
            });
        }
        None => {
            b.set("hist.last_to.date", "None since 2005");
            b.set("hist.last_to.ago", "");
        }
    }
    // Station climate.
    b.set("hist.station_name", &if !h.station_name.is_empty() { title_case(&h.station_name) } else if !h.station.is_empty() { h.station.clone() } else { "the nearest NWS station".to_string() });
    let yrs: Vec<i32> = h.daily.iter().map(|d| d.date.year()).collect();
    if let (Some(a), Some(z)) = (yrs.iter().min(), yrs.iter().max()) {
        b.set("hist.daily_years", &format!("{a}–{z}"));
    } else {
        b.set("hist.daily_years", missing);
    }
    let fz = freezes(&h);
    let this = fz.iter().find(|f| f.0 == h.year);
    b.set("hist.frz.last", &this.and_then(|f| f.1).map(fmt_date).unwrap_or_else(|| if h.daily.is_empty() { missing.into() } else { "None".into() }));
    b.set("hist.frz.first", &this.and_then(|f| f.2).map(fmt_date).unwrap_or_else(|| if h.daily.is_empty() { missing.into() } else { "Not yet".into() }));
    let past: Vec<_> = fz.iter().filter(|f| f.0 < h.year).collect();
    let lasts: Vec<NaiveDate> = past.iter().filter_map(|f| f.1).collect();
    let firsts: Vec<NaiveDate> = past.iter().filter_map(|f| f.2).collect();
    b.set("hist.frz.avg_last", &avg_doy(&lasts).map(fmt_date).unwrap_or_else(|| missing.into()));
    b.set("hist.frz.avg_first", &avg_doy(&firsts).map(fmt_date).unwrap_or_else(|| missing.into()));
    let seasons: Vec<i64> = past.iter().filter_map(|f| Some((f.2? - f.1?).num_days())).collect();
    b.set("hist.frz.avg_days", &if seasons.is_empty() { missing.to_string() } else { format!("{} days", seasons.iter().sum::<i64>() / seasons.len() as i64) });
    // Tornado database.
    b.set("hist.trk.count", &h.tornadoes.len().to_string());
    b.set("hist.trk.last_year", &h.tornadoes.iter().map(|t| t.year).max().map(|y| y.to_string()).unwrap_or_default());
    match strongest(&h, (settings.home.lat, settings.home.lon)) {
        Some((t, same)) => {
            b.set("hist.str.ef", &if t.year >= 2007 { format!("EF{}", t.ef) } else { format!("F{}", t.ef) });
            b.set("hist.str.date", &NaiveDate::parse_from_str(&t.date, "%Y-%m-%d").map(|d| d.format("%B %-d, %Y").to_string()).unwrap_or(t.date.clone()));
            b.set("hist.str.miles", &format!("{:.1} miles", miles(t.start, t.end)));
            b.set("hist.str.fat", &t.fatalities.to_string());
            b.set("hist.str.same", &same.to_string());
        }
        None => {
            for k in ["ef", "date", "miles", "fat", "same"] {
                b.set(&format!("hist.str.{k}"), missing);
            }
        }
    }
}

pub const MISSING: &str = "Data not available yet";

pub fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) if w.chars().all(|ch| !ch.is_lowercase()) => f.to_string() + &c.as_str().to_lowercase(),
                Some(f) => f.to_string() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
