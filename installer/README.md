# Building the installer

Windows (NSIS, per-user install, no admin prompt):

```bash
cargo install cargo-packager --locked
cargo packager --release -p openwx-broadcaster
```

The installer lands in `dist/` with the studio (`openwx-broadcaster.exe`), the relay server
(`openwx-relay.exe`) and the CLI (`openwx.exe`). The CC0 loop bundle is included when
`app/assets/music/abstraction/` holds it (see `app/assets/music/README.md`).

## Signed updates

The app installs updates only when they are signed with the OpenWX key whose public half is
`UPDATE_PUBLIC_KEY` in `app/src/updater.rs`.

1. Once: `cargo run -p openwx-broadcaster --bin openwx-release -- keygen` (writes the secret key to
   `%USERPROFILE%\.tauri\openwx-broadcaster.key`; back it up privately) and put the printed public key in `updater.rs`.
2. Per release: `cargo run -p openwx-broadcaster --bin openwx-release -- bump`, rebuild and package, then
   `cargo run --release -p openwx-broadcaster --bin openwx-release -- dist/<installer>.exe <tag>` (the tag
   you'll give the GitHub release, e.g. `0.1.2`).
3. Create the release on https://github.com/jdc20181/OpenWXBroadcasting/releases with that tag and attach
   the installer and `dist/latest.json`. Marking it a prerelease is fine: the app reads the repository's
   release list, so Stable gets the newest full release (or the newest prerelease while there are only
   prereleases) and Beta gets the newest of any kind. A release without `latest.json` is shown to users
   as a manual download only.
