//! `openwx setup`: an interactive, plain-language setup for a new station. It writes the same
//! settings the desktop app's first-run wizard does, so either can be used (or re-run later).

use anyhow::Result;
use serde_json::{json, Value};

use openwx_relay::prompt::{ask, ask_valid, choose, looks_like_contact, yes_no};

use crate::{station, weather};

fn columns(items: &[String]) {
    let width = items.iter().map(|s| s.len()).max().unwrap_or(10) + 8;
    let per_row = (100 / width).max(1);
    for (i, item) in items.iter().enumerate() {
        print!("{:>3}) {:<w$}", i + 1, item, w = width - 5);
        if (i + 1) % per_row == 0 {
            println!();
        }
    }
    println!();
}

pub async fn run() -> Result<()> {
    let mut settings = station::load();
    println!("\nOpenWX Broadcasting setup");
    println!("=========================");
    println!("A few questions about your station. Press Enter to keep the value in [brackets].");
    println!("Settings file: {}\n", station::settings_path().display());

    // 1. Station
    println!("STEP 1 of 4 - Your station\n");
    let name = ask("Station name (shown on air)", settings["station_name"].as_str().unwrap_or("Your Station Weather"));
    let short = ask("Short name (for tight spots)", settings["station_short"].as_str().unwrap_or(&name));
    let tagline = ask("Tagline", settings["tagline"].as_str().unwrap_or("Your Weather. Your Community."));
    println!("\nThe National Weather Service asks every app that reads its data for a contact,");
    println!("so they can reach you if something goes wrong. It is sent with requests, not published.");
    let contact = ask_valid(
        "Contact e-mail or website",
        settings["contact"].as_str().unwrap_or(""),
        "Enter an e-mail address (name@example.org) or an https:// website.",
        looks_like_contact,
    );
    for (key, value) in [("station_name", &name), ("station_short", &short), ("tagline", &tagline), ("contact", &contact)] {
        settings[key] = json!(value);
    }
    let relay = weather::relay(&settings)?;

    // 2. Coverage
    println!("\nSTEP 2 of 4 - Where you cover\n");
    let state = ask_valid(
        "State (name or two letters, e.g. Indiana or IN)",
        settings["coverage"]["states"][0].as_str().unwrap_or(""),
        "That is not a U.S. state or territory. Try the two-letter code.",
        |s| station::state_abbr(s).is_some(),
    );
    let st = station::state_abbr(&state).unwrap_or("IN");
    println!("Looking up {st} counties from the National Weather Service…");
    let all = station::counties(&relay, st).await?;
    columns(&all.iter().map(|c| c.name.clone()).collect::<Vec<_>>());
    let counties = loop {
        let typed = ask("Your counties: numbers or names, separated by commas (e.g. 12, 40, Pike)", "");
        match station::pick_counties(&all, &typed) {
            Ok(c) => break c,
            Err(e) if openwx_relay::prompt::input_ended() => anyhow::bail!("no counties chosen ({e}); run openwx setup in a terminal"),
            Err(e) => println!("  {e}"),
        }
    };
    println!("  Covering: {}", counties.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", "));

    // 3. Main city
    println!("\nSTEP 3 of 4 - Your main city or town (the map centres here)\n");
    let (city, lat, lon) = loop {
        let typed = ask("Main city or town", settings["home_label"].as_str().and_then(|l| l.split(',').next()).unwrap_or(""));
        match station::find_city(&relay, st, &typed).await {
            Ok(Some(found)) => {
                println!("  Found {} at {:.4}, {:.4}", found.0, found.1, found.2);
                break found;
            }
            _ if openwx_relay::prompt::input_ended() => anyhow::bail!("no main city chosen; run openwx setup in a terminal"),
            Ok(None) => println!("  Could not find \"{typed}\" in {st}. Check the spelling, or try a nearby larger town."),
            Err(e) => println!("  Lookup failed ({e}). Check the internet connection and try again."),
        }
    };
    let default_region = settings["coverage"]["name"].as_str().filter(|s| !s.is_empty()).map(str::to_owned).unwrap_or_else(|| {
        if counties.len() == 1 { counties[0].name.clone() } else { format!("{} & surrounding counties", counties[0].name) }
    });
    let region_name = ask("Name for your coverage area (e.g. \"Southern Indiana\")", &default_region);
    let zone = station::county_at(&relay, lat, lon).await.ok().flatten();
    let home_fips = zone.as_deref().and_then(|z| counties.iter().find(|c| c.zone == z)).or(counties.first()).map(|c| c.fips.clone()).unwrap_or_default();
    let region = station::Region { states: vec![st.to_owned()], counties, home_fips, city, state_of_city: st.to_owned(), lat, lon, name: region_name };
    station::apply_region(&mut settings, &region);

    // 4. Backend
    println!("\nSTEP 4 of 4 - Where the weather data comes from\n");
    let current = match settings["backend_mode"].as_str() { Some("remote") => 1, Some("direct") => 2, _ => 0 };
    let mode = choose(
        "OpenWX reads NOAA/NWS data through a small built-in server (OpenWX Relay).",
        &["On this PC (recommended: nothing else to install)", "A hosted OpenWX Relay on my own server", "Direct (troubleshooting only)"],
        current.min(1),
    );
    settings["backend_mode"] = json!(["local", "remote", "direct"][mode]);
    if mode == 1 {
        let url = ask_valid(
            "Relay address (https://your-server/t/your-token)",
            settings["remote_api_base"].as_str().unwrap_or(""),
            "Use the https:// address `openwx-relay setup` printed on the server.",
            |u| u.starts_with("https://") || u.starts_with("http://127.0.0.1"),
        );
        settings["remote_api_base"] = json!(url.trim_end_matches('/'));
    }

    // Check the feeds before saving.
    println!("\nChecking the weather feeds…");
    let relay = weather::relay(&settings)?;
    if let Ok(result) = weather::check(&relay).await {
        print_checks(&result);
    }
    if let Ok(now) = weather::overview(&relay, &settings, None, None).await {
        if let Some(t) = now["current"]["temperatureF"].as_f64() {
            println!("\n  Right now near {}: {:.0}°F, {}", region.city, t, now["current"]["textDescription"].as_str().unwrap_or("conditions unavailable"));
        }
    }

    if !yes_no("\nSave these settings?", true) {
        println!("Nothing saved.");
        return Ok(());
    }
    let path = station::save(&settings)?;
    println!("\nSaved {}.", path.display());
    println!("Start (or restart) OpenWX Broadcasting to use them. You can change anything later in Settings.");
    Ok(())
}

pub fn print_checks(result: &Value) {
    for c in result["checks"].as_array().into_iter().flatten() {
        let ok = c["ok"] == true;
        println!("  [{}] {:<28} {}", if ok { " OK " } else { "FAIL" }, c["name"].as_str().unwrap_or("?"), c["error"].as_str().unwrap_or(""));
    }
}
