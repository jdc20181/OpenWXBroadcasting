//! Sets-local reader for the NOAA RAP regional GRIB2 surface grid. No fallback provider is used.
use anyhow::{Context, Result, anyhow, ensure};
use chrono::NaiveDate;
use grib::{
    GridDefinitionTemplateValues,
    projection::{Ellipsoid, Lcc, LccParams, Project},
};
use std::{collections::BTreeMap, time::Duration};

const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_CELLS: usize = 4_000_000;
const MAX_RUN_AGE: i64 = 24 * 3600;
const MAX_LEAD: u32 = 48;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum Field {
    Temperature,
    Dewpoint,
    Humidity,
    WindU,
    WindV,
    Gust,
    Cloud,
    Pressure,
    Cape,
}
#[derive(Clone, Debug)]
pub(super) struct Site {
    pub id: String,
    pub lon: f64,
    pub lat: f64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Bounds {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}
impl Bounds {
    fn validate(self) -> Result<Self> {
        ensure!(
            [self.west, self.south, self.east, self.north]
                .iter()
                .all(|v| v.is_finite()),
            "RAP bounds are not finite"
        );
        ensure!(
            (-180. ..=180.).contains(&self.west)
                && (-180. ..=180.).contains(&self.east)
                && (-85. ..=85.).contains(&self.south)
                && (-85. ..=85.).contains(&self.north),
            "RAP bounds are outside geographic limits"
        );
        ensure!(
            self.west < self.east && self.south < self.north,
            "RAP bounds are empty"
        );
        let expanded = Self {
            west: self.west.floor(),
            south: self.south.floor(),
            east: self.east.ceil(),
            north: self.north.ceil(),
        };
        ensure!(
            expanded.east - expanded.west <= 20. && expanded.north - expanded.south <= 15.,
            "RAP relay region exceeds 20° × 15° after outward rounding"
        );
        Ok(expanded)
    }
}

struct Lambert {
    projection: Lcc,
    origin: (f64, f64),
    step: (f64, f64),
    ni: usize,
    nj: usize,
    lov: f64,
    cone: f64,
    uv_grid_relative: bool,
}
impl Lambert {
    fn from_template(s: &grib::def::grib2::template::Template3_30) -> Result<Self> {
        let deg = |v: f64| v * 1e-6;
        let (a, b) = s
            .earth_shape
            .radii()
            .context("unknown RAP GRIB Earth shape")?;
        let projection = Lcc::new(&LccParams {
            ellipsoid: Ellipsoid::from_a_and_b(a, b),
            lat_0: deg(s.lad as f64),
            lon_0: deg(s.lov as f64),
            lat_1: deg(s.latin1 as f64),
            lat_2: deg(s.latin2 as f64),
        })
        .map_err(anyhow::Error::msg)?;
        let origin = projection
            .project(
                &(
                    deg(s.first_point_lon as f64).to_radians(),
                    deg(s.first_point_lat as f64).to_radians(),
                ),
                false,
            )
            .map_err(anyhow::Error::msg)?;
        let step = (
            s.dx as f64
                * if s.scanning_mode.scans_positively_for_i() {
                    0.001
                } else {
                    -0.001
                },
            s.dy as f64
                * if s.scanning_mode.scans_positively_for_j() {
                    0.001
                } else {
                    -0.001
                },
        );
        let (ni, nj) = (s.ni as usize, s.nj as usize);
        ensure!(
            ni > 1
                && nj > 1
                && ni.checked_mul(nj).is_some_and(|n| n <= MAX_CELLS)
                && step.0.is_finite()
                && step.1.is_finite()
                && step.0 != 0.
                && step.1 != 0.,
            "RAP grid size or spacing is invalid"
        );
        let (lat1, lat2, lov_deg) = (
            deg(s.latin1 as f64),
            deg(s.latin2 as f64),
            deg(s.lov as f64),
        );
        let cone = cone_constant(lat1, lat2);
        let uv_grid_relative = (s.resolution_and_component_flags.0 & 0x08) != 0;
        Ok(Self {
            projection,
            origin,
            step,
            ni,
            nj,
            lov: lov_deg,
            cone,
            uv_grid_relative,
        })
    }
}

fn cone_constant(lat1: f64, lat2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    if (lat1 - lat2).abs() < 1e-6 {
        return p1.sin();
    }
    let t = |p: f64| (std::f64::consts::FRAC_PI_4 + p / 2.).tan();
    (p1.cos().ln() - p2.cos().ln()) / (t(p2).ln() - t(p1).ln())
}

/// Convert grid-relative Lambert wind components to earth-relative east/north components.
fn earth_wind(u: f32, v: f32, lon: f64, lov: f64, cone: f64) -> (f32, f32) {
    let d = ((lon - lov + 540.).rem_euclid(360.) - 180.).to_radians();
    let (s, c) = ((cone * d) as f32).sin_cos();
    (c * u + s * v, -s * u + c * v)
}

pub(super) struct Frame {
    pub run: i64,
    pub lead: u32,
    pub valid: i64,
    values: BTreeMap<Field, BTreeMap<String, f32>>,
}
impl Frame {
    pub(super) fn sample(&self, field: Field, id: &str) -> Option<f32> {
        self.values.get(&field)?.get(id).copied()
    }
    pub(super) fn wind_speed_ms(&self, id: &str) -> Option<f32> {
        let u = self.sample(Field::WindU, id)?;
        let v = self.sample(Field::WindV, id)?;
        let s = u.hypot(v);
        s.is_finite().then_some(s)
    }
}
fn sample_cells(grid: &Lambert, values: &[f32], lat: f64, lon: f64) -> Option<f32> {
    let (x, y) = grid
        .projection
        .project(&(lon.to_radians(), lat.to_radians()), false)
        .ok()?;
    let (x, y) = (
        (x - grid.origin.0) / grid.step.0,
        (y - grid.origin.1) / grid.step.1,
    );
    if !(0. ..=(grid.ni - 1) as f64).contains(&x) || !(0. ..=(grid.nj - 1) as f64).contains(&y) {
        return None;
    }
    let (i, j) = (x.floor() as usize, y.floor() as usize);
    let (i1, j1) = ((i + 1).min(grid.ni - 1), (j + 1).min(grid.nj - 1));
    let (dx, dy) = ((x - i as f64) as f32, (y - j as f64) as f32);
    let (mut sum, mut weight) = (0., 0.);
    for (a, b, w) in [
        (i, j, (1. - dx) * (1. - dy)),
        (i1, j, dx * (1. - dy)),
        (i, j1, (1. - dx) * dy),
        (i1, j1, dx * dy),
    ] {
        let v = values[b * grid.ni + a];
        if v.is_finite() {
            sum += v * w;
            weight += w;
        }
    }
    (weight > 0.5).then_some(sum / weight)
}

pub(super) async fn latest_run() -> Result<i64> {
    let v: serde_json::Value = crate::net::broadcaster_route("model-run", Duration::from_secs(30))
        .await?
        .json()
        .await?;
    let run = v
        .get("run")
        .and_then(serde_json::Value::as_i64)
        .context("NOAA relay returned no RAP run timestamp")?;
    ensure!(
        (0..=MAX_RUN_AGE).contains(&(chrono::Utc::now().timestamp() - run)),
        "Latest NOAA RAP run is stale or in the future"
    );
    Ok(run)
}
pub(super) async fn fetch(run: i64, lead: u32, bounds: Bounds, sites: Vec<Site>) -> Result<Frame> {
    ensure!(
        (1..=MAX_LEAD).contains(&lead),
        "RAP forecast lead must be 1–48 hours"
    );
    ensure!(
        !sites.is_empty() && sites.len() <= 4000,
        "RAP county-centre sample count is invalid"
    );
    ensure!(
        (0..=MAX_RUN_AGE).contains(&(chrono::Utc::now().timestamp() - run)),
        "Pinned NOAA RAP run expired"
    );
    let b = bounds.validate()?;
    let path = format!(
        "model-frame?source=rap&fields=grid&run={run}&lead={lead}&south={}&west={}&north={}&east={}",
        b.south as i32, b.west as i32, b.north as i32, b.east as i32
    );
    let mut response = crate::net::broadcaster_route(&path, Duration::from_secs(60)).await?;
    ensure!(
        !response
            .content_length()
            .is_some_and(|n| n > MAX_BYTES as u64),
        "RAP GRIB response exceeds 8 MiB"
    );
    let mut bytes = Vec::new();
    while let Some(c) = response.chunk().await? {
        ensure!(
            bytes.len().saturating_add(c.len()) <= MAX_BYTES,
            "RAP GRIB response exceeds 8 MiB"
        );
        bytes.extend_from_slice(&c);
    }
    tokio::task::spawn_blocking(move || decode(&bytes, run, lead, &sites)).await?
}

fn decode(bytes: &[u8], run: i64, lead: u32, sites: &[Site]) -> Result<Frame> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_BYTES,
        "Invalid RAP GRIB response length"
    );
    let msg = grib::from_reader(std::io::Cursor::new(bytes))?;
    let mut grid: Option<Lambert> = None;
    let mut expected: Option<GridDefinitionTemplateValues> = None;
    let mut values = BTreeMap::new();
    for (_, field) in msg.iter() {
        let def = field.prod_def();
        if field.indicator().discipline != 0 || def.prod_tmpl_num() != 0 {
            continue;
        }
        ensure!(
            def.forecast_time() == Some(grib::ForecastTime::from_numbers(1, lead)),
            "RAP GRIB lead differs from request"
        );
        let ref_time = field.identification().ref_time_unchecked();
        let date = NaiveDate::from_ymd_opt(
            ref_time.year as i32,
            ref_time.month as u32,
            ref_time.day as u32,
        )
        .context("Invalid RAP GRIB reference date")?;
        let timestamp = date
            .and_hms_opt(
                ref_time.hour as u32,
                ref_time.minute as u32,
                ref_time.second as u32,
            )
            .context("Invalid RAP GRIB reference time")?
            .and_utc()
            .timestamp();
        ensure!(
            timestamp == run,
            "RAP GRIB reference run differs from request"
        );
        let Some((level, second)) = def.fixed_surfaces() else {
            continue;
        };
        if second.surface_type != 255 {
            continue;
        }
        let (Some(cat), Some(num)) = (def.parameter_category(), def.parameter_number()) else {
            continue;
        };
        let value = level.value() as f64;
        let at = |want: f64| (value - want).abs() < 0.01;
        let kind = match (cat, num, level.surface_type) {
            (0, 0, 103) if at(2.) => Field::Temperature,
            (0, 6, 103) if at(2.) => Field::Dewpoint,
            (1, 1, 103) if at(2.) => Field::Humidity,
            (2, 2, 103) if at(10.) => Field::WindU,
            (2, 3, 103) if at(10.) => Field::WindV,
            (2, 22, 1) => Field::Gust,
            (6, 1, 10) => Field::Cloud,
            (3, 198, 101) if at(0.) => Field::Pressure,
            (7, 6, 1) if at(0.) => Field::Cape,
            _ => continue,
        };
        let definition = GridDefinitionTemplateValues::try_from(field.grid_def())?;
        let GridDefinitionTemplateValues::Template30(ref spec) = definition else {
            anyhow::bail!("RAP field is not on Lambert conformal projection")
        };
        if let Some(ref g) = expected {
            ensure!(
                g == &definition,
                "RAP surface fields use inconsistent grids"
            )
        } else {
            grid = Some(Lambert::from_template(spec)?);
            expected = Some(definition)
        }
        ensure!(
            !values.contains_key(&kind),
            "Duplicate RAP surface field {kind:?}"
        );
        let g = grid.as_ref().context("RAP projection missing")?;
        let count = g.ni * g.nj;
        let indices: Vec<_> = field.ij()?.collect();
        let decoder = grib::Grib2SubmessageDecoder::from(field)?;
        let mut cells = vec![f32::NAN; count];
        let mut seen = 0;
        for ((i, j), v) in indices.into_iter().zip(decoder.dispatch()?) {
            ensure!(i < g.ni && j < g.nj, "RAP GRIB index outside grid");
            let valid = match kind {
                Field::Temperature | Field::Dewpoint => (150.0..=350.0).contains(&v),
                Field::Humidity | Field::Cloud => (0.0..=100.0).contains(&v),
                Field::WindU | Field::WindV => (-150.0..=150.0).contains(&v),
                Field::Gust => (0.0..=150.0).contains(&v),
                Field::Pressure => (50_000.0..=120_000.0).contains(&v),
                Field::Cape => (0.0..=20_000.0).contains(&v),
            };
            cells[j * g.ni + i] = if valid { v } else { f32::NAN };
            seen += 1;
        }
        ensure!(seen == count, "Incomplete RAP GRIB field");
        let sampled: BTreeMap<String, f32> = sites
            .iter()
            .filter_map(|site| {
                sample_cells(g, &cells, site.lat, site.lon).map(|v| (site.id.clone(), v))
            })
            .collect();
        values.insert(kind, sampled);
    }
    for kind in [
        Field::Temperature,
        Field::Dewpoint,
        Field::Humidity,
        Field::WindU,
        Field::WindV,
        Field::Gust,
        Field::Cloud,
    ] {
        ensure!(
            values.contains_key(&kind),
            "RAP GRIB lacks surface field {kind:?}"
        )
    }
    // Rotate the bilinearly sampled grid-relative winds at each point before exposing them.
    let g = grid.as_ref().context("RAP projection missing")?;
    let u = values
        .get(&Field::WindU)
        .context("RAP U wind missing")?
        .clone();
    let v = values
        .get(&Field::WindV)
        .context("RAP V wind missing")?
        .clone();
    if g.uv_grid_relative {
        let (mut east, mut north) = (u.clone(), v.clone());
        for site in sites {
            if let (Some(u), Some(v)) = (u.get(&site.id), v.get(&site.id)) {
                let (e, n) = earth_wind(*u, *v, site.lon, g.lov, g.cone);
                east.insert(site.id.clone(), e);
                north.insert(site.id.clone(), n);
            }
        }
        values.insert(Field::WindU, east);
        values.insert(Field::WindV, north);
    }
    let valid = run
        .checked_add(i64::from(lead) * 3600)
        .context("RAP valid timestamp overflow")?;
    let _grid = grid.context("RAP response contains no selected fields")?;
    Ok(Frame {
        run,
        lead,
        valid,
        values,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use grib::def::grib2::ProductDefinitionTemplate;

    const FIXTURE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/rap-grid-indiana-20261115t12z-f01.grib2"
    ));
    const RUN: i64 = 1_794_744_000;

    #[test]
    fn rap_f01_fixture_contract_and_surface_samples() {
        let message = grib::from_reader(std::io::Cursor::new(FIXTURE)).unwrap();
        // A SubMessage owns the reader's RefMut; inspect and release each in turn.
        let mut field_count = 0;
        let mut shape = None;
        let mut surface_fields = BTreeMap::new();
        let mut apcp_seen = false;
        for (_, field) in message.iter() {
            field_count += 1;
            let def = field.prod_def();
            assert_eq!(field.indicator().discipline, 0);
            let reference = field.identification().ref_time_unchecked();
            let date = NaiveDate::from_ymd_opt(
                reference.year as i32,
                reference.month as u32,
                reference.day as u32,
            )
            .unwrap();
            let run = date
                .and_hms_opt(
                    reference.hour as u32,
                    reference.minute as u32,
                    reference.second as u32,
                )
                .unwrap()
                .and_utc()
                .timestamp();
            assert_eq!(run, RUN);
            if def.prod_tmpl_num() == 8 {
                assert_eq!(def.parameter_category(), Some(1));
                assert_eq!(def.parameter_number(), Some(8));
                assert_eq!(
                    def.forecast_time(),
                    Some(grib::ForecastTime::from_numbers(1, 0))
                );
                let section = field.section4().unwrap();
                let ProductDefinitionTemplate::_4_8(interval) = section.payload.template else {
                    panic!("APCP is not PDT 4.8")
                };
                assert_eq!(interval.time_interval.num_time_ranges, 1);
                let range = &interval.time_interval.time_ranges[0];
                assert_eq!(range.time_range.unit, grib::def::grib2::table::TimeRangeUnit(1));
                assert_eq!(range.time_range.len, 1, "APCP must cover 0–1 h");
                assert_eq!(range.time_increment.unit, grib::def::grib2::table::TimeRangeUnit(255));
                apcp_seen = true;
                continue;
            }
            assert_eq!(def.prod_tmpl_num(), 0);
            assert_eq!(
                def.forecast_time(),
                Some(grib::ForecastTime::from_numbers(1, 1))
            );
            let (Some(cat), Some(num), Some((level, second))) = (
                def.parameter_category(),
                def.parameter_number(),
                def.fixed_surfaces(),
            ) else {
                continue;
            };
            let (key, surface, value, units) = match (cat, num, level.surface_type) {
                (0, 0, 103) => ("TMP", 103, 2., "K"),
                (0, 6, 103) => ("DPT", 103, 2., "K"),
                (1, 1, 103) => ("RH", 103, 2., "%"),
                (2, 2, 103) => ("UGRD", 103, 10., "m/s"),
                (2, 3, 103) => ("VGRD", 103, 10., "m/s"),
                (2, 22, 1) => ("GUST", 1, 0., "m/s"),
                (3, 198, 101) => ("MSLMA", 101, 0., "Pa"),
                (6, 1, 10) => ("TCDC", 10, 0., "%"),
                (7, 6, 1) => ("CAPE", 1, 0., "J/kg"),
                _ => continue,
            };
            assert_eq!(level.surface_type, surface, "{key} level units {units}");
            if value != 0. {
                assert!(
                    (level.value() as f64 - value).abs() < 0.01,
                    "{key} level value"
                )
            }
            assert_eq!(
                second.surface_type, 255,
                "{key} must have one fixed surface"
            );
            let definition = GridDefinitionTemplateValues::try_from(field.grid_def()).unwrap();
            let GridDefinitionTemplateValues::Template30(spec) = definition else {
                panic!("RAP surface field is not Lambert")
            };
            let current = (spec.ni as usize, spec.nj as usize);
            assert_eq!(current, (71, 90));
            if key == "UGRD" || key == "VGRD" {
                assert_ne!(
                    spec.resolution_and_component_flags.0 & 0x08,
                    0,
                    "fixture winds are grid-relative"
                )
            }
            if let Some(previous) = shape {
                assert_eq!(previous, current, "RAP messages do not share grid geometry")
            } else {
                shape = Some(current)
            }
            assert!(
                surface_fields.insert(key, units).is_none(),
                "duplicate field {key}"
            );
        }
        assert_eq!(field_count, 15, "fixture message count changed");
        assert_eq!(shape, Some((71, 90)));
        assert_eq!(
            surface_fields.len(),
            9,
            "expected MSLMA, CAPE, TMP(2m), DPT, RH, U/V, gust and total cloud"
        );
        assert!(apcp_seen, "fixture APCP accumulation missing");

        let site = Site {
            id: "fixture-point".into(),
            lon: -86.5,
            lat: 38.8,
        };
        let frame = decode(FIXTURE, RUN, 1, std::slice::from_ref(&site)).unwrap();
        assert_eq!((frame.run, frame.lead, frame.valid), (RUN, 1, RUN + 3600));
        for field in [
            Field::Temperature,
            Field::Dewpoint,
            Field::Humidity,
            Field::WindU,
            Field::WindV,
            Field::Gust,
            Field::Cloud,
        ] {
            assert!(
                frame.sample(field, &site.id).is_some_and(f32::is_finite),
                "missing decoded {field:?}"
            );
        }
        assert!(
            (150.0..=350.0).contains(&frame.sample(Field::Temperature, &site.id).unwrap()),
            "TMP must remain kelvin"
        );
        assert!(
            (150.0..=350.0).contains(&frame.sample(Field::Dewpoint, &site.id).unwrap()),
            "DPT must remain kelvin"
        );
        assert!(
            (0.0..=100.0).contains(&frame.sample(Field::Humidity, &site.id).unwrap()),
            "RH must remain percent"
        );
        assert!(
            (0.0..=100.0).contains(&frame.sample(Field::Cloud, &site.id).unwrap()),
            "TCDC must remain percent"
        );
        assert!(
            frame.sample(Field::WindU, &site.id).unwrap().abs() <= 150.0,
            "UGRD must remain m/s"
        );
        assert!(
            frame.sample(Field::WindV, &site.id).unwrap().abs() <= 150.0,
            "VGRD must remain m/s"
        );
        assert!(
            (0.0..=150.0).contains(&frame.sample(Field::Gust, &site.id).unwrap()),
            "GUST must remain m/s"
        );
        assert!(frame.wind_speed_ms(&site.id).is_some_and(f32::is_finite));
        let pressure = frame
            .sample(Field::Pressure, &site.id)
            .expect("fixture MSLMA sample");
        assert!(
            (50_000.0..=120_000.0).contains(&pressure),
            "MSLMA must remain Pa"
        );
        let cape = frame
            .sample(Field::Cape, &site.id)
            .expect("fixture CAPE sample");
        assert!((0.0..=20_000.0).contains(&cape), "CAPE must remain J/kg");
        assert!(
            decode(FIXTURE, RUN + 3600, 1, std::slice::from_ref(&site)).is_err(),
            "wrong run must fail closed"
        );
        assert!(
            decode(FIXTURE, RUN, 2, std::slice::from_ref(&site)).is_err(),
            "wrong lead must fail closed"
        );
    }

    #[test]
    fn lambert_wind_rotation_preserves_speed_and_changes_axes() {
        let (u, v) = (10.0_f32, 0.0_f32);
        let (e, n) = earth_wind(u, v, 17.5, -95., 0.8);
        assert!(e.abs() < 1e-5);
        assert!((n + u).abs() < 1e-5);
        assert!((e.hypot(n) - u.hypot(v)).abs() < 1e-5);
    }
}
