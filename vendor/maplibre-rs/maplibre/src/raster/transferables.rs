use std::fmt::{Debug, Formatter};

use image::RgbaImage;

use crate::{
    coords::WorldTileCoords,
    io::apc::{IntoMessage, Message, MessageTag},
    raster::{AvailableRasterLayerData, MissingRasterLayerData},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum RasterMessageTag {
    LayerRaster,
    LayerRasterMissing,
}

impl MessageTag for RasterMessageTag {
    fn dyn_clone(&self) -> Box<dyn MessageTag> {
        Box::new(*self)
    }
}

pub trait LayerRaster: IntoMessage + Debug + Send {
    fn message_tag() -> &'static dyn MessageTag;

    fn build_from(coords: WorldTileCoords, layer_name: String, image: RgbaImage) -> Self;

    fn coords(&self) -> WorldTileCoords;

    fn to_layer(self) -> AvailableRasterLayerData;
}

pub trait LayerRasterMissing: IntoMessage + Debug + Send {
    fn message_tag() -> &'static dyn MessageTag;

    /// CrowdedWX: missing tiles are reported per style layer, like available ones.
    fn build_from(coords: WorldTileCoords, layer_name: String) -> Self;

    fn coords(&self) -> WorldTileCoords;

    fn to_layer(self) -> MissingRasterLayerData;
}

pub struct DefaultLayerRaster {
    pub coords: WorldTileCoords,
    pub layer_name: String,
    pub image: RgbaImage,
}

impl Debug for DefaultLayerRaster {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "DefaultRasterLayer({})", self.coords)
    }
}

impl IntoMessage for DefaultLayerRaster {
    fn into(self) -> Message {
        Message::new(Self::message_tag(), Box::new(self))
    }
}

impl LayerRaster for DefaultLayerRaster {
    fn message_tag() -> &'static dyn MessageTag {
        &RasterMessageTag::LayerRaster
    }

    fn build_from(coords: WorldTileCoords, layer_name: String, image: RgbaImage) -> Self {
        Self {
            coords,
            layer_name,
            image,
        }
    }

    fn coords(&self) -> WorldTileCoords {
        self.coords
    }

    fn to_layer(self) -> AvailableRasterLayerData {
        // CrowdedWX: keep the style layer id instead of collapsing every raster into "raster".
        AvailableRasterLayerData {
            coords: self.coords,
            source_layer: self.layer_name,
            image: self.image,
        }
    }
}

pub struct DefaultLayerRasterMissing {
    pub coords: WorldTileCoords,
    pub layer_name: String,
}

impl Debug for DefaultLayerRasterMissing {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "DefaultRasterLayerMissing({})", self.coords)
    }
}

impl IntoMessage for DefaultLayerRasterMissing {
    fn into(self) -> Message {
        Message::new(Self::message_tag(), Box::new(self))
    }
}

impl LayerRasterMissing for DefaultLayerRasterMissing {
    fn message_tag() -> &'static dyn MessageTag {
        &RasterMessageTag::LayerRasterMissing
    }

    fn build_from(coords: WorldTileCoords, layer_name: String) -> Self {
        Self { coords, layer_name }
    }

    fn coords(&self) -> WorldTileCoords {
        self.coords
    }

    fn to_layer(self) -> MissingRasterLayerData {
        MissingRasterLayerData {
            coords: self.coords,
            source_layer: self.layer_name,
        }
    }
}

pub trait RasterTransferables: Copy + Clone + 'static {
    type LayerRaster: LayerRaster;
    type LayerRasterMissing: LayerRasterMissing;
}

#[derive(Copy, Clone)]
pub struct DefaultRasterTransferables;

impl RasterTransferables for DefaultRasterTransferables {
    type LayerRaster = DefaultLayerRaster;
    type LayerRasterMissing = DefaultLayerRasterMissing;
}
