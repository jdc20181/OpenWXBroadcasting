## What and why

<!-- One change per PR. Link the issue: Fixes #123 -->

## Checklist

- [ ] `cargo fmt` on touched files, `cargo clippy --workspace` and `cargo test --workspace` pass
- [ ] No station-specific values (towns, counties, station names, sites, coordinates) in code or defaults
- [ ] No secrets, personal data or large binaries in the diff
- [ ] New data sources go through the relay allow list, with tests and a docs/licensing.md row
- [ ] UI changes: before/after screenshots (dark and light); Remix icons only
- [ ] Graphics: a 1920x1080 render attached
- [ ] Docs and CHANGELOG.md (Unreleased) updated
