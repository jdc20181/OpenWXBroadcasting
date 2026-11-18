pub(super) struct TownForecastMapArea {
    pub coverage: Coverage,
    pub label: String,
    pub scope: super::board::Scope,
}

/// Strict home-area scopes and immutable labels, computed once by the caller's
/// bounded geography cache rather than repeated for every format/day binding.
pub(super) fn town_forecast_map_areas(home: GeoView) -> Option<Vec<TownForecastMapArea>> {
    let all = crate::map::boundaries::counties_named()?;
    let home_county = unique_home_county(home, &all)?;
    let mut settings = Settings::default();
    settings.home = home;
    let mut areas = Vec::new();
    for coverage in [Coverage::County, Coverage::Region, Coverage::State] {
        let scope = observed::report_scope(&settings, coverage);
        match coverage {
            Coverage::County if scope.county.as_deref() != Some(home_county.fips.as_str()) => {
                return None;
            }
            Coverage::State if !scope.states.iter().any(|s| s == &home_county.state) => {
                return None;
            }
            _ => {}
        }
        let selected = board::selected(&all, &scope);
        if selected.is_empty() {
            return None;
        }
        let label = match coverage {
            Coverage::County => {
                let state = crate::winter::STATES
                    .iter()
                    .find(|(fips, _, _)| *fips == home_county.state)
                    .map(|(_, postal, _)| *postal)
                    .unwrap_or("");
                format!("{} County, {state}", home_county.name)
            }
            Coverage::Region => "Home region • selected county geography".into(),
            Coverage::State => crate::winter::STATES
                .iter()
                .find(|(fips, _, _)| *fips == home_county.state)
                .map(|(_, _, name)| (*name).to_owned())
                .unwrap_or_else(|| "Home state".into()),
            Coverage::Multi => "Selected states".into(),
        };
        areas.push(TownForecastMapArea {
            coverage,
            label,
            scope,
        });
    }
    Some(areas)
}

#[derive(Clone)]
pub(super) struct TownForecastMapRow {
    pub name: String,
    pub lon: f64,
    pub lat: f64,
    pub values: Option<super::social_forecast::CityMapDay>,
}

/// Values from the current bounded city snapshot; no new upstream requests.
pub(super) fn town_forecast_map_rows(
    home: GeoView,
    coverage: Coverage,
    day: u8,
    now: i64,
) -> Option<Vec<TownForecastMapRow>> {
    let snapshot = store()
        .lock()
        .ok()?
        .current
        .as_ref()
        .filter(|s| s.home == home_key(home) && (0..=MAX_AGE).contains(&(now - s.checked)))
        .cloned()?;
    let rows = snapshot.scopes.get(coverage_key(coverage))?;
    Some(
        rows.iter()
            .map(|row| TownForecastMapRow {
                name: row.town.label(),
                lon: row.town.point.lon,
                lat: row.town.point.lat,
                values: row
                    .forecast
                    .as_ref()
                    .and_then(|f| super::social_forecast::city_map_day(f, day, now)),
            })
            .collect(),
    )
}
