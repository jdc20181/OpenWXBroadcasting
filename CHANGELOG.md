# Changelog

## Unreleased

## v0.1.0 build 2 · November 18, 2026 · test build

### Fixed

- The title bar wordmark read "CROWDEDWX BROADCASTER"; it now reads "OPENWX BROADCASTING". The
  built-in fallback logo, the map watermark and the default MOS ID are OpenWX too.

### New

- **Radar server setting** (Settings > Weather, "On this PC" backend). Radar composites come
  from LibreWXR; leave it blank for the public API or enter your own LibreWXR server.
- **Licence: OpenWX License 1.0** (`LICENSE`). Free to use, change and share under the same
  licence; closed commercial use needs a Commercial Waiver (a donation of US $500 or more).

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
