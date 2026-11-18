//! `openwx-relay`: run OpenWX Relay as a server.
//!
//!   openwx-relay                     serve with the default config file
//!   openwx-relay --config FILE       serve with FILE (servers: /etc/openwx/relay.toml)
//!   openwx-relay setup [--config F]  answer a few questions and write the config file
//!   openwx-relay token               print a new random access token
//!   openwx-relay check [--config F]  check the config file and every weather feed, then exit
//!   openwx-relay --version

use std::path::PathBuf;

use openwx_relay::{config::new_token, prompt, RelayConfig, VERSION};

fn usage() {
    println!(
        "OpenWX Relay {VERSION}\n\n\
         Usage:\n  openwx-relay [serve] [--config FILE]\n  openwx-relay setup [--config FILE]\n  \
         openwx-relay check [--config FILE]\n  openwx-relay token\n  openwx-relay --version\n\n\
         Default config: {}",
        RelayConfig::default_path().display()
    );
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut config_path = std::env::var_os("OPENWX_RELAY_CONFIG").map(PathBuf::from).unwrap_or_else(RelayConfig::default_path);
    let mut command = "serve".to_owned();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--config" | "-c" => {
                i += 1;
                match args.get(i) {
                    Some(path) => config_path = PathBuf::from(path),
                    None => {
                        eprintln!("--config needs a file path");
                        std::process::exit(2);
                    }
                }
            }
            "--version" | "-V" => {
                println!("openwx-relay {VERSION}");
                return;
            }
            "--help" | "-h" | "help" => {
                usage();
                return;
            }
            other if !other.starts_with('-') => command = other.to_owned(),
            other => {
                eprintln!("unknown option {other}");
                usage();
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let code = match command.as_str() {
        "serve" => serve(&config_path),
        "setup" => setup(&config_path),
        "token" => {
            println!("{}", new_token());
            0
        }
        "check" => check(&config_path),
        _ => {
            usage();
            2
        }
    };
    std::process::exit(code);
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("tokio runtime")
}

fn serve(path: &PathBuf) -> i32 {
    let config = match RelayConfig::load(path) {
        Ok(config) => config,
        Err(error) if !path.exists() => {
            log::warn!("no config at {} ({error}); serving on 127.0.0.1 with defaults. Run `openwx-relay setup` to configure.", path.display());
            RelayConfig::default()
        }
        Err(error) => {
            eprintln!("config {}: {error}", path.display());
            return 1;
        }
    };
    if config.contact.trim().is_empty() {
        log::warn!("no contact set: NOAA/NWS ask every client for one (run `openwx-relay setup`)");
    }
    runtime().block_on(async {
        let handle = match openwx_relay::start(config, false).await {
            Ok(handle) => handle,
            Err(error) => {
                eprintln!("could not start: {error}");
                return 1;
            }
        };
        println!("OpenWX Relay {VERSION} on {}  (Ctrl+C stops)", handle.base_url());
        let _ = tokio::signal::ctrl_c().await;
        handle.stop();
        0
    })
}

fn setup(path: &PathBuf) -> i32 {
    println!("OpenWX Relay setup  -  {}\n", path.display());
    println!("Press Enter to keep the value in [brackets].\n");
    let mut c = RelayConfig::load_or_default(path);

    c.station = prompt::ask("Station name (shown to NOAA in requests)", &c.station);
    c.contact = prompt::ask_valid(
        "Contact e-mail or website (NOAA/NWS ask every app for one)",
        &c.contact,
        "Enter an e-mail address (name@example.org) or an https:// website.",
        prompt::looks_like_contact,
    );

    let modes = ["This computer only (127.0.0.1) - the desktop app on this PC", "A shared server other studio PCs reach over the network/internet"];
    let shared = prompt::choose("Who will use this relay?", &modes, usize::from(c.bind != "127.0.0.1")) == 1;
    if shared {
        c.bind = "0.0.0.0".into();
        if c.access_token.len() < 24 || prompt::yes_no("Make a new access token?", false) {
            c.access_token = new_token();
        }
        println!("\n  Put the relay behind HTTPS (Caddy or nginx; see server/README.md).");
        println!("  Studio PCs then use this backend URL:\n    https://YOUR-SERVER/t/{}\n", c.access_token);
    } else {
        c.bind = "127.0.0.1".into();
    }
    let port = prompt::ask_valid("Port", &c.port.to_string(), "Type a port number from 1024 to 65535.", |p| p.parse::<u16>().is_ok_and(|n| n >= 1024));
    c.port = port.parse().unwrap_or(c.port);

    let home = prompt::ask_valid(
        "Station home point as lat,lon (for the local SPC risk and lightning; blank to skip)",
        &c.home().map(|(a, b)| format!("{a},{b}")).unwrap_or_default(),
        "Use decimal degrees, e.g. 38.39,-86.93 (west is negative).",
        |s| s.is_empty() || parse_point(s).is_some(),
    );
    match parse_point(&home) {
        Some((lat, lon)) => {
            c.home_lat = Some(lat);
            c.home_lon = Some(lon);
        }
        None => {
            c.home_lat = None;
            c.home_lon = None;
        }
    }
    c.coverage_label = prompt::ask("Coverage area name (e.g. \"Southern Indiana\")", &c.coverage_label);
    c.open_meteo = prompt::yes_no("Allow Open-Meteo for the few features that still use it? (free for non-commercial use)", c.open_meteo);

    match c.save(path) {
        Ok(()) => {
            println!("\nSaved {}.", path.display());
            println!("Check every feed now with:  openwx-relay check --config \"{}\"", path.display());
            0
        }
        Err(error) => {
            eprintln!("\nNot saved: {error}");
            1
        }
    }
}

fn parse_point(s: &str) -> Option<(f64, f64)> {
    let (a, b) = s.split_once(',')?;
    let (lat, lon) = (a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?);
    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)).then_some((lat, lon))
}

fn check(path: &PathBuf) -> i32 {
    let config = match RelayConfig::load(path) {
        Ok(c) => {
            println!("Config {}: OK", path.display());
            c
        }
        Err(error) => {
            println!("Config {}: {error}  (checking feeds with defaults)", path.display());
            RelayConfig::default()
        }
    };
    let state = match openwx_relay::AppState::new(RelayConfig { access_token: String::new(), ..config }) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    runtime().block_on(async move {
        let request = axum::http::Request::get("/api/openwx/check").body(axum::body::Body::empty()).unwrap();
        let response = openwx_relay::oneshot(state, request).await;
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20).await.unwrap_or_default();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
        let mut all = true;
        for c in value["checks"].as_array().into_iter().flatten() {
            let ok = c["ok"] == true;
            all &= ok;
            println!("  [{}] {:<28} {:>5} ms  {}", if ok { " OK " } else { "FAIL" }, c["name"].as_str().unwrap_or("?"), c["ms"], c["error"].as_str().unwrap_or(""));
        }
        if all { 0 } else { 1 }
    })
}
