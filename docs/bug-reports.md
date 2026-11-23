# Bug reports: GitHub Issues

CrowdedWX saves reports locally (`%APPDATA%\CrowdedWXBroadcaster\bug-reports`, text + up to
three screenshots). OpenWX keeps that dialog and, when the user agrees, files the report as a
GitHub issue on the project repository (`jdc20181/OpenWXBroadcasting`).
GitHub is the first backend; the design allows another tracker later.

## How it works

1. The user writes the report in the app (same dialog as today), and can attach screenshots
   and the last part of the app log (opt-in, previewed, with personal data such as the home
   location and tokens removed).
2. **Send to GitHub** opens a preview of exactly what will be published.
3. Two ways to submit, the user picks:
   - **Browser (no account setup)**: the app opens GitHub's new-issue page with the title and
     body pre-filled (`/issues/new?title=…&body=…`). Screenshots are dropped into the page by
     the user. No token in the app.
   - **Direct (GitHub account)**: the user signs in with GitHub's device flow (OAuth app, no
     password in the app) and the app creates the issue through the REST API
     (`POST /repos/{owner}/{repo}/issues`) with labels `bug`, `from-app`, version and build.
     Screenshots go up as issue attachments where the API allows, otherwise as links the user
     uploads.
4. The local copy is always kept, with the issue link once filed.

## Rules

- Nothing is sent without the preview and an explicit Send.
- No shared token is ever compiled into the app (a public repo's token would be abused).
- Labels and templates live in the repo (`.github/ISSUE_TEMPLATE/bug.yml`) so they can change
  without an app update.
- `Reporter` trait (`file(report) -> link`) with `local` and `github` implementations; a
  future tracker is another implementation.
