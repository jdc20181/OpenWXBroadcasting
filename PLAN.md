# OpenWX Broadcasting: root work plan

Source: CrowdedWX Broadcaster v0.1.8 (Rust, GPUI desktop app) plus the relay routes it uses on
the Dubois County Weather site (`src/routes/weatherbroadcaster.js`). The fork lives in this
folder. Nothing is published until every gate below passes.

Repository: https://github.com/jdc20181/OpenWX-Broadcasting (private until public release).

## Progress (2026-11-17)

The owner started the fork on 2026-11-17 (G0 waived for the test build). Done in the first
test build (v0.1.0 build 1):

| Item | State |
|---|---|
| Fork into `app/` from CrowdedWX v0.1.9 build 111620261; crates, binaries, folders, titles renamed | Done |
| OpenWX Relay in Rust (`relay/`), bundled in the app on 127.0.0.1; `openwx-relay` server; token gate | Done (G3: Docker image written, not yet run) |
| Backend choice and feed check in Settings | Done |
| First-run setup wizard (station, contact, state, counties, main city, backend, feed check) | Done (G4 walkthrough pending) |
| `openwx` CLI: setup, status, weather, alerts; MCP server + prompts for Claude | Done |
| `server/install.sh` (install/update/rollback/uninstall), Dockerfile, compose | Done (needs a release with Linux builds) |
| SECURITY, CONTRIBUTING, CODE_OF_CONDUCT, SUPPORT, CLAUDE.md, issue/PR templates, CI | Done |
| Tight `.gitignore` + `.gitattributes` (LFS for the CC0 music) | Done; review `git status --ignored` before the first commit |
| Own update-signing key | Tooling done (`openwx-release keygen`); owner generates and keeps the key |
| Decoupling: remaining Dubois/DCWeather mentions in samples/tests, site features behind `site_integration` | Partly (UI and defaults done; samples and tests still name real places) |
| Posting dashboard, GitHub bug reports, four app themes | Not started |
| Licence: OpenWX License 1.0 (`LICENSE`); radar server setting for LibreWXR | Done (licence needs legal review); Open-Meteo removal still open |

## What OpenWX ships with (owner's scope, 2026-11-16)

| Feature | Doc |
|---|---|
| Setup wizard: choose the state, the covered counties, station, brand, backend | `docs/setup-wizard.md` |
| Backend choice: a local server or a remotely hosted server running the small Rust backend, formed and forked from today's mostly-Node relay | `docs/backend.md` |
| In-app dashboard for forecast and alert posts; the user enters their own Facebook Page credentials (Facebook only for now), modelled on Dubois County Weather's posting | `docs/social-dashboard.md` |
| Modular, adoptable design: nothing tied to one station's needs or infrastructure | `docs/modularity.md` |
| Bug reports filed as GitHub issues (alternative tracker later) | `docs/bug-reports.md` |
| Themes: Dark, Light, Broadcast Crisp/Modern, Classic | `docs/themes.md` |
| Brand: logo, icon, small mark, SVG masters | `brand/BRAND.md` |
| A proper `.gitignore` and repo hygiene so only source, docs and cleared assets are public | `docs/repo-hygiene.md`, `.gitignore` |

## Gates

| Gate | Meaning | Who signs off |
|---|---|---|
| **G0 Polish accepted** | The CrowdedWX polish pass is complete: open P0/P1 board items closed, UI walkthrough screenshots accepted, drawn-graphics audit table A done, no "--" placeholders on air, catalog checks green | Owner |
| **G1 Fork made** | Code copied at a tagged CrowdedWX build into a clean tree (no CrowdedWX history, no build output, QA renders or local data); `.gitignore` reviewed with `git status --ignored` before the first commit; builds and tests pass unchanged | Owner |
| **G2 Decoupled** | No station, brand, host or place is hard-coded (see `docs/decoupling.md`); a fresh install shows no Dubois County / CrowdedWX content | Review against the inventory |
| **G3 Backend ready** | OpenWX Relay runs locally (bundled) and remotely (Docker), passes the API contract tests (`docs/backend.md`) | Tests + owner |
| **G4 Setup ready** | First-run wizard takes a new user from install to a working map, graphic and show without editing files (`docs/setup-wizard.md`) | Walkthrough with a fresh profile |
| **G5 Cleared to publish** | Licence chosen; every third-party asset, font, music track, icon set and data source cleared (`docs/licensing.md`); data-source terms honoured | Owner |
| **G6 Packaged** | Signed installer, own update channel, user guide, release notes | Owner |

## Phases (each ends at its gate)

1. **Polish (CrowdedWX, before forking).** Finish the polish pass in the main project so both
   products start from the same accepted code. Track remaining items on the CWXB board.
2. **Fork.** Copy into `app/` at an owner-chosen build. Rename the crate, binary, app ID,
   config folder (`%APPDATA%\OpenWX`), log names and window titles.
3. **Decouple.** Work through `docs/decoupling.md`: every hard-coded value becomes a station
   setting, a wizard answer or a backend setting. Remove CrowdedWX-only features that depend
   on the Dubois County Weather site (Social Graphics posting to the site, community board,
   studio bridge) or put them behind a "site integration" plug-in.
4. **Backend.** Build OpenWX Relay in `relay/` from the routes the app actually calls (inventory
   in `docs/backend.md`). Two modes: bundled (started by the app on 127.0.0.1) and remote
   (Docker image + `docker compose` file for a station's own server).
5. **Setup wizard.** First run: station identity, state and covered counties, backend mode,
   data sources, outputs, audio, and a test that every feed answers.
5b. **Posting dashboard.** Facebook Page connection with the user's own credentials, alert
   rules, forecast schedule, queue and history (`docs/social-dashboard.md`).
5c. **Bug reports to GitHub** and **themes** (`docs/bug-reports.md`, `docs/themes.md`).
6. **Licensing and assets.** Choose the licence; replace anything that can't be redistributed.
7. **Package and document.** Installer, updater pointed at the OpenWX release host, user guide,
   backend guide, contributor guide.

## Principles carried over

- NOAA/NWS (and other public federal sources: USGS, IEM) first; no live-weather caching.
- Nothing station-specific in code; brand, fonts, colours, places and hosts are settings.
- Remix icons only in the UI.
- Polish must be crisp and accepted before anything is published.
