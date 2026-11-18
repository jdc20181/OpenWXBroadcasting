struct VertexOutput {
    @location(0) tex_coords: vec2<f32>,
    @builtin(position) position: vec4<f32>,
};

// CrowdedWX: per-layer paint properties: raster-opacity, -brightness-min/max, -contrast,
// -saturation and -hue-rotate, pre-converted to MapLibre GL's shader factors. Eight scalars
// (32 bytes) to match `RasterLayerUniform`.
struct RasterLayer {
    opacity: f32,
    brightness_low: f32,
    brightness_high: f32,
    saturation_factor: f32,
    contrast_factor: f32,
    spin_x: f32,
    spin_y: f32,
    spin_z: f32,
};

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;
@group(0) @binding(2)
var<uniform> layer: RasterLayer;

@fragment
fn main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    // Same order as MapLibre GL: hue spin, saturation, contrast, brightness.
    let w = vec3<f32>(layer.spin_x, layer.spin_y, layer.spin_z);
    var rgb = vec3<f32>(dot(color.rgb, w.xyz), dot(color.rgb, w.zxy), dot(color.rgb, w.yzx));
    let average = (color.r + color.g + color.b) / 3.0;
    rgb = rgb + (vec3<f32>(average) - rgb) * layer.saturation_factor;
    rgb = (rgb - vec3<f32>(0.5)) * layer.contrast_factor + vec3<f32>(0.5);
    rgb = mix(vec3<f32>(layer.brightness_low), vec3<f32>(layer.brightness_high), rgb);
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), color.a * layer.opacity);
}
