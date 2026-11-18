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
   `cargo run --release -p openwx-broadcaster --bin openwx-release -- dist/<installer>.exe <download-base-url>`.
3. Upload the installer and `dist/latest.json` to the GitHub release. The app reads
   `https://github.com/jdc20181/OpenWX-Broadcasting/releases/latest/download/latest.json`.
