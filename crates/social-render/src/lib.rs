//! social-render: draws social graphics (alert cards and daily forecast posts).
//!
//! Used two ways, from the same code:
//! - the `social-render` program on the website server (JSON job on stdin, PNG on stdout), called by
//!   the automatic posts;
//! - as a library inside OpenWX Broadcasting (`dcwx_render::render(&job_json)`), so the studio
//!   can make the same graphics, live or historical, without the website.
//!
//! A job is `{ "kind": "alert" | "forecast", "scale": 2, ...fields }`. The caller prepares all
//! data (times, text, tiles, logos); this crate only draws. Nothing here is specific to one
//! county or brand: names, logos and colours all come in the job.

mod alert;
mod broadcast;
mod forecast;
mod gfx;
mod map;
mod meteo;

use serde::Deserialize;

/// Alert card designs. The first is the default used for production posts.
pub const ALERT_DESIGNS: [&str; 3] = ["modern", "classic", "bold"];
/// Forecast post designs. The first is the default used for production posts. "broadcast"
/// matches the Broadcaster's 7-day graphic (and is the look of every regional forecast).
pub const FORECAST_DESIGNS: [&str; 3] = ["modern", "classic", "broadcast"];
/// Forecast post sizes: landscape 1280x720, square 1080x1080, story 1080x1920.
pub const FORECAST_RATIOS: [&str; 3] = ["landscape", "square", "story"];
/// Alert card size in design pixels (the PNG is this times `scale`).
pub const ALERT_SIZE: (f32, f32) = (alert::W, alert::H);

/// Size of the map panel of an alert design, in design pixels. Callers use it (times `scale`)
/// to fetch basemap tiles at full detail for exactly the visible area.
pub fn alert_map_size(design: &str) -> (f32, f32) {
    let (_, _, w, h) = alert::map_rect(design);
    (w, h)
}

/// Size of a forecast post in design pixels.
pub fn forecast_size(ratio: &str) -> (f32, f32) {
    forecast::size(ratio)
}

/// Station branding, passed in every job.
#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct Brand {
    pub name: String,
    /// Small line under the name, e.g. "Dubois County, IN • WFO LMK".
    pub sub: String,
    pub site: String,
    /// Logo as base64 / data: URI (SVG, PNG, JPEG or WebP).
    pub logo: Option<String>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct Sponsor {
    pub label: String,
    pub name: String,
    pub logo: Option<String>,
}

#[derive(Deserialize)]
struct Job {
    kind: String,
    #[serde(default = "two")]
    scale: f32,
}

fn two() -> f32 {
    2.0
}

/// Renders one job (JSON) to PNG bytes.
pub fn render(json: &[u8]) -> Result<Vec<u8>, String> {
    let job: Job = serde_json::from_slice(json).map_err(|e| format!("bad job: {e}"))?;
    let scale = job.scale.clamp(0.5, 4.0);
    let fonts = gfx::Fonts::load();
    match job.kind.as_str() {
        "alert" => {
            let a: alert::Alert = serde_json::from_slice(json).map_err(|e| format!("bad alert: {e}"))?;
            let mut cv = gfx::Canvas::new(alert::W, alert::H, scale, &fonts);
            alert::draw(&mut cv, &a);
            Ok(cv.png())
        }
        "forecast" => {
            let f: forecast::Forecast = serde_json::from_slice(json).map_err(|e| format!("bad forecast: {e}"))?;
            let (w, h) = forecast::size(&f.ratio);
            let mut cv = gfx::Canvas::new(w, h, scale, &fonts);
            forecast::draw(&mut cv, &f);
            Ok(cv.png())
        }
        other => Err(format!("unknown kind \"{other}\" (expected alert or forecast)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_an_alert_and_a_forecast() {
        let alert = br##"{"kind":"alert","scale":1,"event":"Special Weather Statement","headline":"Strong thunderstorm will impact portions of Dubois County","banner_color":"#FFE4B5","kind_color":"#FFE4B5","until":"Nov 14, 7:45 PM EST","rows":[{"kind":"hazard","label":"Hazard","value":"Wind gusts up to 50 mph and pea size hail."}],"area":["Dubois","Perry","Spencer","Crawford","Orange","Martin","Pike","Warrick"],"brand":{"name":"Dubois County Weather","site":"dcweather.xyz"}}"##;
        let png = render(alert).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        for design in ALERT_DESIGNS {
            let job = String::from_utf8_lossy(alert).replacen("{\"kind\"", &format!("{{\"design\":\"{design}\",\"kind\""), 1);
            assert!(render(job.as_bytes()).unwrap().starts_with(b"\x89PNG"), "{design}");
        }
        for design in FORECAST_DESIGNS {
            for ratio in FORECAST_RATIOS {
                let job = format!(r#"{{"kind":"forecast","design":"{design}","scale":1,"ratio":"{ratio}","headline":"TODAY OUTLOOK","description":"Sunny.","days":[{{"label":"Today","date":"Nov 14","icon":"sun-fill","description":"Sunny","high":55,"low":34}},{{"label":"Sat","date":"Nov 15","icon":"rainy-fill","description":"Rain likely","high":48,"low":40}}]}}"#);
                assert!(render(job.as_bytes()).unwrap().starts_with(b"\x89PNG"), "{design} {ratio}");
            }
        }
        assert!(render(br#"{"kind":"nope"}"#).is_err());
    }

    /// Writes the Broadcast design (every size) and a regional table to $DCWX_OUT for review.
    #[test]
    fn broadcast_and_regional() {
        let days = r#"[{"label":"Today","date":"Nov 15","icon":"sun-fill","description":"Sunny","high":55,"low":28,"rain":0},{"label":"Mon","date":"Nov 16","icon":"foggy-fill","description":"Patchy Fog then Sunny","high":57,"low":29,"rain":0},{"label":"Tue","date":"Nov 17","icon":"foggy-fill","description":"Patchy Fog then Sunny","high":60,"low":31,"rain":0},{"label":"Wed","date":"Nov 18","icon":"sun-fill","description":"Sunny","high":63,"low":37,"rain":0},{"label":"Thu","date":"Nov 19","icon":"sun-cloudy-fill","description":"Mostly Sunny","high":65,"low":42,"rain":0},{"label":"Fri","date":"Nov 20","icon":"rainy-fill","description":"Chance Rain Showers","high":68,"low":43,"rain":40},{"label":"Sat","date":"Nov 21","icon":"thunderstorms-fill","description":"Showers And Thunderstorms","high":71,"low":46,"rain":70}]"#;
        let out = std::env::var("DCWX_OUT").ok();
        for ratio in FORECAST_RATIOS {
            let job = format!(r#"{{"kind":"forecast","design":"broadcast","scale":1,"ratio":"{ratio}","location":"Dubois County, IN","headline":"Sunday outlook","description":"Sunny, with a high near 55. Light north wind.","sponsors":[{{"label":"Brought to you by","name":"Cathedral"}}],"brand":{{"name":"Dubois County Weather"}},"days":{days}}}"#);
            let png = render(job.as_bytes()).unwrap();
            assert!(png.starts_with(b"\x89PNG"), "{ratio}");
            if let Some(dir) = &out {
                std::fs::write(format!("{dir}/broadcast-{ratio}.png"), &png).unwrap();
            }
        }
        let cities = r#"[{"name":"Jasper","home":true,"days":[{"icon":"sun-fill","description":"Sunny","high":54},{"icon":"sun-fill","description":"Sunny","high":57,"low":30},{"icon":"sun-fill","description":"Sunny","high":60,"low":29},{"icon":"sun-cloudy-fill","description":"Partly cloudy","high":63,"low":31}]},{"name":"Evansville","days":[{"icon":"sun-fill","description":"Sunny","high":54},{"icon":"sun-fill","description":"Sunny","high":56,"low":30},{"icon":"rainy-fill","description":"Rain","high":60,"low":32},{"icon":"sun-cloudy-fill","description":"Partly cloudy","high":63,"low":34}]}]"#;
        for ratio in FORECAST_RATIOS {
            let job = format!(r#"{{"kind":"forecast","design":"broadcast","scale":1,"ratio":"{ratio}","location":"Southern Indiana","headline":"Regional outlook","brand":{{"name":"Dubois County Weather"}},"days":[{{"label":"Today","date":"Nov 15"}},{{"label":"Mon","date":"Nov 16"}},{{"label":"Tue","date":"Nov 17"}},{{"label":"Wed","date":"Nov 18"}}],"cities":{cities}}}"#);
            let png = render(job.as_bytes()).unwrap();
            assert!(png.starts_with(b"\x89PNG"), "regional {ratio}");
            if let Some(dir) = &out {
                std::fs::write(format!("{dir}/regional-{ratio}.png"), &png).unwrap();
            }
        }
    }
}
