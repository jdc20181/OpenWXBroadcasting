// Additional supplied-field presentation styles. No weather calculations.
fn supplied_field_style(metric: &str) -> Option<(String, Palette, Vec<Band>)> {
    let alias = match metric {
        "Daily high" | "Daily low" | "Feels like" | "Apparent temperature"
        | "850 mb temperature" | "Ground (soil) temperature" => Some("2 m temperature"),
        "Sky cover" => Some("Total cloud cover"),
        "Precipitation mean" => Some("Total precipitation"),
        "Snowfall" => Some("Snowfall 10:1"),
        "Ice accumulation" => Some("Freezing rain accumulation"),
        _ => None,
    };
    if let Some(alias) = alias {
        let (_, palette, bands) = style(alias);
        let title = match metric {
            "Daily high" => "Daily high • °F • Supplied day",
            "Daily low" => "Daily low • °F • Supplied day",
            "Feels like" => "Feels like • °F • Supplied values",
            "Apparent temperature" => "Apparent temperature • °F • Supplied values",
            "850 mb temperature" => "850 mb temperature • °F",
            "Ground (soil) temperature" => "Soil temperature • °F • Supplied depth",
            "Sky cover" => "Sky cover • %",
            "Precipitation mean" => "Mean precipitation • in • Supplied period",
            "Snowfall" => "Snowfall • in • Supplied method and period",
            "Ice accumulation" => "Ice accumulation • in • Supplied period",
            _ => unreachable!(),
        };
        return Some((title.into(), palette, bands));
    }
    if metric == "Hourly precipitation" {
        let labels = ["Below 0.01 in", "0.01 to <0.05 in", "0.05 to <0.1 in",
            "0.1 to <0.25 in", "0.25 to <0.5 in", "0.5 to <1 in", "1 to <2 in", "2 in and above"];
        return Some(("Hourly precipitation • in • Supplied hour".into(), Palette::Category,
            labels.into_iter().zip(crate::grid::RAIN_TOTAL_STOPS).map(|(label, (_, c))| Band {
                label: label.into(), color: format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]),
            }).collect()));
    }
    let title = match metric {
        "Probability of 0.25 in ice" => "Probability of 0.25 in ice • % • Supplied period",
        "Probability of 1 in rain per hour" => "Probability of 1 in rain per hour • % • Supplied hour",
        "Probability of 40 dBZ" => "Probability of 40 dBZ • % • Supplied period",
        "Probability of 6 in snow" => "Probability of 6 in snow • % • Supplied period",
        "Probability of precipitation" => "Precipitation probability • % • Supplied threshold/period",
        "Probability of thunder" => "Thunder probability • % • Supplied definition/period",
        "Snowfall probability" => "Snowfall probability • % • Supplied threshold/period",
        _ => return None,
    };
    let (_, palette, bands) = style("Probability of 1 in rain");
    Some((title.into(), palette, bands))
}
