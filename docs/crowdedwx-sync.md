# Bringing graphics over from CrowdedWX

New graphics are built in CrowdedWX Broadcaster first (it's the private, station-tuned app) and
brought into OpenWX in batches. `tools/sync-crowdedwx.py` does the copying and merging; a
person reviews the result before it's committed.

## What's synced

The graphics engine and graphics sets: CrowdedWX `src/gfx` to OpenWX `app/src/gfx`. The list
of folders, the name changes and the station-specific words to watch for are in
`tools/sync.toml`. Everything else (the shell, settings, map studio, CrowdedWX's website
features) is ported by hand when needed.

## How it works

Every incoming file gets the fork's name changes (CrowdedWX to OpenWX), then a three-way merge
of:

- the CrowdedWX version from the last sync (the baseline, kept in `.sync/base`),
- OpenWX's current file,
- CrowdedWX's current file.

OpenWX's own changes (neutral wording, station settings instead of fixed places) stay, and
CrowdedWX's new work is added. Where both changed the same lines, the file gets conflict
markers.

## Running a sync

Once per CrowdedWX build that adds or changes graphics:

```bash
python tools/sync-crowdedwx.py --from D:/path/to/CrowdedWXBroadcaster
```

This only writes a report (`.sync/report.md`):

| Section | Meaning |
|---|---|
| New | Graphics files OpenWX doesn't have yet |
| Updated | Changed only in CrowdedWX; taken as is |
| Merged with OpenWX changes | Both changed, merged cleanly |
| Conflicts | Both changed the same lines; fix the markers by hand |
| Review (no baseline yet) | Changed on both sides before syncing started; port by hand once |
| Station-specific lines | New lines naming a place, website or station; make them neutral |

Then write the changes, check the station-specific lines, and run the tests:

```bash
python tools/sync-crowdedwx.py --from D:/path/to/CrowdedWXBroadcaster --apply
cargo test -p openwx-broadcaster
```

A file in "Review" stays there until it's ported by hand. After porting it, record it so later
syncs merge it automatically:

```bash
python tools/sync-crowdedwx.py --from D:/path/to/CrowdedWXBroadcaster --apply --mark-synced app/src/gfx/render.rs
```

Add the CrowdedWX build number to the changelog entry ("Graphics from CrowdedWX build
111620261"). `.sync/last.json` records the last one.

## Rules

- Never sync from OpenWX back into CrowdedWX with this tool. Fixes made in OpenWX that
  CrowdedWX also needs are ported by hand.
- Files listed under `keep` in `tools/sync.toml` are OpenWX's own and are never overwritten.
- A new graphic that only makes sense for one station stays in CrowdedWX. Leave it out of
  `sets/mod.rs` in OpenWX, or make its place names settings first.

## Later: one shared graphics crate

Syncing works while the two apps are close. If they drift apart, move the graphics engine
and sets into a shared crate in the OpenWX repo (`crates/graphics`). CrowdedWX would depend on
it and keep only its station extras, so a new graphic is written once. That needs graphics to
read station details only through settings, which is mostly true already: about 50 lines in
20 of the 103 graphics files name a place or station.
