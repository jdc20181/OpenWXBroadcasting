"""Bring graphics updates from CrowdedWX Broadcaster into OpenWX.

    python tools/sync-crowdedwx.py --from D:/path/to/CrowdedWXBroadcaster          # report only
    python tools/sync-crowdedwx.py --from D:/path/to/CrowdedWXBroadcaster --apply  # write changes

Each incoming file gets the fork's name changes (tools/sync.toml), then a three-way merge:
the CrowdedWX version from the last sync (the baseline in .sync/base), OpenWX's current file,
and CrowdedWX's current file. OpenWX's own edits are kept and CrowdedWX's new work is added.
Overlaps are written with conflict markers and listed in the report.

With no baseline for a file (the first sync), a file that differs is reported for review and
left alone. After --apply the baseline is saved, so later syncs merge automatically.

The report (.sync/report.md) also lists incoming lines with station-specific words.
Needs Python 3.11+ and git on PATH. Never writes to the CrowdedWX folder.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent
STATE = ROOT / ".sync"
BASE = STATE / "base"


def load_config() -> dict:
    with open(ROOT / "tools" / "sync.toml", "rb") as f:
        return tomllib.load(f)


def renamed(text: str, renames: list[list[str]]) -> str:
    for old, new in renames:
        text = text.replace(old, new)
    return text


def read(path: pathlib.Path) -> str | None:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return None


def merge(base: str, ours: str, theirs: str) -> tuple[str, bool]:
    """Three-way merge with git merge-file. Returns (text, clean)."""
    with tempfile.TemporaryDirectory() as tmp:
        paths = []
        for name, text in (("openwx", ours), ("baseline", base), ("crowdedwx", theirs)):
            p = pathlib.Path(tmp) / name
            p.write_text(text, encoding="utf-8", newline="")
            paths.append(str(p))
        result = subprocess.run(
            ["git", "merge-file", "-p", "-L", "OpenWX", "-L", "last sync", "-L", "CrowdedWX", *paths],
            capture_output=True,
        )
        if result.returncode < 0 or result.returncode > 127:
            sys.exit(f"git merge-file failed: {result.stderr.decode(errors='replace')}")
        return result.stdout.decode("utf-8"), result.returncode == 0


def station_lines(text: str, before: str | None, terms: list[str]) -> list[str]:
    old = set((before or "").splitlines())
    hits = []
    for number, line in enumerate(text.splitlines(), 1):
        if line in old:
            continue
        if any(term.lower() in line.lower() for term in terms):
            hits.append(f"{number}: {line.strip()[:120]}")
    return hits


def crowdedwx_build(source: pathlib.Path) -> str:
    return (read(source / "BUILD_NUMBER") or "unknown").strip()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--from", dest="source", default=os.environ.get("CROWDEDWX_SRC"),
                        help="CrowdedWX Broadcaster folder (or set CROWDEDWX_SRC)")
    parser.add_argument("--apply", action="store_true", help="write the changes (default: report only)")
    parser.add_argument("--mark-synced", nargs="+", default=[], metavar="FILE",
                        help="files you ported by hand (e.g. app/src/gfx/render.rs): save CrowdedWX's "
                             "current version as their baseline so later syncs merge them")
    args = parser.parse_args()
    marked = {m.replace("\\", "/") for m in args.mark_synced}
    if not args.source:
        parser.error("give --from or set CROWDEDWX_SRC")
    source = pathlib.Path(args.source).resolve()
    if not (source / "Cargo.toml").exists():
        parser.error(f"{source} doesn't look like the CrowdedWX folder")

    config = load_config()
    renames, terms, keep = config["renames"], config["station_terms"], set(config.get("keep", []))
    sections: dict[str, list[str]] = {name: [] for name in (
        "New", "Updated", "Merged with OpenWX changes", "Conflicts", "Review (no baseline yet)",
        "Kept as OpenWX's own", "Removed in CrowdedWX", "Station-specific lines")}
    writes: list[tuple[pathlib.Path, str]] = []
    baseline: list[tuple[pathlib.Path, str]] = []
    stale: list[pathlib.Path] = []

    for folder in config["folders"]:
        src_dir, dst_dir = source / folder["from"], ROOT / folder["to"]
        types = tuple(folder.get("types", [".rs"]))
        incoming = {p.relative_to(src_dir).as_posix() for p in src_dir.rglob("*") if p.is_file() and p.suffix in types}
        existing = {p.relative_to(dst_dir).as_posix() for p in dst_dir.rglob("*") if p.is_file() and p.suffix in types}

        for rel in sorted(incoming):
            label = f"{folder['to']}/{rel}"
            theirs = renamed(read(src_dir / rel) or "", renames)
            ours = read(dst_dir / rel)
            base_path = BASE / folder["to"] / rel
            base = read(base_path)
            baseline.append((base_path, theirs))

            hits = station_lines(theirs, base if base is not None else ours, terms)
            if hits:
                sections["Station-specific lines"].append(f"{label}\n" + "\n".join(f"    {h}" for h in hits))

            if ours == theirs or label in marked:
                continue
            if rel in keep:
                sections["Kept as OpenWX's own"].append(label)
                continue
            if ours is None:
                sections["New"].append(label)
                writes.append((dst_dir / rel, theirs))
            elif base is None:
                # Not merged yet: no baseline until a person ports it and runs --mark-synced.
                baseline.pop()
                sections["Review (no baseline yet)"].append(label)
            elif base == ours:
                sections["Updated"].append(label)
                writes.append((dst_dir / rel, theirs))
            elif base == theirs:
                continue  # only OpenWX changed it
            else:
                text, clean = merge(base, ours, theirs)
                sections["Merged with OpenWX changes" if clean else "Conflicts"].append(label)
                writes.append((dst_dir / rel, text))

        for rel in sorted(existing - incoming):
            if (BASE / folder["to"] / rel).exists():
                sections["Removed in CrowdedWX"].append(f"{folder['to']}/{rel}")
                stale.append(BASE / folder["to"] / rel)

    build = crowdedwx_build(source)
    lines = [f"# CrowdedWX sync report", "",
             f"CrowdedWX build {build}, {dt.date.today().isoformat()}, "
             + ("applied" if args.apply else "report only (run with --apply to write)"), ""]
    for name, items in sections.items():
        if items:
            lines += [f"## {name} ({len(items)})", "", *[f"- {i}" for i in items], ""]
    if not any(sections.values()):
        lines.append("Nothing to bring over.")
    STATE.mkdir(exist_ok=True)
    report = "\n".join(lines) + "\n"
    (STATE / "report.md").write_text(report, encoding="utf-8")
    print(report)

    if marked and not args.apply:
        print("--mark-synced only takes effect with --apply.")
    if args.apply:
        for path, text in writes:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8", newline="")
        for path in stale:
            path.unlink()
        for path, text in baseline:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8", newline="")
        (STATE / "last.json").write_text(json.dumps(
            {"crowdedwx_build": build, "date": dt.date.today().isoformat()}, indent=2), encoding="utf-8")
        print(f"Wrote {len(writes)} file(s). Baseline saved. Next: cargo test -p openwx-broadcaster")


if __name__ == "__main__":
    main()
