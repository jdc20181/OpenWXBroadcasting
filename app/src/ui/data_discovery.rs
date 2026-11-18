//! Data Discovery: every data product, grouped by section. Map layers go onto the live map;
//! graphics open in Graphics Studio; radar palettes are a compact strip.

use gpui_kit::{
    component::input::{Input, InputEvent, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::{AppState, ToastKind, Workspace},
    config::Basemap,
    gfx::templates,
    map::scene::{
        default_panels, DataField, PanelLayout, PanelProduct, PrecipWindow, RadarMode, RadarSource, RangeRings, SatChannel,
        SiteProduct, StormTrack,
    },
    ui::{c, ca, chip, col, muted, row, BG, BORDER, CYAN, DIM, GREEN, MUTED, PANEL, RED, TEXT},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Category {
    All,
    Radar,
    Satellite,
    Severe,
    Alerts,
    Forecast,
    Tools,
}

impl Category {
    const ALL: [Category; 7] = [
        Category::All,
        Category::Radar,
        Category::Satellite,
        Category::Severe,
        Category::Alerts,
        Category::Forecast,
        Category::Tools,
    ];

    fn label(self) -> &'static str {
        match self {
            Category::All => "Everything",
            Category::Radar => "Radar",
            Category::Satellite => "Satellite",
            Category::Severe => "Severe",
            Category::Alerts => "Alerts",
            Category::Forecast => "Forecast",
            Category::Tools => "Tools",
        }
    }
}

#[derive(Clone, PartialEq)]
enum Action {
    Radar(&'static str),
    Nowcast,
    Satellite,
    Warnings,
    Cells,
    Spc(u8),
    Template(&'static str),
    Rings,
    Track,
    Basemap(Basemap),
    Site(SiteProduct),
    Sat(SatChannel),
    Mrms(PrecipWindow),
    Data(DataField),
    Lightning,
    WindFlow,
    VelocityTwoUp,
    /// A product drawn straight from NOAA model files (GFS, HRRR, NBM).
    Noaa(crate::futurecast::Product),
    /// Airport observation plots (METAR).
    Metar,
}

struct Product {
    name: &'static str,
    category: Category,
    description: &'static str,
    tags: &'static str,
    colors: (u32, u32),
    feed: Feed,
    is_new: bool,
    action: Action,
}

#[derive(Clone, Copy)]
enum Feed {
    Radar,
    Overview,
    Alerts,
    Cells,
    Spc,
    None,
}

fn products() -> Vec<Product> {
    use Category::*;
    vec![
        Product { name: "Live Doppler • Modern HD", category: Radar, description: "LibreWXR composite reflectivity, broadcast palette.", tags: "radar reflectivity doppler", colors: (0x0B3D1A, 0xD3100C), feed: Feed::Radar, is_new: false, action: Action::Radar("4") },
        Product { name: "Classic Radar", category: Radar, description: "Original RainViewer-style colors.", tags: "radar classic", colors: (0x1B5E20, 0x6153C1), feed: Feed::Radar, is_new: false, action: Action::Radar("1") },
        Product { name: "NEXRAD Level III Style", category: Radar, description: "National Weather Service color table.", tags: "radar nexrad nws", colors: (0x009CF7, 0xFE00FE), feed: Feed::Radar, is_new: false, action: Action::Radar("6") },
        Product { name: "TITAN", category: Radar, description: "High-contrast radar scope look.", tags: "radar titan", colors: (0x6E0DC6, 0xFAC431), feed: Feed::Radar, is_new: false, action: Action::Radar("3") },
        Product { name: "Universal Blue", category: Radar, description: "Blue-to-red dynamic scale.", tags: "radar blue", colors: (0x0077AA, 0xEE0000), feed: Feed::Radar, is_new: false, action: Action::Radar("2") },
        Product { name: "Meteored", category: Radar, description: "Smooth gradient overlay.", tags: "radar meteored", colors: (0x1790F9, 0xE90000), feed: Feed::Radar, is_new: false, action: Action::Radar("5") },
        Product { name: "Rainbow", category: Radar, description: "Classic multi-color palette.", tags: "radar rainbow", colors: (0x00D319, 0xFF33FF), feed: Feed::Radar, is_new: false, action: Action::Radar("7") },
        Product { name: "Dark Sky", category: Radar, description: "Sleek modern styling.", tags: "radar dark sky", colors: (0x005EB6, 0xFFFD05), feed: Feed::Radar, is_new: false, action: Action::Radar("8") },
        Product { name: "Raw Reflectivity", category: Radar, description: "Grayscale dBZ for analysis.", tags: "radar raw grayscale dbz", colors: (0x4A4A4A, 0xD7D7D7), feed: Feed::Radar, is_new: false, action: Action::Radar("255") },
        Product { name: "Future Radar (Nowcast)", category: Radar, description: "Short-term projected radar frames after the latest scan.", tags: "future nowcast forecast radar", colors: (0x3A2A05, 0xF5A524), feed: Feed::Radar, is_new: false, action: Action::Nowcast },
        Product { name: "Base Velocity (single site)", category: Radar, description: "NEXRAD Level III velocity from the nearest radar: green toward, red away. Find rotation.", tags: "velocity rotation couplet nexrad site tornado", colors: (0x0B5D1E, 0xC81E1E), feed: Feed::None, is_new: false, action: Action::Site(SiteProduct::Velocity) },
        Product { name: "Storm-Relative Velocity", category: Radar, description: "Velocity with storm motion removed - the classic tornado-hunting view.", tags: "srv storm relative velocity rotation tornado", colors: (0x0B5D1E, 0xFF3B3B), feed: Feed::None, is_new: false, action: Action::Site(SiteProduct::StormRelative) },
        Product { name: "Super-Res Reflectivity (single site)", category: Radar, description: "Highest-detail lowest-tilt reflectivity from one NEXRAD site.", tags: "super res reflectivity nexrad site hook", colors: (0x0B3D1A, 0xFF00FF), feed: Feed::None, is_new: false, action: Action::Site(SiteProduct::SuperRes) },
        Product { name: "Echo Tops", category: Radar, description: "How tall storms are, in thousands of feet.", tags: "echo tops height storm", colors: (0x153060, 0xE040D0), feed: Feed::None, is_new: false, action: Action::Site(SiteProduct::EchoTops) },
        Product { name: "Reflectivity / Velocity 2-Up", category: Radar, description: "Composite radar beside single-site velocity, cameras linked.", tags: "2up dual split velocity reflectivity compare", colors: (0x0B3D1A, 0xC81E1E), feed: Feed::Radar, is_new: false, action: Action::VelocityTwoUp },
        Product { name: "MRMS Rainfall 24 hr", category: Radar, description: "Radar + gauge estimated rainfall over the last day.", tags: "rainfall totals mrms precipitation 24 hour", colors: (0x0C3A1A, 0x3DD68C), feed: Feed::None, is_new: false, action: Action::Mrms(PrecipWindow::Hours24) },
        Product { name: "MRMS Rainfall 72 hr", category: Radar, description: "Three-day rainfall estimate for flooding stories.", tags: "rainfall totals mrms flood 72 hour", colors: (0x0C3A1A, 0xF5A524), feed: Feed::None, is_new: false, action: Action::Mrms(PrecipWindow::Hours72) },
        Product { name: "Lightning", category: Severe, description: "Strikes in the last 15 minutes, newest in white.", tags: "lightning strikes bolts", colors: (0x2A2405, 0xFFE600), feed: Feed::None, is_new: false, action: Action::Lightning },
        Product { name: "Futurecast", category: Forecast, description: "NOAA GFS precipitation by type (rain, mix, snow) to the last frame of the run. Change the model in Forecast > Model.", tags: "futurecast future model hrrr precipitation snow rain mix", colors: (0x10301C, 0x5EC8FF), feed: Feed::None, is_new: false, action: Action::Data(DataField::Futurecast) },
        Product { name: "Temperatures", category: Forecast, description: "Color-shaded temperatures with city plots, now and forecast.", tags: "temperature temps map", colors: (0x3C46C8, 0xE14628), feed: Feed::None, is_new: false, action: Action::Data(DataField::Temperature) },
        Product { name: "Feels Like", category: Forecast, description: "Heat index / wind chill shading with city plots.", tags: "feels like heat index wind chill apparent", colors: (0x3C46C8, 0xAA1428), feed: Feed::None, is_new: false, action: Action::Data(DataField::FeelsLike) },
        Product { name: "Dew Points", category: Forecast, description: "Moisture map: 60s muggy, 70s oppressive.", tags: "dew point moisture humidity", colors: (0x968C46, 0x2878C8), feed: Feed::None, is_new: false, action: Action::Data(DataField::DewPoint) },
        Product { name: "Winds", category: Forecast, description: "Wind speed shading with direction arrows.", tags: "wind speed direction", colors: (0x283C6E, 0xE6DC3C), feed: Feed::None, is_new: false, action: Action::Data(DataField::WindSpeed) },
        Product { name: "Wind Gusts", category: Forecast, description: "Peak gust potential.", tags: "wind gusts", colors: (0x283C6E, 0xE13C28), feed: Feed::None, is_new: false, action: Action::Data(DataField::WindGust) },
        Product { name: "Animated Wind Flow", category: Forecast, description: "Flowing particles colored by speed.", tags: "wind flow particles streamlines animation", colors: (0x0B1F3A, 0x96F0C8), feed: Feed::None, is_new: false, action: Action::WindFlow },
        Product { name: "Pressure & Isobars", category: Forecast, description: "Isobars every 4 mb with H and L centres.", tags: "pressure isobars high low", colors: (0x8C288C, 0x283296), feed: Feed::None, is_new: false, action: Action::Data(DataField::Pressure) },
        Product { name: "Snowfall Forecast", category: Forecast, description: "Model snowfall to the chosen hour, as the model reports it (about 7:1). See Snow 10:1 and Kuchera for other ratios.", tags: "snow snowfall totals accumulation winter", colors: (0x8CBEF5, 0xBE46BE), feed: Feed::None, is_new: false, action: Action::Data(DataField::SnowTotal) },
        Product { name: "Rainfall Forecast", category: Forecast, description: "Model rain accumulation to the chosen hour.", tags: "rain rainfall totals accumulation forecast", colors: (0x3CB450, 0xDC2828), feed: Feed::None, is_new: false, action: Action::Data(DataField::RainTotal) },
        Product { name: "Forecast Highs", category: Forecast, description: "Highest temperature in the next 24 hours, with town values.", tags: "high temperature forecast max highs", colors: (0xE6A028, 0xE14628), feed: Feed::None, is_new: false, action: Action::Data(DataField::HighTemp) },
        Product { name: "Forecast Lows", category: Forecast, description: "Lowest temperature in the next 24 hours, with town values.", tags: "low temperature forecast min lows frost freeze", colors: (0x3C46C8, 0x28A0DC), feed: Feed::None, is_new: false, action: Action::Data(DataField::LowTemp) },
        Product { name: "24-Hour Temperature Change", category: Forecast, description: "How much warmer or colder this time tomorrow.", tags: "temperature change warmer colder trend 24 hour", colors: (0x2846C8, 0xD22828), feed: Feed::None, is_new: false, action: Action::Data(DataField::TempChange) },
        Product { name: "Peak Gusts", category: Forecast, description: "Strongest wind gust expected in the next 24 hours.", tags: "wind gust maximum peak damaging", colors: (0x283C6E, 0xE13C28), feed: Feed::None, is_new: false, action: Action::Data(DataField::MaxGust) },
        Product { name: "Ice Forecast", category: Forecast, description: "Freezing rain and wintry mix in the next 48 hours.", tags: "ice freezing rain sleet accrual winter", colors: (0xF0BEE6, 0x961E8C), feed: Feed::None, is_new: false, action: Action::Data(DataField::IceTotal) },
        Product { name: "Surface Convergence", category: Forecast, description: "Where surface winds pile together (storms often start here); per hour x 10^-5.", tags: "mesoscale analysis surface convergence severe ingredients", colors: (0x2A3A6E, 0xE0A040), feed: Feed::None, is_new: false, action: Action::Data(DataField::Convergence) },
        Product { name: "Moisture Convergence", category: Forecast, description: "Convergence weighted by moisture: where storms can feed.", tags: "mesoscale analysis moisture convergence severe ingredients", colors: (0x2A3A6E, 0xE0A040), feed: Feed::None, is_new: false, action: Action::Data(DataField::MoistureConvergence) },
        Product { name: "Theta-E", category: Forecast, description: "Equivalent potential temperature (heat + moisture), K.", tags: "mesoscale analysis theta-e severe ingredients", colors: (0x2A3A6E, 0xE0A040), feed: Feed::None, is_new: false, action: Action::Data(DataField::ThetaE) },
        Product { name: "Theta-E Advection", category: Forecast, description: "Wind carrying warm, moist air in (positive) or out.", tags: "mesoscale analysis theta-e advection severe ingredients", colors: (0x2A3A6E, 0xE0A040), feed: Feed::None, is_new: false, action: Action::Data(DataField::ThetaEAdvection) },
        Product { name: "Pressure Perturbation", category: Forecast, description: "Pressure minus the regional average: meso highs and lows, mb.", tags: "mesoscale analysis pressure perturbation severe ingredients", colors: (0x2A3A6E, 0xE0A040), feed: Feed::None, is_new: false, action: Action::Data(DataField::PressureAnomaly) },
        Product { name: "Dryline", category: Forecast, description: "Sharp dew point change across short distances: the dryline, °F per 10 mi.", tags: "mesoscale analysis dryline severe ingredients", colors: (0x2A3A6E, 0xE0A040), feed: Feed::None, is_new: false, action: Action::Data(DataField::Dryline) },
        Product { name: "Boundaries", category: Forecast, description: "Sharp temperature change: fronts and outflow boundaries, °F per 10 mi.", tags: "mesoscale analysis boundaries severe ingredients", colors: (0x2A3A6E, 0xE0A040), feed: Feed::None, is_new: false, action: Action::Data(DataField::Boundaries) },
        Product { name: "Differential Heating", category: Forecast, description: "Cloudy vs sunny edges where boundaries form, % per 10 mi.", tags: "mesoscale analysis differential heating severe ingredients", colors: (0x2A3A6E, 0xE0A040), feed: Feed::None, is_new: false, action: Action::Data(DataField::DiffHeating) },
        Product { name: "Chance of 1\" Snow", category: Forecast, description: "Chance of an inch or more of snow in the next 24 hours from the model, widening with lead time.", tags: "snow probability snowfall chance winter", colors: (0x2A4E8C, 0xC8E1FA), feed: Feed::None, is_new: false, action: Action::Data(DataField::SnowChance) },
        Product { name: "Winter Road Hazard", category: Forecast, description: "Wet or frosty, icy, or snow-covered roads from temperature, precipitation and recent rain.", tags: "roads ice black ice travel winter hazard snow", colors: (0x5A2A3A, 0xE6785A), feed: Feed::None, is_new: false, action: Action::Data(DataField::RoadHazard) },
        Product { name: "Fog Risk", category: Forecast, description: "Where fog is likely: small dew point spread, high humidity and light wind.", tags: "fog visibility travel morning", colors: (0x606770, 0xF2F3F5), feed: Feed::None, is_new: false, action: Action::Data(DataField::FogRisk) },
        Product { name: "Cold Pools", category: Forecast, description: "Temperature drop over the last 2 hours under rain: storm cold pools, °F.", tags: "mesoscale analysis cold pools severe ingredients", colors: (0x2A3A6E, 0xE0A040), feed: Feed::None, is_new: false, action: Action::Data(DataField::ColdPool) },
        Product { name: "Snow (10:1)", category: Forecast, description: "Snow from the model's liquid equivalent at a fixed 10:1 ratio. Works with GFS, NAM, HRRR, NBM, ECMWF, ICON and GEM.", tags: "snow 10:1 ten to one ratio snowfall totals winter gfs nam hrrr ecmwf model", colors: (0x8CBEF5, 0xBE46BE), feed: Feed::None, is_new: true, action: Action::Data(DataField::Snow10) },
        Product { name: "Snow (Kuchera)", category: Forecast, description: "Snow with the Kuchera ratio from the warmest level aloft: fluffier in cold air, wetter near freezing.", tags: "snow kuchera ratio snowfall totals winter model", colors: (0x8CBEF5, 0x7A2BBE), feed: Feed::None, is_new: true, action: Action::Data(DataField::SnowKuchera) },
        Product { name: "Precipitation Type", category: Forecast, description: "Rain, mix or snow from wet-bulb temperature and the warm layer aloft.", tags: "precipitation type rain snow mix sleet freezing ptype winter", colors: (0x3CB450, 0x8CBEF5), feed: Feed::None, is_new: true, action: Action::Data(DataField::PrecipType) },
        Product { name: "Snow-to-Liquid Ratio", category: Forecast, description: "How fluffy the snow will be (Kuchera-style).", tags: "snow ratio density fluffy wet winter", colors: (0x5A6E8C, 0xE6F0FF), feed: Feed::None, is_new: true, action: Action::Data(DataField::SnowRatio) },
        Product { name: "Total Precipitation (GFS)", category: Forecast, description: "NOAA GFS liquid-equivalent precipitation from now to each hour, from the run-total APCP field.", tags: "total precipitation qpf rainfall accumulation gfs noaa apcp", colors: (0x3CB450, 0xDC3CDC), feed: Feed::None, is_new: true, action: Action::Noaa(crate::futurecast::Product::TotalPrecip) },
        Product { name: "Precipitation Rate (GFS)", category: Forecast, description: "Instantaneous GFS precipitation rate in inches per hour.", tags: "precipitation rate intensity gfs noaa", colors: (0x7BD2EB, 0xF25892), feed: Feed::None, is_new: true, action: Action::Noaa(crate::futurecast::Product::PrecipitationRate) },
        Product { name: "Simulated Radar (HRRR)", category: Forecast, description: "HRRR composite reflectivity: what radar may look like, hour by hour for 48 hours.", tags: "simulated radar reflectivity hrrr future radar model", colors: (0x23A033, 0xCB5AE0), feed: Feed::None, is_new: true, action: Action::Noaa(crate::futurecast::Product::HrrrReflectivity) },
        Product { name: "Snowfall (National Blend)", category: Forecast, description: "NOAA National Blend 6-hour snowfall, with low-end, high-end and chance views.", tags: "nbm national blend snowfall snow winter probability", colors: (0x8CBEF5, 0x4A6ED0), feed: Feed::None, is_new: true, action: Action::Noaa(crate::futurecast::Product::Snowfall) },
        Product { name: "Ice (National Blend)", category: Forecast, description: "NOAA National Blend 6-hour flat ice accumulation.", tags: "nbm national blend ice freezing rain winter", colors: (0xF0BEE6, 0x961E8C), feed: Feed::None, is_new: true, action: Action::Noaa(crate::futurecast::Product::Ice) },
        Product { name: "Dew Point (GFS)", category: Forecast, description: "NOAA GFS 2 m dew point.", tags: "dew point moisture gfs noaa surface", colors: (0x968C46, 0x2878C8), feed: Feed::None, is_new: true, action: Action::Noaa(crate::futurecast::Product::DewPoint) },
        Product { name: "Wind (GFS)", category: Forecast, description: "NOAA GFS 10 m wind speed. Gusts and sea-level pressure are under Forecast > Wind and pressure.", tags: "wind speed gfs noaa surface gust pressure", colors: (0x283C6E, 0xE6DC3C), feed: Feed::None, is_new: true, action: Action::Noaa(crate::futurecast::Product::Wind) },
        Product { name: "Airport Observations", category: Forecast, description: "METAR station plots: temperature, dew point, wind and flight category, updated every 5 minutes.", tags: "metar airport observations station plots asos aviation current", colors: (0x1E3A1E, 0x3CC85A), feed: Feed::None, is_new: true, action: Action::Metar },
        Product { name: "Instability (CAPE)", category: Severe, description: "Thunderstorm energy, J/kg. Loads the upper-air data automatically.", tags: "cape instability severe thunderstorm energy ingredients", colors: (0x283C6E, 0xE13C28), feed: Feed::None, is_new: true, action: Action::Data(DataField::Cape) },
        Product { name: "Significant Tornado Parameter", category: Severe, description: "Estimated STP from instability, cloud base and shear (derived).", tags: "stp tornado parameter severe ingredients", colors: (0x3A1A0A, 0xE040D0), feed: Feed::None, is_new: true, action: Action::Data(DataField::Stp) },
        Product { name: "Supercell Composite", category: Severe, description: "Estimated SCP from instability, helicity and shear (derived).", tags: "scp supercell composite severe rotating storms", colors: (0x3A1A0A, 0xE08040), feed: Feed::None, is_new: true, action: Action::Data(DataField::Scp) },
        Product { name: "Air Quality Index", category: Forecast, description: "US AQI forecast from Open-Meteo air quality.", tags: "air quality aqi smoke ozone pollution", colors: (0x3CB450, 0x8C1E50), feed: Feed::None, is_new: true, action: Action::Data(DataField::AirQuality) },
        Product { name: "Wildfire Smoke", category: Forecast, description: "Fine particles (PM2.5), mostly wildfire smoke.", tags: "smoke wildfire pm2.5 haze air quality", colors: (0x6E6E6E, 0xC86428), feed: Feed::None, is_new: true, action: Action::Data(DataField::Smoke) },
        Product { name: "Fire Danger", category: Forecast, description: "Fosberg fire weather index from temperature, humidity and wind.", tags: "fire danger weather fosberg wildfire red flag", colors: (0x3C3C1E, 0xE13C28), feed: Feed::None, is_new: true, action: Action::Data(DataField::FireDanger) },
        Product { name: "Flash Flood Potential", category: Forecast, description: "0-10 index from wet soil, heavy rain rates and towns.", tags: "flash flood potential flooding rain soil", colors: (0x1E3C5A, 0x2878E6), feed: Feed::None, is_new: true, action: Action::Data(DataField::FloodPotential) },
        Product { name: "Cloud Cover", category: Satellite, description: "Model cloud cover forecast.", tags: "cloud cover sky forecast", colors: (0x141E32, 0xEBEEF5), feed: Feed::None, is_new: false, action: Action::Data(DataField::CloudCover) },
        Product { name: "GOES Visible", category: Satellite, description: "0.5 km daytime visible imagery from GOES-East.", tags: "goes visible satellite clouds", colors: (0x202830, 0xF0F0F0), feed: Feed::None, is_new: false, action: Action::Sat(SatChannel::Visible) },
        Product { name: "Water Vapor", category: Satellite, description: "Mid-level moisture: jet streams, troughs and dry slots.", tags: "water vapor satellite jet trough", colors: (0x101830, 0x50A0E0), feed: Feed::None, is_new: false, action: Action::Sat(SatChannel::WaterVapor) },
        Product { name: "GOES Clean IR", category: Satellite, description: "Band 13 longwave infrared cloud tops.", tags: "goes infrared ir cloud tops", colors: (0x10151F, 0xE0E0E0), feed: Feed::None, is_new: false, action: Action::Sat(SatChannel::CleanIr) },
        Product { name: "Infrared Satellite", category: Satellite, description: "Latest IR satellite under the radar.", tags: "satellite infrared clouds ir", colors: (0x10151F, 0xB7C2D0), feed: Feed::Radar, is_new: false, action: Action::Satellite },
        Product { name: "Satellite Basemap", category: Satellite, description: "Esri World Imagery as the base layer.", tags: "satellite imagery basemap", colors: (0x10301C, 0x506C3A), feed: Feed::None, is_new: false, action: Action::Basemap(Basemap::Satellite) },
        Product { name: "Shaded Terrain", category: Satellite, description: "Relief basemap for terrain stories.", tags: "terrain relief basemap", colors: (0x6C7A5C, 0xCFD6CF), feed: Feed::None, is_new: false, action: Action::Basemap(Basemap::Terrain) },
        Product { name: "NWS Warnings & Watches", category: Alerts, description: "Active polygons in NWS hazard colors.", tags: "warnings watches polygons nws alerts tornado severe flood", colors: (0x5C0E16, 0xFF0000), feed: Feed::Alerts, is_new: false, action: Action::Warnings },
        Product { name: "Weather Alert Card", category: Alerts, description: "Full-screen card for the top local alert.", tags: "alert card warning graphic", colors: (0x22070A, 0xE5383B), feed: Feed::Alerts, is_new: false, action: Action::Template("alert") },
        Product { name: "Alert Crawl", category: Alerts, description: "Scrolling ticker of local alerts.", tags: "crawl ticker alerts", colors: (0x111111, 0xE5383B), feed: Feed::Alerts, is_new: false, action: Action::Template("alert_crawl") },
        Product { name: "Storm Cell Tracking", category: Severe, description: "Detected cells with intensity, hail and 30/60-min motion.", tags: "storm cells tracking hail lightning", colors: (0x3A1A05, 0xFF8A00), feed: Feed::Cells, is_new: false, action: Action::Cells },
        Product { name: "SPC Day 1 Outlook", category: Severe, description: "Storm Prediction Center categorical risk.", tags: "spc outlook severe risk day 1", colors: (0x1E3A1E, 0xFFE066), feed: Feed::Spc, is_new: false, action: Action::Spc(1) },
        Product { name: "SPC Day 2 Outlook", category: Severe, description: "Tomorrow's categorical risk.", tags: "spc outlook severe risk day 2", colors: (0x1E3A1E, 0xFFA366), feed: Feed::Spc, is_new: false, action: Action::Spc(2) },
        Product { name: "SPC Day 3 Outlook", category: Severe, description: "Day 3 categorical risk.", tags: "spc outlook severe risk day 3", colors: (0x1E3A1E, 0xE06666), feed: Feed::Spc, is_new: false, action: Action::Spc(3) },
        Product { name: "Severe Outlook Graphic", category: Severe, description: "Risk scale with SPC Days 1-3.", tags: "spc severe graphic", colors: (0x173A5C, 0xEE99EE), feed: Feed::Spc, is_new: false, action: Action::Template("severe_outlook") },
        Product { name: "Current Conditions", category: Forecast, description: "Observation panel from the nearest station.", tags: "current conditions observation temperature", colors: (0x0C1626, 0x2ED3F3), feed: Feed::Overview, is_new: false, action: Action::Template("current") },
        Product { name: "7-Day Forecast", category: Forecast, description: "Seven-day cards from the NWS forecast.", tags: "7 day forecast extended", colors: (0x0C1626, 0x5EC8FF), feed: Feed::Overview, is_new: false, action: Action::Template("seven_day") },
        Product { name: "3-Day Forecast", category: Forecast, description: "Big three-day outlook.", tags: "3 day forecast", colors: (0x0C1626, 0x3DD68C), feed: Feed::Overview, is_new: false, action: Action::Template("three_day") },
        Product { name: "Tonight / Tomorrow", category: Forecast, description: "Two-panel near-term forecast.", tags: "tonight tomorrow forecast", colors: (0x0C1626, 0xB48CFF), feed: Feed::Overview, is_new: false, action: Action::Template("tonight_tomorrow") },
        Product { name: "Temperature Bug", category: Forecast, description: "Corner temperature overlay.", tags: "temperature bug overlay", colors: (0x161A26, 0x2ED3F3), feed: Feed::Overview, is_new: false, action: Action::Template("temp_bug") },
        Product { name: "Range Rings", category: Tools, description: "10 / 25 / 50 mile rings around home.", tags: "range rings distance", colors: (0x0B0F18, 0x2ED3F3), feed: Feed::None, is_new: false, action: Action::Rings },
        Product { name: "Storm Track", category: Tools, description: "Cone, ETA marks and arrival times for towns in the path.", tags: "storm track eta arrival", colors: (0x2A0B0E, 0xFF3B3B), feed: Feed::None, is_new: false, action: Action::Track },
    ]
}

pub struct DataDiscovery {
    app: Entity<AppState>,
    category: Category,
    search: Entity<InputState>,
    query: String,
    _subscriptions: Vec<Subscription>,
}

impl DataDiscovery {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search by name or tags…"));
        let subscriptions = vec![
            cx.observe(&app, |_, _, cx| cx.notify()),
            cx.subscribe_in(&search, window, |this, state, e: &InputEvent, _, cx| {
                if matches!(e, InputEvent::Change) {
                    this.query = state.read(cx).value().to_lowercase();
                    cx.notify();
                }
            }),
        ];
        Self {
            app,
            category: Category::All,
            search,
            query: String::new(),
            _subscriptions: subscriptions,
        }
    }

    /// Remember the product in the recent list (newest first, ten kept).
    fn remember(&mut self, name: &str, cx: &mut Context<Self>) {
        let name = name.to_string();
        self.app.update(cx, |s, _| {
            let mut settings = s.settings.clone();
            settings.recent_products.retain(|n| *n != name);
            settings.recent_products.insert(0, name);
            settings.recent_products.truncate(10);
            s.apply_settings(settings);
        });
    }

    fn toggle_favorite(&mut self, name: &str, cx: &mut Context<Self>) {
        let name = name.to_string();
        self.app.update(cx, |s, cx| {
            let mut settings = s.settings.clone();
            if let Some(pos) = settings.favorites.iter().position(|n| *n == name) {
                settings.favorites.remove(pos);
            } else {
                settings.favorites.push(name);
            }
            s.apply_settings(settings);
            cx.notify();
        });
    }

    fn apply(&mut self, action: Action, cx: &mut Context<Self>) {
        if let Some(p) = products().into_iter().find(|p| p.action == action) {
            self.remember(p.name, cx);
        }
        self.app.update(cx, |s, cx| {
            match action {
                Action::Radar(palette) => s.update_live(|l| {
                    l.radar.enabled = true;
                    l.radar.source = RadarSource::Composite;
                    l.radar.palette = palette.to_string();
                    l.radar.mode = RadarMode::Loop;
                }),
                Action::Site(product) => s.update_live(|l| {
                    l.radar.enabled = true;
                    l.radar.source = RadarSource::Site;
                    l.radar.product = product;
                    l.data.field = None;
                }),
                Action::VelocityTwoUp => s.update_live(|l| {
                    l.radar.enabled = true;
                    l.radar.source = RadarSource::Composite;
                    l.layout = PanelLayout::Dual;
                    while l.panels.len() < 3 {
                        let next = default_panels()[l.panels.len()].clone();
                        l.panels.push(next);
                    }
                    l.panels[0].product = PanelProduct::Velocity;
                }),
                Action::Sat(channel) => s.update_live(|l| {
                    l.satellite.enabled = true;
                    l.satellite.channel = channel;
                }),
                Action::Mrms(window) => s.update_live(|l| l.precip_total = Some(window)),
                Action::Data(field) => {
                    s.update_live(|l| {
                        // A forecast map replaces the radar / satellite map (CWXB-69).
                        if l.data.field.is_none() {
                            l.radar.enabled = false;
                            l.satellite.enabled = false;
                            l.precip_total = None;
                        }
                        l.data.field = Some(field);
                        if field == DataField::Futurecast {
                            l.data.opacity = l.data.opacity.max(0.85);
                            l.title = "FUTURECAST".into();
                        }
                    });
                    s.set_data_hour(0.0);
                    if field == DataField::Futurecast {
                        s.data_playing = true;
                    }
                }
                Action::Lightning => s.update_live(|l| l.lightning = true),
                Action::Noaa(product) => {
                    s.update_live(|l| {
                        l.data.field = Some(DataField::Futurecast);
                        l.data.model_product = product;
                        l.data.grid_model = match product.source() {
                            crate::futurecast::Source::Gfs => crate::grid::GridModel::Gfs,
                            crate::futurecast::Source::Hrrr => crate::grid::GridModel::Hrrr,
                            crate::futurecast::Source::Nbm => crate::grid::GridModel::Nbm,
                        };
                        if !product.spans().contains(&l.data.loop_hours) {
                            l.data.loop_hours = product.spans().get(1).copied().unwrap_or(24);
                        }
                        l.data.opacity = l.data.opacity.max(0.85);
                        l.radar.enabled = false;
                        l.satellite.enabled = false;
                    });
                    s.set_data_hour(0.0);
                }
                Action::Metar => s.update_live(|l| l.metar_plots = true),
                Action::WindFlow => s.update_live(|l| l.data.wind_particles = true),
                Action::Nowcast => s.update_live(|l| {
                    l.radar.enabled = true;
                    l.radar.include_nowcast = true;
                    l.radar.mode = RadarMode::Loop;
                    l.title = "FUTURE RADAR".into();
                }),
                Action::Satellite => s.update_live(|l| {
                    l.satellite.enabled = true;
                    l.satellite.channel = SatChannel::Infrared;
                }),
                Action::Basemap(b) => s.update_live(|l| l.basemap = b),
                Action::Warnings => s.update_live(|l| {
                    l.warnings = true;
                    l.warnings_fill = true;
                }),
                Action::Cells => s.update_live(|l| {
                    l.storm_cells = true;
                    l.cell_paths = true;
                }),
                Action::Spc(day) => s.update_live(|l| {
                    l.spc_day = Some(day);
                    l.title = format!("SEVERE OUTLOOK • DAY {day}");
                }),
                Action::Rings => {
                    let home = s.settings.home;
                    s.update_live(|l| {
                        l.range_rings = Some(RangeRings {
                            lat: home.lat,
                            lon: home.lon,
                            ..RangeRings::default()
                        })
                    })
                }
                Action::Track => {
                    let view = s.live.view;
                    s.update_live(|l| {
                        l.storm_track = Some(StormTrack {
                            lat: view.lat,
                            lon: view.lon,
                            ..StormTrack::default()
                        })
                    })
                }
                Action::Template(key) => {
                    if let Some(doc) = templates::build(key) {
                        if let Some(saved) = s.save_graphic(doc) {
                            s.toast(ToastKind::Success, format!("Created \"{}\" - it's open in Graphics", saved.name));
                            s.open_graphic(saved.id);
                        }
                    }
                    s.workspace = Workspace::Graphics;
                    cx.notify();
                    return;
                }
            }
            s.workspace = Workspace::Map;
            cx.notify();
        });
    }
}

impl Category {
    fn blurb(self) -> &'static str {
        match self {
            Category::All => "Everything you can put on the map or on air.",
            Category::Radar => "Live composite loop, single-site NEXRAD products and rainfall totals.",
            Category::Satellite => "GOES imagery, cloud forecasts and basemaps.",
            Category::Severe => "Lightning, storm cells, SPC outlooks and severe ingredients (CAPE, STP, supercell composite).",
            Category::Alerts => "NWS warnings on the map and alert graphics.",
            Category::Forecast => "Model maps from GFS, NAM, HRRR, NBM, ECMWF, ICON and GEM (precipitation, snow 10:1 and Kuchera, temperatures, wind, air quality), airport observations and forecast graphics.",
            Category::Tools => "Storm track and range rings.",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    MapLayer,
    Graphic,
    Tool,
}

impl Action {
    fn kind(&self) -> Kind {
        match self {
            Action::Template(_) => Kind::Graphic,
            Action::Rings | Action::Track => Kind::Tool,
            _ => Kind::MapLayer,
        }
    }

    /// Whether this product is what the live map shows right now.
    fn is_on(&self, l: &crate::map::scene::MapScene) -> bool {
        match self {
            Action::Radar(p) => {
                l.radar.enabled && l.radar.source == RadarSource::Composite && l.radar.palette == *p && l.data.field.is_none()
            }
            Action::Nowcast => l.radar.enabled && l.radar.include_nowcast,
            Action::Site(p) => l.radar.enabled && l.radar.source == RadarSource::Site && l.radar.product == *p,
            Action::VelocityTwoUp => l.layout == PanelLayout::Dual,
            Action::Sat(ch) => l.satellite.enabled && l.satellite.channel == *ch,
            Action::Satellite => l.satellite.enabled && l.satellite.channel == SatChannel::Infrared,
            Action::Mrms(w) => l.precip_total == Some(*w),
            Action::Data(f) => l.data.field == Some(*f),
            Action::Noaa(p) => l.data.field == Some(DataField::Futurecast) && l.data.model_product == *p,
            Action::Metar => l.metar_plots,
            Action::Lightning => l.lightning,
            Action::WindFlow => l.data.wind_particles,
            Action::Basemap(b) => l.basemap == *b,
            Action::Warnings => l.warnings,
            Action::Cells => l.storm_cells,
            Action::Spc(d) => l.spc_day == Some(*d),
            Action::Rings => l.range_rings.is_some(),
            Action::Track => l.storm_track.is_some(),
            Action::Template(_) => false,
        }
    }
}

impl Render for DataDiscovery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let health = state.health.clone();
        let live = state.live.clone();
        let feed_ok = |feed: Feed| -> Option<bool> {
            match feed {
                Feed::Radar => health.radar.at.map(|_| health.radar.ok),
                Feed::Overview => health.overview.at.map(|_| health.overview.ok),
                Feed::Alerts => health.alerts.at.map(|_| health.alerts.ok),
                Feed::Cells => health.cells.at.map(|_| health.cells.ok),
                Feed::Spc => health.spc.at.map(|_| health.spc.ok),
                Feed::None => Some(true),
            }
        };

        let query = self.query.clone();
        let matches = |p: &Product| {
            query.is_empty()
                || p.name.to_lowercase().contains(&query)
                || p.tags.contains(&query)
                || p.description.to_lowercase().contains(&query)
        };
        let all: Vec<Product> = products().into_iter().filter(|p| matches(p)).collect();
        // Favorites, recent and context-aware suggestions (strips above the catalogue).
        let (favorites, recent) = {
            let s = self.app.read(cx);
            (s.settings.favorites.clone(), s.settings.recent_products.clone())
        };
        let suggestions: Vec<&'static str> = {
            let s = self.app.read(cx);
            let mut out: Vec<&'static str> = Vec::new();
            if !s.coverage_warnings().is_empty() {
                out.extend(["Warnings & Watches", "Storm Cells", "Lightning"]);
            }
            if let Some(g) = s.data.grid.as_deref() {
                let home = s.settings.home;
                let tf = g.time_index(0.0);
                let v = |f: DataField| g.value(f, tf, home.lat, home.lon).unwrap_or(0.0);
                if v(DataField::SnowTotal) > 0.5 || v(DataField::SnowChance) > 30.0 {
                    out.extend(["Snow Total", "Winter Road Hazard"]);
                }
                if v(DataField::IceTotal) > 0.05 {
                    out.push("Ice Forecast");
                }
                if v(DataField::HighTemp) >= 92.0 {
                    out.push("Feels Like");
                }
                if v(DataField::LowTemp) <= 15.0 {
                    out.push("Forecast Lows");
                }
                if v(DataField::RainTotal) > 1.0 {
                    out.push("Rain Total");
                }
                if v(DataField::FogRisk) > 60.0 {
                    out.push("Fog Risk");
                }
                if v(DataField::MaxGust) >= 40.0 {
                    out.push("Peak Gusts");
                }
            }
            out
        };
        let strip = |label: &'static str, names: Vec<String>, id: &'static str, cx: &mut Context<Self>| {
            let chips = names
                .iter()
                .filter_map(|n| products().into_iter().find(|p| p.name == n.as_str()))
                .enumerate()
                .map(|(i, p)| {
                    let action = p.action.clone();
                    chip((id, i), p.name, false).on_click(cx.listener(move |this, _, _, cx| this.apply(action.clone(), cx)))
                })
                .collect::<Vec<_>>();
            (!chips.is_empty()).then(|| {
                col()
                    .gap(px(6.))
                    .child(div().text_size(px(11.)).text_color(c(DIM)).font_weight(FontWeight::BOLD).child(label))
                    .child(row().flex_wrap().gap(px(6.)).children(chips))
            })
        };
        let fav_strip = strip("FAVORITES", favorites.clone(), "fav", cx);
        let recent_strip = strip("RECENT", recent, "recent", cx);
        let suggest_strip = strip("SUGGESTED RIGHT NOW", suggestions.iter().map(|s| s.to_string()).collect(), "suggest", cx);
        let count_in = |cat: Category| all.iter().filter(|p| cat == Category::All || p.category == cat).count();

        let filters = Category::ALL
            .iter()
            .enumerate()
            .map(|(i, cat)| {
                let cat = *cat;
                let active = self.category == cat;
                row()
                    .id(("cat", i))
                    .mx(px(10.))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_size(px(14.))
                    .when(active, |d| d.bg(ca(CYAN, 0.16)).text_color(c(CYAN)))
                    .when(!active, |d| d.text_color(c(TEXT)).hover(|s| s.bg(c(0x19202F))))
                    .child(div().flex_1().child(cat.label()))
                    .child(div().text_size(px(12.)).text_color(c(MUTED)).child(count_in(cat).to_string()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.category = cat;
                        cx.notify();
                    }))
            })
            .collect::<Vec<_>>();

        let sections: Vec<Category> = if self.category == Category::All {
            Category::ALL[1..].to_vec()
        } else {
            vec![self.category]
        };

        let mut body: Vec<AnyElement> = Vec::new();
        let mut card_index = 0usize;
        for cat in sections {
            let items: Vec<&Product> = all.iter().filter(|p| p.category == cat).collect();
            if items.is_empty() {
                continue;
            }
            // Radar palettes are one compact strip instead of nine cards.
            let palettes: Vec<&Product> = items.iter().copied().filter(|p| matches!(p.action, Action::Radar(_))).collect();
            let others: Vec<&Product> = items.iter().copied().filter(|p| !matches!(p.action, Action::Radar(_))).collect();

            let mut section = col().gap(px(12.)).child(
                col()
                    .gap(px(2.))
                    .child(div().font_family("Bahnschrift").text_size(px(20.)).text_color(c(0xFFFFFF)).child(cat.label()))
                    .child(div().text_size(px(12.)).text_color(c(MUTED)).child(cat.blurb())),
            );

            if !palettes.is_empty() {
                let chips = palettes
                    .iter()
                    .map(|p| {
                        card_index += 1;
                        let on = p.action.is_on(&live);
                        let action = p.action.clone();
                        let name = p.name.replace("Live Doppler • ", "");
                        row()
                            .id(("palette", card_index))
                            .gap(px(8.))
                            .pl(px(4.))
                            .pr(px(12.))
                            .py(px(4.))
                            .rounded(px(18.))
                            .border_1()
                            .cursor_pointer()
                            .bg(c(0x131925))
                            .border_color(if on { c(CYAN) } else { c(BORDER) })
                            .hover(|s| s.border_color(c(CYAN)))
                            .child(div().w(px(40.)).h(px(22.)).rounded(px(11.)).bg(linear_gradient(
                                90.,
                                linear_color_stop(rgb(p.colors.0), 0.),
                                linear_color_stop(rgb(p.colors.1), 1.),
                            )))
                            .child(div().text_size(px(13.)).text_color(if on { c(CYAN) } else { c(TEXT) }).child(name))
                            .on_click(cx.listener(move |this, _, _, cx| this.apply(action.clone(), cx)))
                    })
                    .collect::<Vec<_>>();
                section = section.child(
                    col()
                        .gap(px(8.))
                        .child(div().text_size(px(11.)).text_color(c(DIM)).font_weight(FontWeight::BOLD).child("LIVE RADAR • COLOR PALETTE"))
                        .child(row().flex_wrap().gap(px(8.)).children(chips)),
                );
            }

            let cards = others
                .iter()
                .map(|p| {
                    card_index += 1;
                    let i = card_index;
                    let ok = feed_ok(p.feed);
                    let on = p.action.is_on(&live);
                    let kind = p.action.kind();
                    let action = p.action.clone();
                    let button_action = p.action.clone();
                    let (kind_label, verb) = match kind {
                        Kind::MapLayer => ("MAP LAYER", if on { "On air map ✓" } else { "Show on map" }),
                        Kind::Graphic => ("GRAPHIC", "Open graphic"),
                        Kind::Tool => ("TOOL", if on { "Placed ✓" } else { "Place on map" }),
                    };
                    let accent = p.colors.1;
                    col()
                        .id(("card", i))
                        // 306 wide: three cards fit across a 1280 window (318 left a big gap at two).
                        .w(px(306.))
                        // Grow with the description instead of clipping the button off the bottom.
                        .min_h(px(150.))
                        .rounded(px(10.))
                        .overflow_hidden()
                        .bg(c(0x131925))
                        .border_1()
                        .border_color(if on { c(CYAN) } else { c(BORDER) })
                        .cursor_pointer()
                        .hover(|s| s.border_color(ca(CYAN, 0.7)).bg(c(0x171E2C)))
                        // Colour band previewing the product's palette.
                        .child(div().w_full().h(px(5.)).bg(linear_gradient(
                            90.,
                            linear_color_stop(rgb(p.colors.0), 0.),
                            linear_color_stop(rgb(p.colors.1), 1.),
                        )))
                        .child(
                            col()
                                .flex_1()
                                .px(px(14.))
                                .pt(px(10.))
                                .pb(px(12.))
                                .gap(px(6.))
                                .child(
                                    row()
                                        .gap(px(8.))
                                        .child(div().text_size(px(10.)).font_weight(FontWeight::BOLD).text_color(c(accent)).child(kind_label))
                                        .when(p.is_new, |d| {
                                            d.child(div().text_size(px(10.)).px(px(5.)).rounded(px(3.)).bg(ca(CYAN, 0.18)).text_color(c(CYAN)).child("New"))
                                        })
                                        .child(div().flex_1())
                                        .when(on, |d| d.child(div().text_size(px(10.)).font_weight(FontWeight::BOLD).text_color(c(CYAN)).child("● ON NOW")))
                                        .child({
                                            let fav = favorites.iter().any(|f| f == p.name);
                                            let name = p.name;
                                            div()
                                                .id(("star", i))
                                                .px(px(4.))
                                                .text_size(px(14.))
                                                .text_color(if fav { c(0xF5C542) } else { c(MUTED) })
                                                .cursor_pointer()
                                                .child(if fav { "★" } else { "☆" })
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    this.toggle_favorite(name, cx);
                                                }))
                                        }),
                                )
                                .child(
                                    div()
                                        .text_size(px(16.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(c(0xFFFFFF))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .child(p.name),
                                )
                                .child(div().text_size(px(12.)).line_height(px(16.)).text_color(c(MUTED)).child(p.description))
                                .child(div().flex_1())
                                .child(
                                    row()
                                        .gap(px(6.))
                                        .child(div().w(px(7.)).h(px(7.)).rounded(px(4.)).bg(match ok {
                                            Some(true) => c(GREEN),
                                            Some(false) => c(RED),
                                            None => c(DIM),
                                        }))
                                        .child(div().text_size(px(11.)).text_color(c(MUTED)).child(match ok {
                                            Some(true) => "Data OK",
                                            Some(false) => "Feed error",
                                            None => "Loading",
                                        }))
                                        .child(div().flex_1())
                                        .child(
                                            div()
                                                .id(("go", i))
                                                .px(px(12.))
                                                .py(px(4.))
                                                .rounded(px(5.))
                                                .text_size(px(12.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .when(on, |d| d.bg(ca(CYAN, 0.16)).text_color(c(CYAN)))
                                                .when(!on, |d| d.bg(c(CYAN)).text_color(c(0xFFFFFF)).hover(|s| s.bg(c(0x74A5F2))))
                                                .child(verb)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    this.apply(button_action.clone(), cx);
                                                })),
                                        ),
                                ),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| this.apply(action.clone(), cx)))
                })
                .collect::<Vec<_>>();
            if !cards.is_empty() {
                section = section.child(row().flex_wrap().gap(px(14.)).children(cards));
            }
            body.push(section.into_any_element());
        }
        let empty = body.is_empty();

        row()
            .size_full()
            .items_start()
            .bg(c(BG))
            .child(
                col()
                    .w(px(230.))
                    .h_full()
                    .pt(px(18.))
                    .gap(px(2.))
                    .bg(c(PANEL))
                    .border_r_1()
                    .border_color(c(BORDER))
                    .child(
                        div()
                            .id("dd-desk")
                            .mx(px(10.))
                            .mb(px(10.))
                            .px(px(12.))
                            .py(px(8.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(c(BORDER))
                            .text_size(px(13.))
                            .text_color(c(TEXT))
                            .cursor_pointer()
                            .hover(|s| s.border_color(c(CYAN)))
                            .child("Forecaster desk…")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.modal = crate::app::Modal::Desk;
                                    cx.notify();
                                });
                            })),
                    )
                    .child(
                        div()
                            .id("dd-viewers")
                            .mx(px(10.))
                            .mb(px(10.))
                            .px(px(12.))
                            .py(px(8.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(c(BORDER))
                            .text_size(px(13.))
                            .text_color(c(TEXT))
                            .cursor_pointer()
                            .hover(|s| s.border_color(c(CYAN)))
                            .child({
                                let pending = self.app.read(cx).data.viewer.iter().filter(|s| s.status == "pending").count();
                                if pending > 0 { format!("Viewer submissions ({pending} new)…") } else { "Viewer submissions…".to_string() }
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.modal = crate::app::Modal::Viewers;
                                    cx.notify();
                                });
                            })),
                    )
                    .child(
                        div()
                            .id("dd-publishing")
                            .mx(px(10.))
                            .mb(px(10.))
                            .px(px(12.))
                            .py(px(8.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(c(BORDER))
                            .text_size(px(13.))
                            .text_color(c(TEXT))
                            .cursor_pointer()
                            .hover(|s| s.border_color(c(CYAN)))
                            .child({
                                let queued = crate::publish::pending();
                                if queued > 0 { format!("Publishing ({queued} queued)…") } else { "Publishing and public feeds…".to_string() }
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.modal = crate::app::Modal::Publishing;
                                    cx.notify();
                                });
                            })),
                    )
                    .child(
                        div()
                            .id("dd-eoc")
                            .mx(px(10.))
                            .mb(px(10.))
                            .px(px(12.))
                            .py(px(8.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(c(BORDER))
                            .text_size(px(13.))
                            .text_color(c(TEXT))
                            .cursor_pointer()
                            .hover(|s| s.border_color(c(CYAN)))
                            .child({
                                let name = self.app.read(cx).data.eoc.incident.name.clone();
                                if name.is_empty() { "Emergency operations…".to_string() } else { format!("Emergency operations: {name}…") }
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.modal = crate::app::Modal::Eoc;
                                    cx.notify();
                                });
                            })),
                    )
                    .child(
                        div()
                            .id("dd-workflows")
                            .mx(px(10.))
                            .mb(px(10.))
                            .px(px(12.))
                            .py(px(8.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(c(BORDER))
                            .text_size(px(13.))
                            .text_color(c(TEXT))
                            .cursor_pointer()
                            .hover(|s| s.border_color(c(CYAN)))
                            .child("Workflow automation…")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.modal = crate::app::Modal::Workflows;
                                    cx.notify();
                                });
                            })),
                    )
                    .child(
                        div()
                            .id("dd-lab")
                            .mx(px(10.))
                            .mb(px(10.))
                            .px(px(12.))
                            .py(px(8.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(c(BORDER))
                            .text_size(px(13.))
                            .text_color(c(TEXT))
                            .cursor_pointer()
                            .hover(|s| s.border_color(c(CYAN)))
                            .child("Science lab (verification, analysis)…")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.modal = crate::app::Modal::Lab;
                                    cx.notify();
                                });
                            })),
                    )
                    .child(div().px(px(22.)).pb(px(8.)).text_size(px(11.)).text_color(c(DIM)).font_weight(FontWeight::BOLD).child("BROWSE"))
                    .children(filters)
                    .child(div().h(px(18.)))
                    .child(
                        col()
                            .mx(px(10.))
                            .p(px(12.))
                            .gap(px(8.))
                            .rounded(px(8.))
                            .bg(c(0x0F141F))
                            .border_1()
                            .border_color(c(BORDER))
                            .child(div().text_size(px(12.)).text_color(c(TEXT)).child("Map layers go straight onto the live map in Map Studio. Graphics open in Graphics Studio, ready to edit."))
                            .child(
                                div()
                                    .id("dd-back-radar")
                                    .px(px(10.))
                                    .py(px(6.))
                                    .rounded(px(5.))
                                    .border_1()
                                    .border_color(c(BORDER))
                                    .text_size(px(12.))
                                    .text_color(c(TEXT))
                                    .cursor_pointer()
                                    .hover(|s| s.border_color(c(CYAN)))
                                    .child("↺ Back to live radar")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.app.update(cx, |s, cx| {
                                            s.data_playing = false;
                                            s.update_live(|l| {
                                                l.data.field = None;
                                                l.precip_total = None;
                                                l.satellite.enabled = false;
                                                l.radar.enabled = true;
                                                l.radar.source = RadarSource::Composite;
                                                l.layout = PanelLayout::Single;
                                            });
                                            s.workspace = Workspace::Map;
                                            cx.notify();
                                        });
                                    })),
                            ),
                    ),
            )
            .child(
                col()
                    .id("cards")
                    .flex_1()
                    .h_full()
                    .px(px(28.))
                    .py(px(22.))
                    .gap(px(26.))
                    .overflow_y_scroll()
                    .child(
                        row()
                            .gap(px(16.))
                            .items_end()
                            .child(
                                col()
                                    .gap(px(2.))
                                    .child(div().font_family("Bahnschrift").text_size(px(30.)).text_color(c(0xFFFFFF)).child("Data Discovery"))
                                    .child(div().text_size(px(13.)).text_color(c(MUTED)).child(self.category.blurb())),
                            )
                            .child(div().flex_1())
                            .child(div().w(px(340.)).child(Input::new(&self.search))),
                    )
                    .children(suggest_strip)
                    .children(fav_strip)
                    .children(recent_strip)
                    .when(empty, |d| d.child(muted("Nothing matches that search.")))
                    .children(body),
            )
    }
}
