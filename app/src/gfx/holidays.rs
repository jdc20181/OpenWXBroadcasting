//! US holidays for themed forecast graphics: each holiday's date (fixed, nth-weekday and Easter),
//! its look (colours and motifs), and bindings with the NWS forecast for that day
//! (`holiday.<id>.hi`, `.pop`, `.condition`, ...), plus `holiday.next.*` for the next holiday
//! within the 7-day forecast.

use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};

use super::bind::Bindings;
use crate::{config::Settings, data::WxSnapshot};

/// A holiday's identity and look.
pub struct Holiday {
    pub id: &'static str,
    pub name: &'static str,
    /// Background gradient (top-left, bottom-right).
    pub bg: (&'static str, &'static str),
    /// Card fill and its frame colour.
    pub card: &'static str,
    pub frame: &'static str,
    /// Title colour.
    pub accent: &'static str,
    /// Motifs scattered around the card: (name, colour).
    pub motifs: &'static [(&'static str, &'static str)],
}

pub const HOLIDAYS: &[Holiday] = &[
    Holiday { id: "new_year", name: "New Year's Day", bg: ("#0B1030", "#2A1B5C"), card: "#121733", frame: "#D9B84A", accent: "#F3D46B", motifs: &[("firework", "#F3D46B"), ("confetti", "#FFFFFF"), ("firework", "#7FC8FF"), ("star", "#F3D46B")] },
    Holiday { id: "mlk", name: "Martin Luther King Jr. Day", bg: ("#0D1B2E", "#22364F"), card: "#13233A", frame: "#C9B26A", accent: "#F2E3B0", motifs: &[("dove", "#E9EEF5"), ("star", "#C9B26A")] },
    Holiday { id: "valentines", name: "Valentine's Day", bg: ("#3A0718", "#8E1838"), card: "#2A0814", frame: "#F28AA5", accent: "#FFC2D1", motifs: &[("heart", "#F25C7A"), ("heart", "#FFC2D1"), ("heart", "#D7264B")] },
    Holiday { id: "presidents", name: "Presidents' Day", bg: ("#0B1A3A", "#1F3B73"), card: "#0F1E3D", frame: "#C8102E", accent: "#FFFFFF", motifs: &[("flag", "#FFFFFF"), ("star", "#FFFFFF"), ("star", "#C8102E")] },
    Holiday { id: "st_patricks", name: "St. Patrick's Day", bg: ("#06301A", "#11663A"), card: "#0A2716", frame: "#D9B84A", accent: "#9BE59B", motifs: &[("shamrock", "#3FBF5F"), ("shamrock", "#9BE59B"), ("star", "#D9B84A")] },
    Holiday { id: "easter", name: "Easter", bg: ("#2E2154", "#6A5AA8"), card: "#231A42", frame: "#F6D24A", accent: "#FFE7A3", motifs: &[("egg", "#F49AC1"), ("flower", "#FFFFFF"), ("egg", "#8FD3F4"), ("flower", "#F6B3D3")] },
    Holiday { id: "mothers", name: "Mother's Day", bg: ("#4A1236", "#A23B72"), card: "#36102A", frame: "#F7C6D9", accent: "#FFD9E8", motifs: &[("flower", "#F7A1C4"), ("heart", "#FFD9E8"), ("flower", "#FFFFFF")] },
    Holiday { id: "memorial", name: "Memorial Day", bg: ("#0A1730", "#1B2F5C"), card: "#101C36", frame: "#C8102E", accent: "#FFFFFF", motifs: &[("flag", "#FFFFFF"), ("star", "#FFFFFF"), ("star", "#C8102E")] },
    Holiday { id: "fathers", name: "Father's Day", bg: ("#0C2233", "#1E4A66"), card: "#0F2536", frame: "#7FC8FF", accent: "#CDEBFF", motifs: &[("tie", "#5AA9E6"), ("star", "#CDEBFF"), ("tie", "#F2B705")] },
    Holiday { id: "juneteenth", name: "Juneteenth", bg: ("#1A0D2E", "#3B1E5C"), card: "#170C28", frame: "#E0312B", accent: "#FFFFFF", motifs: &[("burst", "#E0312B"), ("star", "#FFFFFF"), ("burst", "#2E5BBA")] },
    Holiday { id: "independence", name: "Fourth of July", bg: ("#08142E", "#1D3672"), card: "#0D1A38", frame: "#C8102E", accent: "#FFFFFF", motifs: &[("firework", "#E8394A"), ("flag", "#FFFFFF"), ("firework", "#7FC8FF"), ("star", "#FFFFFF")] },
    Holiday { id: "labor", name: "Labor Day", bg: ("#0E2238", "#28507A"), card: "#10233A", frame: "#F2B705", accent: "#FFE08A", motifs: &[("star", "#F2B705"), ("flag", "#FFFFFF")] },
    Holiday { id: "halloween", name: "Halloween", bg: ("#14081F", "#3A1640"), card: "#1A0C22", frame: "#F28C1B", accent: "#FFB347", motifs: &[("pumpkin", "#F28C1B"), ("bat", "#8A6AA8"), ("moon", "#F5E9B8"), ("bat", "#B08AD0")] },
    Holiday { id: "veterans", name: "Veterans Day", bg: ("#1B2420", "#3C4A3A"), card: "#1E2620", frame: "#C9A44D", accent: "#F2C94C", motifs: &[("flag", "#FFFFFF"), ("star", "#C9CED6"), ("star", "#F2C94C")] },
    Holiday { id: "thanksgiving", name: "Thanksgiving", bg: ("#2A1206", "#6B3514"), card: "#24110A", frame: "#E0A040", accent: "#FFD08A", motifs: &[("leaf", "#D9611E"), ("pumpkin", "#E58A1F"), ("leaf", "#B8401A"), ("leaf", "#E6B33A")] },
    Holiday { id: "christmas_eve", name: "Christmas Eve", bg: ("#06241A", "#0E4B33"), card: "#0A2018", frame: "#D9B84A", accent: "#FFFFFF", motifs: &[("snowflake", "#FFFFFF"), ("ornament", "#C8102E"), ("star", "#F3D46B"), ("bell", "#D9B84A")] },
    Holiday { id: "christmas", name: "Christmas", bg: ("#3A0A10", "#7A1422"), card: "#2C080E", frame: "#D9B84A", accent: "#FFFFFF", motifs: &[("ornament", "#1E8A4A"), ("snowflake", "#FFFFFF"), ("star", "#F3D46B"), ("ornament", "#F3D46B")] },
    Holiday { id: "new_years_eve", name: "New Year's Eve", bg: ("#0B0B24", "#2B1A4F"), card: "#100F2B", frame: "#D9B84A", accent: "#F3D46B", motifs: &[("firework", "#F3D46B"), ("confetti", "#FFFFFF"), ("firework", "#E634E8"), ("star", "#FFFFFF")] },
];

pub fn get(id: &str) -> Option<&'static Holiday> {
    HOLIDAYS.iter().find(|h| h.id == id)
}

fn nth_weekday(year: i32, month: u32, weekday: Weekday, n: u32) -> Option<NaiveDate> {
    NaiveDate::from_weekday_of_month_opt(year, month, weekday, n as u8)
}

fn last_weekday(year: i32, month: u32, weekday: Weekday) -> Option<NaiveDate> {
    let next = if month == 12 { NaiveDate::from_ymd_opt(year + 1, 1, 1) } else { NaiveDate::from_ymd_opt(year, month + 1, 1) }?;
    let mut d = next - Duration::days(1);
    while d.weekday() != weekday {
        d -= Duration::days(1);
    }
    Some(d)
}

/// Western Easter (anonymous Gregorian algorithm).
fn easter(year: i32) -> Option<NaiveDate> {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32)
}

/// The holiday's date in `year`.
pub fn date_in(id: &str, year: i32) -> Option<NaiveDate> {
    let ymd = |m, d| NaiveDate::from_ymd_opt(year, m, d);
    match id {
        "new_year" => ymd(1, 1),
        "mlk" => nth_weekday(year, 1, Weekday::Mon, 3),
        "valentines" => ymd(2, 14),
        "presidents" => nth_weekday(year, 2, Weekday::Mon, 3),
        "st_patricks" => ymd(3, 17),
        "easter" => easter(year),
        "mothers" => nth_weekday(year, 5, Weekday::Sun, 2),
        "memorial" => last_weekday(year, 5, Weekday::Mon),
        "fathers" => nth_weekday(year, 6, Weekday::Sun, 3),
        "juneteenth" => ymd(6, 19),
        "independence" => ymd(7, 4),
        "labor" => nth_weekday(year, 9, Weekday::Mon, 1),
        "halloween" => ymd(10, 31),
        "veterans" => ymd(11, 11),
        "thanksgiving" => nth_weekday(year, 11, Weekday::Thu, 4),
        "christmas_eve" => ymd(12, 24),
        "christmas" => ymd(12, 25),
        "new_years_eve" => ymd(12, 31),
        _ => None,
    }
}

/// The next time the holiday comes round (today counts).
pub fn next_date(id: &str, today: NaiveDate) -> Option<NaiveDate> {
    let this_year = date_in(id, today.year())?;
    if this_year >= today { Some(this_year) } else { date_in(id, today.year() + 1) }
}

/// The forecast for one date from the NWS periods: (high/low text, pop text, condition, daypart).
fn forecast_for(data: &WxSnapshot, date: NaiveDate, settings: &Settings) -> Option<(String, String, String, &'static str)> {
    let overview = data.overview.as_ref()?;
    let on_date = |p: &&crate::data::Period| {
        p.start_time
            .as_deref()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            .is_some_and(|t| t.with_timezone(&Local).date_naive() == date)
    };
    let periods: Vec<&crate::data::Period> = overview.forecast.iter().filter(on_date).collect();
    let day = periods.iter().find(|p| p.is_daytime).or(periods.first())?;
    let night = periods.iter().find(|p| !p.is_daytime);
    let pop = periods.iter().filter_map(|p| p.precipitation_probability).fold(None::<f64>, |a, b| Some(a.map_or(b, |a| a.max(b))));
    let temp = super::bind::temp(day.temperature, settings.units);
    let _ = night;
    Some((
        format!("{temp}°"),
        pop.map(|p| format!("{p:.0}%")).unwrap_or_else(|| "0%".into()),
        day.short_forecast.clone(),
        if day.is_daytime { "day" } else { "night" },
    ))
}

pub fn bind(b: &mut Bindings, data: &WxSnapshot, settings: &Settings) {
    let today = Local::now().date_naive();
    let mut next: Option<(NaiveDate, &Holiday)> = None;
    for h in HOLIDAYS {
        let Some(date) = next_date(h.id, today) else { continue };
        let k = |field: &str| format!("holiday.{}.{field}", h.id);
        b.set(&k("name"), h.name);
        b.set(&k("date"), &date.format("%A, %B %-d").to_string());
        b.set(&k("weekday"), &date.format("%A").to_string());
        match forecast_for(data, date, settings) {
            Some((hi, pop, condition, daypart)) => {
                b.set(&k("hi"), &hi);
                b.set(&k("hi_label"), "High");
                b.set(&k("pop"), &pop);
                b.set(&k("pop_line"), &format!("{pop} rain chance"));
                b.set(&k("icon"), &condition);
                b.set(&k("condition"), &condition);
                b.set(&k("daypart"), daypart);
                b.set(&k("source"), "Forecast: National Weather Service");
            }
            None => {
                // Past the NWS 7-day forecast: what's normal for the date, from NOAA's
                // 1991-2020 normals for home (no made-up icon, no "--"; CWXB-74).
                let deg = |v: f32| match settings.units {
                    crate::config::Units::Metric => format!("{:.0}°", (v - 32.0) * 5.0 / 9.0),
                    _ => format!("{v:.0}°"),
                };
                let i = crate::climate::DayNormals::index(date);
                let n = data.climate.normals.as_ref();
                let normal = n.and_then(|n| Some((*n.hi.get(i)?, *n.lo.get(i)?)));
                b.set(&k("icon"), "No icon");
                b.set(&k("daypart"), "day");
                match normal {
                    Some((hi, lo)) => {
                        b.set(&k("hi"), &deg(hi));
                        b.set(&k("hi_label"), "Normal high");
                        b.set(&k("pop"), "");
                        b.set(&k("pop_line"), &format!("Normal low {}", deg(lo)));
                        let rec = n.and_then(|n| n.rec_hi.get(i)).map(|(v, y)| format!("Record high {} ({y})", deg(*v)));
                        b.set(&k("condition"), &rec.unwrap_or_else(|| "What's normal for the day".into()));
                        b.set(&k("source"), &format!("{} • forecast within 7 days", n.map(|n| n.source_label()).unwrap_or_default()));
                    }
                    None => {
                        b.set(&k("hi"), "Soon");
                        b.set(&k("hi_label"), "Forecast");
                        b.set(&k("pop"), "");
                        b.set(&k("pop_line"), "Within 7 days of the day");
                        b.set(&k("condition"), "Forecast coming closer to the day");
                        b.set(&k("source"), "Forecast: National Weather Service");
                    }
                }
            }
        }
        if (date - today).num_days() <= 7 && next.is_none_or(|(d, _)| date < d) {
            next = Some((date, h));
        }
    }
    if let Some((_, h)) = next {
        b.set("holiday.next.id", h.id);
        for field in ["name", "date", "weekday", "hi", "hi_label", "pop", "pop_line", "icon", "condition", "daypart", "source"] {
            let v = b.get(&format!("holiday.{}.{field}", h.id)).unwrap_or_default().to_string();
            b.set(&format!("holiday.next.{field}"), &v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holiday_dates() {
        let d = |y, m, dd| NaiveDate::from_ymd_opt(y, m, dd);
        assert_eq!(date_in("easter", 2026), d(2026, 4, 5));
        assert_eq!(date_in("easter", 2027), d(2027, 3, 28));
        assert_eq!(date_in("thanksgiving", 2026), d(2026, 11, 26));
        assert_eq!(date_in("memorial", 2026), d(2026, 5, 25));
        assert_eq!(date_in("mlk", 2027), d(2027, 1, 18));
        assert_eq!(date_in("labor", 2026), d(2026, 9, 7));
        assert_eq!(next_date("veterans", d(2026, 11, 12).unwrap()), d(2027, 11, 11));
        for h in HOLIDAYS {
            assert!(date_in(h.id, 2026).is_some(), "{}", h.id);
        }
    }
}
