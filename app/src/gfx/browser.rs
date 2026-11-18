//! Self-contained HTML5/WebGL2 graphics. Artwork is captured at export; motion and alpha
//! are composed on the GPU by the browser with the native renderer's sampled poses. Every
//! layer kind (risk meters and maps, angled plates, motifs) is captured as rendered; motion
//! covers fade, slide, scale, rotation and left or centre wipes. A meter's growing fill shows
//! settled (the layer still animates in).
use super::{
    bind::Bindings,
    model::{AnimKind, GraphicDoc},
    render::{browser_pose, GraphicsRenderer, RenderCtx, SETTLED},
};
use crate::{
    config::{LibraryPaths, Settings},
    data::WxSnapshot,
};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::json;
use std::path::PathBuf;

pub fn export(doc: &GraphicDoc, settings: &Settings, data: &WxSnapshot) -> Result<PathBuf> {
    let resolved = doc.resolved();
    let mut bindings = Bindings::build(data, settings, None, &settings.radar_palette);
    // Match native per-document bindings, including saved overrides.
    for (k, v) in &resolved.data_overrides {
        let value = bindings.apply(v);
        bindings.set(k, &value);
    }
    let doc = resolved.reflow(|l| match &l.kind {
        super::model::LayerKind::Text { text, .. } => bindings.apply(text).trim().is_empty(),
        _ => false,
    });
    anyhow::ensure!(
        doc.width > 0 && doc.height > 0 && doc.width <= 7680 && doc.height <= 4320,
        "Unsupported browser canvas size"
    );
    anyhow::ensure!(
        doc.layers.len() <= 80,
        "Browser export supports up to 80 layers"
    );
    let media = LibraryPaths::get().media;
    let mut renderer = GraphicsRenderer::new();
    let mut items = Vec::new();
    let duration = doc.duration.max(doc.intro_length() + 1.).clamp(1., 120.);
    let frames = (duration * 30.).ceil() as usize;
    let ctx = |time, out_time| RenderCtx {
        bindings: &bindings,
        brand: &settings.brand,
        time,
        out_time,
        media_dir: &media,
    };
    let mut bg = doc.clone();
    bg.layers.clear();
    if bg.background.is_some() {
        let mut pm = tiny_skia::Pixmap::new(doc.width, doc.height)
            .ok_or_else(|| anyhow::anyhow!("Canvas allocation failed"))?;
        renderer.render(&mut pm, &bg, &ctx(SETTLED, None));
        items.push(json!({"png":STANDARD.encode(pm.encode_png()?),"center":[0,0],"box":[0,0,doc.width,doc.height],"poses":[[1,0,0,1,0,1]],"out":[[0,0,0,1,0,1]]}));
    }
    for layer in doc.layers.iter().filter(|l| l.visible) {
        let mut single = doc.clone();
        single.background = None;
        let mut still = layer.clone();
        still.opacity = 1.;
        still.rotation = 0.;
        still.anim_in.kind = AnimKind::None;
        still.anim_out.kind = AnimKind::None;
        still.keys.clear();
        still.loop_mode.clear();
        still.reveal.clear();
        still.time_offset = 0.;
        single.layers = vec![still];
        let mut pm = tiny_skia::Pixmap::new(doc.width, doc.height)
            .ok_or_else(|| anyhow::anyhow!("Layer allocation failed"))?;
        renderer.render(&mut pm, &single, &ctx(SETTLED, None));
        let poses: Vec<_> = (0..=frames)
            .map(|i| browser_pose(layer, &doc, &ctx(i as f32 / 30., None)))
            .collect();
        let out: Vec<_> = (0..=60)
            .map(|i| browser_pose(layer, &doc, &ctx(duration, Some(i as f32 / 30.))))
            .collect();
        items.push(json!({"png":STANDARD.encode(pm.encode_png()?),"center":[layer.x+layer.w/2.,layer.y+layer.h/2.],"box":[layer.x,layer.y,layer.w,layer.h],"poses":poses,"out":out}));
    }
    let payload = json!({"width":doc.width,"height":doc.height,"duration":duration,"layers":items});
    let json = serde_json::to_string(&payload)?.replace('<', "\\u003c");
    let html = PAGE.replace("__PAYLOAD__", &json);
    let stem: String = doc
        .name
        .chars()
        .take(70)
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let path = LibraryPaths::get().exports.join(format!(
        "{}-{}.html",
        stem.trim(),
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    std::fs::write(&path, html)?;
    Ok(path)
}

const PAGE: &str = r##"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>OpenWX browser graphic</title>
<style>html,body{margin:0;width:100%;height:100%;overflow:hidden;background:transparent}canvas{width:100%;height:100%;object-fit:contain;display:block}#error{color:#fff;background:#222;padding:20px;font:16px sans-serif;display:none}</style></head><body><canvas id="frame"></canvas><div id="error"></div>
<script id="graphic" type="application/json">__PAYLOAD__</script><script>
'use strict';
(async()=>{const d=JSON.parse(document.getElementById('graphic').textContent),canvas=document.getElementById('frame');canvas.width=d.width;canvas.height=d.height;
const gl=canvas.getContext('webgl2',{alpha:true,premultipliedAlpha:true,antialias:false});if(!gl)throw Error('WebGL2 is unavailable. Enable browser hardware acceleration or use the native PNG/video export.');
function shader(type,source){const s=gl.createShader(type);gl.shaderSource(s,source);gl.compileShader(s);if(!gl.getShaderParameter(s,gl.COMPILE_STATUS))throw Error(gl.getShaderInfoLog(s));return s;}
const program=gl.createProgram();gl.attachShader(program,shader(gl.VERTEX_SHADER,`#version 300 es
in vec2 vertex;uniform vec2 size;uniform vec2 center;uniform vec4 motion;out vec2 uv;
void main(){uv=vertex;vec2 p=vertex*size-center;p*=motion.z;float a=radians(motion.w);p=mat2(cos(a),sin(a),-sin(a),cos(a))*p+center+motion.xy;gl_Position=vec4(p.x/size.x*2.-1.,1.-p.y/size.y*2.,0,1);}`));
gl.attachShader(program,shader(gl.FRAGMENT_SHADER,`#version 300 es
precision highp float;in vec2 uv;uniform sampler2D image;uniform float opacity;uniform float wipe;uniform vec4 box;uniform vec2 size;out vec4 color;
void main(){float x=uv.x*size.x;if(wipe>=2.){float w=wipe-2.;float x0=box.x+box.z*(1.-w)/2.;if(x<x0 || x>x0+box.z*w || uv.y*size.y<box.y || uv.y*size.y>box.y+box.w)discard;}else if(wipe<1. && (x<box.x || x>box.x+box.z*wipe || uv.y*size.y<box.y || uv.y*size.y>box.y+box.w))discard;vec4 c=texture(image,uv);color=vec4(c.rgb*c.a,c.a)*opacity;}`));
gl.linkProgram(program);if(!gl.getProgramParameter(program,gl.LINK_STATUS))throw Error(gl.getProgramInfoLog(program));gl.useProgram(program);
const loc=n=>gl.getUniformLocation(program,n),buffer=gl.createBuffer();gl.bindBuffer(gl.ARRAY_BUFFER,buffer);gl.bufferData(gl.ARRAY_BUFFER,new Float32Array([0,0,1,0,0,1,0,1,1,0,1,1]),gl.STATIC_DRAW);const v=gl.getAttribLocation(program,'vertex');gl.enableVertexAttribArray(v);gl.vertexAttribPointer(v,2,gl.FLOAT,false,0,0);gl.uniform2f(loc('size'),d.width,d.height);gl.enable(gl.BLEND);gl.blendFunc(gl.ONE,gl.ONE_MINUS_SRC_ALPHA);gl.viewport(0,0,d.width,d.height);
for(const l of d.layers){const img=new Image();img.src='data:image/png;base64,'+l.png;await img.decode();l.texture=gl.createTexture();gl.bindTexture(gl.TEXTURE_2D,l.texture);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA,gl.RGBA,gl.UNSIGNED_BYTE,img);delete l.png;}
let started=performance.now(),paused=false,held=0,outAt=null;const query=new URLSearchParams(location.search);let loop=query.get('loop')==='1';
window.cwxGraphic={replay(){started=performance.now();outAt=null;paused=false;},pause(){if(!paused){held=performance.now()-started;paused=true;}},play(){if(paused){started=performance.now()-held;paused=false;}},takeOut(){outAt=performance.now();},setLoop(value){loop=!!value;}};
addEventListener('keydown',e=>{if(e.code==='Space'){e.preventDefault();paused?cwxGraphic.play():cwxGraphic.pause();}if(e.code==='KeyR')cwxGraphic.replay();if(e.code==='KeyO')cwxGraphic.takeOut();});
function draw(now){gl.clearColor(0,0,0,0);gl.clear(gl.COLOR_BUFFER_BIT);let t=(paused?held:now-started)/1000;if(loop&&outAt===null)t%=d.duration;
for(const l of d.layers){const poses=outAt===null?l.poses:l.out;const index=Math.min(poses.length-1,Math.max(0,Math.floor((outAt===null?t:(now-outAt)/1000)*30)));const p=poses[index];gl.bindTexture(gl.TEXTURE_2D,l.texture);gl.uniform2fv(loc('center'),l.center);gl.uniform4f(loc('motion'),p[1],p[2],p[3],p[4]);gl.uniform1f(loc('opacity'),p[0]);gl.uniform1f(loc('wipe'),p[5]);gl.uniform4fv(loc('box'),l.box);gl.drawArrays(gl.TRIANGLES,0,6);}requestAnimationFrame(draw);}requestAnimationFrame(draw);
})().catch(e=>{const el=document.getElementById('error');el.style.display='block';el.textContent=e.message;console.error(e);});
</script></body></html>"##;
