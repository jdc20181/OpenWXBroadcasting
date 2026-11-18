# Working on OpenWX with Claude Code

Claude Code reads `CLAUDE.md` at the repository root for the project rules. Useful starting prompts:

**Understand an area first**

> Read CLAUDE.md, then explain how `<feature>` works: which files, the data flow from the relay to
> the screen, and what tests cover it. Don't change anything yet.

**Add a data source**

> Add `<source>` as a relay provider: host, a path allow list as narrow as possible, the TTL rule
> (live data is never cached), a test for an accepted and a refused path, a row in
> docs/licensing.md for its terms, and the app-side reader. Run `cargo test -p openwx-relay`.

**Add a graphic**

> Add a `<name>` graphic template in the `<category>` category using existing bindings. No
> station-specific text; blank (not "--") when data is missing. Render a 1920x1080 PNG with the
> QA test and show it to me.

**Fix a bug from a report**

> Here is a bug report: `<paste>`. Reproduce it with a failing test first, then fix it, then run
> the workspace tests.

**Before opening a PR**

> Run cargo fmt on the files you touched, cargo clippy --workspace and cargo test --workspace;
> check the diff for station-specific values, secrets and large files; draft the PR description
> with the template's checklist.
