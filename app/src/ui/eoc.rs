//! Emergency operations dashboard (modal): incident status at a glance, shelters, evacuation
//! zones, resources, road closures, contacts, the incident timeline, a generated briefing and
//! the operations report export.

use gpui_kit::{
    component::input::{Input, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::{AppState, ToastKind},
    eoc::{self, Closure, Contact, Resource, Shelter},
    ui::{badge, button_small, c, chip, col, heading, muted, row, toggle, BORDER, TEXT},
};

struct Inputs {
    incident: Entity<InputState>,
    sh_name: Entity<InputState>,
    sh_where: Entity<InputState>,
    sh_cap: Entity<InputState>,
    sh_phone: Entity<InputState>,
    res_name: Entity<InputState>,
    res_where: Entity<InputState>,
    res_note: Entity<InputState>,
    cl_road: Entity<InputState>,
    cl_where: Entity<InputState>,
    cl_reason: Entity<InputState>,
    ct_name: Entity<InputState>,
    ct_role: Entity<InputState>,
    ct_agency: Entity<InputState>,
    ct_phone: Entity<InputState>,
    ct_email: Entity<InputState>,
    note: Entity<InputState>,
    zone_field: Entity<InputState>,
    out_name: Entity<InputState>,
    out_url: Entity<InputState>,
}

pub struct EocView {
    app: Entity<AppState>,
    i: Inputs,
    res_kind: &'static str,
    pets: bool,
    briefing: String,
    _subscriptions: Vec<Subscription>,
}

/// "lat, lon" or a town name.
fn locate(text: &str) -> Option<(f64, f64)> {
    crate::places::parse_lat_lon(text).or_else(|| crate::places::search(text, 1).first().map(|p| (p.lat, p.lon)))
}

impl EocView {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut mk = |ph: &str| {
            let ph = ph.to_string();
            cx.new(|cx| InputState::new(window, cx).placeholder(ph))
        };
        let i = Inputs {
            incident: mk("Incident name, e.g. March 14 tornado outbreak"),
            sh_name: mk("Shelter name"),
            sh_where: mk("Town or lat, lon"),
            sh_cap: mk("Capacity"),
            sh_phone: mk("Phone"),
            res_name: mk("Resource, e.g. County highway staging"),
            res_where: mk("Town or lat, lon"),
            res_note: mk("Note"),
            cl_road: mk("Road, e.g. SR 37 at Bean Blossom Creek"),
            cl_where: mk("Town or lat, lon"),
            cl_reason: mk("Reason, e.g. high water"),
            ct_name: mk("Name"),
            ct_role: mk("Role"),
            ct_agency: mk("Agency"),
            ct_phone: mk("Phone"),
            ct_email: mk("Email"),
            note: mk("Timeline note"),
            zone_field: mk("Zone name field in the file (optional)"),
            out_name: mk("Utility name, e.g. Duke Energy"),
            out_url: mk("Outage map address (Kubra StormCenter …/stormcenter/…/views/…) or JSON feed"),
        };
        Self { app: app.clone(), i, res_kind: "staging", pets: false, briefing: String::new(), _subscriptions: vec![cx.observe(&app, |_, _, cx| cx.notify())] }
    }

    fn val(&self, e: &Entity<InputState>, cx: &mut Context<Self>) -> String {
        e.read(cx).value().trim().to_string()
    }

    fn clear(&self, list: &[&Entity<InputState>], window: &mut Window, cx: &mut Context<Self>) {
        for e in list {
            e.update(cx, |s, cx| s.set_value(String::new(), window, cx));
        }
    }

    fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        self.app.update(cx, |s, cx| {
            s.toast(kind, msg);
            cx.notify();
        });
    }
}

fn tile(label: &str, value: String, color: u32) -> Div {
    col()
        .flex_1()
        .p(px(10.))
        .gap(px(2.))
        .rounded(px(8.))
        .border_1()
        .border_color(c(BORDER))
        .bg(c(0x0F141F))
        .child(div().text_size(px(24.)).font_weight(FontWeight::BOLD).text_color(c(color)).child(value))
        .child(div().text_size(px(12.)).text_color(c(0x9AA3B5)).child(label.to_string()))
}

impl Render for EocView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let e = eoc::get();
        let (warnings, closures_indot, coverage, live_on) = {
            let a = self.app.read(cx);
            // Only closures in the station's coverage area count for the EOC (INDOT sends the
            // whole state: 296 closures read as a county emergency).
            let home = a.settings.home;
            let miles = a.settings.coverage_miles.max(10.0);
            let near = a.data.roads.iter()
                .filter(|r| r.kind == "closure")
                .filter(|r| crate::map::geo::haversine_km(home.lat, home.lon, r.lat, r.lon) / crate::map::geo::KM_PER_MILE <= miles)
                .count();
            (a.data.local_alerts.len(), near, miles, a.live.eoc)
        };
        let open = e.shelters.iter().filter(|s| s.status == "open" || s.status == "full").count();
        let (occ, cap) = e.shelters.iter().filter(|s| s.status == "open" || s.status == "full").fold((0, 0), |(o, c), s| (o + s.occupancy, c + s.capacity));
        let mandatory = e.zones.iter().filter(|z| z.status == "mandatory").count();

        let shelters = e.shelters.iter().enumerate().map(|(i, s)| {
            let color = match s.status.as_str() {
                "open" => 0x3DD68C,
                "full" => 0xE5383B,
                _ => 0x6B7590,
            };
            row()
                .gap(px(6.))
                .py(px(5.))
                .border_b_1()
                .border_color(c(BORDER))
                .child(badge(s.status.to_uppercase(), color))
                .child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(format!("{} • {} / {}{}{}", s.name, s.occupancy, s.capacity, if s.pets { " • pets" } else { "" }, if s.phone.is_empty() { String::new() } else { format!(" • {}", s.phone) })))
                .children(["open", "full", "closed", "standby"].into_iter().enumerate().map(|(j, st)| {
                    let name = s.name.clone();
                    chip(("sh-st", i * 10 + j), st, s.status == st).on_click(cx.listener(move |_, _, _, cx| {
                        let n = name.clone();
                        eoc::update(Some(("shelter", format!("Shelter {n} is {st}"))), |e| {
                            if let Some(s) = e.shelters.iter_mut().find(|s| s.name == n) {
                                s.status = st.into();
                                s.updated = chrono::Utc::now().timestamp();
                            }
                        });
                        cx.notify();
                    }))
                }).collect::<Vec<_>>())
                .children([(-10i32, "−10"), (-1, "−1"), (1, "+1"), (10, "+10")].into_iter().enumerate().map(|(j, (d, t))| {
                    let name = s.name.clone();
                    button_small(("sh-occ", i * 10 + j), t).on_click(cx.listener(move |_, _, _, cx| {
                        let n = name.clone();
                        eoc::update(None, |e| {
                            if let Some(s) = e.shelters.iter_mut().find(|s| s.name == n) {
                                s.occupancy = (s.occupancy as i32 + d).max(0) as u32;
                                s.updated = chrono::Utc::now().timestamp();
                                if s.capacity > 0 && s.occupancy >= s.capacity && s.status == "open" {
                                    s.status = "full".into();
                                }
                            }
                        });
                        cx.notify();
                    }))
                }).collect::<Vec<_>>())
                .child(button_small(("sh-del", i), "✕").on_click(cx.listener({
                    let name = s.name.clone();
                    move |_, _, _, cx| {
                        let n = name.clone();
                        eoc::update(None, |e| e.shelters.retain(|s| s.name != n));
                        cx.notify();
                    }
                })))
        }).collect::<Vec<_>>();

        let zones = e.zones.iter().enumerate().map(|(i, z)| {
            row()
                .gap(px(6.))
                .py(px(4.))
                .child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(z.name.clone()))
                .children(["none", "advisory", "voluntary", "mandatory"].into_iter().enumerate().map(|(j, st)| {
                    chip(("zone-st", i * 10 + j), st, z.status == st || (st == "none" && z.status.is_empty())).on_click(cx.listener(move |_, _, _, cx| {
                        eoc::update(None, |e| {
                            if let Some(z) = e.zones.get_mut(i) {
                                z.status = st.into();
                                let text = if st == "none" { format!("Evacuation order lifted: {}", z.name) } else { format!("{} evacuation: {}", st, z.name) };
                                e.timeline.push(eoc::Entry { time: chrono::Utc::now().timestamp(), kind: "zone".into(), text });
                            }
                        });
                        cx.notify();
                    }))
                }).collect::<Vec<_>>())
                .child(button_small(("zone-del", i), "✕").on_click(cx.listener(move |_, _, _, cx| {
                    eoc::update(None, |e| {
                        if i < e.zones.len() {
                            e.zones.remove(i);
                        }
                    });
                    cx.notify();
                })))
        }).collect::<Vec<_>>();

        let resources = e.resources.iter().enumerate().map(|(i, r)| {
            row().gap(px(6.)).py(px(3.)).child(badge(r.kind.to_uppercase(), 0x4F8FD6)).child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(format!("{}{}", r.name, if r.note.is_empty() { String::new() } else { format!(" • {}", r.note) }))).child(button_small(("res-del", i), "✕").on_click(cx.listener(move |_, _, _, cx| {
                eoc::update(None, |e| {
                    if i < e.resources.len() {
                        e.resources.remove(i);
                    }
                });
                cx.notify();
            })))
        }).collect::<Vec<_>>();

        let closures = e.closures.iter().enumerate().map(|(i, cl)| {
            row().gap(px(6.)).py(px(3.)).child(badge("CLOSED", 0xE5383B)).child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(format!("{} • {}", cl.road, cl.reason))).child(button_small(("cl-del", i), "Reopen").on_click(cx.listener(move |_, _, _, cx| {
                eoc::update(None, |e| {
                    if i < e.closures.len() {
                        let c = e.closures.remove(i);
                        e.timeline.push(eoc::Entry { time: chrono::Utc::now().timestamp(), kind: "road".into(), text: format!("Road reopened: {}", c.road) });
                    }
                });
                cx.notify();
            })))
        }).collect::<Vec<_>>();

        let contacts = e.contacts.iter().enumerate().map(|(i, ct)| {
            row().gap(px(8.)).py(px(3.)).child(div().w(px(180.)).text_size(px(13.)).text_color(c(TEXT)).child(ct.name.clone())).child(div().flex_1().text_size(px(12.)).text_color(c(TEXT)).child(format!("{} • {} • {} • {}", ct.role, ct.agency, ct.phone, ct.email))).child(button_small(("ct-del", i), "✕").on_click(cx.listener(move |_, _, _, cx| {
                eoc::update(None, |e| {
                    if i < e.contacts.len() {
                        e.contacts.remove(i);
                    }
                });
                cx.notify();
            })))
        }).collect::<Vec<_>>();

        let timeline = e.timeline.iter().rev().take(60).map(|t| {
            let when = chrono::DateTime::from_timestamp(t.time, 0).map(|d| d.with_timezone(&chrono::Local).format("%b %-d %-I:%M %p").to_string()).unwrap_or_default();
            let color = match t.kind.as_str() {
                "warning" => 0xE5383B,
                "zone" => 0xE0A030,
                "road" => 0xD06030,
                "shelter" => 0x3DD68C,
                _ => 0x6B7590,
            };
            row().gap(px(8.)).py(px(2.)).child(div().w(px(120.)).text_size(px(12.)).text_color(c(0x9AA3B5)).child(when)).child(badge(t.kind.to_uppercase(), color)).child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(t.text.clone()))
        }).collect::<Vec<_>>();

        col()
            .gap(px(12.))
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.i.incident))).child(button_small("inc-start", if e.incident.name.is_empty() { "Open incident" } else { "Rename" }).on_click(cx.listener(|this, _, _, cx| {
                let name = this.val(&this.i.incident, cx);
                if name.is_empty() {
                    return;
                }
                let first = eoc::get().incident.name.is_empty();
                eoc::update(Some(("note", if first { format!("Incident opened: {name}") } else { format!("Incident renamed: {name}") })), |e| {
                    e.incident.name = name;
                    if first {
                        e.incident.started = chrono::Utc::now().timestamp();
                        e.incident.level = "monitoring".into();
                    }
                });
                cx.notify();
            }))).when(!e.incident.name.is_empty(), |d| d.child(button_small("inc-close", "Close incident").on_click(cx.listener(|_, _, _, cx| {
                eoc::update(Some(("note", "Incident closed".into())), |e| e.incident = Default::default());
                cx.notify();
            })))))
            .child(row().gap(px(4.)).child(muted(if e.incident.name.is_empty() { "No open incident. Timeline entries for warnings and closures start when one is opened.".to_string() } else { format!("{} • activation:", e.incident.name) })).when(!e.incident.name.is_empty(), |d| {
                d.children(["monitoring", "partial", "full"].into_iter().enumerate().map(|(j, lvl)| {
                    chip(("inc-lvl", j), lvl, e.incident.level == lvl).on_click(cx.listener(move |_, _, _, cx| {
                        eoc::update(Some(("note", format!("Activation level: {lvl}"))), |e| e.incident.level = lvl.into());
                        cx.notify();
                    }))
                }).collect::<Vec<_>>())
            }))
            .child(row().gap(px(8.))
                .child(tile("warnings in effect", warnings.to_string(), if warnings > 0 { 0xE5383B } else { 0x3DD68C }))
                .child(tile("shelters open", open.to_string(), 0x3DD68C))
                .child(tile("sheltered / capacity", format!("{occ} / {cap}"), if cap > 0 && occ * 100 >= cap * 85 { 0xE0A030 } else { 0xE4E6EA }))
                .child(tile("mandatory evacuations", mandatory.to_string(), if mandatory > 0 { 0xE5383B } else { 0xE4E6EA }))
                .child(tile("road closures", (closures_indot + e.closures.len()).to_string(), 0xD06030))
                .child(tile("resources", e.resources.len().to_string(), 0x4F8FD6)))
            .child(toggle("eoc-map", "Show shelters, zones, resources and closures on the map (Emergency management layer)", live_on).on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, _| s.update_live(|l| l.eoc = !l.eoc));
            })))
            .child(heading("Shelters"))
            .when(shelters.is_empty(), |d| d.child(muted("No shelters yet.")))
            .children(shelters)
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.i.sh_name))).child(div().flex_1().child(Input::new(&self.i.sh_where))).child(div().w(px(100.)).child(Input::new(&self.i.sh_cap))).child(div().w(px(140.)).child(Input::new(&self.i.sh_phone))).child(chip("sh-pets", "Pets OK", self.pets).on_click(cx.listener(|this, _, _, cx| {
                this.pets = !this.pets;
                cx.notify();
            }))).child(button_small("sh-add", "Add shelter").on_click(cx.listener(|this, _, window, cx| {
                let name = this.val(&this.i.sh_name, cx);
                let Some((lat, lon)) = locate(&this.val(&this.i.sh_where, cx)) else {
                    this.toast(cx, ToastKind::Error, "Couldn't find that place. Use a town name or lat, lon");
                    return;
                };
                if name.is_empty() {
                    return;
                }
                let sh = Shelter { name: name.clone(), lat, lon, capacity: this.val(&this.i.sh_cap, cx).parse().unwrap_or(0), occupancy: 0, status: "open".into(), pets: this.pets, phone: this.val(&this.i.sh_phone, cx), updated: chrono::Utc::now().timestamp() };
                eoc::update(Some(("shelter", format!("Shelter opened: {name}"))), |e| e.shelters.push(sh));
                this.clear(&[&this.i.sh_name, &this.i.sh_where, &this.i.sh_cap, &this.i.sh_phone], window, cx);
                cx.notify();
            }))))
            .child(heading("Evacuation zones"))
            .when(zones.is_empty(), |d| d.child(muted("Load zones from a GeoJSON or shapefile from your county GIS.")))
            .children(zones)
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.i.zone_field))).child(button_small("zone-import", "Load zones…").on_click(cx.listener(|this, _, _, cx| {
                let field = this.val(&this.i.zone_field, cx);
                crate::ui::pick_off_thread(cx, || rfd::FileDialog::new().add_filter("Zones", &["geojson", "json", "zip", "shp"]).pick_file(), move |this: &mut Self, file, cx| {
                    match eoc::import_zones(&file, &field) {
                        Ok(n) => this.toast(cx, ToastKind::Success, format!("{n} zones loaded")),
                        Err(e) => this.toast(cx, ToastKind::Error, e.to_string()),
                    }
                    cx.notify();
                });
            }))))
            .child(heading("Resources"))
            .children(resources)
            .child(row().flex_wrap().gap(px(4.)).children([
                ("staging", "Staging"), ("sandbags", "Sandbags"), ("generator", "Generator"), ("ems", "EMS"),
                ("fire", "Fire"), ("police", "Police"), ("water", "Water"), ("other", "Other"),
            ].into_iter().enumerate().map(|(j, (k, label))| {
                chip(("res-kind", j), label, self.res_kind == k).on_click(cx.listener(move |this, _, _, cx| {
                    this.res_kind = k;
                    cx.notify();
                }))
            }).collect::<Vec<_>>()))
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.i.res_name))).child(div().flex_1().child(Input::new(&self.i.res_where))).child(div().flex_1().child(Input::new(&self.i.res_note))).child(button_small("res-add", "Add resource").on_click(cx.listener(|this, _, window, cx| {
                let name = this.val(&this.i.res_name, cx);
                let Some((lat, lon)) = locate(&this.val(&this.i.res_where, cx)) else {
                    this.toast(cx, ToastKind::Error, "Couldn't find that place");
                    return;
                };
                if name.is_empty() {
                    return;
                }
                let r = Resource { name: name.clone(), kind: this.res_kind.into(), lat, lon, note: this.val(&this.i.res_note, cx) };
                eoc::update(Some(("resource", format!("Resource placed: {name} ({})", this.res_kind))), |e| e.resources.push(r));
                this.clear(&[&this.i.res_name, &this.i.res_where, &this.i.res_note], window, cx);
                cx.notify();
            }))))
            .child(heading("Road closures"))
            .child(muted(format!("{closures_indot} INDOT closures within {coverage:.0} miles of home are counted and on the map automatically. Add local ones here.")))
            .children(closures)
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.i.cl_road))).child(div().flex_1().child(Input::new(&self.i.cl_where))).child(div().flex_1().child(Input::new(&self.i.cl_reason))).child(button_small("cl-add", "Close road").on_click(cx.listener(|this, _, window, cx| {
                let road = this.val(&this.i.cl_road, cx);
                let Some((lat, lon)) = locate(&this.val(&this.i.cl_where, cx)) else {
                    this.toast(cx, ToastKind::Error, "Couldn't find that place");
                    return;
                };
                if road.is_empty() {
                    return;
                }
                let reason = this.val(&this.i.cl_reason, cx);
                let cl = Closure { road: road.clone(), lat, lon, reason: reason.clone(), time: chrono::Utc::now().timestamp() };
                eoc::update(Some(("road", format!("Road closed: {road} ({reason})"))), |e| e.closures.push(cl));
                this.clear(&[&this.i.cl_road, &this.i.cl_where, &this.i.cl_reason], window, cx);
                cx.notify();
            }))))
            .child(heading("Power outages"))
            .child({
                let (o, sources) = {
                    let a = self.app.read(cx);
                    (a.data.outages.clone(), a.settings.outage_sources.clone())
                };
                col()
                    .gap(px(4.))
                    .child(muted(if sources.is_empty() { "Add your utilities' outage maps to track customers out by county (updated every 10 minutes).".to_string() } else { format!("{} customers out • {:.0}% restored since the peak • {}", o.total_out, o.restored_pct(), if o.updated > 0 { chrono::DateTime::from_timestamp(o.updated, 0).map(|d| d.with_timezone(&chrono::Local).format("updated %-I:%M %p").to_string()).unwrap_or_default() } else { "waiting for the first update".into() }) }))
                    .children(o.problems.iter().map(|p| muted(format!("⚠ {p}"))).collect::<Vec<_>>())
                    .children(o.counties.iter().filter(|c| c.out > 0).take(8).map(|c| muted(format!("  {}: {} out ({:.1}%){}", c.name, c.out, c.pct(), if c.etr.is_empty() { String::new() } else { format!(" • ETR {}", c.etr) }))).collect::<Vec<_>>())
                    .children(sources.iter().enumerate().map(|(i, s)| {
                        let name = s.name.clone();
                        row().gap(px(6.)).child(badge("UTILITY", 0x4F8FD6)).child(div().flex_1().text_size(px(12.)).text_color(c(TEXT)).child(format!("{} • {}", s.name, s.url))).child(button_small(("out-del", i), "✕").on_click(cx.listener(move |this, _, _, cx| {
                            let n = name.clone();
                            this.app.update(cx, |s, cx| {
                                let mut next = s.settings.clone();
                                next.outage_sources.retain(|x| x.name != n);
                                s.apply_settings(next);
                                cx.notify();
                            });
                        })))
                    }).collect::<Vec<_>>())
                    .child(row().gap(px(6.)).flex_wrap().child(muted("Presets:")).children(crate::outages::PRESETS.iter().enumerate().filter(|(_, (n, _))| !sources.iter().any(|s| s.name == *n)).map(|(i, (name, url))| {
                        button_small(("out-preset", i), format!("+ {name}")).on_click(cx.listener(move |this, _, _, cx| {
                            this.app.update(cx, |s, cx| {
                                let mut next = s.settings.clone();
                                next.outage_sources.retain(|x| x.name != *name);
                                next.outage_sources.push(crate::outages::Source { name: name.to_string(), url: url.to_string() });
                                s.apply_settings(next);
                                cx.notify();
                            });
                        }))
                    }).collect::<Vec<_>>()))
                    .child(row().gap(px(6.)).child(div().w(px(220.)).child(Input::new(&self.i.out_name))).child(div().flex_1().child(Input::new(&self.i.out_url))).child(button_small("out-add", "Add utility").on_click(cx.listener(|this, _, window, cx| {
                        let src = crate::outages::Source { name: this.val(&this.i.out_name, cx), url: this.val(&this.i.out_url, cx) };
                        if src.name.is_empty() || src.url.is_empty() {
                            return;
                        }
                        this.app.update(cx, |s, cx| {
                            let mut next = s.settings.clone();
                            next.outage_sources.retain(|x| x.name != src.name);
                            next.outage_sources.push(src);
                            s.apply_settings(next);
                            cx.notify();
                        });
                        this.clear(&[&this.i.out_name, &this.i.out_url], window, cx);
                    }))))
            })
            .child(heading("Contacts"))
            .children(contacts)
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.i.ct_name))).child(div().flex_1().child(Input::new(&self.i.ct_role))).child(div().flex_1().child(Input::new(&self.i.ct_agency))))
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.i.ct_phone))).child(div().flex_1().child(Input::new(&self.i.ct_email))).child(button_small("ct-add", "Add contact").on_click(cx.listener(|this, _, window, cx| {
                let ct = Contact { name: this.val(&this.i.ct_name, cx), role: this.val(&this.i.ct_role, cx), agency: this.val(&this.i.ct_agency, cx), phone: this.val(&this.i.ct_phone, cx), email: this.val(&this.i.ct_email, cx) };
                if ct.name.is_empty() {
                    return;
                }
                eoc::update(None, |e| e.contacts.push(ct));
                this.clear(&[&this.i.ct_name, &this.i.ct_role, &this.i.ct_agency, &this.i.ct_phone, &this.i.ct_email], window, cx);
                cx.notify();
            }))))
            .child(heading("Timeline"))
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.i.note))).child(button_small("note-add", "Add note").on_click(cx.listener(|this, _, window, cx| {
                let t = this.val(&this.i.note, cx);
                if t.is_empty() {
                    return;
                }
                eoc::update(Some(("note", t)), |_| {});
                this.clear(&[&this.i.note], window, cx);
                cx.notify();
            }))))
            .when(timeline.is_empty(), |d| d.child(muted("Nothing yet.")))
            .children(timeline)
            .child(heading("Briefing and report"))
            .child(row().gap(px(6.)).child(button_small("brief", "Generate briefing").on_click(cx.listener(|this, _, _, cx| {
                let (data, settings) = {
                    let a = this.app.read(cx);
                    (a.data.clone(), a.settings.clone())
                };
                this.briefing = eoc::briefing(&data, &settings);
                cx.notify();
            }))).when(!self.briefing.is_empty(), |d| d.child(button_small("brief-copy", "Copy").on_click(cx.listener(|this, _, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(this.briefing.clone()));
                this.toast(cx, ToastKind::Success, "Briefing copied");
            })))).child(button_small("report", "Export operations report").on_click(cx.listener(|this, _, _, cx| {
                let (data, settings) = {
                    let a = this.app.read(cx);
                    (a.data.clone(), a.settings.clone())
                };
                match eoc::export_report(&data, &settings) {
                    Ok(p) => {
                        let _ = open::that(&p);
                        this.toast(cx, ToastKind::Success, format!("Saved {}", p.display()));
                    }
                    Err(e) => this.toast(cx, ToastKind::Error, e.to_string()),
                }
            }))))
            .when(!self.briefing.is_empty(), |d| d.child(div().p(px(12.)).rounded(px(6.)).bg(c(0x0F141F)).border_1().border_color(c(BORDER)).text_size(px(13.)).text_color(c(TEXT)).font_family("Consolas").children(self.briefing.lines().map(|l| div().min_h(px(16.)).child(l.to_string())).collect::<Vec<_>>())))
    }
}
