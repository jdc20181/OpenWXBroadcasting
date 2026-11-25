# How OpenWX works

OpenWX Broadcasting is three programs that ship together.

| Part | What it does |
|---|---|
| **OpenWX Broadcasting** (`app/`) | The studio: Map Studio, Graphics Studio, Show Builder, Severe Weather Mode, Winter Studio, Program output to a second screen, OBS workflows, recording and export |
| **OpenWX Relay** (`relay/`) | The small Rust backend between the studio and public weather data. It runs inside the app on your PC (on `127.0.0.1:8737`). You can also host it on your own server for several studio PCs: see [server/README.md](../server/README.md) |
| **`openwx`** (`cli/`) | Setup from a terminal (`openwx setup`), feed checks, and an [MCP server](../prompts/README.md) so Claude and other AI assistants can read your station's weather and help write posts and briefings |

## Data sources

OpenWX reads free public data directly from the agencies that publish it:

- National Weather Service: alerts, forecasts, observations
- Storm Prediction Center, Weather Prediction Center, Climate Prediction Center, National Hurricane Center
- NOAA NOMADS model files: GFS, HRRR, RAP, NBM
- NWPS river gauges
- NCEI and RCC-ACIS climate data
- U.S. Drought Monitor
- USGS earthquakes
- Iowa Environmental Mesonet
- NASA GIBS satellite imagery
- SWPC space weather
- FAA airport status
- LibreWXR radar composites (public server or your own; Settings > Weather)

OpenWX sends your station's contact with each request, as the NWS asks, and never caches live
weather. Third-party licences are linked in [licensing.md](licensing.md).

## More design notes

| Document | Covers |
|---|---|
| [backend.md](backend.md) | The relay and how the studio talks to it |
| [setup-wizard.md](setup-wizard.md) | First-run setup |
| [modularity.md](modularity.md) and [decoupling.md](decoupling.md) | How the code is split up |
| [themes.md](themes.md) | Themes and looks |
| [bug-reports.md](bug-reports.md) | In-app bug reports |
| [crowdedwx-sync.md](crowdedwx-sync.md) | How graphics come over from CrowdedWX |
| [repo-hygiene.md](repo-hygiene.md) | What is and isn't kept in the repository |
| [../installer/README.md](../installer/README.md) | Building installers and signed updates |
