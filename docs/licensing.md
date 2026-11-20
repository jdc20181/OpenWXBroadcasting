# Licensing

## OpenWX

OpenWX Broadcasting is released under the [OpenWX License 1.0](../LICENSE).

| | |
|---|---|
| Use | Free for anyone |
| Change and share | Allowed. Shared and hosted versions keep the same licence and include their source |
| Closed commercial use | Needs a Commercial Waiver, granted for a donation of US $500 or more |
| Third-party parts | Not covered (below) |
| Warranty | None |

The licence limits closed commercial use, so it is source-available rather than OSI open
source. GitHub lists it as "Other".

## Third-party projects

OpenWX uses the projects below. They belong to their authors and are under their own licences
and terms, not the OpenWX License. We don't relicense them. Read their licences at the links.

| Project | Used for | Licence |
|---|---|---|
| [maplibre-rs](https://github.com/maplibre/maplibre-rs) | Map engine (`vendor/maplibre-rs`) | [MIT](../vendor/maplibre-rs/LICENSE-MIT) or [Apache-2.0](../vendor/maplibre-rs/LICENSE-APACHE) |
| [GPUI](https://github.com/zed-industries/zed) | Desktop UI framework | See the project |
| [gpui-kit](https://github.com/longbridge/gpui-kit) | UI components | Apache-2.0 |
| [Remix Icon](https://github.com/Remix-Design/RemixIcon) | Interface icons | [Licence](../app/assets/icons/remix/LICENSE) |
| [Meteocons](https://github.com/basmilius/weather-icons) by Bas Milius | Weather icons | [Licence](../app/assets/icons/meteocons/LICENSE.txt) |
| [LibreWXR](https://github.com/JoshuaKimsey/LibreWXR) | Radar, satellite, storm cells, lightning | [Licence](https://github.com/JoshuaKimsey/LibreWXR/blob/main/LICENSE); its public API has its own terms (see the project) |
| [Open-Meteo](https://open-meteo.com) | Some forecast extras, through the relay | [Terms](https://open-meteo.com/en/terms) |
| [Esri](https://www.esri.com) | Basemap tiles | Esri's terms |
| [plotly/datasets](https://github.com/plotly/datasets) | County outlines | See the project |
| [PublicaMundi MappingAPI](https://github.com/PublicaMundi/MappingAPI) | State outlines | See the project |
| Rust crates | Everything else in `Cargo.lock` | Each crate's own licence |

### Music

| Track | Author | Source |
|---|---|---|
| Mellow Cafe Vibe | John Bartmann | [Free Music Archive](https://freemusicarchive.org/index.php/music/John_Bartmann/picnic-on-the-seine-euro-vacation-sounds/mellow-cafe-vibe) |
| Busted Guitar 2 | HoliznaCC0 | [Free Music Archive](https://freemusicarchive.org/music/holiznacc0/busted-guitar-jazz/2-jazz/) |
| Bossa Nova | Joth | [OpenGameArt](https://opengameart.org/content/bossa-nova) |
| Moil | Ruskerdax | [OpenGameArt](https://opengameart.org/content/moil) |
| Catchy Swing | Doge | [OpenGameArt](https://opengameart.org/content/catchy-swing) |
| Music Loop Bundle | Abstraction | [abstractionmusic.com](https://abstractionmusic.com) ([licence](../app/assets/music/abstraction/LICENSE.txt), installed with the app) |

Each source marks its recording CC0 ([CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/)).

## Weather data

The National Weather Service, SPC, WPC, CPC, NHC, SWPC, NWPS, NCEI, RCC-ACIS, USGS, NASA GIBS
and the FAA publish U.S. government data. The [Iowa Environmental Mesonet](https://mesonet.agron.iastate.edu)
and the [U.S. Drought Monitor](https://droughtmonitor.unl.edu) ask to be credited. OpenWX
sends each station's contact details with requests, as the NWS asks.

## Still to check before the public release

- A lawyer's review of the OpenWX License.
- The licences of every Rust dependency (`cargo deny` or `cargo about`).
- A fully open basemap option alongside Esri.
- Replacing Open-Meteo with NOAA sources.
- OpenWX's own update-signing key (`openwx-release keygen`). The CrowdedWX key is not reused.
