use anyhow::{ensure, Context, Result};
use grib::{GridDefinitionTemplateValues, projection::{Ellipsoid, Lcc, LccParams, Project}};

use super::Source;

struct LambertGrid {
    projection: Lcc,
    origin: (f64, f64),
    step: (f64, f64),
    width: usize,
    height: usize,
}

pub(super) struct ProjectedFields {
    grid: LambertGrid,
    pub values: [Option<Vec<f32>>; 9],
}

impl std::fmt::Debug for ProjectedFields {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectedFields").field("width", &self.grid.width).field("height", &self.grid.height).finish()
    }
}

impl ProjectedFields {
    pub fn sample(&self, slot: usize, lat: f64, lon: f64) -> Option<f32> {
        let values = self.values[slot].as_ref()?;
        let (x, y) = self.grid.projection.project(&(lon.to_radians(), lat.to_radians()), false).ok()?;
        let x = (x - self.grid.origin.0) / self.grid.step.0;
        let y = (y - self.grid.origin.1) / self.grid.step.1;
        let (width, height) = (self.grid.width, self.grid.height);
        if !(0.0..=(width - 1) as f64).contains(&x) || !(0.0..=(height - 1) as f64).contains(&y) {
            return None;
        }
        let (i, j) = (x as usize, y as usize);
        let (next_i, next_j) = ((i + 1).min(width - 1), (j + 1).min(height - 1));
        let (dx, dy) = ((x - i as f64) as f32, (y - j as f64) as f32);
        let mut total = 0.0;
        let mut weight_sum = 0.0;
        for (i, j, weight) in [(i, j, (1.0-dx)*(1.0-dy)), (next_i, j, dx*(1.0-dy)),
            (i, next_j, (1.0-dx)*dy), (next_i, next_j, dx*dy)] {
            let value = values[j * width + i];
            if value.is_finite() {
                total += value * weight;
                weight_sum += weight;
            }
        }
        (weight_sum > 0.5).then(|| total / weight_sum)
    }
}

pub(super) fn decode(bytes: &[u8], source: Source, lead: i64) -> Result<ProjectedFields> {
    let grib = grib::from_reader(std::io::Cursor::new(bytes))?;
    let mut grid = None;
    let mut expected_definition = None;
    let mut values = std::array::from_fn(|_| None);
    for (_, field) in grib.iter() {
        if field.indicator().discipline != 0 { continue; }
        let def = field.prod_def();
        let slot = match (source, def.parameter_category(), def.parameter_number(), def.prod_tmpl_num()) {
            (Source::Hrrr, Some(16), Some(196), 0) => 0,
            (Source::Nbm, Some(1), Some(number @ (29 | 228)), 8)
                if def.forecast_time() == Some(grib::ForecastTime::from_numbers(1, (lead - 6) as u32)) =>
            {
                if number == 29 { 1 } else { 2 }
            }
            (Source::Nbm, Some(1), Some(number @ (29 | 228)), 10)
                if def.forecast_time() == Some(grib::ForecastTime::from_numbers(1, (lead - 6) as u32)) =>
            {
                // GRIB2 template 4.10 octet 35, relative to the section 4 header.
                match (number, def.iter().nth(29).copied()) {
                    (29, Some(10)) => 3,
                    (29, Some(90)) => 4,
                    (228, Some(10)) => 5,
                    (228, Some(90)) => 6,
                    _ => continue,
                }
            }
            (Source::Nbm, Some(1), Some(number @ (29 | 228)), 9)
                if def.forecast_time() == Some(grib::ForecastTime::from_numbers(1, (lead - 6) as u32)) =>
            {
                // Template 4.9 octets 37 and 43–47. Type 1 is probability above the upper limit.
                if def.iter().nth(31) != Some(&1) { continue; }
                let mut limit = def.iter().copied().skip(37);
                let (Some(scale @ 0..=8), Some(a), Some(b), Some(c), Some(d)) =
                    (limit.next(), limit.next(), limit.next(), limit.next(), limit.next()) else { continue };
                let upper = f64::from(i32::from_be_bytes([a, b, c, d])) * 10f64.powi(-i32::from(scale));
                match number {
                    29 if (upper - 0.0254).abs() < 1e-6 => 7,
                    228 if (upper - 6.35).abs() < 1e-6 => 8,
                    _ => continue,
                }
            }
            _ => continue,
        };
        let definition = GridDefinitionTemplateValues::try_from(field.grid_def())?;
        let GridDefinitionTemplateValues::Template30(ref spec) = definition else {
            anyhow::bail!("NOAA regional model uses an unsupported projection");
        };
        let (width, height) = (spec.ni as usize, spec.nj as usize);
        let count = width.checked_mul(height).context("Invalid regional grid dimensions")?;
        ensure!(width > 1 && height > 1 && count <= 4_000_000, "Regional grid exceeds size limit");
        if let Some(expected) = &expected_definition {
            ensure!(expected == &definition, "Regional model fields use different grids");
        } else {
            let (a, b) = spec.earth_shape.radii().context("Unknown GRIB Earth shape")?;
            let degrees = |value: f64| value * 1e-6;
            let projection = Lcc::new(&LccParams {
                ellipsoid: Ellipsoid::from_a_and_b(a, b),
                lat_0: degrees(spec.lad as f64),
                lon_0: degrees(spec.lov as f64),
                lat_1: degrees(spec.latin1 as f64),
                lat_2: degrees(spec.latin2 as f64),
            }).map_err(anyhow::Error::msg)?;
            let origin = projection.project(&(degrees(spec.first_point_lon as f64).to_radians(), degrees(spec.first_point_lat as f64).to_radians()), false)
                .map_err(anyhow::Error::msg)?;
            let step = (
                spec.dx as f64 * if spec.scanning_mode.scans_positively_for_i() { 0.001 } else { -0.001 },
                spec.dy as f64 * if spec.scanning_mode.scans_positively_for_j() { 0.001 } else { -0.001 },
            );
            ensure!(step.0 != 0.0 && step.1 != 0.0, "Invalid regional grid spacing");
            grid = Some(LambertGrid { projection, origin, step, width, height });
            expected_definition = Some(definition);
        }
        let indices = field.ij()?;
        let decoder = grib::Grib2SubmessageDecoder::from(field)?;
        let mut cells = vec![f32::NAN; count];
        let mut decoded = 0;
        for ((i, j), value) in indices.zip(decoder.dispatch()?) {
            cells[j * width + i] = match slot {
                1 | 3 | 4 => value / 0.0254, // ASNOW is physical snowfall depth in metres.
                2 | 5 | 6 => value / 25.4,   // NOAA FRAM thresholds use 25.4 kg/m² per inch.
                _ => value,
            };
            decoded += 1;
        }
        ensure!(decoded == count, "Incomplete NOAA regional grid");
        values[slot] = Some(cells);
    }
    let grid = grid.context("Requested NOAA regional fields are unavailable")?;
    ensure!(match source {
        Source::Hrrr => values[0].is_some(),
        Source::Nbm => values[1].is_some() && values[2].is_some(),
        Source::Gfs => false,
    }, "NOAA regional response is missing a requested field");
    Ok(ProjectedFields { grid, values })
}
