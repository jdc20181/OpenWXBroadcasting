# Security policy

OpenWX Broadcasting is used on air, often during severe weather, so a security problem can
become a public-safety problem. We take reports seriously and fix them first.

## Supported versions

| Version | Security fixes |
|---|---|
| Latest release | Yes |
| The release before it | Critical fixes only, for 60 days after a new release |
| Older | No: please update (Settings > About > Check for updates) |

OpenWX Relay servers installed with `server/install.sh` update with `sudo bash install.sh update`.

## Reporting a vulnerability

**Please do not open a public issue for a security problem.**

Report it privately through GitHub: **Security > Report a vulnerability** on the
[repository](https://github.com/jdc20181/OpenWX-Broadcasting/security/advisories/new).
If you cannot use GitHub, open an issue that only asks for a private contact (no details).

Include what you can:

- what is affected (desktop app, OpenWX Relay, the `openwx` CLI/MCP server, the installer,
  `server/install.sh`) and the version (Settings > About, or `openwx-relay --version`);
- steps to reproduce, or a proof of concept;
- what an attacker could do with it, and what they would need first.

### What to expect

| When | What happens |
|---|---|
| Within 3 days | We confirm we received it |
| Within 10 days | We confirm the problem (or explain why it isn't one) and agree a timeline |
| Fixed | A release with the fix, release notes that describe it, and credit if you want it |
| 90 days after the report at most | Public disclosure, sooner if a fix is out and stations have had time to update |

We will not take legal action against good-faith research that follows this policy: test only
against your own installation, don't access other people's data, don't disrupt a station's
broadcast or someone else's relay, and give us a reasonable time to fix before you publish.

## Scope

In scope:

- **OpenWX Relay** (`relay/`): bypassing the provider/path allow list (using it as an open proxy),
  reaching hosts other than the listed weather providers (SSRF), access-token bypass, response
  size or concurrency limits that can be evaded, crashes from crafted requests.
- **Desktop app** (`app/`): code execution from crafted weather data, map tiles, GRIB/NetCDF/HDF5
  files, show/graphic files, media imports or update files; credential leaks (Facebook page
  tokens, OBS passwords, GitHub tokens) to logs, bug reports or other hosts.
- **Updates**: anything that lets an update be installed without a valid signature.
- **`openwx` CLI / MCP server** (`cli/`): a tool call that writes outside the station's settings
  file, posts or publishes anything, or reaches non-weather hosts.
- **`server/install.sh`**, the Docker image and the systemd unit.

Out of scope: problems in the public weather services themselves (report those to NOAA/NWS, SPC,
USGS etc.), attacks that need an already compromised PC or server, missing best-practice headers
with no demonstrated impact, and denial of service by flooding a relay you were given the token for.

## How OpenWX is built to be safe

- **The relay is not an open proxy.** It answers `GET` only, for a fixed list of providers, each
  with one host and an allow list of paths. Clients send a provider ID and a path, never a URL.
  Redirects are not followed, responses have size limits and a content-type allow list.
- **Private by default.** The relay binds to `127.0.0.1`. It refuses to start on any other address
  without an access token of 24+ random characters, compared in constant time.
- **No live-weather caching**: what goes on air is what the source published.
- **Signed updates**: the app installs an update only after its minisign signature checks out
  against the key built into the app.
- **Secrets stay on the PC**: sign-in tokens the app keeps go to Windows Credential Manager, not
  settings files (the Facebook page token will too, when the posting dashboard ships), and are
  never included in bug reports or logs.
- **The MCP server only reads** public weather data and edits the station's own settings file; it
  cannot post, publish or delete anything.
- **Hardened service**: the systemd unit runs as its own user with `ProtectSystem=strict`, no
  capabilities and read-only access to its settings.

## For maintainers

- Run `cargo audit` / `cargo deny` before a release; review dependency updates.
- Never commit tokens, page IDs, e-mail addresses or a station's home coordinates
  (`docs/repo-hygiene.md`); run a secrets scan before each release.
- Sign every release; publish the SHA-256 of every artefact next to it.
