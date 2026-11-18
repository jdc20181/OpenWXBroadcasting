//! NOAA GFS precipitation frames fetched through DCWeatherSite.
use anyhow::{ensure, Context, Result};
use futures_util::{stream, StreamExt};
use grib::LatLons;

use crate::{config::GeoView, grid::PrecipKind, net};

pub const MAX_LEAD: u32 = 384;
pub const SPANS: [u32; 9] = [3, 12, 24, 48, 72, 120, 168, 240, MAX_LEAD];
const RESOLUTION: f64 = 0.25;

#[path = "futurecast_projected.rs"]
mod projected;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source { Gfs, Hrrr, Nbm }

impl Source {
    pub const ALL: [Self; 3] = [Self::Gfs, Self::Hrrr, Self::Nbm];

    pub fn index(self) -> usize { self as usize }

    pub fn label(self) -> &'static str {
        match self { Self::Gfs => "GFS", Self::Hrrr => "HRRR", Self::Nbm => "NBM" }
    }

    fn max_lead(self) -> i64 {
        match self { Self::Gfs => i64::from(MAX_LEAD), Self::Hrrr => 48, Self::Nbm => 72 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RunPin {
    pub run: i64,
    pub start: i64,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum Product {
    #[default]
    Precipitation,
    Temperature,
    GfsReflectivity,
    HrrrReflectivity,
    Snowfall,
    Ice,
    SnowfallLow,
    SnowfallHigh,
    IceLow,
    IceHigh,
    SnowfallChance,
    IceChance,
    PrecipitationRate,
    /// GFS total precipitation (liquid equivalent) from now to the frame, from APCP.
    TotalPrecip,
    /// GFS 2 m dew point.
    DewPoint,
    /// GFS 2 m relative humidity.
    Humidity,
    /// GFS 10 m wind speed.
    Wind,
    /// GFS surface wind gust.
    Gust,
    /// GFS mean sea-level pressure.
    Pressure,
}

impl Product {
    /// Top-level choices in the forecast picker. Winter variants live under Display.
    pub const PICKER_GROUPS: [(&'static str, &'static [Self]); 4] = [
        ("Precipitation", &[Self::Precipitation, Self::PrecipitationRate, Self::TotalPrecip]),
        ("Surface", &[Self::Temperature, Self::DewPoint, Self::Humidity, Self::Wind, Self::Gust, Self::Pressure]),
        ("Simulated radar", &[Self::GfsReflectivity, Self::HrrrReflectivity]),
        ("Winter", &[Self::Snowfall, Self::Ice]),
    ];

    pub fn source(self) -> Source {
        match self {
            Self::HrrrReflectivity => Source::Hrrr,
            Self::Snowfall | Self::Ice | Self::SnowfallLow | Self::SnowfallHigh | Self::IceLow | Self::IceHigh | Self::SnowfallChance | Self::IceChance => Source::Nbm,
            _ => Source::Gfs,
        }
    }

    pub fn broadcast_title(self) -> &'static str {
        match self {
            Self::Precipitation => "Rain, Mix and Snow Forecast",
            Self::PrecipitationRate => "Precipitation Rate Forecast",
            Self::TotalPrecip => "Total Precipitation Forecast",
            Self::DewPoint => "Dew Point Forecast",
            Self::Humidity => "Humidity Forecast",
            Self::Wind => "Wind Forecast",
            Self::Gust => "Wind Gust Forecast",
            Self::Pressure => "Pressure Forecast",
            Self::Temperature => "Temperature Outlook",
            Self::GfsReflectivity | Self::HrrrReflectivity => "Future Radar",
            Self::Snowfall => "Snowfall Forecast",
            Self::SnowfallLow => "Low-End Snowfall Forecast",
            Self::SnowfallHigh => "High-End Snowfall Forecast",
            Self::Ice => "Ice Accumulation Forecast",
            Self::IceLow => "Low-End Ice Forecast",
            Self::IceHigh => "High-End Ice Forecast",
            Self::SnowfallChance => "Chance of 1 Inch of Snow",
            Self::IceChance => "Chance of ¼ Inch of Ice",
        }
    }

    pub fn winter_choices(self) -> Option<[Self; 4]> {
        match self {
            Self::Snowfall | Self::SnowfallLow | Self::SnowfallHigh | Self::SnowfallChance => Some([Self::Snowfall, Self::SnowfallLow, Self::SnowfallHigh, Self::SnowfallChance]),
            Self::Ice | Self::IceLow | Self::IceHigh | Self::IceChance => Some([Self::Ice, Self::IceLow, Self::IceHigh, Self::IceChance]),
            _ => None,
        }
    }

    pub fn projected_slot(self) -> Option<usize> {
        match self {
            Self::HrrrReflectivity => Some(0),
            Self::Snowfall => Some(1),
            Self::Ice => Some(2),
            Self::SnowfallLow => Some(3),
            Self::SnowfallHigh => Some(4),
            Self::IceLow => Some(5),
            Self::IceHigh => Some(6),
            Self::SnowfallChance => Some(7),
            Self::IceChance => Some(8),
            _ => None,
        }
    }

    pub fn spans(self) -> &'static [u32] {
        match self.source() {
            Source::Gfs => &SPANS,
            Source::Hrrr => &[3, 6, 12, 24, 48],
            Source::Nbm => &[12, 24, 48, 72],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Precipitation => "Rain, mix and snow",
            Self::PrecipitationRate => "Precipitation rate",
            Self::TotalPrecip => "Total precipitation",
            Self::DewPoint => "Dew point",
            Self::Humidity => "Humidity",
            Self::Wind => "Wind (10 m)",
            Self::Gust => "Wind gusts",
            Self::Pressure => "Sea-level pressure",
            Self::Temperature => "Temperature",
            Self::GfsReflectivity | Self::HrrrReflectivity => "Simulated radar",
            Self::Snowfall => "6h snowfall",
            Self::Ice => "6h flat ice",
            Self::SnowfallLow => "6h snowfall P10",
            Self::SnowfallHigh => "6h snowfall P90",
            Self::IceLow => "6h flat ice P10",
            Self::IceHigh => "6h flat ice P90",
            Self::SnowfallChance => "6h chance of 1 inch of snow",
            Self::IceChance => "6h chance of ¼ inch of ice",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    south: f64,
    west: f64,
    north: f64,
    east: f64,
}

impl Region {
    pub fn for_view(view: GeoView) -> Result<Self> {
        let (north, south, west, east) = crate::grid::view_bounds(view, 0.08);
        let region = Self {
            south: (south / 5.0).floor() * 5.0,
            west: (west / 5.0).floor() * 5.0,
            north: (north / 5.0).ceil() * 5.0,
            east: (east / 5.0).ceil() * 5.0,
        };
        ensure!(
            region.east - region.west <= 100.0
                && region.north - region.south <= 65.0
                && region.west >= -180.0
                && region.east < 180.0
                && region.south >= -85.0
                && region.north <= 85.0,
            "Zoom in to a region that does not cross the date line to load GFS"
        );
        Ok(region)
    }

    pub fn validate(self, source: Source) -> Result<()> {
        ensure!(source == Source::Gfs || (self.south >= 15.0 && self.north <= 60.0
            && self.west >= -135.0 && self.east <= -55.0
            && self.north - self.south <= 20.0 && self.east - self.west <= 30.0),
            "Zoom to a region within the continental U.S. model domain");
        Ok(())
    }

    pub fn covers(self, view: GeoView) -> bool {
        let (north, south, west, east) = crate::grid::view_bounds(view, 0.0);
        south >= self.south && north <= self.north && west >= self.west && east <= self.east
    }
}

#[derive(Debug)]
pub struct ModelFrame {
    pub time: i64,
    north: f64,
    west: f64,
    width: usize,
    height: usize,
    rate: Vec<f32>,
    instantaneous_rate: bool,
    kind: Vec<Option<PrecipKind>>,
    temperature: Option<Vec<f32>>,
    reflectivity: Option<Vec<f32>>,
    /// Run-total APCP (inches); after fetch, the total since the loop's first frame.
    apcp: Option<Vec<f32>>,
    /// Dew point (K), RH (%), 10 m U and V (m/s), gust (m/s), sea-level pressure (Pa).
    surface: [Option<Vec<f32>>; 6],
    projected: Option<projected::ProjectedFields>,
}

impl ModelFrame {
    pub fn has_product(&self, product: Product) -> bool {
        match product {
            Product::Precipitation => !self.rate.is_empty() && !self.kind.is_empty(),
            Product::PrecipitationRate => self.instantaneous_rate && !self.rate.is_empty(),
            Product::Temperature => self.temperature.is_some(),
            Product::GfsReflectivity => self.reflectivity.is_some(),
            Product::TotalPrecip => self.apcp.is_some(),
            Product::DewPoint => self.surface[0].is_some(),
            Product::Humidity => self.surface[1].is_some(),
            Product::Wind => self.surface[2].is_some() && self.surface[3].is_some(),
            Product::Gust => self.surface[4].is_some(),
            Product::Pressure => self.surface[5].is_some(),
            product => product.projected_slot().is_some_and(|slot| {
                self.projected.as_ref().is_some_and(|fields| fields.values[slot].is_some())
            }),
        }
    }

    pub fn temperature(&self, lat: f64, lon: f64) -> Option<f32> {
        let kelvin = self.scalar(self.temperature.as_ref()?, lat, lon)?;
        Some((kelvin - 273.15) * 1.8 + 32.0)
    }

    pub fn value(&self, product: Product, lat: f64, lon: f64) -> Option<f32> {
        match product {
            Product::PrecipitationRate if self.has_product(product) => {
                self.scalar(&self.rate, lat, lon).map(|rate| rate.max(0.0))
            }
            Product::Temperature => self.temperature(lat, lon),
            Product::GfsReflectivity => self.scalar(self.reflectivity.as_ref()?, lat, lon),
            Product::TotalPrecip => self.scalar(self.apcp.as_ref()?, lat, lon).map(|v| v.max(0.0)),
            Product::DewPoint => self.scalar(self.surface[0].as_ref()?, lat, lon).map(|k| (k - 273.15) * 1.8 + 32.0),
            Product::Humidity => self.scalar(self.surface[1].as_ref()?, lat, lon).map(|v| v.clamp(0.0, 100.0)),
            Product::Wind => {
                let u = self.scalar(self.surface[2].as_ref()?, lat, lon)?;
                let v = self.scalar(self.surface[3].as_ref()?, lat, lon)?;
                Some(u.hypot(v) * 2.236_94)
            }
            Product::Gust => self.scalar(self.surface[4].as_ref()?, lat, lon).map(|ms| ms * 2.236_94),
            Product::Pressure => self.scalar(self.surface[5].as_ref()?, lat, lon).map(|pa| pa / 100.0),
            product => self.projected.as_ref()?.sample(product.projected_slot()?, lat, lon),
        }
    }

    fn scalar(&self, grid: &[f32], lat: f64, lon: f64) -> Option<f32> {
        let (x, y) = self.grid_position(lat, lon)?;
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.width - 1), (y0 + 1).min(self.height - 1));
        let (dx, dy) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32);
        let north = grid[y0 * self.width + x0] * (1.0 - dx) + grid[y0 * self.width + x1] * dx;
        let south = grid[y1 * self.width + x0] * (1.0 - dx) + grid[y1 * self.width + x1] * dx;
        let value = north * (1.0 - dy) + south * dy;
        value.is_finite().then_some(value)
    }

    fn grid_position(&self, lat: f64, lon: f64) -> Option<(f64, f64)> {
        let x = (lon - self.west) / RESOLUTION;
        let y = (self.north - lat) / RESOLUTION;
        if x < 0.0 || y < 0.0 || x > (self.width - 1) as f64 || y > (self.height - 1) as f64 {
            return None;
        }
        Some((x, y))
    }

    pub fn sample(&self, lat: f64, lon: f64) -> Option<(f32, PrecipKind)> {
        if !self.has_product(Product::Precipitation) {
            return None;
        }
        let (x, y) = self.grid_position(lat, lon)?;
        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let dx = (x - x0 as f64) as f32;
        let dy = (y - y0 as f64) as f32;
        let kind = self.kind[y.round() as usize * self.width + x.round() as usize]?;
        // Smooth the intensity within each category while keeping rain/snow edges distinct.
        let mut rate = 0.0;
        for (i, j, weight) in [
            (x0, y0, (1.0 - dx) * (1.0 - dy)),
            (x1, y0, dx * (1.0 - dy)),
            (x0, y1, (1.0 - dx) * dy),
            (x1, y1, dx * dy),
        ] {
            let index = j * self.width + i;
            if self.kind[index] == Some(kind) && self.rate[index].is_finite() {
                rate += self.rate[index] * weight;
            }
        }
        Some((rate, kind))
    }
}

#[derive(Debug)]
pub struct ModelLoop {
    pub region: Region,
    pub hours: u32,
    pub run: i64,
    pub fetched_at: i64,
    pub skipped: usize,
    frames: Vec<ModelFrame>,
}

impl ModelLoop {
    pub fn pin(&self) -> RunPin { RunPin { run: self.run, start: self.start_time() } }

    pub fn start_time(&self) -> i64 {
        self.frames[0].time
    }

    pub fn timeline_hours(&self, span: u32, product: Product) -> impl Iterator<Item = u32> + '_ {
        self.frames.iter().filter(move |frame| frame.has_product(product))
            .map(|frame| ((frame.time - self.start_time()) / 3600) as u32)
            .take_while(move |&hour| hour <= span)
    }

    pub fn exact_frame(&self, valid: i64) -> Option<&ModelFrame> {
        self.frames.binary_search_by_key(&valid, |frame| frame.time).ok().map(|index| &self.frames[index])
    }

    pub fn frame(&self, hour: f32, span: u32, product: Product) -> Option<&ModelFrame> {
        let valid = self.start_time() + (hour.clamp(0.0, span as f32) * 3600.0).round() as i64;
        let at = self.frames.partition_point(|frame| frame.time <= valid);
        let end = self.start_time() + i64::from(span) * 3600;
        self.frames[..at].iter().rev().find(|frame| frame.has_product(product))
            .or_else(|| self.frames[at..].iter().take_while(|frame| frame.time <= end)
                .find(|frame| frame.has_product(product)))
    }
}

fn decode(bytes: &[u8], time: i64) -> Result<ModelFrame> {
    let message = grib::from_reader(std::io::Cursor::new(bytes))?;
    let mut fields: [Option<Vec<f32>>; 14] = Default::default();
    let mut grid = None;
    let mut instantaneous_rate = false;
    for (_, field) in message.iter() {
        if field.indicator().discipline != 0 {
            continue;
        }
        let def = field.prod_def();
        let instantaneous = def.prod_tmpl_num() == 0;
        // Surface type and value of the first fixed surface (103 = height above ground,
        // 1 = ground, 101 = mean sea level).
        let level_is = |kind: u8, value: f64| def.fixed_surfaces().is_some_and(|(level, _)| {
            level.surface_type == kind && (kind == 1 || kind == 101 || (level.value() as f64 - value).abs() < 0.01)
        });
        let slot = match (def.parameter_category(), def.parameter_number()) {
            (Some(1), Some(7)) if instantaneous || fields[0].is_none() => 0,
            (Some(16), Some(196)) => 6,
            // APCP. NOAA ships two records per lead: the 6-hour bucket and the run total from
            // hour 0 (identical at f003/f006). The run total contains the bucket, so the
            // point-wise maximum is the run total in every case.
            (Some(1), Some(8)) => 7,
            (Some(0), Some(6)) if level_is(103, 2.0) => 8,
            (Some(1), Some(1)) if level_is(103, 2.0) => 9,
            (Some(2), Some(2)) if level_is(103, 10.0) => 10,
            (Some(2), Some(3)) if level_is(103, 10.0) => 11,
            (Some(2), Some(22)) if level_is(1, 0.0) => 12,
            (Some(3), Some(1)) if level_is(101, 0.0) => 13,
            (Some(1), Some(33 | 192)) => 1,
            (Some(1), Some(34 | 193)) => 2,
            (Some(1), Some(35 | 194)) => 3,
            (Some(1), Some(36 | 195)) => 4,
            (Some(0), Some(0)) if def.fixed_surfaces().is_some_and(|(level, _)| {
                level.surface_type == 103 && level.value() == 2.0
            }) => 5,
            _ => continue,
        };
        let coordinates: Vec<_> = field
            .latlons()?
            .map(|(lat, lon)| (lat as f64, (lon as f64 + 180.0).rem_euclid(360.0) - 180.0))
            .collect();
        ensure!(
            !coordinates.is_empty() && coordinates.len() <= 120_000,
            "Invalid GFS region size"
        );
        let north = coordinates
            .iter()
            .map(|p| p.0)
            .fold(f64::NEG_INFINITY, f64::max);
        let south = coordinates
            .iter()
            .map(|p| p.0)
            .fold(f64::INFINITY, f64::min);
        let east = coordinates
            .iter()
            .map(|p| p.1)
            .fold(f64::NEG_INFINITY, f64::max);
        let west = coordinates
            .iter()
            .map(|p| p.1)
            .fold(f64::INFINITY, f64::min);
        let width = ((east - west) / RESOLUTION).round() as usize + 1;
        let height = ((north - south) / RESOLUTION).round() as usize + 1;
        ensure!(
            width * height == coordinates.len(),
            "GFS region is not a rectangular 0.25° grid"
        );
        let geometry = (north, west, width, height);
        ensure!(
            grid.is_none() || grid == Some(geometry),
            "GFS fields have different grids"
        );
        grid = Some(geometry);
        let decoder = grib::Grib2SubmessageDecoder::from(field)?;
        let mut values = vec![f32::NAN; width * height];
        for ((lat, lon), value) in coordinates.into_iter().zip(decoder.dispatch()?) {
            let row = ((north - lat) / RESOLUTION).round() as usize;
            let col = ((lon - west) / RESOLUTION).round() as usize;
            values[row * width + col] = value;
        }
        if slot == 7 {
            if let Some(previous) = fields[7].take() {
                values = values.into_iter().zip(previous).map(|(a, b)| if a.is_nan() { b } else if b.is_nan() { a } else { a.max(b) }).collect();
            }
        }
        fields[slot] = Some(values);
        if slot == 0 {
            instantaneous_rate = instantaneous;
        }
    }
    let (north, west, width, height) = grid.context("GFS response has no precipitation fields")?;
    let [rate, rain, freezing, pellets, snow, temperature, reflectivity, apcp, dew, rh, u10, v10, gust, mslp] = fields;
    let surface = [dew, rh, u10, v10, gust, mslp];
    // kg/m² is mm of liquid water.
    let apcp = apcp.map(|v| v.into_iter().map(|mm| mm / 25.4).collect::<Vec<f32>>());
    let rate = rate.unwrap_or_default();
    let kind = if let (Some(rain), Some(freezing), Some(pellets), Some(snow)) = (rain, freezing, pellets, snow) {
        (0..rate.len())
        .map(|i| {
            if freezing[i] > 0.5 || pellets[i] > 0.5 || (rain[i] > 0.5 && snow[i] > 0.5) {
                Some(PrecipKind::Mix)
            } else if snow[i] > 0.5 {
                Some(PrecipKind::Snow)
            } else if rain[i] > 0.5 {
                Some(PrecipKind::Rain)
            } else {
                None
            }
        })
        .collect()
    } else {
        Vec::new()
    };
    // kg/m²/s equals mm/s of liquid water. Convert to inches of liquid equivalent per hour.
    let rate = rate.into_iter().map(|r| r * 3600.0 / 25.4).collect();
    Ok(ModelFrame {
        time,
        north,
        west,
        width,
        height,
        rate,
        instantaneous_rate,
        kind,
        temperature,
        reflectivity,
        apcp,
        surface,
        projected: None,
    })
}

fn forecast_leads(first: i64, hours: u32) -> Vec<i64> {
    let step = match hours {
        0..=12 => 1,
        13..=48 => 3,
        49..=120 => 6,
        121..=240 => 12,
        _ => 24,
    };
    let last = (first + hours as i64).min(MAX_LEAD as i64);
    let last = if last > 120 { last / 3 * 3 } else { last };
    // NOAA publishes every third hour after hour 120. Keep the last published frame.
    let mut leads: Vec<_> = (first..last).step_by(step).chain(std::iter::once(last))
        .map(|lead| if lead > 120 { (lead + 2) / 3 * 3 } else { lead })
        .collect();
    leads.dedup();
    leads
}

pub async fn fetch(base: &str, region: Region, hours: u32, source: Source, pin: Option<RunPin>) -> Result<ModelLoop> {
    region.validate(source)?;
    let base = format!("{}/api/weatherbroadcaster", base.trim_end_matches('/'));
    let now = chrono::Utc::now().timestamp();
    let (run, start) = if let Some(pin) = pin {
        ensure!((0..=72 * 3600).contains(&(now - pin.run)), "Pinned NOAA run has expired");
        (pin.run, pin.start)
    } else {
        let run = net::get_json(&format!("{base}/model-run")).await?["run"]
            .as_i64().context("Missing NOAA run")?;
        ensure!((6 * 3600..=18 * 3600).contains(&(now - run)), "NOAA run is out of date");
        (run, now)
    };
    let first = ((start - run).div_euclid(3600)).max(1);
    let leads = match source {
        Source::Gfs => forecast_leads(first, hours),
        Source::Hrrr => (first..=(first + i64::from(hours)).min(source.max_lead()))
            .step_by(if hours > 24 { 2 } else { 1 })
            .chain(std::iter::once((first + i64::from(hours)).min(source.max_lead()))).collect(),
        Source::Nbm => (((first + 5) / 6 * 6)..=(first + i64::from(hours)).min(source.max_lead()))
            .step_by(6).collect(),
    };
    let mut leads = leads;
    leads.dedup();
    let outcomes = stream::iter(leads)
        .map(|lead| {
            let url = format!(
                "{base}/model-frame?source={}&fields=standard&run={run}&lead={lead}&south={}&west={}&north={}&east={}",
                source.label().to_ascii_lowercase(), region.south, region.west, region.north, region.east
            );
            async move {
                let response = net::get_logged(&url, std::time::Duration::from_secs(35)).await?;
                let bytes = response.bytes().await?;
                ensure!(bytes.len() <= 8 * 1024 * 1024, "GFS response too large");
                tokio::task::spawn_blocking(move || {
                    let time = run + lead * 3600;
                    if source == Source::Gfs { return decode(&bytes, time); }
                    Ok(ModelFrame {
                        time, north: 0.0, west: 0.0, width: 0, height: 0,
                        rate: Vec::new(), instantaneous_rate: false, kind: Vec::new(), temperature: None, reflectivity: None, apcp: None, surface: Default::default(),
                        projected: Some(projected::decode(&bytes, source, lead)?),
                    })
                }).await?
            }
        })
        .buffer_unordered(2)
        .collect::<Vec<Result<ModelFrame>>>()
        .await;
    let mut first_error = None;
    let mut skipped = 0;
    let mut frames = Vec::new();
    for outcome in outcomes {
        match outcome {
            Ok(frame) => frames.push(frame),
            Err(error) => {
                skipped += 1;
                if first_error.is_none() { first_error = Some(error); }
            }
        }
    }
    if frames.len() < 2 {
        return Err(first_error.unwrap_or_else(|| anyhow::anyhow!("Fewer than two GFS frames returned")));
    }
    frames.sort_by_key(|f| f.time);
    // Total precipitation is measured from the first frame (now), never by adding buckets.
    if let Some(base) = frames[0].apcp.clone() {
        for frame in &mut frames {
            if let Some(total) = frame.apcp.as_mut().filter(|t| t.len() == base.len()) {
                for (value, start) in total.iter_mut().zip(&base) {
                    *value = (*value - start).max(0.0);
                }
            } else {
                frame.apcp = None;
            }
        }
    }
    ensure!(pin.is_none_or(|pin| pin.start == frames[0].time), "Pinned NOAA start frame is unavailable");
    Ok(ModelLoop {
        region,
        hours,
        run,
        fetched_at: now,
        skipped,
        frames,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperature_interpolates_and_converts_kelvin_to_fahrenheit() {
        let frame = ModelFrame {
            time: 0,
            north: 40.0,
            west: -90.0,
            width: 2,
            height: 2,
            rate: vec![0.0; 4],
            instantaneous_rate: true,
            kind: vec![None; 4],
            temperature: Some(vec![273.15, 283.15, 283.15, 293.15]),
            reflectivity: None,
            apcp: None,
            surface: Default::default(),
            projected: None,
        };
        let sample = frame.temperature(39.875, -89.875).unwrap();
        assert!((sample - 50.0).abs() < 0.001);
        assert!(frame.temperature(39.0, -90.0).is_none());
    }

    #[test]
    fn lead_schedule_reaches_the_end_of_noaa_run() {
        assert_eq!(forecast_leads(9, 3), [9, 10, 11, 12]);
        let full = forecast_leads(9, MAX_LEAD);
        assert_eq!(full.first(), Some(&9));
        assert_eq!(full.last(), Some(&384));
        assert!(full.windows(2).all(|w| w[0] < w[1]));
        assert!(full.iter().all(|&lead| lead <= 120 || lead % 3 == 0));
    }

    #[test]
    #[ignore = "requires real regional NOAA GRIB2 fixtures in CWX_HRRR_FIXTURE and CWX_NBM_FIXTURE"]
    fn decodes_projected_noaa_products() {
        for (variable, source, lead, slots) in [
            ("CWX_HRRR_FIXTURE", Source::Hrrr, 18, &[0][..]),
            ("CWX_NBM_FIXTURE", Source::Nbm, 24, &[1, 2, 3, 4, 5, 6, 7, 8][..]),
        ] {
            let bytes = std::fs::read(std::env::var_os(variable).unwrap()).unwrap();
            let fields = projected::decode(&bytes, source, lead).unwrap();
            for &slot in slots {
                let value = fields.sample(slot, 39.0, -87.0).expect("finite field at fixture location");
                if slot >= 7 { assert!((0.0..=100.0).contains(&value)); }
                assert!(fields.sample(slot, 0.0, 0.0).is_none());
            }
        }
    }

    #[test]
    #[ignore = "requires a NOMADS full-horizon subset in CWX_GFS_FULL_FIXTURE"]
    fn decodes_gfs_full_run_temperature_and_reflectivity() {
        let bytes = std::fs::read(std::env::var_os("CWX_GFS_FULL_FIXTURE").unwrap()).unwrap();
        let frame = decode(&bytes, 384 * 3600).unwrap();
        assert!(frame.temperature(39.0, -87.0).is_some());
        assert!(frame.value(Product::GfsReflectivity, 39.0, -87.0).is_some());
    }

    #[test]
    #[ignore = "requires a NOMADS GFS GRIB2 subset in CWX_GFS_FIXTURE"]
    fn decodes_noaa_precipitation_subset() {
        let bytes = std::fs::read(std::env::var_os("CWX_GFS_FIXTURE").unwrap()).unwrap();
        let frame = decode(&bytes, 3600).unwrap();
        assert_eq!(frame.time, 3600);
        assert!(frame.width > 1 && frame.height > 1);
        assert!(frame
            .rate
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0));
        assert!(frame.kind.iter().any(Option::is_some));
        let i = frame.kind.iter().position(Option::is_some).unwrap();
        let lat = frame.north - (i / frame.width) as f64 * RESOLUTION;
        let lon = frame.west + (i % frame.width) as f64 * RESOLUTION;
        assert_eq!(
            frame.sample(lat, lon),
            Some((frame.rate[i], frame.kind[i].unwrap()))
        );
    }
}
