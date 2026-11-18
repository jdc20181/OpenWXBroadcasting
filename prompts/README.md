# OpenWX with Claude and other AI assistants

OpenWX ships an [MCP](https://modelcontextprotocol.io) server, `openwx mcp`, so an assistant such
as Claude can read your station's weather (NWS, SPC) and help with setup and writing. It only
reads public weather data and edits your station's own settings file: it cannot post, publish
or delete anything.

## Connect it

`openwx` is installed next to the app (`%LOCALAPPDATA%\OpenWX Broadcasting\openwx.exe` by
default). Print the exact snippet for your PC with:

```bash
openwx mcp-config
```

- **Claude Desktop**: Settings > Developer > Edit Config, merge the snippet into
  `claude_desktop_config.json`, restart Claude.
- **Claude Code**: `claude mcp add openwx -- "<path to openwx.exe>" mcp`, or put the snippet in
  a project's `.mcp.json` (see `.mcp.json.example` at the repository root).
- **Other MCP clients**: run `openwx mcp` as a stdio server.

## What the assistant can do

| Tool | What it does |
|---|---|
| `get_station` | Your station name, home city, counties, states, backend |
| `weather_overview` | Current conditions (nearest NWS station) and the NWS 7-day forecast |
| `active_alerts` | Active NWS alerts, flagged when they touch your counties |
| `spc_outlook` | SPC severe outlook, days 1-8, and the risk at your home |
| `check_feeds` | Whether each public data source answers |
| `set_station_region` | Set state, counties, main city and region name |
| `set_station_identity` | Set station name, short name, tagline, NWS contact |

Built-in prompts (pick them from your assistant's prompt/slash menu): **forecast_post**,
**severe_briefing**, **show_rundown**, **station_setup**.

## Prompt library

The files in this folder are ready-to-paste prompts for common jobs. They assume the MCP server
is connected; without it, paste the data you want used.

| File | Use |
|---|---|
| [forecast-post.md](forecast-post.md) | Daily forecast post for social media |
| [severe-briefing.md](severe-briefing.md) | Severe weather briefing (on air or online) |
| [show-rundown.md](show-rundown.md) | Plan today's weather show |
| [station-setup.md](station-setup.md) | Set up a new station by conversation |
| [graphic-copy.md](graphic-copy.md) | Short on-screen text that fits a graphic |
| [contributor.md](contributor.md) | For developers: working on the OpenWX code with Claude Code |

Always check what an assistant writes before it goes on air or online. **The National Weather
Service is the official source for warnings.**
