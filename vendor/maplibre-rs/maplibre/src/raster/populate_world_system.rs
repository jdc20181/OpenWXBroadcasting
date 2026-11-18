use std::{borrow::Cow, marker::PhantomData, rc::Rc, time::Instant};

use crate::{
    context::MapContext,
    environment::Environment,
    io::apc::{AsyncProcedureCall, Message},
    kernel::Kernel,
    raster::{
        transferables::{LayerRaster, LayerRasterMissing, RasterTransferables},
        RasterLayerData, RasterLayersDataComponent, RasterRequestState,
    },
    tcs::system::{System, SystemResult},
};

pub struct PopulateWorldSystem<E: Environment, T> {
    kernel: Rc<Kernel<E>>,
    phantom_t: PhantomData<T>,
}

impl<E: Environment, T> PopulateWorldSystem<E, T> {
    pub fn new(kernel: &Rc<Kernel<E>>) -> Self {
        Self {
            kernel: kernel.clone(),
            phantom_t: Default::default(),
        }
    }
}

impl<E: Environment, T: RasterTransferables> System for PopulateWorldSystem<E, T> {
    fn name(&self) -> Cow<'static, str> {
        "populate_world_system".into()
    }

    fn run(&mut self, MapContext { world, .. }: &mut MapContext) -> SystemResult {
        for message in self.kernel.apc().receive(|message| {
            message.has_tag(T::LayerRaster::message_tag())
                || message.has_tag(T::LayerRasterMissing::message_tag())
        }) {
            let message: Message = message;
            if message.has_tag(T::LayerRaster::message_tag()) {
                let message = message.into_transferable::<T::LayerRaster>();
                let Some(component) = world
                    .tiles
                    .query_mut::<&mut RasterLayersDataComponent>(message.coords())
                else {
                    continue;
                };

                let layer = message.to_layer();
                // CrowdedWX: a re-fetched layer replaces its previous data for this tile.
                component.layers.retain(|existing| match existing {
                    RasterLayerData::Available(data) => data.source_layer != layer.source_layer,
                    RasterLayerData::Missing(data) => data.source_layer != layer.source_layer,
                });
                component.layers.push(RasterLayerData::Available(layer));
            } else if message.has_tag(T::LayerRasterMissing::message_tag()) {
                // CrowdedWX: upstream checked the LayerRaster tag twice here, so missing tiles
                // were never recorded. Record them and remember the failure for a later retry.
                let message = message.into_transferable::<T::LayerRasterMissing>();
                let coords = message.coords();
                let missing = message.to_layer();

                world
                    .resources
                    .get_or_init_mut::<RasterRequestState>()
                    .failed
                    .insert((coords, missing.source_layer.clone()), Instant::now());

                let Some(component) = world
                    .tiles
                    .query_mut::<&mut RasterLayersDataComponent>(coords)
                else {
                    continue;
                };

                component.layers.retain(|existing| match existing {
                    RasterLayerData::Missing(data) => data.source_layer != missing.source_layer,
                    RasterLayerData::Available(_) => true,
                });
                component.layers.push(RasterLayerData::Missing(missing));
            }
        }

        Ok(())
    }
}
