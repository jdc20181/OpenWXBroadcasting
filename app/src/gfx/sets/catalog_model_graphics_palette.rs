fn categorical_bands(m: Metric) -> (&'static str, Vec<Band>) {
    let (title, pairs) = match m {
        Metric::Temperature => (
            "2 m temperature • °F",
            [
                ("Below 20°F", "#3855B8"),
                ("20 to <32°F", "#387DD0"),
                ("32 to <45°F", "#28B8D2"),
                ("45 to <60°F", "#37BE83"),
                ("60 to <75°F", "#D6DB45"),
                ("75 to <90°F", "#F5A332"),
                ("90 to <105°F", "#EB5538"),
                ("105°F and above", "#B52643"),
            ],
        ),
        Metric::Dewpoint => (
            "2 m dew point • °F",
            [
                ("Below 20°F", "#6475A8"),
                ("20 to <32°F", "#447BC2"),
                ("32 to <45°F", "#32A9CC"),
                ("45 to <55°F", "#35B995"),
                ("55 to <65°F", "#B5CA4C"),
                ("65 to <70°F", "#E0A739"),
                ("70 to <75°F", "#D67535"),
                ("75°F and above", "#B83B45"),
            ],
        ),
        Metric::Humidity => (
            "2 m relative humidity • %",
            [
                ("Below 20%", "#C56D39"),
                ("20 to <30%", "#DDAA3B"),
                ("30 to <40%", "#D1CD49"),
                ("40 to <50%", "#83BE5B"),
                ("50 to <60%", "#43B99B"),
                ("60 to <70%", "#35AFC9"),
                ("70 to <80%", "#467FC7"),
                ("80% and above", "#5D60B7"),
            ],
        ),
        Metric::Wind => (
            "10 m wind speed • mph",
            [
                ("Below 5 mph", "#3855B8"),
                ("5 to <10 mph", "#387DD0"),
                ("10 to <15 mph", "#28B8D2"),
                ("15 to <20 mph", "#37BE83"),
                ("20 to <30 mph", "#D6DB45"),
                ("30 to <40 mph", "#F5A332"),
                ("40 to <55 mph", "#EB5538"),
                ("55 mph and above", "#B52643"),
            ],
        ),
        Metric::Gust => (
            "Surface wind gust • mph",
            [
                ("Below 15 mph", "#3855B8"),
                ("15 to <25 mph", "#387DD0"),
                ("25 to <35 mph", "#28B8D2"),
                ("35 to <45 mph", "#37BE83"),
                ("45 to <55 mph", "#D6DB45"),
                ("55 to <65 mph", "#F5A332"),
                ("65 to <80 mph", "#EB5538"),
                ("80 mph and above", "#B52643"),
            ],
        ),
        Metric::Cloud => (
            "Total cloud cover • %",
            [
                ("Below 10%", "#D9B744"),
                ("10 to <25%", "#B5C65A"),
                ("25 to <40%", "#82BA72"),
                ("40 to <55%", "#5FAE91"),
                ("55 to <70%", "#528FAC"),
                ("70 to <85%", "#5B78B4"),
                ("85 to <95%", "#66708E"),
                ("95% and above", "#8B91A3"),
            ],
        ),
        Metric::Pressure => (
            "Mean sea level pressure • hPa",
            [
                ("Below 980 hPa", "#3855B8"),
                ("980 to <990 hPa", "#387DD0"),
                ("990 to <1000 hPa", "#28B8D2"),
                ("1000 to <1010 hPa", "#37BE83"),
                ("1010 to <1020 hPa", "#D6DB45"),
                ("1020 to <1030 hPa", "#F5A332"),
                ("1030 to <1040 hPa", "#EB5538"),
                ("1040 hPa and above", "#B52643"),
            ],
        ),
        Metric::Cape => (
            "CAPE • J/kg",
            [
                ("Below 100 J/kg", "#3855B8"),
                ("100 to <500 J/kg", "#387DD0"),
                ("500 to <1000 J/kg", "#28B8D2"),
                ("1000 to <1500 J/kg", "#37BE83"),
                ("1500 to <2000 J/kg", "#D6DB45"),
                ("2000 to <3000 J/kg", "#F5A332"),
                ("3000 to <4000 J/kg", "#EB5538"),
                ("4000 J/kg and above", "#B52643"),
            ],
        ),
    };
    (
        title,
        pairs
            .into_iter()
            .map(|(label, color)| Band {
                label: label.into(),
                color: color.into(),
            })
            .collect(),
    )
}
