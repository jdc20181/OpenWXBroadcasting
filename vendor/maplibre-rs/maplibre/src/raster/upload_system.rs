use std::collections::HashSet;

use crate::{
    context::MapContext,
    coords::{ViewRegion, WorldTileCoords},
    raster::{
        resolve_raster_layers, resource::RasterResources, RasterLayerData,
        RasterLayersDataComponent, RasterRequestState, ResolvedRasterLayer,
    },
    render::{
        eventually::{Eventually, Eventually::Initialized},
        tile_view_pattern::DEFAULT_TILE_SIZE,
        view_state::ViewStatePadding,
        Renderer,
    },
    tcs::{
        system::{SystemError, SystemResult},
        tiles::Tiles,
    },
};

/// CrowdedWX: above this many bound tile textures, textures of tiles outside the view are evicted.
/// A 512 px radar tile is 1 MB of texture, and on integrated GPUs that memory is shared with the
/// app, so this stays well under what a 12-frame loop needs at once.
const MAX_BOUND_TEXTURES: usize = 600;

pub fn upload_system(
    MapContext {
        world,
        style,
        view_state,
        renderer: Renderer { device, queue, .. },
        ..
    }: &mut MapContext,
) -> SystemResult {
    let layers = resolve_raster_layers(style);
    let layer_ids: HashSet<String> = layers.iter().map(|l| l.layer_id.clone()).collect();

    let view_region = view_state.create_view_region(
        view_state.zoom().zoom_level(DEFAULT_TILE_SIZE),
        ViewStatePadding::Loose,
    );

    let mut untextured: Vec<(WorldTileCoords, String)> = Vec::new();
    let mut visible = 0usize;
    let mut textures = 0usize;
    let evicted = {
        let Some(Initialized(raster_resources)) = world
            .resources
            .query_mut::<&mut Eventually<RasterResources>>()
        else {
            return Err(SystemError::Dependencies);
        };

        // CrowdedWX: layers that left the style free their GPU memory.
        raster_resources.retain_layers(&layer_ids);

        // CrowdedWX: `raster-opacity` is applied through a per-layer uniform every frame, so
        // changing it never requires re-uploading tiles.
        for layer in &layers {
            raster_resources.set_layer_opacity(device, queue, &layer.layer_id, layer.paint);
        }

        let mut evicted = Vec::new();
        if let Some(view_region) = &view_region {
            upload_raster_layers(
                raster_resources,
                device,
                queue,
                &mut world.tiles,
                &layers,
                view_region,
            );

            if raster_resources.texture_count() > MAX_BOUND_TEXTURES {
                let keep: HashSet<WorldTileCoords> = view_region.iter().collect();
                evicted = raster_resources.evict_tiles_outside(&keep);
            }

            // CrowdedWX: which visible tiles still have no texture, per layer (for the
            // stuck-tile watchdog and the periodic diagnostics line).
            for coords in view_region.iter() {
                for layer in &layers {
                    if !raster_resources.has_layer_texture(&layer.layer_id, &coords) {
                        untextured.push((coords, layer.layer_id.clone()));
                    }
                }
            }
            visible = view_region.iter().count();
            textures = raster_resources.texture_count();
        }
        evicted
    };

    // CrowdedWX: decoded images (1 MB per 512 px tile) only leave the CPU when uploaded. Free the
    // ones for layers that left the style (expired radar frames) and for tiles outside the view,
    // which were otherwise kept for the life of the map.
    let mut evicted = evicted;
    if let Some(view_region) = &view_region {
        let keep: HashSet<WorldTileCoords> = view_region.iter().collect();
        world
            .tiles
            .for_each_mut::<RasterLayersDataComponent>(|coords, component| {
                component.layers.retain(|data| {
                    let (id, available) = match data {
                        RasterLayerData::Available(d) => (&d.source_layer, true),
                        RasterLayerData::Missing(d) => (&d.source_layer, false),
                    };
                    if !layer_ids.contains(id) {
                        return false;
                    }
                    if available && !keep.contains(&coords) {
                        evicted.push((id.clone(), coords));
                        return false;
                    }
                    true
                });
            });
    }

    // Tiles whose fetch ended in a recorded "missing" result are handled by the failure retry,
    // not the watchdog.
    let untextured: Vec<(WorldTileCoords, String)> = untextured
        .into_iter()
        .filter(|(coords, id)| {
            world
                .tiles
                .query::<&RasterLayersDataComponent>(*coords)
                .map(|c| {
                    !c.layers.iter().any(|d| match d {
                        RasterLayerData::Missing(m) => &m.source_layer == id,
                        RasterLayerData::Available(_) => false,
                    })
                })
                .unwrap_or(true)
        })
        .collect();

    let state = world.resources.get_or_init_mut::<RasterRequestState>();
    state.retain_layers(&layer_ids);
    for (layer_id, coords) in evicted {
        state.forget(&coords, &layer_id);
    }

    // CrowdedWX: stuck-tile watchdog. A visible tile that was requested but still has no texture
    // after STUCK_AFTER (its response was lost, or its data was dropped) is requested again.
    let now = std::time::Instant::now();
    let waiting: HashSet<(WorldTileCoords, String)> = untextured.iter().cloned().collect();
    state.waiting_since.retain(|key, _| waiting.contains(key));
    let mut requeued = 0usize;
    for key in untextured {
        if !state.requested.contains(&key) || state.failed.contains_key(&key) {
            continue;
        }
        let since = *state.waiting_since.entry(key.clone()).or_insert(now);
        if now.duration_since(since) > RasterRequestState::STUCK_AFTER {
            state.forget(&key.0, &key.1);
            requeued += 1;
        }
    }
    if requeued > 0 {
        log::warn!("map watchdog: {requeued} visible tiles had no data after 20 s and were requested again");
    }

    // CrowdedWX: once a minute, a one-line picture of the map's tile state for Developer Logs.
    {
        use std::sync::Mutex;
        static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
        if let Ok(mut last) = LAST.lock() {
            if last.map(|t| now.duration_since(t).as_secs() >= 60).unwrap_or(true) {
                *last = Some(now);
                log::info!(
                    "map tiles: {visible} visible, {textures} textures, {} waiting, {} requested, {} failed, {} layers",
                    state.waiting_since.len(),
                    state.requested.len(),
                    state.failed.len(),
                    layer_ids.len()
                );
            }
        }
    }

    Ok(())
}

#[tracing::instrument(skip_all)]
fn upload_raster_layers(
    raster_resources: &mut RasterResources,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    tiles: &mut Tiles,
    layers: &[ResolvedRasterLayer],
    view_region: &ViewRegion,
) {
    for coords in view_region.iter() {
        let Some(raster_layers) = tiles.query_mut::<&mut RasterLayersDataComponent>(coords) else {
            continue;
        };

        for layer in layers {
            if raster_resources.has_layer_texture(&layer.layer_id, &coords) {
                continue;
            }

            let Some(position) = raster_layers.layers.iter().position(|data| match data {
                RasterLayerData::Available(data) => data.source_layer == layer.layer_id,
                RasterLayerData::Missing(_) => false,
            }) else {
                continue;
            };

            // CrowdedWX: the CPU copy is released once the texture exists on the GPU.
            let RasterLayerData::Available(data) = raster_layers.layers.swap_remove(position)
            else {
                continue;
            };
            let image = data.image;
            let (width, height) = image.dimensions();

            let texture = raster_resources.create_texture(
                None,
                device,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                width,
                height,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            );

            queue.write_texture(
                wgpu::ImageCopyTexture {
                    aspect: wgpu::TextureAspect::All,
                    texture: &texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                },
                &image,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                texture.size,
            );

            raster_resources.bind_texture(device, &layer.layer_id, &coords, texture);
        }
    }
}
