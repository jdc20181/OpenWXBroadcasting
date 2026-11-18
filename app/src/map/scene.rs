//! A map "scene": everything needed to reproduce a map shot (camera, layers, telestration,
//! storm track, on-air elements). Shows store these, and the live Map Studio edits one.

use serde::{Deserialize, Serialize};

use crate::config::{Basemap, GeoView, Settings, FRAME_H, FRAME_W};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum RadarMode {
    /// Animate through the available frames.
    Loop,
    /// Hold on the newest past frame.
    Latest,
}

/// Where radar comes from: the LibreWXR national composite (loops, palettes) through the
/// site proxy, or one NEXRAD site's Level III products (velocity etc.).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum RadarSource {
    #[default]
    Composite,
    Site,
}

/// Single-site NEXRAD Level III products available as map tiles.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum SiteProduct {
    #[default]
    SuperRes,
    Reflectivity,
    Velocity,
    StormRelative,
    EchoTops,
}

impl SiteProduct {
    pub const ALL: [SiteProduct; 5] = [
        SiteProduct::SuperRes,
        SiteProduct::Reflectivity,
        SiteProduct::Velocity,
        SiteProduct::StormRelative,
        SiteProduct::EchoTops,
    ];

    pub fn code(self) -> &'static str {
        match self {
            SiteProduct::SuperRes => "N0B",
            SiteProduct::Reflectivity => "N0Q",
            SiteProduct::Velocity => "N0U",
            SiteProduct::StormRelative => "N0S",
            SiteProduct::EchoTops => "NET",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SiteProduct::SuperRes => "Super-Res Reflectivity",
            SiteProduct::Reflectivity => "Base Reflectivity",
            SiteProduct::Velocity => "Base Velocity",
            SiteProduct::StormRelative => "Storm-Relative Velocity",
            SiteProduct::EchoTops => "Echo Tops",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            SiteProduct::SuperRes => "REF HD",
            SiteProduct::Reflectivity => "REF",
            SiteProduct::Velocity => "VEL",
            SiteProduct::StormRelative => "SRV",
            SiteProduct::EchoTops => "TOPS",
        }
    }

    pub fn is_velocity(self) -> bool {
        matches!(self, SiteProduct::Velocity | SiteProduct::StormRelative)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RadarLayer {
    pub enabled: bool,
    pub palette: String,
    pub opacity: f32,
    pub smoothing: bool,
    pub mode: RadarMode,
    pub include_nowcast: bool,
    pub source: RadarSource,
    /// NEXRAD site id ("VWX") or "AUTO" for the site nearest the map centre.
    pub site: String,
    pub product: SiteProduct,
    /// Single-site reflectivity under this dBZ is hidden (clear air, insects, clutter); 0 = off.
    pub site_min_dbz: f32,
}

impl Default for RadarLayer {
    fn default() -> Self {
        Self {
            enabled: true,
            palette: "4".into(),
            opacity: 0.85,
            smoothing: true,
            mode: RadarMode::Loop,
            include_nowcast: false,
            source: RadarSource::Composite,
            site: "AUTO".into(),
            product: SiteProduct::SuperRes,
            site_min_dbz: crate::map::site_radar::DEFAULT_MIN_DBZ,
        }
    }
}

impl RadarLayer {
    pub fn resolved_site(&self, lat: f64, lon: f64) -> String {
        crate::map::style::site_for(&self.site, lat, lon)
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum SatChannel {
    /// LibreWXR infrared, through the site proxy.
    #[default]
    Infrared,
    /// GOES-East band 13 "clean" longwave IR.
    CleanIr,
    /// GOES-East band 2 visible (0.5 km).
    Visible,
    /// GOES-East band 9 mid-level water vapor.
    WaterVapor,
}

impl SatChannel {
    pub const ALL: [SatChannel; 4] = [SatChannel::Infrared, SatChannel::CleanIr, SatChannel::Visible, SatChannel::WaterVapor];

    pub fn label(self) -> &'static str {
        match self {
            SatChannel::Infrared => "Infrared",
            SatChannel::CleanIr => "GOES Clean IR",
            SatChannel::Visible => "GOES Visible",
            SatChannel::WaterVapor => "Water Vapor",
        }
    }

    pub fn iem_layer(self) -> &'static str {
        match self {
            SatChannel::Infrared | SatChannel::CleanIr => "goes_east_conus_ch13",
            SatChannel::Visible => "goes_east_conus_ch02",
            SatChannel::WaterVapor => "goes_east_conus_ch09",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SatelliteLayer {
    pub enabled: bool,
    pub opacity: f32,
    pub channel: SatChannel,
}

impl Default for SatelliteLayer {
    fn default() -> Self {
        Self {
            enabled: false,
            opacity: crate::map::style::SAT_OPACITY,
            channel: SatChannel::Infrared,
        }
    }
}

/// MRMS multi-sensor rainfall totals.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PrecipWindow {
    Hour1,
    Hours24,
    Hours48,
    Hours72,
}

impl PrecipWindow {
    pub const ALL: [PrecipWindow; 4] = [PrecipWindow::Hour1, PrecipWindow::Hours24, PrecipWindow::Hours48, PrecipWindow::Hours72];

    pub fn label(self) -> &'static str {
        match self {
            PrecipWindow::Hour1 => "1 hr",
            PrecipWindow::Hours24 => "24 hr",
            PrecipWindow::Hours48 => "48 hr",
            PrecipWindow::Hours72 => "72 hr",
        }
    }

    pub fn iem_layer(self) -> &'static str {
        match self {
            PrecipWindow::Hour1 => "q2-n1p-900913",
            PrecipWindow::Hours24 => "q2-p24h-900913",
            PrecipWindow::Hours48 => "q2-p48h-900913",
            PrecipWindow::Hours72 => "q2-p72h-900913",
        }
    }
}

/// Gridded weather data drawn over the map (Open-Meteo model data).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DataField {
    Temperature,
    FeelsLike,
    DewPoint,
    Humidity,
    WindSpeed,
    WindGust,
    Pressure,
    CloudCover,
    /// Model precipitation rate by type (rain / mix / snow) - the Futurecast.
    Futurecast,
    /// Accumulated precipitation from now to the forecast hour.
    RainTotal,
    /// Accumulated snowfall from now to the forecast hour.
    SnowTotal,
    /// Highest temperature in the next 24 hours.
    HighTemp,
    /// Lowest temperature in the next 24 hours.
    LowTemp,
    /// Temperature this time tomorrow minus now.
    TempChange,
    /// Strongest gust in the next 24 hours.
    MaxGust,
    /// Freezing rain / mix in the next 48 hours (liquid equivalent).
    IceTotal,
    /// Where surface winds pile together (storms often start here); per hour x 10^-5.
    Convergence,
    /// Convergence weighted by moisture: where storms can feed.
    MoistureConvergence,
    /// Equivalent potential temperature (heat + moisture), K.
    ThetaE,
    /// Wind carrying warm, moist air in (positive) or out.
    ThetaEAdvection,
    /// Pressure minus the regional average: meso highs and lows, mb.
    PressureAnomaly,
    /// Sharp dew point change across short distances: the dryline, °F per 10 mi.
    Dryline,
    /// Sharp temperature change: fronts and outflow boundaries, °F per 10 mi.
    Boundaries,
    /// Cloudy vs sunny edges where boundaries form, % per 10 mi.
    DiffHeating,
    /// Temperature drop over the last 2 hours under rain: storm cold pools, °F.
    ColdPool,
    /// Chance of 1" or more of snow in the 24 hours from the forecast hour, %.
    SnowChance,
    /// Winter road hazard 0-3: 1 wet / slushy or frost, 2 icy, 3 snow-covered.
    RoadHazard,
    /// Fog risk from dew point spread, humidity and light wind, %.
    FogRisk,
    /// Convective available potential energy, J/kg (severe ingredients).
    Cape,
    /// STP estimate from CAPE, LCL, low-level and deep shear.
    Stp,
    /// SCP estimate from CAPE, helicity and deep shear.
    Scp,
    /// 850-500 mb lapse rate, °C/km.
    LapseRate,
    /// Lifted condensation level height, feet.
    Lcl,
    /// Height where the wet-bulb temperature reaches 0°C, feet.
    WetBulbZero,
    /// Height of the 0°C level, feet.
    FreezingLevel,
    /// 700 mb vertical velocity: rising (positive) / sinking air.
    Omega,
    /// 850 mb Q-vector convergence: large-scale lift (positive).
    QVector,
    /// Warm (positive) or cold air being blown in, °F per hour.
    TempAdvection,
    /// Where temperature contrasts are tightening: fronts forming.
    Frontogenesis,
    /// Rain / mix / snow from wet-bulb temperature and the warm layer aloft (1 rain, 2 mix, 3 snow).
    PrecipType,
    /// Ice that freezing rain could build up, inches per hour.
    IceAccretion,
    /// Snowfall rate, inches per hour.
    SnowRate,
    /// How fluffy the snow is (Kuchera-style, from the warmest layer).
    SnowRatio,
    /// Lake-air temperature difference with converging winds (higher = stronger bands).
    LakeEffect,
    /// Surface (0 cm) soil temperature, °F: will snow stick?
    GroundTemp,
    /// Snow squall parameter (above 1 = squalls possible).
    SnowSquall,
    /// US AQI (EPA): 0-50 good … 300+ hazardous.
    AirQuality,
    /// Fine particles, mostly wildfire smoke, µg/m³.
    Smoke,
    /// Estimated visibility from smoke particles, miles.
    SmokeVisibility,
    /// Fosberg fire weather index from temperature, humidity and wind (50+ = high).
    FireDanger,
    /// Top-layer soil moisture, percent of saturation: wet ground floods faster.
    SoilMoisture,
    /// Estimated 3-hour rain needed to flood, from soil moisture and towns (lower = easier to flood).
    FloodGuidance,
    /// Forecast 3-hour rain as a percent of the estimated flash flood guidance (100% = flooding likely).
    FloodRatio,
    /// 0-10 index from wet soil, heavy rain rates and towns.
    FloodPotential,
    /// Rain rates of an inch an hour or more.
    RainRateExceed,
    /// Snow accumulation at a fixed 10:1 snow-to-liquid ratio.
    Snow10,
    /// Snow accumulation with the temperature-based Kuchera ratio.
    SnowKuchera,
}

/// How the forecast picker groups products (Pivotal-style categories).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldGroup {
    Precipitation,
    Winter,
    Temperature,
    Wind,
    Severe,
    Clouds,
    AirFire,
    Flooding,
}

impl FieldGroup {
    pub const ALL: [FieldGroup; 8] = [
        Self::Precipitation, Self::Winter, Self::Temperature, Self::Wind,
        Self::Severe, Self::Clouds, Self::AirFire, Self::Flooding,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Precipitation => "Precipitation",
            Self::Winter => "Winter",
            Self::Temperature => "Temperature",
            Self::Wind => "Wind & pressure",
            Self::Severe => "Severe",
            Self::Clouds => "Clouds & fog",
            Self::AirFire => "Air & fire",
            Self::Flooding => "Flooding",
        }
    }
}

impl DataField {
    pub const ALL: [DataField; 57] = [
        DataField::Convergence,
        DataField::MoistureConvergence,
        DataField::ThetaE,
        DataField::ThetaEAdvection,
        DataField::PressureAnomaly,
        DataField::Dryline,
        DataField::Boundaries,
        DataField::DiffHeating,
        DataField::ColdPool,

        DataField::HighTemp,
        DataField::LowTemp,
        DataField::TempChange,
        DataField::MaxGust,
        DataField::IceTotal,
        DataField::SnowChance,
        DataField::RoadHazard,
        DataField::FogRisk,
        DataField::Cape,
        DataField::Stp,
        DataField::Scp,
        DataField::LapseRate,
        DataField::Lcl,
        DataField::WetBulbZero,
        DataField::FreezingLevel,
        DataField::Omega,
        DataField::QVector,
        DataField::TempAdvection,
        DataField::Frontogenesis,
        DataField::PrecipType,
        DataField::IceAccretion,
        DataField::SnowRate,
        DataField::SnowRatio,
        DataField::LakeEffect,
        DataField::GroundTemp,
        DataField::SnowSquall,
        DataField::AirQuality,
        DataField::Smoke,
        DataField::SmokeVisibility,
        DataField::FireDanger,
        DataField::SoilMoisture,
        DataField::FloodGuidance,
        DataField::FloodRatio,
        DataField::FloodPotential,
        DataField::RainRateExceed,
        DataField::Futurecast,
        DataField::Temperature,
        DataField::FeelsLike,
        DataField::DewPoint,
        DataField::Humidity,
        DataField::WindSpeed,
        DataField::WindGust,
        DataField::Pressure,
        DataField::CloudCover,
        DataField::RainTotal,
        DataField::SnowTotal,
        DataField::Snow10,
        DataField::SnowKuchera,
    ];

    /// Picker category.
    pub fn group(self) -> FieldGroup {
        use DataField::*;
        match self {
            Futurecast | RainTotal | PrecipType | RainRateExceed => FieldGroup::Precipitation,
            SnowTotal | Snow10 | SnowKuchera | SnowRate | SnowRatio | SnowChance | IceTotal | IceAccretion
            | FreezingLevel | WetBulbZero | GroundTemp | LakeEffect | SnowSquall | RoadHazard => FieldGroup::Winter,
            Temperature | FeelsLike | DewPoint | Humidity | HighTemp | LowTemp | TempChange | TempAdvection | ThetaE => FieldGroup::Temperature,
            WindSpeed | WindGust | MaxGust | Pressure | PressureAnomaly => FieldGroup::Wind,
            Cape | Stp | Scp | LapseRate | Lcl | Convergence | MoistureConvergence | ThetaEAdvection | Dryline
            | Boundaries | DiffHeating | ColdPool | Omega | QVector | Frontogenesis => FieldGroup::Severe,
            CloudCover | FogRisk => FieldGroup::Clouds,
            AirQuality | Smoke | SmokeVisibility | FireDanger => FieldGroup::AirFire,
            SoilMoisture | FloodGuidance | FloodRatio | FloodPotential => FieldGroup::Flooding,
        }
    }

    /// Accumulated from "now" to the forecast hour.
    pub fn is_total(self) -> bool {
        matches!(self, DataField::RainTotal | DataField::SnowTotal | DataField::Snow10 | DataField::SnowKuchera)
    }

    /// Needs the upper-air / soil variables, so the grid loads them automatically.
    pub fn needs_upper_air(self) -> bool {
        use DataField::*;
        matches!(self, Cape | Stp | Scp | LapseRate | Lcl | WetBulbZero | FreezingLevel | Omega | QVector
            | TempAdvection | Frontogenesis | PrecipType | IceAccretion | SnowRatio | SnowKuchera | LakeEffect
            | GroundTemp | SnowSquall | SoilMoisture | FloodGuidance | FloodRatio | FloodPotential
            | ThetaE | ThetaEAdvection | MoistureConvergence)
    }

    /// Needs the air-quality feed.
    pub fn needs_air(self) -> bool {
        matches!(self, DataField::AirQuality | DataField::Smoke | DataField::SmokeVisibility)
    }

    pub fn label(self) -> &'static str {
        match self {
            DataField::Temperature => "Temperature",
            DataField::FeelsLike => "Feels like",
            DataField::DewPoint => "Dew point",
            DataField::Humidity => "Humidity",
            DataField::WindSpeed => "Wind",
            DataField::WindGust => "Wind gusts",
            DataField::Pressure => "Pressure",
            DataField::CloudCover => "Cloud cover",
            DataField::Futurecast => "Futurecast",
            DataField::RainTotal => "Rain total",
            DataField::SnowTotal => "Snow total",
            DataField::HighTemp => "Forecast highs",
            DataField::LowTemp => "Forecast lows",
            DataField::TempChange => "24-hour change",
            DataField::MaxGust => "Peak gusts",
            DataField::IceTotal => "Ice forecast",
            DataField::Convergence => "Surface convergence",
            DataField::MoistureConvergence => "Moisture convergence",
            DataField::ThetaE => "Theta-E",
            DataField::ThetaEAdvection => "Theta-E advection",
            DataField::PressureAnomaly => "Pressure perturbation",
            DataField::Dryline => "Dryline",
            DataField::Boundaries => "Boundaries",
            DataField::DiffHeating => "Differential heating",
            DataField::ColdPool => "Cold pools",
            DataField::SnowChance => "Chance of 1\" snow",
            DataField::RoadHazard => "Winter road hazard",
            DataField::FogRisk => "Fog risk",
            DataField::Cape => "Instability (CAPE)",
            DataField::Stp => "Significant tornado parameter",
            DataField::Scp => "Supercell composite",
            DataField::LapseRate => "Mid-level lapse rate",
            DataField::Lcl => "Cloud base (LCL)",
            DataField::WetBulbZero => "Wet-bulb zero",
            DataField::FreezingLevel => "Freezing level",
            DataField::Omega => "Vertical motion (700 mb)",
            DataField::QVector => "Q-vector forcing",
            DataField::TempAdvection => "Temperature advection",
            DataField::Frontogenesis => "Frontogenesis",
            DataField::PrecipType => "Precip type (wet-bulb)",
            DataField::IceAccretion => "Ice accretion risk",
            DataField::SnowRate => "Snowfall rate",
            DataField::SnowRatio => "Snow-to-liquid ratio",
            DataField::LakeEffect => "Lake-effect potential",
            DataField::GroundTemp => "Ground temperature",
            DataField::SnowSquall => "Snow squall parameter",
            DataField::AirQuality => "Air quality index",
            DataField::Smoke => "Smoke (PM2.5)",
            DataField::SmokeVisibility => "Visibility in smoke",
            DataField::FireDanger => "Fire danger (Fosberg)",
            DataField::SoilMoisture => "Soil moisture",
            DataField::FloodGuidance => "Flash flood guidance (est.)",
            DataField::FloodRatio => "Rain vs flood guidance",
            DataField::FloodPotential => "Flash flood potential",
            DataField::RainRateExceed => "Heavy rain rate",
            DataField::Snow10 => "Snow (10:1)",
            DataField::SnowKuchera => "Snow (Kuchera)",

        }
    }

    pub fn viewer_label(self) -> &'static str {
        match self {
            Self::FeelsLike => "Feels-like temperature",
            Self::TempChange => "Temperature change in 24 hours",
            Self::Convergence => "Where winds come together",
            Self::MoistureConvergence => "Where moist air comes together",
            Self::ThetaE => "Warmth and moisture",
            Self::ThetaEAdvection => "Movement of warm, moist air",
            Self::PressureAnomaly => "Pressure compared with nearby areas",
            Self::Dryline => "Dry air boundary",
            Self::Boundaries => "Temperature boundaries",
            Self::DiffHeating => "Differences in daytime heating",
            Self::ColdPool => "Cool air behind storms",
            Self::Cape => "Thunderstorm energy",
            Self::Stp => "Conditions for strong tornadoes",
            Self::Scp => "Conditions for rotating storms",
            Self::LapseRate => "Cooling with height",
            Self::Lcl => "Cloud base height",
            Self::WetBulbZero => "Wet-bulb freezing height",
            Self::Omega => "Rising and sinking air",
            Self::QVector => "Areas of rising air",
            Self::TempAdvection => "Movement of warm and cold air",
            Self::Frontogenesis => "Developing weather fronts",
            Self::PrecipType => "Rain, snow or wintry mix",
            Self::IceAccretion => "Ice buildup risk",
            Self::SnowRatio => "Snow density",
            Self::LakeEffect => "Lake-effect snow potential",
            Self::SnowSquall => "Conditions for snow squalls",
            Self::Smoke => "Smoke levels",
            Self::FireDanger => "Fire danger",
            Self::FloodGuidance => "Rainfall needed for flash flooding",
            Self::FloodRatio => "Rainfall compared with flood thresholds",
            _ => self.label(),
        }
    }

    pub fn fixed_window(self) -> Option<&'static str> {
        match self {
            Self::HighTemp | Self::LowTemp | Self::MaxGust => Some("Next 24 hours"),
            Self::TempChange => Some("Change over the next 24 hours"),
            Self::IceTotal => Some("Next 48 hours"),
            _ => None,
        }
    }

    pub fn broadcast_title(self) -> &'static str {
        match self {
            Self::Temperature => "Temperature Outlook",
            Self::WindSpeed => "Wind Forecast",
            Self::WindGust => "Wind Gust Forecast",
            Self::RainTotal => "Rainfall Forecast",
            Self::SnowTotal | Self::Snow10 | Self::SnowKuchera => "Snowfall Forecast",
            Self::CloudCover => "Cloud Cover Forecast",
            _ => self.viewer_label(),
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            DataField::Temperature | DataField::FeelsLike | DataField::DewPoint | DataField::HighTemp | DataField::LowTemp | DataField::TempChange => "°",
            DataField::MaxGust => " mph",
            DataField::IceTotal => "\"",
            DataField::ThetaE => " K",
            DataField::PressureAnomaly => " mb",
            DataField::ColdPool | DataField::Dryline | DataField::Boundaries => "°",
            DataField::SnowChance | DataField::FogRisk => "%",
            DataField::RoadHazard => "",
            DataField::Cape => " J/kg",
            DataField::Stp => "",
            DataField::Scp => "",
            DataField::LapseRate => " °C/km",
            DataField::Lcl => " ft",
            DataField::WetBulbZero => " ft",
            DataField::FreezingLevel => " ft",
            DataField::Omega => "",
            DataField::QVector => "",
            DataField::TempAdvection => "°/hr",
            DataField::Frontogenesis => "",
            DataField::PrecipType => "",
            DataField::IceAccretion => "\"",
            DataField::SnowRate => "\"/hr",
            DataField::SnowRatio => ":1",
            DataField::LakeEffect => "",
            DataField::GroundTemp => "°",
            DataField::SnowSquall => "",
            DataField::AirQuality => "",
            DataField::Smoke => " µg/m³",
            DataField::SmokeVisibility => " mi",
            DataField::FireDanger => "",
            DataField::SoilMoisture => "%",
            DataField::FloodGuidance => "\"",
            DataField::FloodRatio => "%",
            DataField::FloodPotential => "",
            DataField::RainRateExceed => "\"/hr",
            DataField::DiffHeating => "%",
            DataField::Convergence | DataField::MoistureConvergence | DataField::ThetaEAdvection => "",
            DataField::Humidity | DataField::CloudCover => "%",
            DataField::WindSpeed | DataField::WindGust => " mph",
            DataField::Pressure => " mb",
            DataField::Futurecast => "",
            DataField::RainTotal | DataField::SnowTotal | DataField::Snow10 | DataField::SnowKuchera => "\"",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DataLayer {
    pub field: Option<DataField>,
    pub model_product: crate::futurecast::Product,
    pub model_pins: [Option<crate::futurecast::RunPin>; 3],
    /// Model behind the gridded data maps (Forecast > Model).
    pub grid_model: crate::grid::GridModel,
    pub opacity: f32,
    /// Value placards on towns (temps, winds…), de-cluttered.
    pub city_plots: bool,
    /// Animated wind flow particles.
    pub wind_particles: bool,
    /// Station-style wind barbs on a regular grid.
    pub wind_barbs: bool,
    /// Isobars with H / L centres.
    pub isobars: bool,
    /// Forecast offset from now, in hours (fractional while animating).
    pub hour: f32,
    /// How many hours the Futurecast / data loop covers.
    pub loop_hours: u32,
    pub play_on_take: bool,
}

impl Default for DataLayer {
    fn default() -> Self {
        Self {
            field: None,
            model_product: crate::futurecast::Product::default(),
            model_pins: [None; 3],
            grid_model: crate::grid::GridModel::default(),
            opacity: 0.7,
            city_plots: true,
            wind_particles: false,
            wind_barbs: false,
            isobars: false,
            hour: 0.0,
            loop_hours: 24,
            play_on_take: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum AnnotationKind {
    Pen,
    Arrow,
    Circle,
    Text,
    Polygon,
    RaisedArea,
    Marker,
    ColdFront,
    WarmFront,
    StationaryFront,
    OccludedFront,
    Trough,
    High,
    Low,
    /// Two points: distance and drive time.
    Ruler,
    /// Darkens everything outside a circle to highlight a storm.
    Spotlight,
}

impl AnnotationKind {
    pub fn label(self) -> &'static str {
        match self {
            AnnotationKind::Pen => "Pen",
            AnnotationKind::Arrow => "Arrow",
            AnnotationKind::Circle => "Circle",
            AnnotationKind::Text => "Text",
            AnnotationKind::Polygon => "Area",
            AnnotationKind::RaisedArea => "Raised area",
            AnnotationKind::Marker => "Marker",
            AnnotationKind::ColdFront => "Cold front",
            AnnotationKind::WarmFront => "Warm front",
            AnnotationKind::StationaryFront => "Stationary front",
            AnnotationKind::OccludedFront => "Occluded front",
            AnnotationKind::Trough => "Trough",
            AnnotationKind::High => "High",
            AnnotationKind::Low => "Low",
            AnnotationKind::Ruler => "Ruler",
            AnnotationKind::Spotlight => "Spotlight",
        }
    }

    pub fn is_front(self) -> bool {
        matches!(
            self,
            AnnotationKind::ColdFront
                | AnnotationKind::WarmFront
                | AnnotationKind::StationaryFront
                | AnnotationKind::OccludedFront
                | AnnotationKind::Trough
        )
    }
}

/// A telestrator mark, geo-anchored so it stays put while the map moves.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Annotation {
    pub id: String,
    pub kind: AnnotationKind,
    /// (lat, lon) points. Circle: [center, edge]. Text/Marker: [anchor].
    pub points: Vec<(f64, f64)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub area_rings: Vec<Vec<(f64, f64)>>,
    pub color: String,
    pub width: f32,
    pub text: String,
    pub fill: bool,
    /// Movement path: where the mark travels to, as a (lat, lon) offset. Loops on air.
    #[serde(default)]
    pub motion: Option<(f64, f64)>,
    /// Seconds per movement / reveal loop (0 = 8 s).
    #[serde(default)]
    pub loop_secs: f32,
    /// Draw the line on gradually (fronts, arrows, pen) each loop.
    #[serde(default)]
    pub reveal: bool,
    /// Event replay: only show while the radar loop is between these times (unix seconds).
    #[serde(default)]
    pub show_from: Option<i64>,
    #[serde(default)]
    pub show_until: Option<i64>,
    /// Measurement mark: pens show their length, areas their size.
    #[serde(default)]
    pub measure: bool,
    /// Ground elevation at the ends (feet), filled in after a ruler is drawn.
    #[serde(default)]
    pub elevation_ft: Option<(f64, f64)>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum RaiseStyle {
    #[default]
    Lift,
    Spotlight,
    Outline,
}

impl Annotation {
    pub fn visible_at(&self, radar_time: Option<i64>) -> bool {
        radar_time.is_none_or(|time| {
            self.show_from.is_none_or(|start| time >= start) && self.show_until.is_none_or(|end| time <= end)
        })
    }
}

/// Per-layer look: opacity, blend mode, pulse, geographic clip and an optional colour override
/// (otherwise the layer inherits the station theme).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LayerFx {
    pub opacity: f32,
    /// "normal", "multiply", "screen", "overlay", "lighten", "darken".
    pub blend: String,
    /// Gently pulse the layer (draws the eye on air).
    pub pulse: bool,
    /// Only draw inside this circle: (lat, lon, miles).
    pub clip: Option<(f64, f64, f64)>,
    /// Recolour the whole layer (None = inherit the theme's colours).
    pub tint: Option<String>,
}

impl Default for LayerFx {
    fn default() -> Self {
        Self { opacity: 1.0, blend: "normal".into(), pulse: false, clip: None, tint: None }
    }
}

impl LayerFx {
    pub fn is_plain(&self) -> bool {
        self.opacity >= 0.999 && (self.blend.is_empty() || self.blend == "normal") && !self.pulse && self.clip.is_none() && self.tint.is_none()
    }
}

/// Overlay layers in their default drawing order (bottom first): key, label, group path.
pub const OVERLAY_LAYERS: &[(&str, &str, &str)] = &[
    ("spc", "SPC outlook", "Outlooks"),
    ("qpf", "Rainfall forecast", "Outlooks/Hydro"),
    ("ero", "Flooding rain risk", "Outlooks/Hydro"),
    ("mesoscale", "Mesoscale discussions", "Outlooks"),
    ("boundaries", "Counties and states", "Reference"),
    ("warnings", "Warnings and watches", "Severe"),
    ("roads", "Road conditions", "Travel"),
    ("rings", "Range rings", "Reference"),
    ("cells", "Storm cells", "Severe/Storms"),
    ("tracks", "Storm track", "Severe/Storms"),
    ("reports", "Storm reports", "Severe"),
    ("countdowns", "Warning countdowns", "Severe"),
    ("lightning_density", "Lightning density", "Severe/Lightning"),
    ("lightning", "Lightning strikes", "Severe/Lightning"),
    ("rivers", "River gauges", "Hydro"),
    ("centers", "Warming / cooling centers", "Community"),
    ("regions", "Forecast regions", "Reference"),
    ("town_labels", "Town labels", "Reference"),
    ("mcs", "Storm complexes", "Severe/Storms"),
    ("lightning_anim", "Lightning animation", "Severe/Lightning"),
    ("lightning_clusters", "Lightning clusters", "Severe/Lightning"),
    ("storm_tracks", "Storm tracks", "Severe/Storms"),
    ("traffic", "Traffic", "Travel"),
    ("flood_map", "Flood extent", "Hydro"),
    ("damage_map", "Damage reports", "Severe"),
    ("viewer_reports", "Viewer reports", "Community"),
    ("eoc", "Emergency management", "Community"),
    ("outages", "Power outages", "Community"),
    ("nowcast", "Storm nowcast", "Severe/Storms"),
    ("tornado_history", "Tornado history", "History"),
    ("history", "Event replay", "History"),
    ("drought", "Drought monitor", "Hazards"),
    ("quakes", "Earthquakes (USGS)", "Hazards"),
    ("fire_outlook", "Fire weather outlook", "Hazards/Fire"),
    ("wildfires", "Wildfire perimeters", "Hazards/Fire"),
    ("tropical", "Tropical", "Hazards/Tropical"),
    ("custom", "Custom data", "Reference"),
    ("annotations", "Telestration", "Reference"),
];

/// Built-in dependencies: (layer, needs). A layer is only drawn when what it needs is on.
/// (A drawn storm track used to need Storm cells on, so the Storm Track tool drew nothing after
/// Clear map: it stands on its own now.)
pub const LAYER_DEPENDS: &[(&str, &str)] = &[("countdowns", "warnings")];

impl MapScene {
    /// Overlay layer keys in drawing order (custom order first, then the rest in default order).
    pub fn overlay_order(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for k in &self.layer_order {
            if let Some((key, _, _)) = OVERLAY_LAYERS.iter().find(|(key, _, _)| key == k) {
                if !out.contains(key) {
                    out.push(key);
                }
            }
        }
        for (key, _, _) in OVERLAY_LAYERS {
            if !out.contains(key) {
                out.push(key);
            }
        }
        out
    }

    /// The layer's own switch (before groups and rules).
    pub fn overlay_switch(&self, key: &str) -> bool {
        match key {
            "spc" => self.spc_day.is_some(),
            "qpf" => self.qpf.is_some(),
            "ero" => self.ero_day.is_some(),
            "mesoscale" => self.mesoscale,
            "boundaries" => self.counties || self.states,
            "warnings" => self.warnings,
            "roads" => self.roads,
            "rings" => self.range_rings.is_some(),
            "cells" => self.storm_cells || self.cell_paths,
            "tracks" => self.storm_track.is_some() || !self.extra_tracks.is_empty(),
            "reports" => self.reports,
            "countdowns" => self.warning_countdowns,
            "lightning_density" => self.lightning_density,
            "lightning" => self.lightning,
            "rivers" => self.rivers,
            "centers" => self.centers,
            "regions" => self.regions,
            "town_labels" => self.town_labels,
            "mcs" => self.mcs,
            "lightning_anim" => self.lightning_anim,
            "lightning_clusters" => self.lightning_clusters,
            "storm_tracks" => self.storm_tracks,
            "traffic" => self.traffic,
            "flood_map" => self.flood_map,
            "damage_map" => self.damage_map,
            "annotations" => !self.annotations.is_empty(),
            "drought" => self.drought,
            "quakes" => self.quakes,
            "tornado_history" => self.tornado_history,
            "viewer_reports" => self.viewer_reports,
            "eoc" => self.eoc,
            "outages" => self.outages,
            "nowcast" => self.nowcast,
            "history" => self.history_time.is_some(),
            "fire_outlook" => self.fire_outlook,
            "wildfires" => self.wildfires,
            "tropical" => self.tropical,
            "custom" => self.custom_layers.iter().any(|l| l.visible),
            _ => false,
        }
    }

    /// Whether a layer draws: its switch, its group not hidden, and its dependencies met.
    pub fn overlay_visible(&self, key: &str) -> bool {
        if !self.overlay_switch(key) {
            return false;
        }
        if let Some((_, _, group)) = OVERLAY_LAYERS.iter().find(|(k, _, _)| *k == key) {
            // A hidden group hides everything nested under it.
            if self.hidden_groups.iter().any(|g| *group == g.as_str() || group.starts_with(&format!("{g}/"))) {
                return false;
            }
        }
        let needs = LAYER_DEPENDS
            .iter()
            .filter(|(k, _)| *k == key)
            .map(|(_, n)| n.to_string())
            .chain(self.layer_rules.iter().filter(|(k, _)| k == key).map(|(_, n)| n.clone()));
        for n in needs {
            if n != key && !self.overlay_switch(&n) {
                return false;
            }
        }
        true
    }

    /// Copy the layer setup (switches, looks, order, groups, rules) without the view or marks,
    /// for a layer preset file.
    pub fn layer_preset(&self) -> serde_json::Value {
        let mut v = serde_json::to_value(self).unwrap_or_default();
        if let Some(o) = v.as_object_mut() {
            for k in ["view", "annotations", "storm_track", "extra_tracks", "title", "custom_title", "selected_alert", "overlay_graphic", "overlay_slots", "range_rings", "camera_path", "follow_storm"] {
                o.remove(k);
            }
        }
        v
    }

    /// Apply a layer preset on top of this scene.
    pub fn apply_layer_preset(&mut self, preset: &serde_json::Value) {
        let mut v = serde_json::to_value(&*self).unwrap_or_default();
        if let (Some(o), Some(p)) = (v.as_object_mut(), preset.as_object()) {
            for (k, val) in p {
                o.insert(k.clone(), val.clone());
            }
        }
        if let Ok(scene) = serde_json::from_value::<MapScene>(v) {
            *self = scene;
        }
    }
}

/// A hand adjustment to one town label: nudge it, hide it, or always show it.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LabelOverride {
    pub name: String,
    pub dx: f32,
    pub dy: f32,
    pub hidden: bool,
    pub always: bool,
}

/// A storm track: origin cell, motion, and ETA marks every `step_minutes`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct StormTrack {
    pub lat: f64,
    pub lon: f64,
    pub heading_deg: f64,
    pub speed_mph: f64,
    pub minutes: u32,
    pub step_minutes: u32,
    pub cone_deg: f64,
    pub label: String,
    pub color: String,
    /// Freeform track: hand-drawn path (lat, lon) from the storm onward. When it has two or
    /// more points it replaces the straight cone; ETAs follow the path at `speed_mph`.
    pub path: Vec<(f64, f64)>,
    /// Corridor half-width in miles for freeform tracks (towns inside get ETAs).
    pub corridor_mi: f64,
    /// Sponsor line under the ETA panel ("Storm Tracker brought to you by …").
    pub sponsor: String,
    /// When the cone was last placed or moved with the storm (Unix seconds, 0 = never).
    pub placed_at: i64,
    /// Keep the cone on its storm as new radar data arrives (position, heading, speed).
    pub follow: bool,
}

impl StormTrack {
    pub fn is_freeform(&self) -> bool {
        self.path.len() >= 2
    }
}

impl Default for StormTrack {
    fn default() -> Self {
        Self {
            lat: 38.39,
            lon: -86.93,
            heading_deg: 65.0,
            speed_mph: 35.0,
            minutes: 60,
            step_minutes: 15,
            cone_deg: 18.0,
            label: "STORM TRACK".into(),
            color: "#FF3B3B".into(),
            path: Vec::new(),
            corridor_mi: 6.0,
            sponsor: String::new(),
            placed_at: 0,
            follow: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RangeRings {
    pub lat: f64,
    pub lon: f64,
    /// Ring radii in miles.
    pub miles: Vec<f64>,
}

impl Default for RangeRings {
    fn default() -> Self {
        Self {
            lat: 38.3914,
            lon: -86.9311,
            miles: vec![10.0, 25.0, 50.0],
        }
    }
}

/// One stop on a recorded camera path.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct CameraKey {
    pub view: crate::config::GeoView,
    /// Seconds to fly here from the previous stop.
    pub fly: f32,
    /// Seconds to hold here before the next stop.
    pub hold: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MapScene {
    pub view: GeoView,
    pub basemap: Basemap,
    pub labels: bool,
    pub radar: RadarLayer,
    pub satellite: SatelliteLayer,
    pub warnings: bool,
    pub warnings_fill: bool,
    /// Pattern inside each warning type (accessibility: color is not the only cue).
    pub warning_patterns: bool,
    /// SPC categorical outlook day to show (1-3), if any.
    pub spc_day: Option<u8>,
    /// Which SPC layer: "categorical" (risk levels), "tornado", "hail", "wind" or "severe".
    pub spc_layer: String,
    /// Active SPC mesoscale discussions.
    pub mesoscale: bool,
    /// WPC rainfall forecast to show (Day 1, 3-day, 7-day), if any.
    pub qpf: Option<crate::feeds::QpfPeriod>,
    /// WPC Excessive Rainfall Outlook day (1-3), if any.
    pub ero_day: Option<u8>,
    /// River gauges coloured by flood category.
    pub rivers: bool,
    /// Warming / cooling centers from the community board.
    pub centers: bool,
    /// Lightning density squares (last 30 min).
    pub lightning_density: bool,
    /// Bold town names inside the called-out warning.
    pub warning_towns: bool,
    /// Recorded camera path, played when this map is taken (or with Play path).
    pub camera_path: Vec<CameraKey>,
    /// Keep the camera on the called-out storm (or the strongest cell) as it moves.
    pub follow_storm: bool,
    /// Forecast regions (Forecast editor) outlined with their name and high.
    pub regions: bool,
    /// Adaptive quality (set by the app, never saved): cap on the radar tile zoom when frames
    /// run long, and on the loop length when the radar server is slow.
    #[serde(skip)]
    pub radar_maxzoom_cap: Option<u8>,
    #[serde(skip)]
    pub frame_cap: Option<usize>,
    /// Per-layer look, keyed by OVERLAY_LAYERS key.
    pub layer_fx: std::collections::HashMap<String, LayerFx>,
    /// Custom drawing order (keys, bottom first); missing keys keep their default place.
    pub layer_order: Vec<String>,
    /// Hidden layer groups ("Severe/Lightning" hides that sub-group, "Severe" all of Severe).
    pub hidden_groups: Vec<String>,
    /// Station rules: (layer, needs) - show a layer only while another is on.
    pub layer_rules: Vec<(String, String)>,
    /// Our own town labels (crisp at any size), instead of or with the basemap labels.
    pub town_labels: bool,
    /// "classic", "bold", "minimal" or "dark".
    pub label_style: String,
    /// Custom map theme (Map Studio > Map themes); empty = the basemap setting.
    pub theme: String,
    /// 0.7-1.6: label size multiplier on top of the zoom scaling.
    pub label_scale: f32,
    pub label_overrides: Vec<LabelOverride>,
    /// Interstate / highway shields (Esri transportation reference layer).
    pub highways: bool,
    /// Storm tracks: our persistent ids, path history, trend arrows, lightning jumps.
    pub storm_tracks: bool,
    /// Lightning clusters with motion arrows.
    pub lightning_clusters: bool,
    /// Lightning density animated through the last hour in 10-minute steps.
    pub lightning_anim: bool,
    /// MCS / squall line outlines.
    pub mcs: bool,
    /// Urban drainage: towns shaded by size, highlighted when heavy rain is forecast there.
    pub urban_flood: bool,
    /// Approved viewer reports with a location.
    pub viewer_reports: bool,
    /// Emergency management: evacuation zones, shelters, resources, road closures.
    pub eoc: bool,
    /// Power outages by county: "now" (share out), "restoration" (restored since the peak) or
    /// "infrastructure" (critical facilities in hard-hit counties).
    pub outages: bool,
    pub outage_mode: String,
    /// Storm nowcast: "arrival", "cones", "initiation", "threats" or "verify".
    pub nowcast: bool,
    pub nowcast_mode: String,
    /// Event replay: show radar, satellite, warnings and reports as they were at this time.
    pub history_time: Option<i64>,
    pub radar_replay: Option<crate::history::RadarReplay>,
    pub site_replay: Option<crate::history::SiteReplay>,
    /// SPC tornado tracks since `tornado_since` (year), coloured by EF rating.
    pub tornado_history: bool,
    pub tornado_since: i32,
    /// Tropical: NHC cone, track, points, watches / warnings; wind probability / arrival /
    /// surge / model spaghetti by `tropical_layer` ("cone", "wind_prob", "arrival", "surge", "models").
    pub tropical: bool,
    pub tropical_layer: String,
    /// SPC fire weather outlook (Day 1).
    pub fire_outlook: bool,
    /// Wildfire perimeters (NIFC).
    pub wildfires: bool,
    /// US Drought Monitor categories.
    pub drought: bool,
    /// Which drought map: "now" (US Drought Monitor), "month" or "season" (NOAA CPC outlooks).
    pub drought_view: String,
    /// Earthquakes (USGS): circles sized by magnitude, coloured by age.
    pub quakes: bool,
    /// Days of earthquakes shown (1, 7 or 30) and the smallest magnitude.
    pub quake_days: u32,
    pub quake_min: f32,
    /// Custom data layers (CSV / GeoJSON, Map Studio > Custom data).
    pub custom_layers: Vec<crate::customdata::CustomLayer>,
    /// Ensemble values at towns: "" off, "mean_high", "spread_high", "p10_snow", "p90_snow",
    /// "prob_snow1", "prob_rain1", "prob_freeze".
    pub ens_map: String,
    /// Snowfall band track: where the heaviest snow band moves over the next 12 hours.
    pub snow_band: bool,
    /// Traffic: incidents, slowdowns and closures (INDOT) sized by impact.
    pub traffic: bool,
    /// Camera locations (Settings > Cameras), named.
    pub cameras: bool,
    /// Airport observation plots (METAR): temperature, dew point, wind, flight category.
    pub metar_plots: bool,
    /// Automatic impact labels at the worst spots (heavy snow, ice, gusts, flooding rain, fog…).
    pub impact_callouts: bool,
    /// Post-storm damage map: damage reports sized by magnitude, tornado tracks, summary.
    pub damage_map: bool,
    pub damage_hours: u32,
    /// Flood extent: flood warnings, gauges in flood, flooded / flood-prone roads.
    pub flood_map: bool,
    pub storm_cells: bool,
    pub cell_tracks: bool,
    /// 30 / 60 minute projected paths of the storm cells, on their own switch (independent of
    /// the storm icons).
    pub cell_paths: bool,
    /// Storm cells drawn as icon badges (thunderstorm / hail / heavy rain); the calm name and
    /// details are in the click pop-out.
    pub cell_labels: bool,
    pub range_rings: Option<RangeRings>,
    pub storm_track: Option<StormTrack>,
    pub annotations: Vec<Annotation>,
    pub raise_style: RaiseStyle,
    pub raise_visible: bool,
    /// Title banner across the top ("LIVE DOPPLER", "FUTURE RADAR", …).
    pub title: String,
    pub show_title: bool,
    #[serde(default)]
    pub custom_title: bool,
    pub show_legend: bool,
    pub show_timestamp: bool,
    pub show_logo: bool,
    #[serde(default)]
    pub watermark_visible: Option<bool>,
    #[serde(default)]
    pub show_sponsor: bool,
    #[serde(default)]
    pub sponsor_size: Option<u8>,
    pub show_clock: bool,
    /// Graphic (by id) composited on top of the map, e.g. a lower third.
    pub overlay_graphic: Option<String>,
    pub overlay_slots: [Option<String>; 2],
    /// Single map, side-by-side, or four-panel comparison.
    pub layout: PanelLayout,
    /// Slider position for the Slider layout (0-1 across the frame).
    pub wipe: f32,
    /// Products for panels 2-4 (panel 1 is this scene itself). Cameras are linked.
    pub panels: Vec<PanelSpec>,
    /// MRMS rainfall totals layer.
    pub precip_total: Option<PrecipWindow>,
    /// Lightning strikes (last 15 minutes) from the site's lightning feed.
    pub lightning: bool,
    /// Gridded data / Futurecast.
    pub data: DataLayer,
    /// NWS alert (by id) called out on air with a pop-up "blip".
    pub selected_alert: Option<String>,
    /// Warning countdown labels ("TOR • 14 MIN") on warning polygons.
    pub warning_countdowns: bool,
    /// Which NWS alert classes are drawn.
    pub alert_kinds: AlertKinds,
    /// NWS Local Storm Reports (last 24 h) as map markers.
    pub reports: bool,
    /// Hours of storm reports to show.
    pub report_hours: u32,
    /// Report types hidden from the map, as marker letters ("HS" hides hail and snow).
    pub report_hidden: String,
    /// INDOT road closures / incidents.
    pub roads: bool,
    /// Also show routine roadwork and restrictions.
    pub roads_all: bool,
    /// Presenter-dragged positions of on-air elements (title, legend, time, logo, clock, eta,
    /// overlay), as offsets in frame pixels from their default spots.
    pub onair_offsets: std::collections::BTreeMap<String, (f32, f32)>,
    /// County outlines (from about zoom 5.5).
    pub counties: bool,
    /// State outlines.
    pub states: bool,
    /// Opacity of county and state lines (0.1-1).
    pub boundary_opacity: f32,
    /// Additional storm tracks kept on the map (the main one is `storm_track`).
    pub extra_tracks: Vec<StormTrack>,
}

/// Which alert classes appear on the map.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AlertKinds {
    pub warnings: bool,
    pub watches: bool,
    pub advisories: bool,
    pub statements: bool,
}

impl Default for AlertKinds {
    fn default() -> Self {
        Self {
            warnings: true,
            watches: true,
            advisories: false,
            statements: false,
        }
    }
}

impl AlertKinds {
    pub fn allows(&self, alert: &crate::data::Alert) -> bool {
        use crate::data::AlertClass;
        match alert.class() {
            AlertClass::Warning => self.warnings,
            AlertClass::Watch => self.watches,
            AlertClass::Advisory => self.advisories,
            AlertClass::Statement => self.statements,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum PanelLayout {
    #[default]
    Single,
    Dual,
    Quad,
    /// Two full-frame maps with a slider between them (before / after, radar / model).
    Wipe,
}

impl PanelLayout {
    pub const ALL: [PanelLayout; 4] = [PanelLayout::Single, PanelLayout::Dual, PanelLayout::Quad, PanelLayout::Wipe];

    pub fn label(self) -> &'static str {
        match self {
            PanelLayout::Single => "Single",
            PanelLayout::Dual => "2-Up",
            PanelLayout::Quad => "4-Up",
            PanelLayout::Wipe => "Slider",
        }
    }

    pub fn count(self) -> usize {
        match self {
            PanelLayout::Single => 1,
            PanelLayout::Dual => 2,
            PanelLayout::Quad => 4,
            PanelLayout::Wipe => 2,
        }
    }

    /// Size of one panel in frame pixels.
    pub fn panel_size(self) -> (u32, u32) {
        match self {
            PanelLayout::Single | PanelLayout::Wipe => (FRAME_W, FRAME_H),
            PanelLayout::Dual => (FRAME_W / 2, FRAME_H),
            PanelLayout::Quad => (FRAME_W / 2, FRAME_H / 2),
        }
    }

    /// Top-left of panel `i` in frame pixels.
    pub fn panel_origin(self, i: usize) -> (u32, u32) {
        let (pw, ph) = self.panel_size();
        match self {
            PanelLayout::Single | PanelLayout::Wipe => (0, 0),
            PanelLayout::Dual => (pw * i as u32, 0),
            PanelLayout::Quad => (pw * (i as u32 % 2), ph * (i as u32 / 2)),
        }
    }

    /// Which panel a frame point falls in, and the point in that panel's pixels.
    pub fn locate(self, fx: f64, fy: f64) -> (usize, f64, f64) {
        let (pw, ph) = self.panel_size();
        let col = ((fx / pw as f64).floor().max(0.0) as usize).min(if matches!(self, PanelLayout::Single | PanelLayout::Wipe) { 0 } else { 1 });
        let row = ((fy / ph as f64).floor().max(0.0) as usize).min(if self == PanelLayout::Quad { 1 } else { 0 });
        let index = row * 2 + col;
        let (ox, oy) = self.panel_origin(index);
        (index, fx - ox as f64, fy - oy as f64)
    }
}

/// What a comparison panel shows.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PanelProduct {
    Radar,
    /// Single-site base velocity (the classic reflectivity / velocity 2-up).
    Velocity,
    /// Single-site storm-relative velocity.
    StormRelative,
    RadarSatellite,
    Satellite,
    Warnings,
    Outlook,
    /// Model precipitation (Futurecast) a few hours ahead: radar vs model.
    Futurecast,
    GfsTemperature,
    GfsReflectivity,
    HrrrReflectivity,
    /// The oldest radar frame in the loop: before / after.
    RadarEarlier,
    /// MRMS 24-hour rainfall.
    Rainfall,
    /// Model temperatures with town values (radar + observations side by side).
    Temperature,
    GfsPrecipitationRate,
}

impl PanelProduct {
    pub const ALL: [PanelProduct; 15] = [
        PanelProduct::Futurecast,
        PanelProduct::GfsPrecipitationRate,
        PanelProduct::GfsTemperature,
        PanelProduct::GfsReflectivity,
        PanelProduct::HrrrReflectivity,
        PanelProduct::RadarEarlier,
        PanelProduct::Rainfall,
        PanelProduct::Temperature,
        PanelProduct::Radar,
        PanelProduct::Velocity,
        PanelProduct::StormRelative,
        PanelProduct::RadarSatellite,
        PanelProduct::Satellite,
        PanelProduct::Warnings,
        PanelProduct::Outlook,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PanelProduct::Radar => "Radar",
            PanelProduct::Velocity => "Velocity",
            PanelProduct::StormRelative => "SRV",
            PanelProduct::RadarSatellite => "Radar + Sat",
            PanelProduct::Satellite => "IR Satellite",
            PanelProduct::Warnings => "Warnings",
            PanelProduct::Outlook => "SPC Outlook",
            PanelProduct::Futurecast => "GFS precipitation",
            PanelProduct::GfsPrecipitationRate => "GFS precipitation rate",
            PanelProduct::GfsTemperature => "GFS temperature",
            PanelProduct::GfsReflectivity => "GFS simulated radar",
            PanelProduct::HrrrReflectivity => "HRRR simulated radar",
            PanelProduct::RadarEarlier => "Radar earlier",
            PanelProduct::Rainfall => "Rain 24 h",
            PanelProduct::Temperature => "Temperatures",
        }
    }

    pub fn uses_radar(self) -> bool {
        matches!(self, PanelProduct::Radar | PanelProduct::RadarSatellite)
    }

    pub fn is_site(self) -> bool {
        matches!(self, PanelProduct::Velocity | PanelProduct::StormRelative)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PanelSpec {
    pub product: PanelProduct,
    pub palette: String,
    pub basemap: Basemap,
    /// SPC day for the Outlook product.
    pub spc_day: u8,
    /// Custom panel title; empty = automatic.
    pub title: String,
}

impl Default for PanelSpec {
    fn default() -> Self {
        Self {
            product: PanelProduct::Radar,
            palette: "4".into(),
            basemap: Basemap::BroadcastDark,
            spc_day: 1,
            title: String::new(),
        }
    }
}

pub fn default_panels() -> Vec<PanelSpec> {
    vec![
        PanelSpec {
            product: PanelProduct::RadarSatellite,
            // House colours (Modern HD / TWC), like the main map (CWXB-89).
            palette: "4".into(),
            ..PanelSpec::default()
        },
        PanelSpec {
            product: PanelProduct::Satellite,
            ..PanelSpec::default()
        },
        PanelSpec {
            product: PanelProduct::Outlook,
            basemap: Basemap::Light,
            ..PanelSpec::default()
        },
    ]
}

impl Default for MapScene {
    fn default() -> Self {
        Self::from_settings(&Settings::default())
    }
}

impl MapScene {
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            view: settings.home,
            basemap: settings.basemap,
            labels: true,
            radar: RadarLayer {
                enabled: true,
                palette: settings.radar_palette.clone(),
                opacity: settings.radar_opacity,
                smoothing: settings.radar_smoothing,
                mode: RadarMode::Loop,
                include_nowcast: settings.include_nowcast,
                ..RadarLayer::default()
            },
            satellite: SatelliteLayer::default(),
            warnings: true,
            warnings_fill: true,
            warning_patterns: false,
            spc_day: None,
            spc_layer: "categorical".into(),
            mesoscale: false,
            qpf: None,
            ero_day: None,
            rivers: false,
            centers: false,
            lightning_density: false,
            warning_towns: true,
            camera_path: Vec::new(),
            follow_storm: false,
            regions: false,
            impact_callouts: false,
            traffic: false,
            snow_band: false,
            ens_map: String::new(),
            tropical: false,
            history_time: None,
            radar_replay: None,
            site_replay: None,
            viewer_reports: false,
            eoc: false,
            outages: false,
            outage_mode: "now".into(),
            nowcast: false,
            nowcast_mode: "arrival".into(),
            urban_flood: false,
            tornado_history: false,
            tornado_since: 1950,
            tropical_layer: "cone".into(),
            fire_outlook: false,
            wildfires: false,
            drought: false,
            drought_view: "now".into(),
            quakes: false,
            quake_days: 7,
            quake_min: 2.5,
            custom_layers: Vec::new(),
            storm_tracks: false,
            town_labels: false,
            layer_fx: Default::default(),
            radar_maxzoom_cap: None,
            frame_cap: None,
            layer_order: Vec::new(),
            hidden_groups: Vec::new(),
            layer_rules: Vec::new(),
            label_style: "classic".into(),
            theme: String::new(),
            label_scale: 1.0,
            label_overrides: Vec::new(),
            highways: false,
            lightning_clusters: false,
            lightning_anim: false,
            mcs: false,
            cameras: false,
            metar_plots: false,
            damage_map: false,
            damage_hours: 48,
            flood_map: false,
            storm_cells: false,
            cell_tracks: true,
            cell_labels: true,
            cell_paths: false,
            range_rings: None,
            storm_track: None,
            annotations: Vec::new(),
            raise_style: RaiseStyle::Lift,
            raise_visible: true,
            title: "LIVE DOPPLER".into(),
            show_title: true,
            custom_title: false,
            show_legend: settings.show_legend,
            show_timestamp: settings.show_timestamp,
            show_logo: settings.show_logo_bug,
            watermark_visible: None,
            show_sponsor: false,
            sponsor_size: None,
            show_clock: settings.show_clock_bug,
            overlay_graphic: None,
            overlay_slots: [None, None],
            layout: PanelLayout::Single,
            wipe: 0.5,
            panels: default_panels(),
            precip_total: None,
            lightning: false,
            data: DataLayer::default(),
            selected_alert: None,
            warning_countdowns: true,
            alert_kinds: AlertKinds::default(),
            reports: false,
            report_hours: 6,
            report_hidden: String::new(),
            roads: false,
            roads_all: false,
            extra_tracks: Vec::new(),
            onair_offsets: Default::default(),
            counties: true,
            states: true,
            boundary_opacity: 0.6,
        }
    }

    /// The spec for comparison panel `i` (1-3), padding with defaults for older saved scenes.
    pub fn panel_spec(&self, i: usize) -> PanelSpec {
        self.panels
            .get(i.saturating_sub(1))
            .cloned()
            .or_else(|| default_panels().get(i.saturating_sub(1)).cloned())
            .unwrap_or_default()
    }

    /// The scene a panel renders: panel 0 is this scene; the others apply their product to a
    /// copy that keeps the linked camera, telestration and tools.
    pub fn panel_scene(&self, i: usize) -> MapScene {
        if i == 0 {
            return self.clone();
        }
        let spec = self.panel_spec(i);
        let mut scene = self.clone();
        scene.layout = PanelLayout::Single;
        scene.basemap = spec.basemap;
        scene.radar.palette = spec.palette.clone();
        // Data fields, MRMS totals and call-outs stay on panel 1.
        scene.radar.source = RadarSource::Composite;
        scene.data.field = None;
        scene.data.wind_particles = false;
        scene.precip_total = None;
        scene.selected_alert = None;
        match spec.product {
            PanelProduct::Radar => {
                scene.radar.enabled = true;
                scene.satellite.enabled = false;
            }
            PanelProduct::Velocity | PanelProduct::StormRelative => {
                scene.radar.enabled = true;
                scene.radar.source = RadarSource::Site;
                scene.radar.product = if spec.product == PanelProduct::Velocity {
                    SiteProduct::Velocity
                } else {
                    SiteProduct::StormRelative
                };
                scene.radar.opacity = scene.radar.opacity.max(0.9);
                scene.satellite.enabled = false;
                scene.data.field = None;
            }
            PanelProduct::RadarSatellite => {
                scene.radar.enabled = true;
                scene.satellite.enabled = true;
                // Strong enough to read under the radar on the dark base map (CWXB-89).
                scene.satellite.opacity = scene.satellite.opacity.max(0.9);
            }
            PanelProduct::Satellite => {
                scene.radar.enabled = false;
                scene.satellite.enabled = true;
                scene.satellite.opacity = scene.satellite.opacity.max(0.8);
            }
            PanelProduct::Warnings => {
                scene.radar.enabled = false;
                scene.satellite.enabled = false;
                scene.warnings = true;
                scene.warnings_fill = true;
                scene.spc_day = None;
            }
            PanelProduct::Outlook => {
                scene.radar.enabled = false;
                scene.satellite.enabled = false;
                scene.warnings = false;
                scene.spc_day = Some(spec.spc_day.clamp(1, 3));
            }
            PanelProduct::Futurecast | PanelProduct::GfsPrecipitationRate | PanelProduct::GfsTemperature | PanelProduct::GfsReflectivity | PanelProduct::HrrrReflectivity => {
                scene.radar.enabled = false;
                scene.satellite.enabled = false;
                scene.data.field = Some(DataField::Futurecast);
                scene.data.model_product = match spec.product {
                    PanelProduct::GfsPrecipitationRate => crate::futurecast::Product::PrecipitationRate,
                    PanelProduct::GfsTemperature => crate::futurecast::Product::Temperature,
                    PanelProduct::GfsReflectivity => crate::futurecast::Product::GfsReflectivity,
                    PanelProduct::HrrrReflectivity => crate::futurecast::Product::HrrrReflectivity,
                    _ => crate::futurecast::Product::Precipitation,
                };
                scene.data.hour = if self.data.field == Some(DataField::Futurecast) { self.data.hour } else { 2.0 };
            }
            PanelProduct::RadarEarlier => {
                scene.radar.enabled = true;
                scene.satellite.enabled = false;
            }
            PanelProduct::Rainfall => {
                scene.radar.enabled = false;
                scene.satellite.enabled = false;
                scene.precip_total = Some(PrecipWindow::Hours24);
            }
            PanelProduct::Temperature => {
                scene.radar.enabled = false;
                scene.satellite.enabled = false;
                scene.data.field = Some(DataField::Temperature);
                scene.data.city_plots = true;
            }
        }
        scene
    }

    /// Header text for panel `i`.
    pub fn model_products(&self) -> impl Iterator<Item = crate::futurecast::Product> + '_ {
        (0..self.layout.count()).filter_map(|i| {
            if i == 0 {
                return (self.data.field == Some(DataField::Futurecast)).then_some(self.data.model_product);
            }
            match self.panel_spec(i).product {
                PanelProduct::Futurecast => Some(crate::futurecast::Product::Precipitation),
                PanelProduct::GfsPrecipitationRate => Some(crate::futurecast::Product::PrecipitationRate),
                PanelProduct::GfsTemperature => Some(crate::futurecast::Product::Temperature),
                PanelProduct::GfsReflectivity => Some(crate::futurecast::Product::GfsReflectivity),
                PanelProduct::HrrrReflectivity => Some(crate::futurecast::Product::HrrrReflectivity),
                _ => None,
            }
        })
    }

    pub fn is_radar_replay(&self) -> bool {
        self.radar_replay.is_some() || self.site_replay.is_some() || self.history_time.is_some()
    }

    pub fn automatic_title(&self) -> bool {
        !self.custom_title && matches!(self.title.as_str(), "LIVE DOPPLER" | "GFS FUTURECAST")
    }

    pub fn banner_text(&self) -> std::borrow::Cow<'_, str> {
        if !self.automatic_title() {
            return self.title.as_str().into();
        }
        match self.data.field {
            Some(DataField::Futurecast) => self.data.model_product.broadcast_title().into(),
            Some(field) => field.broadcast_title().into(),
            None if self.site_replay.is_some() && self.radar.enabled => "STORM-RELATIVE VELOCITY REPLAY".into(),
            None if self.is_radar_replay() && self.radar.enabled => "RADAR REPLAY".into(),
            None if self.precip_total.is_some() => "RADAR RAINFALL TOTALS".into(),
            None if self.radar.enabled && self.radar.source == RadarSource::Site && self.radar.product.is_velocity() => "Radar Winds".into(),
            None if self.radar.enabled && self.radar.source == RadarSource::Site && self.radar.product == SiteProduct::EchoTops => "Storm Heights".into(),
            None if self.radar.enabled => "LIVE DOPPLER".into(),
            None if self.satellite.enabled => self.satellite.channel.label().into(),
            None if self.spc_day.is_some() || self.ero_day.is_some() || self.fire_outlook => self.outlook_title().into(),
            None => "".into(),
        }
    }

    /// On-air title for the outlook map: "SEVERE WEATHER OUTLOOK · TORNADO · TOMORROW".
    pub fn outlook_title(&self) -> String {
        let day_word = |d: u8| match d {
            1 => "TODAY".to_string(),
            2 => "TOMORROW".to_string(),
            d => (chrono::Local::now().date_naive() + chrono::Duration::days(i64::from(d) - 1)).format("%A").to_string().to_uppercase(),
        };
        if let Some(day) = self.spc_day {
            let what = match self.spc_layer.as_str() {
                "tornado" => " · TORNADO",
                "wind" => " · WIND",
                "hail" => " · HAIL",
                "significant" => " · SIGNIFICANT SEVERE",
                "severe" => " · SEVERE CHANCE",
                _ => "",
            };
            return format!("SEVERE WEATHER OUTLOOK{what} · {}", day_word(day));
        }
        if let Some(day) = self.ero_day {
            return format!("FLOODING RAIN OUTLOOK · {}", day_word(day));
        }
        "FIRE WEATHER OUTLOOK · TODAY".to_string()
    }

    pub const OVERLAY_POSITIONS: [&'static str; 3] = ["overlay", "overlay-crawl", "overlay-sponsor"];

    pub fn shows_watermark(&self) -> bool {
        self.watermark_visible.unwrap_or(self.show_logo)
    }

    pub fn watermark_offset(&self) -> (f32, f32) {
        self.onair_offsets.get("watermark").or_else(|| {
            self.watermark_visible.is_none().then(|| self.onair_offsets.get("logo")).flatten()
        }).copied().unwrap_or_default()
    }

    pub fn sponsor_rect(&self, width: f32) -> (f32, f32, f32, f32) {
        let (dx, dy) = self.onair_offsets.get("sponsor").copied().unwrap_or_default();
        let scale = f32::from(self.sponsor_size.unwrap_or(100).clamp(75, 125)) / 100.0;
        let badge_width = 440.0 * scale;
        (width - 40.0 - badge_width + dx, 242.0 + dy, badge_width, 112.0 * scale)
    }

    pub fn panel_title(&self, i: usize) -> String {
        if i == 0 {
            let mut parts = Vec::new();
            if self.radar.enabled && self.radar.source == RadarSource::Site {
                let site = self.radar.resolved_site(self.view.lat, self.view.lon);
                parts.push(format!("K{site} {}", self.radar.product.label()));
            } else if self.radar.enabled {
                parts.push(format!("RADAR • {}", crate::data::palette_label(&self.radar.palette)));
            }
            if self.satellite.enabled {
                parts.push(self.satellite.channel.label().to_string());
            }
            if let Some(field) = self.data.field {
                if field == DataField::Futurecast {
                    parts.push(format!("{} {}", self.data.model_product.source().label(), self.data.model_product.broadcast_title()));
                } else {
                    parts.push(field.broadcast_title().to_string());
                }
            }
            // An outlook on the main map names panel 1 (it read a bare "MAP"; CWXB-89).
            if parts.is_empty() && (self.spc_day.is_some() || self.ero_day.is_some() || self.fire_outlook) {
                parts.push(self.outlook_title());
            }
            if parts.is_empty() {
                parts.push(if self.warnings { "WARNINGS".into() } else { "LOCAL MAP".into() });
            }
            return parts.join(" + ").to_uppercase();
        }
        let spec = self.panel_spec(i);
        if !spec.title.trim().is_empty() {
            return spec.title.to_uppercase();
        }
        match spec.product {
            PanelProduct::Radar => format!("RADAR • {}", crate::data::palette_label(&spec.palette)).to_uppercase(),
            PanelProduct::RadarSatellite => "RADAR + IR SATELLITE".into(),
            PanelProduct::Velocity | PanelProduct::StormRelative => {
                let site = self.radar.resolved_site(self.view.lat, self.view.lon);
                let what = if spec.product == PanelProduct::Velocity { "VELOCITY" } else { "STORM-RELATIVE VELOCITY" };
                format!("K{site} {what}")
            }
            PanelProduct::Satellite => "IR SATELLITE".into(),
            PanelProduct::Warnings => "NWS WARNINGS & WATCHES".into(),
            PanelProduct::Outlook => format!("SPC DAY {} OUTLOOK", spec.spc_day.clamp(1, 3)),
            PanelProduct::Futurecast => "GFS PRECIPITATION".into(),
            PanelProduct::GfsPrecipitationRate => "GFS PRECIPITATION RATE".into(),
            PanelProduct::GfsTemperature => "GFS TEMPERATURE".into(),
            PanelProduct::GfsReflectivity => "GFS SIMULATED RADAR".into(),
            PanelProduct::HrrrReflectivity => "HRRR SIMULATED RADAR".into(),
            PanelProduct::RadarEarlier => "RADAR • EARLIER".into(),
            PanelProduct::Rainfall => "RAINFALL • LAST 24 HOURS".into(),
            PanelProduct::Temperature => "TEMPERATURES".into(),
        }
    }
}

#[cfg(test)]
mod banner_tests {
    use super::*;

    #[test]
    fn old_default_banner_follows_the_selected_data_field() {
        let mut scene = MapScene::default();
        assert_eq!(scene.banner_text(), "LIVE DOPPLER");
        scene.data.field = Some(DataField::Temperature);
        assert_eq!(scene.banner_text(), "Temperature Outlook");
        scene.data.field = Some(DataField::QVector);
        assert_eq!(scene.banner_text(), "Areas of rising air");
        scene.data.field = Some(DataField::Futurecast);
        assert_eq!(scene.banner_text(), "Rain, Mix and Snow Forecast");
        scene.data.model_product = crate::futurecast::Product::Temperature;
        assert_eq!(scene.banner_text(), "Temperature Outlook");
    }

    #[test]
    fn a_custom_title_can_match_the_legacy_default() {
        let mut scene = MapScene::default();
        scene.custom_title = true;
        scene.data.field = Some(DataField::Temperature);
        assert_eq!(scene.banner_text(), "LIVE DOPPLER");
    }
}

/// Reference places for storm-track ETAs and the map's place markers. Southern Indiana /
/// Ohio Valley first (the station's market), then regional hubs and major cities.
pub const PLACES: &[(&str, f64, f64)] = &[
    ("Jasper", 38.3914, -86.9311),
    ("Huntingburg", 38.2989, -86.9550),
    ("Ferdinand", 38.2242, -86.8622),
    ("Holland", 38.2456, -87.0361),
    ("Birdseye", 38.3150, -86.6950),
    ("Ireland", 38.4142, -87.0011),
    ("Dale", 38.1690, -86.9900),
    ("Santa Claus", 38.1200, -86.9142),
    ("Celestine", 38.3836, -86.7953),
    ("St. Anthony", 38.3214, -86.8317),
    ("Loogootee", 38.6770, -86.9139),
    ("Washington", 38.6592, -87.1728),
    ("Petersburg", 38.4917, -87.2786),
    ("Princeton", 38.3553, -87.5675),
    ("Vincennes", 38.6773, -87.5286),
    ("Oakland City", 38.3387, -87.3450),
    ("Boonville", 38.0492, -87.2742),
    ("Evansville", 37.9716, -87.5711),
    ("Newburgh", 37.9445, -87.4053),
    ("Rockport", 37.8831, -87.0494),
    ("Tell City", 37.9514, -86.7678),
    ("Cannelton", 37.9112, -86.7444),
    ("English", 38.3345, -86.4642),
    ("Paoli", 38.5559, -86.4683),
    ("French Lick", 38.5489, -86.6200),
    ("Shoals", 38.6667, -86.7911),
    ("Bedford", 38.8611, -86.4872),
    ("Mitchell", 38.7328, -86.4736),
    ("Salem", 38.6056, -86.1011),
    ("Corydon", 38.2120, -86.1219),
    ("New Albany", 38.2856, -85.8241),
    ("Jeffersonville", 38.2776, -85.7372),
    ("Bloomington", 39.1653, -86.5264),
    ("Bloomfield", 39.0270, -86.9375),
    ("Linton", 39.0348, -87.1659),
    ("Sullivan", 39.0953, -87.4059),
    ("Terre Haute", 39.4667, -87.4139),
    ("Columbus", 39.2014, -85.9214),
    ("Seymour", 38.9592, -85.8903),
    ("Madison", 38.7359, -85.3799),
    ("Indianapolis", 39.7684, -86.1581),
    ("Louisville", 38.2527, -85.7585),
    ("Owensboro", 37.7719, -87.1112),
    ("Henderson", 37.8362, -87.5900),
    ("Hawesville", 37.9001, -86.7550),
    ("Hardinsburg", 37.7801, -86.4605),
    ("Brandenburg", 37.9987, -86.1694),
    ("Elizabethtown", 37.6940, -85.8591),
    ("Madisonville", 37.3281, -87.4989),
    ("Bowling Green", 36.9685, -86.4808),
    ("Paducah", 37.0834, -88.6000),
    ("Mount Vernon IL", 38.3173, -88.9031),
    ("Mt. Carmel", 38.4109, -87.7614),
    ("Carmi", 38.0909, -88.1587),
    ("Olney", 38.7309, -88.0853),
    ("Lawrenceville", 38.7292, -87.6817),
    ("Robinson", 39.0053, -87.7392),
    ("Effingham", 39.1200, -88.5434),
    ("Carbondale", 37.7273, -89.2168),
    ("Cincinnati", 39.1031, -84.5120),
    ("Lexington", 38.0406, -84.5037),
    ("Nashville", 36.1627, -86.7816),
    ("St. Louis", 38.6270, -90.1994),
    ("Chicago", 41.8781, -87.6298),
    ("Champaign", 40.1164, -88.2434),
    ("Springfield IL", 39.7817, -89.6501),
    ("Dayton", 39.7589, -84.1916),
    ("Columbus OH", 39.9612, -82.9988),
    ("Memphis", 35.1495, -90.0490),
    ("Kansas City", 39.0997, -94.5786),
    ("Detroit", 42.3314, -83.0458),
    ("Atlanta", 33.7490, -84.3880),
    ("Dallas", 32.7767, -96.7970),
    ("Oklahoma City", 35.4676, -97.5164),
    ("Denver", 39.7392, -104.9903),
    ("Minneapolis", 44.9778, -93.2650),
    ("New Orleans", 29.9511, -90.0715),
    ("Birmingham", 33.5186, -86.8104),
    ("Pittsburgh", 40.4406, -79.9959),
    ("Omaha", 41.2565, -95.9345),
];
