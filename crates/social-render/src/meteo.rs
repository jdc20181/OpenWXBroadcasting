//! Meteocons colour weather artwork (Bas Milius, https://github.com/basmilius/weather-icons,
//! MIT license in assets/meteocons/LICENSE.txt). The same set the Broadcaster's Modern
//! graphics use, so the Broadcast forecast card matches the studio's 7-day graphic.

macro_rules! svg {
    ($name:literal) => {
        ($name, include_bytes!(concat!("../assets/meteocons/", $name, ".svg")) as &[u8])
    };
}

const ICONS: &[(&str, &[u8])] = &[
    svg!("clear-day"),
    svg!("clear-night"),
    svg!("partly-cloudy-day"),
    svg!("partly-cloudy-night"),
    svg!("overcast-day"),
    svg!("overcast-night"),
    svg!("cloudy"),
    svg!("overcast"),
    svg!("partly-cloudy-day-rain"),
    svg!("partly-cloudy-night-rain"),
    svg!("rain"),
    svg!("drizzle"),
    svg!("thunderstorms-day-rain"),
    svg!("thunderstorms-night-rain"),
    svg!("thunderstorms-rain"),
    svg!("snow"),
    svg!("partly-cloudy-day-snow"),
    svg!("partly-cloudy-night-snow"),
    svg!("sleet"),
    svg!("fog-day"),
    svg!("fog-night"),
    svg!("fog"),
    svg!("wind"),
    svg!("thermometer-warmer"),
    svg!("thermometer-colder"),
    svg!("hail"),
    svg!("tornado"),
    svg!("smoke"),
    svg!("haze"),
];

/// Meteocons artwork for a day: from the Remix icon name the callers already pick, refined by
/// the forecast words (chance / patchy / drizzle / sleet ...). None for icons with no weather
/// meaning (holiday flag, gift, sparkles): those keep their Remix icon.
pub fn for_day(icon: &str, description: &str) -> Option<&'static [u8]> {
    let d = description.to_lowercase();
    let has = |w: &str| d.contains(w);
    let partial = has("chance") || has("slight") || has("isolated") || has("scattered") || has("patchy") || has("areas") || has("then");
    let night = icon.starts_with("moon");
    let dn = |day: &'static str, nt: &'static str| if night { nt } else { day };
    let name = if has("tornado") || icon == "tornado-fill" {
        "tornado"
    } else if has("hail") || icon == "hail-fill" {
        "hail"
    } else if has("smoke") {
        "smoke"
    } else if has("sleet") || has("freezing") || has("wintry mix") || (has("rain") && has("snow")) {
        "sleet"
    } else {
        match icon {
            "sun-fill" => "clear-day",
            "moon-clear-fill" => "clear-night",
            "sun-cloudy-fill" => "partly-cloudy-day",
            "moon-cloudy-fill" => "partly-cloudy-night",
            "cloudy-2-fill" if has("mostly cloudy") => dn("overcast-day", "overcast-night"),
            "cloudy-fill" | "cloudy-2-fill" => "overcast",
            "drizzle-fill" => "drizzle",
            "rainy-fill" | "showers-fill" | "heavy-showers-fill" if has("drizzle") => "drizzle",
            "rainy-fill" | "showers-fill" if partial || has("shower") => dn("partly-cloudy-day-rain", "partly-cloudy-night-rain"),
            "rainy-fill" | "showers-fill" | "heavy-showers-fill" => "rain",
            "thunderstorms-fill" if partial => dn("thunderstorms-day-rain", "thunderstorms-night-rain"),
            "thunderstorms-fill" => "thunderstorms-rain",
            "snowy-fill" | "snowflake-fill" if partial || has("flurr") => dn("partly-cloudy-day-snow", "partly-cloudy-night-snow"),
            "snowy-fill" | "snowflake-fill" => "snow",
            "foggy-fill" | "mist-fill" if partial => dn("fog-day", "fog-night"),
            "foggy-fill" | "mist-fill" => "fog",
            "sun-foggy-fill" => "fog-day",
            "moon-foggy-fill" => "fog-night",
            "haze-2-fill" => "haze",
            "windy-fill" | "cloud-windy-fill" | "typhoon-fill" => "wind",
            "temp-hot-fill" => "thermometer-warmer",
            "temp-cold-fill" => "thermometer-colder",
            _ => return None,
        }
    };
    ICONS.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_icons_map_to_artwork() {
        assert!(for_day("sun-fill", "Sunny").is_some());
        assert!(for_day("foggy-fill", "Patchy Fog then Sunny").is_some());
        assert!(for_day("rainy-fill", "Chance Rain Showers").is_some());
        assert!(for_day("gift-fill", "Christmas").is_none());
        for (name, data) in ICONS {
            assert!(resvg::usvg::Tree::from_data(data, &resvg::usvg::Options::default()).is_ok(), "{name}");
        }
    }
}
