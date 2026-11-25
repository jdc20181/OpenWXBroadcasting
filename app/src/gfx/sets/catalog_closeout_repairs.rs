// Presentation-only closeout repairs. All estimates, periods and weather remain supplied.
fn snow_range(s:Spec)->bool {s.title.starts_with("Snow range (low / likely / high)")}
fn dayparts(s:Spec)->bool {s.title.starts_with("Morning / afternoon / evening")}
fn cloud_hours(s:Spec)->Option<usize> {if !s.title.starts_with("Cloud cover timeline"){return None;}s.period.split_whitespace().find_map(|v|v.parse().ok())}
fn closeout_contract(s:Spec,f:&Frame)->bool {
    if s.title=="Snow timeline" && ["Starts","Peaks","Ends"].iter().any(|label|!f.values.iter().any(|v|v.label==*label&&!v.value.trim().is_empty())){return false;}
    if snow_range(s) {
        if f.unit!="in" || f.columns.len()!=4 || f.rows.is_empty() || !f.amount_axis_max.is_finite() || f.amount_axis_max<=0. {return false;}
        for row in &f.rows {
            if row.len()!=4 || row[0].trim().is_empty(){return false;}
            let v:Option<Vec<f32>>=row[1..].iter().map(|v|v.parse::<f32>().ok()).collect();let Some(v)=v else{return false;};
            if v.iter().any(|v|!v.is_finite()||*v<0.||*v>f.amount_axis_max)||v[0]>v[1]||v[1]>v[2]{return false;}
        }
    }
    if let Some(hours)=cloud_hours(s) {
        if f.series.len()!=1 || f.series[0].unit!="%" || f.series[0].labels.len()!=hours || f.series[0].labels.iter().collect::<std::collections::HashSet<_>>().len()!=hours || f.series[0].values.iter().flatten().any(|v|!(0.0..=100.).contains(v)){return false;}
    }
    true
}
fn snow_range_draw(pm:&mut Pixmap,g:&mut GlyphCache,font:&str,f:&Frame,r:(f32,f32,f32,f32),compact:bool) {
    let n=f.rows.len().max(1);let cw=if compact{r.2/n as f32}else{r.2};let ch=if compact{r.3}else{r.3/n as f32};
    for (i,row) in f.rows.iter().enumerate(){let x=r.0+if compact{i as f32*cw}else{0.};let y=r.1+if compact{0.}else{i as f32*ch};
        panel(pm,(x+2.,y+2.,cw-4.,ch-4.),if i%2==0{"#19334E"}else{"#142B43"});
        text(pm,g,font,&row[0],(x+10.,y+3.,cw-20.,ch*0.26),if compact{18.}else{27.},"#FFFFFF");
        let caption=format!("Low {} • Likely {} • High {} in",row[1],row[2],row[3]);
        text(pm,g,font,&caption,(x+10.,y+ch*0.29,cw-20.,ch*0.28),if compact{16.}else{24.},"#B9C9D8");
        let v:Vec<f32>=row[1..].iter().map(|v|v.parse().unwrap_or(0.)).collect();let left=x+16.;let width=cw-32.;let axis=y+ch*0.79;
        panel(pm,(left,axis,width,2.),"#48657D");
        let rgb=crate::grid::snow_total_rgb(v[1]);let range_color=format!("#{:02X}{:02X}{:02X}",rgb[0],rgb[1],rgb[2]);
        panel(pm,(left+width*v[0]/f.amount_axis_max,axis-3.,(width*(v[2]-v[0])/f.amount_axis_max).max(2.),8.),&range_color);
        let dot_x=left+width*v[1]/f.amount_axis_max;panel(pm,(dot_x-4.,axis-8.,8.,18.),"#FFFFFF");
    }
}
#[cfg(test)]
fn closeout_sample(s:Spec,f:&mut Frame,frame:usize) {
    if snow_range(s){f.scene=None;f.values.clear();f.columns=["Town","Low (in)","Likely (in)","High (in)"].map(str::to_string).to_vec();
        let towns=if s.scope=="Point (home)"{vec!["SAMPLE Riverton"]}else{vec!["SAMPLE Riverton","SAMPLE Oakdale","SAMPLE Millbrook","SAMPLE Cedar Hill","SAMPLE Pine Grove"]};
        f.rows=towns.iter().enumerate().map(|(i,t)|vec![(*t).into(),["0.5","1.0","1.5","2.0","1.0"][i].into(),["2.0","3.0","4.0","5.0","3.0"][i].into(),["4.0","5.0","6.0","7.0","5.0"][i].into()]).collect();f.valid=format!("SAMPLE • Nov 25 00Z • {}",s.period);f.unit="in".into();f.amount_axis_max=8.;f.heading="SAMPLE • Supplied low / likely / high estimates".into();f.note="SAMPLE ONLY • Snow range in inches • Estimates supplied, not calculated • Axis 0–8 in".into();}
    if dayparts(s){f.values.clear();f.scene=None;f.columns=["Day part","Conditions","Temperature","Feels like","Rain chance","Wind"].map(str::to_string).to_vec();
        f.rows=[("Morning","Mostly sunny","38°F","34°F","10%","SW 5 mph"),("Afternoon","Partly cloudy","49°F","46°F","20%","SW 9 mph"),("Evening","Rain showers","43°F","39°F","60%","SW 7 mph")].iter().map(|r|[r.0,r.1,r.2,r.3,r.4,r.5].map(str::to_string).to_vec()).collect();f.valid=format!("SAMPLE • Nov {}",if s.period=="Tomorrow"{26}else if s.period=="Day 3"{27}else{25});f.heading=format!("SAMPLE • {} • Representative-point day parts",s.period);f.note="SAMPLE ONLY • Temperatures for each day part • County conditions vary".into();}
    if let Some(hours)=cloud_hours(s){f.series=vec![Series{name:"SAMPLE • Hourly sky cover".into(),unit:"%".into(),color:"#70D5EF".into(),labels:(0..hours).map(|i|format!("Nov {} {:02}Z",25+i/24,i%24)).collect(),values:(0..hours).map(|i|Some([20.,25.,35.,50.,65.,80.,90.,85.,70.,55.,40.,30.][i%12])).collect()}];f.valid="SAMPLE • Nov 25 • Hourly sky cover".into();f.heading="SAMPLE • Supplied hourly cloud cover".into();f.note="SAMPLE ONLY • Sky cover in percent • Distinct hourly timestamps".into();}
    if s.kind==Kind::MultiImagery && s.title.starts_with("Futurecast 4-panel"){f.heading="SAMPLE • Future radar".into();let hours=s.period.split_whitespace().find_map(|v|v.parse::<i64>().ok()).unwrap_or(12);let start=chrono::NaiveDate::from_ymd_opt(2026,11,25).unwrap().and_hms_opt(0,0,0).unwrap();f.valid=format!("SAMPLE • {}",(start+chrono::Duration::hours(hours*frame as i64/3)).format("%b %d %HZ"));f.note="SAMPLE ONLY • Supplied future radar imagery • Ordered frames".into();}
    if s.title.starts_with("Probabilistic snow (NWS office)"){let day=if s.period=="Tomorrow"{26}else if s.period=="Day 3"{27}else{25};f.columns=["Location","Snow threshold","Probability","Accumulation period","Units","Source"].map(str::to_string).to_vec();f.rows=["Riverton","Oakdale","Millbrook","Cedar Hill","Pine Grove"].iter().enumerate().map(|(i,t)|vec![format!("SAMPLE {t}"),"4 in".into(),format!("{}%",[20,30,40,50,60][i]),format!("SAMPLE Nov {day} • 24 h"),"%".into(),"SAMPLE NWS office".into()]).collect();f.valid=format!("SAMPLE • Nov {day} 00Z–Nov {} 00Z",day+1);f.heading="SAMPLE • Supplied snowfall probabilities".into();}
    if s.title.starts_with("Mardi Gras / Cinco de Mayo"){f.values=[("Event","SAMPLE Kwanzaa"),("Date","SAMPLE Dec 26, 2026"),("Days remaining","SAMPLE 31"),("Forecast","SAMPLE Mostly clear • 40°F"),("Timing","SAMPLE Evening"),("Location","SAMPLE Riverton")].map(|(a,b)|Datum{label:a.into(),value:b.into()}).to_vec();f.bullets=vec!["SAMPLE ONLY • Producer selects the community holiday and forecast".into()];f.valid="SAMPLE • Dec 26, 2026".into();f.heading="SAMPLE • Community holiday forecast".into();}
    if s.title=="Snow timeline"{f.values=[("Starts","SAMPLE Nov 25 • 8 PM"),("Peaks","SAMPLE Nov 26 • 2–5 AM"),("Ends","SAMPLE Nov 26 • 10 AM"),("Snow amount","SAMPLE 2–4 in"),("Confidence","SAMPLE Moderate"),("Location","SAMPLE Sample County")].map(|(a,b)|Datum{label:a.into(),value:b.into()}).to_vec();f.bullets.clear();}
}
#[cfg(test)]
mod closeout_contract_tests {
    use super::*;
    #[test]
    fn snow_timing_requires_start_peak_and_end_in_event_cards(){for &s in SPECS.iter().filter(|s|s.title=="Snow timeline"){assert!(s.kind==Kind::Cards);let mut f=Frame::default();closeout_sample(s,&mut f,0);assert!(closeout_contract(s,&f));f.values.retain(|v|v.label!="Peaks");assert!(!closeout_contract(s,&f));}}
    #[test]
    fn snow_range_rejects_reversed_or_unitless_estimates(){let &s=SPECS.iter().find(|s|snow_range(**s)).unwrap();let mut f=Frame::default();closeout_sample(s,&mut f,0);assert!(closeout_contract(s,&f));f.rows[0][1]="7".into();assert!(!closeout_contract(s,&f));f.rows[0][1]="0.5".into();f.unit.clear();assert!(!closeout_contract(s,&f));}
    #[test]
    fn cloud_cover_rejects_duplicate_hours_and_impossible_percentages(){let &s=SPECS.iter().find(|s|cloud_hours(**s).is_some()).unwrap();let mut f=Frame::default();closeout_sample(s,&mut f,0);assert!(closeout_contract(s,&f));f.series[0].values[0]=Some(101.);assert!(!closeout_contract(s,&f));f.series[0].values[0]=Some(20.);f.series[0].labels[1]=f.series[0].labels[0].clone();assert!(!closeout_contract(s,&f));}
}

#[cfg(test)]
fn closeout_metadata(s:Spec,p:&mut Input){if snow_range(s)||dayparts(s)||cloud_hours(s).is_some()||s.title.starts_with("Futurecast 4-panel")||s.title.starts_with("Probabilistic snow (NWS office)")||s.title.starts_with("Mardi Gras / Cinco de Mayo")||s.title=="Snow timeline"{p.issued="SAMPLE • Issued Nov 25, 2026 00Z".into();}}
