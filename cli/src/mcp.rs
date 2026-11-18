//! `openwx mcp`: a Model Context Protocol server on stdin/stdout (JSON-RPC 2.0, one message per
//! line). Any MCP client (Claude Desktop, Claude Code, others) can then read the station's weather
//! and help with its setup and writing. Setup: `openwx mcp-config` prints the client snippet.
//!
//! Tools only read public weather data or edit the station's own settings file; nothing is posted
//! or published from here.

use anyhow::Result;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::{station, weather};

const PROTOCOL: &str = "2025-06-18";

fn tools() -> Value {
    json!([
        {
            "name": "get_station",
            "title": "Station settings",
            "description": "The OpenWX station's name, home city, coverage counties, states and backend mode.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "weather_overview",
            "title": "Current conditions and forecast",
            "description": "Current conditions from the nearest NWS observing station and the NWS 7-day forecast. Defaults to the station's home city.",
            "inputSchema": { "type": "object", "properties": {
                "lat": { "type": "number", "description": "Latitude (optional; default home)" },
                "lon": { "type": "number", "description": "Longitude (optional; default home)" }
            } },
            "annotations": { "readOnlyHint": true, "openWorldHint": true }
        },
        {
            "name": "active_alerts",
            "title": "Active NWS alerts",
            "description": "Active National Weather Service watches, warnings and advisories for a state (default: the station's states). Each alert says whether it touches the station's coverage counties.",
            "inputSchema": { "type": "object", "properties": {
                "area": { "type": "string", "description": "Two-letter state code(s), comma separated, e.g. IN or IN,KY" }
            } },
            "annotations": { "readOnlyHint": true, "openWorldHint": true }
        },
        {
            "name": "spc_outlook",
            "title": "SPC severe weather outlook",
            "description": "Storm Prediction Center convective outlook: risk levels shown on each day and the risk at the station's home point.",
            "inputSchema": { "type": "object", "properties": {
                "day": { "type": "integer", "minimum": 1, "maximum": 8, "description": "One day (1-8); omit for all" }
            } },
            "annotations": { "readOnlyHint": true, "openWorldHint": true }
        },
        {
            "name": "check_feeds",
            "title": "Check data feeds",
            "description": "Checks that each public weather source answers (NWS, SPC, radar, USGS, IEM, ACIS, SWPC).",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true, "openWorldHint": true }
        },
        {
            "name": "set_station_region",
            "title": "Set the coverage region",
            "description": "Sets the station's state, covered counties, main city (map centre) and region name in the OpenWX settings. Restart the OpenWX app afterwards.",
            "inputSchema": { "type": "object", "required": ["state", "counties", "city"], "properties": {
                "state": { "type": "string", "description": "State name or two-letter code" },
                "counties": { "type": "string", "description": "County names, comma separated, e.g. \"Dubois, Pike, Spencer\"" },
                "city": { "type": "string", "description": "Main city or town" },
                "region_name": { "type": "string", "description": "Coverage area name, e.g. \"Southern Indiana\"" }
            } },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true }
        },
        {
            "name": "set_station_identity",
            "title": "Set the station name and contact",
            "description": "Sets the on-air station name, short name, tagline and the NWS contact (e-mail or https website). Only the fields given change.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string" },
                "short_name": { "type": "string" },
                "tagline": { "type": "string" },
                "contact": { "type": "string", "description": "E-mail or https:// website sent to NOAA/NWS in requests" }
            } },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true }
        }
    ])
}

const STYLE: &str = "Write in calm, plain language for a general audience: no hype words (\"monster\", \"bomb\", \"nightmare\"), \
no ALL CAPS, temperatures in °F, and say where the information comes from (NWS, SPC). Never invent numbers: \
use only what the tools return, and say so when data is missing. Safety wording follows NWS guidance.";

fn prompts() -> Value {
    json!([
        { "name": "forecast_post", "title": "Forecast post", "description": "Draft a social media forecast post for the station's area.",
          "arguments": [{ "name": "length", "description": "short (default) or detailed", "required": false }] },
        { "name": "severe_briefing", "title": "Severe weather briefing", "description": "An on-air or social briefing on today's severe weather risk and active alerts.", "arguments": [] },
        { "name": "show_rundown", "title": "Show rundown", "description": "Suggest the segments and graphics for today's weather show.",
          "arguments": [{ "name": "minutes", "description": "Show length in minutes (default 3)", "required": false }] },
        { "name": "station_setup", "title": "Station setup helper", "description": "Walk through setting the station name, contact, state, counties and main city.", "arguments": [] }
    ])
}

fn prompt_text(name: &str, args: &Value) -> Option<(String, String)> {
    let text = match name {
        "forecast_post" => {
            let length = args["length"].as_str().unwrap_or("short");
            format!("Call get_station, weather_overview and active_alerts. Then draft a {length} forecast post for the station's \
                     coverage area: today, tonight and the next few days, with highs/lows and rain chances, and any alerts that \
                     touch the coverage counties first. End with the station's tagline. {STYLE}")
        }
        "severe_briefing" => format!(
            "Call get_station, spc_outlook (day 1 and 2) and active_alerts. Write a short briefing: the SPC risk level at the \
             station's home and across the coverage area, the main hazards (tornado, hail, wind) with their probabilities, the \
             timing if the NWS forecast gives it, active warnings in the coverage counties first, and clear safety steps. {STYLE}"
        ),
        "show_rundown" => {
            let minutes = args["minutes"].as_str().unwrap_or("3");
            format!("Call get_station, weather_overview, active_alerts and spc_outlook. Suggest a {minutes}-minute weather show: \
                     an ordered list of segments (current conditions, radar, alerts if any, severe outlook if any risk, 7-day), \
                     with a one-line talking point and a suggested OpenWX graphic for each. {STYLE}")
        }
        "station_setup" => "Help set up this OpenWX station. Call get_station first. Ask, one at a time, for anything missing: \
            the station's on-air name, a contact e-mail or website for the NWS, the state, the covered counties, and the main \
            city or town. Then call set_station_identity and set_station_region, show the result, and remind the user to \
            restart OpenWX Broadcasting.".to_owned(),
        _ => return None,
    };
    let description = prompts().as_array()?.iter().find(|p| p["name"] == name)?["description"].as_str()?.to_owned();
    Some((description, text))
}

async fn call_tool(name: &str, args: &Value) -> Result<Value> {
    let mut settings = station::load();
    let relay = weather::relay(&settings)?;
    let num = |k: &str| args[k].as_f64();
    Ok(match name {
        "get_station" => station::summary(&settings),
        "weather_overview" => weather::overview(&relay, &settings, num("lat"), num("lon")).await?,
        "active_alerts" => weather::alerts(&relay, &settings, args["area"].as_str()).await?,
        "spc_outlook" => weather::spc(&relay, args["day"].as_u64()).await?,
        "check_feeds" => weather::check(&relay).await?,
        "set_station_region" => {
            let get = |k: &str| args[k].as_str().unwrap_or_default().to_owned();
            let region = station::resolve_region(&relay, &get("state"), &get("counties"), &get("city"), &get("region_name")).await?;
            station::apply_region(&mut settings, &region);
            let path = station::save(&settings)?;
            json!({ "saved": path.display().to_string(), "station": station::summary(&settings), "next": "Restart OpenWX Broadcasting to load the new region." })
        }
        "set_station_identity" => {
            for (arg, key) in [("name", "station_name"), ("short_name", "station_short"), ("tagline", "tagline")] {
                if let Some(v) = args[arg].as_str().map(str::trim).filter(|v| !v.is_empty()) {
                    settings[key] = json!(v);
                }
            }
            if let Some(contact) = args["contact"].as_str().map(str::trim).filter(|v| !v.is_empty()) {
                if !openwx_relay::prompt::looks_like_contact(contact) {
                    anyhow::bail!("the contact must be an e-mail address or an https:// website");
                }
                settings["contact"] = json!(contact);
            }
            let path = station::save(&settings)?;
            json!({ "saved": path.display().to_string(), "station": station::summary(&settings) })
        }
        other => anyhow::bail!("unknown tool {other}"),
    })
}

async fn handle(message: &Value) -> Option<Value> {
    let id = message.get("id").cloned();
    let method = message["method"].as_str().unwrap_or_default();
    let params = &message["params"];
    let result: Result<Value, (i64, String)> = match method {
        "initialize" => Ok(json!({
            "protocolVersion": params["protocolVersion"].as_str().unwrap_or(PROTOCOL),
            "capabilities": { "tools": {}, "prompts": {}, "resources": {} },
            "serverInfo": { "name": "openwx", "title": "OpenWX Broadcasting", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Weather data (NOAA/NWS, SPC) and setup for an OpenWX Broadcasting station. Use get_station first to learn the station's area."
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => {
            let name = params["name"].as_str().unwrap_or_default();
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            Ok(match call_tool(name, &args).await {
                Ok(value) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }], "structuredContent": value, "isError": false }),
                Err(error) => json!({ "content": [{ "type": "text", "text": format!("Error: {error:#}") }], "isError": true }),
            })
        }
        "prompts/list" => Ok(json!({ "prompts": prompts() })),
        "prompts/get" => {
            let name = params["name"].as_str().unwrap_or_default();
            match prompt_text(name, &params["arguments"]) {
                Some((description, text)) => Ok(json!({ "description": description, "messages": [{ "role": "user", "content": { "type": "text", "text": text } }] })),
                None => Err((-32602, format!("unknown prompt {name}"))),
            }
        }
        "resources/list" => Ok(json!({ "resources": [{ "uri": "openwx://station", "name": "station", "title": "Station settings summary", "mimeType": "application/json" }] })),
        "resources/read" => match params["uri"].as_str() {
            Some("openwx://station") => Ok(json!({ "contents": [{ "uri": "openwx://station", "mimeType": "application/json", "text": station::summary(&station::load()).to_string() }] })),
            _ => Err((-32602, "unknown resource".into())),
        },
        _ if id.is_none() => return None, // notifications (initialized, cancelled) need no answer
        _ => Err((-32601, format!("method not found: {method}"))),
    };
    let id = id?;
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
    })
}

pub async fn serve() -> Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Array(batch)) => {
                let mut out = Vec::new();
                for m in &batch {
                    if let Some(r) = handle(m).await {
                        out.push(r);
                    }
                }
                (!out.is_empty()).then(|| Value::Array(out))
            }
            Ok(message) => handle(&message).await,
            Err(_) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "parse error" } })),
        };
        if let Some(reply) = reply {
            stdout.write_all(reply.to_string().as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}

/// The snippet for Claude Desktop (`claude_desktop_config.json`) or a project's `.mcp.json`.
pub fn client_config() -> Value {
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "openwx".into());
    json!({ "mcpServers": { "openwx": { "command": exe, "args": ["mcp"] } } })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lists_and_initializes() {
        let init = handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": PROTOCOL } })).await.unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "openwx");
        let list = handle(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await.unwrap();
        assert!(list["result"]["tools"].as_array().unwrap().len() >= 7);
        assert!(handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).await.is_none());
        let missing = handle(&json!({ "jsonrpc": "2.0", "id": 3, "method": "nope" })).await.unwrap();
        assert_eq!(missing["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn prompts_resolve() {
        let p = handle(&json!({ "jsonrpc": "2.0", "id": 4, "method": "prompts/get", "params": { "name": "forecast_post", "arguments": { "length": "detailed" } } })).await.unwrap();
        let text = p["result"]["messages"][0]["content"]["text"].as_str().unwrap();
        assert!(text.contains("detailed forecast post"));
        for prompt in prompts().as_array().unwrap() {
            assert!(prompt_text(prompt["name"].as_str().unwrap(), &json!({})).is_some());
        }
    }

    #[tokio::test]
    async fn unknown_tool_is_an_error_result() {
        let r = handle(&json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": "launch_rockets" } })).await.unwrap();
        assert_eq!(r["result"]["isError"], true);
    }
}
