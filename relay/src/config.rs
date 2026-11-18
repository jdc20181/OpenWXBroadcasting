//! Relay settings (`relay.toml`). Every value has a safe default; `openwx-relay setup` and the
//! desktop app's setup wizard write it for the station.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const DEFAULT_PORT: u16 = 8737;
/// The public LibreWXR API: free for personal and trial use. Self-hosted LibreWXR has no such limit.
pub const DEFAULT_RADAR_UPSTREAM: &str = "https://api.librewxr.net";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RelayConfig {
    /// Address to listen on. `127.0.0.1` keeps the relay private to this computer (the default,
    /// and what the desktop app uses). Use `0.0.0.0` only behind HTTPS with `access_token` set.
    pub bind: String,
    pub port: u16,
    /// Required on a shared/remote server. Clients then use
    /// `https://your-server/t/<access_token>` as their backend URL. Empty = no token (local only).
    pub access_token: String,
    /// Contact sent to NOAA/NWS in the User-Agent, as their terms ask (an e-mail or a website).
    pub contact: String,
    /// Station name, for the User-Agent and the health page.
    pub station: String,
    /// The station's home point (SPC "at the coverage point" summary, lightning around home).
    pub home_lat: Option<f64>,
    pub home_lon: Option<f64>,
    /// Shown with the SPC summary ("Dubois County, IN" style). Free text.
    pub coverage_label: String,
    /// Radar/satellite composite tiles, storm cells and GLM lightning (RainViewer-compatible API).
    pub radar_upstream: String,
    /// Allow the Open-Meteo providers some app features still use. Open-Meteo's free API is for
    /// non-commercial use; commercial stations need their own Open-Meteo plan.
    pub open_meteo: bool,
    /// Memory for files that never change (finished model runs, basemap tiles, past radar frames,
    /// normals), in MB. Live weather is never cached.
    pub cache_mb: u64,
    /// Upstream requests in flight at once; more get "busy, retry in 5 s".
    pub max_inflight: usize,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1".into(),
            port: DEFAULT_PORT,
            access_token: String::new(),
            contact: String::new(),
            station: String::new(),
            home_lat: None,
            home_lon: None,
            coverage_label: String::new(),
            radar_upstream: DEFAULT_RADAR_UPSTREAM.into(),
            open_meteo: true,
            cache_mb: 64,
            max_inflight: 32,
        }
    }
}

impl RelayConfig {
    /// `relay.toml` next to the app's settings (`%APPDATA%\OpenWXBroadcasting` on Windows,
    /// `~/.config/OpenWXBroadcasting` on Linux). Servers usually pass `--config /etc/openwx/relay.toml`.
    pub fn default_path() -> PathBuf {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("OpenWXBroadcasting").join("relay.toml")
    }

    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&text)?;
        config.validate()?;
        Ok(config)
    }

    pub fn load_or_default(path: &Path) -> Self {
        match Self::load(path) {
            Ok(config) => config,
            Err(error) => {
                if path.exists() {
                    log::warn!("relay config {} unreadable ({error}); using defaults", path.display());
                }
                Self::default()
            }
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        self.validate()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = format!(
            "# OpenWX Relay settings. Edit, then restart the relay (or run `openwx-relay setup`).\n{}",
            toml::to_string_pretty(self)?
        );
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Refuses settings that would expose an open relay to the internet.
    pub fn validate(&self) -> anyhow::Result<()> {
        let local = matches!(self.bind.as_str(), "127.0.0.1" | "::1" | "localhost");
        if !local && self.access_token.trim().len() < 24 {
            anyhow::bail!(
                "bind = \"{}\" makes the relay reachable from other computers: set access_token \
                 (24+ random characters, `openwx-relay token` makes one) or bind to 127.0.0.1",
                self.bind
            );
        }
        if !self.radar_upstream.starts_with("https://") && !self.radar_upstream.starts_with("http://127.0.0.1") {
            anyhow::bail!("radar_upstream must be an https:// address");
        }
        if let (Some(lat), Some(lon)) = (self.home_lat, self.home_lon) {
            if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
                anyhow::bail!("home_lat/home_lon are out of range");
            }
        }
        Ok(())
    }

    pub fn home(&self) -> Option<(f64, f64)> {
        Some((self.home_lat?, self.home_lon?))
    }

    /// NWS asks every client to identify itself with a contact.
    pub fn user_agent(&self) -> String {
        let contact = if self.contact.trim().is_empty() { "no contact set" } else { self.contact.trim() };
        let station = if self.station.trim().is_empty() { "OpenWX station" } else { self.station.trim() };
        format!("OpenWXRelay/{} ({station}; {contact})", env!("CARGO_PKG_VERSION"))
    }
}

/// A random access token (48 hex characters) from the OS random source.
pub fn new_token() -> String {
    let mut bytes = [0u8; 24];
    if getrandom(&mut bytes).is_err() {
        // Fallback: time and address entropy (only if the OS source failed).
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = ((seed >> ((i % 16) * 8)) as u8) ^ (i as u8).wrapping_mul(97);
        }
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(unix)]
fn getrandom(buf: &mut [u8]) -> std::io::Result<()> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")?.read_exact(buf)
}

#[cfg(windows)]
fn getrandom(buf: &mut [u8]) -> std::io::Result<()> {
    // RtlGenRandom (SystemFunction036) from advapi32, always present on Windows.
    #[link(name = "advapi32")]
    extern "system" {
        #[link_name = "SystemFunction036"]
        fn rtl_gen_random(buffer: *mut u8, length: u32) -> u8;
    }
    let ok = unsafe { rtl_gen_random(buf.as_mut_ptr(), buf.len() as u32) };
    if ok != 0 { Ok(()) } else { Err(std::io::Error::other("RtlGenRandom failed")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_bind_needs_a_token() {
        let mut c = RelayConfig { bind: "0.0.0.0".into(), ..Default::default() };
        assert!(c.validate().is_err());
        c.access_token = new_token();
        assert!(c.validate().is_ok());
    }

    #[test]
    fn tokens_are_random_hex() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 48);
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn round_trips_through_toml() {
        let c = RelayConfig { contact: "wx@example.org".into(), home_lat: Some(38.4), home_lon: Some(-86.9), ..Default::default() };
        let text = toml::to_string_pretty(&c).unwrap();
        assert_eq!(toml::from_str::<RelayConfig>(&text).unwrap(), c);
    }
}
