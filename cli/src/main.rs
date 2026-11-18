//! `openwx`: station setup, status and the MCP server for AI assistants.
//!
//!   openwx setup          interactive setup: station, state, counties, main city, backend
//!   openwx status         station summary and a check of every weather feed
//!   openwx weather        current conditions and forecast for the station (or --lat/--lon)
//!   openwx alerts [IN]    active NWS alerts for the station's states (or the given ones)
//!   openwx mcp            run the MCP server on stdin/stdout (for Claude and other assistants)
//!   openwx mcp-config     print the snippet that adds the MCP server to an assistant

mod mcp;
mod setup;
mod station;
mod weather;

use serde_json::Value;

const HELP: &str = "openwx - OpenWX Broadcasting station tool

Usage:
  openwx setup              Set up your station (name, contact, state, counties, main city)
  openwx status             Station summary and a check of every weather feed
  openwx weather [--lat N --lon N]
                            Current conditions and forecast (default: your home city)
  openwx alerts [STATES]    Active NWS alerts (default: your states), e.g. openwx alerts IN,KY
  openwx mcp                Run the MCP server (Claude Desktop, Claude Code and other assistants)
  openwx mcp-config         Print the config snippet that connects an assistant to `openwx mcp`
  openwx --version

Settings: %APPDATA%\\OpenWXBroadcasting\\settings.json (OPENWX_SETTINGS overrides)";

fn print(value: &Value) {
    println!("{}", serde_json::to_string_pretty(value).unwrap_or_default());
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");
    if command != "mcp" {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    }
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<f64>().ok());
    let result: anyhow::Result<()> = async {
        match command {
            "setup" => setup::run().await?,
            "status" => {
                let settings = station::load();
                print(&station::summary(&settings));
                println!("\nWeather feeds:");
                setup::print_checks(&weather::check(&weather::relay(&settings)?).await?);
            }
            "weather" => {
                let settings = station::load();
                print(&weather::overview(&weather::relay(&settings)?, &settings, flag("--lat"), flag("--lon")).await?);
            }
            "alerts" => {
                let settings = station::load();
                print(&weather::alerts(&weather::relay(&settings)?, &settings, args.get(1).map(String::as_str)).await?);
            }
            "mcp" => mcp::serve().await?,
            "mcp-config" => print(&mcp::client_config()),
            "--version" | "-V" | "version" => println!("openwx {}", env!("CARGO_PKG_VERSION")),
            _ => println!("{HELP}"),
        }
        Ok(())
    }
    .await;
    if let Err(error) = result {
        eprintln!("openwx: {error:#}");
        std::process::exit(1);
    }
}
