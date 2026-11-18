//! Bounded, strict geographic ArcGIS polygon rings for graphics-local NOAA products.
use serde_json::Value;
pub(super) type Rings = Vec<Vec<(f64, f64)>>;
pub(super) fn rings(value: &Value, vertices: &mut usize) -> Option<Rings> {
    let input = value.get("rings")?.as_array()?;
    if input.is_empty() || input.len() > 512 {
        return None;
    }
    let mut result = Vec::new();
    for raw in input {
        let points = raw.as_array()?;
        if !(4..=100000).contains(&points.len()) {
            return None;
        }
        *vertices = vertices.checked_add(points.len())?;
        if *vertices > 200000 {
            return None;
        }
        let mut ring = Vec::new();
        for p in points {
            let xy = p.as_array()?;
            if xy.len() != 2 {
                return None;
            }
            let (x, y) = (xy[0].as_f64()?, xy[1].as_f64()?);
            if !x.is_finite()
                || !y.is_finite()
                || !(-180.0..=180.).contains(&x)
                || !(-90.0..=90.).contains(&y)
            {
                return None;
            }
            ring.push((x, y));
        }
        if ring.first() != ring.last()
            || ring
                .windows(2)
                .map(|p| p[0].0 * p[1].1 - p[1].0 * p[0].1)
                .sum::<f64>()
                .abs()
                < 1e-12
        {
            return None;
        }
        result.push(ring);
    }
    Some(result)
}
/// Even-odd holes/islands; a boundary hit is ambiguous, never silently assigned.
pub(super) fn contains(rings: &Rings, x: f64, y: f64) -> Option<bool> {
    let mut inside = false;
    for ring in rings {
        for pair in ring.windows(2) {
            let ((ax, ay), (bx, by)) = (pair[0], pair[1]);
            let cross = (x - ax) * (by - ay) - (y - ay) * (bx - ax);
            if cross.abs() < 1e-10
                && x >= ax.min(bx) - 1e-10
                && x <= ax.max(bx) + 1e-10
                && y >= ay.min(by) - 1e-10
                && y <= ay.max(by) + 1e-10
            {
                return None;
            }
            if (ay > y) != (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax {
                inside = !inside;
            }
        }
    }
    Some(inside)
}
