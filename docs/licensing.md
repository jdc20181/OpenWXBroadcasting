# Licensing and third-party clearance

## Licence for OpenWX itself

Decided 2026-11-18: the project's own **OpenWX License 1.0** (`LICENSE`). Anyone may use, adapt
and redistribute; redistributed and network-hosted versions keep the same licence with source
(share-alike); closed commercial use needs a Commercial Waiver, granted for a donation of US $500
or more; third-party components keep their own licences; no warranty or liability. Because it
limits commercial use it is "source-available", not OSI-approved "open source"; GitHub shows
it as "Other". Have it reviewed by a lawyer before the public release. CrowdedWX Broadcaster
stays proprietary.

## Inventory to clear before G5

| Item | Where | Status to confirm |
|---|---|---|
| maplibre-rs (vendored) | `vendor/maplibre-rs` | Upstream licence and our modifications' notices |
| GPUI / gpui-component / gpui-kit | Cargo dependencies | Licences of each and their transitive crates (`cargo about` or `cargo deny`) |
| Remix Icon | `assets/icons` | Apache-2.0 (attribution in About) |
| Meteocons | Modern weather icons | MIT (already credited) |
| Fonts | Brand fonts per station; bundled defaults | Ship only OFL/Apache fonts; stations add their own |
| Music | `assets/music` (e.g. CC0 tracks) | Keep only CC0/royalty-free with recorded sources (`docs/SHOW-AUDIO-SOURCES.md`) |
| Basemap tiles | Esri | Terms for redistributed apps; provide an open alternative |
| Radar composite | LibreWXR via the CrowdedWX site | Needs a source OpenWX may use |
| Boundaries | US states/counties GeoJSON (downloaded) | Source licence (public domain Census-derived?) |
| Data sources | NWS, SPC, WPC, CPC, NHC, SWPC, NWPS, NCEI, ACIS, IEM, USGS, NASA GIBS, FAA, USDM | Attribution and acceptable-use terms (IEM and USDM ask for credit) |

## Updates found during the fork (2026-11-17)

| Item | Finding | Action |
|---|---|---|
| Starter music beds (5 MP3s, `app/assets/music/`) | Recorded as CC0 in CrowdedWX `docs/SHOW-AUDIO-SOURCES.md` | Committed through Git LFS; keep the source links |
| Abstraction loop bundle | CC0 (`abstraction/LICENSE.txt`) | Not committed (800 MB); bundled into the installer from a local download |
| DCWeather brand logo in the social renderer | Station brand, not redistributable | Removed from `crates/social-render/assets` |
| LibreWXR (radar, satellite, storm cells, GLM lightning) | Public API used by default (`radar_upstream`) | Owner to confirm the terms allow OpenWX stations to use it; the relay setting lets a station point elsewhere |
| Open-Meteo | Free tier is non-commercial only | Relay setting `open_meteo` (on); replace with NOAA sources before G5 |
| Update signing key | CrowdedWX key must not be reused | New key with `openwx-release keygen` (owner keeps it) |
