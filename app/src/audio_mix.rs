//! Music for exported videos: mix what played during a recording (or a graphic's soundtrack)
//! offline, encode it to AAC with Windows' built-in encoder (Media Foundation), and add it to
//! the MP4 as a second track. The video stream is copied, never re-encoded, and nothing needs
//! ffmpeg.

use std::{collections::HashMap, path::Path};

use anyhow::{anyhow, Context as _, Result};

use crate::audio::{Bus, CapturedClip, ClipSource};

pub const RATE: u32 = 48_000;

/// Decode a sound to interleaved stereo f32 at `RATE`.
fn decode(source: &ClipSource) -> Option<Vec<f32>> {
    use rodio::Source;
    let decoder = match source {
        ClipSource::File(path) => rodio::Decoder::try_from(std::fs::File::open(path).ok()?).ok()?,
        ClipSource::Wav(bytes) => {
            let decoder = rodio::Decoder::try_from(std::io::Cursor::new(bytes.clone())).ok()?;
            return Some(resample(decoder));
        }
    };
    let _ = decoder.total_duration();
    Some(resample(decoder))
}

fn resample<S: rodio::Source>(source: S) -> Vec<f32>
where
    S: Iterator<Item = f32>,
{
    let channels: u16 = source.channels().into();
    let rate: u32 = source.sample_rate().into();
    let channels = channels.max(1) as usize;
    let raw: Vec<f32> = source.collect();
    let frames = raw.len() / channels;
    // To stereo frames.
    let stereo: Vec<[f32; 2]> = (0..frames)
        .map(|i| {
            let f = &raw[i * channels..i * channels + channels];
            if channels == 1 { [f[0], f[0]] } else { [f[0], f[1]] }
        })
        .collect();
    if rate == RATE || stereo.is_empty() {
        return stereo.into_iter().flatten().collect();
    }
    // Linear resampling is fine for a music bed.
    let ratio = rate as f64 / RATE as f64;
    let out_frames = ((stereo.len() as f64) / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_frames * 2);
    for i in 0..out_frames {
        let pos = i as f64 * ratio;
        let a = pos.floor() as usize;
        let t = (pos - a as f64) as f32;
        let b = (a + 1).min(stereo.len() - 1);
        out.push(stereo[a][0] + (stereo[b][0] - stereo[a][0]) * t);
        out.push(stereo[a][1] + (stereo[b][1] - stereo[a][1]) * t);
    }
    out
}

/// Mix the clips into `seconds` of interleaved stereo at `RATE`. Mirrors the live rules: a
/// scene's own sound silences the show bed, and an alert tone ducks the bed and graphics.
pub fn mix(clips: &[CapturedClip], seconds: f64) -> Vec<f32> {
    let total = (seconds.max(0.0) * RATE as f64).round() as usize;
    let mut out = vec![0f32; total * 2];
    let mut cache: HashMap<String, Option<std::sync::Arc<Vec<f32>>>> = HashMap::new();
    let key = |s: &ClipSource| match s {
        ClipSource::File(p) => p.to_string_lossy().into_owned(),
        ClipSource::Wav(b) => format!("wav:{:p}", b.as_ptr()),
    };
    let decoded: Vec<Option<std::sync::Arc<Vec<f32>>>> = clips
        .iter()
        .map(|c| cache.entry(key(&c.source)).or_insert_with(|| decode(&c.source).map(std::sync::Arc::new)).clone())
        .collect();

    // When each clip is actually sounding, in frames: [from, to).
    let span = |c: &CapturedClip, pcm: &Option<std::sync::Arc<Vec<f32>>>| -> Option<(i64, i64)> {
        let pcm = pcm.as_ref()?;
        let len = (pcm.len() / 2) as i64;
        if len == 0 {
            return None;
        }
        let from = (c.start * RATE as f64).round() as i64;
        let mut to = c.stop.map(|s| (s * RATE as f64).round() as i64).unwrap_or(total as i64);
        if !c.looped {
            to = to.min(from + len);
        }
        Some((from.max(0), to.min(total as i64)))
    };
    let spans: Vec<Option<(i64, i64)>> = clips.iter().zip(&decoded).map(|(c, p)| span(c, p)).collect();
    let active = |bus: Bus, frame: i64| {
        clips.iter().zip(&spans).any(|(c, s)| c.bus == bus && s.is_some_and(|(a, b)| frame >= a && frame < b))
    };

    const BLOCK: i64 = 480;
    for ((clip, pcm), span) in clips.iter().zip(&decoded).zip(&spans) {
        let (Some(pcm), Some((from, to))) = (pcm, *span) else { continue };
        let len = (pcm.len() / 2) as i64;
        let origin = (clip.start * RATE as f64).round() as i64;
        let mut block = from;
        while block < to {
            let end = (block + BLOCK).min(to);
            let mut gain = clip.volume.clamp(0.0, 2.0);
            match clip.bus {
                Bus::Show if active(Bus::Graphic, block) => gain = 0.0,
                Bus::Show | Bus::Graphic if active(Bus::Alert, block) => gain *= 0.25,
                _ => {}
            }
            if gain > 0.0 {
                for frame in block..end {
                    let mut i = frame - origin;
                    if clip.looped {
                        i = i.rem_euclid(len);
                    } else if i < 0 || i >= len {
                        continue;
                    }
                    let (o, s) = (frame as usize * 2, i as usize * 2);
                    out[o] += pcm[s] * gain;
                    out[o + 1] += pcm[s + 1] * gain;
                }
            }
            block = end;
        }
    }
    out
}

/// Add the clips' mix to an MP4 as an AAC track (the video is copied as is). Does nothing when
/// no sound played.
pub fn add_to_mp4(video: &Path, clips: &[CapturedClip]) -> Result<()> {
    if clips.is_empty() {
        return Ok(());
    }
    let seconds = video_seconds(video)?;
    let pcm = mix(clips, seconds);
    if pcm.iter().all(|s| *s == 0.0) {
        log::info!("export: no audible sound to add to {}", video.display());
        return Ok(());
    }
    let m4a = video.with_extension("music.m4a");
    let result = encode_aac(&pcm, &m4a).and_then(|_| remux(video, &m4a));
    let _ = std::fs::remove_file(&m4a);
    result
}

/// Add a music file (a graphic's soundtrack) to an MP4, trimmed to the video.
pub fn add_file_to_mp4(video: &Path, audio: &Path) -> Result<()> {
    let clip = CapturedClip {
        source: ClipSource::File(audio.to_path_buf()),
        bus: Bus::Graphic,
        volume: 1.0,
        looped: false,
        start: 0.0,
        stop: None,
    };
    if decode(&clip.source).is_none() {
        return Err(anyhow!("can't read the music file {}", audio.display()));
    }
    add_to_mp4(video, &[clip])
}

fn video_seconds(video: &Path) -> Result<f64> {
    let file = std::fs::File::open(video)?;
    let size = file.metadata()?.len();
    let reader = mp4::Mp4Reader::read_header(std::io::BufReader::new(file), size)?;
    Ok(reader.duration().as_secs_f64())
}

/// Rewrite the MP4 with its video track plus the AAC track from `m4a`.
fn remux(video: &Path, m4a: &Path) -> Result<()> {
    use std::io::{BufReader, BufWriter};
    let open = |p: &Path| -> Result<mp4::Mp4Reader<BufReader<std::fs::File>>> {
        let f = std::fs::File::open(p)?;
        let size = f.metadata()?.len();
        Ok(mp4::Mp4Reader::read_header(BufReader::new(f), size)?)
    };
    let mut vr = open(video)?;
    let mut ar = open(m4a)?;
    let vt = vr.tracks().values().find(|t| matches!(t.track_type(), Ok(mp4::TrackType::Video))).ok_or_else(|| anyhow!("no video track"))?;
    let at = ar.tracks().values().find(|t| matches!(t.track_type(), Ok(mp4::TrackType::Audio))).ok_or_else(|| anyhow!("no audio track"))?;
    let (vid, aid) = (vt.track_id(), at.track_id());
    let video_conf = mp4::TrackConfig {
        track_type: mp4::TrackType::Video,
        timescale: vt.timescale(),
        language: "und".into(),
        media_conf: mp4::MediaConfig::AvcConfig(mp4::AvcConfig {
            width: vt.width(),
            height: vt.height(),
            seq_param_set: vt.sequence_parameter_set()?.to_vec(),
            pic_param_set: vt.picture_parameter_set()?.to_vec(),
        }),
    };
    let audio_conf = mp4::TrackConfig {
        track_type: mp4::TrackType::Audio,
        timescale: at.timescale(),
        language: "und".into(),
        media_conf: mp4::MediaConfig::AacConfig(mp4::AacConfig {
            bitrate: at.bitrate(),
            profile: at.audio_profile()?,
            freq_index: at.sample_freq_index()?,
            chan_conf: at.channel_config()?,
        }),
    };
    let video_end = vt.duration().as_secs_f64();
    let audio_scale = at.timescale() as f64;
    let (vcount, acount) = (vt.sample_count(), at.sample_count());

    let tmp = video.with_extension("withmusic.mp4");
    {
        let config = mp4::Mp4Config {
            major_brand: "isom".parse().map_err(|e| anyhow!("{e:?}"))?,
            minor_version: 512,
            compatible_brands: ["isom", "iso2", "avc1", "mp41"].iter().filter_map(|b| b.parse().ok()).collect(),
            timescale: 1000,
        };
        let mut w = mp4::Mp4Writer::write_start(BufWriter::new(std::fs::File::create(&tmp)?), &config)?;
        w.add_track(&video_conf)?;
        w.add_track(&audio_conf)?;
        // Interleave roughly by time (half-second chunks) so players stream it smoothly.
        let (mut vi, mut ai) = (1u32, 1u32);
        let vscale = video_conf.timescale as f64;
        while vi <= vcount || ai <= acount {
            let vnext = if vi <= vcount { vr.read_sample(vid, vi)? } else { None };
            let take_video = match &vnext {
                Some(s) => {
                    let vt_s = s.start_time as f64 / vscale;
                    // Write audio up to this video time first.
                    while ai <= acount {
                        let Some(a) = ar.read_sample(aid, ai)? else { ai = acount + 1; break };
                        let t = a.start_time as f64 / audio_scale;
                        if t > vt_s {
                            break;
                        }
                        if t < video_end {
                            w.write_sample(2, &a)?;
                        }
                        ai += 1;
                    }
                    true
                }
                None => false,
            };
            if take_video {
                w.write_sample(1, vnext.as_ref().expect("video sample"))?;
                vi += 1;
            } else {
                vi = vcount + 1;
                while ai <= acount {
                    if let Some(a) = ar.read_sample(aid, ai)? {
                        if (a.start_time as f64 / audio_scale) < video_end {
                            w.write_sample(2, &a)?;
                        }
                    }
                    ai += 1;
                }
            }
        }
        w.write_end()?;
    }
    drop(vr);
    std::fs::rename(&tmp, video).context("replace the video with the version with music")?;
    Ok(())
}

/// Encode interleaved stereo f32 at `RATE` to an AAC .m4a (192 kbps) with Media Foundation.
#[cfg(windows)]
fn encode_aac(pcm: &[f32], out: &Path) -> Result<()> {
    use windows::{
        core::HSTRING,
        Win32::{
            Media::MediaFoundation::*,
            System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED},
        },
    };
    let _ = std::fs::remove_file(out);
    // Run on a fresh thread so COM / MF state never touches the UI thread.
    let pcm: Vec<i16> = pcm.iter().map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16).collect();
    let out = out.to_path_buf();
    std::thread::spawn(move || -> Result<()> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().ok();
            MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
            let result = (|| -> windows::core::Result<()> {
                let writer = MFCreateSinkWriterFromURL(&HSTRING::from(out.as_os_str()), None, None)?;
                let aac = MFCreateMediaType()?;
                aac.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
                aac.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)?;
                aac.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
                aac.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, RATE)?;
                aac.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
                aac.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 24_000)?;
                let stream = writer.AddStream(&aac)?;
                let input = MFCreateMediaType()?;
                input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
                input.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
                input.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
                input.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, RATE)?;
                input.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
                input.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)?;
                input.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, RATE * 4)?;
                writer.SetInputMediaType(stream, &input, None)?;
                writer.BeginWriting()?;
                // 100 ms per sample.
                let chunk_frames = (RATE / 10) as usize;
                let mut frame = 0usize;
                for chunk in pcm.chunks(chunk_frames * 2) {
                    let bytes = chunk.len() * 2;
                    let buffer = MFCreateMemoryBuffer(bytes as u32)?;
                    let mut ptr = std::ptr::null_mut();
                    buffer.Lock(&mut ptr, None, None)?;
                    std::ptr::copy_nonoverlapping(chunk.as_ptr() as *const u8, ptr, bytes);
                    buffer.Unlock()?;
                    buffer.SetCurrentLength(bytes as u32)?;
                    let sample = MFCreateSample()?;
                    sample.AddBuffer(&buffer)?;
                    let frames = chunk.len() / 2;
                    sample.SetSampleTime((frame as i64) * 10_000_000 / RATE as i64)?;
                    sample.SetSampleDuration((frames as i64) * 10_000_000 / RATE as i64)?;
                    writer.WriteSample(stream, &sample)?;
                    frame += frames;
                }
                writer.Finalize()?;
                Ok(())
            })();
            let _ = MFShutdown();
            CoUninitialize();
            result.map_err(|e| anyhow!("Windows AAC encoder: {e}"))
        }
    })
    .join()
    .map_err(|_| anyhow!("audio encoder thread panicked"))?
}

#[cfg(not(windows))]
fn encode_aac(pcm: &[f32], out: &Path) -> Result<()> {
    // Elsewhere ffmpeg does the encoding: write a WAV and convert it.
    let wav = out.with_extension("wav");
    let samples: Vec<i16> = pcm.iter().map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16).collect();
    std::fs::write(&wav, crate::audio::wav_bytes(&samples, 2, RATE))?;
    let status = std::process::Command::new("ffmpeg").arg("-y").arg("-loglevel").arg("error").arg("-i").arg(&wav).args(["-c:a", "aac", "-b:a", "192k"]).arg(out).status();
    let _ = std::fs::remove_file(&wav);
    match status {
        Ok(s) if s.success() => Ok(()),
        _ => Err(anyhow!("adding music needs ffmpeg on this system")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_sound_silences_the_show_bed() {
        let wav = |v: i16| -> std::sync::Arc<[u8]> { crate::audio::wav_bytes(&vec![v; RATE as usize], 1, RATE).into() };
        let bed = CapturedClip { source: ClipSource::Wav(wav(8000)), bus: Bus::Show, volume: 1.0, looped: true, start: -0.5, stop: None };
        let scene = CapturedClip { source: ClipSource::Wav(wav(16000)), bus: Bus::Graphic, volume: 1.0, looped: false, start: 1.0, stop: None };
        let out = mix(&[bed, scene], 3.0);
        let at = |s: f64| out[(s * RATE as f64) as usize * 2];
        assert!((at(0.5) - 8000.0 / 32768.0).abs() < 0.01, "bed plays");
        assert!((at(1.5) - 16000.0 / 32768.0).abs() < 0.01, "scene sound replaces the bed");
        assert!((at(2.5) - 8000.0 / 32768.0).abs() < 0.01, "bed returns after the scene sound ends");
    }

    /// A real export: 2 s of video, a tone, Windows' AAC encoder, then the remux.
    #[cfg(windows)]
    #[test]
    fn music_is_added_to_an_mp4() {
        let dir = std::env::temp_dir().join(format!("cwx-mix-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let video = crate::record::export_animation_mp4(
            |t| {
                let mut p = tiny_skia::Pixmap::new(320, 180).unwrap();
                p.fill(tiny_skia::Color::from_rgba8((t * 100.0) as u8, 40, 90, 255));
                p
            },
            2.0,
            30,
            dir.join("t.mp4"),
        )
        .unwrap();
        let tone: Vec<i16> = (0..RATE * 3).map(|i| ((i as f32 * 440.0 * std::f32::consts::TAU / RATE as f32).sin() * 8000.0) as i16).collect();
        let wav = dir.join("tone.wav");
        std::fs::write(&wav, crate::audio::wav_bytes(&tone, 1, RATE)).unwrap();
        add_file_to_mp4(&video, &wav).unwrap();
        let f = std::fs::File::open(&video).unwrap();
        let size = f.metadata().unwrap().len();
        let r = mp4::Mp4Reader::read_header(std::io::BufReader::new(f), size).unwrap();
        let kinds: Vec<_> = r.tracks().values().filter_map(|t| t.track_type().ok()).collect();
        assert!(kinds.contains(&mp4::TrackType::Video) && kinds.contains(&mp4::TrackType::Audio), "{kinds:?}");
        let audio = r.tracks().values().find(|t| t.track_type().ok() == Some(mp4::TrackType::Audio)).unwrap();
        assert!(audio.duration().as_secs_f64() <= 2.2, "trimmed to the video: {:?}", audio.duration());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
