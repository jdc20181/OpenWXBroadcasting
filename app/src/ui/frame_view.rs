//! Paints a broadcast frame (16:9) letterboxed into its bounds and remembers where, so mouse
//! positions can be mapped back to frame pixels.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use gpui_kit::*;

use crate::config::{FRAME_H, FRAME_W};

#[derive(Default)]
pub struct ImageSlot {
    last: RefCell<Option<Arc<RenderImage>>>,
    rect: Cell<Bounds<Pixels>>,
}

impl ImageSlot {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    /// Window position → frame pixel (0..1920, 0..1080), or None outside the picture.
    pub fn to_frame(&self, position: Point<Pixels>) -> Option<(f64, f64)> {
        let rect = self.rect.get();
        let w = f32::from(rect.size.width);
        let h = f32::from(rect.size.height);
        if w <= 1.0 || h <= 1.0 {
            return None;
        }
        let fx = f32::from(position.x - rect.origin.x) / w;
        let fy = f32::from(position.y - rect.origin.y) / h;
        if !(0.0..=1.0).contains(&fx) || !(0.0..=1.0).contains(&fy) {
            return None;
        }
        Some((fx as f64 * FRAME_W as f64, fy as f64 * FRAME_H as f64))
    }

    /// Like [`Self::to_frame`] but clamped to the picture (for drags that leave it).
    pub fn to_frame_clamped(&self, position: Point<Pixels>) -> (f64, f64) {
        let rect = self.rect.get();
        let w = f32::from(rect.size.width).max(1.0);
        let h = f32::from(rect.size.height).max(1.0);
        let fx = (f32::from(position.x - rect.origin.x) / w).clamp(0.0, 1.0);
        let fy = (f32::from(position.y - rect.origin.y) / h).clamp(0.0, 1.0);
        (fx as f64 * FRAME_W as f64, fy as f64 * FRAME_H as f64)
    }

    /// Scale from frame pixels to window pixels.
    pub fn scale(&self) -> f32 {
        f32::from(self.rect.get().size.width) / FRAME_W as f32
    }

    pub fn rect(&self) -> Bounds<Pixels> {
        self.rect.get()
    }

    pub fn set_rect(&self, rect: Bounds<Pixels>) {
        self.rect.set(rect);
    }

    /// Remember the image painted this frame and free the previous one from the window's atlas.
    pub fn track(&self, image: Arc<RenderImage>, window: &mut Window) {
        let mut last = self.last.borrow_mut();
        if let Some(previous) = last.as_ref() {
            if !Arc::ptr_eq(previous, &image) {
                let _ = window.drop_image(previous.clone());
            }
        }
        *last = Some(image);
    }
}

/// Letterboxed 16:9 rect inside `bounds`.
pub fn fit_16_9(bounds: Bounds<Pixels>, aspect: f32) -> Bounds<Pixels> {
    let bw = f32::from(bounds.size.width);
    let bh = f32::from(bounds.size.height);
    let (w, h) = if bw / bh.max(1.0) > aspect {
        (bh * aspect, bh)
    } else {
        (bw, bw / aspect)
    };
    Bounds::new(
        point(
            bounds.origin.x + px((bw - w) / 2.0),
            bounds.origin.y + px((bh - h) / 2.0),
        ),
        size(px(w), px(h)),
    )
}

/// A canvas that paints `image` (or a "no signal" plate) into the slot.
pub fn frame_canvas(image: Option<Arc<RenderImage>>, slot: Rc<ImageSlot>) -> Canvas<Bounds<Pixels>> {
    frame_canvas_with_trace(image, slot, Vec::new(), false)
}

pub fn frame_canvas_with_trace(image: Option<Arc<RenderImage>>, slot: Rc<ImageSlot>, trace: Vec<(f64, f64)>, show_closure: bool) -> Canvas<Bounds<Pixels>> {
    let aspect = FRAME_W as f32 / FRAME_H as f32;
    canvas(
        move |bounds, _, _| bounds,
        move |_, bounds, window, _cx| {
            window.paint_quad(fill(bounds, rgb(0x05070B)));
            let rect = fit_16_9(bounds, aspect);
            slot.rect.set(rect);
            match image {
                Some(image) => {
                    let _ = window.paint_image(rect, rect, Corners::default(), image.clone(), 0, false);
                    let mut last = slot.last.borrow_mut();
                    if let Some(previous) = last.as_ref() {
                        if !Arc::ptr_eq(previous, &image) {
                            let _ = window.drop_image(previous.clone());
                        }
                    }
                    *last = Some(image);
                }
                None => {
                    window.paint_quad(fill(rect, rgb(0x0E1320)));
                }
            }
            if let Some(&first) = trace.first() {
                let to_window = |(x, y): (f64, f64)| {
                    let scale = f32::from(rect.size.width) / FRAME_W as f32;
                    point(rect.origin.x + px(x as f32 * scale), rect.origin.y + px(y as f32 * scale))
                };
                let start = to_window(first);
                if trace.len() > 1 {
                    let mut path = PathBuilder::stroke(px(2.0));
                    path.move_to(start);
                    for &point in &trace[1..] {
                        path.line_to(to_window(point));
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, rgb(0xFFFFFF));
                    }
                }
                if show_closure {
                    if let Some(&last) = trace.last() {
                        let (dx, dy) = (first.0 - last.0, first.1 - last.1);
                        let steps = ((dx.hypot(dy) * f64::from(slot.scale()) / 10.0).ceil() as usize).max(1);
                        let mut guide = PathBuilder::stroke(px(1.5));
                        for i in 0..steps {
                            let t = i as f64 / steps as f64;
                            let end = (i as f64 + 0.5) / steps as f64;
                            guide.move_to(to_window((last.0 + dx * t, last.1 + dy * t)));
                            guide.line_to(to_window((last.0 + dx * end, last.1 + dy * end)));
                        }
                        if let Ok(guide) = guide.build() { window.paint_path(guide, rgb(0x2ED3F3)); }
                    }
                }
                let marker = Bounds::new(point(start.x - px(4.0), start.y - px(4.0)), size(px(8.0), px(8.0)));
                window.paint_quad(fill(marker, rgb(0x2ED3F3)));
            }
        },
    )
}
