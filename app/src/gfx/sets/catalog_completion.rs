//! Parameterized catalog presentations for source-supplied products and producer copy.
//! No live fetching or meteorological calculation. Exact row identity, period, scope,
//! source, units, and reviewed input are required before any data is drawn.
use super::{board::{self, Layout, Scene}, reference};
use crate::{config::Settings, gfx::{bind::Bindings, model::{AnimKind, GraphicDoc, Layer, LayerKind, Keyframe}, text::{Align, GlyphCache, TextStyle, Weight, draw_text_box}}};
use serde::{Deserialize, Serialize};
use tiny_skia::{Color, Mask, Paint, Pixmap, PixmapPaint, Rect, Transform};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind { Map, Imagery, Line, Bars, Table, Cards, Calendar, Photo, Timeline, Compare, MultiImagery, Matrix, Daily, Tornado }
#[derive(Clone, Copy, PartialEq, Eq)]
enum Format { Wide, Portrait, Square, Lower, Overlay, Ticker, Bug }
#[derive(Clone, Copy)]
struct Spec {
    id: &'static str, key: &'static str, title: &'static str, category: &'static str,
    description: &'static str, scope: &'static str, period: &'static str,
    format: Format, kind: Kind, palette: u8, fields: &'static [&'static str], animate: bool, pages: usize,
}
include!("catalog_completion_specs.rs");
#[derive(Clone, Serialize, Deserialize)]
struct Datum { label: String, value: String }
#[derive(Clone, Serialize, Deserialize)]
struct Series { name: String, unit: String, color: String, labels: Vec<String>, values: Vec<Option<f32>> }
#[derive(Clone, Serialize, Deserialize)]
struct MoonDay { day: u8, phase: Option<f32>, label: String }
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Frame {
    valid: String, heading: String, note: String, unit: String,
    values: Vec<Datum>, columns: Vec<String>, rows: Vec<Vec<String>>, bullets: Vec<String>,
    scene: Option<Scene>, media: String, credit: String, series: Vec<Series>,
    days: Vec<MoonDay>, first_weekday: u8,
    page: usize, first_hour: usize, total_hours: usize, amount_axis_max: f32, track: Vec<[f64;2]>, ahead: Vec<board::ReportPoint>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Input {
    id: String, scope: String, period: String, reviewed: bool,
    source: String, issued: String, frames: Vec<Frame>,
}
include!("catalog_reference_tables.rs");
include!("catalog_purpose_repairs.rs");
include!("catalog_closeout_repairs.rs");
fn prefix(s: Spec) -> String { format!("set.catalog.{}",s.id) }
fn token(s: Spec,field:&str) -> String { format!("{{{{{}.{field}}}}}",prefix(s)) }
fn short(value:&str,n:usize) -> bool { value.chars().count()<=n }
fn checked(s:Spec,raw:&str) -> Option<Input> {
    if raw.len()>4_000_000 { return None; }
    let p:Input=serde_json::from_str(raw).ok()?;
    if !p.reviewed || p.id!=s.id || p.scope!=s.scope || p.period!=s.period
        || p.source.trim().is_empty() || !short(&p.source,240) || p.issued.trim().is_empty()
        || !short(&p.issued,100) || p.frames.len()!=if matches!(s.kind,Kind::Timeline|Kind::MultiImagery){4}else if s.kind==Kind::Compare{2}else{1} {return None;}
    for f in &p.frames {
        if f.valid.trim().is_empty() || !short(&f.valid,100) || !short(&f.heading,160)
            || !short(&f.note,400) || !short(&f.unit,80) || f.values.len()>6 || f.rows.len()>if s.kind==Kind::Matrix{24}else if s.kind==Kind::Daily{14}else if ranked(s){10}else{8}
            || f.columns.len()>if s.kind==Kind::Matrix{20}else{6} || f.bullets.len()>4 || f.series.len()>8 || f.days.len()>31
            || f.values.iter().any(|d|!short(&d.label,60)||!short(&d.value,140))
            || f.columns.iter().any(|c|!short(c,60)) || f.rows.iter().any(|row|row.len()!=f.columns.len()||row.iter().any(|c|!short(c,180)))
            || f.bullets.iter().any(|b|!short(b,350)) { return None; }
        if !purpose_contract(s,f) || !closeout_contract(s,f){return None;}
        match s.kind {
            Kind::Map | Kind::Timeline | Kind::Compare | Kind::Tornado => {
                let sc=f.scene.as_ref()?;
                if !sc.reviewed || sc.panels.len()!=1 || !sc.panels[0].available()
                    || sc.panels[0].valid!=f.valid || sc.legend_title.trim().is_empty()
                    || board::bands(sc).is_empty() || !scope_matches(s,&sc.scope)
                    || (s.palette==1 && sc.palette!=board::Palette::RainTotal)
                    || (s.palette==2 && sc.palette!=board::Palette::SnowTotal)
                    || board::parse(&serde_json::to_string(sc).ok()?,Layout::Single).is_none() {return None;}
            },
            Kind::Imagery | Kind::Photo | Kind::MultiImagery => {
                if f.media.trim().is_empty() || !short(&f.media,180) || f.credit.trim().is_empty() || !short(&f.credit,240)
                    || std::path::Path::new(&f.media).file_name()!=Some(std::ffi::OsStr::new(&f.media)) {return None;}
            },
            Kind::Line | Kind::Bars => {
                if f.series.is_empty() || f.series.iter().any(|s|s.unit.trim().is_empty() || !short(&s.unit,35)
                    || s.name.trim().is_empty() || !short(&s.name,70) || crate::gfx::parse_hex(&s.color).is_none()
                    || s.labels.is_empty() || s.labels.len()>96 || s.labels.len()!=s.values.len()
                    || s.labels.iter().any(|l|!short(l,65)) || s.values.iter().flatten().any(|v|!v.is_finite())
                    || s.values.iter().all(Option::is_none)) {return None;}
                if f.series.iter().any(|a|a.unit!=f.series[0].unit || a.labels!=f.series[0].labels){return None;}
            },
            Kind::Calendar => {
                if f.first_weekday>6 || f.days.is_empty() || f.days.iter().any(|d|d.day<1 || d.day>31
                    || !short(&d.label,70) || d.phase.is_some_and(|p|!p.is_finite() || !(0.0..=1.0).contains(&p))) {return None;}
                let mut days=std::collections::HashSet::new();if f.days.iter().any(|d|!days.insert(d.day)){return None;}
            },
            Kind::Table | Kind::Matrix | Kind::Daily => { if f.rows.is_empty() || f.columns.is_empty(){return None;} },
            Kind::Cards => {if f.values.is_empty() && f.bullets.is_empty(){return None;}},
        }
    }
    if s.kind==Kind::Compare {
        let a=p.frames[0].scene.as_ref()?;let b=p.frames[1].scene.as_ref()?;
        if serde_json::to_string(&a.scope).ok()!=serde_json::to_string(&b.scope).ok()
            || serde_json::to_string(&board::bands(a)).ok()!=serde_json::to_string(&board::bands(b)).ok()
            || a.legend_title!=b.legend_title {return None;}
    }
    Some(p)
}
fn scope_matches(s:Spec,scope:&board::Scope)->bool {
    match s.scope {
        "National" => scope.county.is_none() && scope.bbox.is_some_and(|b|b[0]<=-120. && b[2]>=-70.),
        "County" => scope.county.is_some(),
        "State" => scope.county.is_none() && scope.states.len()==1 && scope.bbox.is_none(),
        "Multi-state" => scope.county.is_none() && ((2..=8).contains(&scope.states.len())||scope.bbox.is_some()),
        "Region" => scope.county.is_none() && scope.bbox.is_some(),
        _ => true,
    }
}
fn default_scope(s:Spec,settings:&Settings)->board::Scope {
    if s.scope=="National"{return board::Scope{states:vec![],county:None,bbox:Some([-125.,24.,-66.,50.])};}
    super::observed::report_scope(settings,match s.scope {
        "County"=>super::observed::Coverage::County,"State"=>super::observed::Coverage::State,
        "Multi-state"=>super::observed::Coverage::Multi,_=>super::observed::Coverage::Region,
    })
}
pub(super) fn bind(b:&mut Bindings) {
    // Only pure unavailable strings are retained. Producer payloads are never cached.
    static DEFAULTS:std::sync::OnceLock<Vec<(String,String)>>=std::sync::OnceLock::new();
    for (key,value) in DEFAULTS.get_or_init(||SPECS.iter().flat_map(|s|[
        (format!("{}.subtitle",prefix(*s)),if s.kind==Kind::Matrix{"Educational reference • °F".into()}else{purpose_subtitle(*s)}),
        (format!("{}.source",prefix(*s)),if s.kind==Kind::Matrix{reference_input(*s).source}else{"Data not available yet".into()}),
        (format!("{}.note",prefix(*s)),if s.kind==Kind::Matrix{reference_input(*s).frames[0].note.clone()}else{"Data not available yet".into()}),
        (format!("{}.payload",prefix(*s)),if s.kind==Kind::Matrix{serde_json::to_string(&reference_input(*s)).unwrap()}else{"{}".into()}),
        (format!("{}.crawl",prefix(*s)),"Data not available yet".into()),
        (format!("{}.payload_0",prefix(*s)),"{}".into()),(format!("{}.payload_1",prefix(*s)),"{}".into()),
        (format!("{}.payload_2",prefix(*s)),"{}".into()),(format!("{}.payload_3",prefix(*s)),"{}".into()),
        (format!("{}.payload_4",prefix(*s)),"{}".into()),(format!("{}.payload_5",prefix(*s)),"{}".into()),
    ]).collect()) {if b.get(key).is_none(){b.set(key,value);}}
}
fn keys(rect:(f32,f32,f32,f32),frame:usize,count:usize)->Vec<Keyframe> {
    let key=|t,active|Keyframe{t,x:rect.0,y:rect.1,scale:1.,rotation:0.,opacity:if active{1.}else{0.},easing:"linear".into()};
    let mut keys=vec![key(0.,frame==0)];
    for boundary in 1..count {let t=boundary as f32*2.;keys.push(key(t,frame==boundary-1));keys.push(key(t,frame==boundary));}
    keys.push(key(count as f32*2.,frame==count-1));keys.push(key(count as f32*2.,frame==0));keys
}
pub(super) fn build(key:&str)->Option<GraphicDoc> {
    let s=*SPECS.iter().find(|s|s.key==key)?;
    let title=presentation_title(s);let subtitle=token(s,"subtitle");let source=token(s,"source");
    let mut d=match s.format {
        Format::Portrait=>reference::social_frame(title,s.category,&subtitle,&source,true),
        Format::Square=>reference::social_frame(title,s.category,&subtitle,&source,false),
        Format::Lower|Format::Ticker=>reference::lowerthird_frame(title,s.category,&subtitle,&source),
        Format::Overlay=>super::product_overlay::template(title,s.category,&format!("catalog.{}",s.id)),
        _=>reference::frame(title,s.category,&subtitle,&source),
    };
    let rect=match s.format {
        Format::Portrait=>(54.,242.,972.,1400.),Format::Square=>(54.,205.,972.,700.),
        Format::Lower|Format::Ticker=>(80.,870.,1760.,118.),Format::Overlay=>(82.,698.,1132.,240.),
        Format::Bug=>(1400.,155.,440.,130.),Format::Wide=>(70.,205.,1780.,715.),
    };
    if s.format==Format::Overlay { d.layers.retain(|l|!matches!(l.kind,LayerKind::RiskMap{..})); }
    if s.format==Format::Bug {
        d.background=None;d.layers.clear();
        d.layers.push(Layer::rect("Corner information panel",1390.,65.,460.,230.,crate::gfx::model::Fill::solid("$panel"),12.));
    }
    if s.format==Format::Ticker {
        d.layers.push(Layer::new("Catalog crawl",rect.0,rect.1,rect.2,rect.3,LayerKind::Ticker {
            text:token(s,"crawl"),font:"$display".into(),size:34.,color:"$text".into(),speed:110.,
        }));
        d.template=Some(s.key.into());return Some(d);
    }
    if s.format==Format::Bug {
        reference::label(&mut d,"Corner station",(1400.,70.,440.,28.),"{{station}}",20.,"$muted",Align::Left);
        reference::label(&mut d,"Corner title",(1400.,103.,440.,44.),title,26.,"$text",Align::Left);
    }
    for frame in 0..s.pages {
        let field=if s.pages>1{format!("payload_{frame}")}else{"payload".into()};
        let mut layer=Layer::risk_map("Catalog product presentation",rect.0,rect.1,rect.2,rect.3,&format!("set:catalogpresentation:{}:{}",s.key,token(s,&field))).with_anim(AnimKind::Fade,0.2,0.4);
        if s.pages>1 {layer.anim_in.kind=AnimKind::None;layer.loop_mode="loop".into();layer.loop_secs=s.pages as f32*2.;layer.keys=keys(rect,frame,s.pages);}
        d.layers.push(layer);
    }
    if s.pages>1 { d.duration=0.; }
    if matches!(s.format,Format::Wide|Format::Portrait|Format::Square) {
        let (x,y,w,h)=match s.format{Format::Portrait=>(54.,1656.,972.,68.),Format::Square=>(54.,918.,972.,48.),_=>(80.,936.,1760.,44.)};
        reference::label(&mut d,"Product source and coverage note",(x,y,w,h),&token(s,"note"),22.,"$muted",Align::Left);
    }
    for l in &mut d.layers {
        if l.name=="Title"{l.w=980.;}
        if let LayerKind::Text{auto_fit,wrap,..}=&mut l.kind {*auto_fit=true;*wrap=true;}
    }
    d.template=Some(s.key.into());Some(d)
}
fn color(s:&str)->Color {crate::gfx::parse_hex(s).unwrap_or(Color::WHITE)}
fn panel(pm:&mut Pixmap,r:(f32,f32,f32,f32),c:&str){if let Some(r)=Rect::from_xywh(r.0,r.1,r.2.max(1.),r.3.max(1.)){let mut p=Paint::default();p.set_color(color(c));pm.fill_rect(r,&p,Transform::from_scale(draw_scale(),draw_scale()),None);}}
fn text(pm:&mut Pixmap,g:&mut GlyphCache,font:&str,value:&str,r:(f32,f32,f32,f32),size:f32,c:&str) {
    draw_text_box(pm,g,value,scaled_rect(r),&TextStyle{family:font.into(),weight:Weight::SemiBold,size:size*draw_scale(),color:color(c),letter_spacing:0.},Align::Left,true,true,true,false,1.);
}
fn image(pm:&mut Pixmap,f:&Frame,r:(f32,f32,f32,f32))->bool {
    let Some(root)=crate::config::portable_root()else{return false;};
    let path=root.join("Library").join("Media").join(&f.media);
    let Some(img)=crate::gfx::render::load_image_pixmap(&path)else{return false;};
    let scale=(r.2/img.width() as f32).min(r.3/img.height() as f32);
    let t=Transform::from_translate((r.0+(r.2-img.width() as f32*scale)*0.5)*draw_scale(),(r.1+(r.3-img.height() as f32*scale)*0.5)*draw_scale()).pre_scale(scale*draw_scale(),scale*draw_scale());
    pm.draw_pixmap(0,0,img.as_ref(),&PixmapPaint::default(),t,None);true
}
fn chart(pm:&mut Pixmap,g:&mut GlyphCache,font:&str,f:&Frame,bars:bool,r:(f32,f32,f32,f32)) {
    use tiny_skia::{PathBuilder, Stroke};
    let values:Vec<_>=f.series.iter().flat_map(|s|s.values.iter().flatten().copied()).collect();
    let lo=values.iter().copied().fold(if bars{0.}else{f32::MAX},f32::min);let hi=values.iter().copied().fold(if bars{0.}else{f32::MIN},f32::max);
    let pad=((hi-lo)*0.1).max(1.);let percent=f.series.iter().all(|s|s.unit=="%");let lo=if percent{0.}else{lo-pad};let hi=if percent{100.}else{hi+pad};let n=f.series[0].labels.len();
    let plot=(r.0+r.2*0.13,r.1+45.,r.2*0.82,(r.3-110.).max(20.));
    for k in 0..5 {let y=plot.1+plot.3*k as f32/4.;panel(pm,(plot.0,y,plot.2,1.),"#38516A");text(pm,g,font,&format!("{:.1}",hi-(hi-lo)*k as f32/4.),(r.0,y-13.,r.2*0.12,28.),18.,"#B9C9D8");}
    for (si,s) in f.series.iter().enumerate() {
        let mut p=Paint::default();p.set_color(color(&s.color));p.anti_alias=true;
        let mut run=Vec::new();
        let flush=|run:&mut Vec<(f32,f32)>,pm:&mut Pixmap|{if run.len()>1 {let mut path=PathBuilder::new();path.move_to(run[0].0,run[0].1);for &(x,y) in run.iter().skip(1){path.line_to(x,y);}if let Some(path)=path.finish(){pm.stroke_path(&path,&p,&Stroke{width:3.,..Default::default()},Transform::from_scale(draw_scale(),draw_scale()),None);}}run.clear();};
        for (i,value) in s.values.iter().enumerate() {
            let x=if bars {plot.0+plot.2*(i as f32+0.1)/n.max(1) as f32}else{plot.0+plot.2*i as f32/(n.max(2)-1) as f32};
            if let Some(v)=value {let y=plot.1+plot.3*(hi-v)/(hi-lo);
                if bars {let bw=plot.2/n.max(1) as f32/f.series.len() as f32*0.75;let zero=plot.1+plot.3*hi/(hi-lo);panel(pm,(x+si as f32*bw,y.min(zero),bw,(zero-y).abs().max(1.)),&s.color);}else{run.push((x,y));}
            } else {flush(&mut run,pm);}
        }flush(&mut run,pm);
        text(pm,g,font,&format!("{} • {}",s.name,s.unit),(r.0+si as f32*r.2/f.series.len() as f32,r.1,r.2/f.series.len() as f32-8.,35.),20.,&s.color);
    }
    for i in 0..n {if i==0||i==n-1||i%(n/4).max(1)==0 {text(pm,g,font,&f.series[0].labels[i],(plot.0+plot.2*i as f32/(n.max(2)-1) as f32-20.,plot.1+plot.3+8.,(plot.2/4.).max(35.),45.),17.,"#B9C9D8");}}
}
fn calendar(pm:&mut Pixmap,g:&mut GlyphCache,font:&str,f:&Frame,r:(f32,f32,f32,f32)) {
    let single=f.days.len()==1;let cw=if single{r.2}else{r.2/7.};let ch=if single{r.3}else{r.3/6.};
    for d in &f.days {let index=if single{0}else{d.day as usize-1+f.first_weekday as usize};let x=r.0+(index%7) as f32*cw;let y=r.1+(index/7) as f32*ch;
        panel(pm,(x+2.,y+2.,cw-4.,ch-4.),"#19334E");text(pm,g,font,&d.day.to_string(),(x+8.,y+5.,cw-16.,25.),19.,"#FFFFFF");
        if let Some(phase)=d.phase {let logical_size=(cw.min(ch)*0.42).clamp(12.,if single{480.}else{60.});let size=(logical_size*draw_scale()).round().max(1.) as u32;
            if let Some(mut moon)=Pixmap::new(size,size) {let radius=size as f32*0.48;let angle=phase*std::f32::consts::TAU;
                for py in 0..size {for px in 0..size {let xx=(px as f32-size as f32*0.5)/radius;let yy=(py as f32-size as f32*0.5)/radius;let z2=1.-xx*xx-yy*yy;if z2>=0. {let bright=xx*angle.sin()-z2.sqrt()*angle.cos()>0.;let c=if bright{[229,235,240,255]}else{[45,56,72,255]};let n=((py*size+px)*4) as usize;moon.data_mut()[n..n+4].copy_from_slice(&c);}}}
                pm.draw_pixmap(((x+(cw-logical_size)*0.5)*draw_scale()) as i32,((y+ch*0.27)*draw_scale()) as i32,moon.as_ref(),&PixmapPaint::default(),Transform::identity(),None);
            }
        }
        text(pm,g,font,&d.label,(x+6.,y+ch*0.70,cw-12.,ch*0.26),15.,"#D2DFEA");
    }
}
fn content(pm:&mut Pixmap,g:&mut GlyphCache,font:&str,s:Spec,f:&Frame,r:(f32,f32,f32,f32),compact:bool) {
    if snow_range(s){snow_range_draw(pm,g,font,f,r,compact);return;}
    if compact && s.kind==Kind::Tornado {tornado_callout(pm,g,font,f,r);return;}
    if compact && s.kind==Kind::Daily {daily_strip(pm,g,font,f,r);return;}
    if compact && hourly_count(s).is_some() {hourly_strip(pm,g,font,f,r);return;}
    if compact {
        let mut parts:Vec<_>=f.values.iter().map(|d|format!("{}: {}",d.label,d.value)).collect();
        if parts.is_empty(){parts.extend(f.bullets.iter().cloned());}
        if parts.is_empty() && !f.rows.is_empty(){parts.extend(f.rows.iter().take(2).map(|row|f.columns.iter().zip(row).map(|(c,v)|format!("{c}: {v}")).collect::<Vec<_>>().join(" • ")));}
        if parts.is_empty(){for series in &f.series {if let Some(Some(v))=series.values.last(){parts.push(format!("{}: {} {}",series.name,v,series.unit));}}}
        if parts.is_empty(){parts.push(if f.note.trim().is_empty(){reference::MISSING.into()}else{f.note.clone()});}
        text(pm,g,font,&parts.join(" • "),r,if s.format==Format::Bug{23.}else{28.},"#FFFFFF");return;
    }
    match s.kind {
        Kind::Daily=>daily_cards(pm,g,font,f,r),
        Kind::Tornado=>tornado_callout(pm,g,font,f,r),
        Kind::Map|Kind::Timeline|Kind::Compare=>{if let Some(sc)=&f.scene {
            let raw=serde_json::to_string(sc).unwrap();
            if s.format==Format::Overlay {super::product_overlay::draw(pm,g,Transform::from_translate(r.0*draw_scale(),r.1*draw_scale()).pre_scale(draw_scale(),draw_scale()),None,r.2,r.3,&format!("set:{}:{raw}",if sc.panels[0].points.is_some(){"reportoverlay"}else{"productoverlay"}),1.,font);}
            else {board::draw(pm,g,Transform::from_translate(r.0*draw_scale(),r.1*draw_scale()).pre_scale(draw_scale(),draw_scale()),None,r.2,r.3,&format!("set:board:single:{raw}"),1.,font);}
        }},
        Kind::Imagery|Kind::Photo|Kind::MultiImagery=>{if !image(pm,f,(r.0,r.1,r.2,r.3-60.)){text(pm,g,font,reference::MISSING,r,30.,"#FFFFFF");}text(pm,g,font,&f.credit,(r.0,r.1+r.3-50.,r.2,45.),20.,"#B9C9D8");},
        Kind::Line|Kind::Bars=>chart(pm,g,font,f,s.kind==Kind::Bars,r),
        Kind::Calendar=>calendar(pm,g,font,f,r),
        Kind::Matrix=>matrix(pm,g,font,f,r),
        Kind::Table=>{let rh=r.3/(f.rows.len()+1) as f32;let cw=r.2/f.columns.len() as f32;
            for (j,c) in f.columns.iter().enumerate(){text(pm,g,font,c,(r.0+j as f32*cw+8.,r.1,cw-16.,rh-6.),22.,"#70D5EF");}
            for (i,row) in f.rows.iter().enumerate(){let y=r.1+(i+1) as f32*rh;panel(pm,(r.0,y,r.2,rh-5.),if i%2==0{"#19334E"}else{"#142B43"});hourly_bars(pm,s,f,row,(r.0,y,r.2,rh-5.));for (j,c) in row.iter().enumerate(){text(pm,g,font,if c.trim().is_empty(){reference::MISSING}else{c},(r.0+j as f32*cw+8.,y+3.,cw-16.,rh-10.),24.,"#FFFFFF");}}
        },
        Kind::Cards=>{let portrait=r.3>r.2;let count=f.values.len();let cols=if portrait{1}else{2};let rows=count.div_ceil(cols).max(1);let value_h=if f.bullets.is_empty(){r.3}else{r.3*0.55};let cw=r.2/cols as f32;let ch=value_h/rows as f32;
            for(i,d)in f.values.iter().enumerate(){let x=r.0+(i%cols) as f32*cw;let y=r.1+(i/cols) as f32*ch;panel(pm,(x+3.,y+3.,cw-6.,ch-6.),"#19334E");text(pm,g,font,&d.label,(x+18.,y+8.,cw-36.,ch*0.30),22.,"#70D5EF");text(pm,g,font,&if d.value.trim().is_empty(){reference::MISSING.into()}else{d.value.clone()},(x+18.,y+ch*0.34,cw-36.,ch*0.58),if portrait{40.}else{45.},"#FFFFFF");}
            let start=if count==0{r.1}else{r.1+value_h+8.};let bh=(r.1+r.3-start)/f.bullets.len().max(1) as f32;
            for(i,b)in f.bullets.iter().enumerate(){text(pm,g,font,b,(r.0+15.,start+i as f32*bh,r.2-30.,bh-6.),30.,"#FFFFFF");}
        },
    }
}
pub(super) fn draw(pm:&mut Pixmap,g:&mut GlyphCache,local:Transform,mask:Option<&Mask>,w:f32,h:f32,spec:&str,opacity:f32,font:&str)->bool {
    let Some(rest)=spec.strip_prefix("set:catalogpresentation:")else{return false;};let Some((key,raw))=rest.split_once(':')else{return true;};let Some(&s)=SPECS.iter().find(|s|s.key==key)else{return true;};
    let dw=(w*local.sx.abs()).round().clamp(1.,4096.) as u32;let dh=(h*local.sy.abs()).round().clamp(1.,4096.) as u32;let Some(mut out)=Pixmap::new(dw,dh)else{return true;};
    out.fill(Color::from_rgba8(16,36,58,255));
    let _scale_guard=CanvasScale::new(dw as f32/w);let lw=w;let lh=dh as f32/draw_scale();
    if let Some(p)=checked(s,raw) {
        if s.kind==Kind::Compare {
            let mut sc=p.frames[0].scene.as_ref().unwrap().clone();
            sc.panels=vec![p.frames[0].scene.as_ref().unwrap().panels[0].clone(),p.frames[1].scene.as_ref().unwrap().panels[0].clone()];
            board::draw(&mut out,g,Transform::from_scale(draw_scale(),draw_scale()),None,lw,lh-40.,&format!("set:board:compare:{}",serde_json::to_string(&sc).unwrap()),1.,font);
            text(&mut out,g,font,&p.source,(12.,lh-34.,lw-24.,28.),18.,"#B9C9D8");
            pm.draw_pixmap(0,0,out.as_ref(),&PixmapPaint{opacity:opacity.clamp(0.,1.),..Default::default()},Transform::from_translate(local.tx,local.ty),mask);return true;
        }
        let compact=matches!(s.format,Format::Lower|Format::Ticker|Format::Bug);let count=p.frames.len();let cols=if count==4 || (count==2 && lw>lh){2}else{1};let rows=count/cols;let cw=lw/cols as f32;let ch=lh/rows as f32;
        for(i,f)in p.frames.iter().enumerate(){let x=(i%cols) as f32*cw;let y=(i/cols) as f32*ch;
            if s.format==Format::Overlay {content(&mut out,g,font,s,f,(x,y,cw,ch),false);}
            else if compact{content(&mut out,g,font,s,f,(x+8.,y+5.,cw-16.,ch-10.),true);}else{
                text(&mut out,g,font,&format!("{} • {}",f.heading,f.valid),(x+12.,y+4.,cw-if s.title.starts_with("Futurecast 4-panel"){112.}else{24.},42.),22.,"#D2DFEA");
                if s.title.starts_with("Futurecast 4-panel") {text(&mut out,g,font,&if i<3{format!("{} > {}",i+1,i+2)}else{"4".into()},(x+cw-88.,y+8.,76.,30.),22.,"#70D5EF");}
                content(&mut out,g,font,s,f,(x+8.,y+52.,cw-16.,ch-115.),false);
                text(&mut out,g,font,&format!("{} • {}",p.source,p.issued),(x+12.,y+ch-55.,cw-24.,25.),17.,"#B9C9D8");
                text(&mut out,g,font,&f.note,(x+12.,y+ch-30.,cw-24.,26.),16.,"#B9C9D8");
            }
        }
    }else{text(&mut out,g,font,reference::MISSING,(20.,lh*0.35,lw-40.,lh*0.30),if lh<200.{28.}else{36.},"#FFFFFF");}
    pm.draw_pixmap(0,0,out.as_ref(),&PixmapPaint{opacity:opacity.clamp(0.,1.),..Default::default()},Transform::from_translate(local.tx,local.ty),mask);true
}
#[cfg(test)]
pub(super) fn sample(d:&mut GraphicDoc) {
    let Some(&s)=SPECS.iter().find(|s|Some(s.key)==d.template.as_deref())else{return;};
    if s.kind==Kind::Matrix {
        let p=reference_input(s);
        d.data_overrides.extend([(format!("{}.payload",prefix(s)),serde_json::to_string(&p).unwrap()),(format!("{}.source",prefix(s)),p.source),(format!("{}.subtitle",prefix(s)),"Educational reference • °F".into()),(format!("{}.note",prefix(s)),p.frames[0].note.clone())]);return;
    }
    let settings=Settings::default();let scope=default_scope(s,&settings);
    for phase in 0..s.pages{
        let mut p=Input{id:s.id.into(),scope:s.scope.into(),period:s.period.into(),reviewed:true,source:"SAMPLE ONLY • Synthetic layout fixture".into(),issued:"SAMPLE issue • Nov 23,2026".into(),frames:Vec::new()};
        for i in 0..if matches!(s.kind,Kind::Timeline|Kind::MultiImagery){4}else if s.kind==Kind::Compare{2}else{1}{
            let mut f=Frame{valid:format!("SAMPLE • Frame {}",i+phase+1),heading:s.title.into(),note:"SAMPLE ONLY • Synthetic values • Not for broadcast".into(),unit:"SAMPLE units".into(),..Default::default()};
            f.values=s.fields.iter().take(6).enumerate().map(|(i,label)|Datum{label:(*label).into(),value:format!("SAMPLE value {}",i+1)}).collect();
            match s.kind {
                Kind::Map|Kind::Timeline|Kind::Compare|Kind::Tornado=>{let mut sc=board::empty(Layout::Single);sc.reviewed=true;sc.scope=scope.clone();sc.legend_title=format!("SAMPLE • {} • Supplied units",s.title);sc.palette=match s.palette{1=>board::Palette::RainTotal,2=>board::Palette::SnowTotal,_=>board::Palette::Category};sc.legend=["#3855B8","#387DD0","#28B8D2","#37BE83","#D6DB45","#F5A332","#EB5538","#B52643"].iter().enumerate().map(|(i,c)|board::Band{label:format!("SAMPLE class {}",i+1),color:(*c).into()}).collect();sc.panels[0].title=s.title.chars().take(80).collect();sc.panels[0].valid=f.valid.clone();sc.panels[0].detail="SAMPLE ONLY • Synthetic county categories".into();let bins=board::bands(&sc).len();if let Some(all)=crate::map::boundaries::counties_named(){sc.panels[0].counties=Some(board::selected(&all,&scope).iter().enumerate().map(|(j,c)|(c.fips.clone(),(j+i+phase)%bins)).collect());}f.scene=Some(sc);},
                Kind::Imagery|Kind::Photo|Kind::MultiImagery=>{f.media="sets-qa-image.png".into();f.credit="SAMPLE IMAGE • Synthetic fixture • Not source imagery".into();},
                Kind::Line|Kind::Bars=>{f.series=vec![Series{name:s.title.chars().take(65).collect(),unit:"SAMPLE units".into(),color:"#70D5EF".into(),labels:(0..8).map(|j|format!("SAMPLE {j}")).collect(),values:(0..8).map(|j|if j==4{None}else{Some((j*3+phase) as f32)}).collect()}];},
                Kind::Calendar=>{f.first_weekday=2;f.days=(1..=30).map(|day|MoonDay{day,phase:Some(day as f32/30.),label:"SAMPLE phase".into()}).collect();},
                Kind::Matrix=>unreachable!("reference handled above"),
                Kind::Daily=>{},
                Kind::Table=>{f.columns=s.fields.iter().take(6).map(|c|(*c).into()).collect();f.rows=(0..5).map(|i|f.columns.iter().map(|_|format!("SAMPLE {}",i+1)).collect()).collect();},
                Kind::Cards=>{f.bullets=vec!["SAMPLE ONLY • Producer-supplied copy appears here".into(),"Synthetic text exercises wrapping and spacing".into()];},
            }purpose_sample(s,&mut f,phase,i,&scope);closeout_sample(s,&mut f,i);p.frames.push(f);
        }
        closeout_metadata(s,&mut p);
        let field=if s.pages>1{format!("payload_{phase}")}else{"payload".into()};d.data_overrides.push((format!("{}.{field}",prefix(s)),serde_json::to_string(&p).unwrap()));
    }
    d.data_overrides.push((format!("{}.crawl",prefix(s)),"SAMPLE ONLY • Synthetic crawl • Not for broadcast".into()));
    d.data_overrides.extend([(format!("{}.source",prefix(s)),"SAMPLE ONLY • Not for broadcast".into()),(format!("{}.subtitle",prefix(s)),format!("SAMPLE • {}",purpose_subtitle(s))),(format!("{}.note",prefix(s)),"SAMPLE ONLY • Synthetic layout fixture".into())]);
}

#[cfg(test)]
mod export_pages {
    use super::*;
    #[test]
    fn qa_catalog_pages() {
        let Some(dir)=std::env::var_os("CWX_QA_NEW_DIR")else{return;};
        let Some(selected)=super::super::qa_selection::selected()else{return;};
        let out=std::path::PathBuf::from(dir).join("catalog-pages");std::fs::create_dir_all(&out).unwrap();
        for &s in SPECS.iter().filter(|s|s.pages>1 && selected.contains(s.key)) {
            for look in ["modern","classic"] {
                let mut doc=crate::gfx::templates::build_in(s.key,look).unwrap();sample(&mut doc);
                for phase in 0..s.pages {
                    let pm=crate::program::render_graphic_mode(&doc,&Settings::default(),&crate::data::WxSnapshot::default(),
                        12001.+phase as f32*2.,doc.width,doc.height,None,crate::gfx::check::PreviewData::Sample);
                    pm.save_png(out.join(format!("{}-{look}-page-{phase}.png",s.key))).unwrap();
                }
            }
        }
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    #[test]
    fn every_catalog_sample_is_accepted_by_its_product_contract() {
        let mut failures=Vec::new();
        for &s in SPECS {
            let mut d=build(s.key).unwrap();sample(&mut d);
            for page in 0..s.pages {
                let field=if s.pages>1{format!("payload_{page}")}else{"payload".into()};
                let key=format!("{}.{field}",prefix(s));
                let raw=&d.data_overrides.iter().find(|(k,_)|k==&key).unwrap().1;
                if checked(s,raw).is_none(){failures.push(format!("{} {} page{}",s.id,s.title,page));}
            }
        }
        assert!(failures.is_empty(),"{} rejected sample pages: {:?}",failures.len(),&failures[..failures.len().min(35)]);
    }
    #[test]
    fn product_identity_review_and_timestamps_are_required() {
        for kind in [Kind::Map,Kind::Imagery,Kind::Line,Kind::Bars,Kind::Table,Kind::Cards,Kind::Calendar,Kind::Timeline,Kind::Compare,Kind::MultiImagery,Kind::Matrix,Kind::Daily,Kind::Tornado] {
            let Some(&s)=SPECS.iter().find(|s|s.kind==kind && s.pages==1)else{continue;};
            let mut d=build(s.key).unwrap();sample(&mut d);
            let raw=&d.data_overrides.iter().find(|(k,_)|k==&format!("{}.payload",prefix(s))).unwrap().1;
            let valid=checked(s,raw).expect("valid fixture");
            let mut wrong=valid.clone();wrong.id="G00000".into();assert!(checked(s,&serde_json::to_string(&wrong).unwrap()).is_none());
            let mut wrong=valid.clone();wrong.reviewed=false;assert!(checked(s,&serde_json::to_string(&wrong).unwrap()).is_none());
            let mut wrong=valid.clone();wrong.period="another period".into();assert!(checked(s,&serde_json::to_string(&wrong).unwrap()).is_none());
            let mut wrong=valid.clone();wrong.scope="another scope".into();assert!(checked(s,&serde_json::to_string(&wrong).unwrap()).is_none());
            let mut wrong=valid.clone();wrong.frames[0].valid.clear();assert!(checked(s,&serde_json::to_string(&wrong).unwrap()).is_none());
            assert!(checked(s,"{}").is_none());
        }
    }
    #[test]
    fn all_catalog_formats_keep_layers_inside_native_canvas_and_overlays_transparent() {
        let mut failures=Vec::new();
        for &s in SPECS {for look in ["modern","classic"] {
            let d=crate::gfx::templates::build_in(s.key,look).unwrap();
            let expected=match s.format {Format::Portrait=>(1080,1920),Format::Square=>(1080,1080),_=>(1920,1080)};
            if (d.width,d.height)!=expected{failures.push(format!("{} {look} canvas",s.id));}
            for l in &d.layers {if l.x<0. || l.y<0. || l.x+l.w>d.width as f32+0.5 || l.y+l.h>d.height as f32+0.5 {failures.push(format!("{} {look} {} bounds",s.id,l.name));}}
            if matches!(s.format,Format::Lower|Format::Overlay|Format::Ticker|Format::Bug){assert!(d.background.is_none(),"{} opaque overlay",s.id);}
        }}
        assert!(failures.is_empty(),"{} layout failures: {:?}",failures.len(),&failures[..failures.len().min(35)]);
    }
}

#[cfg(test)]
mod reference_contract_tests {
    use super::*;
    #[test]
    fn published_reference_cells_match_nws_examples() {
        let heat=*SPECS.iter().find(|s|s.title=="Heat index chart").unwrap();
        let p=reference_input(heat);assert_eq!(p.frames[0].rows[8][0],"96°F");assert_eq!(p.frames[0].columns[6],"65%");assert_eq!(p.frames[0].rows[8][6],"121");
        let chill=*SPECS.iter().find(|s|s.title=="Wind chill chart").unwrap();let p=reference_input(chill);
        assert_eq!(p.frames[0].rows[3][0],"20");assert_eq!(p.frames[0].columns[4],"20°F");assert_eq!(p.frames[0].rows[3][4],"4");
    }
    #[test]
    fn comparison_rejects_different_geography_or_legend() {
        let &s=SPECS.iter().find(|s|s.kind==Kind::Compare).unwrap();let mut d=build(s.key).unwrap();sample(&mut d);
        let raw=&d.data_overrides.iter().find(|(k,_)|k==&format!("{}.payload",prefix(s))).unwrap().1;
        let valid=checked(s,raw).unwrap();let mut wrong=valid.clone();wrong.frames[1].scene.as_mut().unwrap().legend[0].label="Other units".into();assert!(checked(s,&serde_json::to_string(&wrong).unwrap()).is_none());
        let mut wrong=valid;wrong.frames[1].scene.as_mut().unwrap().scope.bbox=Some([-88.,37.,-86.,39.]);assert!(checked(s,&serde_json::to_string(&wrong).unwrap()).is_none());
    }
}
