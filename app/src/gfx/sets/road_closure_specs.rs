// Parameters for one shared manual closure-list builder.
const SPECS: &[Spec] = &[
    Spec {
        key: "set_road_closures__county_overlay",
        county: true,
        format: 4,
    },
    Spec {
        key: "set_road_closures__region_overlay",
        county: false,
        format: 4,
    },
    Spec {
        key: "set_road_closures__county",
        county: true,
        format: 0,
    },
    Spec {
        key: "set_road_closures__region",
        county: false,
        format: 0,
    },
    Spec {
        key: "set_road_closures__county_portrait",
        county: true,
        format: 1,
    },
    Spec {
        key: "set_road_closures__region_portrait",
        county: false,
        format: 1,
    },
    Spec {
        key: "set_road_closures__county_square",
        county: true,
        format: 2,
    },
    Spec {
        key: "set_road_closures__region_square",
        county: false,
        format: 2,
    },
    Spec {
        key: "set_road_closures__county_lowerthird",
        county: true,
        format: 3,
    },
    Spec {
        key: "set_road_closures__region_lowerthird",
        county: false,
        format: 3,
    },
];
