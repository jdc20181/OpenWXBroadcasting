# OpenWX Relay server

**Most stations do not need this page.** OpenWX Broadcasting runs its backend (OpenWX Relay)
inside the app on your own PC, with nothing to install. Use a server only when several studio
PCs should share one relay, or when you want to run it on hardware you already host.

| Way | For | Steps |
|---|---|---|
| Built into the app | One studio PC | Nothing: Settings > General > Backend = **On this PC** (default) |
| Linux server (systemd) | A newsroom, several PCs, a VPS | `install.sh` below |
| Docker | Servers that already run containers | `compose.yaml` below |
| Windows PC as a server | A spare studio PC | Run `openwx-relay setup`, then `openwx-relay` (installed next to the app) |

## Linux: install in one command

On Debian, Ubuntu, Fedora, Raspberry Pi OS (64-bit) or any Linux with systemd:

```bash
curl -fsSL https://raw.githubusercontent.com/jdc20181/OpenWXBroadcasting/main/server/install.sh -o install.sh
sudo bash install.sh
```

It downloads the relay, checks its checksum, asks a few questions (station name, a contact for
the National Weather Service, who will use it, your home point), starts it as a service, and checks
every weather feed. When you choose "a shared server" it prints the address studio PCs use:

```
https://YOUR-SERVER/t/<access token>
```

Paste that into OpenWX Broadcasting: **Settings > General > Backend > Hosted server**.

### Everyday commands

| Task | Command |
|---|---|
| Update to the newest release (settings kept; rolls back if the new one fails) | `sudo bash install.sh update` |
| Change settings (station, contact, home, new token) | `sudo bash install.sh setup` |
| Is it running? Are the feeds answering? | `sudo bash install.sh status` |
| Watch the log | `sudo bash install.sh logs` |
| Remove (keeps settings) | `sudo bash install.sh uninstall` |
| Remove everything | `sudo bash install.sh uninstall --purge` |

A release may not include a Linux download. Then build it on the server with `--from-source`,
or copy the `openwx-relay` program over and run
`sudo bash install.sh --binary ./openwx-relay`. Building from source needs Rust.

### HTTPS

The relay speaks plain HTTP and should sit behind HTTPS when anything outside the server talks
to it. With [Caddy](https://caddyserver.com) (free automatic certificates):

```
wx.your-domain.org {
    reverse_proxy 127.0.0.1:8737
}
```

Keep `bind = "127.0.0.1"` in `/etc/openwx/relay.toml` when Caddy runs on the same server, so only
Caddy can reach the relay. Studio PCs then use `https://wx.your-domain.org/t/<access token>`.

## Docker

```bash
cd server
docker compose build
docker compose run --rm relay setup     # choose "A shared server"
docker compose up -d
```

The container listens on `127.0.0.1:8737` on the host; put your HTTPS proxy in front of it.

## Settings (`relay.toml`)

| Key | Meaning |
|---|---|
| `bind`, `port` | Where it listens. `127.0.0.1` = this computer only (default) |
| `access_token` | Required whenever `bind` is not `127.0.0.1`; clients put it in the URL as `/t/<token>` |
| `contact` | E-mail or website sent to NOAA/NWS with every request (they ask for one) |
| `station` | Your station name, sent with the contact |
| `home_lat`, `home_lon`, `coverage_label` | Your home point: the SPC "risk at home" summary and lightning near home |
| `radar_upstream` | RainViewer-compatible radar/satellite tiles, storm cells and lightning (LibreWXR by default) |
| `open_meteo` | Allow the few features that still use Open-Meteo (free for non-commercial use only) |
| `cache_mb`, `max_inflight` | Memory for files that never change; requests in flight at once |

Live weather is never cached. Only files that cannot change (finished model runs, basemap tiles,
past radar frames, climate normals) are kept briefly in memory.

## What the relay serves

`/health` (no token needed), `/api/openwx/check` (feed check), `/api/weatherbroadcaster/*`
(fixed providers: NWS, NOAA map services, SPC, NHC, SWPC, NWPS, IEM, USGS, ACIS, NCEI, NOMADS model
files, drought, FAA), `/api/weather/overview`, `/api/weather/spc-outlooks`, `/api/radar`,
`/api/radar/storm-cells` and `/api/public/lightning.json`. It only answers `GET`, only for the
listed hosts and paths, with response size limits. See `docs/backend.md` for the full contract.
