//! Remix Icon (Apache-2.0, https://remixicon.com) navigation icons, embedded in the binary.
//! The SVGs live in assets/icons/remix with the license; credit is in About.

use std::borrow::Cow;

use gpui_kit::*;

const REMIX: &[(&str, &[u8])] = &[
    ("add-line", include_bytes!("../../assets/icons/remix/add-line.svg")),
    ("alarm-warning-line", include_bytes!("../../assets/icons/remix/alarm-warning-line.svg")),
    ("arrow-down-s-line", include_bytes!("../../assets/icons/remix/arrow-down-s-line.svg")),
    ("arrow-go-back-line", include_bytes!("../../assets/icons/remix/arrow-go-back-line.svg")),
    ("base-station-line", include_bytes!("../../assets/icons/remix/base-station-line.svg")),
    ("broadcast-line", include_bytes!("../../assets/icons/remix/broadcast-line.svg")),
    ("brush-line", include_bytes!("../../assets/icons/remix/brush-line.svg")),
    ("close-line", include_bytes!("../../assets/icons/remix/close-line.svg")),
    ("cloud-line", include_bytes!("../../assets/icons/remix/cloud-line.svg")),
    ("contrast-2-line", include_bytes!("../../assets/icons/remix/contrast-2-line.svg")),
    ("copyright-line", include_bytes!("../../assets/icons/remix/copyright-line.svg")),
    ("cpu-line", include_bytes!("../../assets/icons/remix/cpu-line.svg")),
    ("dashboard-3-line", include_bytes!("../../assets/icons/remix/dashboard-3-line.svg")),
    ("download-2-line", include_bytes!("../../assets/icons/remix/download-2-line.svg")),
    ("drag-move-2-line", include_bytes!("../../assets/icons/remix/drag-move-2-line.svg")),
    ("drop-line", include_bytes!("../../assets/icons/remix/drop-line.svg")),
    ("earth-line", include_bytes!("../../assets/icons/remix/earth-line.svg")),
    ("eraser-line", include_bytes!("../../assets/icons/remix/eraser-line.svg")),
    ("film-line", include_bytes!("../../assets/icons/remix/film-line.svg")),
    ("fire-line", include_bytes!("../../assets/icons/remix/fire-line.svg")),
    ("flashlight-line", include_bytes!("../../assets/icons/remix/flashlight-line.svg")),
    ("flood-line", include_bytes!("../../assets/icons/remix/flood-line.svg")),
    ("focus-3-line", include_bytes!("../../assets/icons/remix/focus-3-line.svg")),
    ("gallery-line", include_bytes!("../../assets/icons/remix/gallery-line.svg")),
    ("hand-heart-line", include_bytes!("../../assets/icons/remix/hand-heart-line.svg")),
    ("history-line", include_bytes!("../../assets/icons/remix/history-line.svg")),
    ("home-4-line", include_bytes!("../../assets/icons/remix/home-4-line.svg")),
    ("layout-column-line", include_bytes!("../../assets/icons/remix/layout-column-line.svg")),
    ("layout-left-line", include_bytes!("../../assets/icons/remix/layout-left-line.svg")),
    ("lightbulb-line", include_bytes!("../../assets/icons/remix/lightbulb-line.svg")),
    ("list-check", include_bytes!("../../assets/icons/remix/list-check.svg")),
    ("map-2-line", include_bytes!("../../assets/icons/remix/map-2-line.svg")),
    ("map-pin-line", include_bytes!("../../assets/icons/remix/map-pin-line.svg")),
    ("palette-line", include_bytes!("../../assets/icons/remix/palette-line.svg")),
    ("pause-line", include_bytes!("../../assets/icons/remix/pause-line.svg")),
    ("pencil-line", include_bytes!("../../assets/icons/remix/pencil-line.svg")),
    ("play-line", include_bytes!("../../assets/icons/remix/play-line.svg")),
    ("skip-back-line", include_bytes!("../../assets/icons/remix/skip-back-line.svg")),
    ("skip-forward-line", include_bytes!("../../assets/icons/remix/skip-forward-line.svg")),
    ("repeat-line", include_bytes!("../../assets/icons/remix/repeat-line.svg")),
    ("arrow-left-s-line", include_bytes!("../../assets/icons/remix/arrow-left-s-line.svg")),
    ("arrow-right-s-line", include_bytes!("../../assets/icons/remix/arrow-right-s-line.svg")),
    ("stop-line", include_bytes!("../../assets/icons/remix/stop-line.svg")),
    ("restart-line", include_bytes!("../../assets/icons/remix/restart-line.svg")),
    ("arrow-right-line", include_bytes!("../../assets/icons/remix/arrow-right-line.svg")),
    ("zoom-in-line", include_bytes!("../../assets/icons/remix/zoom-in-line.svg")),
    ("zoom-out-line", include_bytes!("../../assets/icons/remix/zoom-out-line.svg")),
    ("radar-line", include_bytes!("../../assets/icons/remix/radar-line.svg")),
    ("refresh-line", include_bytes!("../../assets/icons/remix/refresh-line.svg")),
    ("search-line", include_bytes!("../../assets/icons/remix/search-line.svg")),
    ("send-plane-line", include_bytes!("../../assets/icons/remix/send-plane-line.svg")),
    ("settings-3-line", include_bytes!("../../assets/icons/remix/settings-3-line.svg")),
    ("shape-line", include_bytes!("../../assets/icons/remix/shape-line.svg")),
    ("slideshow-line", include_bytes!("../../assets/icons/remix/slideshow-line.svg")),
    ("snowy-line", include_bytes!("../../assets/icons/remix/snowy-line.svg")),
    ("stack-line", include_bytes!("../../assets/icons/remix/stack-line.svg")),
    ("sun-cloudy-line", include_bytes!("../../assets/icons/remix/sun-cloudy-line.svg")),
    ("temp-hot-line", include_bytes!("../../assets/icons/remix/temp-hot-line.svg")),
    ("text", include_bytes!("../../assets/icons/remix/text.svg")),
    ("thunderstorms-line", include_bytes!("../../assets/icons/remix/thunderstorms-line.svg")),
    ("time-line", include_bytes!("../../assets/icons/remix/time-line.svg")),
    ("windy-line", include_bytes!("../../assets/icons/remix/windy-line.svg")),
    ("tornado-line", include_bytes!("../../assets/icons/remix/tornado-line.svg")),
    ("hail-line", include_bytes!("../../assets/icons/remix/hail-line.svg")),
    ("error-warning-line", include_bytes!("../../assets/icons/remix/error-warning-line.svg")),
    ("image-line", include_bytes!("../../assets/icons/remix/image-line.svg")),
    ("layout-grid-line", include_bytes!("../../assets/icons/remix/layout-grid-line.svg")),
    ("apps-line", include_bytes!("../../assets/icons/remix/apps-line.svg")),
    ("arrow-left-line", include_bytes!("../../assets/icons/remix/arrow-left-line.svg")),
    ("magic-line", include_bytes!("../../assets/icons/remix/magic-line.svg")),
    ("star-line", include_bytes!("../../assets/icons/remix/star-line.svg")),
    ("movie-2-line", include_bytes!("../../assets/icons/remix/movie-2-line.svg")),
    ("ruler-line", include_bytes!("../../assets/icons/remix/ruler-line.svg")),
    ("calendar-line", include_bytes!("../../assets/icons/remix/calendar-line.svg")),
    ("mark-pen-line", include_bytes!("../../assets/icons/remix/mark-pen-line.svg")),
    ("arrow-right-up-line", include_bytes!("../../assets/icons/remix/arrow-right-up-line.svg")),
    ("checkbox-blank-circle-line", include_bytes!("../../assets/icons/remix/checkbox-blank-circle-line.svg")),
    ("map-pin-2-line", include_bytes!("../../assets/icons/remix/map-pin-2-line.svg")),
    ("route-line", include_bytes!("../../assets/icons/remix/route-line.svg")),
    ("compass-3-line", include_bytes!("../../assets/icons/remix/compass-3-line.svg")),
    ("arrow-up-line", include_bytes!("../../assets/icons/remix/arrow-up-line.svg")),
    ("arrow-down-line", include_bytes!("../../assets/icons/remix/arrow-down-line.svg")),
    ("rainy-line", include_bytes!("../../assets/icons/remix/rainy-line.svg")),
    ("layout-4-line", include_bytes!("../../assets/icons/remix/layout-4-line.svg")),
    ("layout-row-line", include_bytes!("../../assets/icons/remix/layout-row-line.svg")),
    ("eye-line", include_bytes!("../../assets/icons/remix/eye-line.svg")),
    ("file-add-line", include_bytes!("../../assets/icons/remix/file-add-line.svg")),
    ("shape-2-line", include_bytes!("../../assets/icons/remix/shape-2-line.svg")),
    ("road-map-line", include_bytes!("../../assets/icons/remix/road-map-line.svg")),
    ("vidicon-line", include_bytes!("../../assets/icons/remix/vidicon-line.svg")),
    ("arrow-left-right-line", include_bytes!("../../assets/icons/remix/arrow-left-right-line.svg")),
];

/// True when a Remix icon with this name is bundled.
pub fn has(name: &str) -> bool {
    REMIX.iter().any(|(n, _)| *n == name)
}

/// The app's asset source: Remix icons under `remix/`, everything else from gpui-kit.
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if let Some(name) = path.strip_prefix("remix/").and_then(|p| p.strip_suffix(".svg")) {
            return Ok(REMIX.iter().find(|(n, _)| *n == name).map(|(_, bytes)| Cow::Borrowed(*bytes)));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        let mut out = gpui_kit::assets::Assets.list(path)?;
        if "remix/".starts_with(path) || path.starts_with("remix/") {
            out.extend(REMIX.iter().map(|(n, _)| SharedString::from(format!("remix/{n}.svg"))).filter(|p| p.starts_with(path)));
        }
        Ok(out)
    }
}

/// A Remix icon tinted with `color` (e.g. `ri("radar-line", 14., MUTED)`).
pub fn ri(name: &'static str, size: f32, color: u32) -> Svg {
    debug_assert!(REMIX.iter().any(|(n, _)| *n == name), "missing Remix icon {name}");
    svg().path(SharedString::from(format!("remix/{name}.svg"))).size(px(size)).flex_shrink_0().text_color(super::c(color))
}
