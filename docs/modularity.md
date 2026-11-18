# Modularity: from "built for one station" to "adoptable by any"

CrowdedWX Broadcaster is tailored to one owner, one coverage area and one infrastructure
(the Dubois County Weather site, its relay, its radar proxy, its Facebook setup). OpenWX must
work for a station that has none of that. The rule: **anything that names a place, a host, a
brand or a service is either a setting, a wizard answer, or a plug-in behind an interface.**

## Interfaces to introduce (each with at least one built-in implementation)

| Interface | Built-in | Replaces today |
|---|---|---|
| `Backend` (relay client) | Local bundled relay, remote relay, direct | Fixed `dcweather.xyz/api/weatherbroadcaster` |
| `RadarSource` | Configurable composite source; IEM single-site | LibreWXR through the DCWeather proxy |
| `Basemap` | Open tile source + Esri where licensed | Esri only |
| `Publisher` | Facebook (Graph API) | Site-side `facebook-alerts.js` |
| `Reporter` | Local files, GitHub Issues | Local files only |
| `CoverageArea` | Wizard: state(s), counties, towns per county | Settings > Coverage area (already close) |
| `Look` / `AppTheme` | Crisp/Modern, Classic, Light, Dark | Modern/Classic only |
| Optional feeds (`RoadFeed`, `LightningFeed`, `OutageFeed`) | Off by default; INDOT etc. as plug-ins | Hard-wired DCWeather routes |

## Rules for the fork

- Defaults ship neutral: no town, county, station or sample that names a real local place
  unless marked SAMPLE.
- Features that only make sense with the DCWeather site (community board, studio bridge,
  site social graphics) become optional plug-ins or are left out of OpenWX.
- Configuration is versioned and exportable, so a station can move to a new PC or share a
  setup with a sister station.
- Every plug-in reports health in one place (feed status list), so a station knows what is
  connected without reading logs.
