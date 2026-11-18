//! CrowdedWX: an off-screen, camera-driven map.
//!
//! [`super::map::HeadlessMap`] renders a single, pre-processed tile. Applications that embed
//! maplibre-rs in another UI toolkit (CrowdedWXBroadcaster embeds it in GPUI) need the regular
//! map loop instead: the camera decides which tiles are requested, tiles stream in through the
//! style's sources, and every rendered frame is handed back as pixels.
//!
//! [`OffscreenMap`] is that loop. It is built from the same pieces as the headed map
//! ([`RenderPlugin`], [`BackgroundPlugin`], [`RasterPlugin`]) and the headless surface, and it
//! adds [`FrameReadbackPlugin`], which copies each frame into a [`FrameSink`].

use std::{
    borrow::Cow,
    rc::Rc,
    sync::{Arc, Mutex},
};

use crate::{
    background::BackgroundPlugin,
    context::MapContext,
    coords::{LatLon, WorldCoords, Zoom},
    environment::OffscreenKernelConfig,
    headless::{
        environment::HeadlessEnvironment,
        graph_node::CopySurfaceBufferNode,
        window::{HeadlessMapWindow, HeadlessMapWindowConfig},
    },
    io::apc::SchedulerAsyncProcedureCall,
    kernel::{Kernel, KernelBuilder},
    map::MapError,
    platform::{http_client::ReqwestHttpClient, scheduler::TokioScheduler},
    plugin::Plugin,
    raster::{DefaultRasterTransferables, RasterPlugin},
    render::{
        builder::RendererBuilder, graph::RenderGraph, resource::Head, view_state::ViewState,
        RenderPlugin, RenderStageLabel, Renderer,
    },
    schedule::{Schedule, Stage},
    style::Style,
    tcs::{
        system::{System, SystemContainer, SystemError, SystemResult},
        world::World,
    },
    window::{MapWindowConfig, PhysicalSize},
};

/// Labels of the render graph the frame copy is attached to (see `render::draw_graph`).
const DRAW_GRAPH: &str = "draw";
const MAIN_PASS: &str = "main_pass";
const TRANSLUCENT_PASS: &str = "translucent_pass";
const COPY_PASS: &str = "crowdedwx_copy_pass";

/// MapLibre GL's default vertical field of view (≈36.87°), as used by the headed map.
const FOVY_RADIANS: f64 = 0.6435011087932844;

/// One rendered frame, tightly packed (no row padding), 4 bytes per pixel.
#[derive(Clone)]
pub struct OffscreenFrame {
    pub width: u32,
    pub height: u32,
    /// Pixel data in the surface's byte order, see [`Self::bgra`].
    pub pixels: Vec<u8>,
    /// `true` when `pixels` are BGRA, `false` when RGBA.
    pub bgra: bool,
    /// Increments with every rendered frame.
    pub serial: u64,
}

/// Shared slot that always holds the most recent frame.
#[derive(Clone, Default)]
pub struct FrameSink(Arc<Mutex<Option<OffscreenFrame>>>);

impl FrameSink {
    /// Take the latest frame, leaving the slot empty until the next render.
    pub fn take(&self) -> Option<OffscreenFrame> {
        self.0.lock().ok()?.take()
    }

    /// Copy of the latest frame without consuming it.
    pub fn latest(&self) -> Option<OffscreenFrame> {
        self.0.lock().ok()?.clone()
    }

    fn put(&self, frame: OffscreenFrame) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some(frame);
        }
    }
}

/// The camera, in MapLibre terms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraOptions {
    pub latitude: f64,
    pub longitude: f64,
    pub zoom: f64,
}

pub struct OffscreenMap {
    // Kept alive for the async procedure calls that systems schedule through it.
    _kernel: Rc<Kernel<HeadlessEnvironment>>,
    schedule: Schedule,
    context: MapContext,
    sink: FrameSink,
    camera: CameraOptions,
}

impl OffscreenMap {
    /// Create an off-screen map of `width`×`height` physical pixels.
    ///
    /// `cache_path` enables the on-disk HTTP tile cache (honours `Cache-Control`).
    /// Must be called inside a multi-threaded Tokio runtime: tile requests run on it.
    pub async fn new(
        style: Style,
        width: u32,
        height: u32,
        cache_path: Option<String>,
    ) -> Result<Self, MapError> {
        let size = PhysicalSize::new(width.max(1), height.max(1))
            .expect("map size must be non-zero");

        let kernel: Kernel<HeadlessEnvironment> = KernelBuilder::new()
            .with_map_window_config(HeadlessMapWindowConfig::new(size))
            .with_http_client(ReqwestHttpClient::new(cache_path.clone()))
            .with_apc(SchedulerAsyncProcedureCall::new(
                TokioScheduler::new(),
                OffscreenKernelConfig {
                    cache_directory: cache_path,
                },
            ))
            .with_scheduler(TokioScheduler::new())
            .build();

        let window: HeadlessMapWindow = kernel.map_window_config().create()?;

        let mut renderer = RendererBuilder::new()
            .build()
            .initialize_headless::<HeadlessMapWindowConfig>(&window)
            .await
            .map_err(MapError::DeviceInit)?;

        let window_size = renderer.state().surface().size();
        let center = style.center.unwrap_or([0.0, 0.0]);
        let camera = CameraOptions {
            latitude: center[0],
            longitude: center[1],
            zoom: style.zoom.unwrap_or(2.0),
        };
        let zoom = Zoom::new(camera.zoom);

        let view_state = ViewState::new(
            window_size,
            WorldCoords::from_lat_lon(LatLon::new(camera.latitude, camera.longitude), zoom),
            zoom,
            cgmath::Deg::<f64>(0.0),
            cgmath::Rad(FOVY_RADIANS),
        );

        let kernel = Rc::new(kernel);
        let mut world = World::default();
        let mut schedule = Schedule::default();
        let sink = FrameSink::default();

        let plugins: Vec<Box<dyn Plugin<HeadlessEnvironment>>> = vec![
            Box::new(RenderPlugin::default()),
            Box::new(BackgroundPlugin::default()),
            Box::new(RasterPlugin::<DefaultRasterTransferables>::default()),
            Box::new(FrameReadbackPlugin { sink: sink.clone() }),
        ];
        for plugin in &plugins {
            plugin.build(
                &mut schedule,
                kernel.clone(),
                &mut world,
                &mut renderer.render_graph,
            );
        }

        Ok(Self {
            _kernel: kernel,
            schedule,
            context: MapContext {
                style,
                view_state,
                world,
                renderer,
            },
            sink,
            camera,
        })
    }

    /// Where rendered frames are delivered.
    pub fn frame_sink(&self) -> FrameSink {
        self.sink.clone()
    }

    pub fn camera(&self) -> CameraOptions {
        self.camera
    }

    /// Move the camera so `latitude`/`longitude` is at the center of the view (like
    /// `map.jumpTo({center, zoom})` in MapLibre GL).
    pub fn jump_to(&mut self, camera: CameraOptions) {
        self.camera = camera;
        let zoom = Zoom::new(camera.zoom);
        let view_state = &mut self.context.view_state;
        view_state.update_zoom(zoom);
        let world = WorldCoords::from_lat_lon(LatLon::new(camera.latitude, camera.longitude), zoom);
        view_state
            .camera_mut()
            .move_to(cgmath::Point2::new(world.x, world.y));
    }

    /// Replace the style (like `map.setStyle(style, {diff: true})`): raster layers that stay keep
    /// their tiles, new layers are requested, removed layers are evicted.
    pub fn set_style(&mut self, style: Style) {
        self.context.style = style;
    }

    pub fn style(&self) -> &Style {
        &self.context.style
    }

    /// Run one update + render pass. The frame lands in [`Self::frame_sink`].
    pub fn render(&mut self) -> Result<(), MapError> {
        self.schedule.run(&mut self.context)?;
        Ok(())
    }

    pub fn size(&self) -> (u32, u32) {
        let size = self.context.renderer.state().surface().size();
        (size.width(), size.height())
    }
}

/// Copies every rendered frame from the headless surface into a [`FrameSink`].
pub struct FrameReadbackPlugin {
    pub sink: FrameSink,
}

impl Plugin<HeadlessEnvironment> for FrameReadbackPlugin {
    fn build(
        &self,
        schedule: &mut Schedule,
        _kernel: Rc<Kernel<HeadlessEnvironment>>,
        _world: &mut World,
        graph: &mut RenderGraph,
    ) {
        let draw_graph = graph
            .get_sub_graph_mut(DRAW_GRAPH)
            .expect("draw sub graph must exist (RenderPlugin builds it)");
        draw_graph.add_node(COPY_PASS, CopySurfaceBufferNode::default());
        draw_graph
            .add_node_edge(MAIN_PASS, COPY_PASS)
            .expect("main pass must exist");
        // The translucent pass is optional; when present the copy must run after it too.
        let _ = draw_graph.add_node_edge(TRANSLUCENT_PASS, COPY_PASS);

        schedule.add_system_to_stage(
            RenderStageLabel::Cleanup,
            SystemContainer::new(FrameReadbackSystem {
                sink: self.sink.clone(),
                serial: 0,
            }),
        );
    }
}

struct FrameReadbackSystem {
    sink: FrameSink,
    serial: u64,
}

impl System for FrameReadbackSystem {
    fn name(&self) -> Cow<'static, str> {
        "crowdedwx_frame_readback".into()
    }

    fn run(
        &mut self,
        MapContext {
            renderer:
                Renderer {
                    resources: state,
                    device,
                    ..
                },
            ..
        }: &mut MapContext,
    ) -> SystemResult {
        let surface = state.surface();
        let Head::Headless(buffered_texture) = surface.head() else {
            return Err(SystemError::Setup);
        };
        let buffered_texture = buffered_texture.clone();

        let size = surface.size();
        let (width, height) = (size.width(), size.height());
        let format = surface.surface_format();

        let device = device.clone();
        let buffer_slice = buffered_texture.map_async(&device);
        let padded = buffer_slice.get_mapped_range();

        let padded_row = buffered_texture.bytes_per_row() as usize;
        let row = width as usize * 4;
        let mut pixels = Vec::with_capacity(row * height as usize);
        for chunk in padded.chunks(padded_row).take(height as usize) {
            pixels.extend_from_slice(&chunk[..row]);
        }

        // All mapped views must be dropped before the buffer is unmapped.
        drop(padded);
        buffered_texture.unmap();

        self.serial += 1;
        self.sink.put(OffscreenFrame {
            width,
            height,
            pixels,
            bgra: matches!(
                format,
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
            ),
            serial: self.serial,
        });

        Ok(())
    }
}
