use super::Points;
use anyhow::{anyhow, Result};
use hdf5_reader::{dataset::Dataset, messages::datatype::Datatype, Hdf5File};
use std::path::Path;

pub fn numeric(d: &Dataset) -> Result<Vec<f64>> {
    let count = d
        .shape()
        .iter()
        .try_fold(1u64, |a, b| a.checked_mul(*b))
        .ok_or_else(|| anyhow!("HDF5 dimensions overflow"))?;
    anyhow::ensure!(
        count <= 20_000_000,
        "HDF5 dataset exceeds 20 million values; subset it first"
    );
    macro_rules! read {
        ($t:ty) => {
            d.read_array::<$t>()
                .map(|a| a.iter().map(|v| *v as f64).collect())
                .map_err(|e| anyhow!("HDF5: {e}"))
        };
    }
    match d.dtype() {
        Datatype::FloatingPoint { size: 4, .. } => read!(f32),
        Datatype::FloatingPoint { size: 8, .. } => read!(f64),
        Datatype::FixedPoint {
            size: 1,
            signed: true,
            ..
        } => read!(i8),
        Datatype::FixedPoint {
            size: 1,
            signed: false,
            ..
        } => read!(u8),
        Datatype::FixedPoint {
            size: 2,
            signed: true,
            ..
        } => read!(i16),
        Datatype::FixedPoint {
            size: 2,
            signed: false,
            ..
        } => read!(u16),
        Datatype::FixedPoint {
            size: 4,
            signed: true,
            ..
        } => read!(i32),
        Datatype::FixedPoint {
            size: 4,
            signed: false,
            ..
        } => read!(u32),
        Datatype::FixedPoint {
            size: 8,
            signed: true,
            ..
        } => read!(i64),
        Datatype::FixedPoint {
            size: 8,
            signed: false,
            ..
        } => read!(u64),
        _ => Err(anyhow!("HDF5 dataset is not a supported numeric type")),
    }
}
fn coord(f: &Hdf5File, path: &str, names: &[&str]) -> Option<Dataset> {
    let parent = path.rsplit_once('/').map(|p| p.0).unwrap_or("");
    for base in [parent, ""] {
        for name in names {
            if let Ok(d) = f.dataset(&format!("{base}/{name}")) {
                return Some(d);
            }
        }
    }
    None
}
pub fn mappable(f: &Hdf5File, path: &str) -> bool {
    coord(f, path, &["latitude", "lat", "Latitude"]).is_some()
        && coord(f, path, &["longitude", "lon", "Longitude"]).is_some()
}
pub fn decode(path: &Path, name: &str, bbox: Option<(f64, f64, f64, f64)>) -> Result<Points> {
    let f = Hdf5File::open(path).map_err(|e| anyhow!("HDF5: {e}"))?;
    let d = f.dataset(name).map_err(|e| anyhow!("HDF5: {e}"))?;
    let lat = coord(&f, name, &["latitude", "lat", "Latitude"])
        .ok_or_else(|| anyhow!("No geographic latitude dataset beside this variable"))?;
    let lon = coord(&f, name, &["longitude", "lon", "Longitude"])
        .ok_or_else(|| anyhow!("No geographic longitude dataset beside this variable"))?;
    let (data, lats, lons) = (numeric(&d)?, numeric(&lat)?, numeric(&lon)?);
    let shape = d.shape();
    anyhow::ensure!(
        shape.len() >= 2,
        "Select a two-dimensional geographic field"
    );
    let (ny, nx) = (
        shape[shape.len() - 2] as usize,
        shape[shape.len() - 1] as usize,
    );
    let rect = lats.len() == ny && lons.len() == nx;
    let curv = lat.shape() == lon.shape() && lats.len() == nx * ny;
    anyhow::ensure!(
        rect || curv,
        "Latitude/longitude shapes do not match the final two field dimensions"
    );
    let attrs = d.attributes();
    let attr = |n: &str| {
        attrs
            .iter()
            .find(|a| a.name == n)
            .and_then(|a| a.read_as_f64().ok())
    };
    let fill = attr("_FillValue").or_else(|| attr("missing_value"));
    let (scale, offset) = (
        attr("scale_factor").unwrap_or(1.),
        attr("add_offset").unwrap_or(0.),
    );
    let units = attrs
        .iter()
        .find(|a| a.name == "units")
        .and_then(|a| a.read_string().ok())
        .unwrap_or_default();
    let mut points = Points {
        name: name.into(),
        units,
        values: Vec::new(),
    };
    for j in 0..ny {
        for i in 0..nx {
            let k = j * nx + i;
            let a = if rect { lats[j] } else { lats[k] };
            let b = if rect { lons[i] } else { lons[k] };
            let b = (b + 180.).rem_euclid(360.) - 180.;
            let Some(raw) = data.get(k) else {
                continue;
            };
            let v = raw * scale + offset;
            if b.is_finite()
                && raw.is_finite()
                && v.is_finite()
                && fill != Some(*raw)
                && a.is_finite()
                && a.abs() <= 90.
                && bbox
                    .map(|(s, w, n, e)| a >= s && a <= n && b >= w && b <= e)
                    .unwrap_or(true)
            {
                points.values.push((a, b, v as f32));
            }
        }
    }
    Ok(points)
}
