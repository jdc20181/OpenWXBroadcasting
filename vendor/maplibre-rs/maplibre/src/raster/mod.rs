use std::{marker::PhantomData, rc::Rc};

use image::RgbaImage;

use crate::{
    coords::WorldTileCoords,
    environment::Environment,
    kernel::Kernel,
    plugin::Plugin,
    raster::{
        populate_world_system::PopulateWorldSystem, queue_system::queue_system,
        request_system::RequestSystem, resource::RasterResources, resource_system::resource_system,
        upload_system::upload_system,
    },
    render::{eventually::Eventually, tile_view_pattern::ViewTileSources, RenderStageLabel},
    schedule::Schedule,
    tcs::{system::SystemContainer, tiles::TileComponent, world::World},
};

mod populate_world_system;
mod process_raster;
mod queue_system;
mod render_commands;
mod request_system;
mod resource;
mod resource_system;
mod transferables;
mod upload_system;

pub use process_raster::{set_tile_filter, TileFilter};
pub use transferables::{
    DefaultRasterTransferables, LayerRaster, LayerRasterMissing, RasterTransferables,
};

use crate::render::graph::RenderGraph;

pub struct RasterPlugin<T>(PhantomData<T>);

impl<T: RasterTransferables> Default for RasterPlugin<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<E: Environment, T: RasterTransferables> Plugin<E> for RasterPlugin<T> {
    fn build(
        &self,
        schedule: &mut Schedule,
        kernel: Rc<Kernel<E>>,
        world: &mut World,
        _graph: &mut RenderGraph,
    ) {
        world
            .resources
            .insert(Eventually::<RasterResources>::Uninitialized);

        // CrowdedWX: which (tile, style layer) pairs were requested, shared by the request,
        // populate and upload systems so layers can be added, retried and evicted at runtime.
        world.resources.insert(RasterRequestState::default());

        world
            .resources
            .get_or_init_mut::<ViewTileSources>()
            .add_resource_query::<&Eventually<RasterResources>>();

        schedule.add_system_to_stage(
            RenderStageLabel::Extract,
            SystemContainer::new(RequestSystem::<E, T>::new(&kernel)),
        );
        schedule.add_system_to_stage(
            RenderStageLabel::Extract,
            SystemContainer::new(PopulateWorldSystem::<E, T>::new(&kernel)),
        );
        schedule.add_system_to_stage(RenderStageLabel::Prepare, resource_system);
        schedule.add_system_to_stage(RenderStageLabel::Queue, upload_system);
        schedule.add_system_to_stage(RenderStageLabel::Queue, queue_system); // FIXME tcs: Upload updates the TileView in tileviewpattern -> upload most run before prepare
    }
}

pub struct AvailableRasterLayerData {
    pub coords: WorldTileCoords,
    pub source_layer: String,
    pub image: RgbaImage,
}

pub struct MissingRasterLayerData {
    pub coords: WorldTileCoords,
    pub source_layer: String,
}

pub enum RasterLayerData {
    Available(AvailableRasterLayerData),
    Missing(MissingRasterLayerData),
}

#[derive(Default)]
pub struct RasterLayersDataComponent {
    pub layers: Vec<RasterLayerData>,
}

/// CrowdedWX: bookkeeping for raster tile requests, keyed by tile and style layer id.
///
/// MapLibre GL requests every raster source referenced by the style, including layers whose
/// `raster-opacity` is 0 (the usual way to pre-load animation frames). This mirrors that.
#[derive(Default)]
pub struct RasterRequestState {
    /// (tile, style layer id) pairs that were requested and not evicted.
    pub requested: std::collections::HashSet<(WorldTileCoords, String)>,
    /// Pairs whose last fetch failed, with the time of failure, so they can be retried later.
    pub failed: std::collections::HashMap<(WorldTileCoords, String), std::time::Instant>,
    /// CrowdedWX: visible pairs that were requested but have no texture yet, with when that was
    /// first seen. A pair stuck longer than [`Self::STUCK_AFTER`] is requested again, so a lost
    /// response can never leave the map blank.
    pub waiting_since: std::collections::HashMap<(WorldTileCoords, String), std::time::Instant>,
}

impl RasterRequestState {
    /// How long a failed tile waits before it is requested again.
    pub const RETRY_AFTER: std::time::Duration = std::time::Duration::from_secs(15);
    /// How long a visible tile may wait for its data before it is requested again.
    pub const STUCK_AFTER: std::time::Duration = std::time::Duration::from_secs(20);

    /// Forget every request for layers that are no longer in the style.
    pub fn retain_layers(&mut self, layer_ids: &std::collections::HashSet<String>) {
        self.requested.retain(|(_, id)| layer_ids.contains(id));
        self.failed.retain(|(_, id), _| layer_ids.contains(id));
        self.waiting_since.retain(|(_, id), _| layer_ids.contains(id));
    }

    /// Forget a single tile/layer pair so it is requested again when visible.
    pub fn forget(&mut self, coords: &WorldTileCoords, layer_id: &str) {
        let key = (*coords, layer_id.to_string());
        self.requested.remove(&key);
        self.failed.remove(&key);
        self.waiting_since.remove(&key);
    }
}

/// CrowdedWX: a raster style layer resolved against its style source, the way MapLibre GL
/// resolves `layer.source` → `sources[id].tiles`.
#[derive(Clone, Debug)]
pub struct ResolvedRasterLayer {
    /// Position of the layer in `style.layers`, used as draw order.
    pub index: u32,
    pub layer_id: String,
    pub source_id: String,
    pub template: String,
    pub scheme: crate::style::source::TileAddressingScheme,
    pub minzoom: u8,
    pub maxzoom: u8,
    pub opacity: f32,
    /// CrowdedWX: raster-brightness-min/max, -contrast, -saturation, -hue-rotate as uniform.
    pub paint: crate::raster::resource::RasterLayerUniform,
}

/// CrowdedWX: resolve all raster layers of a style to their tile templates, in draw order.
pub fn resolve_raster_layers(style: &crate::style::Style) -> Vec<ResolvedRasterLayer> {
    use crate::style::{layer::LayerPaint, source::Source};

    style
        .layers
        .iter()
        .enumerate()
        .filter_map(|(index, layer)| {
            let Some(LayerPaint::Raster(paint)) = &layer.paint else {
                return None;
            };
            let source_id = layer.source.clone()?;
            let Source::Raster(source) = style.sources.get(&source_id)? else {
                return None;
            };
            let template = source.tiles.as_ref()?.first()?.clone();
            Some(ResolvedRasterLayer {
                index: index as u32,
                layer_id: layer.id.clone(),
                source_id,
                template,
                scheme: source.scheme.clone().unwrap_or_default(),
                minzoom: layer.minzoom.unwrap_or(0).max(source.minzoom.unwrap_or(0)),
                maxzoom: source.maxzoom.unwrap_or(22),
                opacity: paint.raster_opacity.unwrap_or(1.0).clamp(0.0, 1.0),
                paint: crate::raster::resource::RasterLayerUniform::from_paint(
                    paint.raster_opacity.unwrap_or(1.0).clamp(0.0, 1.0),
                    paint.raster_brightness_min.unwrap_or(0.0).clamp(0.0, 1.0),
                    paint.raster_brightness_max.unwrap_or(1.0).clamp(0.0, 1.0),
                    paint.raster_contrast.unwrap_or(0.0).clamp(-1.0, 0.99),
                    paint.raster_saturation.unwrap_or(0.0).clamp(-1.0, 1.0),
                    paint.raster_hue_rotate.unwrap_or(0.0),
                ),
            })
        })
        .collect()
}

impl TileComponent for RasterLayersDataComponent {}
