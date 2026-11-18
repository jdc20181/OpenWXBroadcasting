//! Requests tiles which are currently in view
//!
//! CrowdedWX: rewritten so raster tiles follow the style. Every raster layer is resolved against
//! its style source (`tiles` URL template, `scheme`, `minzoom`/`maxzoom`) and each
//! (tile, style layer) pair is requested once. Layers added to the style later (for example a
//! new radar frame) are requested on the next frame, failed tiles are retried after
//! [`RasterRequestState::RETRY_AFTER`], and tiles above a source's `maxzoom` are over-zoomed from
//! their ancestor tile like MapLibre GL does.

use std::{borrow::Cow, collections::HashMap, marker::PhantomData, rc::Rc, time::Instant};

use crate::{
    context::MapContext,
    coords::{WorldTileCoords, ZoomLevel},
    environment::{Environment, OffscreenKernel},
    io::{
        apc::{AsyncProcedureCall, AsyncProcedureFuture, Context, Input, ProcedureError},
        source_type::{RasterSource, SourceType},
    },
    kernel::Kernel,
    raster::{
        process_raster::{process_raster_tile, ProcessRasterContext, RasterTileRequest},
        resolve_raster_layers,
        transferables::{LayerRasterMissing, RasterTransferables},
        RasterLayerData, RasterLayersDataComponent, RasterRequestState,
    },
    render::{tile_view_pattern::DEFAULT_TILE_SIZE, view_state::ViewStatePadding},
    style::Style,
    tcs::system::{System, SystemResult},
};

pub struct RequestSystem<E: Environment, T: RasterTransferables> {
    kernel: Rc<Kernel<E>>,
    phantom_t: PhantomData<T>,
}

impl<E: Environment, T: RasterTransferables> RequestSystem<E, T> {
    pub fn new(kernel: &Rc<Kernel<E>>) -> Self {
        Self {
            kernel: kernel.clone(),
            phantom_t: Default::default(),
        }
    }
}

impl<E: Environment, T: RasterTransferables> System for RequestSystem<E, T> {
    fn name(&self) -> Cow<'static, str> {
        "raster_request".into()
    }

    fn run(
        &mut self,
        MapContext {
            style,
            view_state,
            world,
            ..
        }: &mut MapContext,
    ) -> SystemResult {
        let view_region = view_state.create_view_region(
            view_state.zoom().zoom_level(DEFAULT_TILE_SIZE),
            ViewStatePadding::Loose,
        );

        let layers = resolve_raster_layers(style);

        if let (Some(view_region), false) = (&view_region, layers.is_empty()) {
            let now = Instant::now();
            // Requests to send, grouped per tile so one tile entity is spawned once.
            let mut to_request: HashMap<WorldTileCoords, Vec<String>> = HashMap::new();
            // CrowdedWX: failed tiles being retried. Their old "missing" record is cleared so the
            // stuck-tile watchdog covers the retry: if the retry's answer is lost (a timeout, a
            // dropped connection), the tile is requested again instead of staying blank.
            let mut retrying: Vec<(WorldTileCoords, String)> = Vec::new();

            {
                let state = world
                    .resources
                    .get_or_init_mut::<RasterRequestState>();

                for coords in view_region.iter() {
                    if coords.build_quad_key().is_none() {
                        continue;
                    }
                    let z = u8::from(coords.z);

                    for layer in &layers {
                        if z < layer.minzoom {
                            continue;
                        }
                        let key = (coords, layer.layer_id.clone());
                        if state.requested.contains(&key) {
                            // Retry tiles whose fetch failed a while ago (e.g. a 503 from a proxy).
                            match state.failed.get(&key) {
                                Some(failed_at)
                                    if now.duration_since(*failed_at)
                                        >= RasterRequestState::RETRY_AFTER => {}
                                _ => continue,
                            }
                        }
                        if state.failed.remove(&key).is_some() {
                            retrying.push(key.clone());
                        }
                        state.requested.insert(key);
                        to_request
                            .entry(coords)
                            .or_default()
                            .push(layer.layer_id.clone());
                    }
                }
            }

            for (coords, layer_id) in retrying {
                if let Some(component) = world.tiles.query_mut::<&mut RasterLayersDataComponent>(coords) {
                    component.layers.retain(|data| match data {
                        RasterLayerData::Missing(m) => m.source_layer != layer_id,
                        RasterLayerData::Available(_) => true,
                    });
                }
            }

            for (coords, layer_ids) in to_request {
                if world
                    .tiles
                    .query::<&RasterLayersDataComponent>(coords)
                    .is_none()
                {
                    world
                        .tiles
                        .spawn_mut(coords)
                        .expect("unable to spawn a raster tile")
                        .insert(RasterLayersDataComponent::default());
                }

                for layer_id in layer_ids {
                    log::debug!("raster tile request started: {coords} ({layer_id})");

                    // One procedure call per style layer, so layers load in parallel. The style
                    // passed along contains only that layer and its source.
                    let Some(layer_style) = single_layer_style(style, &layer_id) else {
                        continue;
                    };

                    self.kernel
                        .apc()
                        .call(
                            Input::TileRequest {
                                coords,
                                style: layer_style,
                            },
                            fetch_raster_apc::<
                                E::OffscreenKernelEnvironment,
                                T,
                                <E::AsyncProcedureCall as AsyncProcedureCall<
                                    E::OffscreenKernelEnvironment,
                                >>::Context,
                            >,
                        )
                        .expect("unable to call APC"); // TODO: Remove unwrap
                }
            }
        }

        view_state.update_references();

        Ok(())
    }
}

/// CrowdedWX: a copy of `style` that only contains `layer_id` and the source it uses.
fn single_layer_style(style: &Style, layer_id: &str) -> Option<Style> {
    let layer = style.layers.iter().find(|layer| layer.id == layer_id)?.clone();
    let source_id = layer.source.clone()?;
    let source = style.sources.get(&source_id)?.clone();

    let mut sources = HashMap::new();
    sources.insert(source_id, source);

    Some(Style {
        version: style.version,
        name: None,
        metadata: Default::default(),
        sources,
        layers: vec![layer],
        center: None,
        zoom: None,
        pitch: None,
    })
}

/// CrowdedWX: when `coords` is deeper than the source's `maxzoom`, the tile to fetch is its
/// ancestor at `maxzoom`, plus the sub-rectangle of that ancestor that covers `coords`
/// (`(x, y, size)` in units of `1 / 2^dz` of the ancestor).
fn overzoom(coords: WorldTileCoords, maxzoom: u8) -> (WorldTileCoords, Option<(u32, u32, u32)>) {
    let z = u8::from(coords.z);
    if z <= maxzoom {
        return (coords, None);
    }
    let dz = (z - maxzoom) as u32;
    let parent = WorldTileCoords {
        x: coords.x >> dz,
        y: coords.y >> dz,
        z: ZoomLevel::new(maxzoom),
    };
    let divisions = 1u32 << dz;
    let sub_x = (coords.x as u32) & (divisions - 1);
    let sub_y = (coords.y as u32) & (divisions - 1);
    (parent, Some((sub_x, sub_y, divisions)))
}

pub fn fetch_raster_apc<K: OffscreenKernel, T: RasterTransferables, C: Context + Clone + Send>(
    input: Input,
    context: C,
    kernel: K,
) -> AsyncProcedureFuture {
    Box::pin(async move {
        let Input::TileRequest { coords, style } = input else {
            return Err(ProcedureError::IncompatibleInput);
        };

        let client = kernel.source_client();

        for layer in resolve_raster_layers(&style) {
            let (fetch_coords, sub_tile) = overzoom(coords, layer.maxzoom);
            let source =
                SourceType::Raster(RasterSource::from_template(&layer.template, layer.scheme));

            let result = match client.fetch(&fetch_coords, &source).await {
                Ok(data) => {
                    let mut process_context = ProcessRasterContext::<T, C>::new(context.clone());
                    process_raster_tile(
                        &data,
                        RasterTileRequest {
                            coords,
                            layer_id: layer.layer_id.clone(),
                            sub_tile,
                        },
                        &mut process_context,
                    )
                    .map_err(|e| format!("{e:?}"))
                }
                Err(e) => Err(format!("{e:?}")),
            };

            if let Err(e) = result {
                log::warn!(
                    "raster tile {coords} for layer {} failed: {e}",
                    layer.layer_id
                );
                context
                    .send_back(<T as RasterTransferables>::LayerRasterMissing::build_from(
                        coords,
                        layer.layer_id.clone(),
                    ))
                    .map_err(ProcedureError::Send)?;
            }
        }

        Ok(())
    })
}
