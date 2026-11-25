// Purpose-specific presentations and immutable synthetic QA fixtures. No fetching or weather derivation.
thread_local! { static CANVAS_SCALE: std::cell::Cell<f32> = const { std::cell::Cell::new(1.) }; }
fn draw_scale()->f32 { CANVAS_SCALE.with(|s|s.get()) }
fn scaled_rect(r:(f32,f32,f32,f32))->(f32,f32,f32,f32) {let u=draw_scale();(r.0*u,r.1*u,r.2*u,r.3*u)}
struct CanvasScale(f32);
impl CanvasScale {fn new(u:f32)->Self {Self(CANVAS_SCALE.with(|s|s.replace(u)))}}
impl Drop for CanvasScale {fn drop(&mut self){CANVAS_SCALE.with(|s|s.set(self.0));}}
fn purpose_subtitle(s:Spec)->String {let mut parts=Vec::new();if s.scope!="n/a"{parts.push(s.scope);}if s.period!="Supplied valid period" && !s.period.eq_ignore_ascii_case(s.scope){parts.push(s.period);}parts.join(" • ")}
fn spc_long_range(s:Spec)->bool {s.title.starts_with("SPC categorical outlook") && s.period=="Days 4-8"}
fn no_high_risk_period(s:Spec)->bool {(s.title.starts_with("SPC categorical outlook")&&s.period=="Day 3")||((s.title.starts_with("Flooding rain risk")||s.title.starts_with("Excessive rainfall"))&&matches!(s.period,"Day 4"|"Day 5"))}
fn presentation_title(s:Spec)->&'static str {if s.kind==Kind::Tornado{"Tornado confirmed"}else if spc_long_range(s){"SPC severe weather probabilities (Days 4–8)"}else if s.title.starts_with("Mardi Gras / Cinco de Mayo"){"Community holiday forecast"}else{s.title}}
fn ranked(s:Spec)->bool {s.title.starts_with("Top 10 weather events")}
fn daily_count(s:Spec)->usize {if dayparts(s){return 3;}s.title.split('-').next().and_then(|v|v.parse().ok()).unwrap_or(7)}
fn hourly_count(s:Spec)->Option<usize> {if s.kind!=Kind::Table || s.category!="Forecast by hour" {return None;}s.period.split_whitespace().find_map(|v|v.parse::<usize>().ok()).filter(|v|[6,12,24,48].contains(v))}
fn purpose_contract(s:Spec,f:&Frame)->bool {
    if no_high_risk_period(s) && f.scene.as_ref().is_none_or(|scene|board::bands(scene).iter().any(|b|b.label.eq_ignore_ascii_case("High"))){return false;}
    if spc_long_range(s) {let Some(scene)=&f.scene else{return false;};let bands=board::bands(scene);if bands.len()!=3 || bands.iter().map(|v|v.label.as_str()).collect::<Vec<_>>()!=["No delineated area","15%","30%"] || !bands[1].color.eq_ignore_ascii_case("#FFFF00") || !bands[2].color.eq_ignore_ascii_case("#E69800"){return false;}}
    if (s.kind==Kind::Daily || ranked(s) || hourly_count(s).is_some()) && (f.columns.is_empty() || f.rows.iter().any(Vec::is_empty)){return false;}
    if s.kind==Kind::Daily && (f.columns.len()!=6 || f.rows.len()!=daily_count(s) || f.rows.iter().any(|r|r[0].trim().is_empty()) || f.rows.iter().map(|r|&r[0]).collect::<std::collections::HashSet<_>>().len()!=f.rows.len()){return false;}
    if ranked(s) && (f.rows.len()!=10 || f.columns.len()!=4){return false;}
    if let Some(hours)=hourly_count(s) {if f.total_hours!=hours || f.page>=s.pages || f.first_hour!=f.page*8 || f.first_hour>=hours || f.rows.len()!=(hours-f.first_hour).min(8) || f.rows.iter().map(|r|&r[0]).collect::<std::collections::HashSet<_>>().len()!=f.rows.len(){return false;}}
    if s.title.starts_with("Hourly rain (") && (f.columns.len()!=4 || !f.columns[1].to_ascii_lowercase().contains("chance") || !f.columns[2].to_ascii_lowercase().contains("amount") || !f.amount_axis_max.is_finite() || f.amount_axis_max<=0. || f.rows.iter().any(|r|r[1].trim_end_matches('%').parse::<f32>().ok().is_none_or(|n|!n.is_finite() || !(0.0..=100.).contains(&n)) || r[2].parse::<f32>().ok().is_none_or(|n|!n.is_finite() || n<0. || n>f.amount_axis_max))){return false;}
    if s.kind==Kind::Tornado {let Some(scene)=f.scene.as_ref()else{return false;};if scene.palette!=board::Palette::Category || scene.legend.len()!=reference::EF.len() || scene.legend.iter().zip(reference::EF).any(|(a,(label,color))|a.label!=label || a.color!=color){return false;}}
    if s.kind==Kind::Tornado {if !(2..=256).contains(&f.track.len()) || !(1..=7).contains(&f.ahead.len()) || f.track.iter().any(|v|!v[0].is_finite()||!v[1].is_finite()||!(-180.0..=180.).contains(&v[0])||!(-85.0..=85.).contains(&v[1])) || f.ahead.iter().any(|v|v.label.trim().is_empty()||v.label.len()>90||!v.lon.is_finite()||!v.lat.is_finite()||!(-180.0..=180.).contains(&v.lon)||!(-85.0..=85.).contains(&v.lat)) {return false;}
        for name in ["Rating","Peak wind","Location"] {if !f.values.iter().any(|v|v.label==name && !v.value.trim().is_empty()){return false;}}
    }
    true
}
fn daily_cards(pm:&mut Pixmap,g:&mut GlyphCache,font:&str,f:&Frame,r:(f32,f32,f32,f32)) {
    let wide=r.2/r.3>2.;let cols=if wide{f.rows.len()}else if r.3>r.2{1}else{2};let rows=f.rows.len().div_ceil(cols);let cw=r.2/cols as f32;let ch=r.3/rows as f32;
    for (i,raw_row) in f.rows.iter().enumerate() {let display:Vec<String>=raw_row.iter().map(|v|if v.trim().is_empty(){reference::MISSING.into()}else{v.clone()}).collect();let row=&display;let x=r.0+(i%cols)as f32*cw;let y=r.1+(i/cols)as f32*ch;panel(pm,(x+3.,y+3.,cw-6.,ch-6.),"#19334E");
        text(pm,g,font,&row[0],(x+12.,y+ch*0.03,cw-24.,ch*0.16),if wide{28.}else{22.},"#70D5EF");
        let side=if wide{64.}else{(ch*0.29).min(48.)};let ix=if wide{x+(cw-side)*0.5}else{x+12.};let iy=y+ch*0.25;
        let u=draw_scale();super::city_forecast::draw_map(pm,Transform::from_translate(ix*u,iy*u),None,side*u,side*u,&format!("set:cityforecast-icon:{}",super::weather_word::icon_for(&row[1],false)),1.);
        if wide {
            for (value,fy,fh,size,c) in [(&row[1],0.40,0.12,25.,"#FFFFFF"),(&format!("{} {}",f.columns[2],row[2]),0.57,0.10,34.,"#FFFFFF"),(&format!("{} {}",f.columns[3],row[3]),0.68,0.09,29.,"#B9C9D8"),(&format!("Rain {}",row[4]),0.80,0.07,23.,"#70D5EF"),(&row[5],0.90,0.07,20.,"#B9C9D8")] {text(pm,g,font,value,(x+12.,y+ch*fy,cw-24.,ch*fh),size,c);}
        } else {
            text(pm,g,font,&row[1],(x+side+22.,y+ch*0.27,cw*0.51-side-22.,ch*0.31),22.,"#FFFFFF");
            text(pm,g,font,&if f.columns[2]=="Temperature"{format!("Temp {} • Feels {}",row[2],row[3])}else{format!("{} / {}",row[2],row[3])},(x+cw*0.54,y+ch*0.27,cw*0.43-10.,ch*0.30),27.,"#FFFFFF");
            text(pm,g,font,&format!("Rain {}",row[4]),(x+12.,y+ch*0.66,cw*0.45,ch*0.18),18.,"#70D5EF");
            text(pm,g,font,&row[5],(x+cw*0.49,y+ch*0.66,cw*0.48-10.,ch*0.26),18.,"#B9C9D8");
        }
    }
}
fn daily_strip(pm:&mut Pixmap,g:&mut GlyphCache,font:&str,f:&Frame,r:(f32,f32,f32,f32)) {
    let cw=r.2/f.rows.len()as f32;
    for(i,raw_row)in f.rows.iter().enumerate(){let display:Vec<String>=raw_row.iter().map(|v|if v.trim().is_empty(){reference::MISSING.into()}else{v.clone()}).collect();let row=&display;let x=r.0+i as f32*cw;panel(pm,(x+2.,r.1,cw-4.,r.3),"#19334E");text(pm,g,font,&row[0],(x+8.,r.1+2.,cw-16.,22.),18.,"#70D5EF");text(pm,g,font,&if f.columns[2]=="Temperature"{format!("Temp {} • Feels {}",row[2],row[3])}else{format!("{} / {}",row[2],row[3])},(x+8.,r.1+26.,cw-16.,29.),24.,"#FFFFFF");let u=draw_scale();super::city_forecast::draw_map(pm,Transform::from_translate((x+8.)*u,(r.1+56.)*u),None,22.*u,22.*u,&format!("set:cityforecast-icon:{}",super::weather_word::icon_for(&row[1],false)),1.);text(pm,g,font,&row[1],(x+35.,r.1+55.,cw-43.,24.),16.,"#FFFFFF");text(pm,g,font,&format!("Rain {} • {}",row[4],row[5]),(x+8.,r.1+81.,cw-16.,(r.3-83.).max(15.)),14.,"#B9C9D8");}
}
fn hourly_strip(pm:&mut Pixmap,g:&mut GlyphCache,font:&str,f:&Frame,r:(f32,f32,f32,f32)) {
    // Every supplied hour on the current page remains visible in compact formats.
    // Keep the compact columns in the same chronological order as full-screen rows.
    let cw=r.2/f.rows.len().max(1) as f32;
    for (i,row) in f.rows.iter().enumerate() {
        let x=r.0+i as f32*cw;
        panel(pm,(x+2.,r.1,cw-4.,r.3),if i%2==0{"#19334E"}else{"#142B43"});
        let rh=r.3/row.len().max(1) as f32;
        for (j,value) in row.iter().enumerate() {
            let label=f.columns.get(j).map(String::as_str).unwrap_or("");
            let value=if value.trim().is_empty(){reference::MISSING}else{value};
            let line=if j==0{value.to_string()}else{format!("{label}: {value}")};
            text(pm,g,font,&line,(x+7.,r.1+j as f32*rh,cw-14.,rh-1.),if j==0{18.}else{16.},if j==0{"#70D5EF"}else{"#FFFFFF"});
        }
    }
}
fn hourly_bars(pm:&mut Pixmap,s:Spec,f:&Frame,row:&[String],r:(f32,f32,f32,f32)) {
    if !s.title.starts_with("Hourly rain ("){return;}let cw=r.2/f.columns.len()as f32;
    if let Some(pop)=row.get(1).and_then(|v|v.trim_end_matches('%').parse::<f32>().ok()){panel(pm,(r.0+cw+6.,r.1+4.,(cw-12.)*pop.clamp(0.,100.)/100.,r.3-8.),"#214F72");}
    if let Some(amount)=row.get(2).and_then(|v|v.parse::<f32>().ok()){let (_,rgb)=crate::grid::RAIN_TOTAL_STOPS.iter().copied().filter(|(min,_)|amount>=*min).last().unwrap_or(crate::grid::RAIN_TOTAL_STOPS[0]);let ink=format!("#{:02X}{:02X}{:02X}",rgb[0],rgb[1],rgb[2]);panel(pm,(r.0+cw*2.+6.,r.1+r.3-11.,(cw-12.)*amount/f.amount_axis_max,8.),&ink);}
}
fn path_line(pm:&mut Pixmap,pts:&[[f32;2]],hex:&str,width:f32,clip:Option<&Mask>) {
    let mut b=tiny_skia::PathBuilder::new();if let Some(p)=pts.first(){b.move_to(p[0],p[1]);}for p in pts.iter().skip(1){b.line_to(p[0],p[1]);}
    if let Some(path)=b.finish(){let mut ink=Paint::default();ink.set_color(color(hex));ink.anti_alias=true;pm.stroke_path(&path,&ink,&tiny_skia::Stroke{width,..Default::default()},Transform::from_scale(draw_scale(),draw_scale()),clip);}
}
fn tornado_callout(pm:&mut Pixmap,g:&mut GlyphCache,font:&str,f:&Frame,r:(f32,f32,f32,f32)) {
    let Some(sc)=&f.scene else{return;};let small=r.3<200.;let portrait=r.3>r.2;let map=if small{(r.0,r.1,r.2*0.25,r.3)}else if portrait{(r.0,r.1,r.2,r.3*0.57)}else{(r.0,r.1,r.2*0.64,r.3)};let card=if small{(r.0+r.2*0.27,r.1,r.2*0.73,r.3)}else if portrait{(r.0,r.1+r.3*0.59,r.2,r.3*0.41)}else{(r.0+r.2*0.66,r.1,r.2*0.34,r.3)};
    panel(pm,map,"#10263D");panel(pm,card,"#19334E");let Some(all)=crate::map::boundaries::counties_named()else{return;};let selected=board::selected(&all,&sc.scope);let mut b=[f64::MAX,f64::MAX,f64::MIN,f64::MIN];
    for c in &selected{for ring in &c.rings{for &(x,y) in ring{b=[b[0].min(x),b[1].min(y),b[2].max(x),b[3].max(y)];}}}if b[0]>=b[2]||b[1]>=b[3]{return;}
    for v in f.track.iter().copied().chain(f.ahead.iter().map(|p|[p.lon,p.lat])){b=[b[0].min(v[0]),b[1].min(v[1]),b[2].max(v[0]),b[3].max(v[1])];}let pad_x=(b[2]-b[0])*0.08;let pad_y=(b[3]-b[1])*0.08;b=[b[0]-pad_x,b[1]-pad_y,b[2]+pad_x,b[3]+pad_y];
    let mut map_mask=Mask::new(pm.width(),pm.height()).unwrap();let mr=scaled_rect(map);let mask_path=tiny_skia::PathBuilder::from_rect(Rect::from_xywh(mr.0,mr.1,mr.2,mr.3).unwrap());map_mask.fill_path(&mask_path,tiny_skia::FillRule::Winding,true,Transform::identity());let clip=Some(&map_mask);
    let k=((b[1]+b[3])*0.5).to_radians().cos();let scale=((map.2-64.)as f64/((b[2]-b[0])*k)).min((map.3-if small{16.}else{90.})as f64/(b[3]-b[1]));let xy=|p:[f64;2]|[map.0+map.2*0.5+((p[0]-(b[0]+b[2])*0.5)*k*scale)as f32,map.1+map.3*0.5-((p[1]-(b[1]+b[3])*0.5)*scale)as f32];
    for c in selected {for ring in &c.rings {let pts:Vec<_>=ring.iter().map(|&(x,y)|xy([x,y])).collect();path_line(pm,&pts,"#6E8AA4",1.,clip);}}
    for point in sc.panels[0].points.iter().flatten(){let v=xy([point.lon,point.lat]);if let Some(circle)=tiny_skia::PathBuilder::from_circle(v[0],v[1],if small{4.}else{8.}){let mut ink=Paint::default();ink.set_color(color(reference::EF[point.category.min(6)].1));pm.fill_path(&circle,&ink,tiny_skia::FillRule::Winding,Transform::from_scale(draw_scale(),draw_scale()),clip);}}
    let pts:Vec<_>=f.track.iter().map(|&p|xy(p)).collect();path_line(pm,&pts,"#FFDA67",if small{3.}else{5.},clip);let end=pts[pts.len()-1];let prev=pts[pts.len()-2];let angle=(end[1]-prev[1]).atan2(end[0]-prev[0]);let arrow_size=if small{8.}else{18.};let arrow=[[end[0]-arrow_size*(angle-0.5).cos(),end[1]-arrow_size*(angle-0.5).sin()],end,[end[0]-arrow_size*(angle+0.5).cos(),end[1]-arrow_size*(angle+0.5).sin()]];path_line(pm,&arrow,"#FFDA67",if small{3.}else{5.},clip);
    let mut occupied:Vec<(f32,f32,f32,f32)>=f.ahead.iter().map(|p|{let v=xy([p.lon,p.lat]);(v[0]-6.,v[1]-6.,12.,12.)}).collect();
    occupied.push((map.0,map.1,map.2,if small{0.}else{44.}));if !small && r.3<300.{occupied.push((map.0,map.1+map.3-27.,map.2,27.));}
    for p in &f.ahead {let v=xy([p.lon,p.lat]);if v[0]<map.0||v[0]>map.0+map.2||v[1]<map.1||v[1]>map.1+map.3{continue;}panel(pm,(v[0]-2.,v[1]-2.,4.,4.),"#FFFFFF");if small{continue;}
        let label_width=260_f32.min(map.2*0.45);let mut chosen=None;
        for dy in [-22.,22.,-66.,66.,-110.,110.] {for dx in [12.,-label_width-12.] {let box2=((v[0]+dx).clamp(map.0+8.,map.0+map.2-label_width-8.),(v[1]+dy).clamp(map.1+46.,map.1+map.3-50.),label_width,44.);if !occupied.iter().any(|p|box2.0<p.0+p.2+4.&&box2.0+box2.2+4.>p.0&&box2.1<p.1+p.3+4.&&box2.1+box2.3+4.>p.1){chosen=Some(box2);break;}}if chosen.is_some(){break;}}
        if let Some(box2)=chosen {path_line(pm,&[v,[box2.0,box2.1+22.]],"#9AB4C9",1.,clip);panel(pm,box2,"#10263D");text(pm,g,font,&p.label,box2,20.,"#FFFFFF");occupied.push(box2);}
    }
    if small {
        let value=|label:&str|f.values.iter().find(|v|v.label==label).map(|v|v.value.as_str()).unwrap_or(reference::MISSING);
        text(pm,g,font,"Tornado confirmed",(card.0+12.,card.1+2.,card.2*0.35,29.),26.,"#FFDA67");text(pm,g,font,&f.valid,(card.0+card.2*0.39,card.1+3.,card.2*0.59-12.,26.),17.,"#B9C9D8");
        text(pm,g,font,&format!("{} • {} • {}",value("Location"),value("Rating"),value("Peak wind")),(card.0+12.,card.1+32.,card.2-24.,31.),24.,"#FFFFFF");
        let towns=f.ahead.iter().take(2).map(|p|p.label.as_str()).collect::<Vec<_>>().join(" • ");let rest=if f.ahead.len()>2{format!(" • +{} more",f.ahead.len()-2)}else{String::new()};
        text(pm,g,font,&format!("Towns ahead: {towns}{rest}"),(card.0+12.,card.1+69.,card.2-24.,(card.3-71.).max(24.)),18.,"#B9C9D8");return;
    }
    if r.3<300.{text(pm,g,font,&f.valid,(map.0+10.,map.1+map.3-25.,map.2-20.,23.),16.,"#B9C9D8");}
    text(pm,g,font,"Tornado path and towns ahead",(map.0+12.,map.1+8.,map.2-24.,34.),22.,"#B9C9D8");
    text(pm,g,font,"Tornado confirmed",(card.0+16.,card.1+10.,card.2-32.,card.3*0.12),33.,"#FFDA67");let count=f.values.len();let rowh=(card.3*0.82/count.max(1)as f32).min(110.);
    for(i,v)in f.values.iter().enumerate(){let y=card.1+card.3*0.16+i as f32*rowh;text(pm,g,font,&v.label,(card.0+16.,y,card.2-32.,rowh*0.33),20.,"#70D5EF");text(pm,g,font,if v.value.trim().is_empty(){reference::MISSING}else{&v.value},(card.0+16.,y+rowh*0.35,card.2-32.,rowh*0.60),28.,"#FFFFFF");}
}
#[cfg(test)]
fn purpose_sample(s:Spec,f:&mut Frame,page:usize,panel_index:usize,scope:&board::Scope) {
    if s.kind==Kind::Daily {let start=chrono::NaiveDate::from_ymd_opt(2026,11,24).unwrap();f.columns=["Date","Conditions","High","Low","Rain chance","Wind"].into_iter().map(str::to_string).collect();f.values.clear();f.bullets.clear();f.unit="°F • % • mph".into();f.heading="SAMPLE • County representative-point forecast".into();f.rows=(0..daily_count(s)).map(|i|vec![(start+chrono::Duration::days(i as i64)).format("%a %b %d").to_string(),["Mostly sunny","Rain showers","Partly cloudy"][i%3].into(),format!("{}°F",47+i%5),format!("{}°F",31+i%4),format!("{}%",[10,60,20][i%3]),format!("SW {}–{} mph",5+i%4,10+i%6)]).collect();f.valid=format!("SAMPLE • {}–{}, 2026",start.format("%b %d"),(start+chrono::Duration::days(daily_count(s)as i64-1)).format("%b %d"));f.note="SAMPLE ONLY • High/low °F • County conditions vary".into();return;}
    if s.title=="Current conditions (Now)" {f.heading="SAMPLE • Home-area station observation".into();f.note="SAMPLE ONLY • Designated station conditions; conditions vary across the county".into();f.unit="°F • % • mph".into();f.bullets.clear();f.values=[("Conditions","Partly cloudy"),("Temperature","47°F"),("Feels like","43°F"),("Humidity","62%"),("Dew point","35°F"),("Wind","SW 8 mph • Gust 15 mph")].into_iter().map(|(a,b)|Datum{label:a.into(),value:b.into()}).collect();f.valid="SAMPLE • Nov 24, 14:00 UTC".into();}
    if ranked(s) {f.columns=["Rank","Date","Event","Location"].into_iter().map(str::to_string).collect();f.values.clear();f.rows=(0..10).map(|i|vec![(i+1).to_string(),format!("SAMPLE 20{:02}",10+i),["Winter storm","Flooding rain","Damaging wind","Significant snowfall"][i%4].into(),["Northeast Sample County","Sample County","Oakdale"][i%3].into()]).collect();f.heading="SAMPLE • Producer-ranked county weather events".into();}
    if let Some(hours)=hourly_count(s) {f.page=page;f.first_hour=page*8;f.total_hours=hours;f.values.clear();if s.title.starts_with("Hourly rain ("){f.amount_axis_max=0.10;f.columns=["Hour UTC","Rain chance","Amount (in)","Conditions"].into_iter().map(str::to_string).collect();}
        let start=chrono::DateTime::parse_from_rfc3339("2026-11-24T00:00:00Z").unwrap();f.rows=(f.first_hour..(f.first_hour+8).min(hours)).map(|h|f.columns.iter().enumerate().map(|(j,label)|{if j==0{return (start+chrono::Duration::hours(h as i64)).format("%b %d %HZ").to_string();}let key=label.to_ascii_lowercase();if key.contains("chance"){format!("{}%",(h%8)*10)}else if key.contains("amount"){format!("{:.2}",(h%7)as f32*0.01)}else if key.contains("temperature")||key.contains("feels"){format!("{}°F",38+h%9)}else if key.contains("wind"){format!("SW {} mph",5+h%10)}else if key.contains("humidity"){format!("{}%",50+h%30)}else if key.contains("cloud"){format!("{}%",20+h%70)}else{"Partly cloudy".into()}}).collect()).collect();f.valid=format!("SAMPLE • Page {}/{} • {}–{}",page+1,s.pages,(start+chrono::Duration::hours(f.first_hour as i64)).format("%b %d %HZ"),(start+chrono::Duration::hours(((f.first_hour+8).min(hours)-1)as i64)).format("%b %d %HZ"));f.note=if s.title.starts_with("Hourly rain ("){"SAMPLE ONLY • Distinct hourly records • Amount bars: 0–0.10 in"}else{"SAMPLE ONLY • Distinct hourly records • °F / % / mph"}.into();}
    let title=s.title.to_ascii_lowercase();let Some(sc)=f.scene.as_mut()else{return;};
    let palette:Option<Vec<(&str,&str)>>=if title.contains("flooding rain risk")||title.contains("excessive rainfall"){Some(vec![("Below marginal","#23435B"),("Marginal","#00CC00"),("Slight","#FFFF00"),("Moderate","#FF0000"),("High","#FF00FF")])}else if spc_long_range(s){Some(vec![("No delineated area","#23435B"),("15%","#FFFF00"),("30%","#E69800")])}else if title.contains("categorical outlook"){Some(vec![("General thunder","#C0E8C0"),("Marginal","#66A366"),("Slight","#FFE066"),("Enhanced","#FFA347"),("Moderate","#F05050"),("High","#FF66FF")])}else if title.contains("flight conditions"){Some(vec![("VFR","#00CC66"),("MVFR","#3399FF"),("IFR","#FF3333"),("LIFR","#FF33FF")])}else if title.contains("lightning strikes"){Some(vec![("Past 5 minutes","#FFF066"),("5–10 minutes","#FFA640"),("10–15 minutes","#FF5544")])}else if title.contains("active alerts map"){Some(vec![("Winter storm warning","#FF69B4"),("Winter weather advisory","#7B68EE"),("Ice storm warning","#8B008B"),("Blizzard warning","#FF4500")])}else if title.contains("statewide climate rankings"){Some(vec![("Rank 1","#294DB8"),("Ranks 2–13","#3C88D4"),("Ranks 14–40","#79BEDC"),("Ranks 41–92","#D9E2DC"),("Ranks 93–119","#EDC468"),("Ranks 120–131","#E87E45"),("Rank 132","#C83F3F")])}else if s.kind==Kind::Compare{Some(vec![("-30°F","#334DA8"),("-25°F","#416CC4"),("-20°F","#5598D4"),("-15°F","#70BACA"),("-10°F","#A7D5C8"),("-5°F","#D4E3C1"),("0°F","#EEE6BA")])}else{None};
    if title.contains("statewide climate rankings"){let month=s.title.split('(').nth(1).unwrap_or("Month)").trim_end_matches(')');f.valid=format!("SAMPLE • {month} 2026");sc.panels[0].valid=f.valid.clone();f.unit="Rank / 132 years".into();f.note="SAMPLE ONLY • Supplied monthly temperature ranks • Baseline 1895–2026".into();}
    if let Some(mut pal)=palette {if no_high_risk_period(s){pal.retain(|(label,_)|*label!="High");}sc.palette=board::Palette::Category;sc.legend=pal.into_iter().map(|(label,color)|board::Band{label:label.into(),color:color.into()}).collect();sc.legend_title=if spc_long_range(s){"SAMPLE • Severe weather within 25 miles • Probability (%)".into()}else if title.contains("climate rankings"){"SAMPLE • Monthly rank • 1895–2026 baseline".into()}else if s.kind==Kind::Compare{"SAMPLE threshold • °F • Not operational criteria".into()}else if title.contains("lightning"){"Strike age • minutes".into()}else if title.contains("flight"){"Flight category • Ceiling ft AGL / visibility miles".into()}else{"SAMPLE • Supplied official product categories".into()};let bins=sc.legend.len();if let Some(all)=crate::map::boundaries::counties_named(){sc.panels[0].counties=Some(board::selected(&all,scope).iter().enumerate().map(|(j,c)|(c.fips.clone(),if title.contains("climate rankings"){c.state.parse::<usize>().unwrap_or(0)%bins}else if s.kind==Kind::Compare{if panel_index==0{4}else{1}}else{j%bins})).collect());}
        sc.panels[0].detail="SAMPLE ONLY • Synthetic product-specific geometry and categories".into();
        if title.contains("flooding rain risk")||title.contains("excessive rainfall")||title.contains("categorical outlook") {
            if let Some(all)=crate::map::boundaries::counties_named(){let selected=board::selected(&all,scope);let b=selected.iter().flat_map(|c|&c.rings).flatten().fold([f64::MAX,f64::MAX,f64::MIN,f64::MIN],|b,p|[b[0].min(p.0),b[1].min(p.1),b[2].max(p.0),b[3].max(p.1)]);
                if b[0]<b[2]&&b[1]<b[3]{let dx=b[2]-b[0];let dy=b[3]-b[1];sc.panels[0].counties=None;sc.panels[0].zones=Some((1..bins).map(|i|{let inset=i as f64*0.07;board::Zone{category:i,ring:vec![(b[0]+dx*inset,b[1]+dy*inset),(b[2]-dx*inset,b[1]+dy*(inset+0.04)),(b[2]-dx*(inset+0.05),b[3]-dy*inset),(b[0]+dx*(inset+0.03),b[3]-dy*(inset+0.05))],label:String::new(),label_at:None}}).collect());}
            }
        }
        if title.contains("flight conditions")||title.contains("lightning strikes") {sc.panels[0].counties=None;sc.panels[0].points=Some([("KHNB",-86.9537,38.2489),("KEVV",-87.5324,38.0370),("KSDF",-85.736,38.174),("KBMG",-86.6167,39.146)].into_iter().enumerate().map(|(i,(id,lon,lat))|board::ReportPoint{id:format!("SAMPLE-{id}"),lon,lat,label:format!("SAMPLE {id} • {}",sc.legend[i%bins].label),category:i%bins,value:None}).collect());}
        if s.kind==Kind::Compare {sc.panels[0].title=if panel_index==0{"Advisory threshold"}else{"Warning threshold"}.into();sc.panels[0].detail="SAMPLE ONLY • Layout thresholds • Production uses office-published criteria".into();}
    }
    if spc_long_range(s) {f.heading="SAMPLE • SPC Day 4 probabilistic outlook".into();f.unit="%".into();sc.panels[0].title="SPC Day 4 severe weather probability".into();sc.panels[0].detail="SAMPLE ONLY • Probability within 25 miles • Supplied Day 4 valid period".into();}
    if s.kind==Kind::Tornado {f.heading="SAMPLE • Sample County survey".into();f.valid="SAMPLE • Nov 24, 19:30 UTC".into();sc.panels[0].valid=f.valid.clone();f.values=[("Location","SAMPLE • Near Oakdale"),("Rating","SAMPLE EF2"),("Peak wind","SAMPLE 120 mph"),("Survey status","SAMPLE confirmed"),("Event time","SAMPLE Nov 24 • 19:30 UTC")].into_iter().map(|(a,b)|Datum{label:a.into(),value:b.into()}).collect();f.track=vec![[-87.02,38.40],[-86.98,38.35],[-86.94,38.29],[-86.90,38.25]];f.ahead=vec![board::ReportPoint{id:"SAMPLE-town1".into(),lon:-86.862,lat:38.225,label:"SAMPLE Millbrook".into(),category:0,value:None},board::ReportPoint{id:"SAMPLE-town2".into(),lon:-86.809,lat:38.170,label:"SAMPLE Saint Meinrad".into(),category:0,value:None}];sc.legend=reference::EF.iter().map(|(label,color)|board::Band{label:(*label).into(),color:(*color).into()}).collect();sc.panels[0].points=Some(vec![board::ReportPoint{id:"SAMPLE-tornado".into(),lon:-86.94,lat:38.29,label:"SAMPLE EF2".into(),category:2,value:None}]);sc.panels[0].counties=None;}
}

#[cfg(test)]
mod repair_contract_tests {
    use super::*;
    #[test]
    fn long_lead_categories_do_not_allow_high_risk(){for &s in SPECS.iter().filter(|s|no_high_risk_period(**s)){let p=inputs(s);let scene=p[0].frames[0].scene.as_ref().unwrap();assert!(!board::bands(scene).iter().any(|b|b.label=="High"));let mut wrong=p[0].clone();wrong.frames[0].scene.as_mut().unwrap().legend.push(board::Band{label:"High".into(),color:"#FF00FF".into()});assert!(checked(s,&serde_json::to_string(&wrong).unwrap()).is_none());}}
    fn inputs(s:Spec)->Vec<Input>{let mut d=build(s.key).unwrap();sample(&mut d);(0..s.pages).map(|page|{let field=if s.pages>1{format!("payload_{page}")}else{"payload".into()};let raw=&d.data_overrides.iter().find(|(k,_)|k==&format!("{}.{field}",prefix(s))).unwrap().1;checked(s,raw).expect(s.key)}).collect()}
    #[test]
    fn forecast_days_and_ranked_events_have_complete_distinct_rows(){for &s in SPECS.iter().filter(|s|s.kind==Kind::Daily||ranked(**s)){let p=inputs(s);assert_eq!(p[0].frames[0].rows.len(),if ranked(s){10}else{daily_count(s)});let mut wrong=p[0].clone();wrong.frames[0].rows.pop();assert!(checked(s,&serde_json::to_string(&wrong).unwrap()).is_none());}}
    #[test]
    fn hourly_pages_cover_every_hour_once(){for &s in SPECS.iter().filter(|s|hourly_count(**s).is_some()){let inputs=inputs(s);let rows:Vec<_>=inputs.iter().flat_map(|p|&p.frames[0].rows).collect();assert_eq!(rows.len(),hourly_count(s).unwrap(),"{}",s.key);assert_eq!(rows.iter().map(|r|&r[0]).collect::<std::collections::HashSet<_>>().len(),rows.len(),"{}",s.key);for (i,p)in inputs.iter().enumerate(){assert_eq!(p.frames[0].page,i);assert_eq!(p.frames[0].first_hour,i*8);}}}
    #[test]
    fn tornado_requires_supplied_path_towns_and_callout_values(){let &s=SPECS.iter().find(|s|s.kind==Kind::Tornado).unwrap();let p=inputs(s).remove(0);let mut palette_mismatch=p.clone();palette_mismatch.frames[0].scene.as_mut().unwrap().palette=board::Palette::SnowTotal;assert!(checked(s,&serde_json::to_string(&palette_mismatch).unwrap()).is_none());for field in 0..3{let mut wrong=p.clone();match field{0=>wrong.frames[0].track.clear(),1=>wrong.frames[0].ahead.clear(),_=>wrong.frames[0].values.clear()}assert!(checked(s,&serde_json::to_string(&wrong).unwrap()).is_none());}}
    #[test]
    fn official_category_fixtures_use_real_classes_and_point_products(){for (needle,count)in [("Flight conditions",4),("Flooding rain risk",5)]{let &s=SPECS.iter().find(|s|s.title.starts_with(needle)).unwrap();let p=inputs(s);let scene=p[0].frames[0].scene.as_ref().unwrap();assert_eq!(board::bands(scene).len(),count);if count==5{assert!(scene.panels[0].zones.is_some());assert!(scene.panels[0].counties.is_none());}assert!(!scene.legend.iter().any(|b|b.label.contains("SAMPLE class")));if count==4{assert!(scene.panels[0].points.is_some());assert!(scene.panels[0].counties.is_none());}}}
    #[test]
    fn long_range_spc_uses_probabilities_instead_of_categorical_risks(){
        for &s in SPECS.iter().filter(|s|spc_long_range(**s)) {
            let p=inputs(s);let bands=board::bands(p[0].frames[0].scene.as_ref().unwrap());
            assert_eq!(bands.iter().map(|b|b.label.as_str()).collect::<Vec<_>>(),["No delineated area","15%","30%"]);
            let mut wrong=p[0].clone();wrong.frames[0].scene.as_mut().unwrap().legend[1].label="Marginal".into();
            assert!(checked(s,&serde_json::to_string(&wrong).unwrap()).is_none());
        }
    }

}

#[cfg(test)]
mod scale_review {
    use super::*;
    #[test]
    fn qa_repaired_scales(){
        let Some(dir)=std::env::var_os("CWX_QA_SCALE_DIR")else{return;};let Some(keys)=super::super::qa_selection::selected()else{return;};let out=std::path::PathBuf::from(dir);std::fs::create_dir_all(&out).unwrap();
        for key in keys.iter(){for look in ["modern","classic"]{let Some(mut doc)=crate::gfx::templates::build_in(key,look)else{continue;};super::super::tests::sample(&mut doc);for mode in ["preview","4k"]{let width=if mode=="preview"{480}else if doc.height>doc.width{2160}else{3840};let height=(doc.height as f64*width as f64/doc.width as f64).round()as u32;let pm=crate::program::render_graphic_mode(&doc,&Settings::default(),&crate::data::WxSnapshot::default(),10000.,width,height,None,crate::gfx::check::PreviewData::Sample);pm.save_png(out.join(format!("{key}-{look}-{mode}.png"))).unwrap();}}}
    }
}
