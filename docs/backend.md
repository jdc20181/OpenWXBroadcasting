# OpenWX Relay (backend server)

CrowdedWX Broadcaster talks to the Dubois County Weather site at
`https://dcweather.xyz/api/weatherbroadcaster/…` (a fixed-provider relay: the app sends a
provider ID and a path, never a raw URL). OpenWX must ship its own.

## Why a relay at all

- One place to set the NWS-required `User-Agent` contact for the station.
- Shares identical simultaneous requests (no live-data cache), and keeps immutable files
  (finished model runs, basemap tiles, past radar frames, normals).
- Hides upstream quirks (CORS, size caps, rate limits) and keeps the app's network surface small.
- Optional geometry simplification for very large polygon feeds (see the DCWeather
  Rust/Node backend plan, Phase 3).

## Hosting modes

| Mode | How it runs | For |
|---|---|---|
| **Bundled (local)** | The app starts the relay on `127.0.0.1:<port>` at launch and stops it on exit | Single-PC stations; default |
| **Remote** | Docker image + `docker compose` on the station's server, behind its HTTPS proxy | Several studio PCs, a newsroom, or a station that already runs a website |
| **Direct (no relay)** | The app calls upstreams itself (today's "legacy upstream" fallbacks) | Troubleshooting only |

Implementation choice: a small Rust backend (axum), formed and forked from today's mostly-Node
relay (`DCWeatherSite/src/routes/weatherbroadcaster.js` and the libs it calls), so the bundled
mode is one small binary with no Node runtime to install; the remote image is the same binary.
The DCWeather site's own Rust/Node plan (`DCWeatherSite/docs/RUST-NODE-BACKEND-PLAN.md`,
Phase 3) builds the same relay transforms, so the two should share one crate.

## API contract to reproduce (inventory from the current relay)

Fixed providers (ID → host, allowed paths): `nws` (api.weather.gov), `noaaMap`
(mapservices.weather.noaa.gov), `water` (api.water.noaa.gov NWPS), `spc`, `nhc` (ATCF), `space`
(SWPC), `faa` (NAS status), `drought` (USDM), `iem`, `esri` / `esriServices` / `esriTiles`,
`gibs` (NASA), `zip`, `ncei`, `dat` (NWS DAT). Open-Meteo providers (`meteo`, `air`,
`ensemble`, `archive`, `historical`, `places`) are **dropped** in OpenWX once the app no longer
uses them (see the gap audit's Open-Meteo replacement item).

Dedicated routes: `aviation/airport-status`, `aviation/metar`, `outlooks/qpf`,
`outlooks/drought`, `nws/forecast`, `climate/daily`, `climate/normals`, `model-run`,
`model-frame`, `archive-scans`, `archive-velocity-tile`, `feed`.

Not part of the relay (CrowdedWX site features): `/api/radar` (LibreWXR proxy), INDOT roads,
lightning, storm cells. Each needs a decision: a station-configurable source, a plug-in, or
removal (radar needs a replacement composite source before G3).

## Contract tests

One test per route: valid request returns the upstream shape; disallowed paths are refused;
size caps hold; immutable caching only where listed; `User-Agent` carries the station contact.

## Status (2026-11-17)

Built in `relay/` (Rust, axum). Every route above is served, plus `/api/weather/overview`,
`/api/weather/spc-outlooks`, `/api/radar`, `/api/radar/storm-cells`, `/api/public/lightning.json`,
`/health` and `/api/openwx/check`. Added providers: `usgs`, `acis`, NWS `zones` and latest
observations, Zippopotam place search. INDOT roads and the site studio API answer 404 (plug-ins).
Remote mode: `https://server/t/<access_token>`; the relay refuses a non-loopback bind without a
token. Contract tests: `cargo test -p openwx-relay`.
