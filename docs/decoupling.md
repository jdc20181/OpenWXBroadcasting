# Decoupling inventory (from CrowdedWX Broadcaster v0.1.8 source, 2026-11-16)

Counts are case-insensitive matches in `src/**/*.rs`; each must become a setting, a wizard
answer, a backend setting or be removed. Re-run the counts on the fork.

| Pattern | Files | Hits | Becomes |
|---|---:|---:|---|
| `dcweather.xyz` | 12 | 24 | Backend URL setting (relay) and site-integration plug-in |
| `CrowdedWX` | 37 | 89 | Product name constant (`OpenWX`) for app chrome; station brand for anything on air |
| `Dubois` | 20 | 57 | Station settings (name, region); remove from defaults and samples |
| `Jasper` | 12 | 32 | Coverage towns from the wizard; sample fixtures say SAMPLE |
| `librewxr` | 21 | 40 | Radar source setting (needs a redistributable composite source) |
| `server.arcgisonline` | 4 | 19 | Basemap provider setting; check Esri terms for redistribution |
| `open-meteo` | 23 | 93 | Remove (NOAA sources) before G5 |

Also:

- `net::user_agent()` names the CrowdedWX build and dcweather.xyz; must carry the station's
  contact from the wizard.
- `Cargo.toml`: crate/binary names, `license = "Proprietary"`.
- Config/AppData folder names, log names, bug-report folder, window titles, installer IDs,
  updater host (`docs/UPDATE-HOSTING.md`).
- Wordmark and CrowdedWX-only UI (`ui::wordmark`, cyan reserved for the CrowdedWX wordmark).
- Site-coupled features: Social Graphics posting, community board, studio bridge, publishing
  portal defaults. Keep behind a "site integration" plug-in or remove.
- Sample data and QA fixtures that use real local places: mark SAMPLE, use neutral places.
