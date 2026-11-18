//! Community board: school / business closings and warming / cooling centers, entered by the
//! station (there is no public feed for these). Saved as `Documents\OpenWXBroadcasting\community.json`.
//!
//! Text format used by the editor, one entry per line:
//! - Closings: `Name - Status` (e.g. `Jasper Schools - 2 hour delay`)
//! - Centers: `warming | Name | Town or address | Hours` (the town places it on the map)

use serde::{Deserialize, Serialize};

use crate::map::extras::Center;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Closing {
    pub name: String,
    pub status: String,
    /// "schools", "business", "government" or "events" (blank counts as schools).
    pub category: String,
}

pub const CATEGORIES: &[(&str, &str)] = &[("schools", "School"), ("business", "Business"), ("government", "Government"), ("events", "Event")];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Community {
    pub closings: Vec<Closing>,
    pub centers: Vec<Center>,
    /// Unix seconds of the last edit.
    pub updated: i64,
}

fn path() -> std::path::PathBuf {
    crate::config::LibraryPaths::get().root.join("community.json")
}

pub fn load() -> Community {
    std::fs::read_to_string(path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save(c: &Community) -> anyhow::Result<()> {
    std::fs::write(path(), serde_json::to_string_pretty(c)?)?;
    Ok(())
}

impl Community {
    pub fn closings_text(&self) -> String {
        self.closings.iter().map(|c| format!("{} - {}", c.name, c.status)).collect::<Vec<_>>().join("\n")
    }

    pub fn centers_text(&self) -> String {
        self.centers
            .iter()
            .map(|c| format!("{} | {} | {} | {}", c.kind, c.name, c.address, c.hours))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn parse_closings(text: &str) -> Vec<Closing> {
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| match l.split_once(" - ").or_else(|| l.split_once(':')) {
                Some((n, s)) => Closing { name: n.trim().into(), status: s.trim().into(), category: String::new() },
                None => Closing { name: l.into(), status: "Closed".into(), category: String::new() },
            })
            .collect()
    }

    /// Parse centers; a center keeps its old position if its address did not change, otherwise
    /// it is placed on the first town found in the address.
    pub fn parse_centers(text: &str, old: &[Center]) -> Vec<Center> {
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| {
                let f: Vec<&str> = l.split('|').map(str::trim).collect();
                let (kind, name, address, hours) = match f.len() {
                    0 | 1 => ("warming", f.first().copied().unwrap_or(""), "", ""),
                    2 => (f[0], f[1], "", ""),
                    3 => (f[0], f[1], f[2], ""),
                    _ => (f[0], f[1], f[2], f[3]),
                };
                let kind = match kind.to_lowercase() {
                    k if k.starts_with('c') => "cooling",
                    k if k.starts_with('s') => "shelter",
                    _ => "warming",
                };
                let mut c = Center {
                    name: name.into(),
                    kind: kind.into(),
                    address: address.into(),
                    hours: hours.into(),
                    lat: 0.0,
                    lon: 0.0,
                };
                if let Some(prev) = old.iter().find(|o| o.name == c.name && o.address == c.address && o.lat != 0.0) {
                    c.lat = prev.lat;
                    c.lon = prev.lon;
                } else if let Some((lat, lon)) = crate::places::parse_lat_lon(address) {
                    c.lat = lat;
                    c.lon = lon;
                } else {
                    // Try each comma part of the address as a town name ("123 Main St, Jasper, IN").
                    let parts: Vec<&str> = address.split(',').map(str::trim).collect();
                    let found = parts
                        .iter()
                        .enumerate()
                        .find_map(|(i, p)| {
                            let q = match parts.get(i + 1) {
                                Some(st) if st.len() == 2 => format!("{p}, {st}"),
                                _ => p.to_string(),
                            };
                            crate::places::search(&q, 1).into_iter().next()
                        });
                    if let Some(p) = found {
                        c.lat = p.lat;
                        c.lon = p.lon;
                    }
                }
                c
            })
            .collect()
    }
}
