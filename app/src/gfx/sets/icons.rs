//! Unmodified bundled Remix calendar SVG (Apache-2.0), rasterised in this lane.
use std::sync::OnceLock;
use tiny_skia::{Mask, Pixmap, PixmapPaint, Transform};

pub(super) fn draw(
    pm: &mut Pixmap,
    local: Transform,
    mask: Option<&Mask>,
    w: f32,
    h: f32,
    spec: &str,
    opacity: f32,
) -> bool {
    if spec != "set:calendar-icon" {
        return false;
    }
    static ICON: OnceLock<Option<Pixmap>> = OnceLock::new();
    let icon = ICON.get_or_init(|| {
        let svg = include_str!("../../../assets/icons/remix/calendar-line.svg")
            .replace("currentColor", "#FFFFFF");
        let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).ok()?;
        let mut raster = resvg::tiny_skia::Pixmap::new(96, 96)?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(
                96. / tree.size().width(),
                96. / tree.size().height(),
            ),
            &mut raster.as_mut(),
        );
        Pixmap::from_vec(raster.take(), tiny_skia::IntSize::from_wh(96, 96)?)
    });
    if let Some(icon) = icon {
        pm.draw_pixmap(
            0,
            0,
            icon.as_ref(),
            &PixmapPaint {
                opacity: opacity.clamp(0., 1.),
                ..Default::default()
            },
            Transform::from_scale(w / 96., h / 96.).post_concat(local),
            mask,
        );
    }
    true
}
