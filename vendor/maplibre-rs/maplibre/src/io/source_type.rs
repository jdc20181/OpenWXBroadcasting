use crate::{coords::WorldTileCoords, style::source::TileAddressingScheme};

/// Represents a source from which the vector tile are fetched.
#[derive(Clone)]
pub struct TessellateSource {
    pub url: String,
    pub filetype: String,
}

impl TessellateSource {
    pub fn new(url: &str, filetype: &str) -> Self {
        Self {
            url: url.to_string(),
            filetype: filetype.to_string(),
        }
    }

    pub fn format(&self, coords: &WorldTileCoords) -> String {
        let tile_coords = coords.into_tile(TileAddressingScheme::XYZ).unwrap();
        format!(
            "{url}/{z}/{x}/{y}.{filetype}",
            url = self.url,
            z = tile_coords.z,
            x = tile_coords.x,
            y = tile_coords.y,
            filetype = self.filetype,
        )
    }
}

impl Default for TessellateSource {
    fn default() -> Self {
        Self::new("https://maps.tuerantuer.org/europe_germany", "pbf")
        // Self::new("https://demotiles.maplibre.org/tiles", "pbf")
    }
}

/// Represents a source from which the raster tile are fetched.
#[derive(Clone)]
pub struct RasterSource {
    pub url: String,
    pub filetype: String,
    pub key: String,
    /// CrowdedWX: a MapLibre style-spec tile URL template (`tiles: ["…/{z}/{x}/{y}.png"]`).
    /// When set, it takes precedence over `url`/`filetype`/`key`.
    pub template: Option<String>,
    /// CrowdedWX: the source's `scheme` (`xyz` or `tms`).
    pub scheme: TileAddressingScheme,
}

impl RasterSource {
    pub fn new(url: &str, filetype: &str, key: &str) -> Self {
        Self {
            url: url.to_string(),
            filetype: filetype.to_string(),
            key: key.to_string(),
            template: None,
            scheme: TileAddressingScheme::XYZ,
        }
    }

    /// CrowdedWX: build a raster source from a style-spec tile URL template, as MapLibre GL does
    /// for `{"type": "raster", "tiles": [...]}` sources.
    pub fn from_template(template: &str, scheme: TileAddressingScheme) -> Self {
        Self {
            url: String::new(),
            filetype: String::new(),
            key: String::new(),
            template: Some(template.to_string()),
            scheme,
        }
    }

    pub fn format(&self, coords: &WorldTileCoords) -> String {
        if let Some(template) = &self.template {
            let tile_coords = coords
                .into_tile(self.scheme.clone())
                .expect("tile coordinates out of bounds");
            return template
                .replace("{z}", &u8::from(tile_coords.z).to_string())
                .replace("{x}", &tile_coords.x.to_string())
                .replace("{y}", &tile_coords.y.to_string());
        }

        let tile_coords = coords.into_tile(TileAddressingScheme::XYZ).unwrap();
        format!(
            "{url}/{z}/{x}/{y}.{filetype}?key={key}",
            url = self.url,
            z = tile_coords.z,
            x = tile_coords.x,
            y = tile_coords.y,
            filetype = self.filetype,
            key = self.key,
        )
    }
}

impl Default for RasterSource {
    fn default() -> Self {
        Self::new(
            "https://api.maptiler.com/tiles/satellite-v2",
            "jpg",
            "qnePkfbGpMsLCi3KFBs3",
        )
    }
}

/// Represents the tiles' different types of source.
#[derive(Clone)]
pub enum SourceType {
    Raster(RasterSource),
    Tessellate(TessellateSource),
}

impl SourceType {
    pub fn format(&self, coords: &WorldTileCoords) -> String {
        match self {
            SourceType::Raster(raster_source) => raster_source.format(coords),
            SourceType::Tessellate(tessellate_source) => tessellate_source.format(coords),
        }
    }
}
