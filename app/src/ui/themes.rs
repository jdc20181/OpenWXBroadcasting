//! Map themes (modal): the custom broadcast basemap designer. Base map, terrain, water, roads and
//! labels each get colour controls; county / state lines, town labels, map symbols and the legend
//! are styled here too. Saving keeps every version; "Preview" puts the theme on the live map and
//! steps through zoom levels.

use gpui_kit::{
    component::input::{Input, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::{AppState, ToastKind},
    maptheme::{self, Adjust, Symbol, Theme, BASES},
    ui::{button_small, c, chip, col, heading, muted, row, slider, toggle, BORDER, TEXT},
};

pub struct ThemesView {
    app: Entity<AppState>,
    theme: Theme,
    dirty: bool,
    name: Entity<InputState>,
    colors: Vec<(&'static str, Entity<InputState>)>,
    _subscriptions: Vec<Subscription>,
}

/// Colour fields edited as text: (key, label).
const COLOR_FIELDS: &[(&str, &str)] = &[
    ("background", "Background"),
    ("county", "County lines"),
    ("state", "State lines"),
    ("label", "Town labels"),
    ("label_font", "Label font (empty = brand)"),
    ("town_fill", "Town symbol fill"),
    ("town_outline", "Town symbol outline"),
    ("home_fill", "Home symbol fill"),
    ("home_outline", "Home symbol outline"),
    ("legend_bg", "Legend background"),
    ("legend_text", "Legend text"),
];

fn get_color(t: &Theme, key: &str) -> String {
    match key {
        "background" => t.background.clone(),
        "county" => t.county_color.clone(),
        "state" => t.state_color.clone(),
        "label" => t.label_color.clone(),
        "label_font" => t.label_font.clone(),
        "town_fill" => t.town_symbol.fill.clone(),
        "town_outline" => t.town_symbol.outline.clone(),
        "home_fill" => t.home_symbol.fill.clone(),
        "home_outline" => t.home_symbol.outline.clone(),
        "legend_bg" => t.legend_background.clone(),
        "legend_text" => t.legend_text.clone(),
        _ => String::new(),
    }
}

fn set_color(t: &mut Theme, key: &str, v: String) {
    match key {
        "background" => t.background = v,
        "county" => t.county_color = v,
        "state" => t.state_color = v,
        "label" => t.label_color = v,
        "label_font" => t.label_font = v,
        "town_fill" => t.town_symbol.fill = v,
        "town_outline" => t.town_symbol.outline = v,
        "home_fill" => t.home_symbol.fill = v,
        "home_outline" => t.home_symbol.outline = v,
        "legend_bg" => t.legend_background = v,
        "legend_text" => t.legend_text = v,
        _ => {}
    }
}

/// Stable index per block, for element ids.
fn slot(key: &str) -> usize {
    match key {
        "base" => 0,
        "terrain" => 1,
        "water" => 2,
        "roads" => 3,
        "labels" => 4,
        "town" => 5,
        "home" => 6,
        _ => 7,
    }
}

impl ThemesView {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Theme name"));
        let colors = COLOR_FIELDS.iter().map(|(k, label)| (*k, cx.new(|cx| InputState::new(window, cx).placeholder(label.to_string())))).collect();
        let mut v = Self { app: app.clone(), theme: Theme::default(), dirty: false, name, colors, _subscriptions: vec![cx.observe(&app, |_, _, cx| cx.notify())] };
        let first = maptheme::list().into_iter().next().unwrap_or_default();
        v.load(first, window, cx);
        v
    }

    fn load(&mut self, t: Theme, window: &mut Window, cx: &mut Context<Self>) {
        let n = t.name.clone();
        self.name.update(cx, |s, cx| s.set_value(n, window, cx));
        for (k, input) in &self.colors {
            let v = get_color(&t, k);
            input.update(cx, |s, cx| s.set_value(v, window, cx));
        }
        self.theme = t;
        self.dirty = false;
        cx.notify();
    }

    /// Pull the typed fields into the theme.
    fn collect(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).value().trim().to_string();
        if !name.is_empty() {
            self.theme.name = name;
        }
        let values: Vec<(&'static str, String)> = self.colors.iter().map(|(k, i)| (*k, i.read(cx).value().trim().to_string())).collect();
        for (k, v) in values {
            set_color(&mut self.theme, k, v);
        }
    }

    fn change(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Theme)) {
        f(&mut self.theme);
        self.dirty = true;
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) -> bool {
        self.collect(cx);
        match maptheme::save(self.theme.clone()) {
            Ok(t) => {
                let v = t.version;
                self.theme = t;
                self.dirty = false;
                self.toast(cx, ToastKind::Success, format!("Theme saved (version {v})"));
                // The live map picks up the saved theme if it's in use.
                let name = self.theme.name.clone();
                self.app.update(cx, |s, cx| {
                    if s.live.theme == name {
                        s.update_live(|l| l.theme = name.clone());
                    }
                    cx.notify();
                });
                true
            }
            Err(e) => {
                self.toast(cx, ToastKind::Error, e.to_string());
                false
            }
        }
    }

    fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        self.app.update(cx, |s, cx| {
            s.toast(kind, msg);
            cx.notify();
        });
    }

    fn adjust_block(&self, key: &'static str, title: &str, a: &Adjust, cx: &mut Context<Self>) -> Div {
        fn pick<'a>(t: &'a mut Theme, key: &str) -> &'a mut Adjust {
            match key {
                "terrain" => &mut t.terrain,
                "water" => &mut t.water,
                "roads" => &mut t.roads,
                "labels" => &mut t.labels,
                _ => &mut t.base_adjust,
            }
        }
        let line = |label: &str, value: f32, min: f32, max: f32, id: &'static str, cx: &mut Context<Self>, set: fn(&mut Adjust, f32)| {
            row()
                .gap(px(8.))
                .child(div().w(px(110.)).text_size(px(12.)).text_color(c(TEXT)).child(label.to_string()))
                .child(div().flex_1().child(slider((id, slot(key)), value, min, max, cx, move |this: &mut Self, v, _, cx| this.change(cx, |t| set(pick(t, key), v)))))
                .child(div().w(px(46.)).text_size(px(12.)).text_color(c(TEXT)).child(format!("{value:.2}")))
        };
        col()
            .gap(px(4.))
            .p(px(10.))
            .rounded(px(8.))
            .border_1()
            .border_color(c(BORDER))
            .child(toggle(("theme-on", slot(key)), title.to_string(), a.on).on_click(cx.listener(move |this, _, _, cx| this.change(cx, |t| {
                let x = pick(t, key);
                x.on = !x.on;
            }))))
            .when(a.on, |d| {
                d.child(line("Opacity", a.opacity, 0.0, 1.0, "th-op", cx, |a, v| a.opacity = v))
                    .child(line("Darkest", a.brightness_min, 0.0, 1.0, "th-bmin", cx, |a, v| a.brightness_min = v))
                    .child(line("Brightest", a.brightness_max, 0.0, 1.0, "th-bmax", cx, |a, v| a.brightness_max = v))
                    .child(line("Contrast", a.contrast, -1.0, 0.95, "th-con", cx, |a, v| a.contrast = v))
                    .child(line("Saturation", a.saturation, -1.0, 1.0, "th-sat", cx, |a, v| a.saturation = v))
                    .child(line("Hue shift", a.hue, -180.0, 180.0, "th-hue", cx, |a, v| a.hue = v))
            })
    }

    fn symbol_block(&self, key: &'static str, title: &str, s: &Symbol, cx: &mut Context<Self>) -> Div {
        fn pick<'a>(t: &'a mut Theme, key: &str) -> &'a mut Symbol {
            if key == "home" { &mut t.home_symbol } else { &mut t.town_symbol }
        }
        row()
            .gap(px(4.))
            .child(div().w(px(110.)).text_size(px(12.)).text_color(c(TEXT)).child(title.to_string()))
            .children(["circle", "square", "diamond", "triangle", "star"].into_iter().enumerate().map(|(i, shape)| {
                chip(("sym-shape", slot(key) * 10 + i), shape, s.shape == shape).on_click(cx.listener(move |this, _, _, cx| this.change(cx, |t| pick(t, key).shape = shape.into())))
            }).collect::<Vec<_>>())
            .child(div().w(px(160.)).child(slider(("sym-size", slot(key)), s.size, 2.0, 16.0, cx, move |this: &mut Self, v, _, cx| this.change(cx, |t| pick(t, key).size = v))))
            .child(muted(format!("{:.0} px", s.size)))
    }
}

impl Render for ThemesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let list = maptheme::list();
        let t = self.theme.clone();
        let live_theme = self.app.read(cx).live.theme.clone();
        let versions = maptheme::versions(&t.name);

        let sidebar = col()
            .w(px(220.))
            .gap(px(4.))
            .child(button_small("theme-new", "+ New theme").on_click(cx.listener(|this, _, window, cx| {
                let t = Theme { name: format!("Theme {}", maptheme::list().len() + 1), ..Default::default() };
                this.load(t, window, cx);
                this.dirty = true;
            })))
            .child(button_small("theme-dup", "Duplicate").on_click(cx.listener(|this, _, window, cx| {
                this.collect(cx);
                let mut t = this.theme.clone();
                t.name = format!("{} copy", t.name);
                t.version = 1;
                this.load(t, window, cx);
                this.dirty = true;
            })))
            .children(list.iter().enumerate().map(|(i, x)| {
                let th = x.clone();
                chip(("theme-item", i), format!("{}{} (v{})", if x.name == live_theme { "● " } else { "" }, x.name, x.version), x.name == t.name).on_click(cx.listener(move |this, _, window, cx| this.load(th.clone(), window, cx)))
            }).collect::<Vec<_>>())
            .child(button_small("theme-none", "Use standard basemap").on_click(cx.listener(|this, _, _, cx| {
                this.app.update(cx, |s, cx| {
                    s.update_live(|l| l.theme.clear());
                    cx.notify();
                });
            })));

        let colors = self.colors.iter().map(|(_, input)| div().w(px(250.)).child(Input::new(input))).collect::<Vec<_>>();

        let editor = col()
            .flex_1()
            .min_w_0()
            .gap(px(10.))
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.name))).child(button_small("theme-save", if self.dirty { "Save •" } else { "Save" }).on_click(cx.listener(|this, _, _, cx| {
                this.save(cx);
            }))).child(button_small("theme-use", "Use on the map").on_click(cx.listener(|this, _, _, cx| {
                if this.save(cx) {
                    let name = this.theme.name.clone();
                    this.app.update(cx, |s, cx| {
                        s.update_live(|l| l.theme = name);
                        cx.notify();
                    });
                }
            }))).child(button_small("theme-del", "Delete").on_click(cx.listener(|this, _, window, cx| {
                maptheme::delete(&this.theme.name);
                let next = maptheme::list().into_iter().next().unwrap_or_default();
                this.load(next, window, cx);
            }))))
            .child(row().gap(px(4.)).child(muted("Preview at")).children([(5.0f64, "State"), (7.0, "Region"), (9.0, "County"), (11.0, "City")].into_iter().enumerate().map(|(i, (z, label))| {
                button_small(("theme-zoom", i), format!("{label} (z{z:.0})")).on_click(cx.listener(move |this, _, _, cx| {
                    if this.save(cx) {
                        let name = this.theme.name.clone();
                        this.app.update(cx, |s, cx| {
                            s.update_live(|l| {
                                l.theme = name;
                                l.view.zoom = z;
                            });
                            cx.notify();
                        });
                    }
                }))
            }).collect::<Vec<_>>()))
            .child(heading("Base map"))
            .child(row().gap(px(4.)).flex_wrap().children(BASES.iter().enumerate().map(|(i, (k, label))| {
                let k: &'static str = k;
                chip(("theme-base", i), *label, t.base == k).on_click(cx.listener(move |this, _, _, cx| this.change(cx, |t| t.base = k.into())))
            }).collect::<Vec<_>>()))
            .child(self.adjust_block("base", "Base map colours", &t.base_adjust, cx))
            .child(self.adjust_block("terrain", "Terrain (hillshade)", &t.terrain, cx))
            .child(self.adjust_block("water", "Rivers and lakes", &t.water, cx))
            .child(self.adjust_block("roads", "Roads", &t.roads, cx))
            .child(self.adjust_block("labels", "Place labels (map)", &t.labels, cx))
            .child(row().gap(px(4.)).child(muted("Map labels")).children([("dark", "For dark maps"), ("light", "For light maps")].into_iter().enumerate().map(|(i, (k, label))| {
                chip(("theme-lstyle", i), label, t.labels_style == k).on_click(cx.listener(move |this, _, _, cx| this.change(cx, |t| t.labels_style = k.into())))
            }).collect::<Vec<_>>()))
            .child(heading("Colours and fonts"))
            .child(muted("Colours as #RRGGBB, #RRGGBBAA or rgba(r,g,b,a). Saved with the theme."))
            .child(row().gap(px(6.)).flex_wrap().children(colors))
            .child(row().gap(px(8.)).child(div().w(px(110.)).text_size(px(12.)).text_color(c(TEXT)).child("County width")).child(div().w(px(200.)).child(slider("th-cw", t.county_width, 0.3, 4.0, cx, |this: &mut Self, v, _, cx| this.change(cx, |t| t.county_width = v)))).child(div().w(px(110.)).text_size(px(12.)).text_color(c(TEXT)).child("State width")).child(div().w(px(200.)).child(slider("th-sw", t.state_width, 0.5, 6.0, cx, |this: &mut Self, v, _, cx| this.change(cx, |t| t.state_width = v)))))
            .child(row().gap(px(8.)).child(div().w(px(110.)).text_size(px(12.)).text_color(c(TEXT)).child("Label size")).child(div().w(px(200.)).child(slider("th-ls", t.label_size, 12.0, 40.0, cx, |this: &mut Self, v, _, cx| this.change(cx, |t| t.label_size = v)))).child(muted(format!("{:.0} px", t.label_size))).child(toggle("th-caps", "Capitals", t.label_caps).on_click(cx.listener(|this, _, _, cx| this.change(cx, |t| t.label_caps = !t.label_caps)))))
            .child(heading("Map symbols"))
            .child(self.symbol_block("town", "Towns", &t.town_symbol, cx))
            .child(self.symbol_block("home", "Home", &t.home_symbol, cx))
            .child(heading("Legend"))
            .child(row().gap(px(4.)).children([("bottom-left", "Bottom left"), ("bottom-right", "Bottom right"), ("top-right", "Top right"), ("hidden", "Hidden")].into_iter().enumerate().map(|(i, (k, label))| {
                chip(("theme-legend", i), label, t.legend_position == k).on_click(cx.listener(move |this, _, _, cx| this.change(cx, |t| t.legend_position = k.into())))
            }).collect::<Vec<_>>()))
            .child(heading("Versions"))
            .child(muted(format!("Current: version {}{}", t.version, if self.dirty { " (unsaved changes)" } else { "" })))
            .children(versions.into_iter().take(10).enumerate().map(|(i, (v, when))| {
                let name = t.name.clone();
                row().gap(px(8.)).child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(format!("Version {v} • {}", chrono::DateTime::from_timestamp(when, 0).map(|d| d.with_timezone(&chrono::Local).format("%b %-d %-I:%M %p").to_string()).unwrap_or_default()))).child(button_small(("theme-restore", i), "Restore").on_click(cx.listener(move |this, _, window, cx| {
                    match maptheme::restore(&name, v) {
                        Ok(t) => {
                            this.load(t, window, cx);
                            this.toast(cx, ToastKind::Success, format!("Version {v} restored as a new version"));
                        }
                        Err(e) => this.toast(cx, ToastKind::Error, e.to_string()),
                    }
                })))
            }).collect::<Vec<_>>());

        row().gap(px(14.)).items_start().child(sidebar).child(editor)
    }
}
