//! Publishing (modal): delivery targets (folders, CDN / storage uploads, Discord, Slack), the
//! publishing queue with delivery status and retries, scheduled and conditional publishing, the
//! audit history, and the public image / data / widget endpoints with their access key, cache
//! lifetime and usage counts.

use gpui_kit::{
    component::input::{Input, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::{AppState, ToastKind},
    config::Settings,
    publish::{self, Schedule, Target},
    ui::{badge, button_small, c, chip, col, heading, muted, row, toggle, BORDER, TEXT},
};

pub struct PublishingView {
    app: Entity<AppState>,
    t_name: Entity<InputState>,
    t_address: Entity<InputState>,
    t_header: Entity<InputState>,
    t_purge: Entity<InputState>,
    t_kind: &'static str,
    t_format: &'static str,
    t_encoding: &'static str,
    /// DCWeather target: chosen sponsor id, and the site's sponsor list once loaded.
    t_sponsor: String,
    dcw_sponsors: Vec<crate::dcweather::RemoteSponsor>,
    graphic: Entity<InputState>,
    chosen: Vec<String>,
    every: u32,
    condition: &'static str,
    key: Entity<InputState>,
    allowed: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl PublishingView {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let s = app.read(cx).settings.clone();
        let input = |window: &mut Window, cx: &mut Context<Self>, ph: &str, value: String| {
            let ph = ph.to_string();
            let st = cx.new(|cx| InputState::new(window, cx).placeholder(ph));
            if !value.is_empty() {
                st.update(cx, |s, cx| s.set_value(value, window, cx));
            }
            st
        };
        let t_name = input(window, cx, "Target name, e.g. Website, Facebook folder, Newsroom Slack", String::new());
        let t_address = input(window, cx, "Folder, upload URL with {file} (CDN / storage), or webhook URL", String::new());
        let t_header = input(window, cx, "Optional upload header, e.g. AccessKey: 1234 (HTTP only)", String::new());
        let t_purge = input(window, cx, "Optional cache purge URL with {file} (called after each upload)", String::new());
        let graphic = input(window, cx, "Graphic name (as in the library), e.g. 7-Day Forecast", String::new());
        let key = input(window, cx, "Access key for public endpoints (empty = open to everyone)", s.public_key.clone());
        let allowed = input(window, cx, "Library graphics allowed on /public/graphic.png, separated by ;", s.public_graphics.join("; "));
        Self {
            app: app.clone(),
            t_name,
            t_address,
            t_header,
            t_purge,
            t_kind: "folder",
            t_format: "landscape",
            t_encoding: "png",
            t_sponsor: String::new(),
            dcw_sponsors: Vec::new(),
            graphic,
            chosen: Vec::new(),
            every: 60,
            condition: "always",
            key,
            allowed,
            _subscriptions: vec![cx.observe(&app, |_, _, cx| cx.notify())],
        }
    }

    fn update(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Settings)) {
        self.app.update(cx, |s, cx| {
            let mut next = s.settings.clone();
            f(&mut next);
            s.apply_settings(next);
            cx.notify();
        });
    }

    fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: String) {
        self.app.update(cx, |s, cx| {
            s.toast(kind, msg);
            cx.notify();
        });
    }

    fn add_target(&mut self, cx: &mut Context<Self>) {
        let name = self.t_name.read(cx).value().trim().to_string();
        let mut address = self.t_address.read(cx).value().trim().to_string();
        if self.t_kind == "dcweather" {
            if !crate::dcweather::has_token(&self.app.read(cx).settings.api_base) {
                self.toast(cx, ToastKind::Error, "Sign in to DCWeather first (Settings > Integrations)".into());
                return;
            }
            // For DCWeather the address box is the caption.
            if address.is_empty() {
                address = "{name}".into();
            }
        }
        if name.is_empty() || address.is_empty() {
            self.toast(cx, ToastKind::Error, "A target needs a name and an address".into());
            return;
        }
        let t = Target {
            name: name.clone(),
            kind: self.t_kind.into(),
            address,
            header: self.t_header.read(cx).value().trim().to_string(),
            purge_url: self.t_purge.read(cx).value().trim().to_string(),
            format: self.t_format.into(),
            encoding: self.t_encoding.into(),
            sponsor_id: if self.t_kind == "dcweather" { self.t_sponsor.clone() } else { String::new() },
        };
        self.update(cx, move |s| {
            s.publish_targets.retain(|x| x.name != t.name);
            s.publish_targets.push(t);
        });
    }

    /// Fetch the DCWeather sponsor list for the target's sponsor chips.
    fn load_dcw_sponsors(&mut self, cx: &mut Context<Self>) {
        let base = self.app.read(cx).settings.api_base.clone();
        if !crate::dcweather::has_token(&base) {
            return;
        }
        cx.spawn(async move |this, cx| {
            let result = crate::net::spawn(crate::dcweather::sponsors(base)).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(list)) => this.dcw_sponsors = list,
                    Ok(Err(e)) => this.toast(cx, ToastKind::Error, format!("DCWeather sponsors: {e}")),
                    Err(e) => this.toast(cx, ToastKind::Error, format!("DCWeather sponsors: {e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn graphic_id(&self, cx: &mut Context<Self>) -> Option<(String, String)> {
        let name = self.graphic.read(cx).value().trim().to_string();
        self.app.read(cx).graphics.iter().find(|g| g.name.eq_ignore_ascii_case(&name)).map(|g| (g.id.clone(), g.name.clone()))
    }

    fn publish_now(&mut self, cx: &mut Context<Self>) {
        let targets = self.targets_for(cx);
        let name = self.graphic.read(cx).value().trim().to_string();
        let result = {
            let app = self.app.read(cx);
            match app.graphics.iter().find(|g| g.name.eq_ignore_ascii_case(&name)) {
                Some(doc) => publish::enqueue(doc, &app.settings, &targets),
                None => Err(anyhow::anyhow!("No graphic named \"{name}\" in the library")),
            }
        };
        match result {
            Ok(n) => self.toast(cx, ToastKind::Success, format!("Queued for {n} target(s)")),
            Err(e) => self.toast(cx, ToastKind::Error, e.to_string()),
        }
    }

    /// Targets ticked in the chips, or every target when none are ticked.
    fn targets_for(&self, cx: &mut Context<Self>) -> Vec<String> {
        if self.chosen.is_empty() {
            self.app.read(cx).settings.publish_targets.iter().map(|t| t.name.clone()).collect()
        } else {
            self.chosen.clone()
        }
    }

    fn add_schedule(&mut self, cx: &mut Context<Self>) {
        let Some((id, _)) = self.graphic_id(cx) else {
            self.toast(cx, ToastKind::Error, "Type the name of a library graphic first".into());
            return;
        };
        let targets = self.targets_for(cx);
        if targets.is_empty() {
            self.toast(cx, ToastKind::Error, "Add a publishing target first".into());
            return;
        }
        let sched = Schedule { graphic_id: id, targets, every_minutes: self.every, condition: self.condition.into(), last_run: 0 };
        self.update(cx, move |s| s.publish_schedules.push(sched));
    }
}

impl Render for PublishingView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (settings, graphics) = {
            let a = self.app.read(cx);
            (a.settings.clone(), a.graphics.clone())
        };
        let (pending, history) = publish::status();
        let name_of = |id: &str| graphics.iter().find(|g| g.id == id).map(|g| g.name.clone()).unwrap_or_else(|| "(deleted graphic)".into());

        let targets = settings.publish_targets.iter().enumerate().map(|(i, t)| {
            let name = t.name.clone();
            let picked = self.chosen.contains(&t.name);
            row()
                .gap(px(8.))
                .py(px(6.))
                .border_b_1()
                .border_color(c(BORDER))
                .child(chip(("pub-pick", i), t.name.clone(), picked).on_click(cx.listener({
                    let name = name.clone();
                    move |this, _, _, cx| {
                        if let Some(p) = this.chosen.iter().position(|n| *n == name) {
                            this.chosen.remove(p);
                        } else {
                            this.chosen.push(name.clone());
                        }
                        cx.notify();
                    }
                })))
                .child(badge(t.kind.to_uppercase(), 0x4F8FD6))
                .child(badge(format!("{} {}", t.format, t.encoding).to_uppercase(), 0x6B7590))
                .child(div().flex_1().min_w_0().text_size(px(12.)).text_color(c(TEXT)).truncate().child(t.address.clone()))
                .when(!t.purge_url.is_empty(), |d| d.child(badge("PURGE", 0x3DD68C)))
                .child(button_small(("pub-del", i), "Remove").on_click(cx.listener(move |this, _, _, cx| {
                    let n = name.clone();
                    this.chosen.retain(|x| *x != n);
                    this.update(cx, move |s| s.publish_targets.retain(|t| t.name != n));
                })))
        }).collect::<Vec<_>>();

        let schedules = settings.publish_schedules.iter().enumerate().map(|(i, s)| {
            row()
                .gap(px(8.))
                .py(px(6.))
                .border_b_1()
                .border_color(c(BORDER))
                .child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(format!(
                    "{} every {} min → {} ({})",
                    name_of(&s.graphic_id),
                    s.every_minutes,
                    s.targets.join(", "),
                    match s.condition.as_str() {
                        "warnings" => "only while warnings are in coverage",
                        "calm" => "only without warnings",
                        "changed" => "only when the picture changed",
                        _ => "always",
                    }
                )))
                .child(button_small(("sched-del", i), "Remove").on_click(cx.listener(move |this, _, _, cx| {
                    this.update(cx, move |s| {
                        if i < s.publish_schedules.len() {
                            s.publish_schedules.remove(i);
                        }
                    });
                })))
        }).collect::<Vec<_>>();

        let job_row = |j: &publish::Job| {
            let when = chrono::DateTime::from_timestamp(j.time, 0).map(|d| d.with_timezone(&chrono::Local).format("%-I:%M:%S %p").to_string()).unwrap_or_default();
            let color = match j.status.as_str() {
                "delivered" => 0x3DD68C,
                "failed" => 0xE5383B,
                "queued" => 0x6B7590,
                _ => 0xE0A030,
            };
            row()
                .gap(px(8.))
                .py(px(4.))
                .child(badge(j.status.to_uppercase(), color))
                .child(div().flex_1().text_size(px(12.)).text_color(c(TEXT)).child(format!("{when} • {} (version {}) → {}{}", j.graphic, j.version, j.target, if j.error.is_empty() { String::new() } else { format!(" • {}", j.error) })))
        };
        let pending_rows = pending.iter().map(job_row).collect::<Vec<_>>();
        let history_rows = history.iter().map(job_row).collect::<Vec<_>>();

        let base = format!("http://<this PC or your public address>:{}", settings.portal_port);
        let usage = publish::usage_rows();

        col()
            .gap(px(12.))
            .child(muted("Send graphics to your website, CDN, social folders and chat. Each target gets the graphic laid out for its shape. Failed deliveries retry three times, then alert you through the notification webhook (Settings > Weather & maps > Warning desk). Every delivery is written to Publishing history.csv in the library."))
            .child(heading("Targets"))
            .child(muted("Tick targets to choose where Publish now and new schedules go (none ticked = all)."))
            .when(targets.is_empty(), |d| d.child(muted("No targets yet.")))
            .children(targets)
            .child(div().child(Input::new(&self.t_name)))
            .child(div().child(Input::new(&self.t_address)))
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.t_header))).child(div().flex_1().child(Input::new(&self.t_purge))))
            .child(row().gap(px(4.)).children([("folder", "Folder"), ("http", "Upload (CDN / storage)"), ("discord", "Discord"), ("slack", "Slack"), ("dcweather", "DCWeather Facebook")].into_iter().enumerate().map(|(i, (k, t))| {
                chip(("t-kind", i), t, self.t_kind == k).on_click(cx.listener(move |this, _, _, cx| {
                    this.t_kind = k;
                    if k == "dcweather" && this.dcw_sponsors.is_empty() {
                        this.load_dcw_sponsors(cx);
                    }
                    cx.notify();
                }))
            }).collect::<Vec<_>>()))
            .when(self.t_kind == "dcweather", |d| {
                let signed_in = crate::dcweather::has_token(&settings.api_base);
                d.child(muted(if signed_in {
                    "Posts to the DCWeather Facebook page through the site. Type the caption in the address box ({name} = graphic name, {station} = station name). Facebook posts are never retried automatically, so nothing is posted twice."
                } else {
                    "Sign in to DCWeather first: Settings > Integrations > DCWeather account."
                }))
                .child(row().gap(px(4.)).flex_wrap().child(muted("Sponsor")).child(chip("t-sp-none", "None", self.t_sponsor.is_empty()).on_click(cx.listener(|this, _, _, cx| {
                    this.t_sponsor.clear();
                    cx.notify();
                }))).children(self.dcw_sponsors.iter().filter(|s| s.active).enumerate().map(|(i, s)| {
                    let id = s.id.clone();
                    chip(("t-sp", i), s.name.clone(), self.t_sponsor == s.id).on_click(cx.listener(move |this, _, _, cx| {
                        this.t_sponsor = id.clone();
                        cx.notify();
                    }))
                }).collect::<Vec<_>>()).child(button_small("t-sp-load", "Reload sponsors").on_click(cx.listener(|this, _, _, cx| this.load_dcw_sponsors(cx)))))
            })
            .child(row().gap(px(4.)).children([("landscape", "16:9"), ("square", "Square"), ("vertical", "9:16"), ("story", "4:5")].into_iter().enumerate().map(|(i, (k, t))| {
                chip(("t-format", i), t, self.t_format == k).on_click(cx.listener(move |this, _, _, cx| {
                    this.t_format = k;
                    cx.notify();
                }))
            }).collect::<Vec<_>>()).child(div().w(px(16.))).children([("png", "PNG"), ("jpg", "JPEG")].into_iter().enumerate().map(|(i, (k, t))| {
                chip(("t-enc", i), t, self.t_encoding == k).on_click(cx.listener(move |this, _, _, cx| {
                    this.t_encoding = k;
                    cx.notify();
                }))
            }).collect::<Vec<_>>()).child(div().flex_1()).child(button_small("t-add", "Add target").on_click(cx.listener(|this, _, _, cx| this.add_target(cx)))))
            .child(heading("Publish"))
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.graphic))).child(button_small("pub-now", "Publish now").on_click(cx.listener(|this, _, _, cx| this.publish_now(cx)))))
            .when(settings.require_approval, |d| d.child(muted("Approval is on: only graphics approved in Graphics Studio can be published.")))
            .child(row().gap(px(4.)).child(muted("Every")).children([15u32, 30, 60, 180, 360].into_iter().enumerate().map(|(i, m)| {
                chip(("sched-every", i), if m < 60 { format!("{m} min") } else { format!("{} h", m / 60) }, self.every == m).on_click(cx.listener(move |this, _, _, cx| {
                    this.every = m;
                    cx.notify();
                }))
            }).collect::<Vec<_>>()))
            .child(row().gap(px(4.)).child(muted("When")).children([("always", "Always"), ("warnings", "Warnings in coverage"), ("calm", "No warnings"), ("changed", "Picture changed")].into_iter().enumerate().map(|(i, (k, t))| {
                chip(("sched-cond", i), t, self.condition == k).on_click(cx.listener(move |this, _, _, cx| {
                    this.condition = k;
                    cx.notify();
                }))
            }).collect::<Vec<_>>()).child(div().flex_1()).child(button_small("sched-add", "Add schedule").on_click(cx.listener(|this, _, _, cx| this.add_schedule(cx)))))
            .children(schedules)
            .child(heading(format!("Queue ({})", pending.len())))
            .when(pending_rows.is_empty(), |d| d.child(muted("Nothing waiting.")))
            .children(pending_rows)
            .child(heading("Recent deliveries"))
            .when(history_rows.is_empty(), |d| d.child(muted("Nothing delivered yet this session. Older deliveries are in Publishing history.csv.")))
            .children(history_rows)
            .child(button_small("pub-history", "Open Publishing history.csv").on_click(|_, _, _| {
                let _ = open::that(crate::config::LibraryPaths::get().root.join("Publishing history.csv"));
            }))
            .child(heading("Public endpoints"))
            .child(toggle("pub-public", "Serve public images, data and widgets on the viewer portal", settings.public_endpoints).on_click(cx.listener(|this, _, _, cx| {
                this.update(cx, |s| s.public_endpoints = !s.public_endpoints);
            })))
            .when(!settings.portal_enabled, |d| d.child(muted("Turn on the viewer portal in Settings > Integrations first (public endpoints share its port).")))
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.key))).child(button_small("pub-key", "Save key").on_click(cx.listener(|this, _, _, cx| {
                let k = this.key.read(cx).value().trim().to_string();
                this.update(cx, move |s| s.public_key = k);
            }))))
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.allowed))).child(button_small("pub-allowed", "Save list").on_click(cx.listener(|this, _, _, cx| {
                let list: Vec<String> = this.allowed.read(cx).value().split(';').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
                this.update(cx, move |s| s.public_graphics = list);
            }))))
            .child(row().gap(px(4.)).child(muted("Images are re-rendered after")).children([60u32, 120, 300, 900].into_iter().enumerate().map(|(i, secs)| {
                chip(("pub-cache", i), if secs < 120 { format!("{secs} s") } else { format!("{} min", secs / 60) }, settings.public_cache_secs == secs).on_click(cx.listener(move |this, _, _, cx| {
                    this.update(cx, move |s| s.public_cache_secs = secs);
                }))
            }).collect::<Vec<_>>()).child(muted("or when the data refreshes")))
            .child(muted(format!(
                "{base}/public/ lists everything. Images: /public/forecast.png, /public/current.png, /public/warning.png, /public/radar.jpg, /public/graphic.png?name=…  Data: /public/data.json  Widget: <script src=\"{base}/public/widget.js\"></script>  Embed: <iframe src=\"{base}/embed?what=forecast\"></iframe>{}",
                if settings.public_key.is_empty() { String::new() } else { "  Add key=… to each address.".to_string() }
            )))
            .child(heading("Usage"))
            .when(usage.is_empty(), |d| d.child(muted("No requests yet.")))
            .children(usage.into_iter().map(|(day, endpoint, n)| row().gap(px(8.)).child(div().w(px(110.)).text_size(px(12.)).text_color(c(TEXT)).child(day)).child(div().flex_1().text_size(px(12.)).text_color(c(TEXT)).child(endpoint)).child(muted(n.to_string()))).collect::<Vec<_>>())
    }
}
