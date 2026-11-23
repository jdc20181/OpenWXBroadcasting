//! Weather data files: automatic format detection, metadata inspection, and decoding of GRIB2
//! (grib crate), NetCDF 3 / 4 (netcdf-reader), HDF5 (hdf5-reader) and BUFR (our decoder with the
//! WMO tables). A decoded field can be regridded (nearest, bilinear, inverse distance) onto a
//! regular grid over the coverage area, run through a transformation pipeline, and saved as a
//! map layer with a provenance record (source file, format, checksum, steps).

pub mod bufr;
mod hdf;
mod spatial;

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Grib2,
    Grib1,
    NetCdf3,
    NetCdf4,
    Hdf5,
    Bufr,
    GeoJson,
    Csv,
    Shapefile,
    Unknown,
}

impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Format::Grib2 => "GRIB2",
            Format::Grib1 => "GRIB1",
            Format::NetCdf3 => "NetCDF-3 (classic)",
            Format::NetCdf4 => "NetCDF-4 (HDF5)",
            Format::Hdf5 => "HDF5",
            Format::Bufr => "BUFR",
            Format::GeoJson => "GeoJSON",
            Format::Csv => "CSV",
            Format::Shapefile => "Shapefile",
            Format::Unknown => "Unknown",
        }
    }
}

/// Detect the format from the file's first bytes (falling back to the extension).
pub fn detect(path: &Path) -> Format {
    use std::io::Read;
    let mut head = vec![0u8; 1024];
    let n = std::fs::File::open(path)
        .and_then(|mut f| f.read(&mut head))
        .unwrap_or(0);
    head.truncate(n);
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if head.starts_with(b"GRIB") {
        return if head.get(7) == Some(&1) {
            Format::Grib1
        } else {
            Format::Grib2
        };
    }
    if head.starts_with(b"CDF\x01") || head.starts_with(b"CDF\x02") || head.starts_with(b"CDF\x05")
    {
        return Format::NetCdf3;
    }
    let hdf = b"\x89HDF\r\n\x1a\n";
    if [0usize, 512]
        .iter()
        .any(|o| head.get(*o..*o + 8) == Some(&hdf[..]))
    {
        return if ext == "nc"
            || ext == "nc4"
            || ext == "netcdf"
            || contains(&head, b"_NCProperties")
        {
            Format::NetCdf4
        } else {
            Format::Hdf5
        };
    }
    if contains(&head[..head.len().min(64)], b"BUFR") {
        return Format::Bufr;
    }
    if head.len() >= 4 && head[0..4] == [0x00, 0x00, 0x27, 0x0A] {
        return Format::Shapefile;
    }
    if head.starts_with(b"PK") && ext == "zip" {
        return Format::Shapefile;
    }
    let text = String::from_utf8_lossy(&head);
    let t = text.trim_start();
    if t.starts_with('{') && (t.contains("\"Feature") || ext == "geojson" || ext == "json") {
        return Format::GeoJson;
    }
    if ext == "csv"
        || (t
            .lines()
            .next()
            .map(|l| l.matches(',').count() >= 2)
            .unwrap_or(false)
            && !t.contains('\0'))
    {
        return Format::Csv;
    }
    Format::Unknown
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[derive(Clone, Debug, Default)]
pub struct VarInfo {
    pub index: usize,
    pub name: String,
    pub description: String,
    pub shape: String,
    pub units: String,
    /// Can be decoded to a lat / lon field.
    pub mappable: bool,
}

#[derive(Clone, Debug)]
pub struct Inventory {
    pub format: Format,
    pub size: u64,
    pub summary: Vec<String>,
    pub attributes: Vec<(String, String)>,
    pub variables: Vec<VarInfo>,
}

/// Metadata of a file: format, global attributes and every variable / message.
pub fn inspect(path: &Path) -> Result<Inventory> {
    check_file(path)?;
    let format = detect(path);
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let mut inv = Inventory {
        format,
        size,
        summary: vec![format!(
            "{} • {:.1} MB",
            format.label(),
            size as f64 / 1_048_576.0
        )],
        attributes: Vec::new(),
        variables: Vec::new(),
    };
    match format {
        Format::Grib2 => {
            let grib2 = grib::from_reader(std::io::BufReader::new(std::fs::File::open(path)?))
                .map_err(|e| anyhow!("GRIB2: {e}"))?;
            for (i, (index, sub)) in grib2.iter().enumerate() {
                let discipline = sub.indicator().discipline;
                let name = match (
                    sub.prod_def().parameter_category(),
                    sub.prod_def().parameter_number(),
                ) {
                    (Some(cat), Some(num)) => {
                        use grib::codetables::{CodeTable4_2, Lookup};
                        CodeTable4_2::new(discipline, cat)
                            .lookup(usize::from(num))
                            .to_string()
                    }
                    _ => "unknown parameter".into(),
                };
                let ft = sub
                    .prod_def()
                    .forecast_time()
                    .map(|f| f.to_string())
                    .unwrap_or_default();
                let level = sub
                    .prod_def()
                    .fixed_surfaces()
                    .map(|(a, _)| format!("{} {}", a.value(), a.unit().unwrap_or("")))
                    .unwrap_or_default();
                let points = sub.grid_def().num_points();
                inv.variables.push(VarInfo {
                    index: i,
                    name: format!("{name}"),
                    description: format!("message {}.{} • {ft} • {level}", index.0, index.1),
                    shape: format!("{points} points"),
                    units: String::new(),
                    mappable: true,
                });
            }
            inv.summary
                .push(format!("{} messages", inv.variables.len()));
        }
        Format::Grib1 => {
            inv.summary.push(
                "GRIB edition 1: listing only (convert to GRIB2 with wgrib2 / cdo to decode)"
                    .into(),
            );
            inv.variables = grib1_list(path)?;
        }
        Format::NetCdf3 | Format::NetCdf4 => {
            let f = netcdf_reader::NcFile::open(path).map_err(|e| anyhow!("NetCDF: {e}"))?;
            inv.summary.push(format!("{:?}", f.format()));
            for a in f.global_attributes().map_err(|e| anyhow!("{e}"))? {
                inv.attributes.push((
                    a.name.clone(),
                    a.value
                        .as_string()
                        .or_else(|| a.value.as_f64().map(|v| v.to_string()))
                        .unwrap_or_default(),
                ));
            }
            let dims: Vec<String> = f
                .dimensions()
                .map_err(|e| anyhow!("{e}"))?
                .iter()
                .map(|d| {
                    format!(
                        "{}={}{}",
                        d.name,
                        d.size,
                        if d.is_unlimited { " (unlimited)" } else { "" }
                    )
                })
                .collect();
            inv.summary.push(format!("dimensions: {}", dims.join(", ")));
            for (i, v) in f
                .variables()
                .map_err(|e| anyhow!("{e}"))?
                .iter()
                .enumerate()
            {
                let attr = |k: &str| {
                    v.attributes
                        .iter()
                        .find(|a| a.name == k)
                        .and_then(|a| a.value.as_string())
                        .unwrap_or_default()
                };
                let dn: Vec<String> = v
                    .dimensions()
                    .iter()
                    .map(|d| d.name.to_lowercase())
                    .collect();
                let mappable = dn.iter().any(|d| d.starts_with("lat") || d == "y")
                    && dn.iter().any(|d| d.starts_with("lon") || d == "x");
                inv.variables.push(VarInfo {
                    index: i,
                    name: v.name().to_string(),
                    description: if attr("long_name").is_empty() {
                        attr("standard_name")
                    } else {
                        attr("long_name")
                    },
                    shape: format!("{:?} {:?}", v.dtype(), v.shape()),
                    units: attr("units"),
                    mappable,
                });
            }
        }
        Format::Hdf5 => {
            let f = hdf5_reader::Hdf5File::open(path).map_err(|e| anyhow!("HDF5: {e}"))?;
            let root = f.root_group().map_err(|e| anyhow!("{e}"))?;
            for a in root.attributes().unwrap_or_default() {
                inv.attributes.push((
                    a.name.clone(),
                    a.read_string()
                        .ok()
                        .or_else(|| a.read_as_f64().ok().map(|v| v.to_string()))
                        .unwrap_or_default(),
                ));
            }
            let mut list = Vec::new();
            walk_hdf5(&root, "", &mut list, 0);
            for (i, (name, shape)) in list.into_iter().enumerate() {
                inv.variables.push(VarInfo {
                    index: i,
                    mappable: hdf::mappable(&f, &name),
                    name,
                    description: String::new(),
                    shape,
                    units: String::new(),
                });
            }
            inv.summary
                .push(format!("{} datasets", inv.variables.len()));
        }
        Format::Bufr => {
            let bytes = std::fs::read(path)?;
            let msgs = bufr::decode_all(&bytes);
            inv.summary.push(format!("{} BUFR messages", msgs.len()));
            for (i, m) in msgs.iter().enumerate().take(500) {
                inv.variables.push(VarInfo {
                    index: i,
                    name: format!(
                        "Message {} • category {} ({})",
                        i + 1,
                        m.category,
                        bufr::category_name(m.category)
                    ),
                    description: format!(
                        "edition {} • centre {} • {} • {} subsets{}",
                        m.edition,
                        m.centre,
                        m.time,
                        m.subsets.len(),
                        if m.error.is_empty() {
                            String::new()
                        } else {
                            format!(" • {}", m.error)
                        }
                    ),
                    shape: format!(
                        "{} values per subset",
                        m.subsets.first().map(|s| s.len()).unwrap_or(0)
                    ),
                    units: String::new(),
                    mappable: m.subsets.iter().any(|s| bufr::lat_lon(s).is_some()),
                });
            }
        }
        Format::GeoJson | Format::Csv | Format::Shapefile => {
            let ds =
                crate::customdata::load(path).ok_or_else(|| anyhow!("Couldn't read the file"))?;
            inv.summary.push(format!(
                "{} features • fields: {}",
                ds.features.len(),
                ds.fields.join(", ")
            ));
            for (i, f) in ds.numeric_fields.iter().enumerate() {
                inv.variables.push(VarInfo {
                    index: i,
                    name: f.clone(),
                    description: "numeric field".into(),
                    shape: format!("{} features", ds.features.len()),
                    units: String::new(),
                    mappable: true,
                });
            }
        }
        Format::Unknown => return Err(anyhow!("Unrecognised format")),
    }
    Ok(inv)
}

fn walk_hdf5(
    g: &hdf5_reader::group::Group,
    prefix: &str,
    out: &mut Vec<(String, String)>,
    depth: usize,
) {
    if depth > 8 || out.len() > 2000 {
        return;
    }
    if let Ok((groups, datasets)) = g.members() {
        for d in datasets {
            out.push((
                format!(
                    "{prefix}/{}",
                    d.name().rsplit('/').next().unwrap_or(d.name())
                ),
                format!("{:?}", d.shape()),
            ));
        }
        for sub in groups {
            let name = format!(
                "{prefix}/{}",
                sub.name().rsplit('/').next().unwrap_or(sub.name())
            );
            walk_hdf5(&sub, &name, out, depth + 1);
        }
    }
}

/// GRIB1: walk the messages and list parameter / level / time from section 1 (no decoding).
fn grib1_list(path: &Path) -> Result<Vec<VarInfo>> {
    let b = std::fs::read(path)?;
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 8 < b.len() && out.len() < 2000 {
        if &b[i..i + 4] != b"GRIB" {
            i += 1;
            continue;
        }
        let len = ((b[i + 4] as usize) << 16) | ((b[i + 5] as usize) << 8) | b[i + 6] as usize;
        let s1 = i + 8;
        if s1 + 28 > b.len() || len < 28 {
            break;
        }
        let param = b[s1 + 8];
        let level_type = b[s1 + 9];
        let level = ((b[s1 + 10] as u16) << 8) | b[s1 + 11] as u16;
        let (yy, mm, dd, hh) = (b[s1 + 12], b[s1 + 13], b[s1 + 14], b[s1 + 15]);
        let p1 = b[s1 + 18];
        out.push(VarInfo {
            index: out.len(),
            name: format!("parameter {param}"),
            description: format!(
                "{yy:02}-{mm:02}-{dd:02} {hh:02}Z +{p1} • level type {level_type} value {level}"
            ),
            shape: format!("{len} bytes"),
            units: String::new(),
            mappable: false,
        });
        i += len.max(8);
    }
    Ok(out)
}

/// Decoded points of one variable / message: (lat, lon, value).
#[derive(Clone, Debug, Default)]
pub struct Points {
    pub name: String,
    pub units: String,
    pub values: Vec<(f64, f64, f32)>,
}

/// Decode a variable / message to points (lat, lon, value), keeping those inside `bbox`
/// (south, west, north, east) when given.
pub fn decode(path: &Path, index: usize, bbox: Option<(f64, f64, f64, f64)>) -> Result<Points> {
    check_file(path)?;
    let inside = |lat: f64, lon: f64| {
        bbox.map(|(s, w, n, e)| lat >= s && lat <= n && lon >= w && lon <= e)
            .unwrap_or(true)
    };
    let norm = |lon: f64| if lon > 180.0 { lon - 360.0 } else { lon };
    match detect(path) {
        Format::Grib2 => {
            let grib2 = grib::from_reader(std::io::BufReader::new(std::fs::File::open(path)?))
                .map_err(|e| anyhow!("GRIB2: {e}"))?;
            let (_, sub) = grib2
                .iter()
                .nth(index)
                .ok_or_else(|| anyhow!("no message {index}"))?;
            use grib::LatLons;
            let latlons = sub.latlons().map_err(|e| anyhow!("GRIB2 grid: {e}"))?;
            let decoder =
                grib::Grib2SubmessageDecoder::from(sub).map_err(|e| anyhow!("GRIB2: {e}"))?;
            let values = decoder
                .dispatch()
                .map_err(|e| anyhow!("GRIB2 decode: {e}"))?;
            let pts = latlons
                .zip(values)
                .filter_map(|((lat, lon), v)| {
                    let (lat, lon) = (lat as f64, norm(lon as f64));
                    (v.is_finite() && inside(lat, lon)).then_some((lat, lon, v))
                })
                .collect();
            Ok(Points {
                name: format!("GRIB2 message {}", index + 1),
                units: String::new(),
                values: pts,
            })
        }
        Format::Hdf5 => {
            let inventory = inspect(path)?;
            let variable = inventory
                .variables
                .get(index)
                .ok_or_else(|| anyhow!("no dataset {index}"))?;
            hdf::decode(path, &variable.name, bbox)
        }
        Format::NetCdf3 | Format::NetCdf4 => {
            let f = netcdf_reader::NcFile::open(path).map_err(|e| anyhow!("NetCDF: {e}"))?;
            let vars = f.variables().map_err(|e| anyhow!("{e}"))?;
            let v = vars
                .get(index)
                .ok_or_else(|| anyhow!("no variable {index}"))?;
            let name = v.name().to_string();
            let units = v
                .attributes
                .iter()
                .find(|a| a.name == "units")
                .and_then(|a| a.value.as_string())
                .unwrap_or_default();
            let dims: Vec<String> = v.dimensions().iter().map(|d| d.name.clone()).collect();
            let shape = v.shape();
            let total = shape
                .iter()
                .try_fold(1u64, |n, d| n.checked_mul(*d))
                .ok_or_else(|| anyhow!("NetCDF dimensions overflow"))?;
            anyhow::ensure!(
                total <= 20_000_000,
                "NetCDF variable exceeds 20 million values; subset it first"
            );
            let lat_dim = dims
                .iter()
                .position(|d| d.to_lowercase().starts_with("lat") || d.to_lowercase() == "y")
                .ok_or_else(|| anyhow!("{name} has no latitude dimension"))?;
            let lon_dim = dims
                .iter()
                .position(|d| d.to_lowercase().starts_with("lon") || d.to_lowercase() == "x")
                .ok_or_else(|| anyhow!("{name} has no longitude dimension"))?;
            let coordinate = |axis: &str, dim: &str| -> Result<Vec<f64>> {
                let standard = if axis == "lat" {
                    "latitude"
                } else {
                    "longitude"
                };
                let candidate = vars.iter().find(|v| v.attributes.iter().any(|a| a.name == "standard_name" && a.value.as_string().as_deref() == Some(standard)))
                    .or_else(|| vars.iter().find(|v| v.name() == standard || v.name() == axis))
                    .or_else(|| vars.iter().find(|v| v.name() == dim && v.attributes.iter().any(|a| a.name == "units" && a.value.as_string().map(|u| u.contains("degree")).unwrap_or(false))))
                    .ok_or_else(|| anyhow!("No geographic {standard} coordinate; projected x/y requires geographic coordinates"))?;
                nc_values(&f, candidate)
            };
            let lats = coordinate("lat", &dims[lat_dim])?;
            let lons = coordinate("lon", &dims[lon_dim])?;
            let flat = nc_values(&f, v)?;
            // First slice of every leading dimension (time, level): take the last two dims.
            let (ny, nx) = (shape[lat_dim] as usize, shape[lon_dim] as usize);
            let rectilinear = lats.len() == ny && lons.len() == nx;
            let curvilinear = lat_dim < lon_dim && lats.len() == nx * ny && lons.len() == nx * ny;
            anyhow::ensure!(
                rectilinear || curvilinear,
                "Geographic coordinate shapes do not match the field"
            );
            let mut pts = Vec::new();
            for j in 0..ny {
                for i in 0..nx {
                    // Every non-spatial dimension is explicitly sliced at index zero.
                    // Honor actual strides even for [lat,time,lon] or [lon,lat,level].
                    let stride = |dim: usize| {
                        shape[dim + 1..]
                            .iter()
                            .map(|v| *v as usize)
                            .product::<usize>()
                    };
                    let k = j * stride(lat_dim) + i * stride(lon_dim);
                    let Some(v) = flat.get(k) else { continue };
                    let (lat, lon) = (
                        lats[if rectilinear { j } else { j * nx + i }],
                        norm(lons[if rectilinear { i } else { j * nx + i }]),
                    );
                    if v.is_finite()
                        && lat.is_finite()
                        && lat.abs() <= 90.0
                        && lon.abs() <= 180.0
                        && inside(lat, lon)
                    {
                        pts.push((lat, lon, *v as f32));
                    }
                }
            }
            Ok(Points {
                name,
                units,
                values: pts,
            })
        }
        Format::Bufr => {
            let bytes = std::fs::read(path)?;
            let msgs = bufr::decode_all(&bytes);
            let m = msgs
                .get(index)
                .ok_or_else(|| anyhow!("no message {index}"))?;
            anyhow::ensure!(m.error.is_empty(), "BUFR: {}", m.error);
            let element =
                bufr::main_element(m).ok_or_else(|| anyhow!("no numeric element to map"))?;
            let pts = m
                .subsets
                .iter()
                .filter_map(|s| {
                    let (lat, lon) = bufr::lat_lon(s)?;
                    let v = s
                        .iter()
                        .find(|x| x.fxy == element.0)
                        .and_then(|x| x.value)?;
                    inside(lat, lon).then_some((lat, lon, v as f32))
                })
                .collect();
            Ok(Points {
                name: element.1,
                units: element.2,
                values: pts,
            })
        }
        Format::GeoJson | Format::Csv | Format::Shapefile => {
            let ds =
                crate::customdata::load(path).ok_or_else(|| anyhow!("Couldn't read the file"))?;
            let field = ds
                .numeric_fields
                .get(index)
                .cloned()
                .ok_or_else(|| anyhow!("no field {index}"))?;
            let pts = ds
                .features
                .iter()
                .filter_map(|f| {
                    let (lat, lon) = f.point?;
                    let v: f32 = f.props.get(&field)?.parse().ok()?;
                    inside(lat, lon).then_some((lat, lon, v))
                })
                .collect();
            Ok(Points {
                name: field,
                units: String::new(),
                values: pts,
            })
        }
        f => Err(anyhow!("{} can't be decoded to a map field", f.label())),
    }
}

/// A regular grid over (south, west, north, east).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Grid2D {
    pub bbox: (f64, f64, f64, f64),
    pub nx: usize,
    pub ny: usize,
    /// Row-major from the north-west corner; NaN = no data.
    pub values: Vec<f32>,
}

impl Grid2D {
    pub fn latlon(&self, i: usize, j: usize) -> (f64, f64) {
        let (s, w, n, e) = self.bbox;
        (
            n - (n - s) * j as f64 / (self.ny.max(2) - 1) as f64,
            w + (e - w) * i as f64 / (self.nx.max(2) - 1) as f64,
        )
    }
}

/// Regrid points: nearest neighbour, true bilinear on rectilinear coordinates, or IDW (8 nearest).
pub fn regrid(
    p: &Points,
    bbox: (f64, f64, f64, f64),
    nx: usize,
    ny: usize,
    method: &str,
) -> Grid2D {
    spatial::run(p, bbox, nx, ny, method)
}

/// Transformation pipeline steps, one per line: "scale 1.8", "offset 32", "k_to_f", "c_to_f",
/// "mm_to_in", "ms_to_mph", "pa_to_mb", "clip 0 100", "smooth 2", "mask_below 0.01", "abs".
pub fn apply_pipeline(g: &mut Grid2D, steps: &[String]) -> Vec<String> {
    let mut log = Vec::new();
    for raw in steps {
        let parts: Vec<&str> = raw.split_whitespace().collect();
        let Some(op) = parts.first() else { continue };
        let a: f32 = parts.get(1).and_then(|x| x.parse().ok()).unwrap_or(0.0);
        let b: f32 = parts.get(2).and_then(|x| x.parse().ok()).unwrap_or(0.0);
        let map = |g: &mut Grid2D, f: &dyn Fn(f32) -> f32| {
            for v in g.values.iter_mut().filter(|v| v.is_finite()) {
                *v = f(*v);
            }
        };
        match *op {
            "scale" => map(g, &|v| v * a),
            "offset" => map(g, &|v| v + a),
            "k_to_f" => map(g, &|v| (v - 273.15) * 1.8 + 32.0),
            "c_to_f" => map(g, &|v| v * 1.8 + 32.0),
            "mm_to_in" => map(g, &|v| v / 25.4),
            "ms_to_mph" => map(g, &|v| v * 2.23694),
            "pa_to_mb" => map(g, &|v| v / 100.0),
            "abs" => map(g, &|v| v.abs()),
            "clip" => map(g, &|v| v.clamp(a.min(b), a.max(b))),
            "mask_below" => {
                for v in g.values.iter_mut() {
                    if *v < a {
                        *v = f32::NAN;
                    }
                }
            }
            "smooth" => {
                for _ in 0..(a.max(1.0) as usize).min(10) {
                    let src = g.values.clone();
                    for j in 0..g.ny {
                        for i in 0..g.nx {
                            let (mut s, mut n) = (0.0, 0.0);
                            for dj in -1i64..=1 {
                                for di in -1i64..=1 {
                                    let (x, y) = (i as i64 + di, j as i64 + dj);
                                    if x < 0 || y < 0 || x >= g.nx as i64 || y >= g.ny as i64 {
                                        continue;
                                    }
                                    let v = src[y as usize * g.nx + x as usize];
                                    if v.is_finite() {
                                        s += v;
                                        n += 1.0;
                                    }
                                }
                            }
                            if n > 0.0 && src[j * g.nx + i].is_finite() {
                                g.values[j * g.nx + i] = s / n;
                            }
                        }
                    }
                }
            }
            other => {
                log.push(format!("skipped unknown step \"{other}\""));
                continue;
            }
        }
        log.push(raw.trim().to_string());
    }
    log
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Provenance {
    pub name: String,
    pub source_file: String,
    pub format: String,
    pub variable: String,
    pub units: String,
    pub size_bytes: u64,
    /// SHA-256 of the source file (older records may contain FNV-1a).
    pub checksum: String,
    pub imported: i64,
    pub regrid: String,
    pub steps: Vec<String>,
    pub output: String,
}

fn checksum(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("sha256:{}", hash.finalize().iter().map(|b| format!("{b:02x}")).collect::<String>()))
}

pub fn data_dir() -> PathBuf {
    crate::config::LibraryPaths::get().root.join("Data")
}

/// Write the grid as a CSV (lat, lon, value) map layer plus its provenance record.
pub fn save_layer(
    name: &str,
    src: &Path,
    var: &Points,
    grid: &Grid2D,
    method: &str,
    steps: &[String],
) -> Result<(PathBuf, Provenance)> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    anyhow::ensure!(!clean.trim().is_empty(), "A layer name is required");
    let stem = format!(
        "{}-{}",
        clean.trim().chars().take(80).collect::<String>(),
        uuid::Uuid::new_v4()
    );
    let csv = dir.join(format!("{stem}.csv"));
    let mut out = String::from("lat,lon,value\n");
    for j in 0..grid.ny {
        for i in 0..grid.nx {
            let v = grid.values[j * grid.nx + i];
            if v.is_finite() {
                let (lat, lon) = grid.latlon(i, j);
                out += &format!("{lat:.8},{lon:.8},{v}\n");
            }
        }
    }
    std::fs::write(&csv, out)?;
    let prov = Provenance {
        name: name.to_string(),
        source_file: src.display().to_string(),
        format: detect(src).label().to_string(),
        variable: var.name.clone(),
        units: output_units(&var.units, steps),
        size_bytes: std::fs::metadata(src).map(|m| m.len()).unwrap_or(0),
        checksum: checksum(src)?,
        imported: chrono::Utc::now().timestamp(),
        regrid: format!(
            "{method} onto {}x{} over {:.2},{:.2} to {:.2},{:.2}",
            grid.nx, grid.ny, grid.bbox.0, grid.bbox.1, grid.bbox.2, grid.bbox.3
        ),
        steps: steps.to_vec(),
        output: csv.display().to_string(),
    };
    std::fs::write(
        dir.join(format!("{stem}.provenance.json")),
        serde_json::to_string_pretty(&prov)?,
    )?;
    Ok((csv, prov))
}

/// Every provenance record in the data folder, newest first.
pub fn provenance_list() -> Vec<Provenance> {
    let mut v: Vec<Provenance> = std::fs::read_dir(data_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .ends_with(".provenance.json")
        })
        .filter_map(|e| serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok())
        .collect();
    v.sort_by(|a: &Provenance, b: &Provenance| b.imported.cmp(&a.imported));
    v
}

fn check_file(path: &Path) -> Result<()> {
    let size = std::fs::metadata(path)?.len();
    anyhow::ensure!(size > 0, "The file is empty");
    anyhow::ensure!(
        size <= 512 * 1024 * 1024,
        "File exceeds the 512 MiB import limit; subset it first"
    );
    Ok(())
}

/// Reject invalid steps before changing the grid; typos never silently alter imported data.
pub fn apply_pipeline_checked(grid: &mut Grid2D, steps: &[String]) -> Result<Vec<String>> {
    for raw in steps {
        let p: Vec<_> = raw.split_whitespace().collect();
        let Some(op) = p.first() else {
            continue;
        };
        let count = match *op {
            "scale" | "offset" | "smooth" | "mask_below" => 1,
            "clip" => 2,
            "k_to_f" | "c_to_f" | "mm_to_in" | "ms_to_mph" | "pa_to_mb" | "abs" => 0,
            _ => return Err(anyhow!("Unknown pipeline step: {op}")),
        };
        anyhow::ensure!(p.len() == count + 1, "Wrong number of arguments for {op}");
        for value in &p[1..] {
            anyhow::ensure!(
                value.parse::<f32>().map(|v| v.is_finite()).unwrap_or(false),
                "Invalid number in {raw}"
            );
        }
        if *op == "smooth" {
            let n: f32 = p[1].parse()?;
            anyhow::ensure!(
                n >= 1. && n <= 10. && n.fract() == 0.,
                "Smooth expects an integer from 1 to 10"
            );
        }
    }
    Ok(apply_pipeline(grid, steps))
}

fn output_units(source: &str, steps: &[String]) -> String {
    let mut units = source.to_string();
    for step in steps {
        match step.split_whitespace().next().unwrap_or("") {
            "k_to_f" | "c_to_f" => units = "°F".into(),
            "mm_to_in" => units = "in".into(),
            "ms_to_mph" => units = "mph".into(),
            "pa_to_mb" => units = "hPa".into(),
            _ => (),
        }
    }
    units
}

#[cfg(test)]
mod tests;

/// Mask packed sentinels before applying CF scale/offset; reader unpacking alone does
/// not mask every NetCDF4 integer fill value. Apply the same rules to coordinates.
fn nc_values(file: &netcdf_reader::NcFile, var: &netcdf_reader::NcVariable) -> Result<Vec<f64>> {
    let attr = |name: &str| {
        var.attributes
            .iter()
            .find(|a| a.name == name)
            .and_then(|a| a.value.as_f64())
    };
    let missing: Vec<f64> = var
        .attributes
        .iter()
        .filter(|a| a.name == "_FillValue" || a.name == "missing_value")
        .flat_map(|a| a.value.as_f64_vec().unwrap_or_default())
        .collect();
    let range = var
        .attributes
        .iter()
        .find(|a| a.name == "valid_range")
        .and_then(|a| a.value.as_f64_vec());
    let min = attr("valid_min")
        .or_else(|| range.as_ref().and_then(|v| v.first().copied()))
        .unwrap_or(f64::NEG_INFINITY);
    let max = attr("valid_max")
        .or_else(|| range.as_ref().and_then(|v| v.get(1).copied()))
        .unwrap_or(f64::INFINITY);
    let (scale, offset) = (
        attr("scale_factor").unwrap_or(1.),
        attr("add_offset").unwrap_or(0.),
    );
    let values = file
        .read_variable_as_f64(var.name())
        .map_err(|e| anyhow!("NetCDF: {e}"))?;
    Ok(values
        .iter()
        .map(|raw| {
            if !raw.is_finite() || missing.contains(raw) || *raw < min || *raw > max {
                f64::NAN
            } else {
                *raw * scale + offset
            }
        })
        .collect())
}
