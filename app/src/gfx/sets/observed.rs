//! Observed-report graphics only. Consume the existing NOAA/NWS LSR snapshot via IEM;
//! longer windows use the graphics-local IEM reader; never sum or interpolate point totals.
use super::{
    board::{self, Band, Layout, Palette, Panel, ReportPoint, Scene, Scope},
    comparison,
    reference::{frame, label, lowerthird_frame, social_frame, MISSING},
};
use crate::{
    config::Settings,
    data::{StormReport, WxSnapshot},
    gfx::{
        bind::Bindings,
        model::{AnimKind, Fill, GraphicDoc, Layer},
        templates::panel,
        text::Align,
    },
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Snow,
    Rain,
    Wind,
    Hail,
}
impl Kind {
    fn key(self) -> &'static str {
        match self {
            Self::Snow => "snow",
            Self::Rain => "rain",
            Self::Wind => "wind",
            Self::Hail => "hail",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Snow => "Reported Snow Totals",
            Self::Rain => "Rainfall Reports",
            Self::Wind => "Reported Wind Gusts",
            Self::Hail => "Largest Hail Reported",
        }
    }
    fn unit(self) -> &'static str {
        if self == Self::Wind {
            "mph"
        } else {
            "in"
        }
    }
    fn accepts(self, r: &StormReport) -> bool {
        match self {
            Self::Snow => r.kind == "S",
            Self::Rain => r.kind == "R",
            Self::Hail => r.kind == "H",
            Self::Wind => {
                r.kind == "G"
                    || (r.kind == "N" && {
                        let t = r.text.to_ascii_uppercase();
                        t.contains("GST") || t.contains("GUST")
                    })
            }
        }
    }
    fn palette(self) -> Palette {
        match self {
            Self::Snow => Palette::SnowTotal,
            Self::Rain => Palette::RainTotal,
            _ => Palette::Category,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Coverage {
    County,
    Region,
    State,
    Multi,
}
impl Coverage {
    fn key(self) -> &'static str {
        match self {
            Self::County => "county",
            Self::Region => "region",
            Self::State => "state",
            Self::Multi => "multi",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Map,
    Table,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Broadcast,
    Portrait,
    Square,
    LowerThird,
}

pub(super) struct Spec {
    pub key: &'static str,
    kind: Kind,
    coverage: Coverage,
    view: View,
    both: bool,
    format: Format,
    hours: u16,
}
pub(super) const SPECS: &[Spec] = &[
    Spec {
        key: "set_snow_leaderboard__county_lowerthird",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 24,
    },
    Spec {
        key: "set_snow_leaderboard__region_lowerthird",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 24,
    },
    Spec {
        key: "set_snow_leaderboard__state_lowerthird",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 24,
    },
    Spec {
        key: "set_snow_leaderboard__48h_county_lowerthird",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 48,
    },
    Spec {
        key: "set_snow_leaderboard__48h_region_lowerthird",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 48,
    },
    Spec {
        key: "set_snow_leaderboard__48h_state_lowerthird",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 48,
    },
    Spec {
        key: "set_snow_leaderboard__72h_county_lowerthird",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 72,
    },
    Spec {
        key: "set_snow_leaderboard__72h_region_lowerthird",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 72,
    },
    Spec {
        key: "set_snow_leaderboard__72h_state_lowerthird",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 72,
    },
    Spec {
        key: "set_snow_leaderboard__168h_county_lowerthird",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 168,
    },
    Spec {
        key: "set_snow_leaderboard__168h_region_lowerthird",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 168,
    },
    Spec {
        key: "set_snow_leaderboard__168h_state_lowerthird",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 168,
    },
    Spec {
        key: "set_rain_leaderboard__county_lowerthird",
        kind: Kind::Rain,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 24,
    },
    Spec {
        key: "set_rain_leaderboard__region_lowerthird",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 24,
    },
    Spec {
        key: "set_rain_leaderboard__state_lowerthird",
        kind: Kind::Rain,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 24,
    },
    Spec {
        key: "set_rain_leaderboard__48h_county_lowerthird",
        kind: Kind::Rain,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 48,
    },
    Spec {
        key: "set_rain_leaderboard__48h_region_lowerthird",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 48,
    },
    Spec {
        key: "set_rain_leaderboard__48h_state_lowerthird",
        kind: Kind::Rain,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 48,
    },
    Spec {
        key: "set_rain_leaderboard__72h_county_lowerthird",
        kind: Kind::Rain,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 72,
    },
    Spec {
        key: "set_rain_leaderboard__72h_region_lowerthird",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 72,
    },
    Spec {
        key: "set_rain_leaderboard__72h_state_lowerthird",
        kind: Kind::Rain,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 72,
    },
    Spec {
        key: "set_rain_leaderboard__168h_county_lowerthird",
        kind: Kind::Rain,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 168,
    },
    Spec {
        key: "set_rain_leaderboard__168h_region_lowerthird",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 168,
    },
    Spec {
        key: "set_rain_leaderboard__168h_state_lowerthird",
        kind: Kind::Rain,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::LowerThird,
        hours: 168,
    },
    Spec {
        key: "set_snow_observed__48h_county_square",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Map,
        both: false,
        format: Format::Square,
        hours: 48,
    },
    Spec {
        key: "set_snow_observed__48h_region_square",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Map,
        both: false,
        format: Format::Square,
        hours: 48,
    },
    Spec {
        key: "set_snow_observed__48h_state_square",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Map,
        both: false,
        format: Format::Square,
        hours: 48,
    },
    Spec {
        key: "set_snow_observed__48h_multi_square",
        kind: Kind::Snow,
        coverage: Coverage::Multi,
        view: View::Map,
        both: false,
        format: Format::Square,
        hours: 48,
    },
    Spec {
        key: "set_snow_observed__48h_county_portrait",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Map,
        both: false,
        format: Format::Portrait,
        hours: 48,
    },
    Spec {
        key: "set_snow_observed__48h_region_portrait",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Map,
        both: false,
        format: Format::Portrait,
        hours: 48,
    },
    Spec {
        key: "set_snow_observed__48h_state_portrait",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Map,
        both: false,
        format: Format::Portrait,
        hours: 48,
    },
    Spec {
        key: "set_snow_observed__48h_multi_portrait",
        kind: Kind::Snow,
        coverage: Coverage::Multi,
        view: View::Map,
        both: false,
        format: Format::Portrait,
        hours: 48,
    },
    Spec {
        key: "set_snow_leaderboard__48h_county_square",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 48,
    },
    Spec {
        key: "set_snow_leaderboard__48h_region_square",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 48,
    },
    Spec {
        key: "set_snow_leaderboard__48h_state_square",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 48,
    },
    Spec {
        key: "set_rain_leaderboard__48h_county_square",
        kind: Kind::Rain,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 48,
    },
    Spec {
        key: "set_rain_leaderboard__48h_region_square",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 48,
    },
    Spec {
        key: "set_rain_leaderboard__48h_state_square",
        kind: Kind::Rain,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 48,
    },
    Spec {
        key: "set_peak_gusts__48h_county_square",
        kind: Kind::Wind,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 48,
    },
    Spec {
        key: "set_peak_gusts__48h_region_square",
        kind: Kind::Wind,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 48,
    },
    Spec {
        key: "set_peak_gusts__48h_state_square",
        kind: Kind::Wind,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 48,
    },
    Spec {
        key: "set_largest_hail__48h_county_square",
        kind: Kind::Hail,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 48,
    },
    Spec {
        key: "set_largest_hail__48h_region_square",
        kind: Kind::Hail,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 48,
    },
    Spec {
        key: "set_largest_hail__48h_state_square",
        kind: Kind::Hail,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 48,
    },
    Spec {
        key: "set_snow_leaderboard__48h_county_portrait",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 48,
    },
    Spec {
        key: "set_snow_leaderboard__48h_region_portrait",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 48,
    },
    Spec {
        key: "set_snow_leaderboard__48h_state_portrait",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 48,
    },
    Spec {
        key: "set_rain_leaderboard__48h_county_portrait",
        kind: Kind::Rain,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 48,
    },
    Spec {
        key: "set_rain_leaderboard__48h_region_portrait",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 48,
    },
    Spec {
        key: "set_rain_leaderboard__48h_state_portrait",
        kind: Kind::Rain,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 48,
    },
    Spec {
        key: "set_peak_gusts__48h_county_portrait",
        kind: Kind::Wind,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 48,
    },
    Spec {
        key: "set_peak_gusts__48h_region_portrait",
        kind: Kind::Wind,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 48,
    },
    Spec {
        key: "set_peak_gusts__48h_state_portrait",
        kind: Kind::Wind,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 48,
    },
    Spec {
        key: "set_largest_hail__48h_county_portrait",
        kind: Kind::Hail,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 48,
    },
    Spec {
        key: "set_largest_hail__48h_region_portrait",
        kind: Kind::Hail,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 48,
    },
    Spec {
        key: "set_largest_hail__48h_state_portrait",
        kind: Kind::Hail,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 48,
    },
    Spec {
        key: "set_snow_observed__72h_county_square",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Map,
        both: false,
        format: Format::Square,
        hours: 72,
    },
    Spec {
        key: "set_snow_observed__72h_region_square",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Map,
        both: false,
        format: Format::Square,
        hours: 72,
    },
    Spec {
        key: "set_snow_observed__72h_state_square",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Map,
        both: false,
        format: Format::Square,
        hours: 72,
    },
    Spec {
        key: "set_snow_observed__72h_multi_square",
        kind: Kind::Snow,
        coverage: Coverage::Multi,
        view: View::Map,
        both: false,
        format: Format::Square,
        hours: 72,
    },
    Spec {
        key: "set_snow_observed__72h_county_portrait",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Map,
        both: false,
        format: Format::Portrait,
        hours: 72,
    },
    Spec {
        key: "set_snow_observed__72h_region_portrait",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Map,
        both: false,
        format: Format::Portrait,
        hours: 72,
    },
    Spec {
        key: "set_snow_observed__72h_state_portrait",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Map,
        both: false,
        format: Format::Portrait,
        hours: 72,
    },
    Spec {
        key: "set_snow_observed__72h_multi_portrait",
        kind: Kind::Snow,
        coverage: Coverage::Multi,
        view: View::Map,
        both: false,
        format: Format::Portrait,
        hours: 72,
    },
    Spec {
        key: "set_snow_leaderboard__72h_county_square",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 72,
    },
    Spec {
        key: "set_snow_leaderboard__72h_region_square",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 72,
    },
    Spec {
        key: "set_snow_leaderboard__72h_state_square",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 72,
    },
    Spec {
        key: "set_rain_leaderboard__72h_county_square",
        kind: Kind::Rain,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 72,
    },
    Spec {
        key: "set_rain_leaderboard__72h_region_square",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 72,
    },
    Spec {
        key: "set_rain_leaderboard__72h_state_square",
        kind: Kind::Rain,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 72,
    },
    Spec {
        key: "set_peak_gusts__72h_county_square",
        kind: Kind::Wind,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 72,
    },
    Spec {
        key: "set_peak_gusts__72h_region_square",
        kind: Kind::Wind,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 72,
    },
    Spec {
        key: "set_peak_gusts__72h_state_square",
        kind: Kind::Wind,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 72,
    },
    Spec {
        key: "set_largest_hail__72h_county_square",
        kind: Kind::Hail,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 72,
    },
    Spec {
        key: "set_largest_hail__72h_region_square",
        kind: Kind::Hail,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 72,
    },
    Spec {
        key: "set_largest_hail__72h_state_square",
        kind: Kind::Hail,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 72,
    },
    Spec {
        key: "set_snow_leaderboard__72h_county_portrait",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 72,
    },
    Spec {
        key: "set_snow_leaderboard__72h_region_portrait",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 72,
    },
    Spec {
        key: "set_snow_leaderboard__72h_state_portrait",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 72,
    },
    Spec {
        key: "set_rain_leaderboard__72h_county_portrait",
        kind: Kind::Rain,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 72,
    },
    Spec {
        key: "set_rain_leaderboard__72h_region_portrait",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 72,
    },
    Spec {
        key: "set_rain_leaderboard__72h_state_portrait",
        kind: Kind::Rain,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 72,
    },
    Spec {
        key: "set_peak_gusts__72h_county_portrait",
        kind: Kind::Wind,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 72,
    },
    Spec {
        key: "set_peak_gusts__72h_region_portrait",
        kind: Kind::Wind,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 72,
    },
    Spec {
        key: "set_peak_gusts__72h_state_portrait",
        kind: Kind::Wind,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 72,
    },
    Spec {
        key: "set_largest_hail__72h_county_portrait",
        kind: Kind::Hail,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 72,
    },
    Spec {
        key: "set_largest_hail__72h_region_portrait",
        kind: Kind::Hail,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 72,
    },
    Spec {
        key: "set_largest_hail__72h_state_portrait",
        kind: Kind::Hail,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 72,
    },
    Spec {
        key: "set_snow_observed__168h_county_square",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Map,
        both: false,
        format: Format::Square,
        hours: 168,
    },
    Spec {
        key: "set_snow_observed__168h_region_square",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Map,
        both: false,
        format: Format::Square,
        hours: 168,
    },
    Spec {
        key: "set_snow_observed__168h_state_square",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Map,
        both: false,
        format: Format::Square,
        hours: 168,
    },
    Spec {
        key: "set_snow_observed__168h_multi_square",
        kind: Kind::Snow,
        coverage: Coverage::Multi,
        view: View::Map,
        both: false,
        format: Format::Square,
        hours: 168,
    },
    Spec {
        key: "set_snow_observed__168h_county_portrait",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Map,
        both: false,
        format: Format::Portrait,
        hours: 168,
    },
    Spec {
        key: "set_snow_observed__168h_region_portrait",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Map,
        both: false,
        format: Format::Portrait,
        hours: 168,
    },
    Spec {
        key: "set_snow_observed__168h_state_portrait",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Map,
        both: false,
        format: Format::Portrait,
        hours: 168,
    },
    Spec {
        key: "set_snow_observed__168h_multi_portrait",
        kind: Kind::Snow,
        coverage: Coverage::Multi,
        view: View::Map,
        both: false,
        format: Format::Portrait,
        hours: 168,
    },
    Spec {
        key: "set_snow_leaderboard__168h_county_square",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 168,
    },
    Spec {
        key: "set_snow_leaderboard__168h_region_square",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 168,
    },
    Spec {
        key: "set_snow_leaderboard__168h_state_square",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 168,
    },
    Spec {
        key: "set_rain_leaderboard__168h_county_square",
        kind: Kind::Rain,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 168,
    },
    Spec {
        key: "set_rain_leaderboard__168h_region_square",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 168,
    },
    Spec {
        key: "set_rain_leaderboard__168h_state_square",
        kind: Kind::Rain,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 168,
    },
    Spec {
        key: "set_peak_gusts__168h_county_square",
        kind: Kind::Wind,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 168,
    },
    Spec {
        key: "set_peak_gusts__168h_region_square",
        kind: Kind::Wind,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 168,
    },
    Spec {
        key: "set_peak_gusts__168h_state_square",
        kind: Kind::Wind,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 168,
    },
    Spec {
        key: "set_largest_hail__168h_county_square",
        kind: Kind::Hail,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 168,
    },
    Spec {
        key: "set_largest_hail__168h_region_square",
        kind: Kind::Hail,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 168,
    },
    Spec {
        key: "set_largest_hail__168h_state_square",
        kind: Kind::Hail,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 168,
    },
    Spec {
        key: "set_snow_leaderboard__168h_county_portrait",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 168,
    },
    Spec {
        key: "set_snow_leaderboard__168h_region_portrait",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 168,
    },
    Spec {
        key: "set_snow_leaderboard__168h_state_portrait",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 168,
    },
    Spec {
        key: "set_rain_leaderboard__168h_county_portrait",
        kind: Kind::Rain,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 168,
    },
    Spec {
        key: "set_rain_leaderboard__168h_region_portrait",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 168,
    },
    Spec {
        key: "set_rain_leaderboard__168h_state_portrait",
        kind: Kind::Rain,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 168,
    },
    Spec {
        key: "set_peak_gusts__168h_county_portrait",
        kind: Kind::Wind,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 168,
    },
    Spec {
        key: "set_peak_gusts__168h_region_portrait",
        kind: Kind::Wind,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 168,
    },
    Spec {
        key: "set_peak_gusts__168h_state_portrait",
        kind: Kind::Wind,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 168,
    },
    Spec {
        key: "set_largest_hail__168h_county_portrait",
        kind: Kind::Hail,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 168,
    },
    Spec {
        key: "set_largest_hail__168h_region_portrait",
        kind: Kind::Hail,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 168,
    },
    Spec {
        key: "set_largest_hail__168h_state_portrait",
        kind: Kind::Hail,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 168,
    },
    Spec {
        key: "set_snow_observed__72h_county",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Map,
        both: false,
        format: Format::Broadcast,
        hours: 72,
    },
    Spec {
        key: "set_snow_observed__72h_region",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Map,
        both: false,
        format: Format::Broadcast,
        hours: 72,
    },
    Spec {
        key: "set_snow_observed__72h_state",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Map,
        both: false,
        format: Format::Broadcast,
        hours: 72,
    },
    Spec {
        key: "set_snow_observed__72h_multi",
        kind: Kind::Snow,
        coverage: Coverage::Multi,
        view: View::Map,
        both: false,
        format: Format::Broadcast,
        hours: 72,
    },
    Spec {
        key: "set_snow_leaderboard__72h_county",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 72,
    },
    Spec {
        key: "set_snow_leaderboard__72h_region",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 72,
    },
    Spec {
        key: "set_snow_leaderboard__72h_state",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 72,
    },
    Spec {
        key: "set_peak_gusts__72h_county",
        kind: Kind::Wind,
        coverage: Coverage::County,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 72,
    },
    Spec {
        key: "set_peak_gusts__72h_region",
        kind: Kind::Wind,
        coverage: Coverage::Region,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 72,
    },
    Spec {
        key: "set_peak_gusts__72h_state",
        kind: Kind::Wind,
        coverage: Coverage::State,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 72,
    },
    Spec {
        key: "set_largest_hail__72h_county",
        kind: Kind::Hail,
        coverage: Coverage::County,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 72,
    },
    Spec {
        key: "set_largest_hail__72h_region",
        kind: Kind::Hail,
        coverage: Coverage::Region,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 72,
    },
    Spec {
        key: "set_largest_hail__72h_state",
        kind: Kind::Hail,
        coverage: Coverage::State,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 72,
    },
    Spec {
        key: "set_rain_leaderboard__72h_county",
        kind: Kind::Rain,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 72,
    },
    Spec {
        key: "set_rain_leaderboard__72h_region",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 72,
    },
    Spec {
        key: "set_rain_leaderboard__72h_state",
        kind: Kind::Rain,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 72,
    },
    Spec {
        key: "set_snow_observed__168h_county",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Map,
        both: false,
        format: Format::Broadcast,
        hours: 168,
    },
    Spec {
        key: "set_snow_observed__168h_region",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Map,
        both: false,
        format: Format::Broadcast,
        hours: 168,
    },
    Spec {
        key: "set_snow_observed__168h_state",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Map,
        both: false,
        format: Format::Broadcast,
        hours: 168,
    },
    Spec {
        key: "set_snow_observed__168h_multi",
        kind: Kind::Snow,
        coverage: Coverage::Multi,
        view: View::Map,
        both: false,
        format: Format::Broadcast,
        hours: 168,
    },
    Spec {
        key: "set_snow_leaderboard__168h_county",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 168,
    },
    Spec {
        key: "set_snow_leaderboard__168h_region",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 168,
    },
    Spec {
        key: "set_snow_leaderboard__168h_state",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 168,
    },
    Spec {
        key: "set_peak_gusts__168h_county",
        kind: Kind::Wind,
        coverage: Coverage::County,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 168,
    },
    Spec {
        key: "set_peak_gusts__168h_region",
        kind: Kind::Wind,
        coverage: Coverage::Region,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 168,
    },
    Spec {
        key: "set_peak_gusts__168h_state",
        kind: Kind::Wind,
        coverage: Coverage::State,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 168,
    },
    Spec {
        key: "set_largest_hail__168h_county",
        kind: Kind::Hail,
        coverage: Coverage::County,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 168,
    },
    Spec {
        key: "set_largest_hail__168h_region",
        kind: Kind::Hail,
        coverage: Coverage::Region,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 168,
    },
    Spec {
        key: "set_largest_hail__168h_state",
        kind: Kind::Hail,
        coverage: Coverage::State,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 168,
    },
    Spec {
        key: "set_rain_leaderboard__168h_county",
        kind: Kind::Rain,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 168,
    },
    Spec {
        key: "set_rain_leaderboard__168h_region",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 168,
    },
    Spec {
        key: "set_rain_leaderboard__168h_state",
        kind: Kind::Rain,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 168,
    },
    Spec {
        key: "set_snow_observed__48h_county",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Map,
        both: false,
        format: Format::Broadcast,
        hours: 48,
    },
    Spec {
        key: "set_snow_observed__48h_region",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Map,
        both: false,
        format: Format::Broadcast,
        hours: 48,
    },
    Spec {
        key: "set_snow_observed__48h_state",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Map,
        both: false,
        format: Format::Broadcast,
        hours: 48,
    },
    Spec {
        key: "set_snow_observed__48h_multi",
        kind: Kind::Snow,
        coverage: Coverage::Multi,
        view: View::Map,
        both: false,
        format: Format::Broadcast,
        hours: 48,
    },
    Spec {
        key: "set_snow_leaderboard__48h_county",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 48,
    },
    Spec {
        key: "set_snow_leaderboard__48h_region",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 48,
    },
    Spec {
        key: "set_snow_leaderboard__48h_state",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 48,
    },
    Spec {
        key: "set_peak_gusts__48h_county",
        kind: Kind::Wind,
        coverage: Coverage::County,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 48,
    },
    Spec {
        key: "set_peak_gusts__48h_region",
        kind: Kind::Wind,
        coverage: Coverage::Region,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 48,
    },
    Spec {
        key: "set_peak_gusts__48h_state",
        kind: Kind::Wind,
        coverage: Coverage::State,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 48,
    },
    Spec {
        key: "set_largest_hail__48h_county",
        kind: Kind::Hail,
        coverage: Coverage::County,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 48,
    },
    Spec {
        key: "set_largest_hail__48h_region",
        kind: Kind::Hail,
        coverage: Coverage::Region,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 48,
    },
    Spec {
        key: "set_largest_hail__48h_state",
        kind: Kind::Hail,
        coverage: Coverage::State,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 48,
    },
    Spec {
        key: "set_rain_leaderboard__48h_county",
        kind: Kind::Rain,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 48,
    },
    Spec {
        key: "set_rain_leaderboard__48h_region",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 48,
    },
    Spec {
        key: "set_rain_leaderboard__48h_state",
        kind: Kind::Rain,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 48,
    },
    Spec {
        key: "set_snow_observed__county_square",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Map,
        both: false,
        format: Format::Square,
        hours: 24,
    },
    Spec {
        key: "set_snow_observed__region_square",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Map,
        both: false,
        format: Format::Square,
        hours: 24,
    },
    Spec {
        key: "set_snow_observed__state_square",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Map,
        both: false,
        format: Format::Square,
        hours: 24,
    },
    Spec {
        key: "set_snow_observed__multi_square",
        kind: Kind::Snow,
        coverage: Coverage::Multi,
        view: View::Map,
        both: false,
        format: Format::Square,
        hours: 24,
    },
    Spec {
        key: "set_snow_observed__county_portrait",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Map,
        both: false,
        format: Format::Portrait,
        hours: 24,
    },
    Spec {
        key: "set_snow_observed__region_portrait",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Map,
        both: false,
        format: Format::Portrait,
        hours: 24,
    },
    Spec {
        key: "set_snow_observed__state_portrait",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Map,
        both: false,
        format: Format::Portrait,
        hours: 24,
    },
    Spec {
        key: "set_snow_observed__multi_portrait",
        kind: Kind::Snow,
        coverage: Coverage::Multi,
        view: View::Map,
        both: false,
        format: Format::Portrait,
        hours: 24,
    },
    Spec {
        key: "set_snow_leaderboard__county_square",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 24,
    },
    Spec {
        key: "set_snow_leaderboard__region_square",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 24,
    },
    Spec {
        key: "set_snow_leaderboard__state_square",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 24,
    },
    Spec {
        key: "set_rain_leaderboard__county_square",
        kind: Kind::Rain,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 24,
    },
    Spec {
        key: "set_rain_leaderboard__region_square",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 24,
    },
    Spec {
        key: "set_rain_leaderboard__state_square",
        kind: Kind::Rain,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 24,
    },
    Spec {
        key: "set_peak_gusts__county_square",
        kind: Kind::Wind,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 24,
    },
    Spec {
        key: "set_peak_gusts__region_square",
        kind: Kind::Wind,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 24,
    },
    Spec {
        key: "set_peak_gusts__state_square",
        kind: Kind::Wind,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 24,
    },
    Spec {
        key: "set_largest_hail__county_square",
        kind: Kind::Hail,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 24,
    },
    Spec {
        key: "set_largest_hail__region_square",
        kind: Kind::Hail,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 24,
    },
    Spec {
        key: "set_largest_hail__state_square",
        kind: Kind::Hail,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Square,
        hours: 24,
    },
    Spec {
        key: "set_snow_leaderboard__county_portrait",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 24,
    },
    Spec {
        key: "set_snow_leaderboard__region_portrait",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 24,
    },
    Spec {
        key: "set_snow_leaderboard__state_portrait",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 24,
    },
    Spec {
        key: "set_rain_leaderboard__county_portrait",
        kind: Kind::Rain,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 24,
    },
    Spec {
        key: "set_rain_leaderboard__region_portrait",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 24,
    },
    Spec {
        key: "set_rain_leaderboard__state_portrait",
        kind: Kind::Rain,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 24,
    },
    Spec {
        key: "set_peak_gusts__county_portrait",
        kind: Kind::Wind,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 24,
    },
    Spec {
        key: "set_peak_gusts__region_portrait",
        kind: Kind::Wind,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 24,
    },
    Spec {
        key: "set_peak_gusts__state_portrait",
        kind: Kind::Wind,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 24,
    },
    Spec {
        key: "set_largest_hail__county_portrait",
        kind: Kind::Hail,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 24,
    },
    Spec {
        key: "set_largest_hail__region_portrait",
        kind: Kind::Hail,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 24,
    },
    Spec {
        key: "set_largest_hail__state_portrait",
        kind: Kind::Hail,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Portrait,
        hours: 24,
    },
    Spec {
        key: "set_snow_observed__county",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Map,
        both: false,
        format: Format::Broadcast,
        hours: 24,
    },
    Spec {
        key: "set_snow_observed__region",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Map,
        both: false,
        format: Format::Broadcast,
        hours: 24,
    },
    Spec {
        key: "set_snow_observed__state",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Map,
        both: false,
        format: Format::Broadcast,
        hours: 24,
    },
    Spec {
        key: "set_snow_observed__multi",
        kind: Kind::Snow,
        coverage: Coverage::Multi,
        view: View::Map,
        both: false,
        format: Format::Broadcast,
        hours: 24,
    },
    Spec {
        key: "set_snow_leaderboard__county",
        kind: Kind::Snow,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 24,
    },
    Spec {
        key: "set_snow_leaderboard__region",
        kind: Kind::Snow,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 24,
    },
    Spec {
        key: "set_snow_leaderboard__state",
        kind: Kind::Snow,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 24,
    },
    Spec {
        key: "set_peak_gusts__county",
        kind: Kind::Wind,
        coverage: Coverage::County,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 24,
    },
    Spec {
        key: "set_peak_gusts__region",
        kind: Kind::Wind,
        coverage: Coverage::Region,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 24,
    },
    Spec {
        key: "set_peak_gusts__state",
        kind: Kind::Wind,
        coverage: Coverage::State,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 24,
    },
    Spec {
        key: "set_largest_hail__county",
        kind: Kind::Hail,
        coverage: Coverage::County,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 24,
    },
    Spec {
        key: "set_largest_hail__region",
        kind: Kind::Hail,
        coverage: Coverage::Region,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 24,
    },
    Spec {
        key: "set_largest_hail__state",
        kind: Kind::Hail,
        coverage: Coverage::State,
        view: View::Table,
        both: true,
        format: Format::Broadcast,
        hours: 24,
    },
    Spec {
        key: "set_rain_leaderboard__county",
        kind: Kind::Rain,
        coverage: Coverage::County,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 24,
    },
    Spec {
        key: "set_rain_leaderboard__region",
        kind: Kind::Rain,
        coverage: Coverage::Region,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 24,
    },
    Spec {
        key: "set_rain_leaderboard__state",
        kind: Kind::Rain,
        coverage: Coverage::State,
        view: View::Table,
        both: false,
        format: Format::Broadcast,
        hours: 24,
    },
];

fn prefix(kind: Kind, coverage: Coverage, hours: u16) -> String {
    let base = format!("set.obs.{}.{}", kind.key(), coverage.key());
    if hours == 24 {
        base
    } else {
        format!("{base}.{hours}h")
    }
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let s = SPECS.iter().find(|s| s.key == key)?;
    let p = prefix(s.kind, s.coverage, s.hours);
    if s.format == Format::LowerThird {
        return Some(lowerthird_table(s, &p));
    }
    if s.format != Format::Broadcast && s.view == View::Map {
        let portrait = s.format == Format::Portrait;
        let mut d = social_frame(
            s.kind.title(),
            "Winter",
            &format!("{{{{{p}.period}}}}"),
            "{{set.obs.source}}",
            portrait,
        );
        d.layers.push(
            Layer::risk_map(
                "Social report map",
                54.,
                242.,
                972.,
                if portrait { 1400. } else { 650. },
                &format!("set:board:single:{{{{{p}.payload}}}}"),
            )
            .with_anim(AnimKind::Fade, 0.2, 0.6),
        );
        label(
            &mut d,
            "Report caveat",
            if portrait {
                (54., 1656., 972., 68.)
            } else {
                (54., 902., 972., 64.)
            },
            &format!("{{{{{p}.note}}}}"),
            if portrait { 26. } else { 25. },
            "$muted",
            Align::Left,
        );
        return Some(d);
    }
    if s.format != Format::Broadcast {
        return Some(social_table(s, &p));
    }
    let mut d = frame(
        s.kind.title(),
        if matches!(s.kind, Kind::Snow) {
            "Winter"
        } else {
            "Observed"
        },
        &format!("{{{{{p}.period}}}}"),
        "{{set.obs.source}}",
    );
    let views = if s.both {
        vec![
            s.view,
            if s.view == View::Map {
                View::Table
            } else {
                View::Map
            },
        ]
    } else {
        vec![s.view]
    };
    if s.both {
        d.states = views
            .iter()
            .map(|v| {
                if *v == View::Map {
                    "Map".into()
                } else {
                    "Table".into()
                }
            })
            .collect();
    }
    for view in views {
        let start = d.layers.len();
        match view {
            View::Map => d.layers.push(
                Layer::risk_map(
                    "Observed report points",
                    70.,
                    205.,
                    1780.,
                    715.,
                    &format!("set:board:single:{{{{{p}.payload}}}}"),
                )
                .with_anim(AnimKind::Fade, 0.2, 0.6),
            ),
            View::Table => {
                label(
                    &mut d,
                    "Report area",
                    (80., 192., 1760., 54.),
                    &format!("{{{{{p}.area}}}}"),
                    34.,
                    "$text",
                    Align::Left,
                );
                for (name, x, w, align) in [
                    ("RANK", 105., 100., Align::Left),
                    ("LOCATION", 230., 690., Align::Left),
                    (
                        if s.kind == Kind::Hail {
                            "SIZE COMPARISON"
                        } else {
                            "OBSERVED (UTC)"
                        },
                        960.,
                        400.,
                        Align::Left,
                    ),
                    ("REPORTED", 1400., 410., Align::Right),
                ] {
                    label(&mut d, name, (x, 245., w, 44.), name, 24., "$muted", align);
                }
                for i in 0..10 {
                    let y = 297. + i as f32 * 61.;
                    d.layers.push(
                        panel(&format!("Report row {i}"), 80., y, 1760., 54.).with_anim(
                            AnimKind::Rise,
                            0.2 + i as f32 * 0.025,
                            0.4,
                        ),
                    );
                    label(
                        &mut d,
                        &format!("Report rank {i}"),
                        (105., y + 5., 100., 44.),
                        &format!("{{{{{p}.rank{i}}}}}"),
                        28.,
                        "$primary",
                        Align::Left,
                    );
                    label(
                        &mut d,
                        &format!("Report place {i}"),
                        (230., y + 5., 690., 44.),
                        &format!("{{{{{p}.place{i}}}}}"),
                        30.,
                        "$text",
                        Align::Left,
                    );
                    label(
                        &mut d,
                        &format!("Report context {i}"),
                        (960., y + 5., 400., 44.),
                        &format!("{{{{{p}.context{i}}}}}"),
                        25.,
                        "$muted",
                        Align::Left,
                    );
                    label(
                        &mut d,
                        &format!("Report amount {i}"),
                        (1400., y + 5., 410., 44.),
                        &format!("{{{{{p}.amount{i}}}}}"),
                        31.,
                        "$text",
                        Align::Right,
                    );
                }
                d.layers.push(
                    Layer::risk_map(
                        "Empty report table",
                        80.,
                        297.,
                        1760.,
                        603.,
                        &format!("set:reportempty:{{{{{p}.count}}}}"),
                    )
                    .with_anim(AnimKind::Fade, 0.2, 0.4),
                );
            }
        }
        if s.both {
            for layer in &mut d.layers[start..] {
                layer.states = vec![if view == View::Map {
                    "Map".into()
                } else {
                    "Table".into()
                }];
            }
        }
    }
    label(
        &mut d,
        "Report caveat",
        (80., 925., 1760., 42.),
        &format!("{{{{{p}.note}}}}"),
        24.,
        "$muted",
        Align::Left,
    );
    Some(d)
}

fn lowerthird_table(s: &Spec, p: &str) -> GraphicDoc {
    let mut d = lowerthird_frame(
        s.kind.title(),
        if s.kind == Kind::Snow {
            "Winter"
        } else {
            "Observed"
        },
        &format!("{{{{{p}.period}}}}"),
        "{{set.obs.source}}",
    );
    for layer in &mut d.layers {
        if layer.name == "Lower third Title" {
            layer.h = 54.;
        }
    }
    label(
        &mut d,
        "Compact report area",
        (80., 832., 1760., 25.),
        &format!("{{{{{p}.area}}}}"),
        22.,
        "$muted",
        Align::Left,
    );
    d.layers.push(
        Layer::rect(
            "Report lower-third content",
            0.,
            865.,
            1920.,
            131.,
            Fill::solid("$panel"),
            0.,
        )
        .with_anim(AnimKind::Fade, 0.1, 0.3),
    );
    for i in 0..3 {
        let x = 70. + i as f32 * 600.;
        label(
            &mut d,
            &format!("Report compact place {i}"),
            (x, 875., 420., 40.),
            &format!("{{{{{p}.rank{i}}}}}  {{{{{p}.place{i}}}}}"),
            28.,
            "$text",
            Align::Left,
        );
        label(
            &mut d,
            &format!("Report compact amount {i}"),
            (x + 425., 875., 145., 40.),
            &format!("{{{{{p}.amount{i}}}}}"),
            31.,
            "$text",
            Align::Right,
        );
        label(
            &mut d,
            &format!("Report compact time {i}"),
            (x, 917., 570., 28.),
            &format!("{{{{{p}.context{i}}}}}"),
            21.,
            "$muted",
            Align::Left,
        );
    }
    d.layers.push(
        Layer::risk_map(
            "Empty compact reports",
            60.,
            870.,
            1800.,
            78.,
            &format!("set:reportempty:{{{{{p}.count}}}}"),
        )
        .with_anim(AnimKind::Fade, 0.1, 0.3),
    );
    label(&mut d, "Compact report caveat", (70., 953., 1780., 34.),
        "Leading 3 available • Times UTC • Preliminary reports • Periods vary; totals are not summed",
        23., "$muted", Align::Left);
    d
}

/// The report data and ranking are shared with the broadcast tables; only layout varies.
fn social_table(s: &Spec, p: &str) -> GraphicDoc {
    let portrait = s.format == Format::Portrait;
    let mut d = social_frame(
        s.kind.title(),
        if s.kind == Kind::Snow {
            "Winter"
        } else {
            "Observed"
        },
        &format!("{{{{{p}.period}}}}"),
        "{{set.obs.source}}",
        portrait,
    );
    label(
        &mut d,
        "Report area",
        if portrait {
            (54., 242., 972., 66.)
        } else {
            (54., 190., 972., 54.)
        },
        &format!("{{{{{p}.area}}}}"),
        if portrait { 38. } else { 34. },
        "$text",
        Align::Left,
    );
    let columns = if s.kind == Kind::Hail {
        "LOCATION / SIZE COMPARISON"
    } else {
        "LOCATION / OBSERVED (UTC)"
    };
    let columns = if portrait {
        columns.into()
    } else {
        format!("LEADING 6 AVAILABLE • {columns}")
    };
    label(
        &mut d,
        "Report columns",
        if portrait {
            (150., 314., 848., 38.)
        } else {
            (78., 250., 924., 32.)
        },
        &columns,
        if portrait { 25. } else { 22. },
        "$muted",
        Align::Left,
    );
    let (count, start, step, height) = if portrait {
        (10, 365., 120., 110.)
    } else {
        (6, 300., 90., 84.)
    };
    for i in 0..count {
        let y = start + i as f32 * step;
        d.layers.push(
            panel(&format!("Social report row {i}"), 54., y, 972., height).with_anim(
                AnimKind::Rise,
                0.2 + i as f32 * 0.025,
                0.4,
            ),
        );
        for (field, x, dy, w, h, size, color, align) in [
            (
                "rank",
                78.,
                if portrait { 15. } else { 10. },
                62.,
                48.,
                if portrait { 32. } else { 28. },
                "$primary",
                Align::Left,
            ),
            (
                "place",
                155.,
                10.,
                550.,
                52.,
                if portrait { 37. } else { 32. },
                "$text",
                Align::Left,
            ),
            (
                "amount",
                722.,
                10.,
                280.,
                52.,
                if portrait { 42. } else { 36. },
                "$text",
                Align::Right,
            ),
            (
                "context",
                155.,
                if portrait { 65. } else { 49. },
                847.,
                if portrait { 34. } else { 29. },
                if portrait { 27. } else { 23. },
                "$muted",
                Align::Left,
            ),
        ] {
            label(
                &mut d,
                &format!("Report {field} {i}"),
                (x, y + dy, w, h),
                &format!("{{{{{p}.{field}{i}}}}}"),
                size,
                color,
                align,
            );
        }
    }
    d.layers.push(
        Layer::risk_map(
            "Empty social table",
            54.,
            start,
            972.,
            if portrait { 1190. } else { 534. },
            &format!("set:reportempty:{{{{{p}.count}}}}"),
        )
        .with_anim(AnimKind::Fade, 0.2, 0.4),
    );
    label(
        &mut d,
        "Report caveat",
        if portrait {
            (54., 1598., 972., 112.)
        } else {
            (54., 864., 972., 100.)
        },
        &format!("{{{{{p}.note}}}}"),
        if portrait { 29. } else { 25. },
        "$muted",
        Align::Left,
    );
    d
}

#[derive(Clone, Debug)]
pub(super) struct Amount {
    pub value: Option<f32>,
    pub display: String,
}
pub(super) fn amount(raw: &str, kind: Kind) -> Option<Amount> {
    let v = raw.trim().to_ascii_uppercase();
    if matches!(kind, Kind::Snow | Kind::Rain) && matches!(v.as_str(), "TRACE" | "T") {
        return Some(Amount {
            value: None,
            display: "Trace".into(),
        });
    }
    let number = if kind == Kind::Wind {
        v.strip_suffix("MPH").unwrap_or(&v).trim()
    } else {
        v.strip_suffix("INCHES")
            .or_else(|| v.strip_suffix("INCH"))
            .or_else(|| v.strip_suffix("IN"))
            .or_else(|| v.strip_suffix('"'))
            .unwrap_or(&v)
            .trim()
    };
    if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let n = number.parse::<f32>().ok()?;
    let max = match kind {
        Kind::Snow => 240.,
        Kind::Rain => 100.,
        Kind::Wind => 350.,
        Kind::Hail => 12.,
    };
    if !n.is_finite() || n <= 0. || n > max {
        return None;
    }
    let display = match kind {
        Kind::Snow => format!("{n:.1} in"),
        Kind::Rain | Kind::Hail => format!("{n:.2} in"),
        Kind::Wind => format!("{n:.0} mph"),
    };
    Some(Amount {
        value: Some(n),
        display,
    })
}

/// NWS glossary comparison sizes; numeric report diameter remains the primary value.
fn hail_comparison(n: f32) -> String {
    let sizes = [
        (0.25, "pea"),
        (0.75, "penny"),
        (1.0, "quarter"),
        (1.75, "golf ball"),
        (2.75, "baseball"),
    ];
    if let Some((_, name)) = sizes.iter().find(|(size, _)| (n - size).abs() < 0.02) {
        return format!("About {name} size");
    }
    let below = sizes.iter().rev().find(|(size, _)| n > *size);
    let above = sizes.iter().find(|(size, _)| n < *size);
    match (below, above) {
        (Some((_, a)), Some((_, b))) => format!("Between {a} / {b}"),
        (Some((_, a)), None) => format!("Larger than {a}"),
        (None, Some((_, a))) => format!("Smaller than {a}"),
        _ => String::new(),
    }
}

#[derive(Clone, Debug)]
struct Record {
    id: String,
    lon: f64,
    lat: f64,
    place: String,
    time: i64,
    amount: Amount,
}

pub(super) fn report_scope(settings: &Settings, coverage: Coverage) -> Scope {
    let mut s = comparison::scope(
        settings,
        if coverage == Coverage::County {
            Layout::Zones
        } else {
            Layout::Compare
        },
    );
    if matches!(coverage, Coverage::Region | Coverage::Multi) {
        let (x, y) = if coverage == Coverage::Multi {
            (5., 3.)
        } else {
            (2., 1.2)
        };
        s.county = None;
        s.bbox = Some([
            settings.home.lon - x,
            settings.home.lat - y,
            settings.home.lon + x,
            settings.home.lat + y,
        ]);
    }
    s
}

fn records(data: &WxSnapshot, kind: Kind, scope: &Scope, now: i64) -> Vec<Record> {
    records_between(&data.reports, kind, scope, now - 86400, now)
}
fn records_between(
    reports: &[StormReport],
    kind: Kind,
    scope: &Scope,
    start: i64,
    now: i64,
) -> Vec<Record> {
    let all = crate::map::boundaries::counties_named();
    let counties = if scope.bbox.is_none() {
        all.as_ref()
            .map(|v| board::selected(v, scope))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let mut observations: BTreeMap<(String, i64), Option<Record>> = BTreeMap::new();
    for r in reports {
        if !kind.accepts(r)
            || r.time < start
            || r.time > now
            || !r.lat.is_finite()
            || !r.lon.is_finite()
        {
            continue;
        }
        let contains = if let Some([w, s, e, n]) = scope.bbox {
            (w..=e).contains(&r.lon) && (s..=n).contains(&r.lat)
        } else if scope.county.is_none() && r.county.rsplit_once(',').is_some() {
            let state = r.county.rsplit_once(',').unwrap().1.trim();
            scope
                .states
                .iter()
                .any(|s| crate::winter::state_code(s) == state)
        } else {
            counties.iter().any(|c| board::inside(c, r.lon, r.lat))
        };
        if !contains {
            continue;
        }
        let Some(amount) = amount(&r.magnitude, kind) else {
            continue;
        };
        let site = format!("{:.4},{:.4}", r.lon, r.lat);
        let mut record = Record {
            id: format!("{}:{}:{}", kind.key(), site, r.time),
            lon: r.lon,
            lat: r.lat,
            place: if r.city.trim().is_empty() {
                if r.county.trim().is_empty() {
                    "Location not provided".into()
                } else {
                    r.county.clone()
                }
            } else {
                r.city.clone()
            },
            time: r.time,
            amount,
        };
        if scope.bbox.is_some() && !r.city.trim().is_empty() {
            if let Some((_, state)) = r.county.rsplit_once(',') {
                let state = state.trim();
                if state.len() == 2
                    && state.bytes().all(|c| c.is_ascii_uppercase())
                    && !record.place.ends_with(&format!(", {state}"))
                {
                    record.place.push_str(&format!(", {state}"));
                }
            }
        }
        match observations.get_mut(&(site.clone(), r.time)) {
            Some(old) => {
                if old
                    .as_ref()
                    .is_some_and(|v| v.amount.display != record.amount.display)
                {
                    *old = None;
                }
            }
            None => {
                observations.insert((site, r.time), Some(record));
            }
        }
    }
    let mut sites: BTreeMap<String, (i64, Option<Record>)> = BTreeMap::new();
    for ((site, time), record) in observations {
        match sites.get_mut(&site) {
            Some((old_time, old)) => {
                let replace = if matches!(kind, Kind::Wind | Kind::Hail) {
                    record.as_ref().is_some_and(|r| {
                        old.as_ref().is_none_or(|o| {
                            r.amount.value > o.amount.value
                                || (r.amount.value == o.amount.value && time > *old_time)
                        })
                    })
                } else {
                    time > *old_time
                };
                if replace {
                    *old_time = time;
                    *old = record;
                }
            }
            None => {
                sites.insert(site, (time, record));
            }
        }
    }
    let mut out: Vec<_> = sites.into_values().filter_map(|(_, r)| r).collect();
    out.sort_by(|a, b| {
        b.amount
            .value
            .unwrap_or(0.)
            .total_cmp(&a.amount.value.unwrap_or(0.))
            .then(b.time.cmp(&a.time))
            .then(a.place.cmp(&b.place))
    });
    out
}

fn category_bands(kind: Kind) -> Vec<Band> {
    let colors = match kind {
        Kind::Wind => vec![
            "#55D5F9".to_string(),
            "#FFE34A".into(),
            "#FF982F".into(),
            "#F34545".into(),
        ],
        _ => [2, 3, 5, 7]
            .iter()
            .map(|i| {
                let c = crate::grid::RAIN_TOTAL_STOPS[*i].1;
                format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
            })
            .collect(),
    };
    let labels = if kind == Kind::Wind {
        ["Below 40 mph", "40–57 mph", "58–74 mph", "75+ mph"]
    } else {
        ["Below 1 in", "1–1.74 in", "1.75–2.74 in", "2.75+ in"]
    };
    labels
        .iter()
        .zip(colors)
        .map(|(label, color)| Band {
            label: label.to_string(),
            color,
        })
        .collect()
}
fn category(kind: Kind, value: Option<f32>) -> usize {
    let v = value.unwrap_or(0.);
    let cuts = if kind == Kind::Wind {
        [40., 58., 75.]
    } else {
        [1., 1.75, 2.75]
    };
    cuts.iter().filter(|c| v >= **c).count()
}

fn put(
    b: &mut Bindings,
    key: &str,
    kind: Kind,
    scope: Scope,
    area: &str,
    rows: &[Record],
    now: chrono::DateTime<chrono::Utc>,
    hours: u16,
) {
    b.set(
        &format!("{key}.period"),
        &format!("Past {hours}h • As of {}", now.format("%H:%M UTC")),
    );
    b.set(&format!("{key}.area"), area);
    b.set(&format!("{key}.count"), &rows.len().to_string());
    b.set(&format!("{key}.note"),"Preliminary NWS reports may include estimates. Observation periods vary; totals are not summed.");
    let scene = Scene {
        reviewed: !rows.is_empty(),
        scope,
        palette: kind.palette(),
        legend_title: format!(
            "Reported {} • {} • point observations",
            kind.key(),
            kind.unit()
        ),
        legend: if matches!(kind, Kind::Wind | Kind::Hail) {
            category_bands(kind)
        } else {
            Vec::new()
        },
        panels: vec![Panel {
            summary: Some(
                rows.iter()
                    .take(7)
                    .map(|r| (r.place.clone(), r.amount.display.clone()))
                    .collect(),
            ),
            summary_title: "Leading available reports".into(),
            title: area.into(),
            valid: format!(
                "Report times in past {hours}h • {}",
                now.format("%b %-d, %Y %H:%M UTC")
            ),
            detail: if rows.is_empty() {
                String::new()
            } else {
                format!("{} reporting locations", rows.len())
            },
            points: if rows.is_empty() {
                None
            } else {
                Some(
                    rows.iter()
                        .map(|r| ReportPoint {
                            id: r.id.clone(),
                            lon: r.lon,
                            lat: r.lat,
                            label: r.amount.display.clone(),
                            category: if matches!(kind, Kind::Wind | Kind::Hail) {
                                category(kind, r.amount.value)
                            } else {
                                0
                            },
                            value: r.amount.value,
                        })
                        .collect(),
                )
            },
            ..Default::default()
        }],
    };
    b.set(
        &format!("{key}.payload"),
        &serde_json::to_string(&scene).unwrap_or_default(),
    );
    for i in 0..10 {
        let row = rows.get(i);
        b.set(
            &format!("{key}.rank{i}"),
            &row.map(|_| (i + 1).to_string()).unwrap_or_default(),
        );
        b.set(
            &format!("{key}.place{i}"),
            &row.map(|r| r.place.clone()).unwrap_or_default(),
        );
        b.set(
            &format!("{key}.amount{i}"),
            &row.map(|r| r.amount.display.clone()).unwrap_or_default(),
        );
        b.set(
            &format!("{key}.context{i}"),
            &row.map(|r| {
                if kind == Kind::Hail {
                    r.amount.value.map(hail_comparison).unwrap_or_default()
                } else {
                    chrono::DateTime::from_timestamp(r.time, 0)
                        .map(|t| t.format("%b %-d %H:%M").to_string())
                        .unwrap_or_else(|| MISSING.into())
                }
            })
            .unwrap_or_default(),
        );
    }
}

pub(super) fn bind(b: &mut Bindings, data: &WxSnapshot, settings: &Settings) {
    let now = chrono::Utc::now();
    b.set("set.obs.source", "Source: NWS Local Storm Reports via IEM");
    let longer = super::report_feed::current(settings.home, now.timestamp(), 48);
    for kind in [Kind::Snow, Kind::Rain, Kind::Wind, Kind::Hail] {
        for coverage in [
            Coverage::County,
            Coverage::Region,
            Coverage::State,
            Coverage::Multi,
        ] {
            let scope = report_scope(settings, coverage);
            let rows = records(data, kind, &scope, now.timestamp());
            let area = match coverage {
                Coverage::County => format!("{} • County reports", settings.home_label),
                Coverage::Region => format!("{} • Regional reports", settings.home_label),
                Coverage::State => scope
                    .states
                    .first()
                    .map(|s| crate::winter::state_name(s))
                    .unwrap_or("State not available yet")
                    .to_string(),
                Coverage::Multi => format!("{} • Multi-state region", settings.home_label),
            };
            put(
                b,
                &prefix(kind, coverage, 24),
                kind,
                scope.clone(),
                &area,
                &rows,
                now,
                24,
            );
            for hours in [48, 72, 168] {
                let end = longer
                    .as_ref()
                    .and_then(|s| chrono::DateTime::from_timestamp(s.end, 0));
                let rows = longer
                    .as_ref()
                    .map(|s| {
                        records_between(
                            &s.reports,
                            kind,
                            &scope,
                            s.end - i64::from(hours) * 3600,
                            s.end,
                        )
                    })
                    .unwrap_or_default();
                let key = prefix(kind, coverage, hours);
                put(
                    b,
                    &key,
                    kind,
                    scope.clone(),
                    &area,
                    &rows,
                    end.unwrap_or(now),
                    hours,
                );
                if end.is_none() {
                    b.set(
                        &format!("{key}.period"),
                        &format!("Past {hours}h • Awaiting report data"),
                    );
                    let mut scene = board::empty(Layout::Single);
                    scene.scope = report_scope(settings, coverage);
                    scene.panels[0].title = area.clone();
                    scene.panels[0].valid = format!("Past {hours}h • Awaiting report data");
                    b.set(
                        &format!("{key}.payload"),
                        &serde_json::to_string(&scene).unwrap_or_default(),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(s) = SPECS
        .iter()
        .find(|s| Some(s.key) == doc.template.as_deref())
    else {
        return;
    };
    // SAMPLE layout fixture: the sample station sits where the synthetic reports are.
    let mut settings = Settings::default();
    settings.home = crate::config::GeoView { lat: 38.47, lon: -86.89, zoom: 8.0 };
    settings.setup_complete = true;
    let now = chrono::Utc::now();
    let mut data = WxSnapshot::default();
    for i in 0..12 {
        data.reports.push(StormReport {
            lon: -86.94 + (i % 4) as f64 * 0.032,
            lat: 38.43 + (i / 4) as f64 * 0.035,
            kind: match s.kind {
                Kind::Snow => "S",
                Kind::Rain => "R",
                Kind::Wind => "G",
                Kind::Hail => "H",
            }
            .into(),
            text: s.kind.title().into(),
            magnitude: match s.kind {
                Kind::Wind => (45. + i as f32 * 3.).to_string(),
                Kind::Hail => (0.75 + i as f32 * 0.25).to_string(),
                _ => format!("{:.2}", 0.25 + i as f32 * 0.5),
            },
            city: if i == 11 {
                "Jeffersonville".into()
            } else {
                format!("Sample location {}", i + 1)
            },
            county: "Dubois, IN".into(),
            source: "SAMPLE".into(),
            remark: "Synthetic layout fixture".into(),
            time: now.timestamp()
                - if s.hours > 24 {
                    (i64::from(s.hours) - 12) * 3600
                } else {
                    0
                }
                - i as i64 * 300,
        });
    }
    let scope = report_scope(&settings, s.coverage);
    let rows = records_between(
        &data.reports,
        s.kind,
        &scope,
        now.timestamp() - i64::from(s.hours) * 3600,
        now.timestamp(),
    );
    let mut b = Bindings::default();
    let p = prefix(s.kind, s.coverage, s.hours);
    put(
        &mut b,
        &p,
        s.kind,
        scope,
        "SAMPLE • reviewed point-report layout",
        &rows,
        now,
        s.hours,
    );
    if s.format == Format::LowerThird {
        b.set(
            &format!("{p}.area"),
            &format!("SAMPLE • Jeffersonville, IN • {} reports", s.coverage.key()),
        );
    }
    for (k, v) in b.values() {
        doc.data_overrides.push((k.clone(), v.clone()));
    }
    doc.data_overrides.push((
        "set.obs.source".into(),
        "SAMPLE ONLY • Not for broadcast".into(),
    ));
    doc.data_overrides.push((
        format!("{p}.note"),
        "SAMPLE • Synthetic reports • No totals are interpolated or summed.".into(),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_periods_filter_exact_bounds_and_do_not_reuse_shorter_snapshots() {
        let now = chrono::Utc::now().timestamp();
        let scope = Scope {
            bbox: Some([-88., 37., -85., 40.]),
            ..Default::default()
        };
        let report = |age: i64| StormReport {
            lon: -86.9,
            lat: 38.4,
            kind: "S".into(),
            magnitude: "2 IN".into(),
            city: "Jeffersonville".into(),
            time: now - age,
            ..Default::default()
        };
        let mut data = WxSnapshot::default();
        data.reports = vec![report(30 * 3600)];
        assert!(records(&data, Kind::Snow, &scope, now).is_empty());
        assert_eq!(
            records_between(&data.reports, Kind::Snow, &scope, now - 48 * 3600, now).len(),
            1
        );
        assert_eq!(
            records_between(
                &[report(48 * 3600)],
                Kind::Snow,
                &scope,
                now - 48 * 3600,
                now
            )
            .len(),
            1
        );
        assert!(records_between(
            &[report(48 * 3600 + 1), report(-1)],
            Kind::Snow,
            &scope,
            now - 48 * 3600,
            now
        )
        .is_empty());
        let mut b = Bindings::default();
        bind(&mut b, &data, &Settings::default());
        let key = prefix(Kind::Snow, Coverage::County, 48);
        assert_eq!(
            b.values().get(&format!("{key}.count")).map(String::as_str),
            Some("0")
        );
        assert!(b
            .values()
            .get(&format!("{key}.period"))
            .unwrap()
            .contains("Awaiting"));
    }
    #[test]
    fn unknown_home_stays_unavailable_without_panicking() {
        let mut settings = Settings::default();
        settings.home.lat = 0.;
        settings.home.lon = 0.;
        let mut b = Bindings::default();
        bind(&mut b, &WxSnapshot::default(), &settings);
    }
    #[test]
    fn magnitudes_do_not_turn_missing_or_incompatible_units_into_zero() {
        for v in ["", "null", "0", "-9", "NaN", "99 KT", "1.0 CM", "999999"] {
            assert!(amount(v, Kind::Wind).is_none(), "{v}");
        }
        assert_eq!(amount("TRACE", Kind::Snow).unwrap().display, "Trace");
        assert!(amount("TRACE", Kind::Snow).unwrap().value.is_none());
        assert_eq!(amount("1.75 IN", Kind::Hail).unwrap().display, "1.75 in");
        assert_eq!(hail_comparison(1.75), "About golf ball size");
        assert_eq!(amount("65 MPH", Kind::Wind).unwrap().display, "65 mph");
    }
    #[test]
    fn reports_use_latest_site_values_without_summing_or_accepting_conflicts() {
        let now = chrono::Utc::now().timestamp();
        let scope = Scope {
            bbox: Some([-88., 37., -85., 40.]),
            ..Default::default()
        };
        let report = |value: &str, time: i64| StormReport {
            lat: 38.4,
            lon: -86.9,
            kind: "S".into(),
            text: "SNOW".into(),
            magnitude: value.into(),
            city: "Jasper".into(),
            county: "Dubois, IN".into(),
            source: "NWS".into(),
            remark: String::new(),
            time,
        };
        let mut data = WxSnapshot::default();
        data.reports = vec![
            report("2", now - 600),
            report("3", now - 60),
            report("3", now - 60),
            report("20", now - 90000),
            report("8", now + 60),
        ];
        let rows = records(&data, Kind::Snow, &scope, now);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].amount.value, Some(3.));
        assert_eq!(rows[0].place, "Jasper, IN");
        data.reports.push(report("4", now - 60));
        assert!(records(&data, Kind::Snow, &scope, now).is_empty());
        data.reports.push(report("TRACE", now - 1));
        let rows = records(&data, Kind::Snow, &scope, now);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].amount.display, "Trace");
        assert!(rows[0].amount.value.is_none());
        data.reports = vec![report("70", now - 600), report("50", now - 60)];
        for r in &mut data.reports {
            r.kind = "G".into();
            r.text = "TSTM WND GST".into();
        }
        let rows = records(&data, Kind::Wind, &scope, now);
        assert_eq!(
            rows[0].amount.value,
            Some(70.),
            "Keep the highest gust, rather than the latest lower gust"
        );
        for r in &mut data.reports {
            r.kind = "H".into();
        }
        data.reports[0].magnitude = "2.75".into();
        data.reports[1].magnitude = "1.00".into();
        let rows = records(&data, Kind::Hail, &scope, now);
        assert_eq!(rows[0].amount.value, Some(2.75));
    }
}
