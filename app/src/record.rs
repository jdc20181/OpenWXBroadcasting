//! Records the Program bus to MP4 (H.264 via OpenH264) or animated GIF.

use std::{
    fs::File,
    io::BufWriter,
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    thread::JoinHandle,
    time::Instant,
};

use anyhow::{anyhow, Context as _, Result};

use crate::{
    config::{RecordFormat, FRAME_H, FRAME_W},
    program::{Compositor, RecordFrame},
};

pub struct Recording {
    pub path: PathBuf,
    pub format: RecordFormat,
    pub started: Instant,
    handle: Option<JoinHandle<Result<PathBuf>>>,
    map_bus: bool,
}

impl Recording {
    /// Start recording the program bus into `dir`.
    pub fn start(compositor: &Compositor, format: RecordFormat, fps: u32, dir: PathBuf) -> Result<Self> {
        Self::start_bus(compositor, format, fps, dir, false)
    }

    pub fn start_map(compositor: &Compositor, format: RecordFormat, dir: PathBuf) -> Result<Self> {
        Self::start_bus(compositor, format, crate::program::FPS, dir, true)
    }

    fn start_bus(compositor: &Compositor, format: RecordFormat, fps: u32, dir: PathBuf, map_bus: bool) -> Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
        let ext = match format {
            RecordFormat::Mp4 => "mp4",
            RecordFormat::Gif => "gif",
        };
        let prefix = if map_bus { "Radar_Replay" } else { "OpenWX" };
        let path = dir.join(format!("{prefix}_{stamp}.{ext}"));

        // Bounded: if the encoder falls behind, the compositor drops frames instead of
        // buffering without limit.
        let (tx, rx) = mpsc::sync_channel::<RecordFrame>(45);
        let out = path.clone();
        let handle = std::thread::Builder::new()
            .name("recorder".into())
            .spawn(move || match format {
                RecordFormat::Mp4 => {
                    lower_priority();
                    encode_mp4(rx, out, fps.clamp(10, 60))
                }
                RecordFormat::Gif => encode_gif(rx, out),
            })
            .context("spawn recorder")?;
        if map_bus { compositor.set_map_recorder(Some(tx)); } else { compositor.set_recorder(Some(tx)); }
        // Program MP4s get the sound that played (show bed, scene soundtracks, clip audio).
        if !map_bus && format == RecordFormat::Mp4 {
            crate::audio::capture_start();
        }

        Ok(Self {
            path,
            format,
            started: Instant::now(),
            handle: Some(handle),
            map_bus,
        })
    }

    /// Stop taking frames now and finish the file on a background thread (waiting for the
    /// encoder, then mixing in the sound), so the UI never freezes on a long show export.
    /// The result arrives on the returned channel.
    pub fn stop_in_background(mut self, compositor: &Compositor) -> Receiver<Result<PathBuf>> {
        if self.map_bus { compositor.set_map_recorder(None); } else { compositor.set_recorder(None); }
        let clips = if !self.map_bus && self.format == RecordFormat::Mp4 { crate::audio::capture_take() } else { Vec::new() };
        let handle = self.handle.take();
        let fallback = self.path.clone();
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new().name("recording-finish".into()).spawn(move || {
            let result = match handle {
                Some(handle) => handle.join().map_err(|_| anyhow!("recorder thread panicked")).and_then(|r| r),
                None => Ok(fallback),
            };
            let result = result.inspect(|path| {
                if !clips.is_empty() {
                    if let Err(e) = crate::audio_mix::add_to_mp4(path, &clips) {
                        log::warn!("recording {} saved without its sound: {e:#}", path.display());
                    }
                }
            });
            let _ = tx.send(result);
        });
        if let Err(e) = spawned {
            let (tx, rx) = mpsc::channel();
            let _ = tx.send(Err(anyhow!("could not finish the recording: {e}")));
            return rx;
        }
        rx
    }

    /// Stop and finalize the file. Returns its path.
    pub fn stop(mut self, compositor: &Compositor) -> Result<PathBuf> {
        if self.map_bus { compositor.set_map_recorder(None); } else { compositor.set_recorder(None); }
        let clips = if !self.map_bus && self.format == RecordFormat::Mp4 { crate::audio::capture_take() } else { Vec::new() };
        let path = match self.handle.take() {
            Some(handle) => handle
                .join()
                .map_err(|_| anyhow!("recorder thread panicked"))??,
            None => self.path.clone(),
        };
        if !clips.is_empty() {
            // The picture is saved either way; a music problem is logged, not fatal.
            if let Err(e) = crate::audio_mix::add_to_mp4(&path, &clips) {
                log::warn!("recording {} saved without its sound: {e:#}", path.display());
            }
        }
        Ok(path)
    }
}

/// The encoder yields to the compositor and UI: it buffers and drops, they must not stutter.
fn lower_priority() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL};
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
}

fn strip_start_code(nal: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < nal.len() && nal[i] == 0 {
        i += 1;
    }
    if i < nal.len() && nal[i] == 1 {
        &nal[i + 1..]
    } else {
        nal
    }
}

/// Add a music / soundtrack file to an exported video (MP4: AAC, WebM: Opus), trimmed to
/// `seconds`. The built-in encoder writes picture only, so the audio is muxed in with
/// ffmpeg afterwards; the video stream is copied, not re-encoded.
pub fn mux_audio(video: &std::path::Path, audio: &std::path::Path, seconds: f32) -> Result<()> {
    let webm = video.extension().is_some_and(|e| e.eq_ignore_ascii_case("webm"));
    // MP4: Windows' own AAC encoder, no ffmpeg needed (the video already has the right length).
    if !webm {
        return crate::audio_mix::add_file_to_mp4(video, audio);
    }
    let tmp = video.with_extension(if webm { "audio.webm" } else { "audio.mp4" });
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.arg("-y").arg("-loglevel").arg("error")
        .arg("-i").arg(video)
        .arg("-i").arg(audio)
        .args(["-map", "0:v:0", "-map", "1:a:0", "-c:v", "copy"])
        .args(if webm { ["-c:a", "libopus", "-b:a", "160k"] } else { ["-c:a", "aac", "-b:a", "192k"] })
        .arg("-t").arg(format!("{seconds:.2}"))
        .arg(&tmp);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // no console window
    }
    let status = cmd.status().map_err(|_| anyhow!("adding the music needs ffmpeg on the PATH (winget install ffmpeg)"))?;
    if !status.success() {
        let _ = std::fs::remove_file(&tmp);
        return Err(anyhow!("ffmpeg could not add the music ({status})"));
    }
    std::fs::rename(&tmp, video)?;
    Ok(())
}

fn encode_mp4(rx: Receiver<RecordFrame>, path: PathBuf, fps: u32) -> Result<PathBuf> {
    encode_mp4_timed(rx, path, fps, false)
}

/// Render an animated graphic offline, frame by frame, to MP4 (H.264). `render(t)` draws the frame
/// at `t` seconds. Frames get exact timestamps (not wall-clock), so slow rendering never stutters.
pub fn export_animation_mp4(mut render: impl FnMut(f32) -> tiny_skia::Pixmap, seconds: f32, fps: u32, path: PathBuf) -> Result<PathBuf> {
    let (tx, rx) = mpsc::sync_channel::<RecordFrame>(8);
    let out = path.clone();
    let fps = fps.clamp(10, 60);
    let handle = std::thread::Builder::new()
        .name("export-mp4".into())
        .spawn(move || encode_mp4_timed(rx, out, fps, true))
        .context("spawn encoder")?;
    let frames = (seconds.max(0.5) * fps as f32).round() as u32;
    for i in 0..frames {
        let pm = render(i as f32 / fps as f32);
        // H.264 needs even dimensions: crop a stray odd row / column.
        let (w, h) = (pm.width() & !1, pm.height() & !1);
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h as usize {
            let start = row * pm.width() as usize * 4;
            rgba.extend_from_slice(&pm.data()[start..start + w as usize * 4]);
        }
        if tx.send(RecordFrame { width: w, height: h, rgba }).is_err() {
            break;
        }
    }
    drop(tx);
    handle.join().map_err(|_| anyhow!("encoder thread panicked"))?
}

/// WebM with transparency (VP9 + alpha) for OBS overlays: renders a PNG sequence and hands it to
/// ffmpeg. Without ffmpeg the PNG sequence folder is kept and returned as the error detail.
pub fn export_animation_webm(render: impl Fn(f32) -> tiny_skia::Pixmap, seconds: f32, fps: u32, path: PathBuf) -> Result<PathBuf> {
    let dir = path.with_extension("frames");
    std::fs::create_dir_all(&dir)?;
    let fps = fps.clamp(10, 60);
    let frames = (seconds.max(0.5) * fps as f32).round() as u32;
    for i in 0..frames {
        render(i as f32 / fps as f32).save_png(dir.join(format!("{i:05}.png")))?;
    }
    let status = std::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-framerate", &fps.to_string(), "-i"])
        .arg(dir.join("%05d.png"))
        .args(["-c:v", "libvpx-vp9", "-pix_fmt", "yuva420p", "-b:v", "6M", "-auto-alt-ref", "0"])
        .arg(&path)
        .status();
    match status {
        Ok(s) if s.success() => {
            let _ = std::fs::remove_dir_all(&dir);
            Ok(path)
        }
        Ok(s) => Err(anyhow!("ffmpeg failed ({s}); the PNG frames are in {}", dir.display())),
        Err(_) => Err(anyhow!("WebM needs ffmpeg on the PATH (winget install ffmpeg). The PNG frames are in {}", dir.display())),
    }
}

fn encode_mp4_timed(rx: Receiver<RecordFrame>, path: PathBuf, fps: u32, synthetic: bool) -> Result<PathBuf> {
    use openh264::{
        encoder::{BitRate, Encoder, EncoderConfig, FrameRate},
        formats::{RgbaSliceU8, YUVBuffer},
        OpenH264API,
    };

    let config = EncoderConfig::new()
        .bitrate(BitRate::from_bps(14_000_000))
        .max_frame_rate(FrameRate::from_hz(fps as f32));
    let mut encoder = Encoder::with_api_config(OpenH264API::from_source(), config)
        .map_err(|e| anyhow!("H.264 encoder: {e}"))?;

    let mut writer: Option<mp4::Mp4Writer<BufWriter<File>>> = None;
    let mut sps: Option<Vec<u8>> = None;
    let mut pps: Option<Vec<u8>> = None;
    // Samples are written one frame late so each gets its real duration.
    let mut pending: Option<(u64, bool, Vec<u8>)> = None;
    let started = Instant::now();
    let mut last_ms: Option<u64> = None;
    let mut index = 0u64;

    for frame in rx {
        let mut ms = if synthetic { index * 1000 / fps as u64 } else { started.elapsed().as_millis() as u64 };
        index += 1;
        if let Some(last) = last_ms {
            if ms <= last {
                ms = last + 1;
            }
        }
        last_ms = Some(ms);

        let yuv = YUVBuffer::from_rgb8_source(RgbaSliceU8::new(
            &frame.rgba,
            (frame.width as usize, frame.height as usize),
        ));
        let bitstream = encoder
            .encode(&yuv)
            .map_err(|e| anyhow!("encode frame: {e}"))?;
        let annex_b = bitstream.to_vec();

        let mut sample = Vec::with_capacity(annex_b.len() + 16);
        let mut is_sync = false;
        for nal in openh264::nal_units(&annex_b) {
            let nal = strip_start_code(nal);
            if nal.is_empty() {
                continue;
            }
            match nal[0] & 0x1F {
                7 => {
                    if sps.is_none() {
                        sps = Some(nal.to_vec());
                    }
                }
                8 => {
                    if pps.is_none() {
                        pps = Some(nal.to_vec());
                    }
                }
                kind => {
                    if kind == 5 {
                        is_sync = true;
                    }
                    sample.extend_from_slice(&(nal.len() as u32).to_be_bytes());
                    sample.extend_from_slice(nal);
                }
            }
        }
        if sample.is_empty() {
            continue;
        }

        if writer.is_none() {
            let (Some(sps), Some(pps)) = (sps.clone(), pps.clone()) else {
                continue;
            };
            let file = BufWriter::new(File::create(&path)?);
            let config = mp4::Mp4Config {
                major_brand: "isom".parse().map_err(|e| anyhow!("{e:?}"))?,
                minor_version: 512,
                compatible_brands: ["isom", "iso2", "avc1", "mp41"]
                    .iter()
                    .filter_map(|b| b.parse().ok())
                    .collect(),
                timescale: 1000,
            };
            let mut w = mp4::Mp4Writer::write_start(file, &config)?;
            w.add_track(&mp4::TrackConfig {
                track_type: mp4::TrackType::Video,
                timescale: 1000,
                language: "und".into(),
                media_conf: mp4::MediaConfig::AvcConfig(mp4::AvcConfig {
                    width: frame.width as u16,
                    height: frame.height as u16,
                    seq_param_set: sps,
                    pic_param_set: pps,
                }),
            })?;
            writer = Some(w);
        }

        if let (Some(w), Some((start, sync, bytes))) = (writer.as_mut(), pending.take()) {
            w.write_sample(
                1,
                &mp4::Mp4Sample {
                    start_time: start,
                    duration: (ms - start).max(1) as u32,
                    rendering_offset: 0,
                    is_sync: sync,
                    bytes: bytes::Bytes::from(bytes),
                },
            )?;
        }
        pending = Some((ms, is_sync, sample));
    }

    if let Some(mut w) = writer {
        if let Some((start, sync, bytes)) = pending.take() {
            // Live recordings: the last picture stays up until the recording stopped (unchanged
            // frames are not resent, so this can be seconds).
            let duration = if synthetic {
                1000 / fps.max(1)
            } else {
                (started.elapsed().as_millis() as u64).saturating_sub(start).clamp(1, 60_000) as u32
            };
            w.write_sample(
                1,
                &mp4::Mp4Sample {
                    start_time: start,
                    duration: duration.max(1),
                    rendering_offset: 0,
                    is_sync: sync,
                    bytes: bytes::Bytes::from(bytes),
                },
            )?;
        }
        w.write_end()?;
        Ok(path)
    } else {
        Err(anyhow!("no frames were recorded"))
    }
}

pub fn export_map_gif(mut render: impl FnMut(f32) -> tiny_skia::Pixmap, seconds: f32, path: PathBuf) -> Result<PathBuf> {
    const FPS: u32 = 8;
    let (width, height) = (FRAME_W / 2, FRAME_H / 2);
    let file = BufWriter::new(File::create(&path)?);
    let mut encoder = gif::Encoder::new(file, width as u16, height as u16, &[])?;
    if seconds > 0.0 { encoder.set_repeat(gif::Repeat::Infinite)?; }
    for i in 0..((seconds * FPS as f32) as u32).max(1) {
        let pixmap = render(i as f32 / FPS as f32);
        let source = image::RgbaImage::from_raw(pixmap.width(), pixmap.height(), pixmap.take())
            .context("Invalid map export dimensions")?;
        let mut pixels = image::imageops::resize(&source, width, height, image::imageops::FilterType::Triangle).into_raw();
        let mut frame = gif::Frame::from_rgba_speed(width as u16, height as u16, &mut pixels, 10);
        frame.delay = if i % 2 == 0 { 12 } else { 13 };
        encoder.write_frame(&frame)?;
    }
    Ok(path)
}

fn encode_gif(rx: Receiver<RecordFrame>, path: PathBuf) -> Result<PathBuf> {
    const W: u32 = FRAME_W / 2;
    const H: u32 = FRAME_H / 2;
    let file = BufWriter::new(File::create(&path)?);
    let mut encoder = gif::Encoder::new(file, W as u16, H as u16, &[])?;
    encoder.set_repeat(gif::Repeat::Infinite)?;

    let mut count = 0u64;
    for frame in rx {
        count += 1;
        // ~15 fps for GIF.
        if count % 2 == 0 {
            continue;
        }
        let Some(image) = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba) else {
            continue;
        };
        let small = image::imageops::resize(&image, W, H, image::imageops::FilterType::Triangle);
        let mut pixels = small.into_raw();
        let mut gif_frame = gif::Frame::from_rgba_speed(W as u16, H as u16, &mut pixels, 10);
        gif_frame.delay = 7;
        encoder.write_frame(&gif_frame)?;
    }
    if count == 0 {
        return Err(anyhow!("no frames were recorded"));
    }
    Ok(path)
}
