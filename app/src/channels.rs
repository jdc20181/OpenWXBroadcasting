//! Extra graphics output channels (Channel 2 and 3): independent of the Program bus, each plays
//! its own graphic with transparency kept, for a second screen, a lobby display, a keyed overlay
//! on a hardware switcher, or a social/stream feed. Served by the control server at /channel2 …

use std::{
    sync::{Arc, Mutex, OnceLock, RwLock},
    time::{Duration, Instant},
};

use crate::{config::Settings, data::WxSnapshot, gfx::model::GraphicDoc, program::BusFrame};

pub const COUNT: usize = 2;

#[derive(Clone)]
struct ChannelState {
    doc: Option<GraphicDoc>,
    started: Instant,
    data: Arc<WxSnapshot>,
    settings: Arc<Settings>,
}

pub struct Channel {
    pub number: usize,
    state: Arc<RwLock<ChannelState>>,
    bus: Arc<Mutex<Option<BusFrame>>>,
}

fn all() -> &'static Vec<Channel> {
    static CHANNELS: OnceLock<Vec<Channel>> = OnceLock::new();
    CHANNELS.get_or_init(|| (0..COUNT).map(|i| Channel::start(i + 2)).collect())
}

/// Channel by its number (2 or 3).
pub fn get(number: usize) -> Option<&'static Channel> {
    all().iter().find(|c| c.number == number)
}

pub fn numbers() -> Vec<usize> {
    (2..2 + COUNT).collect()
}

/// Keep every channel's data and settings current (called with each data refresh).
pub fn update_data(data: Arc<WxSnapshot>, settings: Arc<Settings>) {
    for c in all() {
        // Multi-channel branding: a channel can use its own on-air theme.
        let themed = settings.channel_themes.iter().find(|(n, _)| *n == c.number).and_then(|(_, theme)| {
            crate::config::brand_themes().into_iter().find(|(name, _)| *name == theme.as_str()).map(|(_, brand)| {
                let mut s = (*settings).clone();
                let (logo, display, body) = (s.brand.logo_media.clone(), s.brand.font_display.clone(), s.brand.font_body.clone());
                s.brand = brand;
                s.brand.logo_media = logo;
                s.brand.font_display = display;
                s.brand.font_body = body;
                Arc::new(s)
            })
        });
        if let Ok(mut s) = c.state.write() {
            s.data = data.clone();
            s.settings = themed.unwrap_or_else(|| settings.clone());
        }
    }
}

impl Channel {
    fn start(number: usize) -> Self {
        let state = Arc::new(RwLock::new(ChannelState {
            doc: None,
            started: Instant::now(),
            data: Arc::new(WxSnapshot::default()),
            settings: Arc::new(Settings::default()),
        }));
        let bus = Arc::new(Mutex::new(None));
        let (st, b) = (state.clone(), bus.clone());
        let _ = std::thread::Builder::new().name(format!("channel-{number}")).spawn(move || {
            let mut serial = 0u64;
            loop {
                let snapshot = st.read().ok().map(|s| s.clone());
                let Some(s) = snapshot else {
                    std::thread::sleep(Duration::from_millis(200));
                    continue;
                };
                let Some(doc) = s.doc else {
                    if let Ok(mut slot) = b.lock() {
                        *slot = None;
                    }
                    std::thread::sleep(Duration::from_millis(200));
                    continue;
                };
                let started = Instant::now();
                let t = s.started.elapsed().as_secs_f32();
                let pm = crate::program::render_graphic(&doc, &s.settings, &s.data, t, crate::config::FRAME_W, crate::config::FRAME_H, None);
                // Straight-alpha BGRA so keyed outputs keep their transparency.
                let mut bgra = pm.data().to_vec();
                for px in bgra.chunks_exact_mut(4) {
                    let a = px[3] as u32;
                    if a > 0 && a < 255 {
                        for c in px.iter_mut().take(3) {
                            *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                        }
                    }
                    px.swap(0, 2);
                }
                serial += 1;
                if let Ok(mut slot) = b.lock() {
                    *slot = Some(BusFrame { serial, width: pm.width(), height: pm.height(), bgra: Arc::new(bgra) });
                }
                // 15 fps is plenty for a graphics channel and keeps the CPU free for Program.
                let spent = started.elapsed();
                let budget = Duration::from_millis(66);
                if spent < budget {
                    std::thread::sleep(budget - spent);
                }
            }
        });
        Self { number, state, bus }
    }

    pub fn take(&self, doc: GraphicDoc) {
        if let Ok(mut s) = self.state.write() {
            s.doc = Some(doc);
            s.started = Instant::now();
        }
    }

    /// Start at an exact instant (synchronized takes across channels and Program).
    pub fn take_at(&self, doc: GraphicDoc, started: Instant) {
        if let Ok(mut s) = self.state.write() {
            s.doc = Some(doc);
            s.started = started;
        }
    }

    pub fn clear(&self) {
        if let Ok(mut s) = self.state.write() {
            s.doc = None;
        }
    }

    pub fn current(&self) -> Option<String> {
        self.state.read().ok().and_then(|s| s.doc.as_ref().map(|d| d.name.clone()))
    }

    pub fn frame(&self) -> Option<BusFrame> {
        self.bus.lock().ok().and_then(|s| s.clone())
    }
}
