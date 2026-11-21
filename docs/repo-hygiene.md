# Repository hygiene (public repo)

Repository: https://github.com/jdc20181/OpenWX-Broadcasting (private until release).

The CrowdedWX working folder holds thousands of files that must never reach the public repo:
build output (`target/`, `target-codex/`, ~GBs), QA renders and contact sheets, audit
screenshots, local databases, logs, caches, downloaded boundaries/tiles, installers, music not
cleared for redistribution, and personal settings. The root `.gitignore` in this folder is the
starting point; it is checked **before the first commit**.

## Before the first push

1. `.gitignore` in place (root of this folder); review `git status --ignored` output.
2. Run a size check: nothing over 5 MB is committed unless it is a listed asset.
3. Secrets scan (no tokens, page IDs, e-mail addresses, home coordinates) with a tool such as
   `gitleaks` on the working tree.
4. QA artefacts live outside the repo (or in a separate, ignored `qa-local/`), and only the
   *scripts* that produce them are committed.
5. Large binary assets that must ship (icons, fonts, cleared music) use Git LFS or are fetched
   at build time.

## Ongoing

- Before committing, check `git status` for ignored-type files or anything over the size limit.
- Release artefacts (installers, `.ico` builds) go to GitHub Releases, not the tree.
