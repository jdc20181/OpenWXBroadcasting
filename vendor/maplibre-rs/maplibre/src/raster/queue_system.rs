use crate::{
    context::MapContext,
    raster::{render_commands::DrawRasterTiles, resolve_raster_layers, resource::RasterResources},
    render::{
        eventually::{Eventually, Eventually::Initialized},
        render_commands::DrawMasks,
        render_phase::{DrawState, LayerItem, RenderPhase, TileMaskItem},
        tile_view_pattern::WgpuTileViewPattern,
    },
    tcs::{
        system::{SystemError, SystemResult},
        tiles::Tile,
    },
};

/// CrowdedWX: queues one draw per (tile, raster style layer), ordered by the layer's position in
/// the style, and skips layers whose `raster-opacity` is 0 (still loaded, just not drawn).
pub fn queue_system(MapContext { world, style, .. }: &mut MapContext) -> SystemResult {
    let Some((Initialized(tile_view_pattern), Initialized(raster_resources))) = world
        .resources
        .query::<(
            &Eventually<WgpuTileViewPattern>,
            &Eventually<RasterResources>,
        )>()
    else {
        return Err(SystemError::Dependencies);
    };

    let layers = resolve_raster_layers(style);

    let mut items = Vec::new();
    let mut masks = Vec::new();

    for view_tile in tile_view_pattern.iter() {
        let coords = &view_tile.coords();
        tracing::trace!("Drawing tile at {coords}");

        view_tile.render(|source_shape| {
            let source_coords = source_shape.coords();
            let mut drew_any = false;

            for layer in &layers {
                if layer.opacity <= 0.0 {
                    continue;
                }
                if !raster_resources.has_layer_texture(&layer.layer_id, &source_coords) {
                    continue;
                }
                drew_any = true;
                items.push(LayerItem {
                    draw_function: Box::new(DrawState::<LayerItem, DrawRasterTiles>::new()),
                    index: layer.index,
                    is_line: false,
                    style_layer: layer.layer_id.clone(),
                    tile: Tile {
                        coords: source_coords,
                    },
                    source_shape: source_shape.clone(),
                });
            }

            if drew_any {
                masks.push(TileMaskItem {
                    draw_function: Box::new(DrawState::<TileMaskItem, DrawMasks>::new()),
                    source_shape: source_shape.clone(),
                });
            }
        });
    }

    let Some((layer_item_phase, tile_mask_phase)) = world
        .resources
        .query_mut::<(&mut RenderPhase<LayerItem>, &mut RenderPhase<TileMaskItem>)>()
    else {
        return Err(SystemError::Dependencies);
    };

    for item in items {
        layer_item_phase.add(item);
    }
    for mask in masks {
        tile_mask_phase.add(mask);
    }

    Ok(())
}
