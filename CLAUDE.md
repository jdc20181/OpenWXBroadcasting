# OpenWX Broadcasting: notes for AI assistants

Rust workspace: `app/` (GPUI desktop studio), `relay/` (OpenWX Relay backend: library +
`openwx-relay` server), `cli/` (`openwx`: setup, status, MCP server), `crates/social-render/`,
`vendor/maplibre-rs/` (vendored; its own workspace). Build `cargo build`, test
`cargo test --workspace`.

## Rules

- **Nothing station-specific**: no towns, counties, station names, websites, page IDs or home
  coordinates in code or defaults. Places come from the setup wizard; samples say SAMPLE.
- **Data**: NOAA/NWS and public federal sources through the relay's fixed provider list (host +
  path allow list). Never cache live weather; only immutable files may have a TTL.
- **Relay safety**: GET only, no redirects, size limits, allow lists; every route gets tests for
  what it accepts and what it refuses. Non-loopback binds require an access token.
- **On-air text**: calm wording, no hype, NWS safety language; blank instead of "--" when data
  is missing; never invent values.
- **UI**: Remix icons only; plain words; dark and light themes must both read well.
- **Assets**: only redistributable fonts/music/images; no large binaries in Git (see
  `.gitignore`, `docs/repo-hygiene.md`).
- **Secrets**: never commit tokens, e-mail addresses or a station's coordinates; tokens live in
  Windows Credential Manager, not settings files.
- Keep changes focused; update docs and `CHANGELOG.md` (Unreleased) with behaviour changes.

## Where things are

| Task | Start here |
|---|---|
| Backend routes | `relay/src/lib.rs` (router), `broadcaster.rs` (providers), `weather.rs`, `radar.rs`, `climate.rs` |
| How the app picks its backend | `app/src/backend.rs`, `app/src/net.rs` |
| Settings | `app/src/config.rs`; Settings UI `app/src/ui/settings.rs` |
| First-run wizard | `app/src/ui/setup_wizard.rs`; CLI wizard `cli/src/setup.rs` |
| Graphics templates | `app/src/gfx/templates*.rs`, `app/src/gfx/sets/` |
| Map layers | `app/src/map/` |
| MCP tools and prompts | `cli/src/mcp.rs`; prompt library `prompts/` |
