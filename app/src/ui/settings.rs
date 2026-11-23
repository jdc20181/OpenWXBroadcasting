//! Settings: station, branding, data sources, map defaults, on-air elements, recording, updates.

use gpui_kit::{
    component::input::{Input, InputEvent, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::{AppState, Modal, ToastKind},
    config::{Basemap, RecordFormat, Settings, Units},
    data::RADAR_PALETTES,
    gfx::text::family_names,
    net,
    ui::{button, button_primary, button_small, c, chip, col, heading, hex_color, label, muted, row, slider, toggle, BORDER, MUTED, PANEL_2, TEXT},
};

#[path = "settings_coverage.rs"]
mod coverage_tab;

const TEXT_FIELDS: usize = 44;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsCategory {
    General,
    Weather,
    Audio,
    Output,
    Branding,
    Integrations,
    System,
    Coverage,
}

impl SettingsCategory {
    const ALL: [Self; 8] = [Self::General, Self::Branding, Self::Weather, Self::Audio, Self::Output, Self::Integrations, Self::System, Self::Coverage];

    fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Weather => "Weather & maps",
            Self::Audio => "Audio",
            Self::Output => "On-air & output",
            Self::Branding => "Branding & sponsors",
            Self::Integrations => "Integrations",
            Self::System => "System",
            Self::Coverage => "Coverage area",
        }
    }
}

pub struct SettingsView {
    app: Entity<AppState>,
    draft: Settings,
    fields: Vec<Entity<InputState>>,
    sponsor_rows: Vec<SponsorRow>,
    category: SettingsCategory,
    test_result: Option<(bool, String)>,
    testing: bool,
    /// OBS audio inputs: (name, volume 0-1, muted).
    mixer: Vec<(String, f32, bool)>,
    /// DCWeather sign-in in progress (code shown to the user), and the last account message.
    dcw_link: Option<crate::dcweather::Link>,
    dcw_msg: Option<(bool, String)>,
    /// Coverage area tab: region name, county search, one town field per supported county,
    /// and the clickable county map.
    cov_name: Entity<InputState>,
    cov_search: Entity<InputState>,
    cov_query: String,
    cov_towns: Vec<(String, Entity<InputState>)>,
    cov_slot: std::rc::Rc<crate::ui::frame_view::ImageSlot>,
    cov_image: Option<(String, std::sync::Arc<RenderImage>)>,
    _subscriptions: Vec<Subscription>,
}

struct SponsorRow {
    name: Entity<InputState>,
    tagline: Entity<InputState>,
    logo_media: Option<String>,
}

impl SponsorRow {
    fn new(sponsor: crate::config::Sponsor, window: &mut Window, cx: &mut Context<SettingsView>) -> Self {
        Self {
            name: cx.new(|cx| InputState::new(window, cx).placeholder("Sponsor name").default_value(sponsor.name)),
            tagline: cx.new(|cx| InputState::new(window, cx).placeholder("Sponsor line").default_value(sponsor.tagline)),
            logo_media: sponsor.logo_media,
        }
    }
}

fn field_values(s: &Settings) -> [String; TEXT_FIELDS] {
    [
        s.station_name.clone(),
        s.station_short.clone(),
        s.tagline.clone(),
        s.home_label.clone(),
        format!("{:.4}", s.home.lat),
        format!("{:.4}", s.home.lon),
        s.remote_api_base.clone(),
        s.alert_area.clone(),
        s.update_feed.clone(),
        s.brand.primary.clone(),
        s.brand.secondary.clone(),
        s.brand.alert.clone(),
        s.brand.text.clone(),
        s.obs_url.clone(),
        s.obs_password.clone(),
        s.obs_severe_scene.clone(),
        s.obs_normal_scene.clone(),
        s.sponsor_name.clone(),
        s.sponsor_tagline.clone(),
        s.control_port.to_string(),
        s.operator_name.clone(),
        s.sync_folder.clone(),
        s.notify_webhook.clone(),
        s.gpo_webhook.clone(),
        s.tally_webhook.clone(),
        s.companion_url.clone(),
        s.companion_cut.clone(),
        s.companion_auto.clone(),
        s.companion_severe.clone(),
        s.tricaster_url.clone(),
        s.osc_port.to_string(),
        s.mos_port.to_string(),
        s.mos_id.clone(),
        s.cap_feed_url.clone(),
        s.trusted_senders.join(", "),
        s.obs_mic_input.clone(),
        String::new(),
        s.webcams
            .iter()
            .map(|c| format!("{} | {} | {:.4}, {:.4} | {} | {}", c.name, c.url, c.lat, c.lon, c.ptz, c.credit))
            .collect::<Vec<_>>()
            .join("; "),
        s.travel_routes.iter().map(|r| format!("{} > {}", r.from_label, r.to_label)).collect::<Vec<_>>().join("; "),
        s.shared_graphics_folder.clone(),
        s.youtube_channel_id.clone(),
        s.youtube_live_url.clone(),
        s.radar_upstream.clone(),
        s.contact.clone(),
    ]
}

const FIELD_LABELS: [&str; TEXT_FIELDS] = [
    "Station name",
    "Short name (wordmark)",
    "Tagline",
    "Home location label",
    "Home latitude",
    "Home longitude",
    "Hosted relay address (https://your-server/t/access-token)",
    "NWS alert area (blank = all US, or IN,KY,IL)",
    "Updates from (GitHub repository, or a latest.json feed URL)",
    "Primary color",
    "Secondary color",
    "Alert color",
    "Text color",
    "OBS WebSocket address",
    "OBS WebSocket password",
    "OBS scene for severe weather",
    "OBS scene for normal coverage",
    "Sponsor name",
    "Sponsor line (blank = Brought to you by …)",
    "Control server port (default 8765)",
    "Operator name (acknowledgments and audit log)",
    "Shared sync folder for warning status (blank = this PC only)",
    "Notification webhook (Slack / Teams / Discord)",
    "On-air webhook (GPO, fired when a warning airs)",
    "Tally webhook (fired when Program changes)",
    "Companion address (http://127.0.0.1:8000)",
    "Companion button for Cut (page/row/column)",
    "Companion button for Auto transition",
    "Companion button for Severe Mode",
    "TriCaster address (http://192.168.1.50), blank = off",
    "OSC port (default 9000)",
    "MOS port (default 10540)",
    "MOS ID",
    "CAP feed URL (Atom feed or CAP message), blank = off",
    "Extra trusted CAP senders (comma separated)",
    "OBS audio input to watch for silence",
    "Profile name, e.g. Studio A",
    "Cameras: Name | image URL | lat, lon | PTZ URL with {move} | credit; next camera…",
    "Travel routes: Evansville > Louisville; Evansville > Indianapolis (blank = automatic)",
    "Shared graphics collection folder (network drive or OneDrive)",
    "YouTube channel id for the mobile app (starts with UC…)",
    "YouTube live link for the mobile app (e.g. https://www.youtube.com/@YourChannel/live)",
    "Radar server (blank = public LibreWXR API, or your own https://radar.example.org)",
    "Contact e-mail or website (sent to NOAA/NWS with data requests)",
];

impl SettingsView {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let draft = app.read(cx).settings.clone();
        let values = field_values(&draft);
        let fields = values
            .iter()
            .zip(FIELD_LABELS)
            .map(|(value, placeholder)| {
                let value = value.clone();
                cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(value))
            })
            .collect();
        let sponsor_rows = draft.sponsors.iter().cloned().map(|sponsor| SponsorRow::new(sponsor, window, cx)).collect();
        let cov_name = cx.new(|cx| InputState::new(window, cx).placeholder("Region name (blank = home & surrounding counties)").default_value(draft.coverage.name.clone()));
        let cov_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search a county to add"));
        let _subscriptions = vec![cx.subscribe_in(&cov_search, window, |this, state, e: &InputEvent, _, cx| {
            if matches!(e, InputEvent::Change) {
                this.cov_query = state.read(cx).value().to_string();
                cx.notify();
            }
        })];
        Self {
            app,
            draft,
            fields,
            sponsor_rows,
            category: SettingsCategory::General,
            test_result: None,
            testing: false,
            mixer: Vec::new(),
            dcw_link: None,
            dcw_msg: None,
            cov_name,
            cov_search,
            cov_query: String::new(),
            cov_towns: Vec::new(),
            cov_slot: crate::ui::frame_view::ImageSlot::new(),
            cov_image: None,
            _subscriptions,
        }
    }

    /// DCWeather device link: get a code, open the approval page, poll until approved.
    fn dcw_sign_in(&mut self, cx: &mut Context<Self>) {
        let base = self.draft.api_base.clone();
        let device = format!("OpenWX on {}", std::env::var("COMPUTERNAME").unwrap_or_else(|_| "this PC".into()));
        self.dcw_msg = Some((true, "Asking DCWeather for a code…".into()));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let link = match net::spawn(crate::dcweather::start_link(base.clone(), device)).await {
                Ok(Ok(l)) => l,
                Ok(Err(e)) => {
                    let _ = this.update(cx, |this, cx| {
                        this.dcw_msg = Some((false, format!("DCWeather: {e}")));
                        cx.notify();
                    });
                    return;
                }
                Err(e) => {
                    let _ = this.update(cx, |this, cx| {
                        this.dcw_msg = Some((false, format!("DCWeather: {e}")));
                        cx.notify();
                    });
                    return;
                }
            };
            let _ = open::that(&link.approve_url);
            let (secret, expires) = (link.device_secret.clone(), link.expires_in);
            let _ = this.update(cx, |this, cx| {
                this.dcw_msg = Some((true, "Approve the code in your browser (sign in to DCWeather if asked).".into()));
                this.dcw_link = Some(link);
                cx.notify();
            });
            let started = std::time::Instant::now();
            loop {
                cx.background_executor().timer(std::time::Duration::from_secs(3)).await;
                // Stop if the user cancelled.
                if this.update(cx, |this, _| this.dcw_link.is_none()).unwrap_or(true) {
                    return;
                }
                let r = net::spawn(crate::dcweather::poll_link(base.clone(), secret.clone())).await;
                let done = match r {
                    Ok(Ok(crate::dcweather::Poll::Approved)) => {
                        let who = net::spawn(crate::dcweather::check(base.clone())).await;
                        Some(match who {
                            Ok(Ok(w)) => (true, format!("Signed in as {w}")),
                            Ok(Err(e)) => (false, format!("Signed in, but the check failed: {e}")),
                            Err(e) => (false, format!("Signed in, but the check failed: {e}")),
                        })
                    }
                    Ok(Ok(crate::dcweather::Poll::Expired)) => Some((false, "The code expired. Try again.".into())),
                    Ok(Ok(crate::dcweather::Poll::Pending)) if started.elapsed().as_secs() > expires => Some((false, "The code expired. Try again.".into())),
                    Ok(Ok(crate::dcweather::Poll::Pending)) => None,
                    Ok(Err(e)) => Some((false, format!("DCWeather: {e}"))),
                    Err(e) => Some((false, format!("DCWeather: {e}"))),
                };
                if let Some(msg) = done {
                    let _ = this.update(cx, |this, cx| {
                        this.dcw_link = None;
                        this.dcw_msg = Some(msg);
                        cx.notify();
                    });
                    return;
                }
            }
        })
        .detach();
    }

    fn dcw_check(&mut self, cx: &mut Context<Self>) {
        let base = self.draft.api_base.clone();
        cx.spawn(async move |this, cx| {
            let r = net::spawn(crate::dcweather::check(base)).await;
            let _ = this.update(cx, |this, cx| {
                this.dcw_msg = Some(match r {
                    Ok(Ok(w)) => (true, format!("Signed in as {w}")),
                    Ok(Err(e)) => (false, format!("DCWeather: {e}")),
                    Err(e) => (false, format!("DCWeather: {e}")),
                });
                cx.notify();
            });
        })
        .detach();
    }

    /// Adds the site's active sponsors (with logos) to the lineup; manual ones are kept.
    fn dcw_import_sponsors(&mut self, cx: &mut Context<Self>) {
        let base = self.draft.api_base.clone();
        let existing: Vec<crate::config::Sponsor> = self
            .sponsor_rows
            .iter()
            .map(|r| crate::config::Sponsor { name: r.name.read(cx).value().trim().to_string(), tagline: String::new(), logo_media: None })
            .collect();
        self.dcw_msg = Some((true, "Importing sponsors from DCWeather…".into()));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = net::spawn(crate::dcweather::import_sponsors(base, existing)).await;
            let _ = this.update_in(cx, |this, window, cx| {
                match r {
                    Ok(Ok(list)) => {
                        let n = list.len();
                        for s in list {
                            this.sponsor_rows.push(SponsorRow::new(s, window, cx));
                        }
                        this.dcw_msg = Some((true, if n == 0 { "No new sponsors: every active DCWeather sponsor is already in the lineup.".into() } else { format!("Added {n} sponsor(s) from DCWeather. Save to keep them.") }));
                    }
                    Ok(Err(e)) => this.dcw_msg = Some((false, format!("DCWeather: {e}"))),
                    Err(e) => this.dcw_msg = Some((false, format!("DCWeather: {e}"))),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn dcweather_account(&self, cx: &mut Context<Self>) -> Div {
        let base = self.draft.api_base.clone();
        let signed_in = crate::dcweather::has_token(&base);
        group("DCWeather account")
            .child(muted(format!(
                "Sign this studio in to {} to use its sponsor list and post graphics to its Facebook page (Publishing > DCWeather Facebook). Your password stays on the site: you approve a short code there. The site address is the data address under General.",
                if base.trim().is_empty() { "dcweather.xyz" } else { base.trim() }
            )))
            .when(!signed_in, |g| {
                // How to get the code (owner: "no instructions on how to get a broadcast code").
                let step = |n: &str, text: &str| {
                    row()
                        .w_full()
                        .min_w_0()
                        .items_start()
                        .gap(px(8.))
                        .child(div().flex_shrink_0().w(px(18.)).text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(c(0x22C7E8)).child(n.to_string()))
                        .child(div().flex_1().min_w_0().whitespace_normal().text_size(px(12.)).text_color(c(TEXT)).child(text.to_string()))
                };
                g.child(
                    col()
                        .w_full()
                        .gap(px(4.))
                        .p(px(10.))
                        .rounded(px(6.))
                        .bg(c(0x0B1220))
                        .child(div().text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).child("How to link this studio (getting the code)"))
                        .child(step("1", "Check that the data address under General is your DCWeather site."))
                        .child(step("2", "Click Sign in with DCWeather below. A short code appears here, and your browser opens the site's Link a Studio page."))
                        .child(step("3", "On that page, sign in with an admin account if asked. Check that the code there matches the one here, then click Approve."))
                        .child(step("4", "Within a few seconds this box says Signed in. The code expires after a few minutes; click Sign in again for a new one."))
                        .child(step("5", "If the browser didn't open, use Open the page again, or open the site's admin menu > Broadcaster Studios and type the code there. The same page lists linked studios, and removes them.")),
                )
            })
            .when_some(self.dcw_link.clone(), |g, link| {
                g.child(row().gap(px(10.)).child(div().text_size(px(26.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).child(link.code.clone())).child(muted("Enter or approve this code on the DCWeather page that just opened.")))
                    .child(row().gap(px(6.)).child(button_small("dcw-open", "Open the page again").on_click(move |_, _, _| {
                        let _ = open::that(&link.approve_url);
                    })).child(button_small("dcw-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
                        this.dcw_link = None;
                        this.dcw_msg = None;
                        cx.notify();
                    }))))
            })
            .child(
                row()
                    .flex_wrap()
                    .gap(px(6.))
                    .when(!signed_in && self.dcw_link.is_none(), |r| r.child(button_small("dcw-in", "Sign in with DCWeather").on_click(cx.listener(|this, _, _, cx| this.dcw_sign_in(cx)))))
                    .when(signed_in, |r| {
                        r.child(button_small("dcw-check", "Check sign-in").on_click(cx.listener(|this, _, _, cx| this.dcw_check(cx))))
                            .child(button_small("dcw-out", "Sign out").on_click(cx.listener(|this, _, _, cx| {
                                crate::dcweather::sign_out(&this.draft.api_base);
                                this.dcw_msg = Some((true, "Signed out. The studio's link can also be removed on the site (Admin > Broadcaster Studios).".into()));
                                cx.notify();
                            })))
                    }),
            )
            .when_some(self.dcw_msg.clone().or_else(|| crate::dcweather::status().map(|s| (true, format!("Signed in as {s}")))), |g, (ok, msg)| {
                g.child(div().text_size(px(12.)).text_color(if ok { c(0x3DD68C) } else { c(0xE5383B) }).child(msg))
            })
    }

    /// Refresh the draft from the live settings (called when the dialog opens).
    pub fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft = self.app.read(cx).settings.clone();
        self.sponsor_rows = self.draft.sponsors.iter().cloned().map(|sponsor| SponsorRow::new(sponsor, window, cx)).collect();
        for (input, value) in self.fields.clone().iter().zip(field_values(&self.draft)) {
            input.update(cx, |state, cx| state.set_value(value, window, cx));
        }
        self.test_result = None;
        cx.notify();
    }

    fn value(&self, i: usize, cx: &App) -> String {
        self.fields[i].read(cx).value().trim().to_string()
    }

    fn collect(&self, cx: &App) -> Settings {
        let mut s = self.draft.clone();
        s.coverage.name = self.cov_name.read(cx).value().trim().to_string();
        for c in &mut s.coverage.counties {
            if let Some((_, input)) = self.cov_towns.iter().find(|(f, _)| *f == c.fips) {
                c.town = input.read(cx).value().trim().chars().take(40).collect();
            }
        }
        s.station_name = self.value(0, cx);
        s.station_short = self.value(1, cx);
        s.tagline = self.value(2, cx);
        s.home_label = self.value(3, cx);
        if let Ok(lat) = self.value(4, cx).parse::<f64>() {
            s.home.lat = lat.clamp(-85.0, 85.0);
        }
        if let Ok(lon) = self.value(5, cx).parse::<f64>() {
            s.home.lon = lon.clamp(-180.0, 180.0);
        }
        let api = self.value(6, cx);
        if api.is_empty() || api.starts_with("http") {
            s.remote_api_base = api.trim_end_matches('/').to_string();
        }
        let radar = self.value(TEXT_FIELDS - 2, cx);
        if radar.is_empty() || radar.starts_with("https://") {
            s.radar_upstream = radar.trim_end_matches('/').to_string();
        }
        let contact = self.value(TEXT_FIELDS - 1, cx);
        if contact.is_empty() || openwx_relay::prompt::looks_like_contact(&contact) {
            s.contact = contact;
        }
        s.alert_area = self.value(7, cx).to_uppercase();
        s.update_feed = self.value(8, cx);
        for (i, target) in [9, 10, 11, 12].into_iter().zip([0, 1, 2, 3]) {
            let v = self.value(i, cx);
            if crate::gfx::parse_hex(&v).is_some() {
                match target {
                    0 => s.brand.primary = v,
                    1 => s.brand.secondary = v,
                    2 => s.brand.alert = v,
                    _ => s.brand.text = v,
                }
            }
        }
        s.obs_url = self.value(13, cx);
        s.obs_password = self.value(14, cx);
        s.obs_severe_scene = self.value(15, cx);
        s.obs_normal_scene = self.value(16, cx);
        s.sponsor_name = self.value(17, cx);
        s.sponsor_tagline = self.value(18, cx);
        s.sponsors = self.sponsor_rows.iter().filter_map(|row| {
            let name = row.name.read(cx).value().trim().to_owned();
            if name.is_empty() { return None; }
            Some(crate::config::Sponsor {
                name,
                tagline: row.tagline.read(cx).value().trim().to_owned(),
                logo_media: row.logo_media.clone(),
            })
        }).collect();
        if let Ok(port) = self.value(19, cx).parse::<u16>() {
            if port >= 1024 {
                s.control_port = port;
            }
        }
        s.operator_name = self.value(20, cx);
        s.sync_folder = self.value(21, cx);
        s.notify_webhook = self.value(22, cx);
        s.gpo_webhook = self.value(23, cx);
        s.tally_webhook = self.value(24, cx);
        s.companion_url = self.value(25, cx).trim_end_matches('/').to_string();
        s.companion_cut = self.value(26, cx);
        s.companion_auto = self.value(27, cx);
        s.companion_severe = self.value(28, cx);
        s.tricaster_url = self.value(29, cx).trim_end_matches('/').to_string();
        if let Ok(port) = self.value(30, cx).parse::<u16>() {
            if port >= 1024 {
                s.osc_port = port;
            }
        }
        if let Ok(port) = self.value(31, cx).parse::<u16>() {
            if port >= 1024 {
                s.mos_port = port;
            }
        }
        s.mos_id = self.value(32, cx);
        s.cap_feed_url = self.value(33, cx);
        s.trusted_senders = self.value(34, cx).split(',').map(str::trim).filter(|x| !x.is_empty()).map(String::from).collect();
        s.obs_mic_input = self.value(35, cx);
        s.webcams = self
            .value(37, cx)
            .split(';')
            .filter_map(|entry| {
                let parts: Vec<&str> = entry.split('|').map(str::trim).collect();
                if parts.len() < 2 || parts[0].is_empty() || !parts[1].starts_with("http") {
                    return None;
                }
                let (lat, lon) = parts.get(2).and_then(|p| crate::places::parse_lat_lon(p)).unwrap_or((0.0, 0.0));
                Some(crate::travel::WebCam {
                    name: parts[0].to_string(),
                    url: parts[1].to_string(),
                    lat,
                    lon,
                    ptz: parts.get(3).map(|s| s.to_string()).unwrap_or_default(),
                    credit: parts.get(4).map(|s| s.to_string()).unwrap_or_default(),
                    tags: Vec::new(),
                })
            })
            .collect();
        s.travel_routes = self
            .value(38, cx)
            .split(';')
            .filter_map(|entry| {
                let (a, b) = entry.split_once('>')?;
                let from = crate::places::search(a.trim(), 1).into_iter().next()?;
                let to = crate::places::search(b.trim(), 1).into_iter().next()?;
                Some(crate::travel::TravelRoute { from_label: from.name.clone(), from: (from.lat, from.lon), to_label: to.name.clone(), to: (to.lat, to.lon) })
            })
            .collect();
        s.shared_graphics_folder = self.value(39, cx);
        s.youtube_channel_id = self.value(40, cx);
        s.youtube_live_url = self.value(41, cx);
        s
    }

    fn operations_groups(&self, d: &Settings, cx: &mut Context<Self>) -> Div {
        let stats = crate::broadcast_ops::source_stats();
        col().gap(px(14.))
            .when(self.category == SettingsCategory::Integrations && self.draft.site_integration, |body| body.child(self.dcweather_account(cx)))
            .when(self.category == SettingsCategory::Integrations, |body| body.child(
                group("Mobile app (YouTube)")
                    .child(muted("The app reads /public/shows.json from the viewer portal: live = OBS is streaming (shows the live link), otherwise the newest uploads from this channel."))
                    .child(self.field(40))
                    .child(self.field(41))
            ))
            .when(self.category == SettingsCategory::Integrations, |body| body.child(
                group("Viewer portal")
                    .child(toggle("set-portal", "Viewer portal: photos, video, reports, questions and polls", d.portal_enabled).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.portal_enabled = !this.draft.portal_enabled;
                        cx.notify();
                    })))
                    .child(muted(format!("Viewers open http://<this PC or your public address>:{}/  • Forward that port on your router or put it behind your website's proxy. Restart after changing.", d.portal_port)))
                    .child(row().gap(px(4.)).children([8766u16, 8080, 9090].into_iter().enumerate().map(|(i, p)| {
                        chip(("portal-port", i), format!("Port {p}"), d.portal_port == p).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.portal_port = p;
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
            ))
            .when(self.category == SettingsCategory::Weather, |body| body.child(
                group("Historical climate")
                    .child(muted("Normals, freeze dates and growing season come from NOAA's 1991-2020 normals for the nearest station. No download quota is used."))
                    .child(toggle("set-legacy-climate", "Also download daily records and past-year history (NOAA station data)", d.fetch_legacy_climate_history).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.fetch_legacy_climate_history = !this.draft.fetch_legacy_climate_history;
                        cx.notify();
                    })))
                    .child(muted("Daily records, yesterday, month and year to date, season snowfall and this date in past years come from the nearest NOAA climate station with a long record (ACIS, the data behind NWS NOWData). No relay quota is used.")),
            ))
            .when(self.category == SettingsCategory::Weather, |body| body.child(
                group("Warning desk")
                    .child(toggle("set-approval", "Graphics must be approved before they go to air", d.require_approval).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.require_approval = !this.draft.require_approval;
                        cx.notify();
                    })))
                    .child(toggle("set-failover", "Automation failover during shows (skip stale scenes, backup scene if Program goes black)", d.failover_enabled).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.failover_enabled = !this.draft.failover_enabled;
                        cx.notify();
                    })))
                    .child(self.field(20))
                    .child(self.field(21))
                    .child(muted("With a shared folder (network drive, OneDrive), acknowledgments, assignments and on-air marks are shared by every workstation."))
                    .child(self.field(22))
                    .child(self.field(23))
                    .child(self.field(24))
                    .child(
                        row()
                            .flex_wrap()
                            .gap(px(6.))
                            .child(button_small("set-notify-test", "Send a test notification").on_click(cx.listener(|this, _, _, cx| {
                                let hook = this.value(22, cx);
                                crate::broadcast_ops::notify("OpenWX test", "Notifications are working", &hook);
                            })))
                            .child(button_small("set-rehearse", "Rehearse an outage (20 s)").on_click(cx.listener(|this, _, _, cx| {
                                this.app.update(cx, |s, cx| {
                                    s.modal = Modal::None;
                                    s.rehearse_outage();
                                    cx.notify();
                                });
                            })))
                            .child(button_small("set-incidents", "Incident report (7 days)").on_click(cx.listener(|this, _, _, cx| {
                                match crate::broadcast_ops::incident_report(7) {
                                    Ok(path) => {
                                        let _ = open::that(&path);
                                    }
                                    Err(e) => this.app.update(cx, |s, cx| {
                                        s.toast(ToastKind::Error, format!("Could not write the report: {e}"));
                                        cx.notify();
                                    }),
                                }
                            }))),
                    )
            ))
            .when(self.category == SettingsCategory::Audio, |body| body.child(
                group("Presentation audio")
                    .child(toggle("set-presentation-mute", "Mute presentation audio", d.presentation_muted).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.presentation_muted = !this.draft.presentation_muted;
                        cx.notify();
                    })))
                    .child(muted("After you save, this controls scene soundtracks, graphics, video clips and scheduled cues. Warning tones and OBS inputs have separate controls."))
                    .child(row().justify_between()
                        .child(label("Soundtrack, graphic and clip volume"))
                        .child(muted(format!("{:.0}%", d.graphic_volume * 100.0))))
                    .child(slider("set-graphic-volume", d.graphic_volume, 0.0, 2.0, cx, |this, volume, _, cx| {
                        this.draft.graphic_volume = volume;
                        cx.notify();
                    }))
            ))
            .when(self.category == SettingsCategory::Audio, |body| body.child(
                group("Audio devices and warning tones")
                    .child({
                        let m = crate::audio::meter();
                        let fill = ((m.peak_db + 60.0) / 60.0).clamp(0.0, 1.0);
                        col()
                            .gap(px(4.))
                            .child(row().h(px(12.)).w_full().rounded(px(3.)).bg(c(0x1A2030)).child(div().h_full().w(relative(fill)).rounded(px(3.)).bg(c(if m.peak_db > -3.0 { 0xE5383B } else if m.peak_db > -12.0 { 0xE0A030 } else { 0x3DD68C }))))
                            .child(muted(if m.active { format!("Input peak {:.0} dBFS • loudness {:.0} LUFS", m.peak_db, m.short_lufs) } else { "Input meter: no input yet".to_string() }))
                    })
                    .child(muted("Soundtrack and cue output"))
                    .child(row().flex_wrap().gap(px(3.)).children(std::iter::once(String::new()).chain(crate::audio::output_devices()).enumerate().map(|(i, n)| {
                        let label = if n.is_empty() { "Default".to_string() } else { n.clone() };
                        let on = d.audio_program_device == n;
                        chip(("aud-out", i), label, on).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.audio_program_device = n.clone();
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(muted("Alert tone output (e.g. the studio monitors or the IFB)"))
                    .child(row().flex_wrap().gap(px(3.)).children(std::iter::once(String::new()).chain(crate::audio::output_devices()).enumerate().map(|(i, n)| {
                        let label = if n.is_empty() { "Same as soundtrack".to_string() } else { n.clone() };
                        let on = d.audio_alert_device == n;
                        chip(("aud-alert", i), label, on).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.audio_alert_device = n.clone();
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(muted("Microphone / input (meters, voice-over, ducking)"))
                    .child(row().flex_wrap().gap(px(3.)).children(std::iter::once(String::new()).chain(crate::audio::input_devices()).enumerate().map(|(i, n)| {
                        let label = if n.is_empty() { "Default".to_string() } else { n.clone() };
                        let on = d.audio_input_device == n;
                        chip(("aud-in", i), label, on).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.audio_input_device = n.clone();
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(muted("Duck soundtracks when the mic is louder than"))
                    .child(row().flex_wrap().gap(px(3.)).children([(0.0f32, "Off"), (-40.0, "-40 dB"), (-30.0, "-30 dB"), (-20.0, "-20 dB")].into_iter().enumerate().map(|(i, (v, t))| {
                        chip(("aud-duck", i), t, (d.duck_threshold_db - v).abs() < 0.1).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.duck_threshold_db = v;
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(muted("Audio delay to match video (lip sync)"))
                    .child(row().flex_wrap().gap(px(3.)).children([0u32, 40, 80, 120, 200].into_iter().enumerate().map(|(i, v)| {
                        chip(("aud-off", i), format!("{v} ms"), d.audio_offset_ms == v).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.audio_offset_ms = v;
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(muted("Alert sound (a media file, or the system sound)"))
                    .child(row().flex_wrap().gap(px(3.)).children(std::iter::once(String::new()).chain(self.app.read(cx).media.iter().map(|m| m.name.clone()).filter(|n| [".mp3", ".wav", ".ogg"].iter().any(|e| n.to_lowercase().ends_with(e)))).enumerate().map(|(i, n)| {
                        let label = if n.is_empty() { "System sound".to_string() } else { n.clone() };
                        let on = d.alert_sound_file == n;
                        chip(("aud-alertfile", i), label, on).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.alert_sound_file = n.clone();
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(
                        row()
                            .flex_wrap()
                            .gap(px(6.))
                            .child(button_small("vo-rec", if crate::audio::recording() { "Stop voice-over recording" } else { "Record a voice-over" }).on_click(cx.listener(|this, _, _, cx| {
                                if crate::audio::recording() {
                                    let result = crate::audio::stop_recording(48_000, 1);
                                    this.app.update(cx, |s, cx| {
                                        match result {
                                            Ok(p) => {
                                                s.toast(ToastKind::Success, format!("Saved {}", p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()));
                                                s.reload_library();
                                            }
                                            Err(e) => s.toast(ToastKind::Error, format!("Recording: {e}")),
                                        }
                                        cx.notify();
                                    });
                                } else {
                                    crate::audio::start_recording(48_000);
                                }
                                cx.notify();
                            })))
                            .child(button_small("aud-test", "Test alert tone").on_click(|_, _, _| crate::obs::play_alert_sound(false)))
                            .child(button_small("aud-mixer", "OBS mixer…").on_click(cx.listener(|this, _, _, cx| {
                                let s = this.app.read(cx).settings.clone();
                                cx.spawn(async move |this, cx| {
                                    let inputs = crate::net::spawn(crate::switcher::mixer_inputs(s)).await.ok().and_then(|r| r.ok()).unwrap_or_default();
                                    let _ = this.update(cx, |this, cx| {
                                        this.mixer = inputs;
                                        cx.notify();
                                    });
                                })
                                .detach();
                            }))),
                    )
                    .children(self.mixer.clone().into_iter().enumerate().map(|(i, (name, vol, muted_now))| {
                        let (n1, n2) = (name.clone(), name.clone());
                        row()
                            .gap(px(6.))
                            .child(div().w(px(160.)).truncate().text_size(px(12.)).text_color(c(TEXT)).child(name.clone()))
                            .child(div().flex_1().child(slider(("mix-vol", i), vol, 0.0, 1.0, cx, move |this, v, _, cx| {
                                let s = this.app.read(cx).settings.clone();
                                let n = n1.clone();
                                if let Some(m) = this.mixer.iter_mut().find(|m| m.0 == n) {
                                    m.1 = v;
                                }
                                crate::net::spawn(crate::switcher::mixer_set(s, n, Some(v), None));
                                cx.notify();
                            })))
                            .child(chip(("mix-mute", i), "Mute", muted_now).on_click(cx.listener(move |this, _, _, cx| {
                                let s = this.app.read(cx).settings.clone();
                                let n = n2.clone();
                                if let Some(m) = this.mixer.iter_mut().find(|m| m.0 == n) {
                                    m.2 = !m.2;
                                    crate::net::spawn(crate::switcher::mixer_set(s, n.clone(), None, Some(m.2)));
                                }
                                cx.notify();
                            })))
                    }).collect::<Vec<_>>())
            ))
            .when(self.category == SettingsCategory::Weather, |body| body.child(
                group("Camera moves")
                    .child(muted("How the map camera flies between views (scene changes, zoom to warning, tours)."))
                    .child(row().flex_wrap().gap(px(4.)).children([("smooth", "Smooth"), ("gentle", "Gentle"), ("ease-out", "Fast start"), ("ease-in", "Slow start"), ("linear", "Constant")].into_iter().enumerate().map(|(i, (key, text))| {
                        chip(("set-ease", i), text, d.camera_easing == key || (d.camera_easing.is_empty() && key == "smooth")).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.camera_easing = key.to_string();
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(
                        // Curve preview: 24 bars showing progress over time.
                        row().h(px(48.)).items_end().gap(px(2.)).children((0..24).map(|i| {
                            let v = crate::map::geo::ease(&d.camera_easing, i as f64 / 23.0);
                            div().w(px(8.)).h(px((4.0 + v * 44.0) as f32)).rounded(px(2.)).bg(c(0x4F8FD6))
                        }).collect::<Vec<_>>()),
                    )
                    .child(muted("Speed"))
                    .child(row().flex_wrap().gap(px(4.)).children([(1.6f32, "Slow"), (1.0, "Normal"), (0.6, "Fast")].into_iter().enumerate().map(|(i, (v, text))| {
                        chip(("set-cspeed", i), text, (d.camera_speed - v).abs() < 0.01).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.camera_speed = v;
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(toggle("set-cdip", "Zoom out a little on long moves", d.camera_dip).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.camera_dip = !this.draft.camera_dip;
                        cx.notify();
                    })))
            ))
            .when(self.category == SettingsCategory::Weather, |body| body.child(
                group("Cameras, travel and channels")
                    .child(muted("Cameras: any camera that publishes a still image (DOT traffic cams, school or TV weather cams). Images refresh every minute into Media as cam-<name>.jpg."))
                    .child(self.field(37))
                    .child(self.field(38))
                    .child(self.field(39))
                    .child(muted("Graphics Studio > Sync shared collection copies the newer copy of each graphic both ways, plus the media they use."))
                    .child(muted("Travel times are estimates: distance at highway speed, slowed by the worst weather along the route and INDOT incidents within 3 miles."))
                    .children(crate::channels::numbers().into_iter().map(|n| {
                        let current = d.channel_themes.iter().find(|(c, _)| *c == n).map(|(_, t)| t.clone()).unwrap_or_default();
                        let themes = std::iter::once("Main brand".to_string())
                            .chain(crate::config::brand_themes().into_iter().map(|(name, _)| name.to_string()))
                            .enumerate()
                            .map(|(i, name)| {
                                let active = (i == 0 && current.is_empty()) || current == name;
                                let key = if i == 0 { String::new() } else { name.clone() };
                                chip(("set-chtheme", n * 100 + i), name, active).on_click(cx.listener(move |this, _, _, cx| {
                                    this.draft.channel_themes.retain(|(c, _)| *c != n);
                                    if !key.is_empty() {
                                        this.draft.channel_themes.push((n, key.clone()));
                                    }
                                    cx.notify();
                                }))
                            })
                            .collect::<Vec<_>>();
                        col().gap(px(4.)).child(muted(format!("Channel {n} theme"))).child(row().flex_wrap().gap(px(4.)).children(themes))
                    }).collect::<Vec<_>>())
            ))
            .when(self.category == SettingsCategory::Weather, |body| body.child(
                group("CAP alerts")
                    .child(muted("Common Alerting Protocol 1.2 from a local EM agency or IPAWS feed. Messages are validated; only NWS and the senders below are shown."))
                    .child(self.field(33))
                    .child(self.field(34))
                    .child(muted("Preferred language when a message has several"))
                    .child(row().gap(px(5.)).children([("en", "English"), ("es", "Español")].into_iter().enumerate().map(|(i, (key, text))| {
                        chip(("set-caplang", i), text, d.cap_language == key).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.cap_language = key.to_string();
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(button_small("set-cap-import", "Import a CAP file…").on_click(cx.listener(|_this, _, _, cx| {
                        crate::ui::pick_off_thread(cx, || rfd::FileDialog::new().add_filter("CAP message", &["xml", "cap"]).pick_file(), |this: &mut Self, path, cx| {
                            this.app.update(cx, |s, cx| {
                                s.import_cap_file(path);
                                cx.notify();
                            });
                        });
                    })))
                    .children(self.app.read(cx).ops2.cap_problems.iter().take(4).map(|(id, problems)| {
                        div().text_size(px(12.)).text_color(c(0xE0A030)).child(format!("{id}: {}", problems.join("; ")))
                    }).collect::<Vec<_>>())
            ))
            .when(self.category == SettingsCategory::Integrations, |body| body.child(
                group("Switchers and remote control")
                    .child(row().gap(px(8.)).child(div().flex_1().child(self.field(25))).child(div().flex_1().child(self.field(29))))
                    .child(row().gap(px(8.)).child(div().flex_1().child(self.field(26))).child(div().flex_1().child(self.field(27))))
                    .child(self.field(28))
                    .child(
                        row()
                            .flex_wrap()
                            .gap(px(6.))
                            .child(button_small("set-comp-test", "Test Companion (press Cut)").on_click(cx.listener(|this, _, _, cx| {
                                let (base, loc) = (this.value(25, cx), this.value(26, cx));
                                this.test_result = None;
                                cx.notify();
                                cx.spawn(async move |this, cx| {
                                    let result = net::spawn(crate::broadcast_ops::companion_press(base, loc)).await;
                                    let _ = this.update(cx, |this, cx| {
                                        this.test_result = Some(match result {
                                            Ok(Ok(m)) => (true, format!("Switcher: {m}")),
                                            Ok(Err(e)) => (false, format!("Switcher: {e}")),
                                            Err(e) => (false, format!("Switcher: {e}")),
                                        });
                                        cx.notify();
                                    });
                                })
                                .detach();
                            })))
                            .child(button_small("set-tc-test", "Test TriCaster (Auto)").on_click(cx.listener(|this, _, _, cx| {
                                let base = this.value(29, cx);
                                this.test_result = None;
                                cx.notify();
                                cx.spawn(async move |this, cx| {
                                    let result = net::spawn(crate::broadcast_ops::tricaster(base, "main_auto".into())).await;
                                    let _ = this.update(cx, |this, cx| {
                                        this.test_result = Some(match result {
                                            Ok(Ok(m)) => (true, format!("Switcher: {m}")),
                                            Ok(Err(e)) => (false, format!("Switcher: {e}")),
                                            Err(e) => (false, format!("Switcher: {e}")),
                                        });
                                        cx.notify();
                                    });
                                })
                                .detach();
                            }))),
                    )
                    .when_some(self.test_result.clone().filter(|(_, m)| m.starts_with("Switcher")), |g, (ok, msg)| {
                        g.child(div().text_size(px(12.)).text_color(if ok { c(0x3DD68C) } else { c(0xE5383B) }).child(msg))
                    })
                    .child(toggle("set-osc", "OSC (UDP): /cwx/next, /cwx/take 3, /cwx/graphic, /cwx/caption", d.osc_enabled).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.osc_enabled = !this.draft.osc_enabled;
                        cx.notify();
                    })))
                    .child(toggle("set-mos", "MOS (newsroom rundown systems: ENPS, iNews)", d.mos_enabled).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.mos_enabled = !this.draft.mos_enabled;
                        cx.notify();
                    })))
                    .child(row().gap(px(8.)).child(div().flex_1().child(self.field(30))).child(div().flex_1().child(self.field(31))).child(div().flex_1().child(self.field(32))))
                    .child(muted("OSC and MOS start with the app: restart after changing them."))
                    .child(muted("Profiles: save the switcher and OBS setup above under a name, then switch between them"))
                    .child(
                        row()
                            .gap(px(6.))
                            .child(div().flex_1().child(self.field(36)))
                            .child(button_small("set-prof-save", "Save profile").on_click(cx.listener(|this, _, window, cx| {
                                let name = this.value(36, cx);
                                if name.is_empty() {
                                    return;
                                }
                                let current = this.collect(cx);
                                let profile = crate::config::SwitcherProfile::capture(&name, &current);
                                this.draft = current;
                                this.draft.switcher_profiles.retain(|p| p.name != name);
                                this.draft.switcher_profiles.push(profile);
                                let field = this.fields[36].clone();
                                field.update(cx, |s, cx| s.set_value(String::new(), window, cx));
                                cx.notify();
                            }))),
                    )
                    .child(row().flex_wrap().gap(px(5.)).children(d.switcher_profiles.iter().enumerate().map(|(i, p)| {
                        let profile = p.clone();
                        let active = crate::config::SwitcherProfile::capture(&p.name, d) == *p;
                        row()
                            .gap(px(2.))
                            .child(chip(("set-prof", i), p.name.clone(), active).on_click(cx.listener(move |this, _, window, cx| {
                                profile.apply(&mut this.draft);
                                let values = field_values(&this.draft);
                                let fields = this.fields.clone();
                                for (idx, input) in fields.iter().enumerate() {
                                    if idx != 40 {
                                        input.update(cx, |s, cx| s.set_value(values[idx].clone(), window, cx));
                                    }
                                }
                                cx.notify();
                            })))
                            .child(button_small(("set-prof-del", i), "✕").on_click(cx.listener(move |this, _, _, cx| {
                                if i < this.draft.switcher_profiles.len() {
                                    this.draft.switcher_profiles.remove(i);
                                }
                                cx.notify();
                            })))
                    }).collect::<Vec<_>>()))
            ))
            .when(self.category == SettingsCategory::Output, |body| body.child(
                group("Captions")
                    .child(toggle("set-cc", "Show live captions on the Program output", d.captions_enabled).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.captions_enabled = !this.draft.captions_enabled;
                        cx.notify();
                    })))
                    .child(muted("Caption text comes from the Show workspace, /api/caption?text=… or OSC /cwx/caption. Common weather words are corrected (e.g. \"tornado warming\")."))
                    .child(row().gap(px(5.)).children([("bottom", "Bottom"), ("top", "Top")].into_iter().enumerate().map(|(i, (key, text))| {
                        chip(("set-ccpos", i), text, d.caption_position == key).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.caption_position = key.to_string();
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(row().gap(px(5.)).children([0.8f32, 1.0, 1.25, 1.5].into_iter().enumerate().map(|(i, v)| {
                        chip(("set-ccsize", i), format!("{:.0}%", v * 100.0), (d.caption_size - v).abs() < 0.01).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.caption_size = v;
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(muted("OBS audio input watched for silence while streaming"))
                    .child(self.field(35))
            ))
            .when(self.category == SettingsCategory::System, |body| body.child(
                group("Record keeping and data sources")
                    .child(muted("Keep logs, the warning archive, incidents and the audit log for"))
                    .child(row().flex_wrap().gap(px(5.)).children([30u32, 90, 365, 0].into_iter().enumerate().map(|(i, v)| {
                        chip(("set-ret", i), if v == 0 { "Forever".to_string() } else if v == 365 { "1 year".to_string() } else { format!("{v} days") }, d.retention_days == v).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.retention_days = v;
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(muted("Source health this session (requests, failures, average time)"))
                    .when(stats.is_empty(), |g| g.child(muted("No requests yet")))
                    .children(stats.into_iter().take(14).map(|(host, st)| {
                        row()
                            .gap(px(8.))
                            .child(div().flex_1().min_w_0().text_size(px(12.)).text_color(c(TEXT)).child(host))
                            .child(div().text_size(px(12.)).text_color(if st.failures > 0 { c(0xE0A030) } else { c(MUTED) }).child(format!(
                                "{} ok · {} failed · {:.0} ms",
                                st.requests.saturating_sub(st.failures),
                                st.failures,
                                st.avg_ms()
                            )))
                    }).collect::<Vec<_>>())
            ))
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let settings = self.collect(cx);
        self.draft = settings.clone();
        self.app.update(cx, |s, cx| {
            s.apply_settings(settings);
            s.modal = Modal::None;
            s.refresh_data(cx);
            cx.notify();
        });
    }

    /// Save the sponsor fields, then create a sponsor graphic and open it in Graphics Studio.
    fn open_sponsor_template(&mut self, key: &str, cx: &mut Context<Self>) {
        let settings = self.collect(cx);
        self.draft = settings.clone();
        let key = key.to_string();
        self.app.update(cx, |s, cx| {
            s.apply_settings(settings);
            s.modal = Modal::None;
            if let Some(doc) = crate::gfx::templates::build(&key) {
                if let Some(saved) = s.save_graphic(doc) {
                    s.toast(crate::app::ToastKind::Success, format!("Created \"{}\" - put it on the map from Graphics", saved.name));
                    s.open_graphic(saved.id);
                }
            }
            s.workspace = crate::app::Workspace::Graphics;
            cx.notify();
        });
    }

    fn test_connection(&mut self, cx: &mut Context<Self>) {
        let draft = self.collect(cx);
        let base = crate::backend::effective_base(&draft);
        self.testing = true;
        self.test_result = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = net::spawn(async move {
                if base.is_empty() {
                    anyhow::bail!("Direct mode: there is no relay to check");
                }
                let v = net::get_json(&format!("{base}/api/openwx/check")).await?;
                let checks = v["checks"].as_array().cloned().unwrap_or_default();
                let failed: Vec<String> = checks.iter().filter(|c| c["ok"] != true).filter_map(|c| c["name"].as_str().map(str::to_owned)).collect();
                if failed.is_empty() {
                    Ok(format!("All {} weather sources are answering", checks.len()))
                } else {
                    anyhow::bail!("Not answering: {}", failed.join(", "))
                }
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                this.testing = false;
                this.test_result = Some(match result {
                    Ok(Ok(msg)) => (true, msg),
                    Ok(Err(e)) => (false, e.to_string()),
                    Err(e) => (false, e.to_string()),
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn choose_sponsor_logo(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        crate::ui::pick_off_thread(cx, || rfd::FileDialog::new().add_filter("Images", crate::library::IMPORT_EXTS).pick_file(), move |this: &mut Self, path, cx| {
            this.sponsor_logo_picked(index, path, cx);
        });
    }

    fn sponsor_logo_picked(&mut self, index: Option<usize>, path: std::path::PathBuf, cx: &mut Context<Self>) {
        match crate::library::import_media(&[path]) {
            Ok(names) => {
                let Some(name) = names.into_iter().next() else { return };
                if let Some(index) = index {
                    // The roster may have changed while the dialog was open.
                    let Some(row) = self.sponsor_rows.get_mut(index) else { return };
                    row.logo_media = Some(name);
                } else {
                    self.draft.sponsor_logo_media = Some(name);
                }
                self.app.update(cx, |state, cx| { state.media = crate::library::list_media(); cx.notify(); });
                cx.notify();
            }
            Err(error) => self.app.update(cx, |state, cx| {
                state.toast(ToastKind::Error, format!("Could not add sponsor logo: {error}"));
                cx.notify();
            }),
        }
    }

    /// Labels of the Settings pages, in tab order (for Find anything).
    pub fn page_labels() -> Vec<&'static str> {
        SettingsCategory::ALL.iter().map(|c| c.label()).collect()
    }

    /// Open on the page with this label (Find anything).
    pub fn show_page(&mut self, label: &str, cx: &mut Context<Self>) {
        if let Some(cat) = SettingsCategory::ALL.into_iter().find(|c| c.label() == label) {
            self.category = cat;
            cx.notify();
        }
    }

    /// Open on the Branding & sponsors page (header button).
    pub fn show_branding(&mut self, cx: &mut Context<Self>) {
        self.category = SettingsCategory::Branding;
        cx.notify();
    }

    /// Saved brandings (Dubois County Weather, OpenWX…): switch in one click, or save the
    /// branding being edited as a new one.
    fn brand_profiles_group(&self, cx: &mut Context<Self>) -> Div {
        let d = &self.draft;
        let active = d.current_brand_profile().name;
        let chips = d.brand_profiles_or_defaults().into_iter().enumerate().map(|(i, p)| {
            let name = p.name.clone();
            chip(("brand-profile", i), p.name.clone(), p.name == active).on_click(cx.listener(move |this, _, window, cx| {
                this.draft.switch_brand(&name);
                let values = field_values(&this.draft);
                let fields = this.fields.clone();
                for (i, field) in fields.iter().enumerate() {
                    field.update(cx, |s, cx| s.set_value(values[i].clone(), window, cx));
                }
                cx.notify();
            }))
        }).collect::<Vec<_>>();
        group("Branding in use")
            .child(muted("Switch everything with the station's name on it (graphics, map title bar, crawl) between saved brandings. Changes take effect when you save."))
            .child(row().gap(px(6.)).flex_wrap().children(chips))
            .child(button_small("brand-save-as", "Save what's below as a new branding (named after the station name)").on_click(cx.listener(|this, _, _, cx| {
                let mut profile = this.draft.current_brand_profile();
                profile.name = this.draft.station_name.trim().to_string();
                if profile.name.is_empty() {
                    return;
                }
                let mut profiles = this.draft.brand_profiles_or_defaults();
                profiles.retain(|p| p.name != profile.name);
                this.draft.active_brand = profile.name.clone();
                profiles.push(profile);
                this.draft.brand_profiles = profiles;
                cx.notify();
            })))
    }

    fn sponsor_settings(&self, cx: &mut Context<Self>) -> Div {
        let d = &self.draft;
        let rows = self.sponsor_rows.iter().enumerate().map(|(index, sponsor)| {
            col().gap(px(5.))
                .child(row().gap(px(8.)).child(div().flex_1().child(Input::new(&sponsor.name))).child(div().flex_1().child(Input::new(&sponsor.tagline))))
                .child(row().gap(px(6.)).flex_wrap()
                    .child(muted(sponsor.logo_media.clone().unwrap_or_else(|| "No logo".into())))
                    .child(button_small(("pick-roster-logo", index), "Choose logo…").on_click(cx.listener(move |this, _, _, cx| this.choose_sponsor_logo(Some(index), cx))))
                    .child(button_small(("clear-roster-logo", index), "Remove logo").on_click(cx.listener(move |this, _, _, cx| {
                        this.sponsor_rows[index].logo_media = None;
                        cx.notify();
                    })))
                    .child(button_small(("remove-sponsor", index), "Remove sponsor").on_click(cx.listener(move |this, _, _, cx| {
                        this.sponsor_rows.remove(index);
                        cx.notify();
                    }))))
        }).collect::<Vec<_>>();
        group("Sponsor lineup")
                                    .child(muted("Add a primary sponsor and any additional sponsors. Map badges and sponsor graphics use the active sponsor."))
                                    .child(row().gap(px(8.)).child(div().flex_1().child(self.field(17))).child(div().flex_1().child(self.field(18))))
                                    .child(muted(match self.draft.sponsor_logo_media.as_deref().filter(|m| !m.is_empty()) {
                                        Some(m) => format!("Sponsor logo: {m}"),
                                        None => "Sponsor logo: none yet".to_string(),
                                    }))
                                    .child(
                                        row()
                                            .flex_wrap()
                                            .gap(px(6.))
                                            .child(button_small("sponsor-logo", "Choose logo…").on_click(cx.listener(|this, _, _, cx| this.choose_sponsor_logo(None, cx))))
                                            .child(button_small("clear-sponsor-logo", "Remove logo").on_click(cx.listener(|this, _, _, cx| {
                                                this.draft.sponsor_logo_media = None;
                                                cx.notify();
                                            })))
                                            .child(button_small("sponsor-bug", "Make a Sponsor Bug").on_click(cx.listener(|this, _, _, cx| {
                                                this.open_sponsor_template("sponsor_bug", cx);
                                            })))
                                            .child(button_small("sponsor-lt", "Make a Sponsor Lower Third").on_click(cx.listener(|this, _, _, cx| {
                                                this.open_sponsor_template("sponsor_lower_third", cx);
                                            })))
                                            .child(button_small("sponsor-card", "Make a Sponsor Card").on_click(cx.listener(|this, _, _, cx| {
                                                this.open_sponsor_template("sponsor_full", cx);
                                            }))),
                                    )
                                    .children(rows)
                                    .when(crate::dcweather::has_token(&d.api_base), |g| g.child(row().gap(px(6.)).child(button_small("dcw-import-sponsors", "Add sponsors from DCWeather").on_click(cx.listener(|this, _, _, cx| this.dcw_import_sponsors(cx)))).child(muted("Active sponsors and their logos from the site; the portal stays where they're edited."))))
                                    .child(button_small("add-sponsor", "+ Add sponsor").on_click(cx.listener(|this, _, window, cx| {
                                        this.sponsor_rows.push(SponsorRow::new(crate::config::Sponsor::default(), window, cx));
                                        cx.notify();
                                    })))
                                    .child(muted("Rotate sponsors every (all sponsor graphics follow the rotation; time on air is logged to Sponsor impressions.csv)"))
                                    .child(row().flex_wrap().gap(px(5.)).children([0u32, 5, 10, 15, 30].into_iter().enumerate().map(|(i, m)| {
                                        chip(("set-rot", i), if m == 0 { "Off".to_string() } else { format!("{m} min") }, d.sponsor_rotate_minutes == m).on_click(cx.listener(move |this, _, _, cx| {
                                            this.draft.sponsor_rotate_minutes = m;
                                            cx.notify();
                                        }))
                                    }).collect::<Vec<_>>()))
    }

    fn field(&self, i: usize) -> Div {
        col().gap(px(4.)).child(muted(FIELD_LABELS[i])).child(Input::new(&self.fields[i]))
    }
}

fn group(title: &str) -> Div {
    col()
        .gap(px(10.))
        .p(px(14.))
        .rounded(px(8.))
        .bg(c(PANEL_2))
        .border_1()
        .border_color(c(BORDER))
        .child(heading(title.to_string()))
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.app.read(cx).modal == Modal::Sponsors {
            return col().w_full().gap(px(10.))
                .child(self.sponsor_settings(cx))
                .child(button_primary("save-sponsors", "Save sponsors").on_click(cx.listener(|this, _, _, cx| this.save(cx))))
                .into_any_element();
        }
        if self.category == SettingsCategory::Coverage {
            self.sync_coverage(window, cx);
        }
        let d = self.draft.clone();

        let basemaps = Basemap::ALL
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let b = *b;
                chip(("set-base", i), b.label(), d.basemap == b).on_click(cx.listener(move |this, _, _, cx| {
                    this.draft.basemap = b;
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();
        let palettes = RADAR_PALETTES
            .iter()
            .enumerate()
            .map(|(i, (code, name))| {
                let code = code.to_string();
                chip(("set-pal", i), *name, d.radar_palette == code).on_click(cx.listener(move |this, _, _, cx| {
                    this.draft.radar_palette = code.clone();
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();
        let display_fonts = family_names()
            .into_iter()
            .enumerate()
            .map(|(i, f)| {
                chip(("set-font-d", i), f, d.brand.font_display == f).on_click(cx.listener(move |this, _, _, cx| {
                    this.draft.brand.font_display = f.to_string();
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();
        let body_fonts = family_names()
            .into_iter()
            .enumerate()
            .map(|(i, f)| {
                chip(("set-font-b", i), f, d.brand.font_body == f).on_click(cx.listener(move |this, _, _, cx| {
                    this.draft.brand.font_body = f.to_string();
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();
        let polls = [60u64, 120, 300, 600]
            .into_iter()
            .enumerate()
            .map(|(i, secs)| {
                chip(("set-poll", i), format!("{} min", secs / 60), d.poll_seconds == secs).on_click(cx.listener(move |this, _, _, cx| {
                    this.draft.poll_seconds = secs;
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();
        let swatches = [&d.brand.primary, &d.brand.secondary, &d.brand.alert, &d.brand.text]
            .into_iter()
            .map(|color| div().w(px(28.)).h(px(28.)).rounded(px(4.)).border_1().border_color(c(BORDER)).bg(hex_color(color)))
            .collect::<Vec<_>>();

        let themes = crate::config::brand_themes()
            .into_iter()
            .enumerate()
            .map(|(i, (name, brand))| {
                let active = d.brand.primary.eq_ignore_ascii_case(&brand.primary) && d.brand.panel.eq_ignore_ascii_case(&brand.panel);
                let swatch_color = brand.primary.clone();
                chip(("theme", i), name, active)
                    .child(div().ml(px(6.)).w(px(10.)).h(px(10.)).rounded(px(5.)).bg(hex_color(&swatch_color)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let fonts = (this.draft.brand.font_display.clone(), this.draft.brand.font_body.clone(), this.draft.brand.logo_media.clone());
                        this.draft.brand = brand.clone();
                        this.draft.brand.font_display = fonts.0;
                        this.draft.brand.font_body = fonts.1;
                        this.draft.brand.logo_media = fonts.2;
                        let values = field_values(&this.draft);
                        let fields = this.fields.clone();
                        for i in 9..13 {
                            fields[i].update(cx, |s, cx| s.set_value(values[i].clone(), window, cx));
                        }
                        cx.notify();
                    }))
            })
            .collect::<Vec<_>>();
        let test = self.test_result.clone();
        let obs_result = self.test_result.clone().filter(|(_, m)| m.starts_with("OBS"));
        let opacity = d.radar_opacity;
        let category = self.category;

        let body = col().gap(px(14.))
            .when(category == SettingsCategory::General, |body| body.child(
                group("Station")
                    .child(self.field(0))
                    .child(self.field(1))
                    .child(self.field(2))
                    .child(self.field(3))
                    .child(row().gap(px(8.)).child(div().flex_1().child(self.field(4))).child(div().flex_1().child(self.field(5))))
                    .child(button_small("use-center", "Use current map center as home").on_click(cx.listener(|this, _, window, cx| {
                        let view = this.app.read(cx).live.view;
                        this.draft.home = view;
                        let fields = this.fields.clone();
                        fields[4].update(cx, |s, cx| s.set_value(format!("{:.4}", view.lat), window, cx));
                        fields[5].update(cx, |s, cx| s.set_value(format!("{:.4}", view.lon), window, cx));
                        cx.notify();
                    })))
            ))
            .when(category == SettingsCategory::Branding, |body| body.child(self.brand_profiles_group(cx)))
            .when(category == SettingsCategory::Branding, |body| body.child(
                group("Station names on air")
                    .child(muted("Used by {{station}}, {{station_short}} and {{tagline}} in graphics and on the map."))
                    .child(self.field(0))
                    .child(self.field(1))
                    .child(self.field(2))
            ))
            .when(category == SettingsCategory::Branding, |body| body.child(self.sponsor_settings(cx)))
            .when(category == SettingsCategory::Branding, |body| body.child(
                group("Brand & on-air theme")
                    .child(muted("Theme: restyles every banner, legend, panel, call-out and template on air."))
                    .child(row().flex_wrap().gap(px(5.)).children(themes))
                    .child(row().gap(px(6.)).children(swatches))
                    .child(row().gap(px(8.)).child(div().flex_1().child(self.field(9))).child(div().flex_1().child(self.field(10))))
                    .child(row().gap(px(8.)).child(div().flex_1().child(self.field(11))).child(div().flex_1().child(self.field(12))))
                    .child(muted("Display font (titles, temperatures)"))
                    .child(row().flex_wrap().gap(px(4.)).children(display_fonts))
                    .child(muted("Body font"))
                    .child(row().flex_wrap().gap(px(4.)).children(body_fonts))
                    .child(muted("Set a logo image from the Media workspace (\"Use as logo\")."))
            ))
            .when(category == SettingsCategory::Weather, |body| body.child(
                group("Weather data backend")
                    .child(muted("OpenWX reads NOAA/NWS data through OpenWX Relay: built into this app, or hosted on your own server for several studio PCs."))
                    .child(row().flex_wrap().gap(px(5.)).children(crate::backend::MODES.iter().map(|(id, label, _)| {
                        let id = *id;
                        chip(SharedString::from(format!("backend-{id}")), *label, d.backend_mode == id).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.backend_mode = id.to_string();
                            this.test_result = None;
                            cx.notify();
                        }))
                    })))
                    .child(muted(crate::backend::MODES.iter().find(|m| m.0 == d.backend_mode).map(|m| m.2).unwrap_or("")))
                    .when(d.backend_mode == "remote", |g| g.child(self.field(6)))
                    .child(self.field(TEXT_FIELDS - 1))
                    .when(d.backend_mode == "local", |g| g
                        .child(muted("Radar comes from LibreWXR (github.com/JoshuaKimsey/LibreWXR). Leave this blank for its public API, or enter your own LibreWXR server. Check LibreWXR's terms for your use."))
                        .child(self.field(TEXT_FIELDS - 2)))
                    .child(row().gap(px(8.)).child(button_small("run-setup", "Run setup again").on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.open_request = Some(crate::app::OpenRequest::Modal(Modal::Setup));
                            cx.notify();
                        });
                    }))).child(muted("Station, counties, main city and backend, step by step.")))
                    .child(
                        row()
                            .gap(px(8.))
                            .child(button_small("test-conn", if self.testing { "Checking…" } else { "Check the weather feeds" }).on_click(cx.listener(|this, _, _, cx| this.test_connection(cx))))
                            .when_some(test, |r, (ok, msg)| {
                                r.child(div().text_size(px(12.)).text_color(if ok { c(0x3DD68C) } else { c(0xE5383B) }).child(msg))
                            }),
                    )
                    .child(self.field(7))
                    .child(toggle("set-severe-grid", "Severe and winter ingredients (STP, SCP, lapse rates, wet-bulb zero, snow squalls; more model data)", d.severe_grid).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.severe_grid = !this.draft.severe_grid;
                        cx.notify();
                    })))
                    .child(toggle("set-air", "Air quality and wildfire smoke maps", d.air_quality).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.air_quality = !this.draft.air_quality;
                        cx.notify();
                    })))
                    .child(muted("Refresh data every"))
                    .child(row().flex_wrap().gap(px(5.)).children(polls))
            ))
            .when(category == SettingsCategory::Weather, |body| body.child(
                group("Map defaults")
                    .child(row().flex_wrap().gap(px(5.)).children(basemaps))
                    .child(row().flex_wrap().gap(px(5.)).children(palettes))
                    .child(row().justify_between().child(muted("Radar opacity")).child(muted(format!("{:.0}%", opacity * 100.0))))
                    .child(slider("set-opacity", opacity, 0.1, 1.0, cx, |this, v, _, cx| {
                        this.draft.radar_opacity = v;
                        cx.notify();
                    }))
                    .child(toggle("set-24h", "24-hour clock", d.clock_24h).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.clock_24h = !this.draft.clock_24h;
                        cx.notify();
                    })))
                    .child(toggle("set-metric", "Metric units", d.units == Units::Metric).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.units = if this.draft.units == Units::Metric { Units::Imperial } else { Units::Metric };
                        cx.notify();
                    })))
            ))
            .when(category == SettingsCategory::Output, |body| body.child(
                group("On-air defaults")
                    .child(toggle("set-logo", "Station bug", d.show_logo_bug).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.show_logo_bug = !this.draft.show_logo_bug;
                        cx.notify();
                    })))
                    .child(toggle("set-logo-gfx", "Station logo instead of the station name on graphics", d.logo_on_graphics).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.logo_on_graphics = !this.draft.logo_on_graphics;
                        cx.notify();
                    })))
                    .when(d.brand.logo_media.is_none(), |g| g.child(muted("No station logo yet: in Media, choose Use as logo on an image.")))
                    .child(toggle("set-clock", "Clock", d.show_clock_bug).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.show_clock_bug = !this.draft.show_clock_bug;
                        cx.notify();
                    })))
                    .child(toggle("set-legend", "Radar legend", d.show_legend).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.show_legend = !this.draft.show_legend;
                        cx.notify();
                    })))
                    .child(toggle("set-stamp", "Radar time", d.show_timestamp).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.show_timestamp = !this.draft.show_timestamp;
                        cx.notify();
                    })))
            ))
            .when(category == SettingsCategory::Output, |body| body.child(
                group("Recording & output")
                    .child(
                        row()
                            .gap(px(6.))
                            .child(chip("rec-mp4", "MP4 (H.264)", d.record_format == RecordFormat::Mp4).on_click(cx.listener(|this, _, _, cx| {
                                this.draft.record_format = RecordFormat::Mp4;
                                cx.notify();
                            })))
                            .child(chip("rec-gif", "Animated GIF", d.record_format == RecordFormat::Gif).on_click(cx.listener(|this, _, _, cx| {
                                this.draft.record_format = RecordFormat::Gif;
                                cx.notify();
                            }))),
                    )
                    .child(toggle("set-ontop", "Output window always on top", d.output_always_on_top).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.output_always_on_top = !this.draft.output_always_on_top;
                        cx.notify();
                    })))
            ))
            .when(category == SettingsCategory::Weather, |body| body.child(
                group("Severe weather")
                    .child(muted("Coverage area around home: warnings inside it go to the Severe Center queue and sound an alert."))
                    .child(row().flex_wrap().gap(px(5.)).children([25.0f64, 50.0, 75.0, 100.0, 150.0].into_iter().enumerate().map(|(i, miles)| {
                        chip(("set-cov", i), format!("{miles:.0} mi"), (d.coverage_miles - miles).abs() < 0.5).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.coverage_miles = miles;
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(toggle("set-sounds", "Sound for new warnings", d.alert_sounds).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.alert_sounds = !this.draft.alert_sounds;
                        cx.notify();
                    })))
                    .child(toggle("set-automode", "Turn on Severe Weather Mode automatically", d.auto_severe_mode).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.auto_severe_mode = !this.draft.auto_severe_mode;
                        cx.notify();
                    })))
                    .when(d.auto_severe_mode, |g| {
                        g.child(toggle("set-automode-tor", "Only for tornado warnings and emergencies", d.auto_severe_tornado_only).on_click(cx.listener(|this, _, _, cx| {
                            this.draft.auto_severe_tornado_only = !this.draft.auto_severe_tornado_only;
                            cx.notify();
                        })))
                    })
                    .child(toggle("set-severe-rings", "Severe Weather Mode shows distance rings (10/25/50 mi)", d.severe_range_rings).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.severe_range_rings = !this.draft.severe_range_rings;
                        cx.notify();
                    })))
                    .child(toggle("set-modern-snow", "Repaint radar snow blue-to-violet (off keeps the TWC palette colours)", d.snow_radar_repaint).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.snow_radar_repaint = !this.draft.snow_radar_repaint;
                        cx.notify();
                    })))
                    .child(toggle("set-calm-labels", "Calm storm labels on air (\"Strong storm\", not \"Very heavy storm\")", d.calm_storm_labels).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.calm_storm_labels = !this.draft.calm_storm_labels;
                        cx.notify();
                    })))
                    .child(toggle("set-quiet", "Quiet hours (mute warning sounds)", d.quiet_hours).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.quiet_hours = !this.draft.quiet_hours;
                        cx.notify();
                    })))
                    .when(d.quiet_hours, |g| {
                        let hour = |h: u32| match h % 24 {
                            0 => "12 AM".to_string(),
                            12 => "12 PM".to_string(),
                            h if h < 12 => format!("{h} AM"),
                            h => format!("{} PM", h - 12),
                        };
                        g.child(muted("From"))
                            .child(row().flex_wrap().gap(px(5.)).children([20u32, 21, 22, 23, 0].into_iter().enumerate().map(|(i, h)| {
                                chip(("set-qs", i), hour(h), d.quiet_start == h).on_click(cx.listener(move |this, _, _, cx| {
                                    this.draft.quiet_start = h;
                                    cx.notify();
                                }))
                            }).collect::<Vec<_>>()))
                            .child(muted("Until"))
                            .child(row().flex_wrap().gap(px(5.)).children([5u32, 6, 7, 8].into_iter().enumerate().map(|(i, h)| {
                                chip(("set-qe", i), hour(h), d.quiet_end == h).on_click(cx.listener(move |this, _, _, cx| {
                                    this.draft.quiet_end = h;
                                    cx.notify();
                                }))
                            }).collect::<Vec<_>>()))
                            .child(toggle("set-quiet-urgent", "Tornado warnings and emergencies still sound", d.quiet_allow_urgent).on_click(cx.listener(|this, _, _, cx| {
                                this.draft.quiet_allow_urgent = !this.draft.quiet_allow_urgent;
                                cx.notify();
                            })))
                    })
                    .child(muted("Watched spots (add one from the map: click a spot, then Watch this spot)"))
                    .when(d.alert_zones.is_empty(), |g| g.child(muted("None yet")))
                    .children(d.alert_zones.iter().enumerate().map(|(i, z)| {
                        row()
                            .gap(px(6.))
                            .child(div().flex_1().min_w_0().text_size(px(13.)).text_color(c(TEXT)).child(z.name.clone()))
                            .children([5.0f64, 10.0, 25.0].into_iter().enumerate().map(|(j, miles)| {
                                chip(("set-zone-mi", i * 10 + j), format!("{miles:.0} mi"), (z.miles - miles).abs() < 0.5).on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(zone) = this.draft.alert_zones.get_mut(i) {
                                        zone.miles = miles;
                                    }
                                    cx.notify();
                                }))
                            }).collect::<Vec<_>>())
                            .child(button_small(("set-zone-del", i), "Remove").on_click(cx.listener(move |this, _, _, cx| {
                                if i < this.draft.alert_zones.len() {
                                    this.draft.alert_zones.remove(i);
                                }
                                cx.notify();
                            })))
                    }).collect::<Vec<_>>())
            ))
            .when(category == SettingsCategory::Integrations, |body| body.child(
                group("Broadcast control (OBS)")
                    .child(toggle("set-obs", "Switch OBS scenes with Severe Mode", d.obs_enabled).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.obs_enabled = !this.draft.obs_enabled;
                        cx.notify();
                    })))
                    .child(muted("OBS 28+: Tools > WebSocket Server Settings > Enable."))
                    .child(self.field(13))
                    .child(self.field(14))
                    .child(row().gap(px(8.)).child(div().flex_1().child(self.field(15))).child(div().flex_1().child(self.field(16))))
                    .child(button_small("obs-test", "Test OBS connection").on_click(cx.listener(|this, _, _, cx| {
                        let (url, password) = (this.value(13, cx), this.value(14, cx));
                        this.test_result = None;
                        cx.notify();
                        cx.spawn(async move |this, cx| {
                            let result = net::spawn(crate::obs::scene_list(url, password)).await;
                            let _ = this.update(cx, |this, cx| {
                                this.test_result = Some(match result {
                                    Ok(Ok(scenes)) => (true, format!("OBS connected. Scenes: {}", scenes.join(", "))),
                                    Ok(Err(e)) => (false, e.to_string()),
                                    Err(e) => (false, e.to_string()),
                                });
                                cx.notify();
                            });
                        })
                        .detach();
                    })))
                    .when_some(obs_result, |d, (ok, msg)| {
                        d.child(div().text_size(px(12.)).text_color(if ok { c(0x3DD68C) } else { c(0xE5383B) }).child(msg))
                    })
            ))
            .when(category == SettingsCategory::System, |body| body.child(
                group("Updates")
                    .child(self.field(8))
                    .child(toggle("set-autoupdate", "Check for updates at startup", d.auto_check_updates).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.auto_check_updates = !this.draft.auto_check_updates;
                        cx.notify();
                    })))
                    .child(muted("Release channel"))
                    .child(row().gap(px(5.)).children([("stable", "Stable"), ("beta", "Beta (early features)")].into_iter().enumerate().map(|(i, (key, text))| {
                        chip(("set-chan", i), text, d.update_channel == key || (key == "stable" && d.update_channel.is_empty())).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.update_channel = key.to_string();
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
            ))
            .when(category == SettingsCategory::General, |body| body.child(
                group("Accessibility")
                    .child(muted("On-air text size (every graphic)"))
                    .child(row().flex_wrap().gap(px(5.)).children([0.9f32, 1.0, 1.15, 1.3].into_iter().enumerate().map(|(i, v)| {
                        chip(("set-tscale", i), format!("{:.0}%", v * 100.0), (d.onair_text_scale - v).abs() < 0.01).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.onair_text_scale = v;
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(
                        row()
                            .flex_wrap()
                            .gap(px(6.))
                            .child(button_small("set-hc", "Use the High Contrast theme").on_click(cx.listener(|this, _, _, cx| {
                                if let Some((_, brand)) = crate::config::brand_themes().into_iter().find(|(n, _)| *n == "High Contrast") {
                                    let logo = this.draft.brand.logo_media.clone();
                                    this.draft.brand = brand;
                                    this.draft.brand.logo_media = logo;
                                }
                                cx.notify();
                            })))
                            .child(button_small("set-cb", "Colorblind-safe radar colors").on_click(cx.listener(|this, _, _, cx| {
                                if let Some((code, _)) = crate::data::RADAR_PALETTES.iter().find(|(_, n)| *n == "Universal Blue") {
                                    this.draft.radar_palette = code.to_string();
                                }
                                cx.notify();
                            }))),
                    )
                    .child(muted("Graphics language for the Spanish templates and wording"))
                    .child(row().gap(px(5.)).children([("en", "English"), ("es", "Español")].into_iter().enumerate().map(|(i, (key, text))| {
                        chip(("set-lang", i), text, d.graphics_language == key).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.graphics_language = key.to_string();
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(muted("Read aloud: the Severe Center and map details card have Read aloud buttons (Windows speech)."))
            ))
            .when(category == SettingsCategory::System, |body| body.child(
                group("Performance")
                    .child(toggle("set-lowres", "Low-resource mode (half frame rate, no wind particles)", d.low_resource).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.low_resource = !this.draft.low_resource;
                        cx.notify();
                    })))
                    .child(muted("Warn and free map memory above"))
                    .child(row().flex_wrap().gap(px(5.)).children([2000u32, 3000, 4000, 6000].into_iter().enumerate().map(|(i, v)| {
                        chip(("set-mem", i), format!("{:.0} GB", v as f32 / 1000.0), d.memory_limit_mb == v).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.memory_limit_mb = v;
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(muted("Map tile cache on disk"))
                    .child(row().flex_wrap().gap(px(5.)).children([500u64, 1000, 2000, 5000].into_iter().enumerate().map(|(i, v)| {
                        chip(("set-cache", i), if v < 1000 { format!("{v} MB") } else { format!("{} GB", v / 1000) }, d.tile_cache_mb == v).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.tile_cache_mb = v;
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(muted(match self.app.read(cx).ext.memory_mb {
                        Some(mb) => format!("Using {mb:.0} MB of memory now"),
                        None => "Memory use: checking…".to_string(),
                    }))
                    .child(toggle("set-adaptive", "Adaptive quality (lower radar detail when the PC or network struggles)", d.adaptive_quality).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.adaptive_quality = !this.draft.adaptive_quality;
                        cx.notify();
                    })))
                    .child(button_small("set-bench", "Run benchmark").on_click(cx.listener(|this, _, _, cx| {
                        let (settings, data) = {
                            let s = this.app.read(cx);
                            (s.settings.clone(), s.data.clone())
                        };
                        this.app.update(cx, |s, cx| {
                            s.toast(ToastKind::Info, "Running the benchmark (about 10 seconds)…");
                            cx.notify();
                        });
                        let task = cx.background_executor().spawn(async move {
                            let results = crate::bench::run(&settings, &data);
                            let path = crate::bench::report(&results);
                            (results, path)
                        });
                        cx.spawn(async move |this, cx| {
                            let (results, path) = task.await;
                            let _ = this.update(cx, |this, cx| {
                                let slow = results.iter().filter(|r| r.verdict.starts_with("Slow") || r.verdict.starts_with("Failed")).count();
                                this.app.update(cx, |s, cx| {
                                    match path {
                                        Ok(p) => {
                                            let _ = open::that(&p);
                                            s.toast(if slow == 0 { ToastKind::Success } else { ToastKind::Info }, format!("Benchmark done: {} test(s), {slow} need attention", results.len()));
                                        }
                                        Err(e) => s.toast(ToastKind::Error, format!("Benchmark report: {e}")),
                                    }
                                    cx.notify();
                                });
                            });
                        })
                        .detach();
                    })))
                    .child(button_small("set-diag", "Export diagnostics to the desktop").on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.export_diagnostics();
                            cx.notify();
                        });
                    })))
            ))
            .when(category == SettingsCategory::Integrations, |body| body.child(
                group("OBS Studio Mode and MIDI")
                    .child(toggle("set-studio", "OBS studio mode: preview the scene, then transition", d.obs_studio_mode).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.obs_studio_mode = !this.draft.obs_studio_mode;
                        cx.notify();
                    })))
                    .child(toggle("set-midi", "MIDI pads and keyboards (restart to detect new devices)", d.midi_enabled).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.midi_enabled = !this.draft.midi_enabled;
                        cx.notify();
                    })))
                    .child(muted({
                        let devices = self.app.read(cx).ext.midi_devices.clone();
                        let map = crate::switcher::MIDI_MAP.iter().map(|(n, a)| format!("{} {a}", ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"][*n as usize])).collect::<Vec<_>>().join(", ");
                        if devices.is_empty() {
                            format!("No MIDI device found. Notes: {map} (any octave; C5/D5 = backup scene on/off).")
                        } else {
                            format!("Listening to {}. Notes: {map}.", devices.join(", "))
                        }
                    }))
                    .child(toggle("set-snap", "Scheduled exports include the map (Map.png)", d.scheduled_map_snapshot).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.scheduled_map_snapshot = !this.draft.scheduled_map_snapshot;
                        cx.notify();
                    })))
            ))
            .when(category == SettingsCategory::Integrations, |body| body.child(
                group("Automation and OBS browser source")
                    .child(toggle("set-ctl", "Control server (OBS browser source, Stream Deck / Companion, webhooks)", d.control_server).on_click(cx.listener(|this, _, _, cx| {
                        this.draft.control_server = !this.draft.control_server;
                        cx.notify();
                    })))
                    .child(self.field(19))
                    .child(muted(format!("OBS browser source: http://127.0.0.1:{}/program  •  restart the app after changing the port", d.control_port)))
                    .child(button_small("set-ctl-open", "Open the control page").on_click(cx.listener(|this, _, _, cx| {
                        let port = this.draft.control_port;
                        let _ = open::that(format!("http://127.0.0.1:{port}/"));
                    })))
                    .child(muted("Export these graphics as PNG every (Exports\\Scheduled, overwritten each time)"))
                    .child(row().flex_wrap().gap(px(5.)).children([0u32, 5, 15, 30, 60].into_iter().enumerate().map(|(i, m)| {
                        chip(("set-sched", i), if m == 0 { "Off".to_string() } else { format!("{m} min") }, d.scheduled_export_minutes == m).on_click(cx.listener(move |this, _, _, cx| {
                            this.draft.scheduled_export_minutes = m;
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(row().flex_wrap().gap(px(5.)).children(self.app.read(cx).graphics.iter().take(40).enumerate().map(|(i, g)| {
                        let id = g.id.clone();
                        let on = d.scheduled_exports.contains(&g.id);
                        chip(("set-sched-g", i), g.name.clone(), on).on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(pos) = this.draft.scheduled_exports.iter().position(|x| *x == id) {
                                this.draft.scheduled_exports.remove(pos);
                            } else {
                                this.draft.scheduled_exports.push(id.clone());
                            }
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(button_small("set-sched-now", "Export them now").on_click(cx.listener(|this, _, _, cx| {
                        let draft = this.draft.scheduled_exports.clone();
                        this.app.update(cx, |s, cx| {
                            s.settings.scheduled_exports = draft;
                            s.export_scheduled_now();
                            s.toast(ToastKind::Success, "Scheduled graphics exported");
                            cx.notify();
                        });
                    })))
            ))
            .when(category == SettingsCategory::Coverage, |body| body.child(self.coverage_group(cx)))
            .child(self.operations_groups(&d, cx));

        col()
            .flex_1()
            .min_h_0()
            .gap(px(12.))
            .child(row().flex_wrap().gap(px(6.)).children(
                SettingsCategory::ALL.into_iter().enumerate().map(|(i, section)| {
                    chip(("settings-category", i), section.label(), category == section)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.category = section;
                            cx.notify();
                        }))
                }),
            ))
            .child(
                div()
                    .id(("settings-section", category as usize))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(body),
            )
            .child(
                row()
                    .gap(px(8.))
                    .child(div().flex_1())
                    .child(button("reset", "Reset to defaults").on_click(cx.listener(|this, _, window, cx| {
                        let defaults = Settings::default();
                        this.draft = defaults.clone();
                        for (input, value) in this.fields.clone().iter().zip(field_values(&defaults)) {
                            input.update(cx, |state, cx| state.set_value(value, window, cx));
                        }
                        this.app.update(cx, |s, cx| {
                            s.toast(ToastKind::Info, "Defaults loaded - press Save to apply");
                            cx.notify();
                        });
                        cx.notify();
                    })))
                    .child(button("cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.modal = Modal::None;
                            cx.notify();
                        });
                    })))
                    .child(button_primary("save", "Save settings").on_click(cx.listener(|this, _, _, cx| this.save(cx)))),
            )
            .text_color(c(TEXT))
            .into_any_element()
    }
}
