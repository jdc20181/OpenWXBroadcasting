//! Infographic demonstrations (Claude's lane; Codex owns `gfx::sets`): calm, plain explainers
//! drawn as realistic scenes on the `explain:` custom canvas. Each topic has its own scene
//! (scenes_a / scenes_b, built from the components in kit.rs); labels are drawn by the scene
//! from its own geometry so a label always sits on what it names. Caption boxes and titles are
//! normal text layers so the station's look restyles them.

pub mod kit;
mod scenes_a;
mod scenes_b;
mod scenes_c;
mod scenes_d;
#[cfg(test)]
mod tests;

use std::sync::LazyLock;

use tiny_skia::{Color, Mask, Pixmap, Transform};

use crate::gfx::{
    history_set::paint::{hex, poly, rgba, rrect, solid, Al, Canvas},
    model::{AnimKind, Fill, GraphicDoc, Layer},
    templates::{footer, header, text, wrapped, TemplateInfo, BODY},
    text::{Align, GlyphCache, Weight},
};

pub const CATEGORY: &str = "Explainers";

/// One explainer: scene + title + caption boxes (canvas coordinates, 1920 × 832).
pub struct Ex {
    pub key: &'static str,
    pub name: &'static str,
    pub sub: &'static str,
    pub scene: &'static str,
    pub caps: &'static [(f32, f32, f32, f32, &'static str)],
    pub source: &'static str,
    pub about: &'static str,
}

const NWS: &str = "Source: NOAA / National Weather Service";

macro_rules! ex {
    ($key:expr, $name:expr, $sub:expr, $scene:expr, $about:expr, $src:expr, [$(($x:expr, $y:expr, $w:expr, $h:expr, $t:expr)),* $(,)?]) => {
        Ex { key: $key, name: $name, sub: $sub, scene: $scene, about: $about, source: $src, caps: &[$(($x as f32, $y as f32, $w as f32, $h as f32, $t)),*] }
    };
}

pub const EXPLAINERS: &[Ex] = &[
    // Severe storms
    ex!("explain_tornado_forms", "How a Tornado Forms", "Severe storms", "tornado_forms", "Supercell cut-away: inflow, rotating updraft, wall cloud, tornado.", "Source: NOAA NSSL / NWS",
        [(40, 742, 900, 84, "A storm's rotating updraft can tighten and stretch down to the ground as a tornado.")]),
    ex!("explain_hail_growth", "How Hail Grows", "Severe storms", "hail_growth", "Updraft cycles a stone through freezing air; cut-open layers.", "Source: NOAA NSSL / NWS",
        [(40, 742, 660, 84, "Strong updrafts keep hail aloft so it can grow layer by layer.")]),
    ex!("explain_hail_sizes", "Hail Size Guide", "Severe storms", "hail_sizes", "Pea to softball, drawn to scale, with the 1-inch severe line.", NWS,
        [(40, 30, 760, 84, "Forecasters compare hail to everyday objects to report its size.")]),
    ex!("explain_severe_criteria", "What Makes a Storm Severe", "Severe storms", "severe_criteria", "The NWS severe thunderstorm thresholds: 58 mph wind, 1-inch hail.", NWS,
        [(1180, 742, 700, 84, "A tornado also makes any storm severe.")]),
    ex!("explain_microburst", "What Is a Microburst?", "Severe storms", "microburst", "Downdraft hits the ground and spreads out; trees bent outward.", NWS,
        [(40, 742, 880, 84, "Rain-cooled air rushes down and spreads out. Winds can top 100 mph.")]),
    ex!("explain_derecho", "What Is a Derecho?", "Severe storms", "derecho", "A bowed line of storms crossing farm country, wind behind the line.", "Source: NOAA Storm Prediction Center",
        [(1060, 730, 820, 96, "A long-lived windstorm with damage along a path hundreds of miles long.")]),
    ex!("explain_watch_warning", "Watch vs. Warning", "Severe storms", "watch_warning", "Building clouds (watch) next to a storm overhead (warning).", NWS,
        [(40, 742, 880, 84, "Watch: conditions are right. Be ready."), (1000, 742, 880, 84, "Warning: it is happening or about to. Act now.")]),
    ex!("explain_funnel_vs_tornado", "Funnel Cloud vs. Tornado", "Severe storms", "funnel_vs_tornado", "Funnel aloft vs funnel touching down with debris.", NWS,
        [(40, 742, 880, 84, "A funnel cloud has not reached the ground."), (1000, 30, 880, 84, "A tornado is touching the ground, often with a debris cloud.")]),
    ex!("explain_shelf_vs_wall", "Shelf Cloud vs. Wall Cloud", "Severe storms", "shelf_vs_wall", "Leading-edge shelf vs isolated rotating lowering.", NWS,
        [(40, 742, 880, 84, "Shelf cloud: gusty winds arriving."), (1000, 742, 880, 84, "Wall cloud: watch for rotation.")]),
    ex!("explain_tornado_in_car", "Tornado Warning in a Car", "Severe storms", "tornado_in_car", "Drive to a sturdy building; never shelter under an overpass.", NWS,
        [(40, 730, 1320, 96, "If you can't reach shelter, stay buckled, get below the windows and cover your head.")]),
    ex!("explain_ef_scale", "The Enhanced Fujita Scale", "Severe storms", "ef_scale", "EF0 to EF5 wind ranges with the same house at each damage level.", "Source: NWS / Texas Tech Wind Science",
        [(40, 742, 1840, 84, "Tornadoes are rated after a damage survey, from what they did, not how they looked.")]),
    ex!("explain_outdoor_event", "Outdoor Event Safety", "Severe storms", "outdoor_event", "Stadium with storm approaching; move everyone to a building.", NWS,
        [(40, 30, 700, 84, "Know your shelter plan before the game starts.")]),
    ex!("explain_sirens_indoors", "Sirens Are for Outdoors", "Severe storms", "siren_indoors", "Siren tower vs phone alert and NOAA Weather Radio indoors.", NWS,
        [(40, 30, 900, 84, "Never wait to hear a siren. Have more than one way to get warnings.")]),
    // Lightning
    ex!("explain_lightning_distance", "Lightning Reaches Far", "Lightning", "lightning_distance", "Bolt from the anvil striking 10 miles away under blue sky.", NWS,
        [(40, 30, 880, 84, "If you can hear thunder, you are close enough to be struck.")]),
    ex!("explain_lightning_wait", "Wait 30 Minutes", "Lightning", "lightning_wait", "Storm leaving; clock shading 30 minutes; family indoors.", NWS,
        [(1000, 742, 880, 84, "When thunder roars, go indoors.")]),
    ex!("explain_lightning_shelter", "Where Is Safe From Lightning?", "Lightning", "lightning_shelter", "House and hard-top car vs pavilion, tent and tree.", NWS,
        [(40, 742, 880, 84, "Safe: enclosed buildings with wiring and plumbing, or a hard-topped car."), (1000, 742, 880, 84, "Open shelters, tents and trees do not protect you.")]),
    // Flooding
    ex!("explain_flood_depth", "How Deep Is Too Deep?", "Flooding", "flood_depth", "6 in knocks you down; 1 ft floats a car; 2 ft sweeps away trucks.", NWS,
        [(40, 742, 1840, 84, "Moving water: 6 inches can knock you down, 1 foot can float a car, 2 feet can carry away SUVs and pickups.")]),
    ex!("explain_flood_road", "Turn Around, Don't Drown", "Flooding", "flood_road", "Road dips into water; washed-out hole hidden below.", NWS,
        [(40, 30, 880, 84, "You can't tell how deep the water is or if the road is still there.")]),
    ex!("explain_flash_vs_river", "Flash Flood vs. River Flood", "Flooding", "flash_vs_river", "Steep creek after a downpour vs a wide river over its banks.", NWS,
        [(40, 742, 880, 84, "Flash floods rise fast, often with little warning."), (1000, 742, 880, 84, "River floods rise slowly and can last for days.")]),
    ex!("explain_storm_drains", "Keep Storm Drains Clear", "Flooding", "storm_drain", "Leaves clogging a street drain while rain floods the road.", NWS,
        [(1180, 30, 700, 84, "Rake leaves away from drains before heavy rain.")]),
    ex!("explain_hydroplaning", "Hydroplaning", "Flooding", "hydroplaning", "Tyre close-up riding on a wedge of water.", NWS,
        [(1180, 30, 700, 84, "If you start to slide, ease off the gas and steer straight.")]),
    // Winter
    ex!("explain_precip_types", "Snow, Sleet or Freezing Rain?", "Winter", "precip_ground", "What each looks like on the ground, with its temperature column.", NWS,
        [(40, 742, 1840, 84, "A warm layer aloft melts snow. What happens near the ground decides what you get.")]),
    ex!("explain_frozen_pipes", "Prevent Frozen Pipes", "Winter", "frozen_pipes", "Kitchen cut-away: insulated pipes, open cabinet, dripping faucet.", "Source: American Red Cross / NWS",
        [(1000, 30, 880, 84, "Pipes along outside walls freeze first.")]),
    ex!("explain_car_kit", "Winter Car Kit", "Winter", "car_kit", "SUV in snow with a kit laid out: blankets, light, water, cables.", "Source: NWS / Ready.gov",
        [(40, 30, 620, 120, "Tell someone your route and when you expect to arrive.")]),
    ex!("explain_bridge_ice", "Bridges Freeze First", "Winter", "bridge_ice", "Cold air above and below the deck; the ground holds heat.", NWS,
        [(1180, 600, 700, 84, "Slow down before bridges and overpasses.")]),
    ex!("explain_wind_chill", "Wind Chill Chart", "Winter", "wind_chill_chart", "The NWS wind chill formula for 40 to -45°F and 5 to 60 mph.", "Source: NWS wind chill formula",
        [] ),
    ex!("explain_shoveling", "Shovel Safely", "Winter", "shoveling", "Pushing snow on a cleared path.", NWS,
        [(1180, 742, 700, 84, "Shoveling is hard work. Pace yourself.")]),
    ex!("explain_snow_ratio", "Snow to Water", "Winter", "snow_ratio", "10 inches of snow beside about 1 inch of water.", NWS,
        [(1440, 200, 440, 160, "Ten to one is the average. The real ratio changes storm to storm.")]),
    ex!("explain_snow_squall", "Snow Squalls", "Winter", "snow_squall", "Highway driving into a sudden wall of snow.", NWS,
        [(40, 30, 880, 84, "Brief, intense snow can cause sudden whiteouts.")]),
    ex!("explain_ice_accumulation", "How Much Ice Is Too Much?", "Winter", "ice_accum", "Power lines coated with 1/4, 1/2 and 1 inch of ice.", NWS,
        [(40, 30, 1840, 84, "Ice adds weight to trees and power lines. Small amounts can still cause outages.")]),
    ex!("explain_layering", "Dress in Layers", "Winter", "layering", "Three figures adding base, middle and shell layers.", NWS,
        [(40, 742, 1000, 84, "Layers trap warm air. Stay dry: wet clothes lose heat fast.")]),
    ex!("explain_pets_cold", "Pets in the Cold", "Winter", "pet_cold", "Dog heading indoors; water bowl; snowy yard.", "Source: AVMA / NWS",
        [(40, 30, 880, 84, "If it's too cold for you, it's too cold for them.")]),
    ex!("explain_generator", "Generator Safety", "Home safety", "generator", "Generator crossed out in the garage; 20+ feet from the house outside.", "Source: CDC / NWS",
        [(40, 742, 1000, 84, "Generator exhaust has carbon monoxide. Use a battery CO alarm indoors.")]),
    ex!("explain_space_heater", "Space Heater Safety", "Home safety", "space_heater", "Heater with a 3-foot clear zone, wall plug, smoke alarm.", "Source: U.S. Fire Administration",
        [(40, 742, 900, 84, "Heating equipment is a leading cause of home fires in winter.")]),
    ex!("explain_power_outage_food", "Food Safety in an Outage", "Home safety", "fridge_outage", "Closed fridge in a dark kitchen: 4 hours fridge, 48 hours freezer.", "Source: FoodSafety.gov (USDA)",
        [(1180, 742, 700, 84, "When in doubt, throw it out.")]),
    // Heat
    ex!("explain_heat_index", "Heat Index Chart", "Heat", "heat_index_chart", "NWS heat index from 80 to 112°F and 40 to 100% humidity.", "Source: NWS heat index",
        []),
    ex!("explain_fans", "Seasonal Fan Changes", "Heat", "fans", "Room with two ceiling fans: summer counter-clockwise, winter clockwise.", "Source: U.S. Department of Energy",
        [(40, 742, 880, 84, "Summer: blades push air down for a cooling breeze."), (1000, 742, 880, 84, "Winter: blades pull air up and push warm air down the walls.")]),
    ex!("explain_uv_index", "The UV Index", "Heat", "uv_index", "Sun, protected person and the five UV index levels.", "Source: U.S. EPA / NWS",
        [(820, 600, 1060, 84, "Sunburn can happen on cloudy days too.")]),
    ex!("explain_hot_pavement", "Hot Pavement and Paws", "Heat", "hot_pavement", "Dog on hot asphalt and the 7-second hand test.", "Source: AVMA",
        [(40, 30, 900, 84, "Walk pets early or late, on grass when you can.")]),
    ex!("explain_dew_point", "What the Dew Point Tells You", "Heat", "dew_point", "Sweating glass of ice water and the comfort scale.", NWS,
        [(40, 742, 860, 84, "The higher the dew point, the more moisture in the air.")]),
    ex!("explain_red_flag", "Red Flag Warning", "Heat", "red_flag", "Dry grass, wind and a crossed-out burn barrel.", NWS,
        [(1180, 742, 700, 84, "Fires can spread fast and get out of control.")]),
    ex!("explain_drought_levels", "Drought Levels", "Heat", "drought_levels", "Soil from slightly dry to deeply cracked, D0 to D4.", "Source: U.S. Drought Monitor",
        []),
    // Fog and sky
    ex!("explain_frost_vs_freeze", "Frost vs. Freeze", "Fog and sky", "frost_vs_freeze", "Night field: frost in the low spot while the air reads 36°F.", NWS,
        [(40, 30, 900, 84, "Frost can form when the air is a few degrees above freezing.")]),
    ex!("explain_radiation_fog", "Valley Fog", "Fog and sky", "radiation_fog", "Clear night, calm wind, fog pooling in the valley.", NWS,
        [(40, 742, 700, 84, "It usually burns off by mid-morning.")]),
    ex!("explain_advection_fog", "Fog Over Snow", "Fog and sky", "advection_fog", "Warm moist air flowing over snow cover.", NWS,
        [(1180, 30, 700, 84, "This fog can last all day.")]),
    ex!("explain_fog_driving", "Driving in Fog", "Fog and sky", "fog_driving", "Car with low beams on a foggy road.", NWS,
        [(40, 30, 900, 84, "If you can't see, pull well off the road and turn on hazards.")]),
    ex!("explain_cloud_types", "Cloud Types", "Fog and sky", "cloud_types", "Cirrus, altostratus, cumulus and cumulonimbus by height.", "Source: NOAA SciJinks / NWS",
        []),
    ex!("explain_rainbow", "How to Find a Rainbow", "Fog and sky", "rainbow", "Sun low behind the viewer, rain ahead.", "Source: NOAA SciJinks",
        [(40, 742, 700, 84, "Best early or late in the day.")]),
    ex!("explain_moon_halo", "Ring Around the Moon", "Fog and sky", "moon_halo", "22-degree halo in thin high cloud.", "Source: NOAA SciJinks",
        [(40, 742, 900, 84, "Ice crystals bend the light into a ring.")]),
    // Forecast know-how
    ex!("explain_rain_chance", "What \"40% Chance of Rain\" Means", "Forecast basics", "pop_meaning", "Forecast area with rain on part of it.", NWS,
        [(40, 30, 900, 120, "The chance that any one spot in the forecast area gets measurable rain.")]),
    ex!("explain_wind_direction", "Which Way Is the Wind?", "Forecast basics", "wind_from", "Barn weather vane in a north wind.", NWS,
        [(1180, 742, 700, 84, "A north wind blows from north to south.")]),
    ex!("explain_radar_beam", "Why Radar Misses Low Storms", "Forecast basics", "radar_beam", "0.5° beam height rising with distance over a curved Earth.", "Source: NWS (standard refraction)",
        [(40, 742, 900, 84, "Farther from the radar, the beam sees higher in the storm.")]),
    // Batch 3
    ex!("explain_co_alarms", "Carbon Monoxide Alarms", "Home safety", "co_alarm", "House cut-away with CO alarms on every level and fuel-burning appliances.", "Source: U.S. Fire Administration / CDC",
        [(40, 742, 900, 84, "CO has no colour or smell. Test alarms monthly.")]),
    ex!("explain_heat_work", "Working Outside in Heat", "Heat", "heat_work", "Shade canopy, water cooler, a worker resting and one working in the sun.", "Source: OSHA / NWS",
        [(40, 742, 900, 84, "Water, rest, shade. New workers need time to adjust.")]),
    ex!("explain_pool_lightning", "Lightning at the Pool", "Lightning", "pool_lightning", "Swimmers leaving the pool for a building as a storm nears.", NWS,
        [(40, 30, 900, 84, "Wait 30 minutes after the last thunder to swim again.")]),
    ex!("explain_boating_storms", "Storms on the Water", "Lightning", "boat_storm", "Small boat heading for the dock with a storm over the lake.", NWS,
        [(1180, 30, 700, 120, "On the water you are the tallest thing around.")]),
    ex!("explain_wind_trucks", "High Wind and Tall Vehicles", "Severe storms", "wind_truck", "Box truck on a highway in a strong crosswind.", NWS,
        [(40, 742, 1000, 84, "Slow down and leave room around trucks, trailers and RVs.")]),
    ex!("explain_secure_yard", "Secure Your Yard Before Wind", "Home safety", "secure_yard", "Anchored trampoline, stacked chairs, tied-down grill.", NWS,
        [(1180, 742, 700, 84, "Loose items become flying debris.")]),
    ex!("explain_ice_dams", "Ice Dams", "Winter", "ice_dams", "Roof edge cut-away: warm attic, melting snow refreezing at the eave.", "Source: NWS / U.S. Department of Energy",
        [(1180, 742, 700, 84, "Water backs up under shingles and leaks inside.")]),
    ex!("explain_storm_life_cycle", "A Thunderstorm's Life", "Severe storms", "storm_life_cycle", "Cumulus, mature and dissipating stages side by side.", "Source: NOAA NSSL / NWS",
        []),
    ex!("explain_underpass_flood", "Flooded Underpasses", "Flooding", "underpass_flood", "Road dipping under a bridge, filled with muddy water.", NWS,
        [(1180, 30, 700, 120, "Water can be deeper than it looks under a bridge.")]),
    ex!("explain_flood_stages", "River Flood Stages", "Flooding", "flood_stages", "River gauge with action, minor, moderate and major flood levels.", "Source: NWS river forecasts",
        [(40, 30, 880, 84, "Each river gauge has its own flood levels.")]),
    ex!("explain_lightning_indoors", "Lightning Safety Indoors", "Lightning", "lightning_indoors", "Kitchen sink crossed out; person on the couch with a cell phone.", NWS,
        [(1000, 742, 880, 84, "Lightning can travel through plumbing and wiring.")]),
    ex!("explain_heat_island", "Why Cities Run Warmer", "Heat", "heat_island", "Countryside and city skyline with a rising temperature line.", "Source: U.S. EPA / NOAA",
        [(40, 742, 900, 84, "Nights stay warmer in town, which matters in heat waves.")]),
    ex!("explain_measure_snow", "How to Measure Snow", "Forecast basics", "measure_snow", "Snow boards and rulers in three spots, averaged.", "Source: NWS / CoCoRaHS",
        [(1180, 30, 700, 84, "Report your total to help the forecast.")]),
    ex!("explain_measure_rain", "How to Measure Rain", "Forecast basics", "measure_rain", "Clear 4-inch rain gauge on a post in an open yard.", "Source: NWS / CoCoRaHS",
        [(40, 30, 900, 84, "A clear gauge with marks every hundredth of an inch works best.")]),
    ex!("explain_plow_room", "Give Plows Room", "Winter", "plow_room", "Plow truck spreading salt with a car keeping its distance.", "Source: NWS / State DOT",
        [(1180, 742, 700, 84, "The road behind a plow is the best road.")]),
    ex!("explain_drifting_snow", "Blowing and Drifting Snow", "Winter", "drifting_snow", "Wind across a field drifting snow onto a road.", NWS,
        [(1180, 30, 700, 84, "Visibility can drop fast in open country.")]),    // Batch 4
    ex!("explain_seasons", "Why We Have Seasons", "Fog and sky", "seasons_tilt", "Earth's 23.5° tilt toward and away from the sun in June and December.", "Source: NOAA SciJinks",
        [(40, 742, 1000, 84, "Seasons come from the tilt, not from how close we are to the sun.")]),
    ex!("explain_hail_car", "Protect Your Car From Hail", "Severe storms", "hail_car", "Car in a garage next to one out in falling hail.", NWS,
        [(40, 742, 880, 84, "Move cars inside when a severe storm is coming.")]),
    ex!("explain_tree_limbs", "Trim Dead Limbs", "Home safety", "tree_limbs", "Dead limb hanging over a roof.", NWS,
        [(40, 742, 880, 84, "Falling limbs cause much of the damage and outages in wind storms.")]),
    ex!("explain_sandbags", "Sandbagging Basics", "Flooding", "sandbags", "Staggered sandbag wall between floodwater and a house.", "Source: FEMA / U.S. Army Corps of Engineers",
        [(1180, 30, 700, 84, "Fill bags half full; don't tie them.")]),
    ex!("explain_uv_snow", "Sunburn in the Snow", "Heat", "uv_snow", "Walker on bright snow with direct and reflected sunlight.", "Source: U.S. EPA",
        [(40, 742, 900, 84, "UV is reflected up from snow, so you get it twice.")]),
    ex!("explain_cloudy_nights", "Clear Nights Get Colder", "Fog and sky", "cloudy_nights", "Clear night losing heat vs cloudy night holding it in.", NWS,
        [(40, 742, 1840, 84, "That's why the coldest mornings come after clear, calm nights.")]),
    ex!("explain_sleet_vs_hail", "Sleet vs. Hail", "Winter", "sleet_vs_hail", "Winter sleet pellets beside a layered summer hailstone.", NWS,
        []),
    ex!("explain_ice_thickness", "Is the Ice Safe?", "Winter", "ice_thickness", "Pond cross-section with ice thickness guidelines.", "Source: State DNR ice guidelines",
        [(40, 742, 1840, 84, "No ice is ever completely safe. Stay off ice over moving water.")]),
    ex!("explain_gusts", "Gusts vs. Sustained Wind", "Forecast basics", "gust_vs_sustained", "Anemometer and a wind trace with an average line and gust spikes.", NWS,
        [(40, 742, 1000, 84, "Forecasts give both: the steady wind and how strong the gusts get.")]),
    ex!("explain_windshield", "Clear the Whole Windshield", "Winter", "windshield_clear", "Frosted car with a peephole vs a fully cleared car.", "Source: NWS / State DOT",
        []),
    ex!("explain_snowmelt_flooding", "Snowmelt Flooding", "Flooding", "snowmelt_runoff", "Rain on melting snow over frozen ground running into a creek.", NWS,
        [(1180, 742, 700, 84, "Watch low spots during the first warm, rainy days.")]),
    ex!("explain_sump_pump", "Sump Pump Check", "Home safety", "sump_pump", "Basement cut-away with sump pit, pump, discharge pipe and battery backup.", "Source: FEMA / NWS",
        [(40, 742, 880, 84, "Pour a bucket of water in the pit to test it.")]),];

/// Template list built from the explainer table.
pub static TEMPLATES: LazyLock<Vec<TemplateInfo>> = LazyLock::new(|| EXPLAINERS.iter().map(|e| TemplateInfo { key: e.key, name: e.name, category: CATEGORY, description: e.about }).collect());

/// Sub-group for the Graphics Studio (e.g. "Winter").
pub fn subcategory(key: &str) -> Option<&'static str> {
    EXPLAINERS.iter().find(|e| e.key == key).map(|e| e.sub)
}

pub fn build(key: &str) -> Option<GraphicDoc> {
    let e = EXPLAINERS.iter().find(|e| e.key == key)?;
    let mut d = GraphicDoc::new(e.name, CATEGORY, Some(Fill::solid("#0E1726")));
    d.layers.push(Layer::risk_map("Scene", 0., 148., 1920., 832., &format!("explain:{}", e.scene)).with_anim(AnimKind::Fade, 0.1, 0.6));
    header(&mut d, e.name, "");
    for (i, (x, y, w, h, t)) in e.caps.iter().enumerate() {
        let (x, y) = (*x, *y + 148.);
        d.layers.push(Layer::rect(&format!("Caption {i}"), x, y, *w, *h, Fill::solid("rgba(0,0,0,0.82)"), 10.).with_anim(AnimKind::Rise, 0.45 + i as f32 * 0.15, 0.4));
        d.layers.push(wrapped(text(&format!("Caption {i} text"), (x + 22., y + 12., w - 44., h - 20.), t, BODY, 32., Weight::SemiBold, "#FFFFFF", Align::Left)).with_anim(AnimKind::Fade, 0.55 + i as f32 * 0.15, 0.4));
    }
    footer(&mut d, "{{station}}", e.source);
    d.template = Some(key.to_string());
    Some(d)
}

#[allow(clippy::too_many_arguments)]
pub fn draw_map(pm: &mut Pixmap, glyphs: &mut GlyphCache, local: Transform, mask: Option<&Mask>, w: f32, h: f32, spec: &str, opacity: f32, font: &str) -> bool {
    let Some(name) = spec.trim().strip_prefix("explain:") else { return false };
    let Some(mut cv) = Canvas::new(glyphs, w, h, local, font) else { return true };
    // Scenes are authored at 1920 × 832; other boxes scale the scene to fit.
    let k = (w / 1920.).min(h / 832.);
    cv.s *= k;
    cv.w = 1920.;
    cv.h = 832.;
    draw_scene(&mut cv, name);
    cv.finish(pm, local, mask, opacity);
    true
}

pub fn draw_scene(cv: &mut Canvas, name: &str) {
    use scenes_a as a;
    use scenes_b as b;
    use scenes_c as c;
    use scenes_d as d;
    match name {
        "tornado_forms" => a::tornado_forms(cv),
        "hail_growth" => a::hail_growth(cv),
        "hail_sizes" => a::hail_sizes(cv),
        "severe_criteria" => a::severe_criteria(cv),
        "microburst" => a::microburst(cv),
        "derecho" => a::derecho(cv),
        "watch_warning" => a::watch_warning(cv),
        "funnel_vs_tornado" => a::funnel_vs_tornado(cv),
        "shelf_vs_wall" => a::shelf_vs_wall(cv),
        "tornado_in_car" => a::tornado_in_car(cv),
        "ef_scale" => a::ef_scale(cv),
        "outdoor_event" => a::outdoor_event(cv),
        "siren_indoors" => a::siren_indoors(cv),
        "lightning_distance" => a::lightning_distance(cv),
        "lightning_wait" => a::lightning_wait(cv),
        "lightning_shelter" => a::lightning_shelter(cv),
        "flood_depth" => a::flood_depth(cv),
        "flood_road" => a::flood_road(cv),
        "flash_vs_river" => a::flash_vs_river(cv),
        "pop_meaning" => a::pop_meaning(cv),
        "wind_from" => a::wind_from(cv),
        "radar_beam" => a::radar_beam(cv),
        "fans" => b::fans(cv),
        "precip_ground" => b::precip_ground(cv),
        "frozen_pipes" => b::frozen_pipes(cv),
        "car_kit" => b::car_kit(cv),
        "bridge_ice" => b::bridge_ice(cv),
        "generator" => b::generator(cv),
        "space_heater" => b::space_heater(cv),
        "wind_chill_chart" => b::wind_chill_chart(cv),
        "heat_index_chart" => b::heat_index_chart(cv),
        "frost_vs_freeze" => b::frost_vs_freeze(cv),
        "radiation_fog" => b::radiation_fog(cv),
        "advection_fog" => b::advection_fog(cv),
        "fog_driving" => b::fog_driving(cv),
        "dew_point" => b::dew_point(cv),
        "uv_index" => b::uv_index(cv),
        "hot_pavement" => b::hot_pavement(cv),
        "pet_cold" => b::pet_cold(cv),
        "shoveling" => b::shoveling(cv),
        "snow_ratio" => b::snow_ratio(cv),
        "fridge_outage" => b::fridge_outage(cv),
        "snow_squall" => b::snow_squall(cv),
        "ice_accum" => b::ice_accum(cv),
        "cloud_types" => b::cloud_types(cv),
        "rainbow" => b::rainbow(cv),
        "hydroplaning" => b::hydroplaning(cv),
        "storm_drain" => b::storm_drain(cv),
        "layering" => b::layering(cv),
        "red_flag" => b::red_flag(cv),
        "drought_levels" => b::drought_levels(cv),
        "moon_halo" => b::moon_halo(cv),
        "co_alarm" => c::co_alarm(cv),
        "heat_work" => c::heat_work(cv),
        "pool_lightning" => c::pool_lightning(cv),
        "boat_storm" => c::boat_storm(cv),
        "wind_truck" => c::wind_truck(cv),
        "secure_yard" => c::secure_yard(cv),
        "ice_dams" => c::ice_dams(cv),
        "storm_life_cycle" => c::storm_life_cycle(cv),
        "underpass_flood" => c::underpass_flood(cv),
        "flood_stages" => c::flood_stages(cv),
        "lightning_indoors" => c::lightning_indoors(cv),
        "heat_island" => c::heat_island(cv),
        "measure_snow" => c::measure_snow(cv),
        "measure_rain" => c::measure_rain(cv),
        "plow_room" => c::plow_room(cv),
        "drifting_snow" => c::drifting_snow(cv),        "seasons_tilt" => d::seasons_tilt(cv),
        "hail_car" => d::hail_car(cv),
        "tree_limbs" => d::tree_limbs(cv),
        "sandbags" => d::sandbags(cv),
        "uv_snow" => d::uv_snow(cv),
        "cloudy_nights" => d::cloudy_nights(cv),
        "sleet_vs_hail" => d::sleet_vs_hail(cv),
        "ice_thickness" => d::ice_thickness(cv),
        "gust_vs_sustained" => d::gust_vs_sustained(cv),
        "windshield_clear" => d::windshield_clear(cv),
        "snowmelt_runoff" => d::snowmelt_runoff(cv),
        "sump_pump" => d::sump_pump(cv),        _ => cv.missing("Unknown explainer"),
    }
}

/// Highlighted label: coloured box with dark text at `at` (top-left), with an optional pointer.
pub fn tag(cv: &mut Canvas, label: &str, at: (f32, f32), pointer: Option<(f32, f32)>, bg: &str) {
    let size = 30.;
    let tw = cv.measure(label, size, Weight::Bold);
    let (bw, bh) = (tw + 36., 52.);
    let x = at.0.clamp(10., cv.w - bw - 10.);
    let y = at.1.clamp(40., cv.h - bh - 10.);
    if let Some((px, py)) = pointer {
        let from = (px.clamp(x, x + bw), py.clamp(y, y + bh));
        cv.line(from, (px, py), rgba(0, 0, 0, 0.6), 7.);
        cv.line(from, (px, py), hex(bg), 3.5);
        cv.fill_c(crate::gfx::history_set::paint::ellipse(px, py, 8., 8.), hex(bg));
    }
    cv.fill(rrect(x + 3., y + 5., bw, bh, 10.), &solid(rgba(0, 0, 0, 0.45)));
    cv.fill(rrect(x, y, bw, bh, 10.), &solid(hex(bg)));
    cv.text(label, x + 18., y + 37., size, Weight::Bold, hex("#111418"), Al::Left, 0.);
}

/// Big panel heading label.
pub fn tag_at(cv: &mut Canvas, label: &str, x: f32, y: f32, bg: &str, size: f32) {
    let tw = cv.measure(label, size, Weight::Bold);
    let (bw, bh) = (tw + 44., size * 1.55);
    cv.fill(rrect(x + 4., y + 6., bw, bh, 12.), &solid(rgba(0, 0, 0, 0.5)));
    cv.fill(rrect(x, y, bw, bh, 12.), &solid(hex(bg)));
    cv.text(label, x + 22., y + bh * 0.72, size, Weight::Bold, hex("#111418"), Al::Left, 0.);
}

/// White split line between two panels.
pub fn divider(cv: &mut Canvas, x: f32) {
    cv.line((x + 3., 0.), (x + 3., cv.h), rgba(0, 0, 0, 0.45), 8.);
    cv.fill(poly(&[(x - 4., 0.), (x + 4., 0.), (x + 4., cv.h), (x - 4., cv.h)], true), &solid(Color::WHITE));
}
