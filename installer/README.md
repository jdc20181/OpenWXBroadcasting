# Building the installer

## Releases on GitHub (the normal way)

The **Release** workflow (`.github/workflows/release.yml`) builds every installer on GitHub's free
standard runners, signs the Windows one, writes `latest.json` and creates a **draft** prerelease with
all of it attached:

- `OpenWXBroadcasting-Windows-V<version>-<build>.exe`, its `.sig` and `latest.json`
- `OpenWXBroadcasting-Linux-V<version>-<build>.AppImage` and `.deb`
- `OpenWXBroadcasting-MacOS-V<version>-<build>.dmg`

Once, before the first run: on GitHub open the repository's **Settings > Secrets and variables >
Actions > New repository secret**, name it `OPENWX_UPDATE_KEY`, and paste the whole contents of
`%USERPROFILE%\.tauri\openwx-broadcaster.key` (one line of text). Keep your own copy of the file too.

Each release:

1. Set the version in `Cargo.toml` and `app/Cargo.toml`, the build number in `app/BUILD_NUMBER`
   (`cargo run -p openwx-broadcaster --bin openwx-release -- bump`), and give the changelog's newest
   section its version heading. Commit and push.
2. Push a tag that matches the version, e.g. `0.1.3` (or press **Run workflow** on the Release
   workflow in the Actions tab).
3. Wait for the run to finish (about 1 to 2 hours; the three builds run side by side).
4. Open **Releases**, check the draft, and press **Publish release**. Apps see the update from then on.

If Linux or macOS fails to build, the draft still has Windows and its notes say which is missing.
A release that already exists for that tag isn't touched: delete the draft first to run it again.

## Building on your own computer

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
2. Per release: `cargo run -p openwx-broadcaster --bin openwx-release -- bump`, rebuild and package, rename
   the installer to `OpenWXBroadcasting-Windows-V<version>-<build>.exe` (e.g.
   `OpenWXBroadcasting-Windows-V0.1.2-112320262.exe`), then
   `cargo run --release -p openwx-broadcaster --bin openwx-release -- dist/<installer>.exe <tag>` (the tag
   you'll give the GitHub release, e.g. `0.1.2`).

Release files are named `OpenWXBroadcasting-<Platform>-V<version>-<build>`: `Windows` (.exe), `MacOS`
(.dmg, built by the MacOS Installer workflow), `Linux` (.AppImage and .deb, built by the Linux Installer
workflow). Both workflows run when a release is published, or from the Actions tab with "Run workflow", and
leave the files on the run's page for 14 days; they never upload anything to the release.
3. Create the release on https://github.com/jdc20181/OpenWXBroadcasting/releases with that tag and attach
   the installer and `dist/latest.json`. Marking it a prerelease is fine: the app reads the repository's
   release list, so Stable gets the newest full release (or the newest prerelease while there are only
   prereleases) and Beta gets the newest of any kind. A release without `latest.json` is shown to users
   as a manual download only.
