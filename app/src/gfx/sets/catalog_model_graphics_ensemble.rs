// Multi-field and ensemble presentation. All values, bins, member paths and
// percentile definitions are supplied; no ensemble statistics are computed here.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct EnsemblePath {
    member: String,
    color: String,
    segments: Vec<Vec<[f64; 2]>>,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct EnsembleFrame {
    valid: String,
    definition: String,
    units: String,
    maps: Vec<Scene>,
    members: Vec<EnsemblePath>,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct EnsembleInput {
    model: String,
    metric: String,
    scope_name: String,
    scope: Scope,
    reviewed: bool,
    frames: Vec<EnsembleFrame>,
}
fn ensemble_metric(metric: &str) -> bool {
    matches!(metric, "500 mb heights and vorticity" | "Snowfall mean and spread"
        | "Snowfall 10th/90th percentile" | "Spaghetti 540 line"
        | "Member plumes (home)" | "Updraft helicity paintball")
}
fn ensemble_fields(metric: &str) -> [&str; 2] {
    match metric {
        "500 mb heights and vorticity" => ["500 mb height", "500 mb vorticity"],
        "Snowfall mean and spread" => ["Mean snowfall", "Snowfall spread"],
        "Snowfall 10th/90th percentile" => ["10th percentile snowfall", "90th percentile snowfall"],
        _ => ["", ""],
    }
}
fn ensemble_empty(s: Spec, scope: Scope) -> EnsembleInput {
    EnsembleInput { model: s.model.into(), metric: s.metric.into(), scope_name: s.scope.into(),
        scope, reviewed: false, frames: Vec::new() }
}
fn ensemble_valid(s: Spec, p: &EnsembleInput) -> bool {
    if !p.reviewed || p.model != s.model || p.metric != s.metric || p.scope_name != s.scope
        || !scope_matches(s, &p.scope) || p.frames.len() != if s.view == "4-panel timeline" { 4 } else { 1 } { return false; }
    if p.frames.iter().skip(1).any(|f|f.units!=p.frames[0].units || f.definition!=p.frames[0].definition){return false;}
    let mut member_colors=std::collections::HashMap::new();
    for frame in &p.frames {for member in &frame.members {if member_colors.insert(&member.member,&member.color).is_some_and(|old:&String|!old.eq_ignore_ascii_case(&member.color)){return false;}}}
    let snow=s.metric.starts_with("Snowfall ");
    if !ensemble_fields(s.metric)[0].is_empty() && p.frames.iter().any(|f| f.maps.len()!=2 || f.definition!=p.frames[0].definition || f.units!=p.frames[0].units || (snow && f.units!="in")) {return false;}
    if !ensemble_fields(s.metric)[0].is_empty() && p.frames.iter().skip(1).any(|f|f.maps.iter().zip(&p.frames[0].maps).any(|(a,b)| a.palette!=b.palette || a.legend_title!=b.legend_title || serde_json::to_string(&board::bands(a)).ok()!=serde_json::to_string(&board::bands(b)).ok())) {return false;}
    p.frames.iter().all(|f| {
        if f.valid.trim().is_empty() || f.valid.len() > 100 || f.definition.trim().is_empty()
            || f.definition.len() > 180 || f.units.trim().is_empty() || f.units.len() > 55 { return false; }
        let fields = ensemble_fields(s.metric);
        if !fields[0].is_empty() {
            return f.members.is_empty() && f.maps.len() == 2 && f.maps.iter().zip(fields).all(|(scene, field)| {
                scene.reviewed && (!snow || scene.palette==board::Palette::SnowTotal) && scene.panels.len() == 1 && scene.panels[0].available() && scene.panels[0].valid == f.valid
                && serde_json::to_string(&scene.scope).ok() == serde_json::to_string(&p.scope).ok()
                && scene.legend_title.contains(field) && !board::bands(scene).is_empty()
                && board::parse(&serde_json::to_string(scene).unwrap_or_default(), Layout::Single).is_some()
            });
        }
        f.maps.is_empty() && !f.members.is_empty() && f.members.len() <= 80
            && f.members.iter().map(|m| &m.member).collect::<std::collections::HashSet<_>>().len()==f.members.len()
            && f.members.iter().all(|m| {
            !m.member.trim().is_empty() && m.member.len() <= 60 && crate::gfx::parse_hex(&m.color).is_some()
            && !m.segments.is_empty() && m.segments.len() <= 100 && m.segments.iter().all(|seg| {
                (2..=4096).contains(&seg.len()) && (s.metric!="Member plumes (home)" || (seg.iter().all(|v|v[0]>=0.) && seg.windows(2).all(|w|w[1][0]>w[0][0]))) && seg.iter().all(|v| v.iter().all(|n| n.is_finite())
                    && (s.metric == "Member plumes (home)" || ((-180.0..=180.).contains(&v[0]) && (-85.0..=85.).contains(&v[1]))))
            })
        })
    })
}
fn ensemble_words(pm: &mut Pixmap, g: &mut GlyphCache, font: &str, value: &str, x: f32, y: f32, width: f32, size: f32) {
    use crate::gfx::text::{TextStyle, Weight, draw_line, measure};
    let mut st = TextStyle { family: font.into(), weight: Weight::SemiBold, size,
        color: tiny_skia::Color::WHITE, letter_spacing: 0. };
    let measured = measure(value, &st);
    if measured > width { st.size *= width / measured; }
    draw_line(pm, g, value, x, y + st.size, &st, 1., Some((0, 0, pm.width() as i32, pm.height() as i32)));
}
fn ensemble_path(pm: &mut Pixmap, pts: &[[f32; 2]], color: &str, width: f32) {
    let mut path = tiny_skia::PathBuilder::new();
    if let Some(p) = pts.first() { path.move_to(p[0], p[1]); }
    for p in pts.iter().skip(1) { path.line_to(p[0], p[1]); }
    if let Some(path) = path.finish() {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color(crate::gfx::parse_hex(color).unwrap_or(tiny_skia::Color::WHITE)); paint.anti_alias = true;
        pm.stroke_path(&path, &paint, &tiny_skia::Stroke { width, ..Default::default() }, Transform::identity(), None);
    }
}
fn ensemble_members(pm: &mut Pixmap, g: &mut GlyphCache, font: &str, s: Spec, p: &EnsembleInput, f: &EnsembleFrame, u:f32) {
    let w = pm.width() as f32; let h = pm.height() as f32;
    // Reserve distinct header, axis and member-credit bands at native size.
    let plot = (w * 0.08, 86.*u, w * 0.85, (h - 166.*u).max(20.));
    let plume = s.metric == "Member plumes (home)";
    let bounds = if plume {
        p.frames.iter().flat_map(|frame| &frame.members).flat_map(|m| &m.segments).flatten().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, v|
            [b[0].min(v[0]), b[1].min(v[1]), b[2].max(v[0]), b[3].max(v[1])])
    } else if let Some(b) = p.scope.bbox { b } else {
        crate::map::boundaries::counties_named().map(|all| board::selected(&all, &p.scope)
            .iter().flat_map(|c| &c.rings).flatten().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, v|
                [b[0].min(v.0), b[1].min(v.1), b[2].max(v.0), b[3].max(v.1)]))
            .unwrap_or([-125., 24., -66., 50.])
    };
    if bounds.iter().any(|v| !v.is_finite()) || bounds[0] > bounds[2] || bounds[1] > bounds[3] { return; }
    let dx = (bounds[2]-bounds[0]).max(0.001); let dy = (bounds[3]-bounds[1]).max(0.001);
    let scale = if plume { 1. } else { (plot.2 as f64 / (dx * 0.8)).min(plot.3 as f64 / dy) };
    let xy = |v: [f64;2]| -> [f32;2] {
        if plume { [plot.0 + ((v[0]-bounds[0])/dx) as f32 * plot.2, plot.1 + plot.3 - ((v[1]-bounds[1])/dy) as f32 * plot.3] }
        else { [plot.0+plot.2*0.5+((v[0]-(bounds[0]+bounds[2])*0.5)*0.8*scale) as f32,
            plot.1+plot.3*0.5-((v[1]-(bounds[1]+bounds[3])*0.5)*scale) as f32] }
    };
    if !plume {
        if let Some(all) = crate::map::boundaries::counties_named() {
            for county in board::selected(&all, &p.scope) {
                for ring in &county.rings { let points: Vec<_> = ring.iter().map(|v| xy([v.0,v.1])).collect(); ensemble_path(pm, &points, "#617A91", 0.6*u); }
            }
        }
    } else {
        ensemble_path(pm, &[[plot.0,plot.1],[plot.0,plot.1+plot.3],[plot.0+plot.2,plot.1+plot.3]], "#A7BED5", 1.5*u);
        ensemble_words(pm,g,font,&format!("{:.1}",bounds[3]),0.,plot.1,w*0.07,18.*u);
        ensemble_words(pm,g,font,&format!("{:.1}",bounds[1]),0.,plot.1+plot.3-22.*u,w*0.07,18.*u);
        ensemble_words(pm,g,font,&format!("{:.0} h",bounds[0]),plot.0,plot.1+plot.3+5.*u,w*0.25,18.*u);
        let end_label=format!("{:.0} h",bounds[2]);
        let end_style=crate::gfx::text::TextStyle{family:font.into(),weight:crate::gfx::text::Weight::SemiBold,size:18.*u,color:tiny_skia::Color::WHITE,letter_spacing:0.};
        let end_width=crate::gfx::text::measure(&end_label,&end_style).min(w*0.20);
        ensemble_words(pm,g,font,&end_label,plot.0+plot.2-end_width,plot.1+plot.3+5.*u,w*0.20,18.*u);
    }
    // No segment bridges a missing interval. Geographic paths are clipped to scope.
    let in_scope = |v: &[f64;2]| plume || (v[0]>=bounds[0] && v[0]<=bounds[2] && v[1]>=bounds[1] && v[1]<=bounds[3]);
    for m in &f.members { for seg in &m.segments {
        let mut run = Vec::new();
        for v in seg { if in_scope(v) { run.push(xy(*v)); } else { if run.len()>1 { ensemble_path(pm,&run,&m.color,2.*u); } run.clear(); } }
        if run.len()>1 { ensemble_path(pm,&run,&m.color,2.*u); }
    }}
    ensemble_words(pm,g,font,&format!("{} members shown • {}",f.members.len(),f.units),w*0.03,h-34.*u,w*0.94,20.*u);
}
fn ensemble_draw(pm: &mut Pixmap, g: &mut GlyphCache, local: Transform, mask: Option<&Mask>, w: f32,h: f32, raw: &str,s: Spec,opacity: f32,font: &str) -> bool {
    let dw=(w*local.sx.abs()).round().clamp(1.,4096.) as u32;let dh=(h*local.sy.abs()).round().clamp(1.,4096.) as u32;let u=dw as f32/w;
    let Some(mut out)=Pixmap::new(dw,dh)else{return true;};out.fill(tiny_skia::Color::from_rgba8(16,36,58,255));
    let payload=if raw.len()<=4_000_000{serde_json::from_str::<EnsembleInput>(raw).ok().filter(|p|ensemble_valid(s,p))}else{None};
    if let Some(p)=payload {
        if p.frames[0].maps.len()==2 {
            // Group by field: four timestamps share one legend, projection and units.
            let stack=s.portrait;let gap=16.*u;let cw=if stack{dw as f32}else{(dw as f32-gap)*0.5};let ch=if stack{(dh as f32-gap)*0.5}else{dh as f32};
            for field in 0..2 {let mut scene=p.frames[0].maps[field].clone();scene.panels=p.frames.iter().map(|f|f.maps[field].panels[0].clone()).collect();let x=if stack{0.}else{field as f32*(cw+gap)};let y=if stack{field as f32*(ch+gap)}else{0.};let layout=if p.frames.len()==4{"timeline"}else{"single"};
                board::draw(&mut out,g,Transform::from_translate(x,y).pre_scale(u,u),None,cw/u,(ch-48.*u)/u,&format!("set:board:{layout}:{}",serde_json::to_string(&scene).unwrap()),1.,font);
                ensemble_words(&mut out,g,font,&p.frames[0].definition,x+8.*u,y+ch-39.*u,cw-16.*u,18.*u);
            }
        } else {
            let count=p.frames.len();let cols=if count==4{2}else{1};let rows=count/cols;let cw=dw/cols as u32;let ch=dh/rows as u32;
            for(i,f)in p.frames.iter().enumerate(){let Some(mut cell)=Pixmap::new(cw,ch)else{continue;};cell.fill(tiny_skia::Color::from_rgba8(16,36,58,255));
                ensemble_words(&mut cell,g,font,&f.valid,12.*u,8.*u,cw as f32-24.*u,24.*u);
                ensemble_words(&mut cell,g,font,&f.definition,12.*u,42.*u,cw as f32-24.*u,19.*u);ensemble_members(&mut cell,g,font,s,&p,f,u);
                out.draw_pixmap((i%cols)as i32*cw as i32,(i/cols)as i32*ch as i32,cell.as_ref(),&tiny_skia::PixmapPaint::default(),Transform::identity(),None);
            }
        }
    }else{ensemble_words(&mut out,g,font,reference::MISSING,24.*u,dh as f32*0.45,dw as f32-48.*u,36.*u);}
    pm.draw_pixmap(0,0,out.as_ref(),&tiny_skia::PixmapPaint{opacity:opacity.clamp(0.,1.),..Default::default()},Transform::from_translate(local.tx,local.ty),mask);true
}
#[cfg(test)]
fn ensemble_sample(d:&mut GraphicDoc,s:Spec,settings:&Settings) {
    let scope=geo_scope(s,settings);let snow=s.metric.starts_with("Snowfall ");
    for phase in 0..if is_loop(s){4}else{1}{let mut input=ensemble_empty(s,scope.clone());input.reviewed=true;
        for frame in 0..if s.view=="4-panel timeline"{4}else{1}{let lead=if s.metric=="Updraft helicity paintball"{[12,24,36,48]}else{[18,36,54,72]}[if is_loop(s){phase}else if s.view=="4-panel timeline"{frame}else{3}];let plume=s.metric=="Member plumes (home)";
            let valid=if plume{format!("SAMPLE • Lead 0–{lead}h")}else{format!("SAMPLE • Lead +{lead}h")};let fields=ensemble_fields(s.metric);let mut maps=Vec::new();let mut members=Vec::new();
            let definition=if snow{"SAMPLE • Accumulation since initialization • Supplied ensemble statistic"}else if !fields[0].is_empty(){"SAMPLE • 500 hPa height and vorticity • Init Nov 24 00Z"}else if plume{"SAMPLE • 2 m temperature at home • Init Nov 24 00Z"}else if s.metric=="Spaghetti 540 line"{"SAMPLE • Supplied 540 dam height contours • Init Nov 24 00Z"}else{"SAMPLE • 0–3 km UH ≥75 m²/s² member paths • Not a forecast"};
            let units=if snow{"in"}else if plume{"°F"}else if s.metric=="Spaghetti 540 line"{"dam"}else if fields[0].is_empty(){"m²/s²"}else{"dam; 10^-5 s^-1"};
            if !fields[0].is_empty(){let all=crate::map::boundaries::counties_named().unwrap();
                for(field_index,field)in fields.into_iter().enumerate(){let mut sc=board::empty(Layout::Single);sc.reviewed=true;sc.scope=scope.clone();sc.palette=if snow{board::Palette::SnowTotal}else{board::Palette::Category};let unit=if snow{"in"}else if field_index==0{"dam"}else{"10^-5 s^-1"};sc.legend_title=format!("{field} • {unit} • SAMPLE");sc.legend=sample_additional_legend();
                    if !snow{for(i,band)in sc.legend.iter_mut().enumerate(){band.label=if field_index==0{format!("{}–{}",500+i*12,512+i*12)}else{format!("{}–{}",i*4,(i+1)*4)};}}
                    sc.panels[0].title=field.into();sc.panels[0].valid=valid.clone();sc.panels[0].detail="SAMPLE ONLY • Synthetic values • Not for broadcast".into();let bins=board::bands(&sc).len();sc.panels[0].counties=Some(board::selected(&all,&scope).iter().enumerate().map(|(i,c)|(c.fips.clone(),(i+phase+frame+field_index*3)%bins)).collect());maps.push(sc);
                }
            }else{let b=scope.bbox.unwrap_or([-88.,37.,-84.,42.]);for i in 0..6{let points=(0..if plume{lead/6+1}else{13}).map(|j|if plume{[j as f64*6.,40.+(j as f64*0.55).sin()*6.+i as f64*1.2+(j as f64*0.23+i as f64).cos()*2.]}else{[b[0]+(b[2]-b[0])*j as f64/12.,b[1]+(b[3]-b[1])*(0.2+i as f64*0.09+(j as f64*0.4+phase as f64*0.2).sin()*0.07)]}).collect();members.push(EnsemblePath{member:format!("SAMPLE member {}",i+1),color:sample_additional_legend()[i].color.clone(),segments:vec![points]});}}
            input.frames.push(EnsembleFrame{valid,definition:definition.into(),units:units.into(),maps,members});
        }
        let field=if is_loop(s){format!("payload_{phase}")}else{"payload".into()};d.data_overrides.push((format!("{}.{field}",prefix(s)),serde_json::to_string(&input).unwrap()));
    }
    d.data_overrides.push((format!("{}.subtitle",prefix(s)),format!("SAMPLE • {} • Synthetic ensemble",s.scope)));d.data_overrides.push((format!("{}.source",prefix(s)),"SAMPLE ONLY • Not for broadcast".into()));d.data_overrides.push((format!("{}.note",prefix(s)),"SAMPLE ONLY • Supplied fields/members • No graphic-side statistics".into()));
}

#[cfg(test)]
mod ensemble_repair_tests {
    use super::*;
    #[test]
    fn plume_frames_cannot_mix_units_initializations_or_member_colors(){
        let &s=SPECS.iter().find(|s|s.metric=="Member plumes (home)"&&s.view=="4-panel timeline").unwrap();let mut d=build(s.key).unwrap();ensemble_sample(&mut d,s,&Settings::default());let raw=&d.data_overrides.iter().find(|(k,_)|k==&format!("{}.payload",prefix(s))).unwrap().1;let p:EnsembleInput=serde_json::from_str(raw).unwrap();assert!(ensemble_valid(s,&p));
        let mut bad=p.clone();bad.frames[1].units="°C".into();assert!(!ensemble_valid(s,&bad));let mut bad=p.clone();bad.frames[1].definition="Different initialization".into();assert!(!ensemble_valid(s,&bad));let mut bad=p.clone();bad.frames[1].members[0].color="#FFFFFF".into();assert!(!ensemble_valid(s,&bad));let mut bad=p.clone();bad.frames[0].members[0].segments[0][1][0]=0.;assert!(!ensemble_valid(s,&bad));
    }
    #[test]
    fn snow_requires_inches_snow_palette_and_shared_bins(){for &s in SPECS.iter().filter(|s|s.metric.starts_with("Snowfall ")&&ensemble_metric(s.metric)&&s.view=="4-panel timeline"){let mut d=build(s.key).unwrap();ensemble_sample(&mut d,s,&Settings::default());let raw=&d.data_overrides.iter().find(|(k,_)|k==&format!("{}.payload",prefix(s))).unwrap().1;let p:EnsembleInput=serde_json::from_str(raw).unwrap();assert!(ensemble_valid(s,&p));let mut wrong=p.clone();wrong.frames[0].units="mm".into();assert!(!ensemble_valid(s,&wrong));let mut wrong=p.clone();wrong.frames[0].maps[0].palette=board::Palette::Category;assert!(!ensemble_valid(s,&wrong));let mut wrong=p.clone();wrong.frames[1].maps[0].legend_title.push_str(" changed bins");assert!(!ensemble_valid(s,&wrong));}}
}
