# Contributing to OpenWX Broadcasting

Thanks for helping. OpenWX is a broadcast weather studio for community stations: small teams,
volunteers and weather enthusiasts who want professional-looking weather coverage without a TV
station's budget. Contributions of every size are welcome: a bug report, a clearer sentence, a
new graphic, a data feed, a fix.

By taking part you agree to the [Code of Conduct](CODE_OF_CONDUCT.md). Security problems go
through [SECURITY.md](SECURITY.md), never a public issue.

## Ways to help without writing code

- **Report a bug**: in the app, *Help > Report a bug* (fills in the version and log), or open an
  issue with the *Bug report* template. Screenshots help a lot.
- **Suggest a feature or a graphic**: *Feature request* template. Say what you'd show on air and
  for whom.
- **Test a release** on your own station and report what was confusing.
- **Improve the docs**: if a step confused you, the next person will be confused too.

## Project layout

| Folder | What it is |
|---|---|
| `app/` | The desktop studio (Rust, GPUI): maps, graphics, shows, output |
| `relay/` | OpenWX Relay: the small backend (library, plus the `openwx-relay` server) |
| `cli/` | `openwx`: setup tool, status checks and the MCP server for AI assistants |
| `crates/social-render/` | Social graphics renderer (alert cards, forecast posts) |
| `vendor/maplibre-rs/` | Vendored map engine (upstream MapLibre-rs with our changes) |
| `server/` | Server install script, Dockerfile, compose file |
| `prompts/` | Prompt library for Claude and other assistants |
| `brand/` | Logo, icon and their sources |
| `docs/` | Design docs: backend, setup wizard, modularity, themes, licensing |

## Building

You need the latest stable Rust (`rustup update stable`) and, on Windows, the Visual Studio C++
build tools.

```bash
cargo build                         # app, relay and CLI (debug)
cargo run -p openwx-broadcaster     # the studio
cargo run -p openwx-relay -- --help # the relay server
cargo run -p openwx-cli -- status   # the CLI
cargo test --workspace              # all tests
```

The first build compiles the map engine and takes a while; later builds are incremental.
Packaging an installer: `cargo install cargo-packager --locked`, then
`cargo packager --release -p openwx-broadcaster` (see `installer/README.md`).

## Ground rules for changes

These keep OpenWX usable by any station and trustworthy on air.

1. **Nothing station-specific in code.** No town, county, station name, website, page ID or
   coordinates in defaults or code paths. Places come from the setup wizard; brands from
   Settings. Sample data says SAMPLE and uses neutral places. (`docs/modularity.md`)
2. **Public, documented data sources.** NOAA/NWS and other public federal sources first (SPC,
   WPC, CPC, NHC, NWPS, NCEI/ACIS, USGS, IEM). A new source goes through the relay's provider
   list with a host and a path allow list, and its terms are recorded in `docs/licensing.md`.
3. **No caching of live weather.** What goes on air is what the source published. Only files
   that cannot change (finished model runs, past radar frames, tiles, normals) may be kept.
4. **Calm, accurate wording on air.** No hype words; safety text follows NWS guidance; never
   show a number the source didn't give (show nothing, not "--").
5. **Icons**: Remix Icon only, so the UI stays consistent and the licence stays simple.
6. **Assets must be redistributable.** Fonts OFL/Apache, music CC0 or similar with its source
   recorded, images you made or that are cleared. Large binaries are not committed
   (`.gitignore`, `docs/repo-hygiene.md`).
7. **Keep the relay safe.** `GET` only, fixed hosts, path allow lists, size limits, no redirects.
   Add a test for every new route and for what it refuses.
8. **Accessibility and clarity.** Plain words in the UI, readable contrast in every theme,
   keyboard access for common actions.

## Pull requests

1. For anything bigger than a small fix, open an issue first so we can agree on the approach.
2. Branch from `main`; keep a PR to one change.
3. `cargo fmt` on the files you touched, `cargo clippy --workspace`, and `cargo test --workspace`
   must pass.
4. UI changes: include before/after screenshots (dark and light theme).
5. Graphics changes: include a rendered PNG of the graphic at 1920x1080.
6. Update the docs and `CHANGELOG.md` (under *Unreleased*) when behaviour changes.
7. Fill in the PR template checklist.

A maintainer reviews within about a week. We may ask for changes; that's normal and not a
rejection.

## Commit messages

A short summary line in the present tense ("Add drought outlook to Map Studio"), then a blank
line and the why, if it isn't obvious. Reference issues with `Fixes #123`.

## Using AI assistants

You're welcome to use Claude or another assistant. You are responsible for what you submit:
read and test it, make sure it follows the rules above, and don't paste code you can't license.
`CLAUDE.md` gives assistants the project rules; `prompts/` has prompts for common tasks.

## Licence

OpenWX is under the [OpenWX License 1.0](LICENSE). By contributing you license your work under it,
and you let the maintainers include it in Commercial Waivers (section 7 of the licence); waiver
donations fund the project. If that's a problem for you, say so in your PR before it's merged.
Only add third-party code or assets whose licence allows redistribution, and record them in
`docs/licensing.md`.
