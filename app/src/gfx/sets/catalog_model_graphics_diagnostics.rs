// Supplied diagnostic categories only; no differences, anomalies or omega are derived here.
fn diagnostic_style(metric: &str) -> Option<(String, Palette, Vec<Band>)> {
    let (title, labels, colors): (&str, [&str; 8], [&str; 8]) = match metric {
        "Temperature change 24 h" => ("24-hour temperature change • °F",
            ["Below -20°F", "-20 to <-10°F", "-10 to <-5°F", "-5 to <0°F", "0 to <5°F", "5 to <10°F", "10 to <20°F", "20°F and above"],
            ["#3855B8", "#387DD0", "#28B8D2", "#72C9CC", "#F3DA72", "#F5A332", "#EB5538", "#B52643"]),
        "Temperature anomaly" => ("Temperature anomaly • °F • Supplied baseline",
            ["Below -15°F", "-15 to <-10°F", "-10 to <-5°F", "-5 to <0°F", "0 to <5°F", "5 to <10°F", "10 to <15°F", "15°F and above"],
            ["#3855B8", "#387DD0", "#28B8D2", "#72C9CC", "#F3DA72", "#F5A332", "#EB5538", "#B52643"]),
        "500 mb height anomaly" => ("500 mb height anomaly • m • Supplied baseline",
            ["Below -200 m", "-200 to <-100 m", "-100 to <-50 m", "-50 to <0 m", "0 to <50 m", "50 to <100 m", "100 to <200 m", "200 m and above"],
            ["#3855B8", "#387DD0", "#28B8D2", "#72C9CC", "#F3DA72", "#F5A332", "#EB5538", "#B52643"]),
        "700 mb vertical motion" => ("700 mb omega • Pa/s • Negative = rising",
            ["Below -2 Pa/s", "-2 to <-1 Pa/s", "-1 to <-0.5 Pa/s", "-0.5 to <0 Pa/s", "0 to <0.5 Pa/s", "0.5 to <1 Pa/s", "1 to <2 Pa/s", "2 Pa/s and above"],
            ["#3855B8", "#387DD0", "#28B8D2", "#72C9CC", "#F3DA72", "#F5A332", "#EB5538", "#B52643"]),
        "Visibility" | "Visibility (fog)" => ("Visibility • mi",
            ["Below 0.25 mi", "0.25 to <0.5 mi", "0.5 to <1 mi", "1 to <2 mi", "2 to <3 mi", "3 to <5 mi", "5 to <10 mi", "10 mi and above"],
            ["#9C3EC5", "#AD5DBA", "#8F78C7", "#5B92CF", "#35ABBB", "#47BA9A", "#87C863", "#CFDA74"]),
        _ => return None,
    };
    Some((title.into(), Palette::Category, labels.into_iter().zip(colors).map(|(label, color)| Band {
        label: label.into(), color: color.into(),
    }).collect()))
}
