# Interactive first-run setup

A new user should go from install to a working map, a graphic and a show without editing a
file. Every answer can be changed later in Settings.

| Step | Asks for | Checks before Next |
|---|---|---|
| 1. Welcome | Language; import settings from another install (optional) | |
| 2. Station | Station name, short name, tagline, contact e-mail (used in the NWS `User-Agent`) | E-mail looks valid |
| 3. Brand | Logo (optional), primary/accent colours, fonts (from installed or bundled open fonts), Modern or Classic look | Contrast check on the sample lower third |
| 4. Coverage | **Select the state** (more than one allowed), then **the covered counties** on a county map (click to add/remove, search, select-all-in-state), the home county, a region name and a town per county (pre-filled from NWS, editable) | At least one county |
| 5. Backend | **Local server** (bundled Rust backend on this PC, default) or **remote server** (URL of a hosted OpenWX backend, plus a token if it asks); Direct for troubleshooting | Test: each provider answers (green/red list) |
| 5b. Posting (optional) | Connect a Facebook Page (Page ID and token), or skip and set up later in the dashboard | Test reads the Page name back |
| 5c. Theme | Dark / Light app theme; Broadcast Crisp/Modern, Classic, Light or Dark on-air look | |
| 6. Data | Radar composite source, basemap style, model sources, optional feeds (roads, lightning, outages) | Each enabled feed answers |
| 7. Output | Program output (window/monitor), NDI/OBS integration, recording folder | Test pattern on the chosen output |
| 8. Audio | Output device, music library (bundled royalty-free set or none) | Plays a test tone |
| 9. Finish | Builds a starter show (forecast, current conditions, radar) for the new station | Preview the first scene |

Design rules: one question per screen area, every step skippable except Station and
Coverage, a progress rail, plain wording, and a "Show me what this does" preview on brand and
output steps. Remix icons only.
