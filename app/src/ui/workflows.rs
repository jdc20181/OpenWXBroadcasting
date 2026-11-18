//! Workflow automation designer (modal): workflows as node graphs on a canvas. Drag nodes to
//! arrange them, connect yes / no (or error) paths in the inspector, then save (each save is a
//! new version), run by hand, dry-run, watch runs step by step, replay a run and read metrics.

use gpui_kit::{
    component::input::{Input, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    app::{AppState, ToastKind},
    ui::{badge, button_small, c, chip, col, heading, muted, row, toggle, BORDER, TEXT},
    workflow::{self, Node, Workflow, NODE_H, NODE_W, OPS},
};

pub struct WorkflowsView {
    app: Entity<AppState>,
    wf: Option<Workflow>,
    dirty: bool,
    node: Option<u32>,
    drag: Option<(u32, Point<Pixels>, (f32, f32))>,
    name: Entity<InputState>,
    arg: Entity<InputState>,
    open_run: Option<u64>,
    _subscriptions: Vec<Subscription>,
}

fn kind_color(kind: &str) -> u32 {
    match kind {
        "trigger" => 0x4F8FD6,
        "condition" => 0xE0A030,
        "wait" => 0x8A93A8,
        _ => 0x3DD68C,
    }
}

impl WorkflowsView {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Workflow name"));
        let arg = cx.new(|cx| InputState::new(window, cx).placeholder("Setting for the selected step"));
        Self { app: app.clone(), wf: None, dirty: false, node: None, drag: None, name, arg, open_run: None, _subscriptions: vec![cx.observe(&app, |_, _, cx| cx.notify())] }
    }

    fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        self.app.update(cx, |s, cx| {
            s.toast(kind, msg);
            cx.notify();
        });
    }

    fn open(&mut self, wf: Workflow, window: &mut Window, cx: &mut Context<Self>) {
        let n = wf.name.clone();
        self.name.update(cx, |s, cx| s.set_value(n, window, cx));
        self.wf = Some(wf);
        self.node = None;
        self.dirty = false;
        cx.notify();
    }

    fn select(&mut self, id: u32, window: &mut Window, cx: &mut Context<Self>) {
        self.node = Some(id);
        let arg = self.wf.as_ref().and_then(|w| w.node(id)).map(|n| n.arg.clone()).unwrap_or_default();
        self.arg.update(cx, |s, cx| s.set_value(arg, window, cx));
        cx.notify();
    }

    fn edit(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Workflow)) {
        if let Some(wf) = self.wf.as_mut() {
            f(wf);
            self.dirty = true;
            cx.notify();
        }
    }

    fn edit_node(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Node)) {
        let Some(id) = self.node else { return };
        self.edit(cx, |wf| {
            if let Some(n) = wf.nodes.iter_mut().find(|n| n.id == id) {
                f(n);
            }
        });
    }

    fn add_node(&mut self, kind: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let op = OPS.iter().find(|(k, _, _, _)| *k == kind).map(|(_, o, _, _)| *o).unwrap_or("manual");
        let mut new_id = 0;
        let selected = self.node;
        self.edit(cx, |wf| {
            let id = wf.next_id();
            new_id = id;
            // Below the selected node (and linked from it), or in the next free row.
            let (x, y) = selected
                .and_then(|s| wf.node(s))
                .map(|n| (n.x, n.y + NODE_H + 40.0))
                .unwrap_or((30.0, 20.0 + wf.nodes.len() as f32 * (NODE_H + 30.0) % 400.0));
            wf.nodes.push(Node { id, kind: kind.into(), op: op.into(), x, y, retries: if kind == "action" { 1 } else { 0 }, ..Default::default() });
            if let Some(s) = selected.and_then(|s| wf.nodes.iter_mut().find(|n| n.id == s)) {
                if s.next.is_none() {
                    s.next = Some(id);
                }
            }
        });
        if new_id > 0 {
            self.select(new_id, window, cx);
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(mut wf) = self.wf.clone() else { return };
        let name = self.name.read(cx).value().trim().to_string();
        if !name.is_empty() {
            wf.name = name;
        }
        if wf.name.is_empty() {
            wf.name = "Untitled workflow".into();
        }
        match workflow::save(wf) {
            Ok(saved) => {
                let v = saved.version;
                self.wf = Some(saved);
                self.dirty = false;
                self.toast(cx, ToastKind::Success, format!("Saved (version {v})"));
            }
            Err(e) => self.toast(cx, ToastKind::Error, e.to_string()),
        }
        cx.notify();
    }
}

impl Render for WorkflowsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let list = workflow::list();
        let current_id = self.wf.as_ref().map(|w| w.id.clone()).unwrap_or_default();
        let sidebar = col()
            .w(px(220.))
            .gap(px(4.))
            .child(button_small("wf-new", "+ New workflow").on_click(cx.listener(|this, _, window, cx| {
                let wf = Workflow {
                    name: "New workflow".into(),
                    enabled: false,
                    nodes: vec![Node { id: 1, kind: "trigger".into(), op: "manual".into(), x: 30.0, y: 20.0, ..Default::default() }],
                    ..Default::default()
                };
                this.open(wf, window, cx);
            })))
            .children(list.iter().enumerate().map(|(i, w)| {
                let wf = w.clone();
                chip(("wf-item", i), format!("{}{} (v{})", if w.enabled { "● " } else { "○ " }, w.name, w.version), w.id == current_id).on_click(cx.listener(move |this, _, window, cx| {
                    this.open(wf.clone(), window, cx);
                }))
            }).collect::<Vec<_>>())
            .when(list.is_empty(), |d| d.child(muted("No workflows yet. Example: Warning issued (Tornado Warning) → If show running → Take graphic \"Tornado Warning\" → Wait 60 → Live map.")));

        let Some(wf) = self.wf.clone() else {
            return row().gap(px(14.)).items_start().child(sidebar).child(div().flex_1().child(muted("Pick a workflow or make a new one."))).into_any_element();
        };

        // Connectors (elbows from the bottom of each node to the top of its targets).
        let mut links: Vec<((f32, f32), (f32, f32), u32)> = Vec::new();
        for n in &wf.nodes {
            for (target, color, dx) in [(n.next, 0x3DD68C, 0.35), (n.alt, 0xE5383B, 0.7)] {
                if let Some(t) = target.and_then(|t| wf.node(t)) {
                    links.push(((n.x + NODE_W * dx, n.y + NODE_H), (t.x + NODE_W * 0.5, t.y), color));
                }
            }
        }
        let connectors = canvas(
            move |b, _, _| b,
            move |_, bounds, window, _| {
                let o = bounds.origin;
                let q = |x: f32, y: f32, w: f32, h: f32| Bounds::new(point(o.x + px(x), o.y + px(y)), size(px(w.max(2.0)), px(h.max(2.0))));
                for ((ax, ay), (bx, by), color) in &links {
                    let mid = if by > ay { (ay + by) / 2.0 } else { ay + 16.0 };
                    let col = rgb(*color);
                    window.paint_quad(fill(q(ax - 1.0, *ay, 2.0, mid - ay), col));
                    window.paint_quad(fill(q(ax.min(*bx) - 1.0, mid - 1.0, (ax - bx).abs() + 2.0, 2.0), col));
                    if *by >= mid {
                        window.paint_quad(fill(q(bx - 1.0, mid, 2.0, by - mid), col));
                    } else {
                        window.paint_quad(fill(q(bx - 1.0, *by - 8.0, 2.0, mid - by + 8.0), col));
                    }
                    window.paint_quad(fill(q(bx - 4.0, by - 8.0, 8.0, 8.0), col));
                }
            },
        )
        .absolute()
        .size_full();

        let selected = self.node;
        let nodes = wf.nodes.iter().enumerate().map(|(i, n)| {
            let id = n.id;
            let color = kind_color(&n.kind);
            col()
                .id(("wf-node", i))
                .absolute()
                .left(px(n.x))
                .top(px(n.y))
                .w(px(NODE_W))
                .h(px(NODE_H))
                .p(px(8.))
                .gap(px(2.))
                .rounded(px(8.))
                .bg(c(0x141A26))
                .border_2()
                .border_color(c(if selected == Some(id) { 0xFFFFFF } else { color }))
                .cursor_pointer()
                .child(div().text_size(px(10.)).font_weight(FontWeight::BOLD).text_color(c(color)).child(n.kind.to_uppercase()))
                .child(div().text_size(px(13.)).text_color(c(TEXT)).overflow_hidden().max_h(px(36.)).child(n.label()))
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    let orig = this.wf.as_ref().and_then(|w| w.node(id)).map(|n| (n.x, n.y)).unwrap_or_default();
                    this.drag = Some((id, e.position, orig));
                    this.select(id, window, cx);
                    cx.stop_propagation();
                }))
        }).collect::<Vec<_>>();

        let height = wf.nodes.iter().map(|n| n.y + NODE_H + 40.0).fold(420.0f32, f32::max);
        let board = div()
            .id("wf-board")
            .relative()
            .w_full()
            .h(px(height.min(900.0)))
            .overflow_hidden()
            .rounded(px(8.))
            .bg(c(0x0A0E16))
            .border_1()
            .border_color(c(BORDER))
            .child(connectors)
            .children(nodes)
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if e.pressed_button != Some(MouseButton::Left) {
                    this.drag = None;
                    return;
                }
                let Some((id, start, (ox, oy))) = this.drag else { return };
                let dx = f32::from(e.position.x - start.x);
                let dy = f32::from(e.position.y - start.y);
                let snap = |v: f32| ((v / 10.0).round() * 10.0).max(0.0);
                this.edit(cx, |wf| {
                    if let Some(n) = wf.nodes.iter_mut().find(|n| n.id == id) {
                        n.x = snap(ox + dx);
                        n.y = snap(oy + dy);
                    }
                });
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.drag = None;
                cx.notify();
            }));

        // Inspector for the selected node.
        let inspector = selected.and_then(|id| wf.node(id).cloned()).map(|n| {
            let others: Vec<(u32, String)> = wf.nodes.iter().filter(|o| o.id != n.id && o.kind != "trigger").map(|o| (o.id, o.label())).collect();
            let hint = OPS.iter().find(|(k, o, _, _)| *k == n.kind && *o == n.op).map(|(_, _, _, h)| *h).unwrap_or("");
            let alt_label = if n.kind == "condition" { "If no →" } else if n.kind == "action" { "On error →" } else { "" };
            col()
                .gap(px(6.))
                .p(px(10.))
                .rounded(px(8.))
                .border_1()
                .border_color(c(BORDER))
                .child(row().gap(px(4.)).flex_wrap().children(OPS.iter().filter(|(k, _, _, _)| *k == n.kind).enumerate().map(|(j, (_, op, label, _))| {
                    let op = *op;
                    chip(("wf-op", j), *label, n.op == op).on_click(cx.listener(move |this, _, _, cx| this.edit_node(cx, |n| n.op = op.into())))
                }).collect::<Vec<_>>()))
                .when(!hint.is_empty(), |d| {
                    d.child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.arg))).child(button_small("wf-arg", "Apply").on_click(cx.listener(|this, _, _, cx| {
                        let v = this.arg.read(cx).value().trim().to_string();
                        this.edit_node(cx, |n| n.arg = v);
                    }))))
                    .child(muted(hint.to_string()))
                })
                .map(|d| {
                    d.child(row().gap(px(4.)).flex_wrap().child(muted(if n.kind == "condition" { "If yes →" } else { "Then →" })).child(chip("wf-next-none", "end", n.next.is_none()).on_click(cx.listener(|this, _, _, cx| this.edit_node(cx, |n| n.next = None)))).children(others.iter().enumerate().map(|(j, (oid, l))| {
                        let oid = *oid;
                        chip(("wf-next", j), l.chars().take(28).collect::<String>(), n.next == Some(oid)).on_click(cx.listener(move |this, _, _, cx| this.edit_node(cx, |n| n.next = Some(oid))))
                    }).collect::<Vec<_>>()))
                })
                .when(!alt_label.is_empty(), |d| {
                    d.child(row().gap(px(4.)).flex_wrap().child(muted(alt_label)).child(chip("wf-alt-none", "end", n.alt.is_none()).on_click(cx.listener(|this, _, _, cx| this.edit_node(cx, |n| n.alt = None)))).children(others.iter().enumerate().map(|(j, (oid, l))| {
                        let oid = *oid;
                        chip(("wf-alt", j), l.chars().take(28).collect::<String>(), n.alt == Some(oid)).on_click(cx.listener(move |this, _, _, cx| this.edit_node(cx, |n| n.alt = Some(oid))))
                    }).collect::<Vec<_>>()))
                })
                .when(n.kind == "action", |d| {
                    d.child(row().gap(px(4.)).child(muted("Retries")).children([0u32, 1, 3, 5].into_iter().enumerate().map(|(j, r)| {
                        chip(("wf-retry", j), r.to_string(), n.retries == r).on_click(cx.listener(move |this, _, _, cx| this.edit_node(cx, |n| n.retries = r)))
                    }).collect::<Vec<_>>()))
                })
                .child(button_small("wf-node-del", "Delete step").on_click(cx.listener(move |this, _, _, cx| {
                    let id = n.id;
                    this.node = None;
                    this.edit(cx, |wf| {
                        wf.nodes.retain(|x| x.id != id);
                        for x in wf.nodes.iter_mut() {
                            if x.next == Some(id) {
                                x.next = None;
                            }
                            if x.alt == Some(id) {
                                x.alt = None;
                            }
                        }
                    });
                })))
        });

        // Metrics, versions, runs.
        let m = workflow::metrics(&wf.id);
        let avg = if m.runs > 0 { m.total_ms / m.runs as f32 } else { 0.0 };
        let mut slow: Vec<(u32, f32)> = m.nodes.iter().map(|(id, (n, ms))| (*id, ms / (*n).max(1) as f32)).collect();
        slow.sort_by(|a, b| b.1.total_cmp(&a.1));
        let versions = if wf.id.is_empty() { Vec::new() } else { workflow::versions(&wf.id) };
        let runs: Vec<workflow::Run> = workflow::runs().into_iter().filter(|r| r.workflow == wf.id || (r.workflow.starts_with("replay-") && r.name.starts_with(&wf.name))).collect();
        let open_run = self.open_run;

        let body = col()
            .flex_1()
            .min_w_0()
            .gap(px(10.))
            .child(row().gap(px(6.)).child(div().flex_1().child(Input::new(&self.name))).child(toggle("wf-enabled", "On", wf.enabled).on_click(cx.listener(|this, _, _, cx| this.edit(cx, |w| w.enabled = !w.enabled)))).child(button_small("wf-save", if self.dirty { "Save •" } else { "Save" }).on_click(cx.listener(|this, _, _, cx| this.save(cx)))).child(button_small("wf-run", "Run now").on_click(cx.listener(|this, _, _, cx| {
                if this.dirty {
                    this.save(cx);
                }
                if let Some(w) = &this.wf {
                    workflow::run_now(&w.id, false);
                }
                cx.notify();
            }))).child(button_small("wf-dry", "Dry run").on_click(cx.listener(|this, _, _, cx| {
                if this.dirty {
                    this.save(cx);
                }
                if let Some(w) = &this.wf {
                    workflow::run_now(&w.id, true);
                }
                cx.notify();
            }))).child(button_small("wf-del", "Delete").on_click(cx.listener(|this, _, _, cx| {
                if let Some(w) = this.wf.take() {
                    workflow::delete(&w.id);
                }
                cx.notify();
            }))))
            .child(row().gap(px(6.)).child(muted("Add step:")).children([("trigger", "+ Trigger"), ("condition", "+ Condition"), ("wait", "+ Wait"), ("action", "+ Action")].into_iter().enumerate().map(|(i, (k, t))| {
                button_small(("wf-add", i), t).on_click(cx.listener(move |this, _, window, cx| this.add_node(k, window, cx)))
            }).collect::<Vec<_>>()).child(div().flex_1()).child(badge("THEN", 0x3DD68C)).child(badge("NO / ERROR", 0xE5383B)))
            .child(board)
            .children(inspector)
            .child(heading("Performance"))
            .child(muted(if m.runs == 0 { "No runs yet this session.".to_string() } else { format!("{} runs • {} failed • {:.0} ms average • last {}", m.runs, m.failures, avg, chrono::DateTime::from_timestamp(m.last, 0).map(|d| d.with_timezone(&chrono::Local).format("%-I:%M %p").to_string()).unwrap_or_default()) }))
            .children(slow.into_iter().take(3).map(|(id, ms)| muted(format!("  {} • {:.1} ms per run", wf.node(id).map(|n| n.label()).unwrap_or_else(|| format!("step {id}")), ms))).collect::<Vec<_>>())
            .child(heading("Runs"))
            .when(runs.is_empty(), |d| d.child(muted("Runs appear here with every step and its result.")))
            .children(runs.into_iter().take(20).enumerate().map(|(i, r)| {
                let when = chrono::DateTime::from_timestamp(r.started, 0).map(|d| d.with_timezone(&chrono::Local).format("%b %-d %-I:%M:%S %p").to_string()).unwrap_or_default();
                let color = match r.status.as_str() {
                    "done" => 0x3DD68C,
                    "failed" => 0xE5383B,
                    "running" => 0x4F8FD6,
                    _ => 0x8A93A8,
                };
                let rid = r.id;
                let expanded = open_run == Some(rid);
                col()
                    .gap(px(2.))
                    .py(px(4.))
                    .border_b_1()
                    .border_color(c(BORDER))
                    .child(row().gap(px(8.)).child(badge(r.status.to_uppercase(), color)).child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(format!("{when} • {} • v{} • {} steps", r.trigger, r.version, r.steps.len()))).child(button_small(("wf-run-open", i), if expanded { "Hide" } else { "Steps" }).on_click(cx.listener(move |this, _, _, cx| {
                        this.open_run = if this.open_run == Some(rid) { None } else { Some(rid) };
                        cx.notify();
                    }))).when(r.finished.is_some() && !r.dry, |d| d.child(button_small(("wf-replay", i), "Replay").on_click(cx.listener(move |this, _, _, cx| {
                        workflow::replay(rid);
                        this.toast(cx, ToastKind::Info, "Replaying the run's actions");
                    })))))
                    .when(expanded, |d| d.children(r.steps.iter().map(|s| muted(format!("    {} → {}{} ({:.1} ms)", s.label, s.result, if s.detail.is_empty() { String::new() } else { format!(": {}", s.detail) }, s.ms))).collect::<Vec<_>>()))
            }).collect::<Vec<_>>())
            .child(heading("Versions"))
            .child(muted(format!("Current: version {}{}", wf.version, if self.dirty { " (unsaved changes)" } else { "" })))
            .children(versions.into_iter().take(12).enumerate().map(|(i, (v, t))| {
                let id = wf.id.clone();
                row().gap(px(8.)).child(div().flex_1().text_size(px(13.)).text_color(c(TEXT)).child(format!("Version {v} • saved {}", chrono::DateTime::from_timestamp(t, 0).map(|d| d.with_timezone(&chrono::Local).format("%b %-d %-I:%M %p").to_string()).unwrap_or_default()))).child(button_small(("wf-restore", i), "Restore").on_click(cx.listener(move |this, _, window, cx| {
                    match workflow::restore(&id, v) {
                        Ok(w) => {
                            this.open(w, window, cx);
                            this.toast(cx, ToastKind::Success, format!("Restored version {v} as a new version"));
                        }
                        Err(e) => this.toast(cx, ToastKind::Error, e.to_string()),
                    }
                })))
            }).collect::<Vec<_>>());

        row().gap(px(14.)).items_start().child(sidebar).child(body).into_any_element()
    }
}
