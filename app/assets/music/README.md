# Show music

Only music that may be redistributed is bundled with OpenWX.

| Files | Source | Licence |
|---|---|---|
| The five starter beds in this folder (`*.mp3`) | Free Music Archive / OpenGameArt (sources in `docs/licensing.md`) | CC0 (public domain dedication) |
| `abstraction/` loop bundle | [Abstraction Music](https://abstractionmusic.com) "Music Loop Bundle" 2024 Q1-Q3 | CC0 (`abstraction/LICENSE.txt`) |

The loop bundle (about 800 MB) is not in Git. To build an installer with it, download the
three 2024 bundles from abstractionmusic.com and unpack them into `abstraction/` so the
folders match `abstraction/catalog.json`. Without it, the app works and the loop library is empty.

The starter beds are stored with Git LFS: run `git lfs install` once before cloning.
