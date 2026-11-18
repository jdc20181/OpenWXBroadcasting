//! More built-in templates (child module of `templates`, sharing its layout helpers):
//! continuity, severe, lightning, forecast planning, astronomy, charts, rivers, travel,
//! community, sponsor, accessibility and Spanish graphics.

use super::*;
use crate::gfx::chart::ChartKind;

pub const TEMPLATES_EXT: &[TemplateInfo] = &[
    TemplateInfo { key: "backup", name: "Please Stay With Us", category: "Show", description: "Emergency backup card while a problem is fixed (Severe Center or the control API)." },
    TemplateInfo { key: "storm_reports", name: "Storm Reports", category: "Severe", description: "Newest storm reports, counts by type and the counties with the most reports." },
    TemplateInfo { key: "spc_probs", name: "Tornado / Hail / Wind Chances", category: "Severe", description: "SPC Day 1 hazard chances and the risk level." },
    TemplateInfo { key: "severe_timing", name: "Severe Timing", category: "Severe", description: "When storms are most likely, with the outlook and hourly rain chart." },
    TemplateInfo { key: "lightning_safety", name: "Lightning Safety", category: "Severe", description: "Lightning nearby, the 30-minute all-clear countdown and the strike trend." },
    TemplateInfo { key: "cell_briefing", name: "Storm Cell Briefing", category: "Severe", description: "The strongest storms: position, motion and intensity, with reflectivity history." },
    TemplateInfo { key: "ef_scale", name: "EF Scale", category: "Severe", description: "Enhanced Fujita tornado rating scale with wind speeds and damage." },
    TemplateInfo { key: "watch_warning", name: "Watch vs Warning", category: "Severe", description: "What a watch and a warning mean, and what to do." },
    TemplateInfo { key: "damage_survey", name: "Tornado Damage Survey", category: "Severe", description: "NWS survey result: rating, path length and width, peak wind." },
    TemplateInfo { key: "hourly_strip", name: "Hour by Hour", category: "Forecast", description: "Next 12 hours with icons and temperatures." },
    TemplateInfo { key: "day_parts", name: "Morning / Afternoon / Evening", category: "Forecast", description: "Today in three parts." },
    TemplateInfo { key: "weekend", name: "Weekend Planner", category: "Forecast", description: "Saturday and Sunday at a glance." },
    TemplateInfo { key: "hourly_temp_chart", name: "Temperature Trend", category: "Forecast", description: "Next 24 hours of temperature and feels-like as a chart." },
    TemplateInfo { key: "wind_timeline", name: "Wind Timeline", category: "Forecast", description: "Wind and gusts for the next 24 hours, strongest gust called out." },
    TemplateInfo { key: "rain_chances", name: "Rain Chances", category: "Forecast", description: "7-day rain chances as bars." },
    TemplateInfo { key: "rain_totals", name: "Rain Totals Forecast", category: "Forecast", description: "WPC rainfall forecast for home and nearby towns." },
    TemplateInfo { key: "winter_timeline", name: "Winter Storm Timeline", category: "Forecast", description: "When wintry weather starts and ends, with snow and ice totals." },
    TemplateInfo { key: "pressure_trend", name: "Pressure Trend", category: "Forecast", description: "Barometric pressure now, its trend and the next 24 hours." },
    TemplateInfo { key: "cities_now", name: "Around the Area", category: "Forecast", description: "Current temperatures and skies in nearby towns." },
    TemplateInfo { key: "obs_dashboard", name: "Observation Dashboard", category: "Forecast", description: "Live gauges for humidity, wind and pressure." },
    TemplateInfo { key: "sun_moon", name: "Sun and Moon", category: "Forecast", description: "Sunrise, sunset, daylight and the moon phase." },
    TemplateInfo { key: "season_countdown", name: "Season Countdown", category: "Forecast", description: "Days until the next season, with daylight through the year." },
    TemplateInfo { key: "river_stages", name: "River Levels", category: "Flooding", description: "River gauges near home: stage, flood stage, category and forecast." },
    TemplateInfo { key: "hydrograph", name: "River Hydrograph", category: "Flooding", description: "Observed and forecast river stage with the flood stage line." },
    TemplateInfo { key: "excessive_rain", name: "Flooding Rain Risk", category: "Flooding", description: "WPC excessive rainfall risk at home, Days 1-3." },
    TemplateInfo { key: "travel", name: "Travel Impacts", category: "Travel", description: "Road closures and incidents with commute outlooks." },
    TemplateInfo { key: "commute", name: "Commute Forecast", category: "Travel", description: "Morning and evening drive." },
    TemplateInfo { key: "closings", name: "Closings and Delays", category: "Community", description: "Full-screen list from the community board." },
    TemplateInfo { key: "closings_crawl", name: "Closings Crawl", category: "Community", description: "Scrolling closings ticker (overlay)." },
    TemplateInfo { key: "centers", name: "Warming / Cooling Centers", category: "Community", description: "Open centers with addresses and hours." },
    TemplateInfo { key: "community_forecast", name: "Community Forecast", category: "Community", description: "Town-by-town conditions across the coverage area." },
    TemplateInfo { key: "sponsor_full", name: "Sponsor Card", category: "Show", description: "Full-screen \"weather brought to you by\" card with the rotating sponsor." },
    TemplateInfo { key: "current_hc", name: "Current Conditions (high contrast)", category: "Accessibility", description: "Large, high-contrast current conditions for low-vision viewers." },
    TemplateInfo { key: "seven_day_es", name: "Pronóstico de 7 Días", category: "Español", description: "7-day forecast in Spanish." },
    TemplateInfo { key: "current_es", name: "Condiciones Actuales", category: "Español", description: "Current conditions in Spanish." },    TemplateInfo { key: "heat_index", name: "Heat Index", category: "Forecast", description: "Feels-like heat with the danger categories." },
    TemplateInfo { key: "wind_chill", name: "Wind Chill", category: "Forecast", description: "How cold it feels, with frostbite times." },
    TemplateInfo { key: "visibility_now", name: "Visibility", category: "Forecast", description: "Current visibility with driving advice." },
    TemplateInfo { key: "sky_now", name: "Sky Conditions", category: "Forecast", description: "Cloud cover and sky now and through the day." },
    TemplateInfo { key: "dew_humidity", name: "Dew Point and Humidity", category: "Forecast", description: "How muggy it feels, with the comfort scale." },
    TemplateInfo { key: "temp_departure", name: "Normals and Records", category: "Forecast", description: "Today's normal high and low, records, and how today compares." },
    TemplateInfo { key: "rain_departure", name: "Rain This Month", category: "Forecast", description: "Rain so far this month against normal." },
    TemplateInfo { key: "hourly_rain", name: "Hourly Rain", category: "Forecast", description: "Precipitation hour by hour for the next day." },
    TemplateInfo { key: "wind_advisory", name: "Wind Advisory Impacts", category: "Severe", description: "What strong gusts can do, with the peak gust." },
    TemplateInfo { key: "beaufort", name: "Beaufort Wind Scale", category: "Forecast", description: "Wind speeds and what you'll see at each level." },
    TemplateInfo { key: "snow_ratio", name: "Snow-to-Liquid Ratio", category: "Forecast", description: "How much snow each inch of water makes." },
    TemplateInfo { key: "blizzard", name: "Blizzard Conditions", category: "Severe", description: "Blizzard criteria and impacts." },
    TemplateInfo { key: "hazard_icons", name: "Storm Hazards", category: "Severe", description: "Tornado, hail, wind, flooding and lightning threat at a glance." },
    TemplateInfo { key: "confidence", name: "Forecast Confidence", category: "Forecast", description: "How sure the forecast is, from model agreement." },
    TemplateInfo { key: "tds_explainer", name: "Debris Signature", category: "Severe", description: "What a tornado debris signature on radar means." },
    TemplateInfo { key: "warning_history", name: "Warnings Today", category: "Severe", description: "Warnings issued today for the coverage area." },
    TemplateInfo { key: "nowcast_threats", name: "Storm Nowcast", category: "Severe", description: "Next-hour storm threats: arrival chance, hail, wind, heavy rain." },
    TemplateInfo { key: "power_counter", name: "Customers Without Power", category: "Power", description: "Total customers out, share of customers and the trend." },
    TemplateInfo { key: "power_counties", name: "Outages by County", category: "Power", description: "Counties with the most customers out." },
    TemplateInfo { key: "power_utilities", name: "Outages by Utility", category: "Power", description: "Customers out for each utility." },
    TemplateInfo { key: "power_trend", name: "Outage Trend", category: "Power", description: "Customers out over time." },
    TemplateInfo { key: "power_etr", name: "Restoration Estimates", category: "Power", description: "Estimated restoration times by county." },
    TemplateInfo { key: "power_restoration", name: "Restoration Progress", category: "Power", description: "How much has been restored since the peak, by county." },
    TemplateInfo { key: "power_safety", name: "Power Outage Safety", category: "Power", description: "Generator, food and heat / cold safety during outages." },
    TemplateInfo { key: "power_line_safety", name: "Downed Power Lines", category: "Power", description: "Animated downed-line safety demonstration." },
    TemplateInfo { key: "power_facilities", name: "Critical Facilities Affected", category: "Power", description: "Hospitals, water plants and emergency stations in hard-hit counties." },
    TemplateInfo { key: "photo_grid", name: "Storm Damage Photos", category: "Severe", description: "Four photos with captions (pick them in Media)." },
    TemplateInfo { key: "impact_summary", name: "Storm Impact Summary", category: "Severe", description: "Reports, warnings, rivers and roads after a storm." },
    TemplateInfo { key: "urban_flood", name: "Urban Flooding Risk", category: "Flooding", description: "Street flooding risk with rain rates and safety advice." },
    TemplateInfo { key: "black_ice", name: "Black Ice Risk", category: "Travel", description: "When roads can ice over, and where to watch." },
    TemplateInfo { key: "low_visibility", name: "Reduced Visibility", category: "Travel", description: "Fog and low-visibility driving advice." },
    TemplateInfo { key: "airport_delays", name: "Airport Delays", category: "Travel", description: "FAA airport delays and closures." },
    TemplateInfo { key: "school_transport", name: "School Buses and Weather", category: "Community", description: "Morning bus-stop weather, delays and closings." },
    TemplateInfo { key: "closings_business", name: "Business Closings", category: "Community", description: "Business closings from the community board." },
    TemplateInfo { key: "closings_government", name: "Government Closings", category: "Community", description: "Government office closings from the community board." },
    TemplateInfo { key: "closings_events", name: "Event Cancellations", category: "Community", description: "Cancelled and postponed events." },
    TemplateInfo { key: "emergency_declaration", name: "Emergency Declaration", category: "Community", description: "Local emergency declaration (edit the wording)." },
    TemplateInfo { key: "community_dashboard", name: "Community Impacts", category: "Community", description: "Closings, roads, rivers and alerts on one screen." },
    TemplateInfo { key: "shelters", name: "Emergency Shelters", category: "Community", description: "Open emergency shelters from the community board." },
    TemplateInfo { key: "workweek", name: "Workweek Planner", category: "Forecast", description: "Monday to Friday at a glance." },
    TemplateInfo { key: "ten_day", name: "10-Day Forecast", category: "Forecast", description: "Ten days of highs, lows and rain chances." },
    TemplateInfo { key: "outdoor_planner", name: "Outdoor Planner", category: "Forecast", description: "Good, fair or poor days for outdoor plans." },
    TemplateInfo { key: "impact_by_time", name: "Impacts by Time of Day", category: "Forecast", description: "What to expect morning, afternoon and evening." },
    TemplateInfo { key: "confidence_timeline", name: "Confidence Timeline", category: "Forecast", description: "How confident each day of the forecast is." },
    TemplateInfo { key: "meteor", name: "Meteor Shower", category: "Sky", description: "The next meteor shower and viewing conditions." },
    TemplateInfo { key: "eclipse", name: "Next Eclipse", category: "Sky", description: "The next eclipse visible from the US." },
    TemplateInfo { key: "aurora", name: "Northern Lights", category: "Sky", description: "Aurora chances from the Kp forecast." },
    TemplateInfo { key: "frost_dates", name: "Frost and Freeze Dates", category: "Forecast", description: "Typical last spring and first fall freeze." },
    TemplateInfo { key: "growing_season", name: "Growing Season", category: "Forecast", description: "Typical growing season length and freeze dates." },
    TemplateInfo { key: "night_sky", name: "Night Sky Tonight", category: "Sky", description: "Stargazing conditions: clouds and moonlight." },
    TemplateInfo { key: "model_compare", name: "Model Comparison", category: "Forecast", description: "GFS vs ECMWF highs for the week, with confidence." },
    TemplateInfo { key: "camera_view", name: "Camera View", category: "Travel", description: "Full-screen camera image with its name and current conditions." },
    TemplateInfo { key: "camera_inset", name: "Camera Inset", category: "Travel", description: "Overlay: the camera nearest the strongest storm in a corner of the live radar." },
    TemplateInfo { key: "travel_times", name: "Highway Travel Times", category: "Travel", description: "Estimated drive times now vs normal, with the reason for delays." },
    TemplateInfo { key: "traffic_report", name: "Traffic Report", category: "Travel", description: "Current incidents, slowdowns and closures." },
    TemplateInfo { key: "storm_compare", name: "Storm Comparison", category: "Severe", description: "Strongest storms by radar, lightning, reports and warnings, with how many sources agree." },
    TemplateInfo { key: "local_threats", name: "Local Threat Summary", category: "Severe", description: "Risk level, hazard chances, timing and warnings for the five largest towns." },
    TemplateInfo { key: "safe_tornado_shelter", name: "Where to Shelter", category: "Safety", description: "Animated diagram: Go to the lowest floor." },
    TemplateInfo { key: "safe_mobile_home", name: "Mobile Home Safety", category: "Safety", description: "Animated diagram: Mobile homes are not safe in a tornado." },
    TemplateInfo { key: "safe_vehicle", name: "Caught in a Car", category: "Safety", description: "Animated diagram: Drive at right angles to the storm if you can." },
    TemplateInfo { key: "safe_lightning", name: "Lightning Safety", category: "Safety", description: "Animated diagram: When thunder roars, go indoors." },
    TemplateInfo { key: "safe_flooded_road", name: "Flooded Roads", category: "Safety", description: "Animated diagram: Six inches of water can knock you down." },
    TemplateInfo { key: "safe_generator", name: "Generator Safety", category: "Safety", description: "Animated diagram: Run generators outdoors only." },
    TemplateInfo { key: "safe_power_line", name: "Downed Power Lines", category: "Safety", description: "Animated diagram: Assume every downed line is live." },
    TemplateInfo { key: "safe_heat", name: "Beat the Heat", category: "Safety", description: "Animated diagram: Drink water before you are thirsty." },
    TemplateInfo { key: "safe_winter_kit", name: "Winter Storm Kit", category: "Safety", description: "Animated diagram: Water, food and medicine for 3 days." },
    TemplateInfo { key: "safe_plan", name: "Your Severe Weather Plan", category: "Safety", description: "Animated diagram: Pick your safe place at home and work." },
    TemplateInfo { key: "edu_radar", name: "How Radar Works", category: "Explainers", description: "Animated diagram: The radar sends out a pulse." },
    TemplateInfo { key: "edu_ref_vel", name: "Reflectivity vs Velocity", category: "Explainers", description: "Animated diagram: Reflectivity shows how much is falling." },
    TemplateInfo { key: "edu_cc", name: "Spotting Debris on Radar", category: "Explainers", description: "Animated diagram: Raindrops are alike: high correlation." },
    TemplateInfo { key: "edu_tornado_forms", name: "How Tornadoes Form", category: "Explainers", description: "Animated diagram: Winds change with height and spin the air." },
    TemplateInfo { key: "edu_supercell", name: "Inside a Supercell", category: "Explainers", description: "Animated diagram: The updraft rotates." },
    TemplateInfo { key: "edu_fronts", name: "Fronts", category: "Explainers", description: "Animated diagram: Cold fronts push in fast and lift warm air." },
    TemplateInfo { key: "edu_lightning", name: "How Lightning Forms", category: "Explainers", description: "Animated diagram: Ice collisions separate charge." },
    TemplateInfo { key: "edu_hail", name: "How Hail Grows", category: "Explainers", description: "Animated diagram: The updraft carries drops above freezing level." },
    TemplateInfo { key: "edu_precip_types", name: "Rain, Sleet, Ice or Snow?", category: "Explainers", description: "Animated diagram: All cold: snow." },
    TemplateInfo { key: "edu_watch_warning", name: "Watch, Warning, Advisory", category: "Explainers", description: "Animated diagram: Advisory: be careful." },
    TemplateInfo { key: "demo_tornado_formation", name: "Tornado Formation", category: "Tornado", description: "Animated diagram: Spin develops from wind shear." },
    TemplateInfo { key: "demo_tornado_life", name: "A Tornado's Life", category: "Tornado", description: "Animated diagram: Forms under the wall cloud." },
    TemplateInfo { key: "demo_updraft", name: "The Rotating Updraft", category: "Tornado", description: "Animated diagram: Warm, moist air streams in." },
    TemplateInfo { key: "demo_wall_cloud", name: "The Wall Cloud", category: "Tornado", description: "Animated diagram: Rain-cooled air is pulled into the updraft." },
    TemplateInfo { key: "demo_funnel", name: "Funnel Clouds", category: "Tornado", description: "Animated diagram: Rotation tightens below the wall cloud." },
    TemplateInfo { key: "demo_debris", name: "The Debris Cloud", category: "Tornado", description: "Animated diagram: Tornado winds converge and lift." },
    TemplateInfo { key: "demo_rfd", name: "Rear-Flank Downdraft", category: "Tornado", description: "Animated diagram: Air descends behind the updraft." },
    TemplateInfo { key: "demo_hook", name: "The Hook Echo", category: "Tornado", description: "Animated diagram: Rain wraps around the rotation." },
    TemplateInfo { key: "demo_tornado_damage", name: "Tornado Damage", category: "Tornado", description: "Animated diagram: EF0-EF1: roofs and trees." },
    TemplateInfo { key: "demo_tornado_vs_wind", name: "Tornado or Straight-Line Wind?", category: "Tornado", description: "Animated diagram: Tornado: debris points in many directions." },
    TemplateInfo { key: "shelter_interior", name: "Interior Room", category: "Safety", description: "Animated diagram: Lowest floor." },
    TemplateInfo { key: "shelter_basement", name: "Basement Shelter", category: "Safety", description: "Animated diagram: Go to the basement." },
    TemplateInfo { key: "shelter_hallway", name: "Interior Hallway", category: "Safety", description: "Animated diagram: Lowest floor, center of the building." },
    TemplateInfo { key: "shelter_mobile_evac", name: "Leave the Mobile Home", category: "Safety", description: "Animated diagram: Have a plan before storms." },
    TemplateInfo { key: "shelter_manufactured", name: "Why Manufactured Homes Are at Risk", category: "Safety", description: "Animated diagram: Light frames can roll or come apart." },
    TemplateInfo { key: "shelter_school_drill", name: "Tornado Safety at School", category: "Safety", description: "Animated diagram: Follow the drill." },
    TemplateInfo { key: "shelter_work", name: "Tornado Safety at Work", category: "Safety", description: "Animated diagram: Know the designated shelter area." },
    TemplateInfo { key: "shelter_vehicle_decision", name: "Driving When a Tornado Hits", category: "Safety", description: "Animated diagram: Can you drive away at right angles? Do it." },
    TemplateInfo { key: "shelter_outdoor", name: "Caught Outside", category: "Safety", description: "Animated diagram: Get to a sturdy building if you can." },
    TemplateInfo { key: "shelter_kit", name: "Tornado Kit", category: "Safety", description: "Animated diagram: Helmets and sturdy shoes." },
    TemplateInfo { key: "hur_structure", name: "Hurricane Structure", category: "Hurricane", description: "Animated diagram: Rain bands spiral inward." },
    TemplateInfo { key: "hur_eye", name: "The Eye and Eyewall", category: "Hurricane", description: "Animated diagram: Air sinks in the eye." },
    TemplateInfo { key: "hur_wind", name: "Hurricane Winds", category: "Hurricane", description: "Animated diagram: Winds spin counterclockwise." },
    TemplateInfo { key: "hur_surge_level", name: "Storm Surge", category: "Hurricane", description: "Animated diagram: Wind pushes water toward the coast." },
    TemplateInfo { key: "hur_surge_coast", name: "Coastal Flooding", category: "Hurricane", description: "Animated diagram: Surge floods low-lying areas first." },
    TemplateInfo { key: "hur_rain_flood", name: "Hurricane Rain Floods", category: "Hurricane", description: "Animated diagram: Slow storms drop the most rain." },
    TemplateInfo { key: "hur_categories", name: "Hurricane Categories", category: "Hurricane", description: "Animated diagram: Category 1-2: damaging winds." },
    TemplateInfo { key: "hur_evac_timeline", name: "Evacuation Timeline", category: "Hurricane", description: "Animated diagram: 3 days: check your plan and supplies." },
    TemplateInfo { key: "hur_intensify", name: "How Hurricanes Strengthen", category: "Hurricane", description: "Animated diagram: Warm ocean water fuels the storm." },
    TemplateInfo { key: "hur_landfall", name: "Landfall", category: "Hurricane", description: "Animated diagram: Surge arrives before the center." },
    TemplateInfo { key: "flood_flash", name: "How Flash Floods Happen", category: "Flooding", description: "Animated diagram: Heavy rain falls fast." },
    TemplateInfo { key: "flood_urban", name: "Street Flooding", category: "Flooding", description: "Animated diagram: Pavement can't absorb the rain." },
    TemplateInfo { key: "flood_river", name: "River Flooding", category: "Flooding", description: "Animated diagram: Rain upstream raises the river." },
    TemplateInfo { key: "flood_vehicle", name: "Water vs Your Car", category: "Flooding", description: "Animated diagram: 6 inches: stalls many cars." },
    TemplateInfo { key: "flood_human", name: "Water vs You", category: "Flooding", description: "Animated diagram: 6 inches of moving water knocks you down." },
    TemplateInfo { key: "flood_low_water", name: "Low-Water Crossings", category: "Flooding", description: "Animated diagram: The road may be washed out under the water." },
    TemplateInfo { key: "flood_basement", name: "Basement Flooding", category: "Flooding", description: "Animated diagram: Water seeps in through walls and drains." },
    TemplateInfo { key: "flood_dam", name: "Dam and Levee Failure", category: "Flooding", description: "Animated diagram: Failures send a wall of water downstream." },
    TemplateInfo { key: "flood_evac", name: "Flood Evacuation", category: "Flooding", description: "Animated diagram: Leave early on higher roads." },
    TemplateInfo { key: "flood_compare", name: "Flash Flood or River Flood?", category: "Flooding", description: "Animated diagram: Flash floods: minutes to hours." },
    TemplateInfo { key: "winter_streets", name: "Snow on the Streets", category: "Winter", description: "Animated diagram: Snow sticks to grass and cars first." },
    TemplateInfo { key: "winter_vehicles", name: "Snow Piling Up", category: "Winter", description: "Animated diagram: Clear all snow off your car." },
    TemplateInfo { key: "winter_ice_accretion", name: "Freezing Rain", category: "Winter", description: "Animated diagram: Rain falls through a warm layer." },
    TemplateInfo { key: "winter_ice_lines", name: "Ice on Power Lines", category: "Winter", description: "Animated diagram: A quarter inch weighs down lines." },
    TemplateInfo { key: "winter_blizzard", name: "Blizzard Conditions", category: "Winter", description: "Animated diagram: Strong wind blows snow sideways." },
    TemplateInfo { key: "winter_lake_effect", name: "Lake-Effect Snow", category: "Winter", description: "Animated diagram: Cold air crosses a warm lake." },
    TemplateInfo { key: "winter_squall", name: "Snow Squalls", category: "Winter", description: "Animated diagram: Sudden, intense burst of snow." },
    TemplateInfo { key: "winter_black_ice", name: "Black Ice", category: "Winter", description: "Animated diagram: Wet roads freeze as temperatures drop." },
    TemplateInfo { key: "winter_drifts", name: "Blowing and Drifting Snow", category: "Winter", description: "Animated diagram: Wind picks up loose snow." },
    TemplateInfo { key: "winter_transition", name: "Rain to Snow", category: "Winter", description: "Animated diagram: Warm layer melts snow into rain." },
    TemplateInfo { key: "lightning_cg_demo", name: "Cloud-to-Ground Lightning", category: "Lightning", description: "Animated diagram: A leader steps down from the cloud." },
    TemplateInfo { key: "lightning_ic_demo", name: "Lightning Inside the Cloud", category: "Lightning", description: "Animated diagram: Most lightning stays in the cloud." },
    TemplateInfo { key: "lightning_leader", name: "Leader and Return Stroke", category: "Lightning", description: "Animated diagram: Stepped leader: faint, branching." },
    TemplateInfo { key: "lightning_pos_neg", name: "Positive vs Negative Lightning", category: "Lightning", description: "Animated diagram: Most strikes are negative." },
    TemplateInfo { key: "lightning_distance", name: "Lightning Reach", category: "Lightning", description: "Animated diagram: Lightning can strike 10 miles from the storm." },
    TemplateInfo { key: "lightning_indoors", name: "Safe Indoors", category: "Lightning", description: "Animated diagram: Stay off corded phones." },
    TemplateInfo { key: "lightning_outdoor", name: "Outdoor Events", category: "Lightning", description: "Animated diagram: Have a lightning plan for every event." },
    TemplateInfo { key: "lightning_water", name: "On the Water", category: "Lightning", description: "Animated diagram: Get off the water at the first thunder." },
    TemplateInfo { key: "lightning_timeline", name: "Lightning Risk Timeline", category: "Lightning", description: "Animated diagram: Before: risk as the storm approaches." },
    TemplateInfo { key: "lightning_thunder", name: "How Far Away?", category: "Lightning", description: "Animated diagram: Count seconds from flash to thunder." },
    TemplateInfo { key: "heat_illness", name: "Heat Exhaustion vs Heatstroke", category: "Safety", description: "Animated diagram: Exhaustion: heavy sweating, cool skin, nausea." },
    TemplateInfo { key: "heat_index_impact", name: "What the Heat Index Means", category: "Safety", description: "Animated diagram: 90s: caution with long exposure." },
    TemplateInfo { key: "heat_wet_bulb", name: "Wet-Bulb Temperature", category: "Safety", description: "Animated diagram: Sweat cools you only if it can evaporate." },
    TemplateInfo { key: "heat_timeline", name: "Heat Exposure", category: "Safety", description: "Animated diagram: Minutes: thirst and sweating." },
    TemplateInfo { key: "heat_car", name: "Hot Car Danger", category: "Safety", description: "Animated diagram: Car interiors heat up fast." },
    TemplateInfo { key: "heat_pets", name: "Pets in the Heat", category: "Safety", description: "Animated diagram: Never leave pets in a car." },
    TemplateInfo { key: "cold_frostbite", name: "Frostbite", category: "Safety", description: "Animated diagram: Wind chill below -18°F: frostbite in 30 minutes." },
    TemplateInfo { key: "cold_hypothermia", name: "Hypothermia", category: "Safety", description: "Animated diagram: Dress in layers, stay dry." },
    TemplateInfo { key: "cold_wind_chill", name: "Wind Chill", category: "Safety", description: "Animated diagram: Wind pulls heat from skin." },
    TemplateInfo { key: "cold_co", name: "Carbon Monoxide", category: "Safety", description: "Animated diagram: CO is invisible and odorless." },
    TemplateInfo { key: "sci_layers", name: "Layers of the Atmosphere", category: "Science", description: "Animated diagram: Troposphere: our weather." },
    TemplateInfo { key: "sci_jet", name: "The Jet Stream", category: "Science", description: "Animated diagram: Big temperature contrasts drive it." },
    TemplateInfo { key: "sci_high_low", name: "Highs and Lows", category: "Science", description: "Animated diagram: Highs: sinking air, fair weather." },
    TemplateInfo { key: "sci_sea_breeze", name: "Sea and Land Breezes", category: "Science", description: "Animated diagram: Land heats faster by day." },
    TemplateInfo { key: "sci_inversion", name: "Temperature Inversion", category: "Science", description: "Animated diagram: Usually air cools with height." },
    TemplateInfo { key: "sci_orographic", name: "Mountains Make Rain", category: "Science", description: "Animated diagram: Wind runs into a mountain." },
    TemplateInfo { key: "sci_rain_shadow", name: "Rain Shadow", category: "Science", description: "Animated diagram: Moisture rains out on the way up." },
    TemplateInfo { key: "sci_instability", name: "Unstable Air", category: "Science", description: "Animated diagram: A warm bubble rises." },
    TemplateInfo { key: "sci_clouds", name: "How Clouds Form", category: "Science", description: "Animated diagram: Air rises and expands." },
    TemplateInfo { key: "sci_coriolis", name: "The Coriolis Effect", category: "Science", description: "Animated diagram: Earth spins under moving air." },
    TemplateInfo { key: "tropical_track", name: "Hurricane Track", category: "Tropical", description: "Forecast track, cone and intensity from the National Hurricane Center." },
    TemplateInfo { key: "tropical_category", name: "Hurricane Categories", category: "Tropical", description: "Saffir-Simpson categories with the current storm." },
    TemplateInfo { key: "tropical_arrival", name: "Wind Arrival", category: "Tropical", description: "Earliest reasonable arrival of tropical-storm winds." },
    TemplateInfo { key: "tropical_prep", name: "Hurricane Preparedness", category: "Tropical", description: "Get ready checklist." },
    TemplateInfo { key: "tropical_evac", name: "Evacuation Zones", category: "Tropical", description: "Who should leave: load your county's zones in Map Studio > Custom data." },
    TemplateInfo { key: "tropical_rain", name: "Hurricane Rainfall", category: "Tropical", description: "WPC rainfall forecast for the storm." },
    TemplateInfo { key: "redflag", name: "Red Flag Warning", category: "Fire", description: "Red Flag Warning status with fire danger and nearest fires." },
    TemplateInfo { key: "fire_danger", name: "Fire Danger", category: "Fire", description: "Fosberg fire danger at home." },
    TemplateInfo { key: "fire_evac", name: "Wildfire Evacuation", category: "Fire", description: "Evacuation information for a nearby fire." },
    TemplateInfo { key: "aqi_today", name: "Air Quality", category: "Fire", description: "Air Quality Index with health advice." },
    TemplateInfo { key: "smoke_visibility", name: "Smoke and Visibility", category: "Fire", description: "Smoke levels and how far you can see." },
    TemplateInfo { key: "drought_status", name: "Drought Status", category: "Climate", description: "US Drought Monitor category at home." },
    TemplateInfo { key: "climate_daily", name: "Daily Climate Summary", category: "Climate", description: "Yesterday's numbers against normal and records." },
    TemplateInfo { key: "climate_month_temp", name: "Month So Far", category: "Climate", description: "Month-to-date temperature against normal." },
    TemplateInfo { key: "climate_month_precip", name: "Monthly Precipitation", category: "Climate", description: "Month-to-date precipitation against normal." },
    TemplateInfo { key: "climate_record_high", name: "Record High", category: "Climate", description: "Today's record high and normal." },
    TemplateInfo { key: "climate_record_low", name: "Record Low", category: "Climate", description: "Tonight's record low and normal." },
    TemplateInfo { key: "climate_snow_seasons", name: "Snowfall Compared", category: "Climate", description: "Season snowfall to date for the last 11 seasons." },
    TemplateInfo { key: "climate_ytd", name: "Year-to-Date Precipitation", category: "Climate", description: "Cumulative precipitation this year against normal." },
    TemplateInfo { key: "climate_normals", name: "Normals Compared", category: "Climate", description: "Today against the 30-year normal and records." },
    TemplateInfo { key: "climate_this_day", name: "On This Day", category: "Climate", description: "The most notable weather on this date." },
    TemplateInfo { key: "flood_timeline", name: "Flood Timeline", category: "Flooding", description: "Warnings, heavy rain timing and river crests in order." },
    TemplateInfo { key: "flood_reports", name: "Flood Reports and Rainfall", category: "Flooding", description: "Today's flood reports with the rain that fell there." },
    TemplateInfo { key: "camera_wall", name: "Camera Wall", category: "Travel", description: "Four live cameras with their names, conditions and credits." },
    TemplateInfo { key: "camera_warning", name: "Camera Near the Warning", category: "Severe", description: "The camera nearest the most serious warning, picked automatically." },
    TemplateInfo { key: "viewer_photo", name: "Viewer Photo", category: "Community", description: "The newest approved viewer photo with credit." },
    TemplateInfo { key: "viewer_wall", name: "Viewer Photo Wall", category: "Community", description: "Four approved viewer photos with credits." },
    TemplateInfo { key: "viewer_poll", name: "Viewer Poll", category: "Community", description: "Live poll results from the viewer portal." },
    TemplateInfo { key: "viewer_question", name: "Viewer Question", category: "Community", description: "The viewer question on air." },
    TemplateInfo { key: "hist_timeline", name: "Event Timeline", category: "History", description: "The loaded historical event, warning by warning and report by report." },
    TemplateInfo { key: "hist_forecast", name: "Forecast vs What Happened", category: "History", description: "Archived model forecast against the observed temperature for the event." },
    TemplateInfo { key: "ens_spaghetti", name: "Ensemble Spaghetti", category: "Forecast", description: "Every GFS ensemble member's temperature for the week, the average bold." },
    TemplateInfo { key: "ens_plume", name: "Temperature Plume", category: "Forecast", description: "Ensemble temperature range bands for the week." },
    TemplateInfo { key: "ens_snow_range", name: "Snow Range", category: "Forecast", description: "How much snow the ensemble members show, with the likely range." },
    TemplateInfo { key: "ens_scenarios", name: "Best / Likely / Worst Case", category: "Forecast", description: "Scenario comparison from the ensemble: 10th, 50th and 90th percentiles." },
    TemplateInfo { key: "ens_confidence", name: "Confidence by Day", category: "Forecast", description: "Forecast confidence for each day from the ensemble spread." },
    TemplateInfo { key: "model_agreement", name: "Model Agreement", category: "Forecast", description: "GFS, ECMWF and ICON highs side by side, with how closely they agree." },
    TemplateInfo { key: "severe_ingredients", name: "Severe Ingredients", category: "Severe", description: "Instability, shear, helicity, STP and SCP from the model sounding at home." },
    TemplateInfo { key: "hodograph", name: "Hodograph", category: "Severe", description: "Live hodograph with storm motion, helicity and shear." },
    TemplateInfo { key: "sounding", name: "Sounding", category: "Severe", description: "Live temperature / dew point profile with the parcel path." },
    TemplateInfo { key: "wind_profile", name: "Wind Profile", category: "Forecast", description: "Winds by height for the next 24 hours (time-height display)." },
    TemplateInfo { key: "winter_ingredients", name: "Winter Ingredients", category: "Forecast", description: "Freezing level, wet-bulb zero, snow ratio, road icing and squall timing." },
    TemplateInfo { key: "winter_travel", name: "Winter Travel", category: "Travel", description: "Road hazards, ice and snow timing for drivers." },
    TemplateInfo { key: "explain_thunderstorm", name: "How Thunderstorms Form", category: "Explainers", description: "Animated four-step explainer: how thunderstorms form." },
    TemplateInfo { key: "explain_cold_front", name: "Cold Front Explained", category: "Explainers", description: "Animated four-step explainer: cold front explained." },
    TemplateInfo { key: "explain_lake_effect", name: "How Lake-Effect Snow Forms", category: "Explainers", description: "Animated four-step explainer: how lake-effect snow forms." },
    TemplateInfo { key: "explain_fog", name: "Why Fog Forms", category: "Explainers", description: "Animated four-step explainer: why fog forms." },
    TemplateInfo { key: "explain_hail", name: "How Hail Grows", category: "Explainers", description: "Animated four-step explainer: how hail grows." },
    TemplateInfo { key: "severe_threats", name: "Severe Threats", category: "Severe", description: "Tornado, hail, wind and flooding threat at home as bars on the Marginal-to-High scale." },
    TemplateInfo { key: "severe_threats_blocks", name: "Severe Threats (blocks)", category: "Severe", description: "The same threats as ten-block meters." },
    TemplateInfo { key: "severe_threats_tomorrow", name: "Severe Threats Tomorrow", category: "Severe", description: "Tomorrow's tornado, hail, wind and flooding threat at home." },
    TemplateInfo { key: "risk_levels", name: "Severe Risk Levels", category: "Severe", description: "The five SPC risk levels: should you change plans, how often to check, impacts." },
    TemplateInfo { key: "risk_meaning", name: "What the Risk Levels Mean", category: "Severe", description: "Five coloured cards explaining Marginal to High risk." },
    TemplateInfo { key: "severe_day", name: "Storms Possible Today", category: "Severe", description: "Risk level at home, potential hazards, most likely timing and the 1-5 scale." },
    TemplateInfo { key: "severe_day_tomorrow", name: "Storms Possible Tomorrow", category: "Severe", description: "Tomorrow's risk level, hazards and the 1-5 scale." },
    TemplateInfo { key: "severe_risk_bug", name: "Severe Risk Bug", category: "Overlays", description: "Corner bug with today's risk level and meter, for over the outlook map (overlay)." },    TemplateInfo { key: "wsw_criteria", name: "Winter Storm Warning Criteria", category: "Winter", description: "Indiana county map of the snow needed for a Winter Storm Warning: 6 inches north, 5 central, 4 far south (NWS Indianapolis)." },
    TemplateInfo { key: "storm_timing", name: "Storm Timing Map", category: "Severe", description: "County map shaded by when storms arrive (next hour, 1-2 hours, 2-3 hours) from the storms' current movement." },
    TemplateInfo { key: "severe_day_map", name: "Severe Storms Possible (map)", category: "Severe", description: "County risk map beside the tornado, wind, hail and flooding meters and the timing." },
    TemplateInfo { key: "severe_day_map_tomorrow", name: "Severe Storms Possible Tomorrow (map)", category: "Severe", description: "Tomorrow's county risk map with the hazard meters." },
    TemplateInfo { key: "alert_day", name: "Weather Alert Day", category: "Severe", description: "Bold red card for a day with a severe weather risk: the day and three plain lines from the SPC outlook." },
    TemplateInfo { key: "alert_day_tomorrow", name: "Weather Alert Day Tomorrow", category: "Severe", description: "The Weather Alert Day card for tomorrow." },
    TemplateInfo { key: "risks_overview", name: "Severe Risks Overview", category: "Severe", description: "Four county maps: flooding, wind, hail and tornado chances today, with their scales." },
    TemplateInfo { key: "risks_overview_tomorrow", name: "Severe Risks Overview Tomorrow", category: "Severe", description: "The four-map risk overview for tomorrow." },
    TemplateInfo { key: "risk_map", name: "Severe Risk Map", category: "Severe", description: "Large county map of today's overall severe risk with the level scale." },
    TemplateInfo { key: "risk_map_tomorrow", name: "Severe Risk Map Tomorrow", category: "Severe", description: "Large county map of tomorrow's overall severe risk." },
    TemplateInfo { key: "holiday_new_year", name: "New Year's Day Forecast", category: "Holidays", description: "Themed card with the forecast for New Year's Day: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_mlk", name: "Martin Luther King Jr. Day Forecast", category: "Holidays", description: "Themed card with the forecast for Martin Luther King Jr. Day: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_valentines", name: "Valentine's Day Forecast", category: "Holidays", description: "Themed card with the forecast for Valentine's Day: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_presidents", name: "Presidents' Day Forecast", category: "Holidays", description: "Themed card with the forecast for Presidents' Day: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_st_patricks", name: "St. Patrick's Day Forecast", category: "Holidays", description: "Themed card with the forecast for St. Patrick's Day: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_easter", name: "Easter Forecast", category: "Holidays", description: "Themed card with the forecast for Easter: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_mothers", name: "Mother's Day Forecast", category: "Holidays", description: "Themed card with the forecast for Mother's Day: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_memorial", name: "Memorial Day Forecast", category: "Holidays", description: "Themed card with the forecast for Memorial Day: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_fathers", name: "Father's Day Forecast", category: "Holidays", description: "Themed card with the forecast for Father's Day: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_juneteenth", name: "Juneteenth Forecast", category: "Holidays", description: "Themed card with the forecast for Juneteenth: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_independence", name: "Fourth of July Forecast", category: "Holidays", description: "Themed card with the forecast for Fourth of July: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_labor", name: "Labor Day Forecast", category: "Holidays", description: "Themed card with the forecast for Labor Day: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_halloween", name: "Halloween Forecast", category: "Holidays", description: "Themed card with the forecast for Halloween: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_veterans", name: "Veterans Day Forecast", category: "Holidays", description: "Themed card with the forecast for Veterans Day: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_thanksgiving", name: "Thanksgiving Forecast", category: "Holidays", description: "Themed card with the forecast for Thanksgiving: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_christmas_eve", name: "Christmas Eve Forecast", category: "Holidays", description: "Themed card with the forecast for Christmas Eve: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_christmas", name: "Christmas Forecast", category: "Holidays", description: "Themed card with the forecast for Christmas: icon, high, rain chance and conditions." },
    TemplateInfo { key: "holiday_new_years_eve", name: "New Year's Eve Forecast", category: "Holidays", description: "Themed card with the forecast for New Year's Eve: icon, high, rain chance and conditions." },
];

pub fn build(key: &str) -> Option<GraphicDoc> {
    let info = TEMPLATES_EXT.iter().find(|t| t.key == key)?;
    let mut doc = match key {
        "backup" => backup(),
        "storm_reports" => storm_reports(),
        "spc_probs" => spc_probs(),
        "severe_timing" => severe_timing(),
        "lightning_safety" => lightning_safety(),
        "cell_briefing" => cell_briefing(),
        "ef_scale" => ef_scale(),
        "watch_warning" => watch_warning(),
        "damage_survey" => damage_survey(),
        "hourly_strip" => hourly_strip(),
        "day_parts" => day_parts(),
        "weekend" => weekend(),
        "hourly_temp_chart" => chart_doc("Temperature Trend", "hourly.temp", "hourly.feels", ChartKind::Line, "°", "Temperature and feels like, next 24 hours"),
        "wind_timeline" => wind_timeline(),
        "rain_chances" => chart_doc("Rain Chances", "day.precip", "", ChartKind::Bar, "%", "Chance of rain each day"),
        "rain_totals" => rain_totals(),
        "winter_timeline" => winter_timeline(),
        "pressure_trend" => pressure_trend(),
        "cities_now" => cities_now(),
        "obs_dashboard" => obs_dashboard(),
        "sun_moon" => sun_moon(),
        "season_countdown" => season_countdown(),
        "river_stages" => river_stages(),
        "hydrograph" => hydrograph(),
        "excessive_rain" => excessive_rain(),
        "travel" => travel(),
        "commute" => commute(),
        "closings" => closings(),
        "closings_crawl" => closings_crawl(),
        "centers" => centers(),
        "community_forecast" => community_forecast(),
        "sponsor_full" => sponsor_full(),
        "current_hc" => current_hc(),
        "seven_day_es" => seven_day_es(),
        "current_es" => current_es(),
        "heat_index" => heat_index(),
        "wind_chill" => wind_chill(),
        "visibility_now" => visibility_now(),
        "sky_now" => sky_now(),
        "dew_humidity" => dew_humidity(),
        "temp_departure" => temp_departure(),
        "rain_departure" => rain_departure(),
        "hourly_rain" => hourly_rain(),
        "wind_advisory" => wind_advisory(),
        "beaufort" => beaufort(),
        "snow_ratio" => snow_ratio(),
        "blizzard" => blizzard(),
        "hazard_icons" => hazard_icons(),
        "confidence" => confidence(),
        "tds_explainer" => tds_explainer(),
        "warning_history" => warning_history(),
        "power_counter" => power_counter(),
        "nowcast_threats" => stat_table(
            "Next Hour",
            "Storm nowcast for {{location}}", "Source: radar nowcast",
            &[
                ("Storm reaching {{location}}", "{{nowcast.arrival}} • {{nowcast.eta}}"),
                ("Hail threat", "{{nowcast.hail}}"),
                ("Damaging wind threat", "{{nowcast.wind}}"),
                ("Heavy rain threat", "{{nowcast.rain}}"),
                ("New storms", "{{nowcast.initiation}}"),
            ],
        ),
        "power_counties" => {
            let rows: Vec<(String, String)> = (1..=7).map(|i| (format!("{{{{power.top{i}}}}}"), format!("{{{{power.top{i}.count}}}} • {{{{power.top{i}.pct}}}}"))).collect();
            let refs: Vec<(&str, &str)> = rows.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
            power_doc(stat_table("Outages by County", "{{power.total}} customers without power", "Source: utility outage maps", &refs))
        }
        "power_utilities" => {
            let rows: Vec<(String, String)> = (1..=5).map(|i| (format!("{{{{power.utility{i}}}}}"), format!("{{{{power.utility{i}.count}}}} • {{{{power.utility{i}.pct}}}}"))).collect();
            let refs: Vec<(&str, &str)> = rows.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
            power_doc(stat_table("Outages by Utility", "{{power.total}} customers without power", "Source: utility outage maps", &refs))
        }
        "power_trend" => power_doc(chart_doc("Outage Trend", "power.history", "", ChartKind::Area, "", "Customers without power • {{power.trend}}")),
        "power_etr" => {
            let rows: Vec<(String, String)> = (1..=7).map(|i| (format!("{{{{power.top{i}}}}}"), format!("{{{{power.top{i}.etr}}}}"))).collect();
            let refs: Vec<(&str, &str)> = rows.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
            power_doc(stat_table("Restoration Estimates", "Utility estimates by county", "Source: utility restoration estimates", &refs))
        }
        "power_restoration" => {
            let rows: Vec<(String, String)> = (1..=7).map(|i| (format!("{{{{power.top{i}}}}}"), format!("{{{{power.top{i}.restored}}}} restored"))).collect();
            let refs: Vec<(&str, &str)> = rows.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
            power_doc(stat_table("Restoration Progress", "{{power.restored}} restored since the peak of {{power.peak}}", "Source: utility outage maps", &refs))
        }
        "power_safety" => power_doc(stat_table(
            "Power Outage Safety",
            "Stay safe until the power is back", "Safety advice: Ready.gov / NWS",
            &[
                ("Generators", "Outside only, 20 ft from doors and windows"),
                ("Food", "Keep the fridge closed: 4 hours, freezer 48"),
                ("Heat / cold", "Go to a warming or cooling center"),
                ("Report it", "Call your utility, don't assume they know"),
                ("Traffic lights out", "Treat as a four-way stop"),
            ],
        )),
        "power_line_safety" => power_doc(diagram_explainer(
            "Downed Power Lines",
            "downed_line",
            &["Stay at least 35 feet away and keep others back", "Never touch anything in contact with the line", "Call 911 and your utility to report it"],
        )),
        "power_facilities" => power_doc(stat_table(
            "Critical Facilities",
            "In counties with 10%+ of customers out", "Source: utility outage maps",
            &[("Facilities affected", "{{power.facilities}}"), ("Most outages", "{{power.top1}} • {{power.top1.count}}"), ("Estimated restoration", "{{power.etr}}")],
        )),
        "photo_grid" => photo_grid(),
        "impact_summary" => impact_summary(),
        "urban_flood" => urban_flood(),
        "black_ice" => black_ice(),
        "low_visibility" => low_visibility(),
        "airport_delays" => airport_delays(),
        "school_transport" => school_transport(),
        "closings_business" => closings_business(),
        "closings_government" => closings_government(),
        "closings_events" => closings_events(),
        "emergency_declaration" => emergency_declaration(),
        "community_dashboard" => community_dashboard(),
        "shelters" => shelters(),
        "workweek" => workweek(),
        "ten_day" => ten_day(),
        "outdoor_planner" => outdoor_planner(),
        "impact_by_time" => impact_by_time(),
        "confidence_timeline" => confidence_timeline(),
        "meteor" => meteor(),
        "eclipse" => eclipse(),
        "aurora" => aurora(),
        "frost_dates" => frost_dates(),
        "growing_season" => growing_season(),
        "night_sky" => night_sky(),
        "model_compare" => model_compare(),
        "winter_travel" => winter_travel(),
        "tropical_track" => stat_table("{{tropical.name}}", "Latest advisory", "Source: National Hurricane Center", &[("Strength", "{{tropical.category}}"), ("Winds", "{{tropical.wind}}"), ("Next", "{{tropical.next}}"), ("Here", "{{tropical.arrival}}")]),
        "tropical_category" => stat_table("Hurricane Categories", "{{tropical.name}}: {{tropical.category}}", "Saffir-Simpson scale · National Hurricane Center", &[("Category 1", "74-95 mph: some damage"), ("Category 2", "96-110 mph: extensive damage"), ("Category 3", "111-129 mph: devastating"), ("Category 4", "130-156 mph: catastrophic"), ("Category 5", "157+ mph: catastrophic")]),
        "tropical_arrival" => stat_table("When the Wind Arrives", "{{tropical.name}}", "Source: National Hurricane Center", &[("Here", "{{tropical.arrival}}"), ("Storm", "{{tropical.category}}, {{tropical.wind}}"), ("Before then", "Finish preparations, secure loose objects")]),
        "tropical_prep" => diagram_explainer("Hurricane Prep", "emergency_kit", &["Know your evacuation zone and route", "Water, food and medicine for a week", "Secure your home before the wind arrives"]),
        "tropical_evac" => stat_table("Evacuation Zones", "{{tropical.name}}", "Zones: your county emergency management", &[("Zone A", "Leave first: surge areas"), ("Zone B", "Leave if ordered"), ("Everyone", "Know your zone at your county EMA site")]),
        "tropical_rain" => chart_doc("Hurricane Rainfall", "ens.rain72", "", ChartKind::Distribution, "\"", "{{ens.rain_range}}"),
        "redflag" => stat_table("Fire Weather", "{{redflag.text}}", "Source: National Weather Service · model data: NOAA GFS", &[("Fire danger", "{{fire.danger}}"), ("Nearest fire", "{{fire.nearest}}"), ("Do", "Avoid outdoor burning and sparks")]),
        "fire_danger" => stat_table("Fire Danger", "{{location}}", "Model data: NOAA GFS", &[("Today", "{{fire.danger}}"), ("Humidity", "{{humidity}}"), ("Wind", "{{wind}}")]),
        "fire_evac" => stat_table("Wildfire Evacuation", "{{fire.nearest}}", "Source: National Weather Service", &[("Go", "Leave when told: don't wait"), ("Take", "Medicines, documents, pets"), ("Info", "County emergency management")]),
        "aqi_today" => stat_table("Air Quality", "{{location}}", "Air quality: Open-Meteo (CAMS)", &[("AQI", "{{aqi.home}}"), ("Category", "{{aqi.category}}"), ("Advice", "{{aqi.advice}}")]),
        "smoke_visibility" => stat_table("Smoke", "{{location}}", "Air quality: Open-Meteo (CAMS)", &[("Air quality", "{{aqi.home}} ({{aqi.category}})"), ("Health", "{{aqi.advice}}"), ("Driving", "Use low beams in thick smoke")]),
        "drought_status" => stat_table("Drought", "{{location}} · {{drought.valid}}", "Source: U.S. Drought Monitor · outlook: NOAA CPC", &[("Here", "{{drought.home}}"), ("Next month", "{{drought.outlook_month}}"), ("Rain this month", "{{clim.month_precip}}"), ("This year", "{{clim.ytd_precip}}")]),
        "climate_daily" => stat_table("Yesterday", "{{location}}", "{{clim.source}}", &[("Observed", "{{clim.yesterday}}"), ("Normal", "{{climate.normal_hi}} / {{climate.normal_lo}}"), ("Records", "{{climate.record_hi}} / {{climate.record_lo}}")]),
        "climate_month_temp" => stat_table("This Month So Far", "{{location}}", "{{clim.source}}", &[("Temperature", "{{clim.month_temp}}"), ("Precipitation", "{{clim.month_precip}}")]),
        "climate_month_precip" => stat_table("Precipitation This Month", "{{location}}", "{{clim.source}}", &[("So far", "{{clim.month_precip}}"), ("Year to date", "{{clim.ytd_precip}}")]),
        "climate_record_high" => stat_table("Record High", "{{location}}", "{{clim.source}}", &[("Record", "{{climate.record_hi}}"), ("Normal", "{{climate.normal_hi}}"), ("Forecast", "{{next.hi}}")]),
        "climate_record_low" => stat_table("Record Low", "{{location}}", "{{clim.source}}", &[("Record", "{{climate.record_lo}}"), ("Normal", "{{climate.normal_lo}}"), ("Forecast", "{{next.lo}}")]),
        "climate_snow_seasons" => chart_doc("Snowfall So Far", "clim.snow_seasons", "", ChartKind::Bar, "\"", "{{clim.season_snow}}"),
        "climate_ytd" => chart_doc("Precipitation This Year", "clim.ytd", "clim.ytd_normal", ChartKind::Line, "\"", "{{clim.ytd_precip}}"),
        "climate_normals" => stat_table("Normals", "{{location}}", "{{clim.source}}", &[("Normal high", "{{climate.normal_hi}}"), ("Normal low", "{{climate.normal_lo}}"), ("Record high", "{{climate.record_hi}}"), ("Record low", "{{climate.record_lo}}")]),
        "climate_this_day" => stat_table("On This Day", "{{location}}", "{{clim.source}}", &[("History", "{{clim.this_day}}"), ("Forecast", "{{next.hi}} / {{next.lo}}")]),
        "camera_wall" => camera_wall(),
        "flood_timeline" => stat_table("Flood Timeline", "{{location}}", "Source: National Weather Service · NOAA GFS", &[("1", "{{flood.t1}}"), ("2", "{{flood.t2}}"), ("3", "{{flood.t3}}"), ("4", "{{flood.t4}}"), ("5", "{{flood.t5}}"), ("6", "{{flood.t6}}")]),
        "flood_reports" => stat_table("Flood Reports", "{{floodrep.count}}", "Reports: NWS local storm reports · rain: NOAA GFS", &[("1", "{{floodrep1}}"), ("2", "{{floodrep2}}"), ("3", "{{floodrep3}}"), ("4", "{{floodrep4}}"), ("5", "{{floodrep5}}")]),
        "camera_warning" => {
            let mut d = camera_view();
            for l in &mut d.layers {
                match &mut l.kind {
                    LayerKind::Image { media, .. } => *media = "{{camera.warning.image}}".into(),
                    LayerKind::Text { text, .. } if text.contains("camera1.name") => *text = "{{camera.warning.name}}".into(),
                    _ => {}
                }
            }
            d
        }
        "viewer_photo" => viewer_photo(),
        "viewer_wall" => viewer_wall(),
        "viewer_poll" => chart_doc("{{poll.question}}", "poll.results", "", ChartKind::Bar, "%", "{{poll.total}} • vote at our website"),
        "viewer_question" => stat_table("Your Question", "{{question.from}}", "Send yours at our website", &[("Q", "{{question.text}}")]),
        "hist_timeline" => stat_table("{{hist.name}}", "{{hist.date}} • {{hist.counts}}", "Source: NWS warnings and storm reports", &[("1", "{{hist.t1}}"), ("2", "{{hist.t2}}"), ("3", "{{hist.t3}}"), ("4", "{{hist.t4}}"), ("5", "{{hist.t5}}"), ("6", "{{hist.t6}}"), ("7", "{{hist.t7}}")]),
        "hist_forecast" => chart_doc("Forecast vs What Happened", "hist.obs_temp", "hist.fcst_temp", ChartKind::Line, "°", "{{hist.rain_vs_forecast}}"),
        "ens_spaghetti" => chart_doc("Ensemble Spaghetti", "ens.temp", "", ChartKind::Spaghetti, "°", "Each thin line is one possible forecast"),
        "ens_plume" => chart_doc("Temperature Plume", "ens.temp", "", ChartKind::Plume, "°", "Shaded: where most forecasts fall"),
        "ens_snow_range" => {
            let mut d = chart_doc("Snow Range", "ens.snow72", "", ChartKind::Distribution, "\"", "{{ens.snow_range}}");
            d.name = "Snow Range".into();
            d
        }
        "ens_scenarios" => stat_table("Best, Likely and Worst Case", "Next 72 hours", "Model data: GFS ensemble (Open-Meteo)", &[("Best case", "{{ens.best}}"), ("Most likely", "{{ens.likely}}"), ("Worst case", "{{ens.worst}}"), ("Snow range", "{{ens.snow_range}}"), ("Rain range", "{{ens.rain_range}}")]),
        "ens_confidence" => chart_doc("Confidence by Day", "ens.confidence", "", ChartKind::Bar, "%", "From how much the ensemble members agree"),
        "model_agreement" => chart_doc("Model Agreement", "models.gfs", "models.ecmwf", ChartKind::Line, "°", "{{models.agreement}}"),
        "severe_ingredients" => stat_table("Severe Ingredients", "Model sounding at home", "Model sounding (Open-Meteo)", &[("Instability (MLCAPE)", "{{snd.mlcape}}"), ("Low-level helicity (0-1 km)", "{{snd.srh01}}"), ("Deep shear (0-6 km)", "{{snd.shear06}}"), ("Cloud base", "{{snd.lcl}}"), ("Tornado parameter (STP)", "{{snd.stp}}"), ("Supercell composite (SCP)", "{{snd.scp}}"), ("Storm motion", "{{snd.storm_motion}}")]),
        "hodograph" => diagram_explainer("Hodograph", "hodograph", &["A curved line: storms that can spin", "Bigger loops: more spin in the wind", "White dot: where spinning storms tend to move"]),
        "sounding" => diagram_explainer("Sounding", "skewt", &["Red: temperature, green: dew point", "White: a bubble of air rising", "Bubble warmer than the air around it: storms can grow"]),
        "wind_profile" => diagram_explainer("Wind Profile", "wind_profile", &["Arrows show wind at each height", "Colour shows speed", "Wind changing with height can make storms spin"]),
        "winter_ingredients" => stat_table("Winter Ingredients", "At home", "Model sounding (Open-Meteo)", &[("Freezing level", "{{snd.freezing}}"), ("Wet-bulb zero", "{{snd.wbz}}"), ("Snow-to-liquid ratio", "{{winter.ratio}}"), ("Roads", "{{winter.road_icing}}"), ("Snow squalls", "{{winter.squall_time}}"), ("Lowest inversion", "{{snd.inversion}}")]),
        "safe_tornado_shelter" => diagram_explainer("Where to Shelter", "shelter_house", &["Go to the lowest floor", "Pick an interior room away from windows", "Cover your head and neck"]),
        "safe_mobile_home" => diagram_explainer("Mobile Home Safety", "shelter_mobile", &["Mobile homes are not safe in a tornado", "Know your nearest sturdy shelter now", "Leave when a watch is issued, not the warning"]),
        "safe_vehicle" => diagram_explainer("Caught in a Car", "shelter_vehicle", &["Drive at right angles to the storm if you can", "If not, get out and lie in a low spot", "Never shelter under an overpass"]),
        "safe_lightning" => diagram_explainer("Lightning Safety", "lightning_cg", &["When thunder roars, go indoors", "A car with a metal roof is also safe", "Wait 30 minutes after the last thunder"]),
        "safe_flooded_road" => diagram_explainer("Flooded Roads", "flood_depth", &["Six inches of water can knock you down", "A foot of water can float a car", "Turn around, don't drown"]),
        "safe_generator" => diagram_explainer("Generator Safety", "generator_co", &["Run generators outdoors only", "At least 20 feet from doors and windows", "Install a battery CO alarm"]),
        "safe_power_line" => diagram_explainer("Downed Power Lines", "power_line", &["Assume every downed line is live", "Stay at least 35 feet away", "Call the utility or 911"]),
        "safe_heat" => diagram_explainer("Beat the Heat", "heat_body", &["Drink water before you are thirsty", "Rest in the shade or air conditioning", "Check on neighbors and pets"]),
        "safe_winter_kit" => diagram_explainer("Winter Storm Kit", "emergency_kit", &["Water, food and medicine for 3 days", "Flashlight, radio and batteries", "Blankets and a phone charger"]),
        "safe_plan" => diagram_explainer("Your Severe Weather Plan", "shelter_house", &["Pick your safe place at home and work", "Have two ways to get warnings", "Practice with everyone in the house"]),
        "edu_radar" => diagram_explainer("How Radar Works", "radar_beam", &["The radar sends out a pulse", "Rain and hail bounce some back", "Timing the echo tells the distance"]),
        "edu_ref_vel" => diagram_explainer("Reflectivity vs Velocity", "radar_velocity", &["Bright colors: heavier rain or hail", "Wind colors: green toward the radar, red away", "Green next to red can mean rotation"]),
        "edu_cc" => diagram_explainer("Spotting Debris on Radar", "radar_cc", &["Raindrops all look alike to the radar", "Debris is a jumble of shapes and sizes", "That jumble inside a spinning storm can mean a tornado is on the ground"]),
        "edu_tornado_forms" => diagram_explainer("How Tornadoes Form", "supercell", &["Winds change with height and spin the air", "Strong rising air tips the spin on end", "The rotation tightens and reaches the ground"]),
        "edu_supercell" => diagram_explainer("Inside a Supercell", "rfd", &["The storm's rising air starts to spin", "A wall cloud forms under it", "Cool air sinking behind the storm wraps around it"]),
        "edu_fronts" => diagram_explainer("Fronts", "fronts", &["Cold fronts push in fast and lift warm air", "Storms often form along the front", "Warm fronts slide up slowly with steady rain"]),
        "edu_lightning" => diagram_explainer("How Lightning Forms", "lightning_cg", &["Bumping ice builds up electric charge in the cloud", "A faint channel reaches down from the cloud", "When it connects, the bright flash shoots back up"]),
        "edu_hail" => diagram_explainer("How Hail Grows", "hail", &["Strong rising air carries raindrops up where it is below freezing", "Layers of ice build up on each trip", "It falls when it gets too heavy to hold up"]),
        "edu_precip_types" => diagram_explainer("Rain, Sleet, Ice or Snow?", "precip_types", &["All cold: snow", "Warm layer, cold below: sleet or freezing rain", "Warm all the way: rain"]),
        "edu_watch_warning" => diagram_explainer("Watch, Warning, Advisory", "alert_terms", &["Advisory: be careful", "Watch: be prepared", "Warning: take action now"]),
        "demo_tornado_formation" => diagram_explainer("Tornado Formation", "supercell", &["Winds blowing different ways set the air spinning", "Rising air stretches and tightens the spin", "The tornado touches down"]),
        "demo_tornado_life" => diagram_explainer("A Tornado's Life", "tornado_lifecycle", &["Forms under the wall cloud", "Matures and widens", "Thins out and ends"]),
        "demo_updraft" => diagram_explainer("The Rotating Updraft", "supercell", &["Warm, moist air streams in", "It rises and rotates", "Forecasters call this a mesocyclone"]),
        "demo_wall_cloud" => diagram_explainer("The Wall Cloud", "wall_cloud", &["Cool air from the rain gets pulled into the storm", "It forms a cloud lower than the rest of the storm", "A lowered, rotating wall cloud forms"]),
        "demo_funnel" => diagram_explainer("Funnel Clouds", "funnel_cloud", &["Rotation tightens below the wall cloud", "Moisture in the spinning air makes the funnel visible", "On the ground, it is a tornado"]),
        "demo_debris" => diagram_explainer("The Debris Cloud", "debris_cloud", &["Tornado winds rush in and lift things up", "Debris swirls in a circle", "Radar can see the debris in the air"]),
        "demo_rfd" => diagram_explainer("Rear-Flank Downdraft", "rfd", &["Cool air sinks behind the storm", "It wraps around the spinning part of the storm", "It can tighten rotation into a tornado"]),
        "demo_hook" => diagram_explainer("The Hook Echo", "hook_echo", &["Rain wraps around the rotation", "On radar it looks like a hook", "The tornado threat is near the tip"]),
        "demo_tornado_damage" => diagram_explainer("Tornado Damage", "tornado_vs_straight", &["EF0-EF1: roofs and trees", "EF2-EF3: homes destroyed", "EF4-EF5: well-built homes swept away"]),
        "demo_tornado_vs_wind" => diagram_explainer("Tornado or Straight-Line Wind?", "tornado_vs_straight", &["Tornado: debris points in many directions", "Straight-line: damage all one way", "Both can be just as destructive"]),
        "shelter_interior" => diagram_explainer("Interior Room", "shelter_house", &["Lowest floor", "Small interior room or closet", "Put as many walls between you and outside"]),
        "shelter_basement" => diagram_explainer("Basement Shelter", "shelter_house", &["Go to the basement", "Get under a sturdy table or stairs", "Stay away from heavy objects above"]),
        "shelter_hallway" => diagram_explainer("Interior Hallway", "shelter_school", &["Lowest floor, center of the building", "Away from glass and doors", "Crouch and cover your head"]),
        "shelter_mobile_evac" => diagram_explainer("Leave the Mobile Home", "shelter_mobile", &["Have a plan before storms", "Go when a watch is issued", "A sturdy building nearby is best"]),
        "shelter_manufactured" => diagram_explainer("Why Manufactured Homes Are at Risk", "shelter_mobile", &["Light frames can roll or come apart", "Even tie-downs can fail in strong winds", "Find sturdy shelter early"]),
        "shelter_school_drill" => diagram_explainer("Tornado Safety at School", "shelter_school", &["Follow the drill", "Interior hallway on the lowest floor", "Kneel and cover your head"]),
        "shelter_work" => diagram_explainer("Tornado Safety at Work", "shelter_school", &["Know the designated shelter area", "Stay away from large open rooms", "Account for everyone afterward"]),
        "shelter_vehicle_decision" => diagram_explainer("Driving When a Tornado Hits", "shelter_vehicle", &["Can you drive away at right angles? Do it", "If not, park out of traffic", "Get below the windows or into a ditch"]),
        "shelter_outdoor" => diagram_explainer("Caught Outside", "shelter_outdoors", &["Get to a sturdy building if you can", "If not, lie flat in a low spot", "Cover your head and neck"]),
        "shelter_kit" => diagram_explainer("Tornado Kit", "emergency_kit", &["Helmets and sturdy shoes", "Flashlight, radio, batteries", "Water and medicines"]),
        "hur_structure" => diagram_explainer("Hurricane Structure", "hurricane", &["Rain bands spiral inward", "The eyewall has the strongest winds", "The eye is calm and clear"]),
        "hur_eye" => diagram_explainer("The Eye and Eyewall", "hurricane", &["Air sinks in the eye", "Air races upward in the eyewall", "The calm eye is followed by the other wall"]),
        "hur_wind" => diagram_explainer("Hurricane Winds", "hurricane", &["Winds spin counterclockwise", "The right side is strongest", "Tropical storm winds reach far from the center"]),
        "hur_surge_level" => diagram_explainer("Storm Surge", "storm_surge", &["Wind pushes water toward the coast", "Low pressure raises the sea", "Waves ride on top of the surge"]),
        "hur_surge_coast" => diagram_explainer("Coastal Flooding", "storm_surge", &["Surge floods low-lying areas first", "Water can move miles inland", "Evacuate from surge zones"]),
        "hur_rain_flood" => diagram_explainer("Hurricane Rain Floods", "flash_flood", &["Slow storms drop the most rain", "Rivers rise for days after", "Inland flooding is a leading killer"]),
        "hur_categories" => diagram_explainer("Hurricane Categories", "hurricane_categories", &["Category 1-2: damaging winds", "Category 3-4: devastating damage", "Category 5: catastrophic"]),
        "hur_evac_timeline" => diagram_explainer("Evacuation Timeline", "evac_timeline", &["3 days: check your plan and supplies", "2 days: evacuate if ordered", "Tropical storm winds: shelter in place"]),
        "hur_intensify" => diagram_explainer("How Hurricanes Strengthen", "hurricane", &["Warm ocean water fuels the storm", "Calm winds high up let it organize", "It can strengthen quickly, sometimes in a day"]),
        "hur_landfall" => diagram_explainer("Landfall", "storm_surge", &["Surge arrives before the center", "The eyewall brings the strongest winds", "Flooding rain spreads inland"]),
        "flood_flash" => diagram_explainer("How Flash Floods Happen", "flash_flood", &["Heavy rain falls fast", "Creeks rise in minutes", "Move to higher ground immediately"]),
        "flood_urban" => diagram_explainer("Street Flooding", "urban_flood", &["Pavement can't absorb the rain", "Drains overflow quickly", "Underpasses fill first"]),
        "flood_river" => diagram_explainer("River Flooding", "flash_flood", &["Rain upstream raises the river", "It spills into the floodplain", "Crests can come days later"]),
        "flood_vehicle" => diagram_explainer("Water vs Your Car", "flood_depth", &["6 inches: stalls many cars", "1 foot: floats a car", "2 feet: sweeps away SUVs and trucks"]),
        "flood_human" => diagram_explainer("Water vs You", "flood_depth", &["6 inches of moving water knocks you down", "Water hides holes and debris", "Never walk into floodwater"]),
        "flood_low_water" => diagram_explainer("Low-Water Crossings", "low_water", &["The road may be washed out under the water", "It only takes a little water to float a car", "Turn around, don't drown"]),
        "flood_basement" => diagram_explainer("Basement Flooding", "urban_flood", &["Water seeps in through walls and drains", "Turn off power if water is near outlets", "A sump pump and backflow valve help"]),
        "flood_dam" => diagram_explainer("Dam and Levee Failure", "flash_flood", &["Failures send a wall of water downstream", "Warnings may come with little notice", "Know your evacuation route"]),
        "flood_evac" => diagram_explainer("Flood Evacuation", "urban_flood", &["Leave early on higher roads", "Avoid underpasses and low spots", "Never drive around barricades"]),
        "flood_compare" => diagram_explainer("Flash Flood or River Flood?", "flash_flood", &["Flash floods: minutes to hours", "River floods: days, with forecasts", "Both are deadly in moving water"]),
        "winter_streets" => diagram_explainer("Snow on the Streets", "snow_city", &["Snow sticks to grass and cars first", "Roads cover as it gets heavier", "Plows can't keep up with heavy rates"]),
        "winter_vehicles" => diagram_explainer("Snow Piling Up", "snow_city", &["Clear all snow off your car", "Watch for heavy snow on roofs", "Keep vents and exhausts clear"]),
        "winter_ice_accretion" => diagram_explainer("Freezing Rain", "ice_lines", &["Rain falls through a warm layer", "It freezes on contact", "Ice builds up on everything"]),
        "winter_ice_lines" => diagram_explainer("Ice on Power Lines", "ice_lines", &["A quarter inch weighs down lines", "Half an inch snaps limbs and lines", "Expect power outages"]),
        "winter_blizzard" => diagram_explainer("Blizzard Conditions", "blizzard", &["Strong wind blows snow sideways", "Visibility drops near zero", "Don't travel in a blizzard"]),
        "winter_lake_effect" => diagram_explainer("Lake-Effect Snow", "lake_effect", &["Cold air crosses a warm lake", "It picks up heat and moisture", "Narrow bands bring heavy snow"]),
        "winter_squall" => diagram_explainer("Snow Squalls", "snow_squall", &["Sudden, intense burst of snow", "Visibility drops in seconds", "Slow down and turn on your lights"]),
        "winter_black_ice" => diagram_explainer("Black Ice", "black_ice", &["Wet roads freeze as temperatures drop", "It looks wet but is ice", "Bridges and overpasses freeze first"]),
        "winter_drifts" => diagram_explainer("Blowing and Drifting Snow", "blizzard", &["Wind picks up loose snow", "Drifts build behind obstacles", "Open roads can close quickly"]),
        "winter_transition" => diagram_explainer("Rain to Snow", "precip_types", &["Warm layer melts snow into rain", "As it cools, sleet and ice mix in", "Cold all the way: snow"]),
        "lightning_cg_demo" => diagram_explainer("Cloud-to-Ground Lightning", "lightning_cg", &["A faint channel reaches down from the cloud", "A spark reaches up from the ground to meet it", "The bright flash races back up the channel"]),
        "lightning_ic_demo" => diagram_explainer("Lightning Inside the Cloud", "lightning_ic", &["Most lightning stays in the cloud", "It jumps between charge areas", "A jump in flashes can mean a strengthening storm"]),
        "lightning_leader" => diagram_explainer("Leader and Return Stroke", "lightning_cg", &["First: a faint, branching channel reaches down", "Then: it connects with the ground", "Last: the bright flash you see"]),
        "lightning_pos_neg" => diagram_explainer("Positive vs Negative Lightning", "lightning_positive", &["Most strikes are negative", "A few strikes come from the top of the storm", "They can hit miles from the rain"]),
        "lightning_distance" => diagram_explainer("Lightning Reach", "lightning_positive", &["Lightning can strike 10 miles from the storm", "Blue sky doesn't mean safe", "If you hear thunder, you're in range"]),
        "lightning_indoors" => diagram_explainer("Safe Indoors", "lightning_cg", &["Stay off corded phones", "Avoid plumbing during the storm", "Unplug electronics ahead of time"]),
        "lightning_outdoor" => diagram_explainer("Outdoor Events", "lightning_cg", &["Have a lightning plan for every event", "Head in at the first thunder", "Wait 30 minutes after the last"]),
        "lightning_water" => diagram_explainer("On the Water", "lightning_cg", &["Get off the water at the first thunder", "Boats without cabins are not safe", "Water carries current far"]),
        "lightning_timeline" => diagram_explainer("Lightning Risk Timeline", "thunder_distance", &["Before: risk as the storm approaches", "During: highest overhead", "After: wait 30 minutes"]),
        "lightning_thunder" => diagram_explainer("How Far Away?", "thunder_distance", &["Count seconds from flash to thunder", "Divide by 5 for miles", "Under 30 seconds: get inside"]),
        "heat_illness" => diagram_explainer("Heat Exhaustion vs Heatstroke", "heat_body", &["Exhaustion: heavy sweating, cool skin, nausea", "Heatstroke: hot skin, confusion", "Heatstroke is an emergency: call 911"]),
        "heat_index_impact" => diagram_explainer("What the Heat Index Means", "heat_body", &["90s: caution with long exposure", "103+: heat illness likely", "125+: extreme danger"]),
        "heat_wet_bulb" => diagram_explainer("Wet-Bulb Temperature", "wet_bulb", &["Sweat cools you only if it can evaporate", "Humid air slows evaporation", "When it's hot and humid, your body can't cool off"]),
        "heat_timeline" => diagram_explainer("Heat Exposure", "heat_body", &["Minutes: thirst and sweating", "An hour: exhaustion can set in", "Hours: heatstroke risk rises"]),
        "heat_car" => diagram_explainer("Hot Car Danger", "car_heat", &["Car interiors heat up fast", "Cracking a window doesn't help", "Look before you lock"]),
        "heat_pets" => diagram_explainer("Pets in the Heat", "car_heat", &["Never leave pets in a car", "Pavement can burn paws", "Plenty of water and shade"]),
        "cold_frostbite" => diagram_explainer("Frostbite", "cold_body", &["Wind chill below -18°F: frostbite in 30 minutes", "Cover all skin", "Watch for numb, pale skin"]),
        "cold_hypothermia" => diagram_explainer("Hypothermia", "cold_body", &["Dress in layers, stay dry", "Shivering and confusion are warning signs", "Warm the body core first"]),
        "cold_wind_chill" => diagram_explainer("Wind Chill", "wind_chill", &["Wind pulls heat from skin", "Same temperature, more wind: colder", "Limit time outside"]),
        "cold_co" => diagram_explainer("Carbon Monoxide", "generator_co", &["CO is invisible and odorless", "Never heat your home with an oven or grill", "Have working CO alarms"]),
        "sci_layers" => diagram_explainer("Layers of the Atmosphere", "atmosphere", &["Troposphere: our weather", "Stratosphere: the ozone layer", "Higher layers thin out into space"]),
        "sci_jet" => diagram_explainer("The Jet Stream", "jet_stream", &["Big temperature contrasts drive it", "It steers storms", "Dips bring cold, ridges bring warmth"]),
        "sci_high_low" => diagram_explainer("Highs and Lows", "high_low", &["Highs: sinking air, fair weather", "Lows: rising air, clouds and rain", "Wind flows from high to low"]),
        "sci_sea_breeze" => diagram_explainer("Sea and Land Breezes", "sea_breeze", &["Land heats faster by day", "Cool sea air flows inland", "At night it reverses"]),
        "sci_inversion" => diagram_explainer("Temperature Inversion", "inversion", &["Usually air cools with height", "An inversion is a warm layer above", "It traps smoke and fog"]),
        "sci_orographic" => diagram_explainer("Mountains Make Rain", "orographic", &["Wind runs into a mountain", "Air rises, cools and rains", "The far side stays dry"]),
        "sci_rain_shadow" => diagram_explainer("Rain Shadow", "orographic", &["Moisture rains out on the way up", "Dry air sinks and warms", "Deserts form in the shadow"]),
        "sci_instability" => diagram_explainer("Unstable Air", "instability", &["A warm bubble rises", "It stays warmer than the air around it", "It keeps climbing: storms build"]),
        "sci_clouds" => diagram_explainer("How Clouds Form", "cloud_formation", &["Air rises and expands", "It cools until it can't hold its moisture", "The moisture turns into tiny cloud droplets"]),
        "sci_coriolis" => diagram_explainer("The Coriolis Effect", "coriolis", &["Earth spins under moving air", "Paths curve to the right up north", "That is why storms spin"]),
        "local_threats" => local_threats(),
        "camera_view" => camera_view(),
        "camera_inset" => camera_inset(),
        "travel_times" => travel_times(),
        "traffic_report" => traffic_report(),
        "storm_compare" => storm_compare(),
        "explain_thunderstorm" => explain_thunderstorm(),
        "explain_cold_front" => explain_cold_front(),
        "explain_lake_effect" => explain_lake_effect(),
        "explain_fog" => explain_fog(),
        "explain_hail" => explain_hail(),
        "severe_threats" => severe_threats(1, "bar"),
        "severe_threats_blocks" => severe_threats(1, "blocks"),
        "severe_threats_tomorrow" => severe_threats(2, "bar"),
        "risk_levels" => risk_levels(),
        "risk_meaning" => risk_meaning(),
        "severe_day" => severe_day(1),
        "severe_day_tomorrow" => severe_day(2),
        "severe_risk_bug" => severe_risk_bug(),
        "wsw_criteria" => super::more::wsw_criteria(),
        "storm_timing" => super::more::storm_timing(),
        "severe_day_map" => super::more::severe_day_map(1),
        "severe_day_map_tomorrow" => super::more::severe_day_map(2),
        "alert_day" => super::more::alert_day(1),
        "alert_day_tomorrow" => super::more::alert_day(2),
        "risks_overview" => super::more::risks_overview(1),
        "risks_overview_tomorrow" => super::more::risks_overview(2),
        "risk_map" => super::more::risk_map_big(1),
        "risk_map_tomorrow" => super::more::risk_map_big(2),
        k if k.starts_with("holiday_") => super::more::holiday(&k["holiday_".len()..])?,
        _ => return None,
    };
    doc.name = info.name.to_string();
    doc.category = info.category.to_string();
    doc.template = Some(key.to_string());
    Some(doc)
}

fn chart(name: &str, rect: (f32, f32, f32, f32), series: &str, series2: &str, kind: ChartKind, unit: &str) -> Layer {
    Layer::new(
        name,
        rect.0,
        rect.1,
        rect.2,
        rect.3,
        LayerKind::Chart {
            chart: kind,
            series: series.into(),
            series2: series2.into(),
            color: "$primary".into(),
            color2: "#5EC8FF".into(),
            text_color: "$text".into(),
            show_values: true,
            min: None,
            max: None,
            unit: unit.into(),
            highlight: String::new(),
            note: String::new(),
        },
    )
}

fn row_line(doc: &mut GraphicDoc, i: usize, y: f32, h: f32, left: &str, right: &str, delay: f32) {
    doc.layers.push(panel(&format!("Row {i}"), 80.0, y, 1760.0, h - 12.0).with_anim(AnimKind::SlideRight, delay, 0.45));
    // A sentence on the right (not a short value) gets most of the row and wraps, so it
    // stays TV-size (~30 px and up) instead of shrinking to one tiny line (CWXB-73).
    if right.chars().count() > 26 && !right.contains("{{") && left.chars().count() <= 22 {
        let size = (h * 0.42).min(52.0);
        doc.layers.push(
            text(&format!("Row {i} left"), (110.0, y + 6.0, 460.0, h - 24.0), left, DISPLAY, size, Weight::Bold, "$text", Align::Left)
                .with_anim(AnimKind::SlideRight, delay, 0.45),
        );
        let mut r = text(&format!("Row {i} right"), (590.0, y + 6.0, 1220.0, h - 24.0), right, DISPLAY, (size * 0.85).max(30.0), Weight::Bold, "$primary", Align::Left);
        // Wrap, but stay vertically centred beside the left label.
        if let LayerKind::Text { wrap, .. } = &mut r.kind {
            *wrap = true;
        }
        doc.layers.push(r.with_anim(AnimKind::SlideRight, delay + 0.05, 0.45));
        return;
    }
    doc.layers.push(
        text(&format!("Row {i} left"), (110.0, y + 6.0, 1100.0, h - 24.0), left, DISPLAY, (h * 0.42).min(52.0), Weight::Bold, "$text", Align::Left)
            .with_anim(AnimKind::SlideRight, delay, 0.45),
    );
    doc.layers.push(
        text(&format!("Row {i} right"), (1220.0, y + 6.0, 590.0, h - 24.0), right, DISPLAY, (h * 0.42).min(52.0), Weight::Bold, "$primary", Align::Right)
            .with_anim(AnimKind::SlideRight, delay + 0.05, 0.45),
    );
}

fn big_stat(doc: &mut GraphicDoc, name: &str, x: f32, y: f32, w: f32, label: &str, value: &str, delay: f32) {
    doc.layers.push(panel(&format!("{name} panel"), x, y, w, 300.0).with_anim(AnimKind::SlideUp, delay, 0.5));
    doc.layers.push(
        caps(text(&format!("{name} label"), (x + 20.0, y + 24.0, w - 40.0, 60.0), label, BODY, 36.0, Weight::SemiBold, "$muted", Align::Center), 2.0)
            .with_anim(AnimKind::SlideUp, delay, 0.5),
    );
    doc.layers.push(
        shadowed(text(&format!("{name} value"), (x + 20.0, y + 100.0, w - 40.0, 170.0), value, DISPLAY, 110.0, Weight::Black, "$text", Align::Center))
            .with_anim(AnimKind::Pop, delay + 0.1, 0.5),
    );
}

fn chart_doc(title: &str, series: &str, series2: &str, kind: ChartKind, unit: &str, note: &str) -> GraphicDoc {
    let mut doc = GraphicDoc::new(title, "Forecast", bg_gradient());
    header(&mut doc, title, "{{location}}");
    doc.layers.push(panel("Chart panel", 80.0, 190.0, 1760.0, 760.0).with_anim(AnimKind::Fade, 0.2, 0.5));
    doc.layers.push(chart("Chart", (120.0, 220.0, 1680.0, 700.0), series, series2, kind, unit).with_anim(AnimKind::Wipe, 0.4, 1.0));
    footer(&mut doc, "{{station}}", note);
    doc
}

fn backup() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Please Stay With Us", "Show", Some(Fill::linear("#0A0F1A", "#1D2638", 100.0)));
    doc.layers.push(
        caps(shadowed(text("Station", (100.0, 300.0, 1720.0, 180.0), "{{station_short}}", DISPLAY, 170.0, Weight::Black, "$text", Align::Center)), 6.0)
            .with_anim(AnimKind::Fade, 0.0, 0.4),
    );
    doc.layers.push(Layer::rect("Rule", 760.0, 500.0, 400.0, 8.0, Fill::solid("$primary"), 0.0).with_anim(AnimKind::Wipe, 0.2, 0.4));
    doc.layers.push(
        text("Message", (100.0, 540.0, 1720.0, 110.0), "Please stay with us. We'll be right back.", DISPLAY, 70.0, Weight::Bold, "$text", Align::Center)
            .with_anim(AnimKind::Fade, 0.3, 0.4),
    );
    doc.layers.push(
        wrapped(text("Alerts", (200.0, 700.0, 1520.0, 160.0), "{{alerts.ticker}}", BODY, 40.0, Weight::SemiBold, "$primary", Align::Center))
            .with_anim(AnimKind::Fade, 0.5, 0.4),
    );
    doc.duration = 0.0;
    doc
}

fn storm_reports() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Storm Reports", "Severe", bg_gradient());
    header(&mut doc, "Storm Reports", "Last 24 hours");
    doc.layers.push(
        text("Summary", (80.0, 170.0, 1760.0, 70.0), "{{reports.summary}}", DISPLAY, 44.0, Weight::Bold, "$primary", Align::Left)
            .with_anim(AnimKind::Fade, 0.2, 0.4),
    );
    for i in 0..7 {
        let y = 250.0 + i as f32 * 92.0;
        doc.layers.push(panel(&format!("Report {i}"), 80.0, y, 1760.0, 80.0).with_anim(AnimKind::SlideRight, 0.3 + i as f32 * 0.06, 0.4));
        doc.layers.push(
            text(&format!("Report {i} line"), (110.0, y + 6.0, 1700.0, 68.0), &format!("{{{{report{i}.line}}}}"), BODY, 38.0, Weight::SemiBold, "$text", Align::Left)
                .with_anim(AnimKind::SlideRight, 0.3 + i as f32 * 0.06, 0.4),
        );
    }
    footer(&mut doc, "Most reports: {{reports.counties}}", "Source: NWS Local Storm Reports");
    doc
}

fn spc_probs() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Tornado / Hail / Wind Chances", "Severe", bg_gradient());
    header(&mut doc, "Today's Severe Threats", "Source: NWS Storm Prediction Center");
    big_stat(&mut doc, "Risk", 80.0, 220.0, 1760.0, "Overall risk", "{{spc.home}}", 0.2);
    big_stat(&mut doc, "Tornado", 80.0, 560.0, 560.0, "Tornado", "{{spc.home.tornado}}", 0.4);
    big_stat(&mut doc, "Hail", 680.0, 560.0, 560.0, "Large hail", "{{spc.home.hail}}", 0.5);
    big_stat(&mut doc, "Wind", 1280.0, 560.0, 560.0, "Damaging wind", "{{spc.home.wind}}", 0.6);
    // Each chance on the 1-5 scale under its number.
    for (i, key) in ["tornado", "hail", "wind"].iter().enumerate() {
        let x = 120.0 + i as f32 * 600.0;
        doc.layers.push(Layer::meter(&format!("{key} meter"), x, 880.0, 480.0, 36.0, &format!("{{{{spc.home.{key}.level}}}}"), "blocks").with_anim(AnimKind::Fade, 0.7 + i as f32 * 0.1, 0.3));
    }
    footer(&mut doc, "{{station}}", "Source: NWS Storm Prediction Center • chance within 25 miles of a point");
    doc
}

fn severe_timing() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Severe Timing", "Severe", bg_gradient());
    header(&mut doc, "Storm Timing", "{{location}}");
    doc.layers.push(
        wrapped(shadowed(text("Window", (80.0, 180.0, 1760.0, 170.0), "{{severe.window}}", DISPLAY, 76.0, Weight::Black, "$text", Align::Left)))
            .with_anim(AnimKind::SlideLeft, 0.2, 0.5),
    );
    doc.layers.push(
        text("Risk", (80.0, 350.0, 1760.0, 70.0), "Severe risk today: {{spc.home}}", BODY, 44.0, Weight::SemiBold, "$primary", Align::Left)
            .with_anim(AnimKind::Fade, 0.4, 0.4),
    );
    doc.layers.push(panel("Chart panel", 80.0, 440.0, 1760.0, 510.0).with_anim(AnimKind::Fade, 0.4, 0.5));
    doc.layers.push(chart("Rain per hour", (120.0, 460.0, 1680.0, 470.0), "hourly.rain", "", ChartKind::Bar, "\"").with_anim(AnimKind::Wipe, 0.6, 1.0));
    footer(&mut doc, "{{station}}", "Model guidance, next 24 hours");
    doc
}

fn lightning_safety() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Lightning Safety", "Severe", Some(Fill::linear("#120F24", "#2A2050", 115.0)));
    header(&mut doc, "Lightning", "{{location}}");
    doc.layers.push(
        wrapped(shadowed(text("Near", (80.0, 190.0, 1760.0, 150.0), "{{lightning.near}}", DISPLAY, 70.0, Weight::Black, "$text", Align::Left)))
            .with_anim(AnimKind::SlideLeft, 0.2, 0.5),
    );
    doc.layers.push(
        shadowed(text("All clear", (80.0, 350.0, 1760.0, 120.0), "{{lightning.all_clear}}", DISPLAY, 96.0, Weight::Black, "#FFD60A", Align::Left))
            .with_anim(AnimKind::Pop, 0.4, 0.5),
    );
    doc.layers.push(
        text("Rule", (80.0, 470.0, 1760.0, 60.0), "When thunder roars, go indoors. Wait 30 minutes after the last strike.", BODY, 38.0, Weight::SemiBold, "$muted", Align::Left)
            .with_anim(AnimKind::Fade, 0.5, 0.4),
    );
    doc.layers.push(panel("Trend panel", 80.0, 550.0, 1760.0, 400.0).with_anim(AnimKind::Fade, 0.5, 0.5));
    doc.layers.push(chart("Strike trend", (120.0, 570.0, 1680.0, 360.0), "lightning.rate", "", ChartKind::Bar, "").with_anim(AnimKind::Wipe, 0.7, 1.0));
    footer(&mut doc, "{{lightning.rate}}", "Strikes within 25 miles, per 5 minutes");
    doc
}

fn cell_briefing() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Storm Cell Briefing", "Severe", bg_gradient());
    header(&mut doc, "Strongest Storms", "{{radar.time}}");
    for i in 0..4 {
        let y = 180.0 + i as f32 * 110.0;
        row_line(&mut doc, i, y, 110.0, &format!("{{{{cell{i}.summary}}}}"), "", 0.2 + i as f32 * 0.08);
    }
    doc.layers.push(panel("History panel", 80.0, 640.0, 1760.0, 320.0).with_anim(AnimKind::Fade, 0.5, 0.5));
    doc.layers.push(chart("dBZ history", (120.0, 655.0, 1680.0, 290.0), "cell.dbz", "", ChartKind::Area, " dBZ").with_anim(AnimKind::Wipe, 0.7, 1.0));
    footer(&mut doc, "{{station}}", "Strongest storm's peak reflectivity, last 2 hours");
    doc
}

fn ef_scale() -> GraphicDoc {
    let mut doc = GraphicDoc::new("EF Scale", "Severe", bg_gradient());
    header(&mut doc, "Enhanced Fujita Scale", "Tornado ratings");
    let rows = [
        ("EF0", "65-85 mph", "Shingles off, branches broken", "#6FC2FF"),
        ("EF1", "86-110 mph", "Roofs peeled, mobile homes overturned", "#5FD38D"),
        ("EF2", "111-135 mph", "Roofs torn off, large trees snapped", "#FFE066"),
        ("EF3", "136-165 mph", "Stories of homes destroyed", "#FFA366"),
        ("EF4", "166-200 mph", "Well-built homes leveled", "#E06666"),
        ("EF5", "Over 200 mph", "Strong frame homes swept away", "#EE99EE"),
    ];
    for (i, (r, wind, dmg, color)) in rows.iter().enumerate() {
        let y = 180.0 + i as f32 * 128.0;
        let d = 0.2 + i as f32 * 0.08;
        doc.layers.push(Layer::rect(&format!("{r} tag"), 80.0, y, 220.0, 112.0, Fill::solid(color), 12.0).with_anim(AnimKind::SlideRight, d, 0.4));
        doc.layers.push(text(&format!("{r} name"), (80.0, y, 220.0, 112.0), r, DISPLAY, 64.0, Weight::Black, "#10131A", Align::Center).with_anim(AnimKind::SlideRight, d, 0.4));
        doc.layers.push(panel(&format!("{r} row"), 320.0, y, 1520.0, 112.0).with_anim(AnimKind::SlideRight, d, 0.4));
        doc.layers.push(text(&format!("{r} wind"), (350.0, y + 10.0, 420.0, 92.0), wind, DISPLAY, 50.0, Weight::Bold, "$text", Align::Left).with_anim(AnimKind::Fade, d + 0.1, 0.4));
        doc.layers.push(text(&format!("{r} damage"), (780.0, y + 10.0, 1040.0, 92.0), dmg, BODY, 40.0, Weight::SemiBold, "$muted", Align::Left).with_anim(AnimKind::Fade, d + 0.15, 0.4));
    }
    footer(&mut doc, "{{station}}", "Ratings come from NWS damage surveys");
    doc
}

fn watch_warning() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Watch vs Warning", "Severe", bg_gradient());
    header(&mut doc, "Watch or Warning?", "Know the difference");
    let cols = [
        ("WATCH", "#FFD60A", "Conditions are right for severe weather to develop.", "Be prepared. Know where you'll go. Keep your phone charged."),
        ("WARNING", "#FF3B3B", "Severe weather is happening or about to happen.", "Take action now. Get to your safe place."),
    ];
    for (i, (name, color, means, todo)) in cols.iter().enumerate() {
        let x = 80.0 + i as f32 * 900.0;
        let d = 0.2 + i as f32 * 0.2;
        doc.layers.push(Layer::rect(&format!("{name} bar"), x, 190.0, 860.0, 140.0, Fill::solid(color), 14.0).with_anim(AnimKind::SlideDown, d, 0.45));
        doc.layers.push(caps(text(&format!("{name} title"), (x, 190.0, 860.0, 140.0), name, DISPLAY, 96.0, Weight::Black, "#10131A", Align::Center), 6.0).with_anim(AnimKind::Pop, d + 0.1, 0.5));
        doc.layers.push(panel(&format!("{name} body"), x, 350.0, 860.0, 600.0).with_anim(AnimKind::SlideUp, d, 0.5));
        doc.layers.push(wrapped(text(&format!("{name} means"), (x + 40.0, 380.0, 780.0, 240.0), means, DISPLAY, 54.0, Weight::Bold, "$text", Align::Left)).with_anim(AnimKind::Fade, d + 0.2, 0.4));
        doc.layers.push(wrapped(text(&format!("{name} todo"), (x + 40.0, 640.0, 780.0, 280.0), todo, BODY, 44.0, Weight::SemiBold, color, Align::Left)).with_anim(AnimKind::Fade, d + 0.3, 0.4));
    }
    footer(&mut doc, "{{station}}", "{{tagline}}");
    doc
}

fn damage_survey() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Tornado Damage Survey", "Severe", bg_gradient());
    header(&mut doc, "Tornado Damage Survey", "Source: NWS damage survey");
    big_stat(&mut doc, "Rating", 80.0, 200.0, 560.0, "Rating", "EF1", 0.2);
    big_stat(&mut doc, "Wind", 680.0, 200.0, 560.0, "Peak wind", "105 mph", 0.3);
    big_stat(&mut doc, "Length", 1280.0, 200.0, 560.0, "Path length", "4.2 mi", 0.4);
    big_stat(&mut doc, "Width", 80.0, 540.0, 560.0, "Path width", "150 yd", 0.5);
    doc.layers.push(panel("Notes panel", 680.0, 540.0, 1160.0, 300.0).with_anim(AnimKind::SlideUp, 0.6, 0.5));
    doc.layers.push(
        wrapped(text("Notes", (720.0, 570.0, 1080.0, 250.0), "Tornado touched down near … and lifted near …. Edit this text with the survey details.", BODY, 40.0, Weight::SemiBold, "$text", Align::Left))
            .with_anim(AnimKind::Fade, 0.7, 0.4),
    );
    footer(&mut doc, "{{station}}", "Preliminary; final data from NWS");
    doc
}

fn hourly_strip() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Hour by Hour", "Forecast", bg_gradient());
    header(&mut doc, "Hour by Hour", "{{location}}");
    let n = 12;
    let card_w = 140.0;
    let gap = 6.0;
    let start = (1920.0 - (card_w * n as f32 + gap * (n - 1) as f32)) / 2.0;
    for i in 0..n {
        let x = start + i as f32 * (card_w + gap);
        let d = 0.2 + i as f32 * 0.05;
        doc.layers.push(panel(&format!("Hour {i} card"), x, 220.0, card_w, 700.0).with_anim(AnimKind::SlideUp, d, 0.45));
        doc.layers.push(caps(text(&format!("Hour {i} time"), (x, 240.0, card_w, 60.0), &format!("{{{{hour{i}.time}}}}"), DISPLAY, 34.0, Weight::Bold, "$primary", Align::Center), 1.0).with_anim(AnimKind::SlideUp, d, 0.45));
        doc.layers.push(Layer::icon(&format!("Hour {i} icon"), x + 15.0, 330.0, 110.0, &format!("{{{{hour{i}.cond}}}}"), &format!("{{{{hour{i}.daypart}}}}")).with_anim(AnimKind::Pop, d + 0.1, 0.45));
        doc.layers.push(shadowed(text(&format!("Hour {i} temp"), (x, 470.0, card_w, 100.0), &format!("{{{{hour{i}.temp}}}}"), DISPLAY, 64.0, Weight::Black, "$text", Align::Center)).with_anim(AnimKind::SlideUp, d, 0.45));
        doc.layers.push(wrapped(text(&format!("Hour {i} cond"), (x + 6.0, 590.0, card_w - 12.0, 130.0), &format!("{{{{hour{i}.cond}}}}"), BODY, 26.0, Weight::SemiBold, "$text", Align::Center)).with_anim(AnimKind::Fade, d + 0.1, 0.4));
        doc.layers.push(text(&format!("Hour {i} wind"), (x, 760.0, card_w, 50.0), &format!("{{{{hour{i}.wind}}}}"), BODY, 24.0, Weight::Regular, "$muted", Align::Center).with_anim(AnimKind::Fade, d + 0.15, 0.4));
    }
    footer(&mut doc, "{{station}}", "Model guidance");
    doc
}

fn day_parts() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Morning / Afternoon / Evening", "Forecast", bg_gradient());
    header(&mut doc, "Your Day", "{{location}}");
    for (i, (part, label, daypart)) in [("morning", "Morning", "day"), ("afternoon", "Afternoon", "day"), ("evening", "Evening", "night")].iter().enumerate() {
        let x = 80.0 + i as f32 * 600.0;
        let d = 0.2 + i as f32 * 0.12;
        doc.layers.push(panel(&format!("{label} card"), x, 200.0, 560.0, 740.0).with_anim(AnimKind::SlideUp, d, 0.5));
        doc.layers.push(caps(text(&format!("{label} name"), (x, 230.0, 560.0, 80.0), label, DISPLAY, 56.0, Weight::Bold, "$primary", Align::Center), 2.0).with_anim(AnimKind::SlideUp, d, 0.5));
        doc.layers.push(Layer::icon(&format!("{label} icon"), x + 150.0, 330.0, 260.0, &format!("{{{{part.{part}.cond}}}}"), daypart).with_anim(AnimKind::Pop, d + 0.15, 0.5));
        doc.layers.push(shadowed(text(&format!("{label} temp"), (x, 600.0, 560.0, 160.0), &format!("{{{{part.{part}.temp}}}}"), DISPLAY, 140.0, Weight::Black, "$text", Align::Center)).with_anim(AnimKind::SlideUp, d + 0.05, 0.5));
        doc.layers.push(wrapped(text(&format!("{label} cond"), (x + 30.0, 790.0, 500.0, 120.0), &format!("{{{{part.{part}.cond}}}}"), BODY, 40.0, Weight::SemiBold, "$text", Align::Center)).with_anim(AnimKind::Fade, d + 0.2, 0.4));
    }
    footer(&mut doc, "{{station}}", "{{weekday}}, {{date}}");
    doc
}

fn weekend() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Weekend Planner", "Forecast", bg_gradient());
    header(&mut doc, "Weekend Planner", "{{location}}");
    for (i, key) in ["weekend.sat", "weekend.sun"].iter().enumerate() {
        let y = 220.0 + i as f32 * 360.0;
        let d = 0.2 + i as f32 * 0.15;
        doc.layers.push(panel(&format!("Day {i} panel"), 80.0, y, 1760.0, 320.0).with_anim(AnimKind::SlideRight, d, 0.5));
        doc.layers.push(wrapped(text(&format!("Day {i} line"), (130.0, y + 40.0, 1660.0, 240.0), &format!("{{{{{key}}}}}"), DISPLAY, 64.0, Weight::Bold, "$text", Align::Left)).with_anim(AnimKind::SlideRight, d, 0.5));
    }
    footer(&mut doc, "{{station}}", "Sunrise {{sun.rise}} • Sunset {{sun.set}}");
    doc
}

fn wind_timeline() -> GraphicDoc {
    let mut doc = chart_doc("Wind Timeline", "hourly.wind", "hourly.gust", ChartKind::Area, "", "Wind (solid) and gusts (line), mph");
    doc.layers.push(
        text("Max gust", (80.0, 160.0, 1760.0, 40.0), "Strongest gust: {{gust.max24}}", BODY, 32.0, Weight::Bold, "$primary", Align::Right)
            .with_anim(AnimKind::Fade, 0.5, 0.4),
    );
    doc
}

fn rain_totals() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Rain Totals Forecast", "Forecast", bg_gradient());
    header(&mut doc, "How Much Rain?", "{{location}}");
    big_stat(&mut doc, "Day1", 80.0, 200.0, 560.0, "Next 24 hours", "{{qpf.day1}}", 0.2);
    big_stat(&mut doc, "Day3", 680.0, 200.0, 560.0, "Next 3 days", "{{qpf.day3}}", 0.3);
    big_stat(&mut doc, "Day7", 1280.0, 200.0, 560.0, "Next 7 days", "{{qpf.day7}}", 0.4);
    for i in 0..4 {
        let y = 540.0 + i as f32 * 100.0;
        row_line(&mut doc, i, y, 100.0, &format!("{{{{city{i}.name}}}}"), &format!("{{{{city{i}.rain48}}}}"), 0.5 + i as f32 * 0.06);
    }
    footer(&mut doc, "{{station}}", "WPC rainfall forecast • town totals: model, 48 h");
    doc
}

fn winter_timeline() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Winter Storm Timeline", "Forecast", Some(Fill::linear("#0B1830", "#24486E", 115.0)));
    header(&mut doc, "Winter Storm Timeline", "{{location}}");
    doc.layers.push(
        wrapped(shadowed(text("Timeline", (80.0, 180.0, 1760.0, 150.0), "{{winter.timeline}}", DISPLAY, 70.0, Weight::Black, "$text", Align::Left)))
            .with_anim(AnimKind::SlideLeft, 0.2, 0.5),
    );
    big_stat(&mut doc, "Snow", 80.0, 360.0, 860.0, "Snow, next 48 h", "{{snow.total48}}", 0.3);
    big_stat(&mut doc, "Ice", 980.0, 360.0, 860.0, "Ice / mix, next 48 h", "{{ice.total48}}", 0.4);
    let n = 6;
    for i in 0..n {
        let x = 80.0 + i as f32 * 296.0;
        let h = i * 2;
        doc.layers.push(panel(&format!("Slot {i}"), x, 700.0, 280.0, 250.0).with_anim(AnimKind::SlideUp, 0.5 + i as f32 * 0.05, 0.4));
        doc.layers.push(text(&format!("Slot {i} time"), (x, 715.0, 280.0, 50.0), &format!("{{{{hour{h}.time}}}}"), DISPLAY, 34.0, Weight::Bold, "$primary", Align::Center).with_anim(AnimKind::Fade, 0.55 + i as f32 * 0.05, 0.4));
        doc.layers.push(wrapped(text(&format!("Slot {i} cond"), (x + 10.0, 770.0, 260.0, 110.0), &format!("{{{{hour{h}.cond}}}}"), BODY, 32.0, Weight::SemiBold, "$text", Align::Center)).with_anim(AnimKind::Fade, 0.6 + i as f32 * 0.05, 0.4));
        doc.layers.push(text(&format!("Slot {i} temp"), (x, 880.0, 280.0, 60.0), &format!("{{{{hour{h}.temp}}}}"), DISPLAY, 44.0, Weight::Black, "$text", Align::Center).with_anim(AnimKind::Fade, 0.6 + i as f32 * 0.05, 0.4));
    }
    footer(&mut doc, "{{station}}", "Model guidance");
    doc
}

fn pressure_trend() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Pressure Trend", "Forecast", bg_gradient());
    header(&mut doc, "Barometric Pressure", "{{location}}");
    big_stat(&mut doc, "Now", 80.0, 200.0, 860.0, "Now", "{{pressure}}", 0.2);
    big_stat(&mut doc, "Trend", 980.0, 200.0, 860.0, "Trend", "{{pressure.trend}}", 0.3);
    doc.layers.push(panel("Chart panel", 80.0, 540.0, 1760.0, 410.0).with_anim(AnimKind::Fade, 0.4, 0.5));
    doc.layers.push(chart("Pressure", (120.0, 555.0, 1680.0, 380.0), "hourly.pressure", "", ChartKind::Line, "").with_anim(AnimKind::Wipe, 0.6, 1.0));
    footer(&mut doc, "{{station}}", "Next 24 hours, mb");
    doc
}

fn cities_now() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Around the Area", "Forecast", bg_gradient());
    header(&mut doc, "Around the Area", "{{time}}");
    for i in 0..6 {
        let y = 180.0 + i as f32 * 128.0;
        let d = 0.2 + i as f32 * 0.06;
        doc.layers.push(panel(&format!("City {i}"), 80.0, y, 1760.0, 116.0).with_anim(AnimKind::SlideRight, d, 0.45));
        doc.layers.push(text(&format!("City {i} name"), (120.0, y + 8.0, 760.0, 100.0), &format!("{{{{city{i}.name}}}}"), DISPLAY, 56.0, Weight::Bold, "$text", Align::Left).with_anim(AnimKind::SlideRight, d, 0.45));
        doc.layers.push(Layer::icon(&format!("City {i} icon"), 900.0, y + 8.0, 100.0, &format!("{{{{city{i}.cond}}}}"), "{{daypart}}").with_anim(AnimKind::Pop, d + 0.1, 0.4));
        doc.layers.push(text(&format!("City {i} cond"), (1020.0, y + 8.0, 520.0, 100.0), &format!("{{{{city{i}.cond}}}}"), BODY, 40.0, Weight::SemiBold, "$muted", Align::Left).with_anim(AnimKind::Fade, d + 0.1, 0.4));
        doc.layers.push(shadowed(text(&format!("City {i} temp"), (1540.0, y + 4.0, 280.0, 108.0), &format!("{{{{city{i}.temp}}}}"), DISPLAY, 84.0, Weight::Black, "$text", Align::Right)).with_anim(AnimKind::SlideRight, d + 0.05, 0.45));
    }
    footer(&mut doc, "{{station}}", "Model analysis");
    doc
}

fn obs_dashboard() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Observation Dashboard", "Forecast", bg_gradient());
    header(&mut doc, "Right Now", "{{obs_station}} • {{obs_time}}");
    let gauges = [("Humidity", "{{humidity}}", 0.0, 100.0, "%"), ("Wind", "{{wind_speed}}", 0.0, 60.0, ""), ("Temperature", "{{temp}}", -10.0, 110.0, "°")];
    for (i, (label, binding, min, max, unit)) in gauges.iter().enumerate() {
        let x = 80.0 + i as f32 * 600.0;
        let d = 0.2 + i as f32 * 0.12;
        doc.layers.push(panel(&format!("{label} panel"), x, 200.0, 560.0, 520.0).with_anim(AnimKind::SlideUp, d, 0.5));
        doc.layers.push(caps(text(&format!("{label} label"), (x, 220.0, 560.0, 60.0), label, BODY, 36.0, Weight::SemiBold, "$muted", Align::Center), 2.0).with_anim(AnimKind::Fade, d, 0.4));
        let mut g = chart(&format!("{label} gauge"), (x + 30.0, 290.0, 500.0, 400.0), binding, "", ChartKind::Gauge, unit);
        if let LayerKind::Chart { min: lo, max: hi, .. } = &mut g.kind {
            *lo = Some(*min);
            *hi = Some(*max);
        }
        doc.layers.push(g.with_anim(AnimKind::Pop, d + 0.15, 0.6));
    }
    let rows = [("Dew point", "{{dewpoint}}"), ("Pressure", "{{pressure}} ({{pressure.trend}})"), ("Gusts", "{{gust}}"), ("Visibility", "{{visibility}}")];
    for (i, (l, v)) in rows.iter().enumerate() {
        let x = 80.0 + i as f32 * 445.0;
        doc.layers.push(panel(&format!("{l} box"), x, 750.0, 425.0, 200.0).with_anim(AnimKind::SlideUp, 0.6 + i as f32 * 0.05, 0.4));
        doc.layers.push(caps(text(&format!("{l} label"), (x, 765.0, 425.0, 50.0), l, BODY, 30.0, Weight::SemiBold, "$muted", Align::Center), 1.0).with_anim(AnimKind::Fade, 0.65, 0.4));
        doc.layers.push(text(&format!("{l} value"), (x + 10.0, 820.0, 405.0, 110.0), v, DISPLAY, 52.0, Weight::Bold, "$text", Align::Center).with_anim(AnimKind::Fade, 0.7, 0.4));
    }
    doc
}

fn sun_moon() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Sun and Moon", "Forecast", Some(Fill::linear("#0B1022", "#2B2A55", 120.0)));
    header(&mut doc, "Sun and Moon", "{{location}}");
    big_stat(&mut doc, "Sunrise", 80.0, 200.0, 560.0, "Sunrise", "{{sun.rise}}", 0.2);
    big_stat(&mut doc, "Sunset", 680.0, 200.0, 560.0, "Sunset", "{{sun.set}}", 0.3);
    big_stat(&mut doc, "Daylight", 1280.0, 200.0, 560.0, "Daylight", "{{sun.daylight}}", 0.4);
    doc.layers.push(text("Change", (80.0, 520.0, 1760.0, 60.0), "{{sun.change}}", BODY, 40.0, Weight::SemiBold, "$primary", Align::Center).with_anim(AnimKind::Fade, 0.5, 0.4));
    big_stat(&mut doc, "Moon", 80.0, 610.0, 1160.0, "Moon", "{{moon.phase}}", 0.6);
    big_stat(&mut doc, "Illum", 1280.0, 610.0, 560.0, "Lit", "{{moon.illum}}", 0.7);
    footer(&mut doc, "Next full moon {{moon.next_full}}", "{{station}}");
    doc
}

fn season_countdown() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Season Countdown", "Forecast", bg_gradient());
    header(&mut doc, "Season Countdown", "{{location}}");
    doc.layers.push(shadowed(text("Days", (80.0, 180.0, 1760.0, 200.0), "{{season.days}}", DISPLAY, 180.0, Weight::Black, "$text", Align::Center)).with_anim(AnimKind::Pop, 0.2, 0.6));
    doc.layers.push(text("Until", (80.0, 390.0, 1760.0, 80.0), "until {{season.next}}", DISPLAY, 60.0, Weight::Bold, "$primary", Align::Center).with_anim(AnimKind::Fade, 0.4, 0.4));
    doc.layers.push(panel("Chart panel", 80.0, 500.0, 1760.0, 450.0).with_anim(AnimKind::Fade, 0.5, 0.5));
    doc.layers.push(chart("Daylight", (120.0, 515.0, 1680.0, 420.0), "daylight.year", "", ChartKind::Area, " h").with_anim(AnimKind::Wipe, 0.7, 1.0));
    footer(&mut doc, "Daylight today {{sun.daylight}}", "Hours of daylight through the year");
    doc
}

fn river_stages() -> GraphicDoc {
    let mut doc = GraphicDoc::new("River Levels", "Flooding", Some(Fill::linear("#071A24", "#0F3A4F", 115.0)));
    header(&mut doc, "River Levels", "{{location}}");
    let cols = [("River and place", 110.0, 760.0, Align::Left), ("Now", 880.0, 220.0, Align::Right), ("Flood", 1110.0, 220.0, Align::Right), ("Forecast", 1340.0, 220.0, Align::Right)];
    for (name, x, w, a) in cols {
        doc.layers.push(caps(text(&format!("{name} head"), (x, 170.0, w, 50.0), name, BODY, 28.0, Weight::Bold, "$muted", a), 1.0).with_anim(AnimKind::Fade, 0.2, 0.4));
    }
    for i in 0..6 {
        let y = 230.0 + i as f32 * 120.0;
        let d = 0.25 + i as f32 * 0.06;
        doc.layers.push(panel(&format!("Gauge {i}"), 80.0, y, 1760.0, 108.0).with_anim(AnimKind::SlideRight, d, 0.45));
        doc.layers.push(text(&format!("Gauge {i} name"), (110.0, y + 6.0, 760.0, 96.0), &format!("{{{{river{i}.name}}}}"), BODY, 34.0, Weight::Bold, "$text", Align::Left).with_anim(AnimKind::SlideRight, d, 0.45));
        doc.layers.push(text(&format!("Gauge {i} stage"), (880.0, y + 6.0, 220.0, 96.0), &format!("{{{{river{i}.stage}}}}"), DISPLAY, 44.0, Weight::Bold, "$text", Align::Right).with_anim(AnimKind::Fade, d + 0.05, 0.4));
        doc.layers.push(text(&format!("Gauge {i} flood"), (1110.0, y + 6.0, 220.0, 96.0), &format!("{{{{river{i}.flood}}}}"), DISPLAY, 44.0, Weight::Bold, "$muted", Align::Right).with_anim(AnimKind::Fade, d + 0.05, 0.4));
        doc.layers.push(text(&format!("Gauge {i} forecast"), (1340.0, y + 6.0, 220.0, 96.0), &format!("{{{{river{i}.forecast}}}}"), DISPLAY, 44.0, Weight::Bold, "$primary", Align::Right).with_anim(AnimKind::Fade, d + 0.05, 0.4));
        doc.layers.push(text(&format!("Gauge {i} status"), (1580.0, y + 6.0, 240.0, 96.0), &format!("{{{{river{i}.status}}}}"), BODY, 26.0, Weight::SemiBold, "#FFD60A", Align::Right).with_anim(AnimKind::Fade, d + 0.1, 0.4));
    }
    footer(&mut doc, "{{river.summary}}", "Source: NOAA National Water Prediction Service");
    doc
}

fn hydrograph() -> GraphicDoc {
    let mut doc = chart_doc("River Hydrograph", "river.stage", "river.flood", ChartKind::Line, " ft", "Observed and forecast stage; flood stage line");
    doc.layers.push(
        text("Gauge", (80.0, 160.0, 1760.0, 40.0), "{{river0.name}}: now {{river0.stage}}, forecast {{river0.forecast}}, flood stage {{river0.flood}}", BODY, 30.0, Weight::Bold, "$primary", Align::Left)
            .with_anim(AnimKind::Fade, 0.4, 0.4),
    );
    doc.category = "Flooding".into();
    doc
}

fn excessive_rain() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Flooding Rain Risk", "Flooding", Some(Fill::linear("#071A24", "#0F3A4F", 115.0)));
    header(&mut doc, "Flooding Rain Risk", "Source: NWS Weather Prediction Center");
    big_stat(&mut doc, "Day1", 80.0, 220.0, 560.0, "Today", "{{ero.day1}}", 0.2);
    big_stat(&mut doc, "Day2", 680.0, 220.0, 560.0, "Tomorrow", "{{ero.day2}}", 0.3);
    big_stat(&mut doc, "Day3", 1280.0, 220.0, 560.0, "Day 3", "{{ero.day3}}", 0.4);
    let levels = [("Marginal", "#00B050"), ("Slight", "#FFFF00"), ("Moderate", "#FF0000"), ("High", "#FF00FF")];
    for (i, (l, color)) in levels.iter().enumerate() {
        let x = 80.0 + i as f32 * 445.0;
        doc.layers.push(Layer::rect(&format!("{l} box"), x, 600.0, 425.0, 120.0, Fill::solid(color), 12.0).with_anim(AnimKind::SlideUp, 0.5 + i as f32 * 0.06, 0.4));
        doc.layers.push(caps(text(&format!("{l} text"), (x, 600.0, 425.0, 120.0), l, DISPLAY, 46.0, Weight::Black, "#10131A", Align::Center), 1.0).with_anim(AnimKind::Fade, 0.55, 0.4));
    }
    doc.layers.push(
        text("Rain", (80.0, 760.0, 1760.0, 80.0), "Rain expected: {{qpf.day3}} over 3 days", DISPLAY, 50.0, Weight::Bold, "$text", Align::Center)
            .with_anim(AnimKind::Fade, 0.7, 0.4),
    );
    footer(&mut doc, "{{station}}", "Turn around, don't drown");
    doc
}

fn travel() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Travel Impacts", "Travel", bg_gradient());
    header(&mut doc, "Travel Impacts", "{{roads.summary}}");
    for i in 0..5 {
        let y = 180.0 + i as f32 * 104.0;
        row_line(&mut doc, i, y, 104.0, &format!("{{{{road{i}.line}}}}"), "", 0.2 + i as f32 * 0.06);
    }
    doc.layers.push(panel("AM", 80.0, 720.0, 860.0, 240.0).with_anim(AnimKind::SlideUp, 0.6, 0.45));
    doc.layers.push(caps(text("AM label", (110.0, 735.0, 800.0, 50.0), "Morning drive", BODY, 30.0, Weight::Bold, "$primary", Align::Left), 1.0).with_anim(AnimKind::Fade, 0.65, 0.4));
    doc.layers.push(wrapped(text("AM text", (110.0, 790.0, 800.0, 150.0), "{{commute.am}}", BODY, 36.0, Weight::SemiBold, "$text", Align::Left)).with_anim(AnimKind::Fade, 0.7, 0.4));
    doc.layers.push(panel("PM", 980.0, 720.0, 860.0, 240.0).with_anim(AnimKind::SlideUp, 0.65, 0.45));
    doc.layers.push(caps(text("PM label", (1010.0, 735.0, 800.0, 50.0), "Evening drive", BODY, 30.0, Weight::Bold, "$primary", Align::Left), 1.0).with_anim(AnimKind::Fade, 0.7, 0.4));
    doc.layers.push(wrapped(text("PM text", (1010.0, 790.0, 800.0, 150.0), "{{commute.pm}}", BODY, 36.0, Weight::SemiBold, "$text", Align::Left)).with_anim(AnimKind::Fade, 0.75, 0.4));
    footer(&mut doc, "{{station}}", "INDOT • model guidance");
    doc
}

fn commute() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Commute Forecast", "Travel", bg_gradient());
    header(&mut doc, "Your Commute", "{{location}}");
    for (i, (label, key, daypart)) in [("Morning drive", "commute.am", "day"), ("Evening drive", "commute.pm", "day")].iter().enumerate() {
        let y = 210.0 + i as f32 * 370.0;
        let d = 0.2 + i as f32 * 0.15;
        doc.layers.push(panel(&format!("{label} panel"), 80.0, y, 1760.0, 330.0).with_anim(AnimKind::SlideRight, d, 0.5));
        doc.layers.push(Layer::icon(&format!("{label} icon"), 120.0, y + 40.0, 250.0, &format!("{{{{{key}}}}}"), daypart).with_anim(AnimKind::Pop, d + 0.1, 0.5));
        doc.layers.push(caps(text(&format!("{label} label"), (420.0, y + 30.0, 1380.0, 70.0), label, DISPLAY, 50.0, Weight::Bold, "$primary", Align::Left), 2.0).with_anim(AnimKind::SlideRight, d, 0.5));
        doc.layers.push(wrapped(text(&format!("{label} text"), (420.0, y + 110.0, 1380.0, 190.0), &format!("{{{{{key}}}}}"), DISPLAY, 54.0, Weight::Bold, "$text", Align::Left)).with_anim(AnimKind::Fade, d + 0.1, 0.4));
    }
    footer(&mut doc, "{{station}}", "Leave a little extra time");
    doc
}

fn closings() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Closings and Delays", "Community", bg_gradient());
    header(&mut doc, "Closings and Delays", "{{closings.count}} reported");
    for i in 0..16 {
        let (col, r) = (i / 8, i % 8);
        let x = 80.0 + col as f32 * 900.0;
        let y = 180.0 + r as f32 * 98.0;
        let d = 0.2 + i as f32 * 0.03;
        doc.layers.push(panel(&format!("Closing {i}"), x, y, 860.0, 88.0).with_anim(AnimKind::SlideUp, d, 0.4));
        doc.layers.push(text(&format!("Closing {i} name"), (x + 24.0, y + 4.0, 500.0, 80.0), &format!("{{{{closing{i}.name}}}}"), BODY, 34.0, Weight::Bold, "$text", Align::Left).with_anim(AnimKind::Fade, d, 0.4));
        doc.layers.push(text(&format!("Closing {i} status"), (x + 530.0, y + 4.0, 310.0, 80.0), &format!("{{{{closing{i}.status}}}}"), BODY, 30.0, Weight::SemiBold, "$primary", Align::Right).with_anim(AnimKind::Fade, d + 0.05, 0.4));
    }
    footer(&mut doc, "{{station}}", "Report a closing to the station");
    doc
}

fn closings_crawl() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Closings Crawl", "Community", None);
    doc.layers.push(Layer::rect("Bar", 0.0, 985.0, 1920.0, 95.0, Fill::solid("rgba(10,14,22,0.92)"), 0.0).with_anim(AnimKind::SlideUp, 0.0, 0.4));
    doc.layers.push(Layer::rect("Tag", 0.0, 985.0, 300.0, 95.0, Fill::solid("$primary"), 0.0).with_anim(AnimKind::SlideRight, 0.1, 0.4));
    doc.layers.push(caps(text("Tag text", (0.0, 985.0, 300.0, 95.0), "Closings", DISPLAY, 40.0, Weight::Black, "#0B0F18", Align::Center), 2.0).with_anim(AnimKind::Fade, 0.2, 0.3));
    doc.layers.push(
        Layer::new("Crawl", 310.0, 985.0, 1610.0, 95.0, LayerKind::Ticker { text: "{{closings.ticker}}".into(), font: "$body".into(), size: 44.0, color: "#FFFFFF".into(), speed: 150.0 })
            .with_anim(AnimKind::Fade, 0.3, 0.3),
    );
    doc.duration = 0.0;
    doc
}

fn centers() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Warming / Cooling Centers", "Community", bg_gradient());
    header(&mut doc, "Warming and Cooling Centers", "{{location}}");
    for i in 0..7 {
        let y = 180.0 + i as f32 * 112.0;
        let d = 0.2 + i as f32 * 0.06;
        doc.layers.push(panel(&format!("Center {i}"), 80.0, y, 1760.0, 100.0).with_anim(AnimKind::SlideRight, d, 0.45));
        doc.layers.push(text(&format!("Center {i} name"), (110.0, y + 4.0, 760.0, 92.0), &format!("{{{{center{i}.name}}}}"), DISPLAY, 42.0, Weight::Bold, "$text", Align::Left).with_anim(AnimKind::SlideRight, d, 0.45));
        doc.layers.push(text(&format!("Center {i} details"), (890.0, y + 4.0, 920.0, 92.0), &format!("{{{{center{i}.details}}}}"), BODY, 30.0, Weight::SemiBold, "$muted", Align::Right).with_anim(AnimKind::Fade, d + 0.05, 0.4));
    }
    footer(&mut doc, "{{station}}", "Call ahead to confirm hours");
    doc
}

fn community_forecast() -> GraphicDoc {
    let mut doc = cities_now();
    doc.name = "Community Forecast".into();
    if let Some(l) = doc.layers.iter_mut().find(|l| l.name == "Title") {
        if let LayerKind::Text { text, .. } = &mut l.kind {
            *text = "Across the Area".into();
        }
    }
    doc
}

fn sponsor_full() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Sponsor Card", "Show", Some(Fill::linear("#0A0F1A", "#1D2638", 100.0)));
    doc.layers.push(caps(text("Kicker", (100.0, 260.0, 1720.0, 80.0), "Weather brought to you by", BODY, 54.0, Weight::SemiBold, "$muted", Align::Center), 3.0).with_anim(AnimKind::Fade, 0.1, 0.4));
    doc.layers.push(
        Layer::new("Sponsor logo", 610.0, 360.0, 700.0, 320.0, LayerKind::Image { media: "{{sponsor_logo}}".into(), fit: crate::gfx::model::ImageFit::Contain })
            .with_anim(AnimKind::Pop, 0.3, 0.6),
    );
    doc.layers.push(shadowed(text("Sponsor", (100.0, 690.0, 1720.0, 120.0), "{{sponsor}}", DISPLAY, 96.0, Weight::Black, "$text", Align::Center)).with_anim(AnimKind::SlideUp, 0.4, 0.5));
    doc.layers.push(text("Line", (100.0, 820.0, 1720.0, 70.0), "{{sponsor_line}}", BODY, 44.0, Weight::SemiBold, "$primary", Align::Center).with_anim(AnimKind::Fade, 0.6, 0.4));
    doc.duration = 6.0;
    doc
}

fn current_hc() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Current Conditions (high contrast)", "Accessibility", Some(Fill::solid("#000000")));
    doc.layers.push(caps(text("Title", (80.0, 50.0, 1760.0, 110.0), "Current Conditions", DISPLAY, 90.0, Weight::Black, "#FFFF00", Align::Left), 2.0));
    doc.layers.push(text("Temp", (80.0, 180.0, 900.0, 420.0), "{{temp}}", DISPLAY, 360.0, Weight::Black, "#FFFFFF", Align::Left));
    doc.layers.push(wrapped(text("Condition", (1000.0, 200.0, 840.0, 380.0), "{{condition}}", DISPLAY, 100.0, Weight::Black, "#FFFFFF", Align::Left)));
    let rows = [("Feels like", "{{feels}}"), ("Wind", "{{wind}}"), ("Humidity", "{{humidity}}")];
    for (i, (l, v)) in rows.iter().enumerate() {
        let y = 640.0 + i as f32 * 130.0;
        doc.layers.push(text(&format!("{l} label"), (80.0, y, 800.0, 120.0), l, DISPLAY, 80.0, Weight::Bold, "#FFFF00", Align::Left));
        doc.layers.push(text(&format!("{l} value"), (900.0, y, 940.0, 120.0), v, DISPLAY, 90.0, Weight::Black, "#FFFFFF", Align::Right));
    }
    doc
}

fn seven_day_es() -> GraphicDoc {
    let mut doc = super::build("seven_day").unwrap_or_else(|| GraphicDoc::new("7", "Forecast", bg_gradient()));
    for l in &mut doc.layers {
        if let LayerKind::Text { text, .. } = &mut l.kind {
            *text = text
                .replace("7-Day Forecast", "Pronóstico de 7 Días")
                .replace(".short}}", ".name_es}}")
                .replace(".condition}}", ".condition_es}}")
                .replace("Rain ", "Lluvia ")
                .replace("Forecast from the National Weather Service", "Pronóstico del Servicio Meteorológico Nacional");
        }
    }
    doc
}

fn current_es() -> GraphicDoc {
    let mut doc = super::build("current").unwrap_or_else(|| GraphicDoc::new("C", "Forecast", bg_gradient()));
    let words = [
        ("Current Conditions", "Condiciones Actuales"),
        ("{{condition}}", "{{condition_es}}"),
        ("Feels like", "Sensación térmica"),
        ("Humidity", "Humedad"),
        ("Dew point", "Punto de rocío"),
        ("Wind", "Viento"),
        ("Gusts", "Ráfagas"),
        ("Pressure", "Presión"),
        ("Visibility", "Visibilidad"),
    ];
    for l in &mut doc.layers {
        if let LayerKind::Text { text, .. } = &mut l.kind {
            for (en, es) in words {
                *text = text.replace(en, es);
            }
        }
    }
    doc
}

fn heat_index() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Heat Index", "Forecast", Some(Fill::linear("#2A0D05", "#7A2A0A", 115.0)));
    header(&mut doc, "Heat Index", "{{location}}");
    big_stat(&mut doc, "Now", 80.0, 200.0, 860.0, "Feels like now", "{{feels}}", 0.2);
    big_stat(&mut doc, "High", 980.0, 200.0, 860.0, "Model high, next 24 h", "{{temp.hi24}}", 0.3);
    let levels = [("80-90°", "Caution", "#FFE066"), ("90-103°", "Extreme caution", "#FFB347"), ("103-124°", "Danger", "#FF6B3D"), ("125°+", "Extreme danger", "#C0161C")];
    for (i, (r, l, c)) in levels.iter().enumerate() {
        let x = 80.0 + i as f32 * 445.0;
        doc.layers.push(Layer::rect(&format!("{l} box"), x, 560.0, 425.0, 200.0, Fill::solid(c), 12.0).with_anim(AnimKind::SlideUp, 0.4 + i as f32 * 0.06, 0.4));
        doc.layers.push(text(&format!("{l} range"), (x, 575.0, 425.0, 80.0), r, DISPLAY, 56.0, Weight::Black, "#10131A", Align::Center).with_anim(AnimKind::Fade, 0.5, 0.4));
        doc.layers.push(caps(text(&format!("{l} name"), (x, 660.0, 425.0, 80.0), l, BODY, 32.0, Weight::Bold, "#10131A", Align::Center), 1.0).with_anim(AnimKind::Fade, 0.55, 0.4));
    }
    doc.layers.push(wrapped(text("Advice", (80.0, 800.0, 1760.0, 150.0), "Drink water, take breaks in the shade, never leave people or pets in a parked car, and check on neighbors.", BODY, 40.0, Weight::SemiBold, "$text", Align::Center)).with_anim(AnimKind::Fade, 0.7, 0.4));
    doc
}

fn wind_chill() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Wind Chill", "Forecast", Some(Fill::linear("#06142A", "#1C4A80", 115.0)));
    header(&mut doc, "Wind Chill", "{{location}}");
    big_stat(&mut doc, "Now", 80.0, 200.0, 860.0, "Feels like now", "{{feels}}", 0.2);
    big_stat(&mut doc, "Low", 980.0, 200.0, 860.0, "Model low, next 24 h", "{{temp.lo24}}", 0.3);
    let rows = [("Below 0°", "Frostbite in about 30 minutes"), ("Below -20°", "Frostbite in about 10 minutes"), ("Below -35°", "Frostbite in 5 minutes or less")];
    for (i, (a, b)) in rows.iter().enumerate() {
        row_line(&mut doc, i, 560.0 + i as f32 * 120.0, 120.0, a, b, 0.4 + i as f32 * 0.06);
    }
    footer(&mut doc, "Cover skin, dress in layers", "Wind: {{wind}}");
    doc
}

fn visibility_now() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Visibility", "Forecast", bg_gradient());
    header(&mut doc, "Visibility", "{{obs_station}}");
    big_stat(&mut doc, "Vis", 80.0, 220.0, 1760.0, "Visibility now", "{{visibility}}", 0.2);
    doc.layers.push(wrapped(text("Advice", (80.0, 580.0, 1760.0, 300.0), "Under a mile: slow down, use low beams, leave extra room, and never stop in a travel lane.", DISPLAY, 54.0, Weight::Bold, "$text", Align::Center)).with_anim(AnimKind::Fade, 0.4, 0.4));
    footer(&mut doc, "{{station}}", "{{condition}}");
    doc
}

fn sky_now() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Sky Conditions", "Forecast", bg_gradient());
    header(&mut doc, "Sky Conditions", "{{location}}");
    doc.layers.push(Layer::icon("Now icon", 120.0, 200.0, 360.0, "{{condition}}", "{{daypart}}").with_anim(AnimKind::Pop, 0.2, 0.5));
    doc.layers.push(wrapped(shadowed(text("Now", (520.0, 230.0, 1320.0, 300.0), "{{condition}}", DISPLAY, 110.0, Weight::Black, "$text", Align::Left))).with_anim(AnimKind::SlideLeft, 0.3, 0.5));
    doc.layers.push(panel("Chart panel", 80.0, 600.0, 1760.0, 360.0).with_anim(AnimKind::Fade, 0.4, 0.5));
    doc.layers.push(chart("Cloud cover", (120.0, 615.0, 1680.0, 330.0), "hourly.cloud", "", ChartKind::Area, "%").with_anim(AnimKind::Wipe, 0.6, 1.0));
    footer(&mut doc, "{{station}}", "Cloud cover, next 24 hours");
    doc
}

fn dew_humidity() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Dew Point and Humidity", "Forecast", bg_gradient());
    header(&mut doc, "How Muggy?", "{{location}}");
    big_stat(&mut doc, "Dew", 80.0, 200.0, 860.0, "Dew point", "{{dewpoint}}", 0.2);
    big_stat(&mut doc, "Rh", 980.0, 200.0, 860.0, "Humidity", "{{humidity}}", 0.3);
    let levels = [("Under 55°", "Comfortable", "#6FC2FF"), ("55-64°", "Sticky", "#5FD38D"), ("65-69°", "Muggy", "#FFE066"), ("70°+", "Oppressive", "#FF7A45")];
    for (i, (r, l, c)) in levels.iter().enumerate() {
        let x = 80.0 + i as f32 * 445.0;
        doc.layers.push(Layer::rect(&format!("{l} box"), x, 600.0, 425.0, 220.0, Fill::solid(c), 12.0).with_anim(AnimKind::SlideUp, 0.4 + i as f32 * 0.06, 0.4));
        doc.layers.push(text(&format!("{l} range"), (x, 620.0, 425.0, 80.0), r, DISPLAY, 50.0, Weight::Black, "#10131A", Align::Center).with_anim(AnimKind::Fade, 0.5, 0.4));
        doc.layers.push(caps(text(&format!("{l} name"), (x, 710.0, 425.0, 80.0), l, BODY, 34.0, Weight::Bold, "#10131A", Align::Center), 1.0).with_anim(AnimKind::Fade, 0.55, 0.4));
    }
    doc
}

fn temp_departure() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Normals and Records", "Forecast", bg_gradient());
    header(&mut doc, "Normals and Records", "{{date}}");
    big_stat(&mut doc, "NHi", 80.0, 200.0, 560.0, "Normal high", "{{climate.normal_hi}}", 0.2);
    big_stat(&mut doc, "NLo", 680.0, 200.0, 560.0, "Normal low", "{{climate.normal_lo}}", 0.3);
    big_stat(&mut doc, "Dep", 1280.0, 200.0, 560.0, "Today", "{{departure.hi}}", 0.4);
    row_line(&mut doc, 0, 560.0, 130.0, "Record high", "{{climate.record_hi}}", 0.5);
    row_line(&mut doc, 1, 690.0, 130.0, "Record low", "{{climate.record_lo}}", 0.55);
    row_line(&mut doc, 2, 820.0, 130.0, "Tonight vs normal", "{{departure.lo}}", 0.6);
    footer(&mut doc, "{{station}}", "{{climate.normals_source}}");
    doc
}

fn rain_departure() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Rain This Month", "Forecast", Some(Fill::linear("#071A24", "#0F3A4F", 115.0)));
    header(&mut doc, "Rain This Month", "{{location}}");
    big_stat(&mut doc, "Mtd", 80.0, 220.0, 860.0, "So far", "{{rain.mtd}}", 0.2);
    big_stat(&mut doc, "Normal", 980.0, 220.0, 860.0, "Normal to date", "{{rain.mtd_normal}}", 0.3);
    doc.layers.push(shadowed(text("Departure", (80.0, 600.0, 1760.0, 140.0), "{{rain.departure}}", DISPLAY, 96.0, Weight::Black, "$text", Align::Center)).with_anim(AnimKind::Pop, 0.5, 0.5));
    footer(&mut doc, "Next 7 days: {{qpf.day7}}", "Rain: model analysis · {{climate.normals_source}}");
    doc
}

fn hourly_rain() -> GraphicDoc {
    chart_doc("Hourly Rain", "hourly.rain", "", ChartKind::Bar, "\"", "Precipitation per hour, next 24 hours")
}

fn wind_advisory() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Wind Advisory Impacts", "Severe", bg_gradient());
    header(&mut doc, "Strong Winds", "Peak gust {{gust.max24}}");
    let rows = [("30-45 mph", "Loose objects blow around, tree limbs break"), ("46-57 mph", "Wind Advisory / High Wind level: scattered outages"), ("58 mph+", "Severe-level gusts: trees down, widespread outages")];
    for (i, (a, b)) in rows.iter().enumerate() {
        row_line(&mut doc, i, 200.0 + i as f32 * 150.0, 150.0, a, b, 0.2 + i as f32 * 0.08);
    }
    doc.layers.push(wrapped(text("Advice", (80.0, 700.0, 1760.0, 200.0), "Secure outdoor furniture and decorations; high-profile vehicles use caution.", BODY, 44.0, Weight::SemiBold, "$primary", Align::Center)).with_anim(AnimKind::Fade, 0.5, 0.4));
    doc
}

fn beaufort() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Beaufort Wind Scale", "Forecast", bg_gradient());
    header(&mut doc, "Beaufort Wind Scale", "Now: {{wind}}");
    let rows = [("0-3", "Calm to light air", "Smoke drifts"), ("4-12", "Breeze", "Leaves rustle, flags flap"), ("13-24", "Moderate to fresh", "Small branches move, dust blows"), ("25-38", "Strong to near gale", "Whole trees sway, hard to walk"), ("39-54", "Gale to strong gale", "Twigs break, minor damage"), ("55-72", "Storm to violent storm", "Trees uprooted, damage"), ("73+", "Hurricane force", "Widespread severe damage")];
    for (i, (s, n, e)) in rows.iter().enumerate() {
        let y = 180.0 + i as f32 * 112.0;
        doc.layers.push(panel(&format!("B{i}"), 80.0, y, 1760.0, 100.0).with_anim(AnimKind::SlideRight, 0.2 + i as f32 * 0.05, 0.4));
        doc.layers.push(text(&format!("B{i} speed"), (110.0, y, 260.0, 100.0), &format!("{s} mph"), DISPLAY, 40.0, Weight::Black, "$primary", Align::Left).with_anim(AnimKind::Fade, 0.25 + i as f32 * 0.05, 0.4));
        doc.layers.push(text(&format!("B{i} name"), (390.0, y, 640.0, 100.0), n, DISPLAY, 40.0, Weight::Bold, "$text", Align::Left).with_anim(AnimKind::Fade, 0.3 + i as f32 * 0.05, 0.4));
        doc.layers.push(text(&format!("B{i} effect"), (1040.0, y, 780.0, 100.0), e, BODY, 32.0, Weight::SemiBold, "$muted", Align::Left).with_anim(AnimKind::Fade, 0.3 + i as f32 * 0.05, 0.4));
    }
    doc
}

fn snow_ratio() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Snow-to-Liquid Ratio", "Forecast", Some(Fill::linear("#0B1830", "#24486E", 115.0)));
    header(&mut doc, "Snow-to-Liquid Ratio", "{{location}}");
    let rows = [("Wet, heavy snow (near 32°)", "About 8-10 inches per inch of water"), ("Average snow (20s)", "About 10-12 inches per inch of water"), ("Dry, fluffy snow (teens)", "15-20 inches or more per inch of water")];
    for (i, (a, b)) in rows.iter().enumerate() {
        row_line(&mut doc, i, 200.0 + i as f32 * 150.0, 150.0, a, b, 0.2 + i as f32 * 0.08);
    }
    big_stat(&mut doc, "Snow", 80.0, 660.0, 860.0, "Model snow, 48 h", "{{snow.total48}}", 0.5);
    big_stat(&mut doc, "Liquid", 980.0, 660.0, 860.0, "Model precip, 48 h", "{{rain.total48}}", 0.6);
    doc
}

fn blizzard() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Blizzard Conditions", "Severe", Some(Fill::linear("#0E1626", "#3A4A66", 115.0)));
    header(&mut doc, "Blizzard Conditions", "{{location}}");
    let rows = [("Wind", "Sustained or gusts 35 mph or more"), ("Visibility", "Blowing snow under 1/4 mile"), ("Duration", "At least 3 hours"), ("Impacts", "Whiteouts, drifting, travel impossible")];
    for (i, (a, b)) in rows.iter().enumerate() {
        row_line(&mut doc, i, 190.0 + i as f32 * 140.0, 140.0, a, b, 0.2 + i as f32 * 0.08);
    }
    doc.layers.push(text("Now", (80.0, 780.0, 1760.0, 80.0), "Peak gust {{gust.max24}} • snow {{snow.total48}} in 48 h", DISPLAY, 50.0, Weight::Bold, "$primary", Align::Center).with_anim(AnimKind::Fade, 0.6, 0.4));
    footer(&mut doc, "Stay off the roads", "{{station}}");
    doc
}

fn hazard_icons() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Storm Hazards", "Severe", bg_gradient());
    header(&mut doc, "Today's Storm Hazards", "{{location}}");
    let items = [("Tornado", "Thunderstorm", "{{spc.home.tornado}}"), ("Hail", "Hail", "{{spc.home.hail}}"), ("Wind", "Wind", "{{spc.home.wind}}"), ("Flooding", "Heavy Rain", "{{ero.day1}}"), ("Lightning", "Thunderstorm", "{{lightning.all_clear}}")];
    for (i, (name, icon, value)) in items.iter().enumerate() {
        let x = 80.0 + i as f32 * 356.0;
        let d = 0.2 + i as f32 * 0.08;
        doc.layers.push(panel(&format!("{name} card"), x, 200.0, 336.0, 740.0).with_anim(AnimKind::SlideUp, d, 0.45));
        doc.layers.push(Layer::icon(&format!("{name} icon"), x + 48.0, 240.0, 240.0, icon, "day").with_anim(AnimKind::Pop, d + 0.1, 0.5));
        doc.layers.push(caps(text(&format!("{name} label"), (x, 520.0, 336.0, 70.0), name, DISPLAY, 46.0, Weight::Bold, "$primary", Align::Center), 2.0).with_anim(AnimKind::Fade, d + 0.1, 0.4));
        doc.layers.push(wrapped(text(&format!("{name} value"), (x + 16.0, 620.0, 304.0, 260.0), value, DISPLAY, 50.0, Weight::Black, "$text", Align::Center)).with_anim(AnimKind::Fade, d + 0.15, 0.4));
    }
    doc
}

fn confidence() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Forecast Confidence", "Forecast", bg_gradient());
    header(&mut doc, "Forecast Confidence", "Tomorrow");
    doc.layers.push(shadowed(text("Level", (80.0, 185.0, 1760.0, 135.0), "{{models.confidence}}", DISPLAY, 86.0, Weight::Black, "{{models.confidence_color}}", Align::Center)).with_anim(AnimKind::Fade, 0.2, 0.4));
    for (i, (label, color)) in [("LOW • MODELS APART", "#FFA297"), ("MEDIUM", "#FFD778"), ("HIGH • MODELS CLOSE", "#65E5B1")].into_iter().enumerate() {
        let x = 330.0 + i as f32 * 425.0;
        doc.layers.push(Layer::rect(label, x, 328.0, 410.0, 7.0, Fill::solid(color), 3.0));
        doc.layers.push(text(label, (x, 346.0, 410.0, 45.0), label, BODY, 25.0, Weight::Bold, color, Align::Center));
    }
    doc.layers.push(text("Models", (80.0, 412.0, 1760.0, 70.0), "{{models.tomorrow}}", DISPLAY, 42.0, Weight::Bold, "$primary", Align::Center).with_anim(AnimKind::Fade, 0.4, 0.4));
    doc.layers.push(panel("Chart panel", 80.0, 510.0, 1760.0, 440.0).with_anim(AnimKind::Fade, 0.5, 0.5));
    doc.layers.push(chart("Models", (120.0, 525.0, 1680.0, 410.0), "models.gfs", "models.ecmwf", ChartKind::Line, "°").with_anim(AnimKind::Wipe, 0.7, 1.0));
    footer(&mut doc, "GFS (solid) vs ECMWF (light)", "Agreement = confidence");
    doc
}

fn tds_explainer() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Debris Signature", "Severe", Some(Fill::linear("#22070A", "#5C0E16", 115.0)));
    header(&mut doc, "Tornado Debris Signature", "What radar is seeing");
    let rows = [("What it is", "Radar detecting debris lofted by a tornado, not rain or hail"), ("How we see it", "Low correlation coefficient right where velocity shows rotation"), ("What it means", "A tornado is on the ground and doing damage"), ("What to do", "If you are in the path, take shelter now")];
    for (i, (a, b)) in rows.iter().enumerate() {
        row_line(&mut doc, i, 190.0 + i as f32 * 180.0, 170.0, a, b, 0.2 + i as f32 * 0.08);
    }
    doc
}

fn power_doc(mut doc: GraphicDoc) -> GraphicDoc {
    doc.category = "Power".into();
    // Say where the numbers come from (or that no utilities are set up yet).
    for l in &mut doc.layers {
        if l.name == "Footer right" {
            if let LayerKind::Text { text, .. } = &mut l.kind {
                *text = "{{power.status}}".into();
            }
        }
    }
    doc
}

fn power_counter() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Customers Without Power", "Power", bg_gradient());
    header(&mut doc, "Without Power", "{{location}}");
    doc.layers.push(panel("Counter panel", 360.0, 230.0, 1200.0, 470.0).with_anim(AnimKind::Pop, 0.2, 0.5));
    doc.layers.push(text("Count", (360.0, 260.0, 1200.0, 260.0), "{{power.total}}", DISPLAY, 230.0, Weight::Black, "$primary", Align::Center).with_anim(AnimKind::Pop, 0.35, 0.5));
    doc.layers.push(text("Label", (360.0, 530.0, 1200.0, 70.0), "customers without power ({{power.pct}})", BODY, 48.0, Weight::SemiBold, "$text", Align::Center).with_anim(AnimKind::Fade, 0.5, 0.4));
    doc.layers.push(text("Trend", (360.0, 610.0, 1200.0, 60.0), "{{power.trend}}", BODY, 40.0, Weight::SemiBold, "$text", Align::Center).with_anim(AnimKind::Fade, 0.65, 0.4));
    row_line(&mut doc, 0, 760.0, 110.0, "Most outages: {{power.top1}}", "{{power.top1.count}}", 0.8);
    footer(&mut doc, "Utility outage maps", "{{station}}");
    doc
}

fn warning_history() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Warnings Today", "Severe", bg_gradient());
    header(&mut doc, "Warnings Today", "{{location}}");
    for i in 0..7 {
        row_line(&mut doc, i, 180.0 + i as f32 * 110.0, 110.0, &format!("{{{{history{i}.line}}}}"), "", 0.2 + i as f32 * 0.05);
    }
    footer(&mut doc, "{{history.count}} warnings today", "Source: National Weather Service");
    doc
}

fn photo_grid() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Storm Damage Photos", "Severe", bg_gradient());
    header(&mut doc, "Storm Damage", "{{location}}");
    for i in 0..4 {
        let (x, y) = (80.0 + (i % 2) as f32 * 890.0, 180.0 + (i / 2) as f32 * 400.0);
        let d = 0.2 + i as f32 * 0.1;
        doc.layers.push(Layer::new(&format!("Photo {}", i + 1), x, y, 870.0, 320.0, LayerKind::Image { media: String::new(), fit: crate::gfx::model::ImageFit::Cover }).with_anim(AnimKind::Fade, d, 0.5));
        doc.layers.push(text(&format!("Caption {}", i + 1), (x, y + 325.0, 870.0, 60.0), "Caption: place, time, photo credit", BODY, 30.0, Weight::SemiBold, "$text", Align::Left).with_anim(AnimKind::Fade, d + 0.1, 0.4));
    }
    doc
}

fn impact_summary() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Storm Impact Summary", "Severe", bg_gradient());
    header(&mut doc, "Storm Impacts", "Last 24 hours");
    row_line(&mut doc, 0, 190.0, 150.0, "Storm reports", "{{reports.count}}", 0.2);
    row_line(&mut doc, 1, 340.0, 150.0, "{{reports.summary}}", "", 0.25);
    row_line(&mut doc, 2, 490.0, 150.0, "Warnings issued", "{{history.count}}", 0.3);
    row_line(&mut doc, 3, 640.0, 150.0, "Rivers", "{{river.summary}}", 0.35);
    row_line(&mut doc, 4, 790.0, 150.0, "Roads", "{{roads.summary}}", 0.4);
    doc
}

fn urban_flood() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Urban Flooding Risk", "Flooding", Some(Fill::linear("#071A24", "#0F3A4F", 115.0)));
    header(&mut doc, "Street Flooding Risk", "{{location}}");
    big_stat(&mut doc, "Ero", 80.0, 200.0, 860.0, "Flooding rain risk", "{{ero.day1}}", 0.2);
    big_stat(&mut doc, "Rain", 980.0, 200.0, 860.0, "Rain, next 24 h", "{{qpf.day1}}", 0.3);
    doc.layers.push(wrapped(text("Advice", (80.0, 560.0, 1760.0, 300.0), "Underpasses, low streets and parking lots flood first. Six inches of moving water can knock you down; a foot can float a car. Turn around, don't drown.", DISPLAY, 50.0, Weight::Bold, "$text", Align::Center)).with_anim(AnimKind::Fade, 0.5, 0.4));
    doc
}

fn black_ice() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Black Ice Risk", "Travel", Some(Fill::linear("#06142A", "#1C4A80", 115.0)));
    header(&mut doc, "Black Ice Risk", "{{location}}");
    big_stat(&mut doc, "Low", 80.0, 200.0, 860.0, "Low, next 24 h", "{{temp.lo24}}", 0.2);
    big_stat(&mut doc, "Am", 980.0, 200.0, 860.0, "Morning drive", "{{part.morning.temp}}", 0.3);
    doc.layers.push(wrapped(text("Advice", (80.0, 560.0, 1760.0, 300.0), "Bridges, overpasses and shaded curves freeze first. If the road looks wet but it's below freezing, treat it as ice: slow down and brake gently.", DISPLAY, 50.0, Weight::Bold, "$text", Align::Center)).with_anim(AnimKind::Fade, 0.5, 0.4));
    footer(&mut doc, "{{commute.am}}", "{{station}}");
    doc
}

fn low_visibility() -> GraphicDoc {
    let mut doc = visibility_now();
    doc.name = "Reduced Visibility".into();
    doc
}

fn airport_delays() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Airport Delays", "Travel", bg_gradient());
    header(&mut doc, "Airport Delays", "{{airport.summary}}");
    for i in 0..7 {
        row_line(&mut doc, i, 180.0 + i as f32 * 110.0, 110.0, &format!("{{{{airport{i}.line}}}}"), "", 0.2 + i as f32 * 0.05);
    }
    footer(&mut doc, "Check with your airline", "Source: FAA National Airspace System status");
    doc
}

fn school_transport() -> GraphicDoc {
    let mut doc = GraphicDoc::new("School Buses and Weather", "Community", bg_gradient());
    header(&mut doc, "Bus Stop Forecast", "{{location}}");
    big_stat(&mut doc, "Temp", 80.0, 200.0, 560.0, "7 AM", "{{part.morning.temp}}", 0.2);
    big_stat(&mut doc, "Feels", 680.0, 200.0, 560.0, "Feels like now", "{{feels}}", 0.3);
    big_stat(&mut doc, "Closings", 1280.0, 200.0, 560.0, "Closings / delays", "{{closings.count}}", 0.4);
    doc.layers.push(wrapped(text("Commute", (80.0, 560.0, 1760.0, 200.0), "{{commute.am}}", DISPLAY, 54.0, Weight::Bold, "$text", Align::Center)).with_anim(AnimKind::Fade, 0.5, 0.4));
    footer(&mut doc, "{{closings.schools.ticker}}", "{{station}}");
    doc
}

fn closings_cat(title: &str, key: &str) -> GraphicDoc {
    let mut doc = GraphicDoc::new(title, "Community", bg_gradient());
    header(&mut doc, title, "{{location}}");
    doc.layers.push(panel("Panel", 80.0, 190.0, 1760.0, 760.0).with_anim(AnimKind::Fade, 0.2, 0.5));
    doc.layers.push(wrapped(text("List", (120.0, 220.0, 1680.0, 700.0), &format!("{{{{closings.{key}.list}}}}"), DISPLAY, 46.0, Weight::Bold, "$text", Align::Left)).with_anim(AnimKind::Fade, 0.3, 0.5));
    footer(&mut doc, "{{station}}", "Community board");
    doc
}

fn closings_business() -> GraphicDoc {
    closings_cat("Business Closings", "business")
}

fn closings_government() -> GraphicDoc {
    closings_cat("Government Closings", "government")
}

fn closings_events() -> GraphicDoc {
    closings_cat("Event Cancellations", "events")
}

fn emergency_declaration() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Emergency Declaration", "Community", Some(Fill::linear("#22070A", "#5C0E16", 115.0)));
    doc.layers.push(Layer::rect("Bar", 0.0, 120.0, 1920.0, 180.0, Fill::solid("$alert"), 0.0).with_anim(AnimKind::Wipe, 0.0, 0.5));
    doc.layers.push(caps(text("Title", (80.0, 120.0, 1760.0, 180.0), "Local Emergency Declared", DISPLAY, 110.0, Weight::Black, "#FFFFFF", Align::Center), 4.0).with_anim(AnimKind::Pop, 0.2, 0.5));
    doc.layers.push(wrapped(text("Body", (120.0, 380.0, 1680.0, 420.0), "County officials have declared a local emergency. Travel is restricted to emergency vehicles only. Edit this text with the official wording.", DISPLAY, 60.0, Weight::Bold, "$text", Align::Center)).with_anim(AnimKind::Fade, 0.4, 0.5));
    footer(&mut doc, "{{station}}", "Follow local officials' instructions");
    doc
}

fn community_dashboard() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Community Impacts", "Community", bg_gradient());
    header(&mut doc, "Community Impacts", "{{location}}");
    big_stat(&mut doc, "Alerts", 80.0, 200.0, 425.0, "Alerts", "{{alert.count}}", 0.2);
    big_stat(&mut doc, "Closings", 525.0, 200.0, 425.0, "Closings", "{{closings.count}}", 0.25);
    big_stat(&mut doc, "Reports", 970.0, 200.0, 425.0, "Storm reports", "{{reports.count}}", 0.3);
    big_stat(&mut doc, "Centers", 1415.0, 200.0, 425.0, "Centers open", "{{centers.count}}", 0.35);
    row_line(&mut doc, 0, 560.0, 130.0, "Roads", "{{roads.summary}}", 0.4);
    row_line(&mut doc, 1, 690.0, 130.0, "Rivers", "{{river.summary}}", 0.45);
    row_line(&mut doc, 2, 820.0, 130.0, "{{alerts.ticker}}", "", 0.5);
    doc
}

fn shelters() -> GraphicDoc {
    let mut doc = centers();
    doc.name = "Emergency Shelters".into();
    for l in &mut doc.layers {
        if let LayerKind::Text { text, .. } = &mut l.kind {
            *text = text.replace("Warming and Cooling Centers", "Emergency Shelters").replace("{{center", "{{shelter");
        }
    }
    doc
}

fn workweek() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Workweek Planner", "Forecast", bg_gradient());
    header(&mut doc, "Workweek Planner", "{{location}}");
    for i in 0..5 {
        let x = 80.0 + i as f32 * 356.0;
        let d = 0.2 + i as f32 * 0.08;
        doc.layers.push(panel(&format!("Day {i}"), x, 200.0, 336.0, 740.0).with_anim(AnimKind::SlideUp, d, 0.45));
        doc.layers.push(caps(text(&format!("Day {i} name"), (x, 220.0, 336.0, 70.0), &format!("{{{{week{i}.name}}}}"), DISPLAY, 50.0, Weight::Bold, "$primary", Align::Center), 2.0).with_anim(AnimKind::SlideUp, d, 0.45));
        doc.layers.push(Layer::icon(&format!("Day {i} icon"), x + 68.0, 310.0, 200.0, &format!("{{{{week{i}.cond}}}}"), "day").with_anim(AnimKind::Pop, d + 0.1, 0.45));
        doc.layers.push(shadowed(text(&format!("Day {i} hi"), (x, 540.0, 336.0, 120.0), &format!("{{{{week{i}.hi}}}}"), DISPLAY, 100.0, Weight::Black, "$text", Align::Center)).with_anim(AnimKind::SlideUp, d, 0.45));
        doc.layers.push(wrapped(text(&format!("Day {i} cond"), (x + 12.0, 680.0, 312.0, 160.0), &format!("{{{{week{i}.cond}}}}"), BODY, 32.0, Weight::SemiBold, "$text", Align::Center)).with_anim(AnimKind::Fade, d + 0.1, 0.4));
        doc.layers.push(text(&format!("Day {i} pop"), (x, 860.0, 336.0, 60.0), &format!("Rain {{{{week{i}.pop}}}}"), BODY, 30.0, Weight::Bold, "#5EC8FF", Align::Center).with_anim(AnimKind::Fade, d + 0.15, 0.4));
    }
    doc
}

fn ten_day() -> GraphicDoc {
    let mut doc = GraphicDoc::new("10-Day Forecast", "Forecast", bg_gradient());
    header(&mut doc, "10-Day Forecast", "{{location}}");
    let w = 172.0;
    for i in 0..10 {
        let x = 80.0 + i as f32 * (w + 4.0);
        let d = 0.2 + i as f32 * 0.05;
        doc.layers.push(panel(&format!("Day {i}"), x, 190.0, w, 760.0).with_anim(AnimKind::SlideUp, d, 0.45));
        doc.layers.push(caps(text(&format!("Day {i} name"), (x, 205.0, w, 60.0), &format!("{{{{ten{i}.name}}}}"), DISPLAY, 38.0, Weight::Bold, "$primary", Align::Center), 1.0).with_anim(AnimKind::SlideUp, d, 0.45));
        doc.layers.push(text(&format!("Day {i} date"), (x, 260.0, w, 40.0), &format!("{{{{ten{i}.date}}}}"), BODY, 26.0, Weight::SemiBold, "$muted", Align::Center).with_anim(AnimKind::Fade, d, 0.4));
        doc.layers.push(Layer::icon(&format!("Day {i} icon"), x + 16.0, 320.0, 140.0, &format!("{{{{ten{i}.cond}}}}"), "day").with_anim(AnimKind::Pop, d + 0.1, 0.45));
        doc.layers.push(shadowed(text(&format!("Day {i} hi"), (x, 500.0, w, 100.0), &format!("{{{{ten{i}.hi}}}}"), DISPLAY, 74.0, Weight::Black, "$text", Align::Center)).with_anim(AnimKind::SlideUp, d, 0.45));
        doc.layers.push(text(&format!("Day {i} lo"), (x, 610.0, w, 80.0), &format!("{{{{ten{i}.lo}}}}"), DISPLAY, 50.0, Weight::Bold, "$muted", Align::Center).with_anim(AnimKind::SlideUp, d, 0.45));
        doc.layers.push(text(&format!("Day {i} pop"), (x, 860.0, w, 60.0), &format!("{{{{ten{i}.pop}}}}"), BODY, 30.0, Weight::Bold, "#5EC8FF", Align::Center).with_anim(AnimKind::Fade, d + 0.15, 0.4));
    }
    footer(&mut doc, "{{station}}", "Days 8-10: lower confidence");
    doc
}

fn outdoor_planner() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Outdoor Planner", "Forecast", bg_gradient());
    header(&mut doc, "Outdoor Planner", "{{location}}");
    for i in 0..7 {
        row_line(&mut doc, i, 180.0 + i as f32 * 110.0, 110.0, &format!("{{{{ten{i}.name}}}}: {{{{ten{i}.cond}}}}, {{{{ten{i}.hi}}}}"), &format!("{{{{outdoor{i}}}}}"), 0.2 + i as f32 * 0.05);
    }
    footer(&mut doc, "Good: dry and pleasant • Fair: some rain or heat/cold • Poor: wet or harsh", "{{station}}");
    doc
}

fn impact_by_time() -> GraphicDoc {
    let mut doc = day_parts();
    doc.name = "Impacts by Time of Day".into();
    for l in &mut doc.layers {
        if l.name == "Title" {
            if let LayerKind::Text { text, .. } = &mut l.kind {
                *text = "Impacts Through the Day".into();
            }
        }
    }
    doc
}

fn confidence_timeline() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Confidence Timeline", "Forecast", bg_gradient());
    header(&mut doc, "Forecast Confidence", "{{location}}");
    for i in 0..7 {
        row_line(&mut doc, i, 180.0 + i as f32 * 110.0, 110.0, &format!("{{{{ten{i}.name}}}}: {{{{ten{i}.cond}}}}"), &format!("{{{{confidence{i}}}}}"), 0.2 + i as f32 * 0.05);
    }
    footer(&mut doc, "Confidence falls with time and model disagreement", "{{models.confidence}} for tomorrow");
    doc
}

fn sky_card(title: &str, big: &str, line: &str, note: &str) -> GraphicDoc {
    let mut doc = GraphicDoc::new(title, "Sky", Some(Fill::linear("#05060F", "#1B1F44", 120.0)));
    header(&mut doc, title, "{{location}}");
    doc.layers.push(wrapped(shadowed(text("Big", (80.0, 220.0, 1760.0, 300.0), big, DISPLAY, 90.0, Weight::Black, "$text", Align::Center))).with_anim(AnimKind::Pop, 0.2, 0.5));
    doc.layers.push(wrapped(text("Line", (80.0, 560.0, 1760.0, 200.0), line, DISPLAY, 54.0, Weight::Bold, "$primary", Align::Center)).with_anim(AnimKind::Fade, 0.4, 0.4));
    footer(&mut doc, note, "{{station}}");
    doc
}

fn meteor() -> GraphicDoc {
    sky_card("Meteor Shower", "{{meteor.next}}", "Tonight: {{sky.tonight}}", "Best after midnight, away from city lights")
}

fn eclipse() -> GraphicDoc {
    sky_card("Next Eclipse", "{{eclipse.next}}", "Never look at the sun without certified eclipse glasses", "Confirm local times on NASA's eclipse site")
}

fn aurora() -> GraphicDoc {
    sky_card("Northern Lights", "{{aurora.chance}}", "Forecast {{aurora.kp}} • tonight: {{sky.tonight}}", "Source: NOAA Space Weather Prediction Center")
}

fn frost_dates() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Frost and Freeze Dates", "Forecast", bg_gradient());
    header(&mut doc, "Frost and Freeze", "{{location}}");
    big_stat(&mut doc, "Last", 80.0, 220.0, 860.0, "Typical last spring freeze", "{{frost.last}}", 0.2);
    big_stat(&mut doc, "First", 980.0, 220.0, 860.0, "Typical first fall freeze", "{{frost.first}}", 0.3);
    big_stat(&mut doc, "Low", 80.0, 580.0, 1760.0, "Coldest in the next 24 hours", "{{temp.lo24}}", 0.4);
    doc
}

fn growing_season() -> GraphicDoc {
    let mut doc = frost_dates();
    doc.name = "Growing Season".into();
    if let Some(l) = doc.layers.iter_mut().find(|l| l.name == "Low value") {
        if let LayerKind::Text { text, .. } = &mut l.kind {
            *text = "{{season.growing}}".into();
        }
    }
    if let Some(l) = doc.layers.iter_mut().find(|l| l.name == "Low label") {
        if let LayerKind::Text { text, .. } = &mut l.kind {
            *text = "Typical growing season".into();
        }
    }
    doc
}

fn night_sky() -> GraphicDoc {
    sky_card("Night Sky Tonight", "{{sky.tonight}}", "Moon: {{moon.phase}} ({{moon.illum}} lit) • sunset {{sun.set}}", "{{meteor.next}}")
}

fn model_compare() -> GraphicDoc {
    let mut doc = chart_doc("Model Comparison", "models.gfs", "models.ecmwf", ChartKind::Line, "°", "GFS (solid) vs ECMWF (light): daily highs");
    doc.layers.push(text("Tomorrow", (80.0, 160.0, 1760.0, 40.0), "Tomorrow: {{models.tomorrow}} • {{models.confidence}}", BODY, 30.0, Weight::Bold, "$primary", Align::Left).with_anim(AnimKind::Fade, 0.4, 0.4));
    doc
}

fn winter_travel() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Winter Travel", "Travel", Some(Fill::linear("#06142A", "#1C4A80", 115.0)));
    header(&mut doc, "Winter Travel", "{{location}}");
    row_line(&mut doc, 0, 190.0, 140.0, "When", "{{winter.timeline}}", 0.2);
    row_line(&mut doc, 1, 330.0, 140.0, "Snow / ice", "{{snow.total48}} / {{ice.total48}}", 0.25);
    row_line(&mut doc, 2, 470.0, 140.0, "Morning drive", "{{commute.am}}", 0.3);
    row_line(&mut doc, 3, 610.0, 140.0, "Evening drive", "{{commute.pm}}", 0.35);
    row_line(&mut doc, 4, 750.0, 140.0, "Roads", "{{roads.summary}}", 0.4);
    doc
}

/// Four steps revealed one after another (about 2.2 s apart), each with a numbered badge and a
/// progress bar under the steps: an animated explainer that plays out over ~11 seconds.
fn explainer(title: &str, subtitle: &str, steps: &[(&str, &str)]) -> GraphicDoc {
    let mut doc = GraphicDoc::new(title, "Explainers", bg_gradient());
    header(&mut doc, title, subtitle);
    let n = steps.len().max(1) as f32;
    let gap = 30.0;
    let w = (1760.0 - gap * (n - 1.0)) / n;
    for (i, (name, body)) in steps.iter().enumerate() {
        let x = 80.0 + i as f32 * (w + gap);
        let d = 0.4 + i as f32 * 2.2;
        doc.layers.push(panel(&format!("Step {} panel", i + 1), x, 200.0, w, 700.0).with_anim(AnimKind::SlideUp, d, 0.5));
        doc.layers.push(Layer::rect(&format!("Step {} badge", i + 1), x + 30.0, 230.0, 90.0, 90.0, Fill::solid("$primary"), 45.0).with_anim(AnimKind::Pop, d + 0.15, 0.4));
        doc.layers.push(text(&format!("Step {} number", i + 1), (x + 30.0, 230.0, 90.0, 90.0), &(i + 1).to_string(), DISPLAY, 56.0, Weight::Black, "#FFFFFF", Align::Center).with_anim(AnimKind::Pop, d + 0.2, 0.4));
        doc.layers.push(wrapped(text(&format!("Step {} title", i + 1), (x + 30.0, 350.0, w - 60.0, 110.0), name, DISPLAY, 50.0, Weight::Bold, "$text", Align::Left)).with_anim(AnimKind::Fade, d + 0.3, 0.4));
        doc.layers.push(wrapped(text(&format!("Step {} body", i + 1), (x + 30.0, 470.0, w - 60.0, 400.0), body, BODY, 36.0, Weight::SemiBold, "$text", Align::Left)).with_anim(AnimKind::Fade, d + 0.5, 0.5));
        doc.layers.push(Layer::rect(&format!("Step {} progress", i + 1), x, 930.0, w, 12.0, Fill::solid("$secondary"), 6.0).with_anim(AnimKind::Wipe, d + 0.2, 1.8));
    }
    footer(&mut doc, "{{station}}", "{{tagline}}");
    doc.duration = 0.4 + n * 2.2 + 2.0;
    doc
}

fn explain_thunderstorm() -> GraphicDoc {
    explainer("How Thunderstorms Form", "Warm, moist air rising into cooler air", &[("Moisture", "Warm, humid air sits near the ground. Dew points in the 60s and 70s give storms their fuel."), ("Lift", "A front, a boundary or daytime heating pushes that air upward."), ("Instability", "The rising air stays warmer than its surroundings, so it keeps climbing and builds a tall cloud."), ("Storm", "Rain, hail and lightning form. Downdrafts bring gusty wind to the ground.")])
}

fn explain_cold_front() -> GraphicDoc {
    explainer("Cold Front Explained", "What happens as it passes", &[("Ahead", "Warm, humid and breezy from the south. Clouds build in the afternoon."), ("At the front", "Colder, denser air shoves the warm air up fast. A line of storms can form."), ("Passing", "Wind turns northwest and gusts. The temperature drops quickly."), ("Behind", "Drier, cooler air. Skies clear and the pressure rises.")])
}

fn explain_lake_effect() -> GraphicDoc {
    explainer("How Lake-Effect Snow Forms", "Cold air over warm water", &[("Cold air", "Arctic air blows across a lake that is still relatively warm."), ("Moisture", "The lake adds heat and moisture to the lowest layer of air."), ("Bands", "That air rises and forms narrow bands of heavy snow downwind."), ("Impact", "A few miles can mean the difference between a dusting and a foot or more.")])
}

fn explain_fog() -> GraphicDoc {
    explainer("Why Fog Forms", "Air cooling to its dew point", &[("Clear night", "Clear skies and light wind let the ground lose heat quickly."), ("Cooling", "The air near the ground cools down toward its dew point."), ("Saturation", "Water vapor condenses into tiny droplets: a cloud at ground level."), ("Burn-off", "Morning sun warms the ground and the fog lifts and thins out.")])
}

fn explain_hail() -> GraphicDoc {
    explainer("How Hail Grows", "Ice riding a strong updraft", &[("Updraft", "A strong storm updraft carries raindrops high above the freezing level."), ("Freezing", "The drops freeze into small ice pellets."), ("Growing", "Supercooled water freezes onto the pellet in layers as it cycles in the updraft."), ("Falling", "When it is too heavy for the updraft to hold, it falls as hail.")])
}

fn local_threats() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Local Threat Summary", "Severe", bg_gradient());
    header(&mut doc, "Storm Threat by Town", "{{spc.home}}");
    for i in 0..5 {
        let k = i + 1;
        let y = 200.0 + i as f32 * 150.0;
        let d = 0.2 + i as f32 * 0.12;
        doc.layers.push(panel(&format!("Town {k} panel"), 80.0, y, 1760.0, 130.0).with_anim(AnimKind::SlideLeft, d, 0.45));
        doc.layers.push(text(&format!("Town {k} name"), (110.0, y + 10.0, 520.0, 60.0), &format!("{{{{threat{k}.town}}}}"), DISPLAY, 46.0, Weight::Bold, "$text", Align::Left).with_anim(AnimKind::Fade, d + 0.1, 0.4));
        doc.layers.push(text(&format!("Town {k} level"), (110.0, y + 70.0, 520.0, 50.0), &format!("{{{{threat{k}.level}}}}"), BODY, 34.0, Weight::SemiBold, "$warning", Align::Left).with_anim(AnimKind::Fade, d + 0.15, 0.4));
        doc.layers.push(wrapped(text(&format!("Town {k} summary"), (660.0, y + 14.0, 1150.0, 110.0), &format!("{{{{threat{k}.summary}}}}"), BODY, 36.0, Weight::SemiBold, "$text", Align::Left)).with_anim(AnimKind::Fade, d + 0.2, 0.4));
    }
    footer(&mut doc, "Source: NWS Storm Prediction Center • timing from model guidance", "{{station}}");
    doc
}

fn camera_view() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Camera View", "Travel", bg_gradient());
    doc.layers.push(Layer::new("Camera", 0.0, 0.0, 1920.0, 1080.0, LayerKind::Image { media: "{{camera1.image}}".into(), fit: crate::gfx::model::ImageFit::Cover }).with_anim(AnimKind::Fade, 0.0, 0.5));
    doc.layers.push(panel("Label panel", 80.0, 900.0, 1100.0, 110.0).with_anim(AnimKind::SlideRight, 0.2, 0.45));
    doc.layers.push(text("Camera name", (110.0, 910.0, 1040.0, 56.0), "{{camera1.name}}", DISPLAY, 48.0, Weight::Bold, "$text", Align::Left).with_anim(AnimKind::Fade, 0.35, 0.4));
    doc.layers.push(text("Conditions", (110.0, 962.0, 1040.0, 40.0), "{{temp}} • {{condition}} • Live camera", BODY, 32.0, Weight::SemiBold, "$text", Align::Left).with_anim(AnimKind::Fade, 0.45, 0.4));
    doc
}

fn camera_inset() -> GraphicDoc {
    let mut doc = templates_blank_overlay("Camera Inset");
    doc.layers.push(Layer::rect("Inset frame", 1300.0, 90.0, 540.0, 360.0, Fill::solid("$panel"), 10.0).with_anim(AnimKind::SlideLeft, 0.0, 0.45));
    doc.layers.push(Layer::new("Inset camera", 1310.0, 100.0, 520.0, 292.0, LayerKind::Image { media: "{{camera.storm.image}}".into(), fit: crate::gfx::model::ImageFit::Cover }).with_anim(AnimKind::Fade, 0.2, 0.4));
    doc.layers.push(text("Inset name", (1320.0, 396.0, 500.0, 46.0), "{{camera.storm.name}}", BODY, 26.0, Weight::SemiBold, "$text", Align::Left).with_anim(AnimKind::Fade, 0.3, 0.4));
    doc
}

fn templates_blank_overlay(name: &str) -> GraphicDoc {
    let mut doc = super::blank(true);
    doc.name = name.to_string();
    doc
}

fn travel_times() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Highway Travel Times", "Travel", bg_gradient());
    header(&mut doc, "Drive Times", "Estimated, right now");
    for i in 0..4 {
        let k = i + 1;
        let y = 200.0 + i as f32 * 180.0;
        let d = 0.2 + i as f32 * 0.12;
        doc.layers.push(panel(&format!("Route {k} panel"), 80.0, y, 1760.0, 160.0).with_anim(AnimKind::SlideLeft, d, 0.45));
        doc.layers.push(text(&format!("Route {k} name"), (110.0, y + 14.0, 1000.0, 64.0), &format!("{{{{travel{k}.name}}}}"), DISPLAY, 50.0, Weight::Bold, "$text", Align::Left).with_anim(AnimKind::Fade, d + 0.1, 0.4));
        doc.layers.push(text(&format!("Route {k} note"), (110.0, y + 86.0, 1000.0, 50.0), &format!("{{{{travel{k}.note}}}}"), BODY, 34.0, Weight::SemiBold, "$muted", Align::Left).with_anim(AnimKind::Fade, d + 0.2, 0.4));
        doc.layers.push(text(&format!("Route {k} time"), (1150.0, y + 14.0, 660.0, 70.0), &format!("{{{{travel{k}.time}}}}"), DISPLAY, 60.0, Weight::Black, "$text", Align::Right).with_anim(AnimKind::Pop, d + 0.2, 0.45));
        doc.layers.push(text(&format!("Route {k} delay"), (1150.0, y + 90.0, 660.0, 50.0), &format!("{{{{travel{k}.delay}}}}"), BODY, 36.0, Weight::Bold, "$warning", Align::Right).with_anim(AnimKind::Fade, d + 0.3, 0.4));
    }
    footer(&mut doc, "Estimates from distance, weather and INDOT incidents", "{{station}}");
    doc
}

fn traffic_report() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Traffic Report", "Travel", bg_gradient());
    header(&mut doc, "Traffic Right Now", "{{traffic.count}} incidents");
    for i in 0..6 {
        row_line(&mut doc, i, 200.0 + i as f32 * 125.0, 110.0, &format!("{{{{traffic{}.text}}}}", i + 1), "", 0.2 + i as f32 * 0.1);
    }
    footer(&mut doc, "Source: INDOT TrafficWise", "{{station}}");
    doc
}

fn storm_compare() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Storm Comparison", "Severe", bg_gradient());
    header(&mut doc, "Strongest Storms", "Radar • Lightning • Reports • Warnings");
    for i in 0..5 {
        let k = i + 1;
        let y = 200.0 + i as f32 * 150.0;
        let d = 0.2 + i as f32 * 0.12;
        doc.layers.push(panel(&format!("Storm {k} panel"), 80.0, y, 1760.0, 130.0).with_anim(AnimKind::SlideLeft, d, 0.45));
        doc.layers.push(text(&format!("Storm {k} name"), (110.0, y + 10.0, 700.0, 56.0), &format!("{{{{cmp{k}.name}}}}"), DISPLAY, 42.0, Weight::Bold, "$text", Align::Left).with_anim(AnimKind::Fade, d + 0.1, 0.4));
        doc.layers.push(text(&format!("Storm {k} warning"), (110.0, y + 70.0, 700.0, 46.0), &format!("{{{{cmp{k}.warning}}}}"), BODY, 32.0, Weight::SemiBold, "$warning", Align::Left).with_anim(AnimKind::Fade, d + 0.15, 0.4));
        doc.layers.push(text(&format!("Storm {k} radar"), (830.0, y + 14.0, 480.0, 46.0), &format!("{{{{cmp{k}.radar}}}}"), BODY, 32.0, Weight::SemiBold, "$text", Align::Left).with_anim(AnimKind::Fade, d + 0.2, 0.4));
        doc.layers.push(text(&format!("Storm {k} lightning"), (830.0, y + 70.0, 480.0, 46.0), &format!("{{{{cmp{k}.lightning}}}}"), BODY, 32.0, Weight::SemiBold, "$text", Align::Left).with_anim(AnimKind::Fade, d + 0.25, 0.4));
        doc.layers.push(text(&format!("Storm {k} reports"), (1330.0, y + 14.0, 480.0, 46.0), &format!("{{{{cmp{k}.reports}}}}"), BODY, 32.0, Weight::SemiBold, "$text", Align::Left).with_anim(AnimKind::Fade, d + 0.3, 0.4));
        doc.layers.push(text(&format!("Storm {k} agreement"), (1330.0, y + 70.0, 480.0, 46.0), &format!("{{{{cmp{k}.agreement}}}}"), BODY, 32.0, Weight::Bold, "$primary", Align::Left).with_anim(AnimKind::Fade, d + 0.35, 0.4));
    }
    footer(&mut doc, "DCWeather radar and lightning, IEM reports, NWS warnings", "{{station}}");
    doc
}

/// Animated diagram on the left, three steps revealed one by one on the right.
fn diagram_explainer(title: &str, diagram: &str, steps: &[&str]) -> GraphicDoc {
    let mut doc = GraphicDoc::new(title, "Explainers", bg_gradient());
    header(&mut doc, title, "{{station}}");
    doc.layers.push(Layer::new("Diagram", 80.0, 190.0, 1100.0, 760.0, LayerKind::Diagram { diagram: diagram.to_string() }).with_anim(AnimKind::Fade, 0.2, 0.5));
    for (i, step) in steps.iter().enumerate() {
        let y = 200.0 + i as f32 * 250.0;
        let d = 0.6 + i as f32 * 0.85;
        doc.layers.push(panel(&format!("Step {} panel", i + 1), 1220.0, y, 620.0, 220.0).with_anim(AnimKind::Fade, d, 0.4));
        doc.layers.push(Layer::rect(&format!("Step {} badge", i + 1), 1245.0, y + 25.0, 70.0, 70.0, Fill::solid("$primary"), 35.0).with_anim(AnimKind::Fade, d, 0.4));
        doc.layers.push(text(&format!("Step {} number", i + 1), (1245.0, y + 25.0, 70.0, 70.0), &(i + 1).to_string(), DISPLAY, 44.0, Weight::Black, "#FFFFFF", Align::Center).with_anim(AnimKind::Fade, d, 0.4));
        doc.layers.push(wrapped(text(&format!("Step {}", i + 1), (1335.0, y + 20.0, 480.0, 180.0), step, BODY, 38.0, Weight::SemiBold, "$text", Align::Left)).with_anim(AnimKind::Fade, d, 0.4));
    }
    footer(&mut doc, "{{station}}", "{{tagline}}");
    doc.duration = 1.0 + steps.len().saturating_sub(1) as f32 * 0.85 + 3.0;
    doc
}

/// Name / value rows: a compact data table graphic.
/// `source` is the footer credit: say where these numbers come from.
fn stat_table(title: &str, subtitle: &str, source: &str, rows: &[(&str, &str)]) -> GraphicDoc {
    let mut doc = GraphicDoc::new(title, "Severe", bg_gradient());
    header(&mut doc, title, subtitle);
    // Short tables fill the screen with big numbers instead of 2-4 thin rows at the top
    // of an empty card (CWXB-73).
    if rows.len() <= 4 {
        stat_heroes(&mut doc, rows);
        footer(&mut doc, source, "{{station}}");
        return doc;
    }
    for (i, (name, value)) in rows.iter().enumerate() {
        row_line(&mut doc, i, 200.0 + i as f32 * 110.0, 96.0, name, value, 0.2 + i as f32 * 0.08);
    }
    footer(&mut doc, source, "{{station}}");
    doc
}

/// Hero tiles for a short stat table. Values that are sentences (On this day, a viewer
/// question) get a wide, wrapped panel; numbers get big tiles side by side (2 x 2 for four).
fn stat_heroes(doc: &mut GraphicDoc, rows: &[(&str, &str)]) {
    let long = |v: &str| matches!(v, "{{clim.this_day}}" | "{{question.text}}" | "{{clim.yesterday}}" | "{{clim.month_temp}}" | "{{clim.month_precip}}" | "{{drought.home}}" | "{{drought.outlook_month}}" | "{{drought.outlook_season}}" | "{{clim.ytd_precip}}");
    let (top, bottom) = (190.0, 960.0);
    let wide: Vec<&(&str, &str)> = rows.iter().filter(|r| long(r.1)).collect();
    let tiles: Vec<&(&str, &str)> = rows.iter().filter(|r| !long(r.1)).collect();
    let mut y = top;
    let wide_h = if tiles.is_empty() { (bottom - top - 20.0 * (wide.len() as f32 - 1.0)) / wide.len().max(1) as f32 } else { 250.0 };
    for (i, (name, value)) in wide.iter().enumerate() {
        let d = 0.2 + i as f32 * 0.1;
        let key = format!("Hero {i}");
        doc.layers.push(panel(&format!("{key} panel"), 80.0, y, 1760.0, wide_h).with_anim(AnimKind::SlideUp, d, 0.5));
        doc.layers.push(caps(text(&format!("{key} label"), (120.0, y + 20.0, 1680.0, 54.0), name, BODY, 36.0, Weight::SemiBold, "$muted", Align::Left), 2.0).with_anim(AnimKind::SlideUp, d, 0.5));
        doc.layers.push(
            wrapped(text(&format!("{key} value"), (120.0, y + 84.0, 1680.0, wide_h - 104.0), value, DISPLAY, if tiles.is_empty() && wide.len() == 1 { 72.0 } else { 56.0 }, Weight::Bold, "$text", Align::Left))
                .with_anim(AnimKind::Fade, d + 0.1, 0.5),
        );
        y += wide_h + 20.0;
    }
    if tiles.is_empty() {
        return;
    }
    let cols = if tiles.len() == 4 { 2 } else { tiles.len() };
    let rows_n = tiles.len().div_ceil(cols);
    let gap = 30.0;
    let tw = (1760.0 - gap * (cols as f32 - 1.0)) / cols as f32;
    let th = (bottom - y - gap * (rows_n as f32 - 1.0)) / rows_n as f32;
    for (i, (name, value)) in tiles.iter().enumerate() {
        let (cx, cy) = (80.0 + (i % cols) as f32 * (tw + gap), y + (i / cols) as f32 * (th + gap));
        let d = 0.3 + i as f32 * 0.1;
        let key = format!("Tile {i}");
        doc.layers.push(panel(&format!("{key} panel"), cx, cy, tw, th).with_anim(AnimKind::SlideUp, d, 0.5));
        doc.layers.push(caps(text(&format!("{key} label"), (cx + 24.0, cy + 24.0, tw - 48.0, 60.0), name, BODY, 40.0, Weight::SemiBold, "$muted", Align::Center), 2.0).with_anim(AnimKind::SlideUp, d, 0.5));
        let size = (th * 0.45).min(150.0);
        doc.layers.push(
            shadowed(text(&format!("{key} value"), (cx + 24.0, cy + 90.0, tw - 48.0, th - 120.0), value, DISPLAY, size, Weight::Black, "$text", Align::Center))
                .with_anim(AnimKind::Pop, d + 0.1, 0.5),
        );
    }
}

fn viewer_photo() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Viewer Photo", "Community", bg_gradient());
    doc.layers.push(Layer::new("Photo", 0.0, 0.0, 1920.0, 1080.0, LayerKind::Image { media: "{{viewer1.photo}}".into(), fit: crate::gfx::model::ImageFit::Cover }).with_anim(AnimKind::Fade, 0.0, 0.6));
    doc.layers.push(panel("Credit panel", 80.0, 900.0, 1200.0, 110.0).with_anim(AnimKind::SlideRight, 0.3, 0.45));
    doc.layers.push(text("Credit", (110.0, 910.0, 1140.0, 56.0), "{{viewer1.credit}}", DISPLAY, 44.0, Weight::Bold, "$text", Align::Left).with_anim(AnimKind::Fade, 0.4, 0.4));
    doc.layers.push(text("Caption", (110.0, 962.0, 1140.0, 40.0), "{{viewer1.caption}}", BODY, 30.0, Weight::SemiBold, "$text", Align::Left).with_anim(AnimKind::Fade, 0.5, 0.4));
    doc
}

fn viewer_wall() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Viewer Photo Wall", "Community", bg_gradient());
    header(&mut doc, "Your Weather Photos", "{{station}}");
    for i in 0..4 {
        let (x, y) = (80.0 + (i % 2) as f32 * 890.0, 190.0 + (i / 2) as f32 * 400.0);
        let d = 0.2 + i as f32 * 0.15;
        doc.layers.push(Layer::new(&format!("Photo {}", i + 1), x, y, 870.0, 320.0, LayerKind::Image { media: format!("{{{{viewer{}.photo}}}}", i + 1), fit: crate::gfx::model::ImageFit::Cover }).with_anim(AnimKind::Fade, d, 0.5));
        doc.layers.push(text(&format!("Credit {}", i + 1), (x, y + 328.0, 870.0, 40.0), &format!("{{{{viewer{}.credit}}}}", i + 1), BODY, 28.0, Weight::SemiBold, "$text", Align::Left).with_anim(AnimKind::Fade, d + 0.1, 0.4));
    }
    footer(&mut doc, "Send yours at our website", "{{station}}");
    doc
}

// ---- Severe risk graphics (modern look) -----------------------------------------------------

/// Navy label plate with a coloured edge, like network threat tables.
fn label_plate(doc: &mut GraphicDoc, name: &str, x: f32, y: f32, w: f32, h: f32, edge: &str, delay: f32) {
    doc.layers.push(Layer::plate(&format!("{name} plate"), x, y, w, h, Fill::linear("#0F2448", "#1B3A6B", 90.0), 0.0, true).with_anim(AnimKind::Grow, delay, 0.5));
    doc.layers.push(Layer::rect(&format!("{name} edge"), x, y, 12.0, h, Fill::linear(edge, "rgba(0,0,0,0)", 0.0), 0.0).with_anim(AnimKind::Fade, delay + 0.1, 0.4));
}

fn severe_threats(day: u8, style: &str) -> GraphicDoc {
    let mut doc = GraphicDoc::new("Severe Threats", "Severe", Some(Fill::linear("#050E1C", "#12305A", 155.0)));
    let p = if day == 1 { "spc.home".to_string() } else { format!("spc.day{day}.home") };
    header(&mut doc, "Severe Threats", &format!("{{{{location}}}} • {{{{spc.day{day}.weekday}}}}"));
    // Scale row: RISKS | Marginal Slight Enhanced Moderate High.
    let (lx, lw, mx, mw) = (100.0, 520.0, 660.0, 1160.0);
    doc.layers.push(Layer::plate("Scale plate", lx, 220.0, lw + mw + 40.0, 84.0, Fill::linear("#0B1B36", "#13294F", 90.0), 0.0, false).with_anim(AnimKind::Grow, 0.2, 0.5));
    doc.layers.push(caps(text("Risks", (lx + 30.0, 226.0, lw - 40.0, 72.0), "Risks", DISPLAY, 46.0, Weight::Black, "$text", Align::Left), 3.0).with_anim(AnimKind::Rise, 0.3, 0.5));
    let colors = ["#4CC25A", "#F6DC34", "#F7961E", "#E22828", "#E634E8"];
    for (i, short) in crate::gfx::render::RISK_WORDS.iter().enumerate() {
        let cell = mw / 5.0;
        doc.layers.push(text(&format!("Scale {short}"), (mx + i as f32 * cell, 226.0, cell, 72.0), short, DISPLAY, 44.0, Weight::Black, colors[i], Align::Center).with_anim(AnimKind::Rise, 0.35 + i as f32 * 0.05, 0.5));
    }
    let rows = [("Tornado", "tornado", "#E22828"), ("Hail", "hail", "#3FA9F5"), ("Wind", "wind", "#F7961E"), ("Flooding", "flood", "#4CC25A")];
    for (i, (name, key, edge)) in rows.iter().enumerate() {
        let y = 330.0 + i as f32 * 158.0;
        let d = 0.45 + i as f32 * 0.12;
        label_plate(&mut doc, name, lx, y, lw, 138.0, edge, d);
        doc.layers.push(caps(shadowed(text(&format!("{name} label"), (lx + 40.0, y + 10.0, lw - 60.0, 84.0), name, DISPLAY, 62.0, Weight::Black, "#FFFFFF", Align::Left)), 2.0).with_anim(AnimKind::Rise, d + 0.05, 0.5));
        let detail = if *key == "flood" { format!("{{{{{p}.flood.words}}}} flooding risk") } else { format!("{{{{{p}.{key}}}}} chance {{{{{p}.{key}.sig}}}}") };
        doc.layers.push(text(&format!("{name} detail"), (lx + 42.0, y + 92.0, lw - 60.0, 40.0), &detail, BODY, 26.0, Weight::SemiBold, "#B9C6DA", Align::Left).with_anim(AnimKind::Fade, d + 0.2, 0.4));
        let meter = Layer::meter(&format!("{name} meter"), mx, y, mw, 138.0, &format!("{{{{{p}.{key}.level}}}}"), style).with_anim(AnimKind::Fade, d + 0.15, 0.3);
        doc.layers.push(meter);
    }
    footer(&mut doc, &format!("{{{{{p}.number}}}} • {{{{{p}}}}}"), "Source: NWS Storm Prediction Center and Weather Prediction Center");
    doc.look = "modern".into();
    doc
}

fn risk_levels() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Severe Risk Levels", "Severe", Some(Fill::linear("#050E1C", "#12305A", 155.0)));
    header(&mut doc, "Severe Weather Risks", "Source: NWS Storm Prediction Center");
    let colors = ["#F7B52B", "#F7961E", "#F05A23", "#D92B2B", "#E634E8"];
    let names = ["Marginal", "Slight", "Enhanced", "Moderate", "High"];
    let rows: [(&str, [&str; 5]); 3] = [
        ("Should you change plans?", ["No", "Not yet", "Possibly", "Probably", "Yes"]),
        ("Check the weather?", ["Once or twice", "Multiple times", "Every 2-4 hours", "Every 1-2 hours", "Constantly"]),
        ("Impacts", ["Very isolated, quick", "Spotty and temporary", "Organized, scattered and strong", "Very organized and intense", "Severe, damaging impacts"]),
    ];
    let (lx, lw) = (80.0, 360.0);
    let cw = (1840.0 - (lx + lw)) / 5.0;
    doc.layers.push(panel("Table panel", lx, 200.0, 1760.0, 760.0).with_anim(AnimKind::Fade, 0.15, 0.4));
    for i in 0..5 {
        let x = lx + lw + i as f32 * cw;
        let d = 0.25 + i as f32 * 0.07;
        doc.layers.push(Layer::plate(&format!("Level {} head", i + 1), x + 4.0, 210.0, cw - 8.0, 96.0, Fill::linear(colors[i], colors[i], 0.0), 0.0, true).with_anim(AnimKind::Grow, d, 0.5));
        doc.layers.push(caps(text(&format!("Level {} name", i + 1), (x + 4.0, 214.0, cw - 8.0, 88.0), &format!("{} {}", i + 1, names[i]), DISPLAY, 40.0, Weight::Black, if i == 4 { "#FFFFFF" } else { "#10131A" }, Align::Center), 1.0).with_anim(AnimKind::Rise, d + 0.05, 0.4));
    }
    for (r, (question, answers)) in rows.iter().enumerate() {
        let y = 330.0 + r as f32 * 200.0;
        let d = 0.55 + r as f32 * 0.12;
        doc.layers.push(wrapped(caps(text(&format!("Row {r} question"), (lx + 24.0, y + 20.0, lw - 40.0, 160.0), question, DISPLAY, 34.0, Weight::Bold, "$muted", Align::Left), 1.0)).with_anim(AnimKind::Fade, d, 0.4));
        for (i, answer) in answers.iter().enumerate() {
            let x = lx + lw + i as f32 * cw;
            doc.layers.push(wrapped(text(&format!("Row {r} level {}", i + 1), (x + 14.0, y + 20.0, cw - 28.0, 160.0), answer, DISPLAY, 38.0, Weight::Bold, "#FFFFFF", Align::Center)).with_anim(AnimKind::Rise, d + i as f32 * 0.04, 0.4));
        }
        if r < 2 {
            doc.layers.push(Layer::rect(&format!("Row {r} rule"), lx + 24.0, y + 196.0, 1712.0, 2.0, Fill::solid("rgba(255,255,255,0.14)"), 0.0).with_anim(AnimKind::Grow, d + 0.2, 0.6));
        }
    }
    footer(&mut doc, "Today at {{location}}: {{spc.home.number}}", "{{station}}");
    doc.look = "modern".into();
    doc
}

fn risk_meaning() -> GraphicDoc {
    let mut doc = GraphicDoc::new("What the Risk Levels Mean", "Severe", Some(Fill::linear("#050E1C", "#12305A", 155.0)));
    header(&mut doc, "Severe Weather", "What each risk level means");
    let colors = ["#3FB54A", "#E8DC32", "#C9781E", "#E2332B", "#E86EE6"];
    let text_colors = ["#FFFFFF", "#10131A", "#FFFFFF", "#FFFFFF", "#FFFFFF"];
    let blurbs = [
        // Short enough to stay at full size on the card (CWXB-73: these shrank to ~16 px).
        "Isolated severe storms possible\n\nShort-lived or weak",
        "Scattered severe storms possible\n\nA few could be intense",
        "Numerous severe storms possible\n\nA few intense",
        "Widespread severe storms likely\n\nLong-lived and intense",
        "Widespread severe storms expected\n\nLong-lived and very intense",
    ];
    let names = ["1. Marginal risk", "2. Slight risk", "3. Enhanced risk", "4. Moderate risk", "5. High risk"];
    let w = 330.0;
    for i in 0..5 {
        let x = 80.0 + i as f32 * (w + 27.5);
        let h = 520.0 + i as f32 * 40.0;
        let y = 880.0 - h;
        let d = 0.25 + i as f32 * 0.1;
        doc.layers.push(Layer::plate(&format!("Level {} card", i + 1), x, y, w, h, Fill::linear(colors[i], colors[i], 0.0), 0.0, true).with_anim(AnimKind::Rise, d, 0.6));
        doc.layers.push(wrapped(caps(text(&format!("Level {} text", i + 1), (x + 18.0, y + 30.0, w - 36.0, h - 60.0), blurbs[i], DISPLAY, 34.0, Weight::Black, text_colors[i], Align::Center), 0.5)).with_anim(AnimKind::Fade, d + 0.2, 0.4));
        doc.layers.push(caps(text(&format!("Level {} name", i + 1), (x, 900.0, w, 60.0), names[i], DISPLAY, 32.0, Weight::Bold, "#FFFFFF", Align::Center), 1.0).with_anim(AnimKind::Rise, d + 0.1, 0.4));
    }
    doc.layers.push(Layer::rect("Scale bar", 80.0, 892.0, 1760.0, 76.0, Fill::solid("rgba(0,0,0,0.55)"), 6.0).with_anim(AnimKind::Grow, 0.2, 0.7));
    // The scale bar sits behind the level names.
    let bar = doc.layers.pop().expect("scale bar");
    let at = doc.layers.iter().position(|l| l.name == "Level 1 name").unwrap_or(doc.layers.len());
    doc.layers.insert(at, bar);
    doc.look = "modern".into();
    doc
}

fn severe_day(day: u8) -> GraphicDoc {
    let mut doc = GraphicDoc::new("Storms Possible", "Severe", Some(Fill::linear("#050E1C", "#12305A", 155.0)));
    let p = if day == 1 { "spc.home".to_string() } else { format!("spc.day{day}.home") };
    header(&mut doc, &format!("Strong Storms Possible {{{{spc.day{day}.weekday}}}}"), &format!("{{{{spc.day{day}.date}}}}"));
    // Big level badge.
    doc.layers.push(panel("Level panel", 80.0, 200.0, 700.0, 620.0).with_anim(AnimKind::Zoom, 0.2, 0.5));
    doc.layers.push(caps(text("Level kicker", (80.0, 220.0, 700.0, 60.0), "Risk level", DISPLAY, 40.0, Weight::Bold, "$muted", Align::Center), 3.0).with_anim(AnimKind::Rise, 0.3, 0.4));
    doc.layers.push(shadowed(text("Level number", (80.0, 280.0, 700.0, 330.0), &format!("{{{{{p}.level}}}}"), DISPLAY, 320.0, Weight::Black, "#FFFFFF", Align::Center)).with_anim(AnimKind::Elastic, 0.4, 0.9));
    doc.layers.push(caps(text("Level words", (80.0, 620.0, 700.0, 90.0), &format!("{{{{{p}}}}}"), DISPLAY, 58.0, Weight::Black, "#7CE38B", Align::Center), 1.0).with_anim(AnimKind::Rise, 0.55, 0.4));
    doc.layers.push(text("Level of five", (80.0, 710.0, 700.0, 60.0), &format!("{{{{{p}.number}}}}"), BODY, 34.0, Weight::SemiBold, "$muted", Align::Center).with_anim(AnimKind::Fade, 0.6, 0.4));
    // Potential hazards.
    doc.layers.push(Layer::plate("Hazards head", 840.0, 200.0, 1000.0, 84.0, Fill::linear("#0B1B36", "#13294F", 90.0), 0.0, true).with_anim(AnimKind::Grow, 0.3, 0.5));
    doc.layers.push(caps(text("Hazards title", (870.0, 206.0, 940.0, 72.0), "Potential hazards", DISPLAY, 46.0, Weight::Black, "#FFFFFF", Align::Left), 2.0).with_anim(AnimKind::Rise, 0.35, 0.4));
    let hazards = [("Tornado", "tornado"), ("Damaging wind", "wind"), ("Large hail", "hail"), ("Flooding rain", "flood")];
    for (i, (name, key)) in hazards.iter().enumerate() {
        let y = 300.0 + i as f32 * 108.0;
        let d = 0.45 + i as f32 * 0.1;
        doc.layers.push(panel(&format!("{name} row"), 840.0, y, 1000.0, 96.0).with_anim(AnimKind::Rise, d, 0.45));
        doc.layers.push(text(&format!("{name} name"), (870.0, y + 8.0, 420.0, 80.0), name, DISPLAY, 42.0, Weight::Bold, "#FFFFFF", Align::Left).with_anim(AnimKind::Rise, d, 0.45));
        doc.layers.push(Layer::meter(&format!("{name} meter"), 1300.0, y + 26.0, 510.0, 44.0, &format!("{{{{{p}.{key}.level}}}}"), "blocks").with_anim(AnimKind::Fade, d + 0.1, 0.3));
    }
    doc.layers.push(Layer::plate("Timing plate", 840.0, 740.0, 1000.0, 80.0, Fill::solid("#F2B705"), 0.0, true).with_anim(AnimKind::Grow, 0.8, 0.5));
    doc.layers.push(caps(text("Timing label", (860.0, 744.0, 960.0, 72.0), "{{severe.window}}", DISPLAY, 38.0, Weight::Black, "#10131A", Align::Center), 1.0).with_anim(AnimKind::Rise, 0.85, 0.4));
    doc.layers.push(Layer::meter("Scale", 80.0, 860.0, 1760.0, 92.0, &format!("{{{{{p}.level}}}}"), "pill").with_anim(AnimKind::Rise, 0.9, 0.5));
    footer(&mut doc, "{{station}}", "Source: NWS Storm Prediction Center");
    doc.look = "modern".into();
    doc
}

fn severe_risk_bug() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Severe Risk Bug", "Overlays", None);
    doc.layers.push(Layer::plate("Bug plate", 1380.0, 40.0, 500.0, 150.0, Fill::linear("#0B1B36", "#1B3A6B", 90.0), 0.12, true).with_anim(AnimKind::Grow, 0.0, 0.5));
    doc.layers.push(caps(text("Bug kicker", (1410.0, 48.0, 440.0, 44.0), "Severe risk {{spc.day1.weekday}}", DISPLAY, 30.0, Weight::Bold, "#B9C6DA", Align::Left), 2.0).with_anim(AnimKind::Rise, 0.15, 0.4));
    doc.layers.push(caps(shadowed(text("Bug level", (1410.0, 88.0, 440.0, 52.0), "{{spc.home.number}}", DISPLAY, 42.0, Weight::Black, "#FFFFFF", Align::Left)), 1.0).with_anim(AnimKind::Rise, 0.2, 0.4));
    doc.layers.push(Layer::meter("Bug meter", 1410.0, 146.0, 430.0, 30.0, "{{spc.home.level}}", "blocks").with_anim(AnimKind::Fade, 0.3, 0.3));
    doc.look = "modern".into();
    doc
}

fn camera_wall() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Camera Wall", "Travel", bg_gradient());
    header(&mut doc, "Live Cameras", "{{time}}");
    for i in 0..4 {
        let k = i + 1;
        let (x, y) = (80.0 + (i % 2) as f32 * 890.0, 190.0 + (i / 2) as f32 * 400.0);
        let d = 0.2 + i as f32 * 0.12;
        doc.layers.push(Layer::new(&format!("Camera {k}"), x, y, 870.0, 320.0, LayerKind::Image { media: format!("{{{{camera{k}.image}}}}"), fit: crate::gfx::model::ImageFit::Cover }).with_anim(AnimKind::Fade, d, 0.5));
        doc.layers.push(text(&format!("Camera {k} name"), (x, y + 326.0, 600.0, 40.0), &format!("{{{{camera{k}.name}}}} • {{{{camera{k}.condition}}}}"), BODY, 28.0, Weight::SemiBold, "$text", Align::Left).with_anim(AnimKind::Fade, d + 0.1, 0.4));
        doc.layers.push(text(&format!("Camera {k} credit"), (x + 600.0, y + 330.0, 270.0, 34.0), &format!("{{{{camera{k}.credit}}}}"), BODY, 20.0, Weight::Regular, "$muted", Align::Right).with_anim(AnimKind::Fade, d + 0.1, 0.4));
    }
    footer(&mut doc, "{{station}}", "{{tagline}}");
    doc
}

#[cfg(test)]
mod qa_keys {
    /// Visual QA: render the keys in `CWX_QA_KEYS` (comma list) at half size to `CWX_QA_OUT`.
    #[test]
    #[ignore]
    fn qa_render_keys() {
        let (Ok(keys), Ok(out)) = (std::env::var("CWX_QA_KEYS"), std::env::var("CWX_QA_OUT")) else { return };
        let out = std::path::PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        let settings = crate::config::Settings::default();
        let data = crate::data::WxSnapshot::default();
        for key in keys.split(',').map(str::trim).filter(|k| !k.is_empty()) {
            let Some(doc) = crate::gfx::templates::build_in(key, "modern") else { continue };
            let pm = crate::program::render_graphic_mode(&doc, &settings, &data, 10_000., 960, 540, None, crate::gfx::check::PreviewData::Sample);
            pm.save_png(out.join(format!("{key}.png"))).unwrap();
        }
    }
}
