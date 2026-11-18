# CrowdedWX changes to maplibre-rs

This is a vendored copy of [maplibre-rs](https://github.com/maplibre/maplibre-rs) with additions
CrowdedWXBroadcaster needs. Every change is marked `// CrowdedWX:` in the source and follows the
upstream structure (style spec → systems → render phases), so they can be offered upstream.

## Style sources are honored for raster tiles
- `io/source_type.rs`: `RasterSource::from_template` supports style-spec URL templates
  (`tiles: ["…/{z}/{x}/{y}…"]`) and the source `scheme` (xyz/tms).
- `raster/mod.rs`: `resolve_raster_layers` resolves each raster layer → its `source` → `tiles`,
  `minzoom`/`maxzoom`, `raster-opacity`, in style order.
- `raster/request_system.rs`: tiles are requested per (tile, style layer) instead of from a
  hard-coded MapTiler source. Layers added to the style later are requested on the next frame,
  failed tiles retry after 30 s, and tiles deeper than a source's `maxzoom` are over-zoomed from
  their ancestor tile (as MapLibre GL does).

## Several raster layers per tile
- Textures are bound per (style layer, tile) (`raster/resource/raster.rs`), drawn in style order
  (`raster/queue_system.rs`), and evicted when a layer leaves the style or when far-away tiles
  exceed a budget (`raster/upload_system.rs`).

## `raster-opacity`
- A per-layer uniform (binding 2) feeds `raster.fragment.wgsl`; the raster pipeline now uses alpha
  blending. Layers at opacity 0 stay loaded but are not drawn (the usual way to pre-load
  animation frames).

## Raster colour paint properties
- `raster-brightness-min`, `raster-brightness-max`, `raster-contrast`, `raster-saturation` and
  `raster-hue-rotate` are honored: `raster/mod.rs` converts them to MapLibre GL's shader factors
  (`RasterLayerUniform::from_paint`), the per-layer uniform grew to 32 bytes, and
  `raster.fragment.wgsl` applies hue spin, saturation, contrast and brightness in GL's order.

## Off-screen, camera-driven map
- `headless/offscreen.rs`: `OffscreenMap` runs the regular map loop (camera → tile requests →
  render) on the headless surface and copies every frame into a `FrameSink`, for embedding the
  map in other UI toolkits. `jump_to` / `set_style` mirror MapLibre GL's `jumpTo` / `setStyle`.

## Fixes
- `raster/populate_world_system.rs`: missing tiles were never recorded (the LayerRaster tag was
  checked twice).
- `raster/process_raster.rs`: undecodable tiles no longer panic.
- `io/apc.rs`: `receive` no longer skips a message after `swap_remove`.
- `platform/noweb/mod.rs`: offscreen kernels share one pooled HTTP client per cache directory
  instead of building a new client per tile request.
- `rust-toolchain.toml`: builds on latest stable (the crate's `rust-version` still enforces 1.97.1).

Not changed: the vector pipeline still uses its hard-coded tile source; CrowdedWXBroadcaster uses
raster basemaps.

## Vendored glyph bindings
`maplibre/proto/glyphs.rs` is the prost output for the adjacent glyphs.proto. Normal builds copy these bindings into OUT_DIR. Set CWX_REGENERATE_PROTO=1 and provide protoc when changing the schema; copy the regenerated OUT_DIR/glyphs.rs back into proto/glyphs.rs.
