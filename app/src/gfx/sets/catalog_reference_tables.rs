// Published educational reference values, transcribed without calculating an index.
// Source: https://www.weather.gov/ddc/heat_index (accessed 2026-11-23).
const HEAT_REFERENCE: &[&[i16]] = &[
    &[80,80,81,81,82,82,83,84,84,85,86,86,87],
    &[81,82,83,84,84,85,86,88,89,90,91,93,95],
    &[83,84,85,86,88,89,90,92,94,96,98,100,103],
    &[85,87,88,89,91,93,95,97,100,102,105,108,112],
    &[88,89,91,93,95,98,100,103,106,110,113,117,121],
    &[91,93,95,97,100,103,106,109,113,117,122,127,132],
    &[94,96,99,101,105,108,112,116,121,126,131],
    &[97,100,103,106,110,114,119,124,129,135],
    &[101,104,108,112,116,121,126,132],
    &[105,109,113,117,123,128,134],
    &[109,114,118,124,129,136], &[114,119,124,130,137],
    &[119,124,131,137], &[124,130,137], &[130,137], &[136],
];
// Source: https://www.weather.gov/arx/wind_chill (accessed 2026-11-23).
// Published excerpt: 5-40 mph, air temperature 35 through -45 F.
const CHILL_REFERENCE: &[&[i16]] = &[
    &[31,25,19,13,7,1,-5,-11,-16,-22,-28,-34,-40,-46,-52,-57,-63],
    &[27,21,15,9,3,-4,-10,-16,-22,-28,-35,-41,-47,-53,-59,-66,-72],
    &[25,19,13,6,0,-7,-13,-19,-26,-32,-39,-45,-51,-58,-64,-71,-77],
    &[24,17,11,4,-2,-9,-15,-22,-29,-35,-42,-48,-55,-61,-68,-74,-81],
    &[23,16,9,3,-4,-11,-17,-24,-31,-37,-44,-51,-58,-64,-71,-78,-84],
    &[22,15,8,1,-5,-12,-19,-26,-33,-39,-46,-53,-60,-67,-73,-80,-87],
    &[21,14,7,0,-7,-14,-21,-27,-34,-41,-48,-55,-62,-69,-76,-82,-89],
    &[20,13,6,-1,-8,-15,-22,-29,-36,-43,-50,-57,-64,-71,-78,-84,-91],
];
fn reference_input(s:Spec)->Input {
    let heat=s.title=="Heat index chart";
    let data=if heat{HEAT_REFERENCE}else{CHILL_REFERENCE};
    let mut columns=vec![if heat{"Temp °F"}else{"mph"}.into()];
    columns.extend(if heat{(0..13).map(|i|format!("{}%",40+i*5)).collect::<Vec<_>>()}else{(0..17).map(|i|format!("{}°F",35-i*5)).collect()});
    let rows=data.iter().enumerate().map(|(i,row)|{
        let mut out=vec![if heat{format!("{}°F",80+i*2)}else{format!("{}",5+i*5)}];
        for j in 0..columns.len()-1{out.push(row.get(j).map(i16::to_string).unwrap_or_default());}out
    }).collect();
    let frame=Frame{valid:"Published educational reference • All values °F".into(),
        heading:if heat{"Rows: air temperature • Columns: relative humidity"}else{"Rows: wind speed mph • Columns: air temperature"}.into(),
        note:if heat{"Shade/light wind reference • Empty cells have no numeric value in the source table"}else{"Published reference excerpt: 5–40 mph and 35 to -45°F • Not current conditions"}.into(),
        unit:"°F".into(),columns,rows,..Default::default()};
    Input{id:s.id.into(),scope:s.scope.into(),period:s.period.into(),reviewed:true,
        source:if heat{"Source: NOAA/NWS Dodge City reference table"}else{"Source: NOAA/NWS La Crosse reference table"}.into(),
        issued:"Educational reference • Source accessed Nov 23, 2026".into(),frames:vec![frame]}
}
fn matrix(pm:&mut Pixmap,g:&mut GlyphCache,font:&str,f:&Frame,r:(f32,f32,f32,f32)) {
    let rh=r.3/(f.rows.len()+1) as f32;let cw=r.2/f.columns.len() as f32;
    let size=(cw.min(rh)*0.45).clamp(12.,28.);
    for (j,c)in f.columns.iter().enumerate(){
        panel(pm,(r.0+j as f32*cw,r.1,cw-2.,rh-2.),"#24506A");
        draw_text_box(pm,g,c,scaled_rect((r.0+j as f32*cw+3.,r.1+2.,cw-6.,rh-4.)),&TextStyle{family:font.into(),weight:Weight::SemiBold,size:size*draw_scale(),color:Color::WHITE,letter_spacing:0.},Align::Center,true,true,true,false,1.);
    }
    for(i,row)in f.rows.iter().enumerate(){for(j,c)in row.iter().enumerate(){
        let x=r.0+j as f32*cw;let y=r.1+(i+1)as f32*rh;
        panel(pm,(x,y,cw-2.,rh-2.),if j==0{"#24506A"}else if c.is_empty(){"#111D2A"}else if i%2==0{"#19334E"}else{"#142B43"});
        draw_text_box(pm,g,c,scaled_rect((x+3.,y+2.,cw-6.,rh-4.)),&TextStyle{family:font.into(),weight:Weight::SemiBold,size:size*draw_scale(),color:Color::WHITE,letter_spacing:0.},Align::Center,true,true,true,false,1.);
    }}
}
