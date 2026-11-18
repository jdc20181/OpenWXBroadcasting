# dcwx-render

Draws the automatic social graphics: the alert card (1200 × 630, drawn at 2×) and the daily forecast post
(landscape 1280 × 720, square 1080 × 1080, story 1080 × 1920, drawn at 2×). It replaces headless Chrome
(Puppeteer) for the automatic posts. Chrome stays as an automatic fallback.

```
Node (posting logic, unchanged)  --JSON on stdin-->  dcwx-render  --PNG on stdout-->  Node posts it
```

- **No county, brand or site is hard-coded.** Names, logos, colours, map tiles and county shapes all arrive in the job, so
  the same binary can serve any station.
- **Fonts:** Barlow Condensed (SIL OFL), the font the site already uses. It is embedded, so the server needs no fonts.
- **Icons:** Remix Icon (Apache 2.0), embedded as vectors. No emoji and no icon font.
- **Radar:** LibreWXR tiles in the TWC palette, with the basemap one zoom level sharper. Node fetches the tiles; radar is always fresh.

## Two copies, one design

The drawing code is a library (`src/lib.rs`) plus a thin program (`src/main.rs`). CrowdedWX Broadcaster
uses the same library (its copy is `crates/dcwx-render`) for Graphics Studio > Social Graphics, where the
owner makes these graphics by hand, live or from the NWS archive. Keep `src/` and `assets/` identical in
both repositories.

## Build

The server builds it during `deploy/update.sh` (step 5). Rust is installed once into `/opt/rust` if missing.
`deploy.sh --on-server` rebuilds it when Rust is present. If the build fails, the update carries on and posts use Chrome.

On a dev machine:

```
cargo build --release --manifest-path render/Cargo.toml
cargo test --release --manifest-path render/Cargo.toml
node --env-file=.env scripts/social-graphics-gallery.js   # every design -> docs/social-graphics-gallery/
```

`render/tools/prepare-assets.mjs` regenerates `assets/` (fonts unpacked from @fontsource WOFF, icons from remixicon).
Its output is committed.

## Job format

`{ "kind": "alert" | "forecast", "scale": 2, ... }`. Field lists:

- **Alert:** `Alert` in `src/alert.rs`.
- **Forecast:** `Forecast` in `src/forecast.rs`.
- **Map:** `MapData` in `src/map.rs`.

Node builds the jobs in `src/plugins/social-graphics/jobs.js`.

## Designs

| Kind | Designs | Notes |
|---|---|---|
| Alert | `modern` (default), `classic` (today's layout, fixed), `bold` (full map with a broadcast lower third) | |
| Forecast | `modern` (default), `classic` | Six themes, temperature graph, holidays, sponsor and "powered by" blocks, watermark |

The choice is a setting: Admin > Social Graphics.

## Plugins on the Node side

- `src/plugins/social-graphics/`: settings (design, theme, brand, supported counties), the renderer call with Chrome fallback,
  the admin routes (preview, county shapes) and alert fetching for the extra counties.
- `src/plugins/studio-bridge/`: CrowdedWX Broadcaster sign-in (device link, hashed tokens) plus its sponsor, settings and post routes.
