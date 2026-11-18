# OpenWX Relay

The small Rust backend between OpenWX Broadcasting and public weather data. One crate, two uses:

- **Built into the app** (`openwx_relay::start`), on `127.0.0.1` with the next free port from 8737.
- **`openwx-relay` server** for a station's own server: [../server/README.md](../server/README.md).

| File | Routes |
|---|---|
| `src/broadcaster.rs` | `/api/weatherbroadcaster/*`: the fixed provider list (`/feed`), aviation, QPF, drought, NWS forecast, climate, NOMADS model files, archived radar |
| `src/weather.rs` | `/api/weather/overview` (conditions + forecast), `/api/weather/spc-outlooks` |
| `src/radar.rs` | `/api/radar` (composite tiles), `/api/radar/storm-cells`, `/api/public/lightning.json` |
| `src/climate.rs` | `/api/weatherbroadcaster/climate/normals` |
| `src/region.rs` | State/county/town lookups shared with the setup wizard and the CLI |
| `src/lib.rs` | Router, `/health`, `/api/openwx/check`, access-token gate, `start()` |

Tests: `cargo test -p openwx-relay`. Contract and safety rules: [../docs/backend.md](../docs/backend.md).
