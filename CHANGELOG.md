# Changelog

## Unreleased

## v0.1.2 build 112320262 · November 23, 2026 · alpha

Second build of the day: the fixes from CrowdedWX build 112320261.

### Fixed

- **Model maps load even when the relay is busy:** each NOAA model file is tried up to four times
  before it counts as failed, and if a later hour still fails the map ends at the hour before it
  instead of showing no data at all.
- **Drought monitor:** its legend shows on air even with the radar on, and the drought shapes are
  drawn once and reused instead of every frame (the map lagged while drought was on).
- **Show map scenes no longer take over Map Studio:** after a show has played or previewed a map
  scene, going back to Map Studio puts your own map back. "Load into Map Studio" still loads the
  scene on purpose.
- **Show export no longer freezes the app:** the video is finished in the background. Keep the app
  open until it says the recording is saved, or the MP4 will be cut short.

## v0.1.2 build 112320261 · November 23, 2026 · alpha

Builds are now numbered by date like CrowdedWX: month, day, year, then the build of that day
(112320261 = November 23, 2026, first build).

### New

- **Updates from GitHub:** the app checks the releases at
  https://github.com/jdc20181/OpenWXBroadcasting/releases. Stable takes the newest full release
  (or the newest prerelease while OpenWX is in alpha); Beta takes the newest release of any kind.
  Updates are signed with OpenWX's own key and checked before they install. Builds 1 to 3 pointed
  at a repository address that doesn't exist, so they never found an update; their saved setting
  is moved over automatically.
- **Graphics from CrowdedWX build 112020261 and the batches since:** model presentation graphics,
  community cards (birthday and shout-out, pet forecast) and producer story cards, plus the winter
  and seasonal outlook graphics: El Niño / La Niña status from NOAA CPC, an editable Our Winter
  Outlook card, and CPC monthly and 3-month temperature and precipitation outlooks for your region,
  state and multi-state area (16:9, portrait, square and overlay).

### Fixed

- **Distorted audio:** music, scene audio and alert tones now share one output that runs through a
  peak limiter, so several playing at once, or a volume above 100%, no longer clip.
- **Map Studio model data comes only from NOAA's own model files.** The model picker offers HRRR
  and GFS. HRRR is used on state- and region-size views; wider views, precipitation type and the
  winter and severe ingredients use NOAA GFS, and the loading message names the model actually
  used. If an HRRR file is missing, the map shows GFS for 30 minutes before trying HRRR again.
- **Big NOAA files and relay hiccups:** the SPC tornado database and full-detail WPC rainfall
  contours are read straight from NOAA, and when the relay can't serve any other public NOAA/NWS
  feed (an error, a path it doesn't support, or no answer), the app reads that feed directly and logs a warning.
- **Tornado history graphics** read SPC's current tornado database again (IDs since 2007 are text).

### Changed

- Dependencies updated to their current releases (reqwest 0.13, resvg 0.50 and tiny-skia 0.12,
  gpui-kit 0.10, rfd 0.18, minisign-verify 0.3, sha2 0.11, the GRIB, NetCDF and HDF5 readers, and the
  rest).

## v0.1.0 build 3 · November 20, 2026 · alpha

### New

- **Graphics from CrowdedWX build 111620261:** Forecast Editor cards, Multi-Day Forecast cards,
  Season Countdown cards and Viewer cards, plus updates to holiday forecasts, model maps, town
  forecast maps and weather-word graphics. New graphics now come over with
  `tools/sync-crowdedwx.py` (`docs/crowdedwx-sync.md`).

### Fixed

- **Rain, snow or wintry mix map:** town plates read RAIN, MIX or SNOW instead of a 0 or 1 code,
  and dry towns get no plate.
- Large public files (full-detail WPC rainfall contours) are no longer cut off at 16 MB.
- **Choppy music:** tracks are read into memory before they play and audio outputs use a roomier
  buffer, so playback holds up while the map is busy.
- **Holiday graphics:** the plain holiday-forecast panels no longer crowd the designed Holidays cards
  out of Graphics Studio.

### Changed

- **Licence clarified:** using OpenWX is free for everyone, including businesses, studios and
  pages that earn money from their coverage. The Commercial Waiver covers selling the software
  itself (selling it, charging for access, bundling it into a paid product, reselling it as a
  service), whether or not the source is shared.
- No GitHub Actions or Dependabot: nothing runs on GitHub; builds and tests run locally.
- `docs/licensing.md` credits third-party projects with links to their own licences.

## v0.1.0 build 2 · November 18, 2026 · test build

### Fixed

- The title bar wordmark read "CROWDEDWX BROADCASTER"; it now reads "OPENWX BROADCASTING". The
  built-in fallback logo, the map watermark and the default MOS ID are OpenWX too.

### New

- **Radar server setting** (Settings > Weather, "On this PC" backend). Radar composites come
  from LibreWXR; leave it blank for the public API or enter your own LibreWXR server.
- **Licence: OpenWX License 1.0** (`LICENSE`). Free to use, including for businesses and pages
  that earn money from their coverage, and free to change and share under the same licence.
  Selling the software itself needs a Commercial Waiver (a donation of US $500 or more).

## v0.1.0 build 1 · November 17, 2026 · first test build

The first OpenWX Broadcasting build, forked from CrowdedWX Broadcaster v0.1.9 (build 111620261)
and made station-neutral.

### New

- **Built-in backend (OpenWX Relay).** The app starts its own relay on this PC (127.0.0.1, port
  8737 or the next free one): no website or server needed. Rust port of the fixed-provider
  relay, plus current conditions and forecast, SPC outlooks (days 1-8), radar and satellite
  composites, storm cells and GOES lightning.
- **Backend choice** in Settings > Weather: On this PC (default), Hosted server, or Direct, with
  a "Check the weather feeds" button (green/red per source).
- **First-run setup wizard**: station name and tagline, NWS contact, state, counties (from the
  NWS county list), main city (the map centres there), coverage-area name, backend and a feed
  check. Run it again from Settings > Weather.
- **`openwx` command-line tool**: `setup` (the same questions in a terminal), `status`,
  `weather`, `alerts`, and `mcp`, an MCP server with tools (station, conditions and forecast,
  alerts, SPC outlook, feed check, set region, set identity) and prompts (forecast post, severe
  briefing, show rundown, station setup) for Claude and other assistants. `prompts/` holds a
  prompt library.
- **`openwx-relay` server** for stations that host their own relay: `setup`, `check`, `token`;
  access-token gate (`https://server/t/<token>`); refuses to listen publicly without a token.
- **Server install script** (`server/install.sh`): install, update with automatic rollback,
  setup, status, logs and uninstall on any systemd Linux; checksum-verified downloads;
  hardened service. Docker image and compose file.
- **Requests carry the station's contact** in the User-Agent, as the NWS asks.

### Changed from CrowdedWX

- Names, folders and window titles: OpenWX Broadcasting (`%APPDATA%\OpenWXBroadcasting`,
  `Documents\OpenWXBroadcasting`). New brand icon and wordmark.
- Neutral defaults: no station, town, county or bookmarks until setup; home starts at the
  centre of the contiguous U.S.
- Website-only features (site sign-in, sponsor import, studio bridge) are hidden unless
  `site_integration` is turned on.
- The update feed points at GitHub Releases. OpenWX's own signing key is made in the updater
  phase (`openwx-release keygen`); until then this build finds no updates.

### Known gaps

- Facebook posting dashboard, GitHub bug reports and the four app themes are planned (see
  `PLAN.md`).
- Some features still read Open-Meteo through the relay (free for non-commercial use only).
- Road conditions (INDOT) and the website's own lightning feed are not part of OpenWX.
