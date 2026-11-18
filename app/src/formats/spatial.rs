//! Spatial indexing for nearest/IDW, and true bilinear interpolation on regular lat/lon grids.
use super::{Grid2D, Points};
use std::collections::HashMap;
struct Node {
    p: (f64, f64, f32),
    axis: usize,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}
impl Node {
    fn build(p: &mut [(f64, f64, f32)], depth: usize) -> Option<Box<Self>> {
        if p.is_empty() {
            return None;
        }
        let axis = depth % 2;
        let mid = p.len() / 2;
        p.select_nth_unstable_by(mid, |a, b| {
            if axis == 0 {
                a.0.total_cmp(&b.0)
            } else {
                a.1.total_cmp(&b.1)
            }
        });
        let (left, rest) = p.split_at_mut(mid);
        let (point, right) = rest.split_first_mut()?;
        Some(Box::new(Self {
            p: *point,
            axis,
            left: Self::build(left, depth + 1),
            right: Self::build(right, depth + 1),
        }))
    }
    fn nearest(&self, x: f64, y: f64, k: usize, out: &mut Vec<(f64, f32)>) {
        let d = (x - self.p.0).powi(2) + (y - self.p.1).powi(2);
        out.push((d, self.p.2));
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        out.truncate(k);
        let delta = if self.axis == 0 {
            x - self.p.0
        } else {
            y - self.p.1
        };
        let (near, far) = if delta < 0. {
            (&self.left, &self.right)
        } else {
            (&self.right, &self.left)
        };
        if let Some(n) = near {
            n.nearest(x, y, k, out);
        }
        if out.len() < k || delta * delta <= out.last().map(|v| v.0).unwrap_or(f64::INFINITY) {
            if let Some(n) = far {
                n.nearest(x, y, k, out);
            }
        }
    }
}
fn key(v: f64) -> i64 {
    (v * 1_000_000.).round() as i64
}
fn bracket(axis: &[i64], v: i64) -> Option<(i64, i64)> {
    match axis.binary_search(&v) {
        Ok(i) => Some((axis[i], axis[i])),
        Err(i) if i > 0 && i < axis.len() => Some((axis[i - 1], axis[i])),
        _ => None,
    }
}
pub fn run(p: &Points, bbox: (f64, f64, f64, f64), nx: usize, ny: usize, method: &str) -> Grid2D {
    let mut grid = Grid2D {
        bbox,
        nx,
        ny,
        values: vec![f32::NAN; nx.saturating_mul(ny).min(1_048_576)],
    };
    if nx < 2 || ny < 2 || nx.saturating_mul(ny) > 1_048_576 {
        return grid;
    }
    let valid: Vec<_> = p
        .values
        .iter()
        .copied()
        .filter(|(a, b, v)| a.is_finite() && b.is_finite() && v.is_finite())
        .collect();
    if method == "bilinear" {
        let mut lats: Vec<_> = valid.iter().map(|p| key(p.0)).collect();
        lats.sort_unstable();
        lats.dedup();
        let mut lons: Vec<_> = valid.iter().map(|p| key(p.1)).collect();
        lons.sort_unstable();
        lons.dedup();
        let cells: HashMap<_, _> = valid
            .iter()
            .map(|p| ((key(p.0), key(p.1)), p.2 as f64))
            .collect();
        for j in 0..ny {
            for i in 0..nx {
                let (lat, lon) = grid.latlon(i, j);
                let (a, b) = (key(lat), key(lon));
                let Some((s, n)) = bracket(&lats, a) else {
                    continue;
                };
                let Some((w, e)) = bracket(&lons, b) else {
                    continue;
                };
                let corners = [
                    cells.get(&(s, w)),
                    cells.get(&(s, e)),
                    cells.get(&(n, w)),
                    cells.get(&(n, e)),
                ];
                if let [Some(sw), Some(se), Some(nw), Some(ne)] = corners {
                    let x = if w == e {
                        0.
                    } else {
                        (b - w) as f64 / (e - w) as f64
                    };
                    let y = if s == n {
                        0.
                    } else {
                        (a - s) as f64 / (n - s) as f64
                    };
                    grid.values[j * nx + i] =
                        ((sw * (1. - x) + se * x) * (1. - y) + (nw * (1. - x) + ne * x) * y) as f32;
                }
            }
        }
    } else {
        let cos = ((bbox.0 + bbox.2) * 0.5).to_radians().cos().abs().max(0.01);
        let mut points: Vec<_> = valid.iter().map(|p| (p.0, p.1 * cos, p.2)).collect();
        let Some(tree) = Node::build(&mut points, 0) else {
            return grid;
        };
        for j in 0..ny {
            for i in 0..nx {
                let (lat, lon) = grid.latlon(i, j);
                let mut near = Vec::with_capacity(9);
                tree.nearest(
                    lat,
                    lon * cos,
                    if method == "nearest" { 1 } else { 8 },
                    &mut near,
                );
                grid.values[j * nx + i] = if method == "nearest" || near[0].0 < 1e-14 {
                    near[0].1
                } else {
                    let (sum, weight) = near
                        .iter()
                        .fold((0., 0.), |(s, w), (d, v)| (s + *v as f64 / d, w + 1. / d));
                    (sum / weight) as f32
                };
            }
        }
    }
    grid
}
