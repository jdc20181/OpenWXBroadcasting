use std::collections::{HashMap, HashSet};

use crate::{
    coords::WorldTileCoords,
    render::{resource::Texture, settings::Msaa, tile_view_pattern::HasTile},
    tcs::world::World,
};

/// CrowdedWX: per-layer uniform consumed by `raster.fragment.wgsl`: opacity and the raster
/// colour adjustments as MapLibre GL's shader factors (32 bytes).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck_derive::Pod, bytemuck_derive::Zeroable)]
pub struct RasterLayerUniform {
    pub opacity: f32,
    pub brightness_low: f32,
    pub brightness_high: f32,
    pub saturation_factor: f32,
    pub contrast_factor: f32,
    pub spin: [f32; 3],
}

impl RasterLayerUniform {
    /// From style-spec values (MapLibre GL's raster painter conversions).
    pub fn from_paint(opacity: f32, brightness_min: f32, brightness_max: f32, contrast: f32, saturation: f32, hue_rotate_deg: f32) -> Self {
        let saturation_factor = if saturation > 0.0 { 1.0 - 1.0 / (1.001 - saturation) } else { -saturation };
        let contrast_factor = if contrast > 0.0 { 1.0 / (1.0 - contrast) } else { 1.0 + contrast };
        let angle = hue_rotate_deg.to_radians();
        let (s, c) = angle.sin_cos();
        let r3 = 3f32.sqrt();
        Self {
            opacity,
            brightness_low: brightness_min,
            brightness_high: brightness_max,
            saturation_factor,
            contrast_factor,
            spin: [(2.0 * c + 1.0) / 3.0, (-r3 * s - c + 1.0) / 3.0, (r3 * s - c + 1.0) / 3.0],
        }
    }
}

/// Holds the resources necessary for the raster tiles such as the
/// * sampler
/// * texture
/// * pipeline
/// * bindgroups
///
/// CrowdedWX: textures are bound per (style layer id, tile) so several raster layers can be
/// stacked on one tile (basemap, radar frames, labels), each with its own `raster-opacity`.
pub struct RasterResources {
    sampler: wgpu::Sampler,
    msaa: Msaa,
    pipeline: wgpu::RenderPipeline,
    bound_textures: HashMap<(String, WorldTileCoords), wgpu::BindGroup>,
    layer_uniforms: HashMap<String, wgpu::Buffer>,
}

impl RasterResources {
    pub fn new(msaa: Msaa, device: &wgpu::Device, pipeline: wgpu::RenderPipeline) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            sampler,
            msaa,
            pipeline,
            bound_textures: Default::default(),
            layer_uniforms: Default::default(),
        }
    }

    pub fn create_texture(
        &mut self,
        label: wgpu::Label,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        usage: wgpu::TextureUsages,
    ) -> Texture {
        Texture::new(label, device, format, width, height, self.msaa, usage)
    }

    pub fn get_bound_texture(
        &self,
        layer_id: &str,
        coords: &WorldTileCoords,
    ) -> Option<&wgpu::BindGroup> {
        self.bound_textures.get(&(layer_id.to_string(), *coords))
    }

    pub fn has_layer_texture(&self, layer_id: &str, coords: &WorldTileCoords) -> bool {
        self.bound_textures
            .contains_key(&(layer_id.to_string(), *coords))
    }

    /// CrowdedWX: create or update the paint uniform (opacity and colour adjustments) of a
    /// style layer.
    pub fn set_layer_opacity(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layer_id: &str,
        uniform: RasterLayerUniform,
    ) {
        if let Some(buffer) = self.layer_uniforms.get(layer_id) {
            queue.write_buffer(buffer, 0, bytemuck::bytes_of(&uniform));
            return;
        }
        use wgpu::util::DeviceExt;
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("raster_layer_uniform"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        self.layer_uniforms.insert(layer_id.to_string(), buffer);
    }

    /// Creates a bind group for each fetched raster tile and store it inside a hashmap.
    ///
    /// CrowdedWX: the layer's opacity uniform must exist (see [`Self::set_layer_opacity`]).
    pub fn bind_texture(
        &mut self,
        device: &wgpu::Device,
        layer_id: &str,
        coords: &WorldTileCoords,
        texture: Texture,
    ) {
        let Some(uniform) = self.layer_uniforms.get(layer_id) else {
            log::warn!("raster layer {layer_id} has no opacity uniform yet");
            return;
        };
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
            label: None,
        });
        self.bound_textures
            .insert((layer_id.to_string(), *coords), bind_group);
    }

    /// CrowdedWX: drop GPU data of layers that left the style (e.g. an expired radar frame).
    pub fn retain_layers(&mut self, layer_ids: &HashSet<String>) {
        self.bound_textures
            .retain(|(layer_id, _), _| layer_ids.contains(layer_id));
        self.layer_uniforms
            .retain(|layer_id, _| layer_ids.contains(layer_id));
    }

    /// CrowdedWX: drop textures of tiles outside `keep`, returning the evicted pairs so the
    /// request bookkeeping can forget them.
    pub fn evict_tiles_outside(
        &mut self,
        keep: &HashSet<WorldTileCoords>,
    ) -> Vec<(String, WorldTileCoords)> {
        let evicted: Vec<(String, WorldTileCoords)> = self
            .bound_textures
            .keys()
            .filter(|(_, coords)| !keep.contains(coords))
            .cloned()
            .collect();
        for key in &evicted {
            self.bound_textures.remove(key);
        }
        evicted
    }

    pub fn texture_count(&self) -> usize {
        self.bound_textures.len()
    }

    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }
}

impl HasTile for RasterResources {
    fn has_tile(&self, coords: WorldTileCoords, _world: &World) -> bool {
        self.bound_textures
            .keys()
            .any(|(_, bound_coords)| *bound_coords == coords)
    }
}
