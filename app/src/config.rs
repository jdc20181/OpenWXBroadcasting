//! Persistent settings (config dir) and the on-disk library layout (Documents).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const APP_NAME: &str = "OpenWXBroadcasting";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Build number within a version (from BUILD_NUMBER). An update is newer when its version is
/// higher, or the version is the same and the build is higher.
pub const APP_BUILD: u32 = parse_u32(env!("CWX_BUILD"));

/// "0.1.0 (build 2)" for About, the status bar and logs.
pub fn version_label() -> String {
    format!("{APP_VERSION} (build {APP_BUILD})")
}

/// Builds are numbered by date since 0.1.8 (owner, 2026-11-15): MMDDYYYY then the build of
/// that day, e.g. 111520262 = November 15 2026, second build. Such a number does not sort by
/// date (Jan 1 2027 < Dec 31 2026 as numbers), so updates compare this key instead:
/// YYYYMMDD and the day's build, above every older plain number (1, 2 ... 13).
pub fn build_order(build: u32) -> u64 {
    match parse_dated_build(build) {
        Some((year, month, day, n)) => 1_000_000_000_000 + (year * 10_000 + month * 100 + day) * 1_000 + n,
        None => build as u64,
    }
}

/// (year, month, day, build of the day) of a dated build number. January to September lose the
/// month's leading zero as a number (Jan 1 2027, build 1 = 10120271), so both month widths are
/// tried and the reading with a real date in 2020-2099 wins.
pub fn parse_dated_build(build: u32) -> Option<(u64, u64, u64, u64)> {
    let s = build.to_string();
    for mlen in [2usize, 1] {
        let part = |a: usize, b: usize| s.get(a..b).and_then(|p| p.parse::<u64>().ok());
        let (Some(month), Some(day), Some(year)) = (part(0, mlen), part(mlen, mlen + 2), part(mlen + 2, mlen + 6)) else { continue };
        let rest = s.get(mlen + 6..).unwrap_or("");
        if rest.is_empty() || rest.len() > 2 {
            continue;
        }
        let n: u64 = rest.parse().ok()?;
        if (1..=12).contains(&month) && (1..=31).contains(&day) && (2020..=2099).contains(&year) && n >= 1 {
            return Some((year, month, day, n));
        }
    }
    None
}

#[cfg(test)]
mod build_order_tests {
    use super::build_order;

    #[test]
    fn dated_builds_sort_by_date() {
        assert!(build_order(111520262) > build_order(13));
        assert!(build_order(111520262) > build_order(111520261));
        assert_eq!(super::parse_dated_build(10120271), Some((2027, 1, 1, 1)), "January loses the leading zero");
        assert!(build_order(10120271) > build_order(123120269), "Jan 1 2027 after Dec 31 2026");
        assert!(build_order(1115202610) > build_order(111520269), "10th build of the day");
        assert!(build_order(101202712) > build_order(10120271), "Jan 1 2027: 12th after 1st");
        assert_eq!(build_order(13), 13, "old plain numbers stay as they are");
    }
}

const fn parse_u32(s: &str) -> u32 {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut n = 0u32;
    while i < bytes.len() {
        let d = bytes[i];
        if d >= b'0' && d <= b'9' {
            n = n * 10 + (d - b'0') as u32;
        }
        i += 1;
    }
    n
}

/// Broadcast frame size. Everything (map, overlays, graphics, recordings) is produced at this
/// size and scaled to fit wherever it is shown.
pub const FRAME_W: u32 = 1920;
pub const FRAME_H: u32 = 1080;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct GeoView {
    pub lat: f64,
    pub lon: f64,
    pub zoom: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Bookmark {
    pub name: String,
    pub view: GeoView,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Basemap {
    BroadcastDark,
    Light,
    Voyager,
    Satellite,
    Terrain,
}

impl Basemap {
    pub const ALL: [Basemap; 5] = [
        Basemap::BroadcastDark,
        Basemap::Light,
        Basemap::Voyager,
        Basemap::Satellite,
        Basemap::Terrain,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Basemap::BroadcastDark => "Broadcast Dark",
            Basemap::Light => "Light",
            Basemap::Voyager => "Voyager",
            Basemap::Satellite => "Satellite",
            Basemap::Terrain => "Terrain",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Units {
    Imperial,
    Metric,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecordFormat {
    Mp4,
    Gif,
}

/// Station branding used by templates (`$primary`, `$secondary`, …) and the on-air bugs.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Brand {
    pub primary: String,
    pub secondary: String,
    pub panel: String,
    pub text: String,
    pub muted: String,
    pub alert: String,
    pub warning: String,
    pub font_display: String,
    pub font_body: String,
    /// Optional logo image (a file in the media library).
    pub logo_media: Option<String>,
}

impl Default for Brand {
    fn default() -> Self {
        Self {
            primary: "#2ED3F3".into(),
            secondary: "#222635".into(),
            panel: "#161A26".into(),
            text: "#F5F7FA".into(),
            muted: "#9AA3B2".into(),
            alert: "#E5383B".into(),
            warning: "#F5A524".into(),
            font_display: "Bahnschrift".into(),
            font_body: "Segoe UI".into(),
            logo_media: None,
        }
    }
}

/// Broadcast looks: every on-air box, banner, legend, call-out and template takes these colours.
pub fn brand_themes() -> Vec<(&'static str, Brand)> {
    let make = |primary: &str, secondary: &str, panel: &str, text: &str, muted: &str, alert: &str, warning: &str| Brand {
        primary: primary.into(),
        secondary: secondary.into(),
        panel: panel.into(),
        text: text.into(),
        muted: muted.into(),
        alert: alert.into(),
        warning: warning.into(),
        ..Brand::default()
    };
    vec![
        ("OpenWX Cyan", make("#2ED3F3", "#222635", "#161A26", "#F5F7FA", "#9AA3B2", "#E5383B", "#F5A524")),
        ("Storm Red", make("#E5383B", "#2A1215", "#1C0D10", "#FFFFFF", "#C9A3A6", "#FF4D4F", "#F5A524")),
        ("Network Blue", make("#3B82F6", "#0F1D3D", "#0B1530", "#FFFFFF", "#A5B4CF", "#E5383B", "#F59E0B")),
        ("Emerald", make("#22C55E", "#0F2A1C", "#0B1F15", "#F0FFF5", "#9DC4AE", "#EF4444", "#EAB308")),
        ("Sunset", make("#F97316", "#2B1608", "#1F1007", "#FFF7ED", "#D6B59A", "#DC2626", "#FACC15")),
        ("Royal Purple", make("#A855F7", "#1E1033", "#160B26", "#FAF5FF", "#BCA6D6", "#F43F5E", "#F59E0B")),
        ("Daylight", make("#0EA5E9", "#E8EEF5", "#F4F7FB", "#0F172A", "#475569", "#DC2626", "#D97706")),
        ("High Contrast", make("#FFE600", "#000000", "#000000", "#FFFFFF", "#FFFFFF", "#FF2020", "#FFE600")),
    ]
}

/// A saved branding the station can switch to in one click (e.g. "Dubois County Weather" and
/// "OpenWX"): the names used on air and the colours, fonts and logo.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BrandProfile {
    pub name: String,
    pub station_name: String,
    pub station_short: String,
    pub tagline: String,
    pub brand: Brand,
}

/// One sponsor in the rotation (Settings > Sponsor).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Sponsor {
    pub name: String,
    pub tagline: String,
    pub logo_media: Option<String>,
}

/// Current settings layout version (see `Settings::migrate`).
pub const SETTINGS_VERSION: u32 = 2;

/// An extra spot watched for warnings besides home (a second town, a school, an event venue).
/// Settings > Coverage area: the counties the station covers (like the website's Admin >
/// Social Graphics "Supported counties"). Empty = the home county only, as before.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CoverageArea {
    /// "Southern Indiana" (blank = "<home> & surrounding counties").
    pub name: String,
    /// State abbreviations shown on the county map ("IN", "KY").
    pub states: Vec<String>,
    /// 5-digit FIPS of the home county (blank = the county holding the home point).
    pub home_fips: String,
    pub counties: Vec<CoverageCounty>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CoverageCounty {
    /// 5-digit county FIPS.
    pub fips: String,
    /// "Dubois County"
    pub name: String,
    /// "IN"
    pub state: String,
    /// Town shown for this county; blank = the NWS place nearest the county's centre.
    pub town: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AlertZone {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    /// Radius around the spot, miles.
    pub miles: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Base URL of the backend (OpenWX Relay) the app reads in this session. Set at launch from
    /// `backend_mode`: the bundled relay's `http://127.0.0.1:<port>`, `remote_api_base`, or
    /// empty for Direct (each feed's own fallback).
    pub api_base: String,
    /// "local" (the relay runs inside the app on this PC, the default), "remote" (a hosted
    /// OpenWX Relay at `remote_api_base`) or "direct" (no relay; troubleshooting only).
    pub backend_mode: String,
    /// A hosted relay: `https://server` or `https://server/t/<access token>`.
    pub remote_api_base: String,
    /// Port for the bundled relay (the next free one is used if it is busy).
    pub relay_port: u16,
    /// Station contact sent to NOAA/NWS in every request (their terms ask for one).
    pub contact: String,
    /// Radar composite server (LibreWXR): the public API, or a self-hosted LibreWXR address.
    pub radar_upstream: String,
    /// Optional site-specific features (a website's own studio API, sponsors, posting through a
    /// website). Off in the community edition; plug-ins turn it on.
    pub site_integration: bool,
    /// The first-run setup has been completed (or skipped).
    pub setup_complete: bool,
    pub station_name: String,
    pub station_short: String,
    pub tagline: String,
    /// Sponsor for overlays, storm tracks and the sign-off ({{sponsor}}, {{sponsor_line}}).
    pub sponsor_name: String,
    pub sponsor_tagline: String,
    /// Sponsor logo (a file in the media library) for {{sponsor_logo}}.
    pub sponsor_logo_media: Option<String>,
    pub home: GeoView,
    pub home_label: String,
    pub brand: Brand,
    pub units: Units,
    pub clock_24h: bool,

    // Map defaults
    pub basemap: Basemap,
    pub radar_palette: String,
    pub radar_opacity: f32,
    pub radar_smoothing: bool,
    pub loop_frame_ms: u64,
    pub loop_dwell_ms: u64,
    pub include_nowcast: bool,
    pub bookmarks: Vec<Bookmark>,

    // On-air elements
    pub show_logo_bug: bool,
    /// Graphics show the station logo (Branding) in place of the station name wherever a text
    /// layer is just the name ({{station}} / {{station_short}}). Falls back to the name when no
    /// logo is set.
    pub logo_on_graphics: bool,
    /// Social graphics (Graphics Studio > Social Graphics): last design, theme and size used.
    pub social: crate::social_graphics::Prefs,
    pub show_clock_bug: bool,
    pub show_legend: bool,
    pub show_timestamp: bool,

    // Data
    pub poll_seconds: u64,
    /// Download daily records and past-year climate history (NOAA ACIS station data).
    pub fetch_legacy_climate_history: bool,
    /// Map Studio stages changes in Preview; Program only changes on Take.
    pub map_preview_take: bool,
    /// NWS alert area for map polygons: "" = whole US, or comma separated state codes.
    pub alert_area: String,

    // Output / recording
    pub output_always_on_top: bool,
    pub record_format: RecordFormat,
    pub record_fps: u32,

    // Severe weather
    /// Coverage radius around home (miles): warnings inside it trigger sounds and the queue.
    pub coverage_miles: f64,
    pub alert_sounds: bool,
    /// Turn on Severe Weather Mode automatically for new tornado / severe warnings in coverage.
    pub auto_severe_mode: bool,
    /// Only tornado warnings and emergencies turn Severe Mode on automatically.
    pub auto_severe_tornado_only: bool,
    /// Severe Weather Mode adds 10/25/50-mile distance rings around home.
    pub severe_range_rings: bool,
    /// Storm labels on air use calm plain wording ("Strong storm", "Heavy rain") instead of the
    /// feed's intensity words ("Very heavy storm").
    pub calm_storm_labels: bool,
    /// Optional: repaint radar snow with a pale-blue-to-violet ramp instead of the TWC palette.
    /// Off by default: the owner wants TWC (LibreWXR palette 4) colours. Renamed from
    /// `modern_snow_radar` so build 11 settings (saved on) start off.
    pub snow_radar_repaint: bool,
    /// Mobile app bridge: the YouTube channel id (UC…) whose newest uploads are listed, and the
    /// channel's live link, in /public/shows.json.
    pub youtube_channel_id: String,
    pub youtube_live_url: String,
    /// Extra spots watched for warnings besides the home coverage area.
    pub alert_zones: Vec<AlertZone>,
    /// Supported counties (Settings > Coverage area); the source of the coverage towns.
    pub coverage: CoverageArea,
    /// Mute warning sounds overnight.
    pub quiet_hours: bool,
    /// Quiet hours start and end, local hour 0-23 (may wrap past midnight).
    pub quiet_start: u32,
    pub quiet_end: u32,
    /// Tornado warnings and emergencies still sound during quiet hours.
    pub quiet_allow_urgent: bool,

    // Broadcast control (OBS obs-websocket v5)
    pub obs_enabled: bool,
    pub obs_url: String,
    pub obs_password: String,
    pub obs_severe_scene: String,
    pub obs_normal_scene: String,

    // Updates
    pub update_feed: String,
    pub auto_check_updates: bool,
    pub skipped_version: Option<String>,
    /// "stable" or "beta" (beta reads latest-beta.json next to the stable feed).
    pub update_channel: String,

    // Settings layout version, for migrations.
    pub settings_version: u32,

    // Control server (OBS browser source + automation API, 127.0.0.1 only)
    pub control_server: bool,
    pub control_port: u16,

    // Performance
    /// Lower frame rate, no wind particles, smaller caches.
    pub low_resource: bool,
    /// Warn and free caches above this much memory (MB).
    pub memory_limit_mb: u32,
    /// Map tile cache size cap (MB), trimmed at start.
    pub tile_cache_mb: u64,

    // Sponsors
    /// Extra sponsors rotated with the main one ({{sponsor}} etc. follow the rotation).
    pub sponsors: Vec<Sponsor>,
    /// Minutes per sponsor in the rotation (0 = no rotation).
    pub sponsor_rotate_minutes: u32,

    // Automation
    /// Graphics (ids) exported as PNG on a schedule (for websites, OBS image sources, socials).
    pub scheduled_exports: Vec<String>,
    /// Minutes between scheduled exports (0 = off).
    pub scheduled_export_minutes: u32,

    // Workspace
    /// Favourite Data Discovery products (names).
    pub favorites: Vec<String>,
    /// Recently used Data Discovery products (names, newest first).
    pub recent_products: Vec<String>,

    /// Use OBS studio mode: Severe Mode puts the scene in preview and transitions.
    pub obs_studio_mode: bool,
    /// Listen to MIDI pads / keyboards.
    pub midi_enabled: bool,
    /// Also export the map as Map.png with the scheduled exports.
    pub scheduled_map_snapshot: bool,

    // Operations (warning desk, automation, captions)
    /// Name written to acknowledgments and the audit log.
    pub operator_name: String,
    /// Shared folder for warning status between workstations (empty = this PC only).
    pub sync_folder: String,
    /// Webhook for notifications (Slack / Teams / Discord compatible JSON), empty = desktop only.
    pub notify_webhook: String,
    /// Webhook fired when a warning goes on air (GPO / automation).
    pub gpo_webhook: String,
    /// Webhook fired when the Program output changes (tally).
    pub tally_webhook: String,
    /// Bitfocus Companion base URL and button locations ("page/row/column").
    pub companion_url: String,
    pub companion_cut: String,
    pub companion_auto: String,
    pub companion_severe: String,
    /// TriCaster base URL (http://host), empty = off.
    pub tricaster_url: String,
    pub osc_enabled: bool,
    pub osc_port: u16,
    pub mos_enabled: bool,
    pub mos_port: u16,
    pub mos_id: String,
    /// CAP 1.2 feed (Atom or single message) polled every 2 minutes; empty = off.
    pub cap_feed_url: String,
    /// Preferred CAP info language ("en", "es").
    pub cap_language: String,
    /// Extra trusted CAP senders (besides NWS).
    pub trusted_senders: Vec<String>,
    /// Days to keep logs, archives and recordings metadata (0 = forever).
    pub retention_days: u32,
    /// OBS input watched for audio silence.
    pub obs_mic_input: String,
    /// Timed command queue (Show > Schedule).
    pub schedule: Vec<crate::broadcast_ops::Scheduled>,
    /// Live captions on the Program output.
    pub captions_enabled: bool,
    /// "bottom" or "top".
    pub caption_position: String,
    pub caption_size: f32,
    /// A new tornado warning / emergency in coverage zooms the map to it and calls it out
    /// (only while the live map is on air or nothing is).
    pub auto_zoom_threat: bool,
    /// Saved places in named collections ("Schools", "Stadiums", "Storm spotter spots").
    pub saved_places: Vec<SavedPlace>,
    /// Custom names that find a spot ("the dome", "county fairgrounds").
    pub place_aliases: Vec<SavedPlace>,
    /// Places picked from search, newest first (up to 15).
    pub search_history: Vec<SavedPlace>,
    /// Camera move easing: "smooth", "gentle", "ease-out", "ease-in" or "linear".
    pub camera_easing: String,
    /// Multiplies every camera move's duration (0.6 fast … 1.6 slow).
    pub camera_speed: f32,
    /// Zoom out a little during long moves.
    pub camera_dip: bool,
    /// Shared graphics collection folder (network drive / OneDrive), synced from Graphics Studio.
    pub shared_graphics_folder: String,
    /// Weather / traffic cameras (still-image URLs), refreshed every minute into Media.
    pub webcams: Vec<crate::travel::WebCam>,
    /// Highway travel-time routes (empty = home to the four largest towns nearby).
    pub travel_routes: Vec<crate::travel::TravelRoute>,
    /// On-air theme per graphics channel (channel number, theme name); empty = the main brand.
    pub channel_themes: Vec<(usize, String)>,
    /// Fetch upper-air and ground data with the model grid for the severe and winter
    /// ingredient maps (STP, SCP, lapse rates, wet-bulb zero, snow squalls…). Doubles the
    /// Open-Meteo request weight.
    pub severe_grid: bool,
    /// Viewer portal (photos, video, reports, questions, polls) on its own port, all interfaces.
    pub portal_enabled: bool,
    pub portal_port: u16,
    /// Automated broadcast: segment keys in order ("current", "seven_day", "radar", "regional",
    /// "hourly_strip", "severe_outlook", …), seconds per segment, station ID every N minutes
    /// (0 = never), a conditions ticker, and spoken bulletins every cycle.
    pub autocast_segments: Vec<String>,
    pub autocast_seconds: f32,
    pub autocast_id_minutes: u32,
    pub autocast_ticker: bool,
    pub autocast_bulletin: bool,
    /// Audio: output for graphic soundtracks and cues ("" = default), output for alert tones
    /// ("" = same), input for meters and voice-over ("" = default).
    pub audio_program_device: String,
    pub audio_alert_device: String,
    pub audio_input_device: String,
    /// Media file played for warning alerts instead of the system sound ("" = system sound).
    pub alert_sound_file: String,
    /// Soundtrack volume (0-1) and delay to line up with the video path, ms.
    pub graphic_volume: f32,
    pub presentation_muted: bool,
    pub audio_offset_ms: u32,
    /// Duck soundtracks when the input (presenter mic) is louder than this, dBFS (0 = off).
    pub duck_threshold_db: f32,
    /// Fetch air quality and smoke (PM2.5) with the model grid.
    pub air_quality: bool,
    /// Lower radar tile resolution and loop length automatically when the PC or the network
    /// struggles, and back again when they recover.
    pub adaptive_quality: bool,
    /// Graphics must be approved (Graphics Studio > Approve) before they can go to air.
    pub require_approval: bool,
    /// Calling out a warning also puts its lower third on the map.
    pub callout_lower_third: bool,
    /// When warnings end: clear an expired call-out and leave severe mode once none remain.
    pub return_after_warning: bool,
    /// Publishing targets (folders, CDN / storage uploads, Discord, Slack) and schedules.
    pub publish_targets: Vec<crate::publish::Target>,
    pub publish_schedules: Vec<crate::publish::Schedule>,
    /// Public image / data / widget endpoints on the viewer portal.
    pub public_endpoints: bool,
    /// Access key for public endpoints (empty = open).
    pub public_key: String,
    /// Re-render public images after this many seconds (or when the data refreshes).
    pub public_cache_secs: u32,
    /// Library graphics that may be served on /public/graphic.png.
    pub public_graphics: Vec<String>,
    /// Utility outage maps (Kubra StormCenter addresses or JSON feeds).
    pub outage_sources: Vec<crate::outages::Source>,
    /// State FIPS code for the county outage map ("18" = Indiana, empty = all).
    pub outage_state: String,
    /// Look new graphics are built in: "modern" (default) or "classic".
    pub graphics_look: String,
    /// Graphics Studio: starred template keys, and recently opened templates (newest first).
    pub graphics_favorites: Vec<String>,
    pub graphics_recent_templates: Vec<String>,
    /// Saved brandings to switch between, and the one in use.
    pub brand_profiles: Vec<BrandProfile>,
    pub active_brand: String,
    /// Automation failover: during a show, skip scenes whose data is stale or missing and fall
    /// back to the backup scene if Program goes black.
    pub failover_enabled: bool,
    /// Saved switcher setups (studio A, remote truck…).
    pub switcher_profiles: Vec<SwitcherProfile>,
    /// Data sources (registry names) kept off web exports and control-server pages.
    pub no_redistribute: Vec<String>,

    // Accessibility
    /// Scale for on-air graphic text (1.0 = as designed).
    pub onair_text_scale: f32,
    /// Graphics language for built-in wording: "en" or "es".
    pub graphics_language: String,
}

/// A place kept by the station: in a collection, as an alias, or in the search history.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SavedPlace {
    /// Collection name (saved places) or the alias text (aliases).
    pub group: String,
    pub label: String,
    pub lat: f64,
    pub lon: f64,
}

/// A named switcher / automation setup.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SwitcherProfile {
    pub name: String,
    pub companion_url: String,
    pub companion_cut: String,
    pub companion_auto: String,
    pub companion_severe: String,
    pub tricaster_url: String,
    pub obs_enabled: bool,
    pub obs_studio_mode: bool,
    pub obs_severe_scene: String,
    pub obs_normal_scene: String,
    pub tally_webhook: String,
}

impl SwitcherProfile {
    pub fn capture(name: &str, s: &Settings) -> Self {
        Self {
            name: name.to_string(),
            companion_url: s.companion_url.clone(),
            companion_cut: s.companion_cut.clone(),
            companion_auto: s.companion_auto.clone(),
            companion_severe: s.companion_severe.clone(),
            tricaster_url: s.tricaster_url.clone(),
            obs_enabled: s.obs_enabled,
            obs_studio_mode: s.obs_studio_mode,
            obs_severe_scene: s.obs_severe_scene.clone(),
            obs_normal_scene: s.obs_normal_scene.clone(),
            tally_webhook: s.tally_webhook.clone(),
        }
    }

    pub fn apply(&self, s: &mut Settings) {
        s.companion_url = self.companion_url.clone();
        s.companion_cut = self.companion_cut.clone();
        s.companion_auto = self.companion_auto.clone();
        s.companion_severe = self.companion_severe.clone();
        s.tricaster_url = self.tricaster_url.clone();
        s.obs_enabled = self.obs_enabled;
        s.obs_studio_mode = self.obs_studio_mode;
        s.obs_severe_scene = self.obs_severe_scene.clone();
        s.obs_normal_scene = self.obs_normal_scene.clone();
        s.tally_webhook = self.tally_webhook.clone();
    }
}

impl Settings {
    /// The alert sound file from the saved settings (read at most every 30 s).
    pub fn load_cached_alert_sound() -> Option<PathBuf> {
        static CACHE: std::sync::Mutex<Option<(std::time::Instant, String)>> = std::sync::Mutex::new(None);
        let mut c = CACHE.lock().ok()?;
        if c.as_ref().map(|(t, _)| t.elapsed().as_secs() > 30).unwrap_or(true) {
            *c = Some((std::time::Instant::now(), Settings::load().alert_sound_file));
        }
        let name = c.as_ref()?.1.trim().to_string();
        (!name.is_empty()).then(|| LibraryPaths::get().media.join(name))
    }

    /// Bring settings saved by an older build up to the current layout.
    pub fn migrate(mut self) -> Self {
        if self.settings_version < 1 {
            // Build 1 stored no CARTO-free basemap; any old CARTO URL is replaced by Esri at style time.
            self.settings_version = 1;
        }
        if self.settings_version < 2 {
            // Build 3: control server, sponsors list, performance and accessibility settings.
            if self.control_port == 0 {
                self.control_port = crate::ops::DEFAULT_PORT;
            }
            if self.onair_text_scale <= 0.0 {
                self.onair_text_scale = 1.0;
            }
            if self.update_channel.is_empty() {
                self.update_channel = "stable".into();
            }
            self.settings_version = 2;
            log::info!("settings migrated to version {SETTINGS_VERSION}");
        }
        self
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            api_base: String::new(),
            backend_mode: "local".into(),
            remote_api_base: String::new(),
            relay_port: openwx_relay::DEFAULT_PORT,
            contact: String::new(),
            radar_upstream: openwx_relay::config::DEFAULT_RADAR_UPSTREAM.into(),
            site_integration: false,
            setup_complete: false,
            fetch_legacy_climate_history: true,
            map_preview_take: false,
            station_name: "Your Station Weather".into(),
            station_short: "Your Station".into(),
            tagline: "Your Weather. Your Community.".into(),
            sponsor_name: String::new(),
            sponsor_tagline: String::new(),
            sponsor_logo_media: None,
            // Geographic centre of the contiguous U.S. until the setup wizard sets the station's home.
            home: GeoView {
                lat: 39.8283,
                lon: -98.5795,
                zoom: 4.2,
            },
            home_label: "United States".into(),
            brand: Brand::default(),
            units: Units::Imperial,
            clock_24h: false,
            basemap: Basemap::BroadcastDark,
            radar_palette: "4".into(),
            radar_opacity: 0.85,
            radar_smoothing: true,
            loop_frame_ms: 450,
            loop_dwell_ms: 1600,
            include_nowcast: false,
            bookmarks: vec![
                // The setup wizard adds the station's own home, county and region views.
                Bookmark {
                    name: "CONUS".into(),
                    view: GeoView { lat: 38.5, lon: -96.5, zoom: 3.4 },
                },
            ],
            show_logo_bug: true,
            logo_on_graphics: true,
            social: Default::default(),
            show_clock_bug: true,
            show_legend: true,
            show_timestamp: true,
            poll_seconds: 120,
            alert_area: String::new(),
            output_always_on_top: false,
            record_format: RecordFormat::Mp4,
            record_fps: 30,
            coverage_miles: 75.0,
            alert_sounds: true,
            auto_severe_mode: false,
            auto_severe_tornado_only: false,
            severe_range_rings: false,
            calm_storm_labels: true,
            snow_radar_repaint: false,
            youtube_channel_id: String::new(),
            youtube_live_url: String::new(),
            alert_zones: Vec::new(),
            coverage: CoverageArea::default(),
            quiet_hours: false,
            quiet_start: 22,
            quiet_end: 6,
            quiet_allow_urgent: true,
            obs_enabled: false,
            obs_url: "ws://127.0.0.1:4455".into(),
            obs_password: String::new(),
            obs_severe_scene: "Severe Weather".into(),
            obs_normal_scene: "Weather".into(),
            update_feed: "https://github.com/jdc20181/OpenWX-Broadcasting/releases/latest/download/latest.json".into(),
            auto_check_updates: true,
            skipped_version: None,
            update_channel: "stable".into(),
            settings_version: SETTINGS_VERSION,
            control_server: true,
            control_port: crate::ops::DEFAULT_PORT,
            low_resource: false,
            memory_limit_mb: 3000,
            tile_cache_mb: 2000,
            sponsors: Vec::new(),
            sponsor_rotate_minutes: 0,
            scheduled_exports: Vec::new(),
            scheduled_export_minutes: 0,
            favorites: Vec::new(),
            recent_products: Vec::new(),
            onair_text_scale: 1.0,
            obs_studio_mode: false,
            midi_enabled: true,
            scheduled_map_snapshot: true,
            graphics_language: "en".into(),
            operator_name: std::env::var("USERNAME").unwrap_or_else(|_| "Operator".into()),
            sync_folder: String::new(),
            notify_webhook: String::new(),
            gpo_webhook: String::new(),
            tally_webhook: String::new(),
            companion_url: "http://127.0.0.1:8000".into(),
            companion_cut: String::new(),
            companion_auto: String::new(),
            companion_severe: String::new(),
            tricaster_url: String::new(),
            osc_enabled: false,
            osc_port: 9000,
            mos_enabled: false,
            mos_port: 10540,
            mos_id: "OPENWX".into(),
            cap_feed_url: String::new(),
            cap_language: "en".into(),
            trusted_senders: Vec::new(),
            retention_days: 90,
            obs_mic_input: "Mic/Aux".into(),
            schedule: Vec::new(),
            captions_enabled: false,
            caption_position: "bottom".into(),
            caption_size: 1.0,
            no_redistribute: Vec::new(),
            switcher_profiles: Vec::new(),
            require_approval: false,
            callout_lower_third: true,
            return_after_warning: true,
            publish_targets: Vec::new(),
            publish_schedules: Vec::new(),
            public_endpoints: false,
            public_key: String::new(),
            public_cache_secs: 120,
            public_graphics: Vec::new(),
            outage_sources: Vec::new(),
            outage_state: "18".into(),
            graphics_look: "modern".into(),
            graphics_favorites: Vec::new(),
            graphics_recent_templates: Vec::new(),
            brand_profiles: Vec::new(),
            active_brand: String::new(),
            adaptive_quality: true,
            severe_grid: false,
            portal_enabled: false,
            portal_port: 8766,
            autocast_segments: vec!["current".into(), "radar".into(), "seven_day".into(), "regional".into(), "hourly_strip".into()],
            autocast_seconds: 20.0,
            autocast_id_minutes: 30,
            autocast_ticker: true,
            autocast_bulletin: false,
            audio_program_device: String::new(),
            audio_alert_device: String::new(),
            audio_input_device: String::new(),
            alert_sound_file: String::new(),
            graphic_volume: 0.8,
            presentation_muted: false,
            audio_offset_ms: 0,
            duck_threshold_db: 0.0,
            air_quality: false,
            webcams: Vec::new(),
            shared_graphics_folder: String::new(),
            camera_easing: "smooth".into(),
            saved_places: Vec::new(),
            auto_zoom_threat: false,
            place_aliases: Vec::new(),
            search_history: Vec::new(),
            camera_speed: 1.0,
            camera_dip: true,
            travel_routes: Vec::new(),
            channel_themes: Vec::new(),
            failover_enabled: true,
        }
    }
}

/// Portable mode: a file named `portable.txt` next to the program keeps settings, the library
/// and caches in a `Data` folder beside it (run from a USB stick, nothing written elsewhere).
pub fn portable_root() -> Option<PathBuf> {
    static ROOT: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?;
        dir.join("portable.txt").exists().then(|| dir.join("Data"))
    })
    .clone()
}

pub fn config_dir() -> PathBuf {
    if let Some(root) = portable_root() {
        return root.join("Settings");
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_NAME)
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

/// Disk cache for map tiles (HTTP cache, honours Cache-Control).
pub fn tile_cache_dir() -> PathBuf {
    if let Some(root) = portable_root() {
        return root.join("Cache").join("tiles");
    }
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_NAME)
        .join("tiles")
}

/// Application log (the app has no console window).
pub fn log_path() -> PathBuf {
    // A portable copy (a dev build) keeps its own log so it never mixes into the installed app's.
    if let Some(root) = portable_root() {
        return root.join("Cache").join("logs").join("openwx-broadcaster.log");
    }
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_NAME)
        .join("logs")
        .join("openwx-broadcaster.log")
}

/// Where downloaded updates are staged.
pub fn update_dir() -> PathBuf {
    if let Some(root) = portable_root() {
        return root.join("Cache").join("updates");
    }
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_NAME)
        .join("updates")
}

impl Settings {
    /// The branding in use, as a profile.
    pub fn current_brand_profile(&self) -> BrandProfile {
        BrandProfile {
            name: if self.active_brand.is_empty() { self.station_name.clone() } else { self.active_brand.clone() },
            station_name: self.station_name.clone(),
            station_short: self.station_short.clone(),
            tagline: self.tagline.clone(),
            brand: self.brand.clone(),
        }
    }

    /// Saved brandings, always including the one in use and a OpenWX branding.
    pub fn brand_profiles_or_defaults(&self) -> Vec<BrandProfile> {
        let mut out = self.brand_profiles.clone();
        let current = self.current_brand_profile();
        if !out.iter().any(|p| p.name == current.name) {
            out.insert(0, current.clone());
        }
        if !out.iter().any(|p| p.name.eq_ignore_ascii_case("OpenWX")) {
            out.push(BrandProfile {
                name: "OpenWX".into(),
                station_name: "OpenWX".into(),
                station_short: "OpenWX".into(),
                tagline: current.tagline.clone(),
                brand: Brand { logo_media: None, ..current.brand.clone() },
            });
        }
        out
    }

    /// Switch to a saved branding. The branding in use is saved first, so switching back
    /// restores it exactly (including edits made since).
    pub fn switch_brand(&mut self, name: &str) {
        let mut profiles = self.brand_profiles_or_defaults();
        let current = self.current_brand_profile();
        if let Some(p) = profiles.iter_mut().find(|p| p.name == current.name) {
            *p = current;
        }
        if let Some(target) = profiles.iter().find(|p| p.name == name).cloned() {
            self.station_name = target.station_name;
            self.station_short = target.station_short;
            self.tagline = target.tagline;
            self.brand = target.brand;
            self.active_brand = target.name;
        }
        self.brand_profiles = profiles;
    }

    pub fn load() -> Self {
        match std::fs::read_to_string(settings_path()) {
            Ok(text) => match serde_json::from_str::<Settings>(&text) {
                Ok(s) => s.migrate(),
                Err(e) => {
                    log::warn!("settings.json unreadable, using defaults: {e}");
                    // Keep the unreadable file for support instead of overwriting it silently.
                    let _ = std::fs::copy(settings_path(), config_dir().join("settings.unreadable.json"));
                    Settings::default()
                }
            },
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let dir = config_dir();
        std::fs::create_dir_all(&dir)?;
        let tmp = dir.join("settings.json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(tmp, settings_path())?;
        Ok(())
    }

    /// Update feed for the chosen channel (beta reads latest-beta.json beside latest.json).
    pub fn update_feed_for_channel(&self) -> String {
        if self.update_channel == "beta" {
            match self.update_feed.rsplit_once('/') {
                Some((base, _)) => format!("{base}/latest-beta.json"),
                None => self.update_feed.clone(),
            }
        } else {
            self.update_feed.clone()
        }
    }

    pub fn api_url(&self, path: &str) -> String {
        format!("{}{}", self.api_base.trim_end_matches('/'), path)
    }
}

/// Library folders in Documents\OpenWXBroadcasting.
#[derive(Clone, Debug)]
pub struct LibraryPaths {
    pub root: PathBuf,
    pub shows: PathBuf,
    pub graphics: PathBuf,
    pub media: PathBuf,
    pub exports: PathBuf,
    pub trash: PathBuf,
}

impl LibraryPaths {
    pub fn get() -> Self {
        let root = match portable_root() {
            Some(r) => r.join("Library"),
            None => dirs::document_dir().unwrap_or_else(|| PathBuf::from(".")).join(APP_NAME),
        };
        let paths = Self {
            shows: root.join("Shows"),
            graphics: root.join("Graphics"),
            media: root.join("Media"),
            exports: root.join("Exports"),
            trash: root.join(".trash"),
            root,
        };
        for dir in [
            &paths.root,
            &paths.shows,
            &paths.graphics,
            &paths.media,
            &paths.exports,
            &paths.trash,
        ] {
            let _ = std::fs::create_dir_all(dir);
        }
        paths
    }
}
