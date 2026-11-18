<p align="center"><img src="brand/svg/openwx-logo.svg" alt="OpenWX Broadcasting" width="420"></p>

# OpenWX Broadcasting

A broadcast weather studio for community stations: live radar maps, on-air graphics, shows,
severe weather coverage and playout, built on free public data from NOAA and the National
Weather Service. Made for small teams, volunteers and weather enthusiasts who want
professional-looking coverage without a TV station's budget or an IT department.

> **Status: first test build (v0.1.0).** Private until the public release. Expect rough edges.

> OpenWX is **not an official warning source**. For life-safety decisions, follow the National
> Weather Service and your local emergency management.

## Get started (Windows)

1. Run the installer (`OpenWX Broadcasting_0.1.0_x64-setup.exe`). No admin rights needed.
2. The setup wizard opens: name your station, add a contact for the National Weather Service,
   choose your state, your counties and your main city.
3. That's it. The weather data backend (OpenWX Relay) runs inside the app on your PC.

Prefer a terminal? `openwx setup` asks the same questions. Change anything later in **Settings**.

## What's inside

| Part | What it does |
|---|---|
| **OpenWX Broadcasting** (`app/`) | The studio: Map Studio (radar, satellite, alerts, SPC outlooks, drought, earthquakes, models), Graphics Studio (hundreds of templates), Show Builder, Severe Weather Mode, output to a second screen, OBS and NDI-style workflows |
| **OpenWX Relay** (`relay/`) | The small Rust backend between the studio and public weather data. Built into the app; or host it on your own server for several studio PCs ([server/README.md](server/README.md)) |
| **`openwx`** (`cli/`) | Setup from a terminal, feed checks, and an [MCP server](prompts/README.md) so Claude and other AI assistants can read your station's weather and help write posts and briefings |

## Data sources

National Weather Service (alerts, forecasts, observations), Storm Prediction Center, WPC, CPC,
NHC, NOAA NOMADS models (GFS, HRRR, RAP, NBM), NWPS river gauges, NCEI and RCC-ACIS climate data,
U.S. Drought Monitor, USGS earthquakes, Iowa Environmental Mesonet, NASA GIBS, SWPC space weather,
FAA airport status, and LibreWXR radar composites. OpenWX sends your station's contact with
requests, as the NWS asks, and never caches live weather.

## Building from source

Latest stable Rust (`rustup update stable`), then:

```bash
git lfs install                     # the bundled CC0 music uses Git LFS
cargo run -p openwx-broadcaster     # the studio
cargo test --workspace
```

Details: [CONTRIBUTING.md](CONTRIBUTING.md). Installer: [installer/README.md](installer/README.md).

## Project

| | |
|---|---|
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to help, ground rules, building, pull requests |
| [SECURITY.md](SECURITY.md) | Reporting vulnerabilities privately; how OpenWX is built to be safe |
| [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) | How we treat each other |
| [SUPPORT.md](SUPPORT.md) | Where to get help |
| [CHANGELOG.md](CHANGELOG.md) | Release notes |
| [PLAN.md](PLAN.md) | Road map and release gates |
| [docs/](docs/) | Design docs: backend, setup wizard, modularity, themes, licensing, repo hygiene |
| [brand/](brand/BRAND.md) | Logo, icon and colours |

Licence: [OpenWX License 1.0](LICENSE). Free to use, change and share; anything you share keeps
the same licence and its source. Closed commercial use needs a Commercial Waiver (a donation of
US $500 or more). Third-party parts keep their own licences ([docs/licensing.md](docs/licensing.md)).
