//! Sun, moon and season times, computed locally (no feed needed).
//! Sunrise / sunset use the standard sunrise equation (about a minute of accuracy).

use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone, Utc};

fn jd_from_unix(t: f64) -> f64 {
    t / 86_400.0 + 2_440_587.5
}

fn unix_from_jd(jd: f64) -> f64 {
    (jd - 2_440_587.5) * 86_400.0
}

/// Sunrise and sunset (local time) for a date at a place; None in polar day / night.
pub fn sun_times(date: NaiveDate, lat: f64, lon: f64) -> Option<(DateTime<Local>, DateTime<Local>)> {
    let noon = Utc.from_utc_datetime(&date.and_hms_opt(12, 0, 0)?).timestamp() as f64;
    let n = (jd_from_unix(noon) - 2_451_545.0 + 0.0008).round();
    let j_star = n - lon / 360.0;
    let m = (357.5291 + 0.985_600_28 * j_star).rem_euclid(360.0);
    let mr = m.to_radians();
    let c = 1.9148 * mr.sin() + 0.02 * (2.0 * mr).sin() + 0.0003 * (3.0 * mr).sin();
    let lambda = (m + c + 180.0 + 102.9372).rem_euclid(360.0).to_radians();
    let transit = 2_451_545.0 + j_star + 0.0053 * mr.sin() - 0.0069 * (2.0 * lambda).sin();
    let decl = (lambda.sin() * 23.4397f64.to_radians().sin()).asin();
    let phi = lat.to_radians();
    let cos_w = ((-0.833f64).to_radians().sin() - phi.sin() * decl.sin()) / (phi.cos() * decl.cos());
    if !(-1.0..=1.0).contains(&cos_w) {
        return None;
    }
    let w = cos_w.acos().to_degrees();
    let rise = unix_from_jd(transit - w / 360.0) as i64;
    let set = unix_from_jd(transit + w / 360.0) as i64;
    Some((
        Local.timestamp_opt(rise, 0).single()?,
        Local.timestamp_opt(set, 0).single()?,
    ))
}

/// Daylight length in minutes.
pub fn daylight_minutes(date: NaiveDate, lat: f64, lon: f64) -> Option<i64> {
    let (r, s) = sun_times(date, lat, lon)?;
    Some((s - r).num_minutes())
}

pub fn duration_text(minutes: i64) -> String {
    format!("{} hr {} min", minutes / 60, minutes % 60)
}

/// Moon age in days (0 = new) and illuminated fraction (0-1) at a time.
pub fn moon(at: DateTime<Utc>) -> (f64, f64) {
    const SYNODIC: f64 = 29.530_588_853;
    let jd = jd_from_unix(at.timestamp() as f64);
    let age = ((jd - 2_451_550.1) / SYNODIC).rem_euclid(1.0) * SYNODIC;
    let illum = (1.0 - (2.0 * std::f64::consts::PI * age / SYNODIC).cos()) / 2.0;
    (age, illum)
}

pub fn moon_phase_name(age: f64) -> &'static str {
    match age {
        a if a < 1.85 => "New Moon",
        a if a < 5.54 => "Waxing Crescent",
        a if a < 9.23 => "First Quarter",
        a if a < 12.92 => "Waxing Gibbous",
        a if a < 16.61 => "Full Moon",
        a if a < 20.30 => "Waning Gibbous",
        a if a < 23.99 => "Last Quarter",
        a if a < 27.68 => "Waning Crescent",
        _ => "New Moon",
    }
}

/// Next full and new moon dates after `at` (approximate, within a day).
pub fn next_full_and_new(at: DateTime<Utc>) -> (NaiveDate, NaiveDate) {
    const SYNODIC: f64 = 29.530_588_853;
    let (age, _) = moon(at);
    let to_full = (14.765 - age).rem_euclid(SYNODIC);
    let to_new = (SYNODIC - age).rem_euclid(SYNODIC);
    let d = |days: f64| (at + chrono::Duration::seconds((days * 86_400.0) as i64)).with_timezone(&Local).date_naive();
    (d(to_full), d(to_new))
}

/// The next equinox or solstice after today: (name, date).
pub fn next_season(today: NaiveDate) -> (&'static str, NaiveDate) {
    let y = today.year();
    let marks = |y: i32| {
        [
            ("Spring begins", NaiveDate::from_ymd_opt(y, 3, 20)),
            ("Summer begins", NaiveDate::from_ymd_opt(y, 6, 21)),
            ("Fall begins", NaiveDate::from_ymd_opt(y, 9, 22)),
            ("Winter begins", NaiveDate::from_ymd_opt(y, 12, 21)),
        ]
    };
    marks(y)
        .into_iter()
        .chain(marks(y + 1))
        .filter_map(|(n, d)| d.map(|d| (n, d)))
        .find(|(_, d)| *d > today)
        .unwrap_or(("Spring begins", today))
}
