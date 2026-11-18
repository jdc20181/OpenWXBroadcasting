//! Forecast editor dialog: the station forecast on top of the model and NWS data.

use gpui_kit::{
    component::input::{Input, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::AppState,
    forecast_edit::EditVar,
    ui::{badge, button, button_primary, button_small, c, chip, col, muted, row, section, toggle, AMBER, BORDER, GREEN, MUTED, PANEL_2, RED, TEXT},
};

const CONDITIONS: &[&str] = &[
    "Sunny", "Mostly Sunny", "Partly Cloudy", "Mostly Cloudy", "Cloudy", "Chance Showers", "Showers", "Rain",
    "Chance Thunderstorms", "Thunderstorms", "Snow", "Rain and Snow", "Freezing Rain", "Fog", "Windy",
];

pub struct ForecastEditor {
    app: Entity<AppState>,
    scenario_name: Entity<InputState>,
    region_name: Entity<InputState>,
    area_var: EditVar,
    area_delta: f32,
    open_day: Option<usize>,
}

impl ForecastEditor {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            app,
            scenario_name: cx.new(|cx| InputState::new(window, cx).placeholder("Scenario name (e.g. Colder solution)")),
            region_name: cx.new(|cx| InputState::new(window, cx).placeholder("Region name (e.g. North of I-64)")),
            area_var: EditVar::Temp,
            area_delta: -3.0,
            open_day: Some(0),
        }
    }

    fn edit(&mut self, cx: &mut Context<Self>, what: &str, f: impl FnOnce(&mut crate::forecast_edit::EditSet)) {
        let what = what.to_string();
        self.app.update(cx, |s, cx| {
            f(&mut s.ext.edits.draft);
            s.edits_changed(&what);
            cx.notify();
        });
        cx.notify();
    }
}

fn stepper<V: 'static>(
    id: &'static str,
    i: usize,
    label: String,
    cx: &mut Context<V>,
    on: impl Fn(&mut V, f32, &mut Context<V>) + Clone + 'static,
) -> Div {
    let (a, b) = (on.clone(), on);
    row()
        .gap(px(4.))
        .child(button_small((id, i * 2), "−").on_click(cx.listener(move |this, _, _, cx| a(this, -1.0, cx))))
        .child(div().min_w(px(64.)).text_size(px(13.)).text_color(c(TEXT)).child(label))
        .child(button_small((id, i * 2 + 1), "+").on_click(cx.listener(move |this, _, _, cx| b(this, 1.0, cx))))
}

impl Render for ForecastEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);
        let days = state.data.overview.as_ref().map(|o| o.days()).unwrap_or_default();
        let edits = state.ext.edits.clone();
        let draft = edits.draft.clone();
        let checks = draft.check(&days);
        let diff = draft.diff(&edits.published);
        let versions = crate::forecast_edit::ForecastEdits::versions();
        let painting = state.ext.paint.is_some();

        // ---- Days ----
        let day_rows = days
            .iter()
            .take(7)
            .enumerate()
            .map(|(i, d)| {
                let o = draft.day(i).cloned().unwrap_or_default();
                let hi = o.hi.or(d.high.map(|v| v as f32));
                let lo = o.lo.or(d.low.map(|v| v as f32));
                let pop = o.pop.or(d.precip.map(|v| v as f32)).unwrap_or(0.0);
                let cond = o.condition.clone().unwrap_or_else(|| d.short_forecast.clone());
                let edited = o.hi.is_some() || o.lo.is_some() || o.pop.is_some() || o.condition.is_some();
                let open = self.open_day == Some(i);
                let (h0, l0) = (hi.unwrap_or(60.0), lo.unwrap_or(40.0));
                col()
                    .gap(px(6.))
                    .p(px(8.))
                    .rounded(px(6.))
                    .bg(c(PANEL_2))
                    .child(
                        row()
                            .id(("fe-day", i))
                            .gap(px(8.))
                            .cursor_pointer()
                            .child(div().w(px(110.)).text_size(px(13.)).text_color(c(TEXT)).child(d.name.clone()))
                            .child(muted({
                                // Tonight has only a low (and a late day only a high): name what is
                                // there instead of showing "--".
                                let temps = match (d.high, d.low) {
                                    (Some(h), Some(l)) => format!("{h:.0}° / {l:.0}°"),
                                    (Some(h), None) => format!("High {h:.0}°"),
                                    (None, Some(l)) => format!("Low {l:.0}°"),
                                    (None, None) => "No temperatures yet".to_string(),
                                };
                                format!("NWS {temps} • {} • {:.0}%", d.short_forecast, d.precip.unwrap_or(0.0))
                            }))
                            .child(div().flex_1())
                            .when(edited, |r| r.child(badge("Edited", AMBER)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_day = if this.open_day == Some(i) { None } else { Some(i) };
                                cx.notify();
                            })),
                    )
                    .when(open, |col_| {
                        col_.child(
                            row()
                                .gap(px(14.))
                                .flex_wrap()
                                .child(muted("High"))
                                .child(stepper("fe-hi", i, hi.map(|v| format!("{v:.0}°")).unwrap_or_else(|| "--".into()), cx, move |this: &mut Self, s, cx| {
                                    this.edit(cx, "Changed a high", |e| e.day_mut(i).hi = Some(h0 + s));
                                }))
                                .child(muted("Low"))
                                .child(stepper("fe-lo", i, lo.map(|v| format!("{v:.0}°")).unwrap_or_else(|| "--".into()), cx, move |this: &mut Self, s, cx| {
                                    this.edit(cx, "Changed a low", |e| e.day_mut(i).lo = Some(l0 + s));
                                }))
                                .child(muted("Rain chance"))
                                .child(stepper("fe-pop", i, format!("{pop:.0}%"), cx, move |this: &mut Self, s, cx| {
                                    this.edit(cx, "Changed a rain chance", |e| e.day_mut(i).pop = Some((pop + s * 10.0).clamp(0.0, 100.0)));
                                })),
                        )
                        .child(row().flex_wrap().gap(px(4.)).children(CONDITIONS.iter().enumerate().map(|(k, w)| {
                            let w = w.to_string();
                            let active = cond == w;
                            chip(("fe-cond", i * 100 + k), w.clone(), active).on_click(cx.listener(move |this, _, _, cx| {
                                let w = w.clone();
                                this.edit(cx, "Changed the sky wording", |e| e.day_mut(i).condition = Some(w));
                            }))
                        }).collect::<Vec<_>>()))
                        .child(button_small(("fe-reset", i), "Back to the NWS forecast").on_click(cx.listener(move |this, _, _, cx| {
                            this.edit(cx, "Reset a day", |e| e.days.retain(|d| d.index != i));
                        })))
                    })
            })
            .collect::<Vec<_>>();

        // ---- Area adjustments ----
        let adjustments = draft
            .adjustments
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let (delta, from, to, ramp) = (a.delta, a.from_hour, a.to_hour, a.ramp);
                col()
                    .gap(px(4.))
                    .p(px(8.))
                    .rounded(px(6.))
                    .bg(c(PANEL_2))
                    .child(row().gap(px(8.)).child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(a.name.clone())).child(button_small(("fe-adj-del", i), "Remove").on_click(cx.listener(move |this, _, _, cx| {
                        this.edit(cx, "Removed an adjustment", |e| {
                            if i < e.adjustments.len() {
                                e.adjustments.remove(i);
                            }
                        });
                    }))))
                    .child(
                        row()
                            .gap(px(10.))
                            .flex_wrap()
                            .child(stepper("fe-adj-d", i, format!("{delta:+.0}"), cx, move |this: &mut Self, s, cx| {
                                this.edit(cx, "Changed an adjustment", |e| {
                                    if let Some(a) = e.adjustments.get_mut(i) {
                                        a.delta += s;
                                    }
                                });
                            }))
                            .children([(0.0f32, 12.0f32, "Next 12 h"), (12.0, 24.0, "12-24 h"), (24.0, 48.0, "Tomorrow night on"), (0.0, 48.0, "All 48 h")].into_iter().enumerate().map(|(k, (f, t, l))| {
                                chip(("fe-adj-t", i * 10 + k), l, from == f && to == t).on_click(cx.listener(move |this, _, _, cx| {
                                    this.edit(cx, "Changed an adjustment's timing", |e| {
                                        if let Some(a) = e.adjustments.get_mut(i) {
                                            a.from_hour = f;
                                            a.to_hour = t;
                                        }
                                    });
                                }))
                            }).collect::<Vec<_>>())
                            .child(toggle(("fe-adj-ramp", i), "Ease in and out", ramp).on_click(cx.listener(move |this, _, _, cx| {
                                this.edit(cx, "Changed an adjustment's ramp", |e| {
                                    if let Some(a) = e.adjustments.get_mut(i) {
                                        a.ramp = !a.ramp;
                                    }
                                });
                            }))),
                    )
            })
            .collect::<Vec<_>>();
        let area_var = self.area_var;
        let area_delta = self.area_delta;

        // ---- Regions ----
        let regions = draft
            .regions
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let (hd, ld) = (r.hi_delta, r.lo_delta);
                row()
                    .gap(px(10.))
                    .flex_wrap()
                    .p(px(8.))
                    .rounded(px(6.))
                    .bg(c(PANEL_2))
                    .child(div().w(px(160.)).text_size(px(13.)).text_color(c(TEXT)).child(r.name.clone()))
                    .child(muted("High"))
                    .child(stepper("fe-rg-hi", i, format!("{hd:+.0}°"), cx, move |this: &mut Self, s, cx| {
                        this.edit(cx, "Changed a region", |e| {
                            if let Some(r) = e.regions.get_mut(i) {
                                r.hi_delta += s;
                            }
                        });
                    }))
                    .child(muted("Low"))
                    .child(stepper("fe-rg-lo", i, format!("{ld:+.0}°"), cx, move |this: &mut Self, s, cx| {
                        this.edit(cx, "Changed a region", |e| {
                            if let Some(r) = e.regions.get_mut(i) {
                                r.lo_delta += s;
                            }
                        });
                    }))
                    .child(muted(format!("Graphics: {{{{region{i}.hi}}}}, {{{{region{i}.cond}}}}")))
                    .child(button_small(("fe-rg-del", i), "Remove").on_click(cx.listener(move |this, _, _, cx| {
                        this.edit(cx, "Removed a region", |e| {
                            if i < e.regions.len() {
                                e.regions.remove(i);
                            }
                        });
                    })))
            })
            .collect::<Vec<_>>();

        let scenarios = edits
            .scenarios
            .iter()
            .enumerate()
            .map(|(i, (name, _))| {
                row()
                    .gap(px(6.))
                    .child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(name.clone()))
                    .child(button_small(("fe-sc-air", i), "Show on air").on_click(cx.listener(move |this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.demo_scenario(Some(i));
                            cx.notify();
                        });
                    })))
                    .child(button_small(("fe-sc-load", i), "Load into draft").on_click(cx.listener(move |this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            if let Some((n, set)) = s.ext.edits.scenarios.get(i).cloned() {
                                s.ext.edits.draft = set;
                                s.edits_changed(&format!("Loaded scenario {n}"));
                            }
                            cx.notify();
                        });
                    })))
                    .child(button_small(("fe-sc-del", i), "Delete").on_click(cx.listener(move |this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            if i < s.ext.edits.scenarios.len() {
                                s.ext.edits.scenarios.remove(i);
                                s.edits_changed("Deleted a scenario");
                            }
                            cx.notify();
                        });
                    })))
            })
            .collect::<Vec<_>>();

        let demo = self.app.read(cx).ext.demo_scenario.clone();
        col()
            .gap(px(14.))
            .when_some(demo, |d, name| {
                d.child(
                    row()
                        .gap(px(8.))
                        .child(div().text_size(px(13.)).text_color(c(0xE0A030)).child(format!("Scenario \"{name}\" is on air (not published)")))
                        .child(button_small("fe-sc-back", "Back to the published forecast").on_click(cx.listener(|this, _, _, cx| {
                            this.app.update(cx, |s, cx| {
                                s.demo_scenario(None);
                                cx.notify();
                            });
                        }))),
                )
            })
            .child(muted("Edits stay in a draft until you publish. Published edits change every forecast graphic, the data maps and town values."))
            .child(
                row()
                    .gap(px(8.))
                    .child(button_primary("fe-publish", if diff.is_empty() { "Published" } else { "Publish to air" }).on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.publish_forecast();
                            cx.notify();
                        });
                    })))
                    .child(button("fe-revert", "Discard draft changes").on_click(cx.listener(|this, _, _, cx| {
                        this.app.update(cx, |s, cx| {
                            s.ext.edits.draft = s.ext.edits.published.clone();
                            s.edits_changed("Discarded draft changes");
                            cx.notify();
                        });
                    })))
                    .child(button("fe-clear", "Clear all edits").on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, "Cleared the draft", |e| *e = Default::default());
                    }))),
            )
            .child(
                section(if diff.is_empty() { "Draft matches what is on air".to_string() } else { format!("Draft vs on air ({} change(s))", diff.len()) })
                    .children(diff.iter().map(|d| muted(d.clone()))),
            )
            .child(
                section(if checks.is_empty() { "Checks: nothing looks wrong".to_string() } else { format!("Checks ({})", checks.len()) })
                    .children(checks.iter().map(|t| div().text_size(px(12.)).text_color(c(AMBER)).child(t.clone()))),
            )
            .child(section("Days (click a day to edit)").gap(px(6.)).when(days.is_empty(), |d| d.child(muted("The NWS forecast has not loaded yet"))).children(day_rows))
            .child(
                section(format!("Area adjustments ({})", draft.adjustments.len()))
                    .gap(px(6.))
                    .child(muted("Change the model in part of the map: paint circles on the map, or draw an Area (Draw > Area) and apply. Maps and town values follow."))
                    .child(row().flex_wrap().gap(px(4.)).children(EditVar::ALL.iter().enumerate().map(|(i, v)| {
                        let v = *v;
                        chip(("fe-var", i), v.label(), area_var == v).on_click(cx.listener(move |this, _, _, cx| {
                            this.area_var = v;
                            cx.notify();
                        }))
                    }).collect::<Vec<_>>()))
                    .child(
                        row()
                            .gap(px(10.))
                            .child(muted("Amount"))
                            .child(stepper("fe-amt", 0, format!("{area_delta:+.0}"), cx, |this: &mut Self, s, cx| {
                                let step = if this.area_var == EditVar::Precip || this.area_var == EditVar::Cloud { 10.0 } else { 1.0 };
                                this.area_delta += s * step;
                                cx.notify();
                            }))
                            .child(button_small("fe-from-area", "Apply to drawn area").on_click(cx.listener(move |this, _, _, cx| {
                                let (v, d) = (this.area_var, this.area_delta);
                                this.app.update(cx, |s, cx| {
                                    s.adjustment_from_drawn_area(v, d);
                                    cx.notify();
                                });
                            })))
                            .child(button_small("fe-paint", if painting { "Stop painting" } else { "Paint on the map" }).on_click(cx.listener(move |this, _, _, cx| {
                                let (v, d) = (this.area_var, this.area_delta);
                                this.app.update(cx, |s, cx| {
                                    s.ext.paint = if s.ext.paint.is_some() { None } else { Some((v, d, 10.0)) };
                                    if s.ext.paint.is_some() {
                                        s.modal = crate::app::Modal::None;
                                        s.toast(crate::app::ToastKind::Info, "Click the map to paint 10-mile adjustments; open the Forecast editor again to stop");
                                    }
                                    cx.notify();
                                });
                            }))),
                    )
                    .children(adjustments),
            )
            .child(
                section(format!("Regions ({})", draft.regions.len()))
                    .gap(px(6.))
                    .child(muted("Your own forecast areas (school districts, valleys, lake shores): draw an Area, name it, then set its values."))
                    .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.region_name))).child(button_small("fe-region", "Make region from drawn area").on_click(cx.listener(|this, _, window, cx| {
                        let name = this.region_name.read(cx).value().trim().to_string();
                        let name = if name.is_empty() { "Region".to_string() } else { name };
                        let ok = this.app.update(cx, |s, cx| {
                            let ok = s.region_from_drawn_area(name);
                            cx.notify();
                            ok
                        });
                        if ok {
                            this.region_name.update(cx, |st, cx| st.set_value(String::new(), window, cx));
                        }
                    }))))
                    .children(regions),
            )
            .child(
                section("Scenarios")
                    .gap(px(6.))
                    .child(muted("Save the draft as a scenario (\"warmer\", \"colder\") to switch between them while the forecast evolves."))
                    .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.scenario_name))).child(button_small("fe-sc-save", "Save draft as scenario").on_click(cx.listener(|this, _, window, cx| {
                        let name = this.scenario_name.read(cx).value().trim().to_string();
                        this.app.update(cx, |s, cx| {
                            let n = if name.is_empty() { format!("Scenario {}", s.ext.edits.scenarios.len() + 1) } else { name };
                            let set = s.ext.edits.draft.clone();
                            s.ext.edits.scenarios.retain(|(x, _)| *x != n);
                            s.ext.edits.scenarios.push((n.clone(), set));
                            s.edits_changed(&format!("Saved scenario {n}"));
                            cx.notify();
                        });
                        this.scenario_name.update(cx, |st, cx| st.set_value(String::new(), window, cx));
                    }))))
                    .children(scenarios),
            )
            .child(
                section(format!("Published versions ({})", versions.len()))
                    .gap(px(4.))
                    .children(versions.into_iter().take(8).enumerate().map(|(i, (name, path))| {
                        row()
                            .gap(px(6.))
                            .child(div().flex_1().text_size(px(12.)).text_color(c(TEXT)).child(name.replace('_', " ")))
                            .child(button_small(("fe-ver", i), "Load into draft").on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(set) = crate::forecast_edit::ForecastEdits::load_version(&path) {
                                    this.app.update(cx, |s, cx| {
                                        s.ext.edits.draft = set;
                                        s.edits_changed("Loaded a published version into the draft");
                                        cx.notify();
                                    });
                                }
                            })))
                    }).collect::<Vec<_>>()),
            )
            .child(
                section("Change history")
                    .gap(px(2.))
                    .children(edits.history.iter().rev().take(12).map(|(t, what)| {
                        let when = chrono::DateTime::from_timestamp(*t, 0).map(|d| d.with_timezone(&chrono::Local).format("%b %-d %-I:%M %p").to_string()).unwrap_or_default();
                        muted(format!("{when}  {what}"))
                    }).collect::<Vec<_>>()),
            )
            .child(div().h(px(1.)).bg(c(BORDER)))
            .child(row().gap(px(6.)).child(div().size(px(8.)).rounded(px(4.)).bg(c(if diff.is_empty() { GREEN } else { RED }))).child(div().text_size(px(12.)).text_color(c(MUTED)).child(if diff.is_empty() { "On air matches the draft" } else { "Unpublished changes" })))
    }
}
