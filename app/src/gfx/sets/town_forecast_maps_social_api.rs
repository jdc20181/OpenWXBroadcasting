/// A validated day/night pair for the town-map family. This accepts NWS local
/// day indices 0 through 6 while preserving the original three-day city API.
#[derive(Clone)]
pub(super) struct CityMapDay {
    pub date: String,
    pub high: Option<(String, f32)>,
    pub low: Option<(String, f32)>,
    pub daytime_pop: Option<f32>,
    pub nighttime_pop: Option<f32>,
    pub has_daytime: bool,
    pub issue: String,
}

pub(super) fn city_map_day(s: &CitySnapshot, day: u8, now: i64) -> Option<CityMapDay> {
    if day > 6
        || !(0..=1200).contains(&(now - s.0.checked))
        || !(0..=86400).contains(&(now - s.0.issued))
    {
        return None;
    }
    let (d, n) = selected(&s.0, day, now)?;
    let period = d.or(n)?;
    let format_temp =
        |p: Option<&Period>| p.and_then(|p| p.temp.map(|v| (format!("{v:.0}°F"), v as f32)));
    Some(CityMapDay {
        date: period.start.format("%a %b %d").to_string(),
        high: format_temp(d),
        low: format_temp(n),
        daytime_pop: d.and_then(|p| p.pop.map(|v| v as f32)),
        nighttime_pop: n.and_then(|p| p.pop.map(|v| v as f32)),
        has_daytime: d.is_some(),
        issue: chrono::DateTime::from_timestamp(s.0.issued, 0)
            .map(|t| format!("Updated {} UTC", t.format("%b %d %H:%M")))
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod town_map_period_tests {
    use super::*;

    fn fixture(now: i64) -> CitySnapshot {
        let start = chrono::DateTime::parse_from_rfc3339("2026-11-14T06:00:00-05:00").unwrap();
        let periods = (0..14)
            .map(|i| {
                let begins = start + chrono::Duration::hours(i * 12);
                Period {
                    start: begins,
                    end: (begins + chrono::Duration::hours(12)).timestamp(),
                    day: i % 2 == 0,
                    temp: Some(70. - i as f64),
                    short: "Forecast wording".into(),
                    wind: String::new(),
                    pop: Some(10. + i as f64 * 5.),
                }
            })
            .collect();
        CitySnapshot(Snapshot {
            home: [0, 0],
            checked: now,
            issued: now - 60,
            place: "Sample town".into(),
            periods,
        })
    }

    #[test]
    fn map_api_exposes_day_seven_and_distinct_night_pop() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-11-14T12:00:00-05:00")
            .unwrap()
            .timestamp();
        let snapshot = fixture(now);
        let tonight = city_map_day(&snapshot, 0, now).unwrap();
        assert_eq!(tonight.daytime_pop, Some(10.));
        assert_eq!(tonight.nighttime_pop, Some(15.));
        let day_seven = city_map_day(&snapshot, 6, now).unwrap();
        assert_eq!(day_seven.high.as_ref().map(|v| v.1), Some(58.));
        assert_eq!(day_seven.low.as_ref().map(|v| v.1), Some(57.));
        assert_eq!(day_seven.date, "Fri Nov 20");
        assert_eq!(day_seven.daytime_pop, Some(70.));
        assert_eq!(day_seven.nighttime_pop, Some(75.));
        assert!(city_map_day(&snapshot, 7, now).is_none());
    }
}
