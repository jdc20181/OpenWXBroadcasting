//! Weather file inspector and import pipeline; decoding runs away from the UI thread.
use crate::{
    app::{AppState, ToastKind},
    formats,
    ui::{button_small, chip, col, heading, muted, row},
};
use gpui_kit::{
    component::input::{Input, InputState},
    prelude::FluentBuilder as _,
    *,
};
use std::path::PathBuf;

pub struct DataFilesView {
    app: Entity<AppState>,
    path: Option<PathBuf>,
    inventory: Option<formats::Inventory>,
    selected: usize,
    method: &'static str,
    name: Entity<InputState>,
    steps: Entity<InputState>,
    busy: bool,
    status: String,
}
impl DataFilesView {
    pub fn new(app: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Layer name (blank uses the file name)")
        });
        let steps = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Optional steps, separated by ; e.g. k_to_f; clip -50 130; smooth 1")
        });
        Self {
            app,
            path: None,
            inventory: None,
            selected: 0,
            method: "nearest",
            name,
            steps,
            busy: false,
            status: String::new(),
        }
    }
    fn inspect(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.status = "Choose a weather file…".into();
        let work = cx.background_executor().spawn(async move {
            let path = rfd::FileDialog::new()
                .add_filter(
                    "Weather and geographic data",
                    &[
                        "grb2", "grib2", "grb", "grib", "nc", "nc4", "h5", "hdf5", "bufr", "buf",
                        "csv", "geojson", "json", "shp", "zip",
                    ],
                )
                .pick_file();
            path.map(|p| {
                let inv = formats::inspect(&p);
                (p, inv)
            })
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if let Some((path, result)) = result {
                    this.path = Some(path);
                    this.selected = 0;
                    this.inventory = None;
                    match result {
                        Ok(inv) => {
                            this.status =
                                "Select a variable, then import it over your home coverage area."
                                    .into();
                            this.selected = inv
                                .variables
                                .iter()
                                .find(|v| v.mappable)
                                .map(|v| v.index)
                                .unwrap_or(0);
                            this.inventory = Some(inv);
                        }
                        Err(e) => this.status = format!("Inspection failed: {e}"),
                    }
                } else {
                    this.status = "No file selected.".into();
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn import(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(path) = self.path.clone() else {
            return;
        };
        let index = self.selected;
        let method = self.method;
        let name = self.name.read(cx).value().to_string();
        let name = if name.trim().is_empty() {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        } else {
            name
        };
        let steps: Vec<String> = self
            .steps
            .read(cx)
            .value()
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        let (n, s, w, e) = crate::grid::view_bounds(self.app.read(cx).settings.home, 0.);
        self.busy = true;
        self.status = "Decoding and regridding…".into();
        let work = cx.background_executor().spawn(async move {
            let points = formats::decode(&path, index, Some((s, w, n, e)))?;
            anyhow::ensure!(
                !points.values.is_empty(),
                "No valid geolocated values inside the home coverage area"
            );
            let mut grid = formats::regrid(&points, (s, w, n, e), 120, 80, method);
            let applied = formats::apply_pipeline_checked(&mut grid, &steps)?;
            anyhow::ensure!(
                grid.values.iter().any(|v| v.is_finite()),
                "The pipeline removed all values"
            );
            let (csv, provenance) =
                formats::save_layer(&name, &path, &points, &grid, method, &applied)?;
            let mut layer = crate::customdata::new_layer(&csv)
                .ok_or_else(|| anyhow::anyhow!("Saved layer could not be opened"))?;
            layer.name = name;
            layer.field = "value".into();
            Ok::<_, anyhow::Error>((layer, provenance))
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok((layer, p)) => {
                        this.status = format!(
                            "Imported {}. Source: {}. Checksum: {}",
                            p.name, p.source_file, p.checksum
                        );
                        this.app.update(cx, |s, cx| {
                            s.update_live(|l| l.custom_layers.push(layer));
                            s.toast(
                                ToastKind::Success,
                                "Weather data added to Map Studio > Custom data",
                            );
                            cx.notify();
                        });
                    }
                    Err(e) => this.status = format!("Import failed: {e}"),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
impl Render for DataFilesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected;
        let method = self.method;
        let mut body=col().gap(px(8.)).child(heading("Weather data files"))
            .child(muted("Inspect GRIB2, NetCDF, HDF5 and BUFR, then turn a geolocated variable into a map layer. GRIB1 is metadata only. Unsupported projections and BUFR descriptors report an error."))
            .child(button_small("weather-file-open",if self.busy{"Working…"}else{"Inspect a file…"}).on_click(cx.listener(|this,_,_,cx|this.inspect(cx))))
            .child(muted(self.path.as_ref().map(|p|p.display().to_string()).unwrap_or_default()));
        if let Some(inv) = &self.inventory {
            body = body
                .children(inv.summary.iter().cloned().map(muted).collect::<Vec<_>>())
                .children(
                    inv.attributes
                        .iter()
                        .take(20)
                        .map(|(k, v)| muted(format!("{k}: {v}")))
                        .collect::<Vec<_>>(),
                )
                .children(
                    inv.variables
                        .iter()
                        .take(500)
                        .map(|v| {
                            let index = v.index;
                            chip(
                                ("weather-var", index),
                                format!(
                                    "{} • {} • {} {}",
                                    v.name,
                                    v.shape,
                                    v.units,
                                    if v.mappable { "" } else { "(inspect only)" }
                                ),
                                selected == index,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.selected = index;
                                    cx.notify();
                                },
                            ))
                        })
                        .collect::<Vec<_>>(),
                );
        }
        body.child(Input::new(&self.name)).child(row().gap(px(5.)).children([("nearest","Nearest"),("bilinear","Bilinear"),("idw","Inverse distance")].into_iter().enumerate().map(|(i,(m,title))|chip(("weather-method",i),title,method==m).on_click(cx.listener(move|this,_,_,cx|{this.method=m;cx.notify();}))).collect::<Vec<_>>()))
            .child(Input::new(&self.steps)).child(muted("Steps: scale N; offset N; k_to_f; c_to_f; mm_to_in; ms_to_mph; pa_to_mb; clip MIN MAX; smooth 1–10; mask_below N; abs"))
            .child(button_small("weather-import","Import selected variable onto map").on_click(cx.listener(|this,_,_,cx|this.import(cx))))
            .child(muted(self.status.clone())).child(heading("Import history"))
            .children(formats::provenance_list().into_iter().take(15).map(|p|muted(format!("{} • {} • {} • {}",p.name,p.format,p.variable,p.regrid))).collect::<Vec<_>>())
    }
}
