# OpenWX brand

Source artwork from the owner (2026-11-16): `source/owner-logo.png`, `source/owner-icon.png`.
Everything else here was rebuilt from it by `tools/openwx_brand.py` (text set in Barlow, SIL OFL,
`fonts/OFL.txt`, converted to outlines so the SVGs need no installed font).

| File | Use |
|---|---|
| `svg/openwx-logo.svg`, `png/openwx-logo.png` (+ `@2x`) | Wordmark on navy: splash, About, installer banner, website |
| `svg/openwx-wordmark-on-dark.svg` / `-on-light.svg` (+ PNG) | Transparent wordmark for other backgrounds |
| `svg/openwx-icon.svg`, `png/openwx-icon-{64..1024}.png` | App icon (full wordmark) for 64 px and up |
| `svg/openwx-mark.svg`, `png/openwx-mark-{16..256}.png` | Small mark ("WX" + white bar) for 16–48 px: tray, title bar, tabs, favicon |
| `openwx.ico` | Windows icon: mark at 16/20/24/32/48, full icon at 64/128/256 |

Colours (sampled from the artwork):

| Token | Hex | Where |
|---|---|---|
| Navy | `#1B232E` | Background; icon gradient `#263142` → `#121821` (SVG) |
| White | `#FEFEFE` | "OPEN" |
| Orange | `#EE7D14` | "WX", the accent colour |
| Slate | `#778796` | "BROADCASTING" (on light: `#5B6B7A`) |
| Ink | `#1B232E` | "OPEN" on light backgrounds |

Notes:
- PNG icons use flat navy; the SVG icon has the subtle top-to-bottom gradient of the original.
  Re-render PNGs from the SVG with an SVG renderer if the gradient matters at a given size.
- The wordmark is a close rebuild in Barlow, not the original designer file; if the designer has
  the original font and vector, replace `svg/` with it.
- OpenWX branding is for the app itself; on-air graphics use each station's own brand.
- This replaces the CrowdedWX wordmark and its reserved cyan in the fork.
