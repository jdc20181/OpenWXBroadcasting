//! Native holiday cards bind only to NWS periods for that exact local calendar date.
use crate::{
    config::Settings,
    gfx::{
        bind::Bindings,
        model::{GraphicDoc, LayerKind},
    },
};
use chrono::{Datelike, Duration, NaiveDate, Weekday};
#[derive(Clone, Copy)]
enum Holiday {
    NewYear,
    Mlk,
    Valentines,
    Presidents,
    StPatricks,
    Easter,
    Mothers,
    Memorial,
    Fathers,
    Juneteenth,
    Independence,
    Labor,
    Halloween,
    Veterans,
    Thanksgiving,
    ChristmasEve,
    Christmas,
    NewYearsEve,
}
#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    holiday: Holiday,
    format: u8,
}
include!("holiday_forecast_specs.rs");
impl Holiday {
    fn slug(self) -> &'static str {
        match self {
            Self::NewYear => "new_year",
            Self::Mlk => "mlk",
            Self::Valentines => "valentines",
            Self::Presidents => "presidents",
            Self::StPatricks => "st_patricks",
            Self::Easter => "easter",
            Self::Mothers => "mothers",
            Self::Memorial => "memorial",
            Self::Fathers => "fathers",
            Self::Juneteenth => "juneteenth",
            Self::Independence => "independence",
            Self::Labor => "labor",
            Self::Halloween => "halloween",
            Self::Veterans => "veterans",
            Self::Thanksgiving => "thanksgiving",
            Self::ChristmasEve => "christmas_eve",
            Self::Christmas => "christmas",
            Self::NewYearsEve => "new_years_eve",
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::NewYear => "New Year's Day",
            Self::Mlk => "Martin Luther King Jr. Day",
            Self::Valentines => "Valentine's Day",
            Self::Presidents => "Presidents' Day",
            Self::StPatricks => "St. Patrick's Day",
            Self::Easter => "Easter",
            Self::Mothers => "Mother's Day",
            Self::Memorial => "Memorial Day",
            Self::Fathers => "Father's Day",
            Self::Juneteenth => "Juneteenth",
            Self::Independence => "Fourth of July",
            Self::Labor => "Labor Day",
            Self::Halloween => "Halloween",
            Self::Veterans => "Veterans Day",
            Self::Thanksgiving => "Thanksgiving",
            Self::ChristmasEve => "Christmas Eve",
            Self::Christmas => "Christmas",
            Self::NewYearsEve => "New Year's Eve",
        }
    }
    fn date(self, year: i32) -> Option<NaiveDate> {
        if !(1900..=4099).contains(&year) {
            return None;
        }
        let nth = |month, weekday: Weekday, n: u32| {
            let first = NaiveDate::from_ymd_opt(year, month, 1)?;
            let delta =
                (weekday.num_days_from_monday() + 7 - first.weekday().num_days_from_monday()) % 7;
            first.checked_add_signed(Duration::days(i64::from(delta + 7 * (n - 1))))
        };
        match self {
            Self::NewYear => NaiveDate::from_ymd_opt(year, 1, 1),
            Self::Mlk => crate::gfx::holidays::date_in("mlk", year),
            Self::Valentines => crate::gfx::holidays::date_in("valentines", year),
            Self::Presidents => crate::gfx::holidays::date_in("presidents", year),
            Self::StPatricks => crate::gfx::holidays::date_in("st_patricks", year),
            Self::ChristmasEve => NaiveDate::from_ymd_opt(year, 12, 24),
            Self::Christmas => crate::gfx::holidays::date_in("christmas", year),
            Self::NewYearsEve => NaiveDate::from_ymd_opt(year, 12, 31),
            Self::Mothers => nth(5, Weekday::Sun, 2),
            Self::Fathers => nth(6, Weekday::Sun, 3),
            Self::Labor => nth(9, Weekday::Mon, 1),
            Self::Memorial => {
                let last = NaiveDate::from_ymd_opt(year, 5, 31)?;
                last.checked_sub_signed(Duration::days(i64::from(
                    last.weekday().num_days_from_monday(),
                )))
            }
            Self::Juneteenth => crate::gfx::holidays::date_in("juneteenth", year),
            Self::Independence => crate::gfx::holidays::date_in("independence", year),
            Self::Easter => {
                // Gregorian ecclesiastical calculation documented by USNO (Oudin, 1940).
                let c = year / 100;
                let n = year - 19 * (year / 19);
                let k = (c - 17) / 25;
                let mut i = c - c / 4 - (c - k) / 3 + 19 * n + 15;
                i -= 30 * (i / 30);
                i -= (i / 28) * (1 - (i / 28) * (29 / (i + 1)) * ((21 - n) / 11));
                let mut j = year + year / 4 + i + 2 - c + c / 4;
                j -= 7 * (j / 7);
                let l = i - j;
                let m = 3 + (l + 40) / 44;
                let d = l + 28 - 31 * (m / 4);
                NaiveDate::from_ymd_opt(year, m as u32, d as u32)
            }
            Self::Halloween => crate::gfx::holidays::date_in("halloween", year),
            Self::Veterans => crate::gfx::holidays::date_in("veterans", year),
            Self::Thanksgiving => crate::gfx::holidays::date_in("thanksgiving", year),
        }
    }
    fn next(self, today: NaiveDate) -> Option<NaiveDate> {
        [today.year(), today.year() + 1]
            .into_iter()
            .filter_map(|y| self.date(y))
            .find(|d| *d >= today)
    }
}
impl Holiday {
    fn needs_title_fit(self) -> bool {
        matches!(
            self,
            Self::Mlk
                | Self::Valentines
                | Self::Presidents
                | Self::StPatricks
                | Self::Juneteenth
                | Self::Independence
                | Self::Halloween
                | Self::Veterans
                | Self::Thanksgiving
                | Self::Christmas
        )
    }
}
pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
    let today = super::social_forecast::home_date(settings.home, now);
    for spec in SPECS.iter().filter(|s| s.format == 1) {
        let holiday = spec.holiday;
        let target = today.and_then(|d| holiday.next(d));
        for (field, value) in
            super::social_forecast::named_date_values(settings.home, target, holiday.name(), now)
        {
            b.set(
                &format!("set.holidayforecast.{}.{}", holiday.slug(), field),
                &value,
            );
        }
    }
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let s = *SPECS.iter().find(|s| s.key == key)?;
    let suffix = if s.format == 1 {
        "_portrait"
    } else {
        "_square"
    };
    let mut doc =
        super::social_forecast::build(&format!("set_social_forecast__today_point{suffix}"))?;
    for layer in &mut doc.layers {
        if let LayerKind::Text {
            text,
            auto_fit,
            wrap,
            ..
        } = &mut layer.kind
        {
            *text = text.replace(
                "set.socialforecast.0.",
                &format!("set.holidayforecast.{}.", s.holiday.slug()),
            );
            if layer.name == "Social Title" {
                *text = format!("{} Forecast", s.holiday.name());
                if s.holiday.needs_title_fit() {
                    *auto_fit = true;
                    *wrap = false;
                }
            }
        }
    }
    Some(doc)
}
#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(s) = SPECS
        .iter()
        .find(|s| Some(s.key) == doc.template.as_deref())
    else {
        return;
    };
    let target = s.holiday.next(chrono::Utc::now().date_naive()).unwrap();
    let prefix = format!("set.holidayforecast.{}", s.holiday.slug());
    for (field, value) in [
        ("source", "SAMPLE ONLY • Not for broadcast".into()),
        ("place", "Near Jeffersonville, IN • SAMPLE".into()),
        (
            "period",
            format!("SAMPLE • {}", target.format("%a %b %d, %Y")),
        ),
        ("condition", "Chance of showers, then partly cloudy".into()),
        ("daylabel", "DAYTIME".into()),
        ("daytemp", "74°F".into()),
        ("nighttemp", "51°F".into()),
        ("wind", "Southwest 10 to 15 mph".into()),
        ("poplabel", "Daytime precip. chance".into()),
        ("pop", "40%".into()),
        ("issued", "SAMPLE • Synthetic NWS-style forecast".into()),
        (
            "note",
            "SAMPLE • Named holiday date • Not a real forecast".into(),
        ),
    ] {
        doc.data_overrides
            .push((format!("{prefix}.{field}"), value));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn newly_parameterized() -> [(Holiday, &'static str); 10] {
        [
            (Holiday::Mlk, "mlk"),
            (Holiday::Valentines, "valentines"),
            (Holiday::Presidents, "presidents"),
            (Holiday::StPatricks, "st_patricks"),
            (Holiday::Juneteenth, "juneteenth"),
            (Holiday::Independence, "independence"),
            (Holiday::Halloween, "halloween"),
            (Holiday::Veterans, "veterans"),
            (Holiday::Thanksgiving, "thanksgiving"),
            (Holiday::Christmas, "christmas"),
        ]
    }
    #[test]
    fn holiday_rules_handle_weekdays_easter_and_inclusive_year_rollover() {
        for (holiday, year, month, day) in [
            (Holiday::Easter, 2010, 4, 4),
            (Holiday::Easter, 2026, 4, 5),
            (Holiday::Easter, 2027, 3, 28),
            (Holiday::Mothers, 2026, 5, 10),
            (Holiday::Fathers, 2026, 6, 21),
            (Holiday::Memorial, 2026, 5, 25),
            (Holiday::Labor, 2026, 9, 7),
        ] {
            assert_eq!(
                holiday.date(year),
                NaiveDate::from_ymd_opt(year, month, day)
            );
        }
        let jan1 = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        assert_eq!(Holiday::NewYear.next(jan1), Some(jan1));
        assert_eq!(
            Holiday::NewYear.next(jan1 + Duration::days(1)),
            NaiveDate::from_ymd_opt(2027, 1, 1)
        );
        for year in 1900..=2099 {
            let easter = Holiday::Easter.date(year).unwrap();
            assert_eq!(easter.weekday(), Weekday::Sun);
            assert!(
                easter >= NaiveDate::from_ymd_opt(year, 3, 22).unwrap()
                    && easter <= NaiveDate::from_ymd_opt(year, 4, 25).unwrap()
            );
        }
    }

    #[test]
    fn new_holiday_dates_and_rollovers_delegate_to_the_shared_holiday_api() {
        for (holiday, id) in newly_parameterized() {
            for year in [2026, 2027, 2099] {
                assert_eq!(holiday.date(year), crate::gfx::holidays::date_in(id, year));
            }
            for year in [2026, 2027] {
                let date = crate::gfx::holidays::date_in(id, year).unwrap();
                for today in [date - Duration::days(1), date, date + Duration::days(1)] {
                    assert_eq!(
                        holiday.next(today),
                        crate::gfx::holidays::next_date(id, today),
                        "{id} from {today}"
                    );
                }
            }
            let year_end = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
            assert_eq!(
                holiday.next(year_end),
                crate::gfx::holidays::next_date(id, year_end),
                "{id} year rollover"
            );
        }
    }

    #[test]
    fn all_native_holiday_specs_are_unique_and_new_titles_fit_portrait_and_square() {
        let mut keys = HashSet::new();
        assert_eq!(SPECS.len(), 36);
        assert_eq!(SPECS.iter().filter(|s| s.format == 1).count(), 18);
        assert_eq!(SPECS.iter().filter(|s| s.format == 2).count(), 18);
        for spec in SPECS {
            assert!(keys.insert(spec.key), "duplicate holiday key {}", spec.key);
            assert!(spec.format == 1 || spec.format == 2);
            let doc = build(spec.key).expect("holiday format builds through shared social layout");
            assert_eq!(
                (doc.width, doc.height),
                if spec.format == 1 {
                    (1080, 1920)
                } else {
                    (1080, 1080)
                }
            );
        }
        for key in [
            "set_holiday_forecast__mlk_portrait",
            "set_holiday_forecast__mlk_square",
        ] {
            let doc = build(key).unwrap();
            let title = doc
                .layers
                .iter()
                .find(|layer| layer.name == "Social Title")
                .expect("native social title");
            match &title.kind {
                LayerKind::Text {
                    text,
                    auto_fit,
                    wrap,
                    ..
                } => {
                    assert_eq!(text.as_str(), "Martin Luther King Jr. Day Forecast");
                    assert!(*auto_fit, "long title is fitted to native title box");
                    assert!(!*wrap, "long holiday title remains on one line");
                }
                _ => panic!("Social Title remains a text layer"),
            }
        }
        assert_eq!(keys.len(), 36);
    }

    #[test]
    fn named_date_horizon_and_missing_forecast_values_remain_delegated() {
        let settings = Settings::default();
        let now = chrono::Utc::now().timestamp();
        let beyond_horizon = crate::gfx::holidays::date_in("christmas", 2099).unwrap();
        let values = super::super::social_forecast::named_date_values(
            settings.home,
            Some(beyond_horizon),
            "Christmas",
            now,
        );
        let field = |name: &str| {
            values
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(field("daytemp"), Some(""));
        assert_eq!(
            field("note"),
            Some("NWS forecast not yet available for the named date")
        );
        let expected_period = format!("Christmas • {}", beyond_horizon.format("%a %b %d, %Y"));
        assert_eq!(field("period"), Some(expected_period.as_str()));
    }

    #[test]
    fn new_holiday_sample_binds_through_the_named_identity_namespace() {
        let key = "set_holiday_forecast__thanksgiving_square";
        let mut doc = build(key).unwrap();
        doc.template = Some(key.into());
        sample(&mut doc);
        assert!(doc.data_overrides.iter().any(|(name, value)| {
            name == "set.holidayforecast.thanksgiving.period" && value.contains("SAMPLE")
        }));
        assert!(doc.data_overrides.iter().any(|(name, value)| {
            name == "set.holidayforecast.thanksgiving.note" && value.contains("Named holiday date")
        }));
    }
}
