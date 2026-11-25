// Supplied model fields with producer-defined bins. No severe-weather index or
// aviation/air-quality classification is calculated in this presentation module.
fn additional_model_style(metric: &str) -> Option<(String, Palette, Vec<Band>)> {
    let title = match metric {
        "Updraft helicity" => "Updraft helicity • Supplied layer and units",
        "Supercell composite" => "Supercell composite • Supplied index definition",
        "Significant tornado parameter" => "Significant tornado parameter • Supplied definition",
        "Lightning threat" => "Lightning threat • Supplied definition and units",
        "Smoke (near surface)" => "Near-surface smoke • Supplied concentration units",
        "Cloud ceiling" => "Cloud ceiling • Supplied AGL units",
        "Max hail size" => "Maximum hail size • Supplied diameter units",
        "Echo tops" => "Echo tops • Supplied threshold and height units",
        "Snow level" => "Snow level • Supplied elevation datum and units",
        "Neighbourhood max reflectivity" => "Neighbourhood max reflectivity • dBZ • Supplied radius",
        _ => return None,
    };
    Some((title.into(), Palette::Category, Vec::new()))
}
#[cfg(test)]
fn sample_defined_legend(metric: &str) -> (String, Vec<Band>) {
    // Explicit synthetic definitions, not a claim about an operational model's
    // native products. Live inputs must still supply their reviewed legend.
    let (title, labels) = match metric {
        "Updraft helicity" => ("0–3 km updraft helicity • m²/s²", ["Below 25", "25–50", "50–75", "75–100", "100–150", "150–200", "200–300", "300+"]),
        "Supercell composite" => ("Supercell composite • Unitless", ["Below 1", "1–2", "2–4", "4–6", "6–8", "8–12", "12–16", "16+"]),
        "Significant tornado parameter" => ("Effective-layer STP • Unitless", ["Below 0.5", "0.5–1", "1–2", "2–3", "3–4", "4–6", "6–8", "8+"]),
        "Lightning threat" => ("Lightning within 25 mi • Next hour • %", ["Below 10%", "10–20%", "20–30%", "30–40%", "40–50%", "50–60%", "60–80%", "80–100%"]),
        "Smoke (near surface)" => ("Surface smoke concentration • µg/m³", ["Below 1", "1–5", "5–10", "10–20", "20–40", "40–80", "80–160", "160+"]),
        "Cloud ceiling" => ("Cloud ceiling • ft AGL", ["Below 200", "200–500", "500–1,000", "1,000–2,000", "2,000–3,000", "3,000–5,000", "5,000–10,000", "10,000+"]),
        "Max hail size" => ("Maximum hail diameter • in", ["Below 0.25", "0.25–0.5", "0.5–0.75", "0.75–1", "1–1.5", "1.5–2", "2–3", "3+"]),
        "Echo tops" => ("18 dBZ echo tops • Thousands of ft MSL", ["Below 10", "10–15", "15–20", "20–25", "25–30", "30–40", "40–50", "50+"]),
        "Snow level" => ("Snow level • ft MSL", ["Below 500", "500–1,000", "1,000–2,000", "2,000–3,000", "3,000–4,000", "4,000–6,000", "6,000–8,000", "8,000+"]),
        "Neighbourhood max reflectivity" => ("Maximum reflectivity • 40 km radius • dBZ", ["Below 10", "10–20", "20–30", "30–40", "40–50", "50–60", "60–70", "70+"]),
        _ => unreachable!("Not an additional model field"),
    };
    let colors = ["#3855B8", "#387DD0", "#28B8D2", "#37BE83", "#D6DB45", "#F5A332", "#EB5538", "#B52643"];
    (title.into(), labels.into_iter().zip(colors).map(|(label, color)| Band {
        label: label.into(), color: color.into(),
    }).collect())
}
#[cfg(test)]
fn sample_additional_legend() -> Vec<Band> {
    // Ensemble fixtures reuse these colors and replace the labels with the
    // field-specific height/vorticity bands before serialization.
    sample_defined_legend("Supercell composite").1
}
