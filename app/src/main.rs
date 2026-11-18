//! OpenWXBroadcasting - broadcast weather radar, graphics and presentation suite.
//! Community edition: an on-device backend (OpenWX Relay), setup wizard and modular feeds.

// A GUI app: no console window, in debug or release builds. Logs go to a file instead.
#![windows_subsystem = "windows"]

mod app;
mod audio;
mod backend;
mod audio_catalog;
mod audio_mix;
mod astro;
mod autoshow;
mod bench;
mod bugreport;
mod broadcast_ops;
mod cap;
mod cells;
mod channels;
mod collections;
mod acis;
mod climate;
mod clip;
mod community;
mod config;
mod coverage;
mod customdata;
mod data;
mod dcweather;
mod social_graphics;
mod desk;
mod ensemble;
mod eoc;
mod feeds;
mod forecast_edit;
mod formats;
mod devlog;
mod gis;
mod grid;
mod futurecast;
mod hazards;
mod history;
mod homecast;
mod gfx;
mod jobs;
mod library;
mod map;
mod maptheme;
mod net;
mod nowcast;
mod obs;
mod outages;
mod ops;
mod places;
mod quakes;
mod portal;
mod program;
mod publish;
mod record;
mod show;
mod shows;
mod sounding;
mod sources;
mod twc;
mod travel;
mod switcher;
mod ui;
mod updater;
mod verify;
mod workflow;
mod winter;

use gpui_kit::{component::TitleBar, *};

/// Log to %LOCALAPPDATA%\OpenWXBroadcasting\logs (Settings > About shows the path).
/// The previous run's log is kept as .old.log.
fn init_logging() {
    let mut builder = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"),
    );
    let path = config::log_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::rename(&path, path.with_extension("old.log"));
    if let Ok(file) = std::fs::File::create(&path) {
        builder.target(env_logger::Target::Pipe(Box::new(file)));
    }
    devlog::install(builder);
    // Panics are logged too, since there is no console to print them.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}");
        default_hook(info);
    }));
}

fn main() {
    init_logging();
    // Start the network runtime before the UI so the first data refresh is immediate.
    let _ = net::runtime();
    // Create the library folders on first run.
    let _ = config::LibraryPaths::get();

    gpui_kit::application()
        .with_assets(crate::ui::icons::AppAssets)
        .run(|cx: &mut App| {
            gpui_kit::init(cx);
            ui::apply_component_theme(cx);
            app::init(cx);

            let state = app::app_entity(cx);
            let frames = app::frames_entity(cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Maximized(Bounds::centered(None, size(px(1600.), px(960.)), cx))),
                window_min_size: Some(size(px(960.), px(600.))),
                app_id: Some("openwx-broadcaster".into()),
                ..TitleBar::window_options()
            };
            let opened = gpui_kit::open_window(options, cx, move |window, cx| {
                window.set_window_title("OpenWX Broadcasting");
                cx.new(|cx| ui::shell::MainView::new(state, frames, window, cx))
            });
            if let Err(e) = opened {
                log::error!("could not open the main window: {e:?}");
                cx.quit();
            }
            cx.activate(true);
        });
}
