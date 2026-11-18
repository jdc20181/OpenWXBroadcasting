# OpenWX Broadcasting (desktop app)

The studio. Forked from CrowdedWX Broadcaster v0.1.9 on 2026-11-17 and made station-neutral:
the backend is OpenWX Relay (`src/backend.rs`), places come from the first-run wizard
(`src/ui/setup_wizard.rs`), and site-specific integrations are off unless `site_integration`
is turned on.

Run: `cargo run -p openwx-broadcaster`. Package: see [../installer/README.md](../installer/README.md).
