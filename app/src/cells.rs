//! Storm and lightning analysis on top of the feeds:
//! - persistent storm tracks (our own ids, matched by predicted position between updates),
//!   splits and merges, an intensity trend score, lightning-trend correlation, and a daily
//!   trajectory database (JSONL);
//! - lightning jumps (2-sigma flash-rate algorithm), lightning clusters tracked between
//!   updates, and lightning data quality;
//! - mesoscale convective system (MCS / squall line) identification from organized cells.

use std::sync::Mutex;

use crate::{
    data::{Strike, StormCell},
    map::geo::haversine_km,
};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TrackPoint {
    pub time: i64,
    pub lat: f64,
    pub lon: f64,
    pub dbz: f32,
    pub area: f32,
    /// Strikes within 10 km in the 2 minutes before this point.
    pub flashes: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Track {
    /// "S1", "S2"… stable for the life of the storm.
    pub id: String,
    pub points: Vec<TrackPoint>,
    /// Feed cell id it matched last.
    pub cell_id: String,
    /// -3 (collapsing) … +3 (rapidly strengthening).
    pub trend_score: i32,
    pub trend_label: String,
    /// Pearson correlation of dBZ and flash rate over the track (None = too short).
    pub lightning_corr: Option<f32>,
    /// Lightning jump in the last 10 minutes.
    pub jump: bool,
    pub ended: bool,
}

#[derive(Clone, Debug)]
pub struct CellEvent {
    pub time: i64,
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct Cluster {
    pub lat: f64,
    pub lon: f64,
    pub strikes: usize,
    /// Motion since the last update: (mph, heading degrees).
    pub motion: Option<(f64, f64)>,
}

#[derive(Clone, Debug, Default)]
pub struct Mcs {
    /// (lat, lon) hull around the member cells.
    pub hull: Vec<(f64, f64)>,
    pub length_mi: f64,
    pub squall_line: bool,
    pub cells: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Analysis {
    pub tracks: Vec<Track>,
    pub events: Vec<CellEvent>,
    pub clusters: Vec<Cluster>,
    pub mcs: Vec<Mcs>,
    /// Plain-language lightning feed quality ("" = fine).
    pub lightning_quality: String,
}

struct Tracker {
    next_id: u32,
    tracks: Vec<Track>,
    events: Vec<CellEvent>,
    clusters: Vec<Cluster>,
    last_update: i64,
}

fn tracker() -> &'static Mutex<Tracker> {
    static T: std::sync::OnceLock<Mutex<Tracker>> = std::sync::OnceLock::new();
    T.get_or_init(|| Mutex::new(Tracker { next_id: 1, tracks: Vec::new(), events: Vec::new(), clusters: Vec::new(), last_update: 0 }))
}

fn flashes_near(strikes: &[Strike], lat: f64, lon: f64, km: f64, from_ms: i64, to_ms: i64) -> u32 {
    strikes.iter().filter(|s| s.time_ms >= from_ms && s.time_ms < to_ms && haversine_km(lat, lon, s.lat, s.lon) <= km).count() as u32
}

fn pearson(xs: &[f32], ys: &[f32]) -> Option<f32> {
    let n = xs.len().min(ys.len());
    if n < 4 {
        return None;
    }
    let (mx, my) = (xs[..n].iter().sum::<f32>() / n as f32, ys[..n].iter().sum::<f32>() / n as f32);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (dx, dy) = (xs[i] - mx, ys[i] - my);
        sxy += dx * dy;
        sxx += dx * dx;
        syy += dy * dy;
    }
    (sxx > 0.0 && syy > 0.0).then(|| sxy / (sxx.sqrt() * syy.sqrt()))
}

/// Update with the newest cells and strikes. Call on every cells refresh; returns the analysis.
pub fn update(cells: &[StormCell], strikes: &[Strike]) -> Analysis {
    let now = chrono::Utc::now().timestamp();
    let now_ms = now * 1000;
    let Ok(mut t) = tracker().lock() else { return Analysis::default() };
    let dt = if t.last_update > 0 { (now - t.last_update).max(1) } else { 0 };
    t.last_update = now;

    // ---- Match cells to live tracks by predicted position (nearest within 15 km) ----
    let live: Vec<usize> = (0..t.tracks.len()).filter(|i| !t.tracks[*i].ended).collect();
    let predicted: Vec<(usize, f64, f64)> = live
        .iter()
        .map(|&i| {
            let p = &t.tracks[i].points;
            let last = p.last().cloned().unwrap_or_default();
            // Constant-velocity guess from the last two points.
            let (lat, lon) = if p.len() >= 2 {
                let prev = &p[p.len() - 2];
                let span = (last.time - prev.time).max(1) as f64;
                let k = dt as f64 / span;
                (last.lat + (last.lat - prev.lat) * k, last.lon + (last.lon - prev.lon) * k)
            } else {
                (last.lat, last.lon)
            };
            (i, lat, lon)
        })
        .collect();
    let mut cell_to_track: Vec<Option<usize>> = vec![None; cells.len()];
    let mut track_hits: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for (ci, c) in cells.iter().enumerate() {
        // Same feed id is the strongest evidence; otherwise the nearest predicted position.
        let by_id = predicted.iter().find(|(i, _, _)| t.tracks[*i].cell_id == c.id && !c.id.is_empty()).map(|(i, _, _)| *i);
        let nearest = predicted
            .iter()
            .map(|(i, lat, lon)| (*i, haversine_km(*lat, *lon, c.lat, c.lon)))
            .filter(|(_, d)| *d <= 15.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        if let Some(i) = by_id.or(nearest) {
            cell_to_track[ci] = Some(i);
            track_hits.entry(i).or_default().push(ci);
        }
    }
    let mut new_events = Vec::new();
    // Merges: one cell matched by several tracks within 15 km (the others get absorbed).
    for (ci, c) in cells.iter().enumerate() {
        let near: Vec<usize> = predicted
            .iter()
            .filter(|(i, lat, lon)| Some(*i) != cell_to_track[ci] && haversine_km(*lat, *lon, c.lat, c.lon) <= 10.0 && track_hits.get(i).map(|v| v.is_empty()).unwrap_or(true))
            .map(|(i, _, _)| *i)
            .collect();
        if let (Some(keep), false) = (cell_to_track[ci], near.is_empty()) {
            for gone in near {
                t.tracks[gone].ended = true;
                let (a, b) = (t.tracks[gone].id.clone(), t.tracks[keep].id.clone());
                new_events.push(CellEvent { time: now, text: format!("Storms {a} and {b} merged") });
            }
        }
    }
    let flashes = |c: &StormCell| flashes_near(strikes, c.lat, c.lon, 10.0, now_ms - 120_000, now_ms);
    // Splits: a track matched by two or more cells keeps the strongest; the rest start new tracks.
    for (ti, cis) in track_hits.clone() {
        if cis.len() < 2 {
            continue;
        }
        let mut sorted = cis.clone();
        sorted.sort_by(|a, b| cells[*b].max_dbz.total_cmp(&cells[*a].max_dbz));
        for &ci in &sorted[1..] {
            cell_to_track[ci] = None;
        }
        let parent = t.tracks[ti].id.clone();
        new_events.push(CellEvent { time: now, text: format!("Storm {parent} split into {} storms", sorted.len()) });
    }
    // Extend or start tracks.
    let mut seen = vec![false; t.tracks.len()];
    for (ci, c) in cells.iter().enumerate() {
        let point = TrackPoint { time: now, lat: c.lat, lon: c.lon, dbz: c.max_dbz as f32, area: c.area_sq_mi as f32, flashes: flashes(c) };
        match cell_to_track[ci] {
            Some(i) => {
                seen[i] = true;
                let tr = &mut t.tracks[i];
                tr.cell_id = c.id.clone();
                if tr.points.last().map(|p| now - p.time >= 50).unwrap_or(true) {
                    tr.points.push(point);
                }
            }
            None => {
                let id = format!("S{}", t.next_id);
                t.next_id += 1;
                t.tracks.push(Track { id, cell_id: c.id.clone(), points: vec![point], ..Default::default() });
                seen.push(true);
            }
        }
    }
    for (i, tr) in t.tracks.iter_mut().enumerate() {
        if !seen.get(i).copied().unwrap_or(false) && !tr.ended {
            // Gone for 15 minutes: the storm has ended.
            if tr.points.last().map(|p| now - p.time > 900).unwrap_or(true) {
                tr.ended = true;
            }
        }
    }

    // ---- Trend score, lightning correlation, lightning jump ----
    for tr in t.tracks.iter_mut().filter(|tr| !tr.ended) {
        let recent: Vec<&TrackPoint> = tr.points.iter().filter(|p| now - p.time <= 30 * 60).collect();
        if let (Some(first), Some(last)) = (recent.first(), recent.last()) {
            let dd = last.dbz - first.dbz;
            let da = last.area - first.area;
            let df = last.flashes as f32 - first.flashes as f32;
            let mut score = 0;
            score += if dd >= 6.0 { 2 } else if dd >= 3.0 { 1 } else if dd <= -6.0 { -2 } else if dd <= -3.0 { -1 } else { 0 };
            score += if da >= 20.0 { 1 } else if da <= -20.0 { -1 } else { 0 };
            score += if df >= 10.0 { 1 } else if df <= -10.0 { -1 } else { 0 };
            tr.trend_score = score.clamp(-3, 3);
            tr.trend_label = match tr.trend_score {
                3 => "Strengthening fast",
                1..=2 => "Strengthening",
                0 => "Steady",
                -2..=-1 => "Weakening",
                _ => "Collapsing",
            }
            .to_string();
        }
        let dbz: Vec<f32> = tr.points.iter().map(|p| p.dbz).collect();
        let fl: Vec<f32> = tr.points.iter().map(|p| p.flashes as f32).collect();
        tr.lightning_corr = pearson(&dbz, &fl);
        // 2-sigma lightning jump: rate change now vs the spread of changes over the prior 10 min.
        let rates: Vec<(i64, f32)> = tr.points.iter().filter(|p| now - p.time <= 14 * 60).map(|p| (p.time, p.flashes as f32 / 2.0)).collect();
        tr.jump = false;
        if rates.len() >= 5 {
            let diffs: Vec<f32> = rates.windows(2).map(|w| (w[1].1 - w[0].1) / ((w[1].0 - w[0].0).max(1) as f32 / 60.0)).collect();
            let (hist, latest) = diffs.split_at(diffs.len() - 1);
            let mean = hist.iter().sum::<f32>() / hist.len() as f32;
            let sd = (hist.iter().map(|d| (d - mean).powi(2)).sum::<f32>() / hist.len() as f32).sqrt();
            let rate_now = rates.last().map(|r| r.1).unwrap_or(0.0);
            if rate_now >= 5.0 && latest[0] > 2.0 * sd.max(0.5) {
                tr.jump = true;
                new_events.push(CellEvent { time: now, text: format!("Lightning jump in storm {} ({:.0} flashes/min)", tr.id, rate_now) });
            }
        }
    }

    // ---- Trajectory database ----
    let dir = crate::config::LibraryPaths::get().root.join("Storm tracks");
    if std::fs::create_dir_all(&dir).is_ok() {
        let path = dir.join(format!("{}.jsonl", chrono::Local::now().format("%Y-%m-%d")));
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            use std::io::Write;
            for tr in t.tracks.iter().filter(|tr| !tr.ended) {
                if let Some(p) = tr.points.last().filter(|p| p.time == now) {
                    let _ = writeln!(f, "{}", serde_json::json!({ "track": tr.id, "point": p, "trend": tr.trend_label }));
                }
            }
        }
    }
    // Forget tracks that ended over 2 hours ago; keep points for 3 hours.
    t.tracks.retain(|tr| !(tr.ended && tr.points.last().map(|p| now - p.time > 7200).unwrap_or(true)));
    for tr in t.tracks.iter_mut() {
        tr.points.retain(|p| now - p.time <= 3 * 3600);
    }
    t.events.extend(new_events);
    t.events.retain(|e| now - e.time <= 2 * 3600);

    // ---- Lightning clusters (10-minute window, 0.1° bins merged with neighbours) ----
    let recent: Vec<&Strike> = strikes.iter().filter(|s| now_ms - s.time_ms <= 10 * 60_000).collect();
    let mut bins: std::collections::HashMap<(i32, i32), Vec<&Strike>> = std::collections::HashMap::new();
    for s in &recent {
        bins.entry(((s.lat * 10.0).floor() as i32, (s.lon * 10.0).floor() as i32)).or_default().push(s);
    }
    let mut keys: Vec<(i32, i32)> = bins.keys().copied().collect();
    keys.sort();
    let mut used = std::collections::HashSet::new();
    let mut clusters = Vec::new();
    for k in keys {
        if used.contains(&k) {
            continue;
        }
        // Flood fill over neighbouring bins.
        let mut stack = vec![k];
        let mut members: Vec<&Strike> = Vec::new();
        while let Some(b) = stack.pop() {
            if !used.insert(b) {
                continue;
            }
            if let Some(v) = bins.get(&b) {
                members.extend(v.iter().copied());
                for (dy, dx) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)] {
                    let n = (b.0 + dy, b.1 + dx);
                    if bins.contains_key(&n) && !used.contains(&n) {
                        stack.push(n);
                    }
                }
            }
        }
        if members.len() >= 5 {
            let n = members.len() as f64;
            let lat = members.iter().map(|s| s.lat).sum::<f64>() / n;
            let lon = members.iter().map(|s| s.lon).sum::<f64>() / n;
            clusters.push(Cluster { lat, lon, strikes: members.len(), motion: None });
        }
    }
    // Motion: match to the nearest previous cluster within 40 km.
    for c in clusters.iter_mut() {
        if let Some(prev) = t.clusters.iter().map(|p| (p, haversine_km(p.lat, p.lon, c.lat, c.lon))).filter(|(_, d)| *d <= 40.0).min_by(|a, b| a.1.total_cmp(&b.1)) {
            if dt > 30 {
                let mph = prev.1 / 1.609 / (dt as f64 / 3600.0);
                let heading = crate::map::geo::bearing_deg(prev.0.lat, prev.0.lon, c.lat, c.lon);
                if mph < 90.0 {
                    c.motion = Some((mph, heading));
                }
            }
        }
    }
    t.clusters = clusters.clone();

    // ---- Lightning quality ----
    let newest = strikes.iter().map(|s| s.time_ms).max();
    let storms = cells.iter().any(|c| c.max_dbz >= 45.0);
    let lightning_quality = match newest {
        None if storms => "No lightning data while storms are on radar".to_string(),
        Some(t) if storms && now_ms - t > 15 * 60_000 => format!("Newest strike is {} min old while storms are on radar", (now_ms - t) / 60_000),
        _ => String::new(),
    };

    Analysis {
        tracks: t.tracks.clone(),
        events: t.events.clone(),
        clusters,
        mcs: find_mcs(cells),
        lightning_quality,
    }
}

/// MCS identification: cells of 40+ dBZ chained within 30 km form a system; one longer than
/// 100 km (60 mi) is an MCS, and a long, thin one (length >= 3x width) is a squall line.
pub fn find_mcs(cells: &[StormCell]) -> Vec<Mcs> {
    let strong: Vec<&StormCell> = cells.iter().filter(|c| c.max_dbz >= 40.0).collect();
    let mut group = vec![usize::MAX; strong.len()];
    let mut next = 0;
    for i in 0..strong.len() {
        if group[i] != usize::MAX {
            continue;
        }
        let mut stack = vec![i];
        while let Some(a) = stack.pop() {
            if group[a] != usize::MAX {
                continue;
            }
            group[a] = next;
            for b in 0..strong.len() {
                if group[b] == usize::MAX && haversine_km(strong[a].lat, strong[a].lon, strong[b].lat, strong[b].lon) <= 30.0 {
                    stack.push(b);
                }
            }
        }
        next += 1;
    }
    let mut out = Vec::new();
    for g in 0..next {
        let members: Vec<&StormCell> = strong.iter().zip(&group).filter(|(_, gg)| **gg == g).map(|(c, _)| *c).collect();
        if members.len() < 3 {
            continue;
        }
        // Longest distance between members = system length.
        let mut length = 0.0f64;
        let mut ends = (0, 0);
        for a in 0..members.len() {
            for b in a + 1..members.len() {
                let d = haversine_km(members[a].lat, members[a].lon, members[b].lat, members[b].lon);
                if d > length {
                    length = d;
                    ends = (a, b);
                }
            }
        }
        if length < 100.0 {
            continue;
        }
        // Width: farthest member from the line between the two ends.
        let (p, q) = (members[ends.0], members[ends.1]);
        let width = members
            .iter()
            .map(|m| {
                let along = haversine_km(p.lat, p.lon, m.lat, m.lon);
                let to_end = haversine_km(q.lat, q.lon, m.lat, m.lon);
                // Heron's formula for the triangle's height over the base p-q.
                let s = (along + to_end + length) / 2.0;
                let area = (s * (s - along) * (s - to_end) * (s - length)).max(0.0).sqrt();
                2.0 * area / length
            })
            .fold(0.0, f64::max)
            .max(10.0);
        out.push(Mcs {
            hull: convex_hull(members.iter().map(|m| (m.lat, m.lon)).collect()),
            length_mi: length / 1.609,
            squall_line: length >= 3.0 * width,
            cells: members.len(),
        });
    }
    out
}

/// Monotone-chain convex hull of (lat, lon) points.
fn convex_hull(mut pts: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    pts.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0)));
    pts.dedup();
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| (a.1 - o.1) * (b.0 - o.0) - (a.0 - o.0) * (b.1 - o.1);
    let mut lower: Vec<(f64, f64)> = Vec::new();
    for &p in &pts {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<(f64, f64)> = Vec::new();
    for &p in pts.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}
