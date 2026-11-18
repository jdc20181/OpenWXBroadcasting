//! NOAA Regional Climate Centers ACIS (data.rcc-acis.org): the daily station climate data
//! behind NWS NOWData. Used for past-day history at home (yesterday, month to date, year to
//! date, season snowfall, this date in past years) and daily record highs / lows.
//! Keyless; no relay quota. Completed days don't change, so nothing here is live weather.

use anyhow::{anyhow, Result};
use chrono::NaiveDate;

const BASE: &str = "https://data.rcc-acis.org";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Station {
    /// ACIS station id ("129253 2").
    pub sid: String,
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}

/// Elements in StnMeta / StnData order.
const ELEMS: &str = "maxt,mint,pcpn,snow";

async fn get(path: &str, params: serde_json::Value) -> Result<serde_json::Value> {
    let url = format!("{BASE}/{path}?params={}", urlencode(&params.to_string()));
    let v = crate::net::get_json(&url).await?;
    if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
        return Err(anyhow!("ACIS {path}: {e}"));
    }
    Ok(v)
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, la2) = (a.0.to_radians(), b.0.to_radians());
    let dla = la2 - la1;
    let dlo = (b.1 - a.1).to_radians();
    let h = (dla / 2.).sin().powi(2) + la1.cos() * la2.cos() * (dlo / 2.).sin().powi(2);
    6371. * 2. * h.sqrt().asin()
}

/// One candidate from StnMeta: station plus the (start, end) of each element's record.
#[derive(Clone, Debug)]
struct Candidate {
    station: Station,
    ranges: [Option<(NaiveDate, NaiveDate)>; 4],
}

fn parse_candidates(v: &serde_json::Value) -> Vec<Candidate> {
    let date = |x: &serde_json::Value| x.as_str().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    v.get("meta").and_then(|m| m.as_array()).into_iter().flatten().filter_map(|m| {
        let ll = m.get("ll")?.as_array()?;
        let (lon, lat) = (ll.first()?.as_f64()?, ll.get(1)?.as_f64()?);
        let sid = m.get("sids")?.as_array()?.first()?.as_str()?.to_string();
        let name = m.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
        let mut ranges = [None; 4];
        for (i, r) in m.get("valid_daterange").and_then(|r| r.as_array()).into_iter().flatten().take(4).enumerate() {
            let pair = r.as_array();
            ranges[i] = pair.and_then(|p| Some((date(p.first()?)?, date(p.get(1)?)?)));
        }
        Some(Candidate { station: Station { sid, name, lat, lon }, ranges })
    }).collect()
}

/// The nearest station whose records for `elems` (indexes into maxt, mint, pcpn, snow) start
/// on or before `since` and are current (reported within the last two weeks).
fn nearest(cands: &[Candidate], home: (f64, f64), elems: &[usize], since: NaiveDate, today: NaiveDate) -> Option<Station> {
    let recent = today - chrono::Duration::days(14);
    cands.iter()
        .filter(|c| elems.iter().all(|&i| c.ranges[i].is_some_and(|(s, e)| s <= since && e >= recent)))
        .min_by(|a, b| km(home, (a.station.lat, a.station.lon)).total_cmp(&km(home, (b.station.lat, b.station.lon))))
        .map(|c| c.station.clone())
}

/// Stations for home: (temperature + precipitation station, snowfall station). Prefers long
/// records (30+ years, for records), falling back to 11 years (enough for the season charts).
pub async fn stations(lat: f64, lon: f64) -> Result<(Station, Option<Station>)> {
    let today = chrono::Local::now().date_naive();
    let mut last_err = None;
    for pad in [0.6, 1.2] {
        let bbox = format!("{:.3},{:.3},{:.3},{:.3}", lon - pad, lat - pad, lon + pad, lat + pad);
        let v = match get("StnMeta", serde_json::json!({ "bbox": bbox, "elems": ELEMS, "meta": "name,sids,ll,valid_daterange" })).await {
            Ok(v) => v,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };
        let cands = parse_candidates(&v);
        let long = NaiveDate::from_ymd_opt(1995, 1, 1).unwrap_or(today);
        let short = today - chrono::Duration::days(11 * 366);
        let temp = nearest(&cands, (lat, lon), &[0, 1, 2], long, today).or_else(|| nearest(&cands, (lat, lon), &[0, 1, 2], short, today));
        if let Some(temp) = temp {
            let snow = nearest(&cands, (lat, lon), &[3], short, today);
            return Ok((temp, snow));
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("no NOAA climate station with current records near home")))
}

/// One ACIS daily value: "M" missing → NaN, "T" trace → 0, "S" (included in a later total) →
/// NaN, flags such as "0.25A" (accumulated) keep the number.
pub fn value(s: &str) -> f32 {
    match s.trim() {
        "T" => 0.0,
        "M" | "S" | "" => f32::NAN,
        v => v.trim_end_matches(|c: char| c.is_ascii_alphabetic()).parse().unwrap_or(f32::NAN),
    }
}

/// Daily max temp, min temp, precipitation and snowfall for `sid`, `start..=end`.
pub async fn daily(sid: &str, start: NaiveDate, end: NaiveDate) -> Result<Vec<(NaiveDate, [f32; 4])>> {
    let v = get("StnData", serde_json::json!({ "sid": sid, "sdate": start.to_string(), "edate": end.to_string(), "elems": ELEMS })).await?;
    Ok(parse_daily(&v))
}

fn parse_daily(v: &serde_json::Value) -> Vec<(NaiveDate, [f32; 4])> {
    v.get("data").and_then(|d| d.as_array()).into_iter().flatten().filter_map(|row| {
        let row = row.as_array()?;
        let date = NaiveDate::parse_from_str(row.first()?.as_str()?, "%Y-%m-%d").ok()?;
        let mut vals = [f32::NAN; 4];
        for (i, slot) in vals.iter_mut().enumerate() {
            *slot = row.get(i + 1).and_then(|x| x.as_str()).map(value).unwrap_or(f32::NAN);
        }
        Some((date, vals))
    }).collect()
}

/// Daily record high and low for each day of the year (366 slots, Feb 29 at index 59), with
/// the year each was set, over the station's full period of record.
pub async fn records(sid: &str) -> Result<(Vec<(f32, i32)>, Vec<(f32, i32)>)> {
    let elem = |name: &str, reduce: &str| serde_json::json!({
        "name": name, "interval": "dly", "duration": "dly",
        "smry": { "reduce": reduce, "add": "date" }, "smry_only": 1, "groupby": "year"
    });
    let v = get("StnData", serde_json::json!({ "sid": sid, "sdate": "por", "edate": "por", "elems": [elem("maxt", "max"), elem("mint", "min")] })).await?;
    parse_records(&v)
}

fn parse_records(v: &serde_json::Value) -> Result<(Vec<(f32, i32)>, Vec<(f32, i32)>)> {
    let smry = v.get("smry").and_then(|s| s.as_array()).ok_or_else(|| anyhow!("ACIS records: no summary"))?;
    let column = |i: usize| -> Vec<(f32, i32)> {
        smry.get(i).and_then(|c| c.as_array()).into_iter().flatten().map(|cell| {
            let cell = cell.as_array();
            let val = cell.and_then(|c| c.first()).and_then(|x| x.as_str()).map(value).unwrap_or(f32::NAN);
            let year = cell.and_then(|c| c.get(1)).and_then(|x| x.as_str()).and_then(|d| d.get(..4)).and_then(|y| y.parse().ok()).unwrap_or(0);
            (val, year)
        }).collect()
    };
    let (hi, lo) = (column(0), column(1));
    anyhow::ensure!(hi.len() == 366 && lo.len() == 366, "ACIS records: expected 366 days");
    Ok((hi, lo))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_follow_acis_flags() {
        assert_eq!(value("T"), 0.0);
        assert!(value("M").is_nan());
        assert!(value("S").is_nan());
        assert_eq!(value("0.25A"), 0.25);
        assert_eq!(value("-7"), -7.0);
    }

    #[test]
    fn parses_daily_rows() {
        // Real shape from StnData (WASHINGTON 1 W, IN, Nov 2026).
        let v: serde_json::Value = serde_json::from_str(r#"{"meta":{"name":"WASHINGTON 1 W"},"data":[["2026-11-01","73","55","0.00","0.0"],["2026-11-02","62","54","T","M"]]}"#).unwrap();
        let rows = parse_daily(&v);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].1[2], 0.0);
        assert!(rows[1].1[3].is_nan());
    }

    #[test]
    fn parses_record_summary_with_years() {
        let day = r#"["67","1952-01-01"]"#;
        let col = std::iter::repeat_n(day, 366).collect::<Vec<_>>().join(",");
        let v: serde_json::Value = serde_json::from_str(&format!(r#"{{"smry":[[{col}],[{col}]]}}"#)).unwrap();
        let (hi, lo) = parse_records(&v).unwrap();
        assert_eq!(hi[0], (67.0, 1952));
        assert_eq!(lo.len(), 366);
    }

    #[test]
    fn picks_nearest_current_long_record_station() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
        let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        let mk = |sid: &str, lat: f64, start: &str, end: &str, snow: bool| Candidate {
            station: Station { sid: sid.into(), name: sid.into(), lat, lon: -86.9 },
            ranges: [Some((d(start), d(end))), Some((d(start), d(end))), Some((d(start), d(end))), snow.then(|| (d(start), d(end)))],
        };
        let cands = vec![
            mk("closed", 38.40, "1950-01-01", "2009-09-30", true),
            mk("near-new", 38.39, "2020-01-01", "2026-11-15", false),
            mk("far-long", 38.70, "1896-06-01", "2026-11-15", true),
        ];
        let since = d("1995-01-01");
        assert_eq!(nearest(&cands, (38.39, -86.9), &[0, 1, 2], since, today).unwrap().sid, "far-long");
    }
}
