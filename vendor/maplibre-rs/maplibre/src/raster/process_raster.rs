use std::marker::PhantomData;

use image::RgbaImage;
use thiserror::Error;

use crate::{
    coords::WorldTileCoords,
    io::apc::Context,
    raster::transferables::{LayerRaster, RasterTransferables},
};

#[derive(Error, Debug)]
pub enum ProcessRasterError {
    /// Error during processing of the pipeline
    #[error("processing data in pipeline failed")]
    Processing(Box<dyn std::error::Error>),
    /// CrowdedWX: the fetched bytes were not a decodable image.
    #[error("raster tile could not be decoded")]
    Decode(#[from] image::ImageError),
}

pub struct RasterTileRequest {
    pub coords: WorldTileCoords,
    /// CrowdedWX: the style layer this raster belongs to.
    pub layer_id: String,
    /// CrowdedWX: when over-zooming, the `(x, y, divisions)` part of the fetched ancestor tile
    /// that covers `coords`.
    pub sub_tile: Option<(u32, u32, u32)>,
}

/// CrowdedWX: a per-layer pixel filter run on every decoded tile (layer id, pixels), e.g. to
/// repaint radar colours. Set once by the app.
pub type TileFilter = fn(&str, &mut RgbaImage);
static TILE_FILTER: std::sync::OnceLock<TileFilter> = std::sync::OnceLock::new();

/// CrowdedWX: install the tile filter (first call wins).
pub fn set_tile_filter(filter: TileFilter) {
    let _ = TILE_FILTER.set(filter);
}

pub fn process_raster_tile<T: RasterTransferables, C: Context>(
    data: &[u8],
    tile_request: RasterTileRequest,
    context: &mut ProcessRasterContext<T, C>,
) -> Result<(), ProcessRasterError> {
    let coords = &tile_request.coords;
    // CrowdedWX: no unwrap - a bad tile becomes a "missing" tile instead of a panic.
    let img = image::load_from_memory(data)?;
    let mut rgba = img.to_rgba8();
    if let Some(filter) = TILE_FILTER.get() {
        filter(&tile_request.layer_id, &mut rgba);
    }

    if let Some((sub_x, sub_y, divisions)) = tile_request.sub_tile {
        let (width, height) = rgba.dimensions();
        let part_w = (width / divisions).max(1);
        let part_h = (height / divisions).max(1);
        let part = image::imageops::crop_imm(&rgba, sub_x * part_w, sub_y * part_h, part_w, part_h)
            .to_image();
        rgba = image::imageops::resize(&part, width, height, image::imageops::FilterType::Triangle);
    }

    context.layer_raster_finished(coords, tile_request.layer_id, rgba)?;

    Ok(())
}

pub struct ProcessRasterContext<T: RasterTransferables, C: Context> {
    context: C,
    phantom_t: PhantomData<T>,
}

impl<T: RasterTransferables, C: Context> ProcessRasterContext<T, C> {
    pub fn new(context: C) -> Self {
        Self {
            context,
            phantom_t: Default::default(),
        }
    }
}

impl<T: RasterTransferables, C: Context> ProcessRasterContext<T, C> {
    fn layer_raster_finished(
        &mut self,
        coords: &WorldTileCoords,
        layer_name: String,
        image_data: RgbaImage,
    ) -> Result<(), ProcessRasterError> {
        self.context
            .send_back(T::LayerRaster::build_from(*coords, layer_name, image_data))
            .map_err(|e| ProcessRasterError::Processing(Box::new(e)))
    }
}

#[cfg(test)]
mod tests {
    use super::process_raster_tile;
    use crate::{
        coords::ZoomLevel,
        io::apc::tests::DummyContext,
        raster::{
            process_raster::{ProcessRasterContext, RasterTileRequest},
            DefaultRasterTransferables,
        },
    };

    #[test] // TODO: Add proper tile byte array
    #[ignore]
    fn test() {
        let _output = process_raster_tile(
            &[0],
            RasterTileRequest {
                coords: (0, 0, ZoomLevel::default()).into(),
                layer_id: "raster".to_string(),
                sub_tile: None,
            },
            &mut ProcessRasterContext::<DefaultRasterTransferables, _>::new(DummyContext),
        );
    }
}
