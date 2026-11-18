//! Which backend the app reads (Settings > General > Backend):
//!
//! - **local** (default): OpenWX Relay runs inside this app on `127.0.0.1` (next free port from
//!   `relay_port`). Nothing to install or host.
//! - **remote**: a hosted OpenWX Relay (`remote_api_base`, e.g. `https://wx.example.org/t/<token>`)
//!   shared by several studio PCs. `server/install.sh` sets one up.
//! - **direct**: no relay; each feed falls back to its upstream where it can (troubleshooting).

use std::sync::{Mutex, OnceLock};

use crate::config::Settings;

struct Local {
    handle: openwx_relay::RelayHandle,
    config: openwx_relay::RelayConfig,
}

fn local() -> &'static Mutex<Option<Local>> {
    static LOCAL: OnceLock<Mutex<Option<Local>>> = OnceLock::new();
    LOCAL.get_or_init(|| Mutex::new(None))
}

fn contact_cell() -> &'static Mutex<String> {
    static CONTACT: OnceLock<Mutex<String>> = OnceLock::new();
    CONTACT.get_or_init(|| Mutex::new(String::new()))
}

/// The station contact for the User-Agent of requests the app makes itself.
pub fn contact() -> String {
    contact_cell().lock().map(|c| c.clone()).unwrap_or_default()
}

/// The bundled relay's settings, from the station's own settings.
pub fn relay_config(settings: &Settings) -> openwx_relay::RelayConfig {
    let label = if settings.coverage.name.trim().is_empty() { settings.home_label.clone() } else { settings.coverage.name.clone() };
    openwx_relay::RelayConfig {
        bind: "127.0.0.1".into(),
        port: if settings.relay_port == 0 { openwx_relay::DEFAULT_PORT } else { settings.relay_port },
        access_token: String::new(),
        contact: settings.contact.trim().to_owned(),
        station: settings.station_name.trim().to_owned(),
        home_lat: settings.setup_complete.then_some(settings.home.lat),
        home_lon: settings.setup_complete.then_some(settings.home.lon),
        coverage_label: label.trim().to_owned(),
        radar_upstream: radar_upstream(settings),
        ..Default::default()
    }
}

/// The radar server from settings, or the public LibreWXR API when blank.
pub fn radar_upstream(settings: &Settings) -> String {
    let address = settings.radar_upstream.trim().trim_end_matches('/');
    if address.is_empty() { openwx_relay::config::DEFAULT_RADAR_UPSTREAM.to_owned() } else { address.to_owned() }
}

/// Start (or restart, when the station's details changed) the bundled relay. Returns its base URL.
fn ensure_local(settings: &Settings) -> Option<String> {
    let wanted = relay_config(settings);
    let mut slot = local().lock().ok()?;
    if let Some(running) = slot.as_ref() {
        if running.config == wanted {
            return Some(running.handle.base_url());
        }
    }
    if let Some(old) = slot.take() {
        old.handle.stop();
    }
    let started = crate::net::runtime().block_on(openwx_relay::start(wanted.clone(), true));
    match started {
        Ok(handle) => {
            let base = handle.base_url();
            log::info!("bundled OpenWX Relay on {base}");
            *slot = Some(Local { handle, config: wanted });
            Some(base)
        }
        Err(error) => {
            log::error!("could not start the bundled relay: {error}");
            None
        }
    }
}

/// The base URL the app should read for these settings (starting the bundled relay if needed).
pub fn effective_base(settings: &Settings) -> String {
    if let Ok(mut contact) = contact_cell().lock() {
        *contact = settings.contact.trim().to_owned();
    }
    match settings.backend_mode.as_str() {
        "remote" => settings.remote_api_base.trim().trim_end_matches('/').to_owned(),
        "direct" => String::new(),
        _ => ensure_local(settings).unwrap_or_default(),
    }
}

/// The backend modes for the Settings picker: (id, label, help).
pub const MODES: &[(&str, &str, &str)] = &[
    ("local", "On this PC (built in)", "OpenWX Relay runs inside the app. Nothing to install."),
    ("remote", "Hosted server", "A shared OpenWX Relay on your own server (see server/README.md)."),
    ("direct", "Direct (troubleshooting)", "No relay: features without a direct source stay empty."),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_and_direct_bases() {
        let mut s = Settings { backend_mode: "remote".into(), remote_api_base: " https://wx.example.org/t/abc/ ".into(), ..Default::default() };
        assert_eq!(effective_base(&s), "https://wx.example.org/t/abc");
        s.backend_mode = "direct".into();
        assert_eq!(effective_base(&s), "");
    }

    #[test]
    fn relay_gets_home_only_after_setup() {
        let mut s = Settings { contact: "wx@example.org".into(), ..Default::default() };
        assert_eq!(relay_config(&s).home_lat, None);
        s.setup_complete = true;
        assert!(relay_config(&s).home_lat.is_some());
        assert_eq!(relay_config(&s).contact, "wx@example.org");
    }

    #[test]
    fn radar_server_defaults_to_the_public_api_and_takes_a_self_hosted_one() {
        let mut s = Settings { radar_upstream: String::new(), ..Default::default() };
        assert_eq!(relay_config(&s).radar_upstream, openwx_relay::config::DEFAULT_RADAR_UPSTREAM);
        s.radar_upstream = "https://radar.example.org/".into();
        assert_eq!(relay_config(&s).radar_upstream, "https://radar.example.org");
    }

    #[test]
    fn local_mode_starts_the_bundled_relay() {
        let s = Settings { relay_port: 0, ..Default::default() };
        let base = effective_base(&s);
        assert!(base.starts_with("http://127.0.0.1:"), "{base}");
        assert_eq!(effective_base(&s), base, "same settings reuse the running relay");
    }
}
