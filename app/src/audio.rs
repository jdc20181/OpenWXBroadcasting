//! Audio: graphic soundtracks and alert sounds on chosen outputs (with failover to the default
//! device), ducking, input meters with peak and short-term loudness (LUFS estimate), voice-over
//! recording to WAV, and the sync offset. All playback runs on one audio thread because output
//! streams can't move between threads.

use std::{
    path::PathBuf,
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex, OnceLock,
    },
    time::Duration,
};

use rodio::cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bus {
    /// Soundtracks of graphics on Program.
    Graphic,
    /// Warning alert tones.
    Alert,
    /// Scheduled audio cues.
    Cue,
    /// The show soundtrack: one bed looping across every scene of a running show.
    Show,
}

enum AudioInput {
    File(PathBuf),
    Wav(Arc<[u8]>),
}

enum Cmd {
    Play { input: AudioInput, bus: Bus, volume: f32, delay_ms: u64, looped: bool },
    Stop(Bus),
    Duck(bool),
    Devices { program: String, alert: String },
    PresentationMix { muted: bool, graphic_gain: f32 },
}

/// Meter readings from the selected input.
#[derive(Clone, Copy, Debug, Default)]
pub struct Meter {
    /// dBFS peak (last ~100 ms).
    pub peak_db: f32,
    /// Short-term loudness estimate (3 s, LUFS-like, without the full K-weighting filter).
    pub short_lufs: f32,
    pub active: bool,
}

fn sender() -> &'static Mutex<Option<Sender<Cmd>>> {
    static S: OnceLock<Mutex<Option<Sender<Cmd>>>> = OnceLock::new();
    S.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Cmd>();
        let _ = std::thread::Builder::new().name("audio".into()).spawn(move || audio_thread(rx));
        Mutex::new(Some(tx))
    })
}

fn send(cmd: Cmd) {
    if let Some(tx) = sender().lock().ok().and_then(|s| s.clone()) {
        let _ = tx.send(cmd);
    }
}

pub fn output_devices() -> Vec<String> {
    rodio::cpal::default_host().output_devices().map(|d| d.filter_map(|x| x.description().ok().map(|d| d.name().to_string())).collect()).unwrap_or_default()
}

pub fn input_devices() -> Vec<String> {
    rodio::cpal::default_host().input_devices().map(|d| d.filter_map(|x| x.description().ok().map(|d| d.name().to_string())).collect()).unwrap_or_default()
}

pub fn play(path: PathBuf, bus: Bus, volume: f32, delay_ms: u64) {
    note_play(ClipSource::File(path.clone()), bus, volume, delay_ms, false);
    send(Cmd::Play { input: AudioInput::File(path), bus, volume, delay_ms, looped: false });
}

/// Play a file on repeat until the bus is stopped (show soundtrack).
pub fn play_loop(path: PathBuf, bus: Bus, volume: f32) {
    note_play(ClipSource::File(path.clone()), bus, volume, 0, true);
    send(Cmd::Play { input: AudioInput::File(path), bus, volume, delay_ms: 0, looped: true });
}

pub fn play_clip(wav: Arc<[u8]>, delay_ms: u64) {
    note_play(ClipSource::Wav(wav.clone()), Bus::Graphic, 1.0, delay_ms, false);
    send(Cmd::Play { input: AudioInput::Wav(wav), bus: Bus::Graphic, volume: 1.0, delay_ms, looped: false });
}

pub fn stop(bus: Bus) {
    note_stop(bus);
    send(Cmd::Stop(bus));
}

// ---- Capture: what played while a video was recorded, so it can be mixed into the file ----

/// Where a played sound came from.
#[derive(Clone)]
pub enum ClipSource {
    File(PathBuf),
    Wav(Arc<[u8]>),
}

/// One sound on the recording's timeline (seconds from the start of the video; `start` is
/// negative for a sound that was already playing when the recording began).
#[derive(Clone)]
pub struct CapturedClip {
    pub source: ClipSource,
    pub bus: Bus,
    pub volume: f32,
    pub looped: bool,
    pub start: f64,
    pub stop: Option<f64>,
}

struct Playing {
    source: ClipSource,
    bus: Bus,
    volume: f32,
    looped: bool,
    at: std::time::Instant,
}

#[derive(Default)]
struct CaptureState {
    playing: Vec<Playing>,
    capture: Option<(std::time::Instant, Vec<CapturedClip>)>,
}

fn capture_state() -> &'static Mutex<CaptureState> {
    static S: OnceLock<Mutex<CaptureState>> = OnceLock::new();
    S.get_or_init(Default::default)
}

fn note_play(source: ClipSource, bus: Bus, volume: f32, delay_ms: u64, looped: bool) {
    let at = std::time::Instant::now() + Duration::from_millis(delay_ms);
    let Ok(mut s) = capture_state().lock() else { return };
    // Same rule as the audio thread: a new sound replaces the one on its bus (alerts stack).
    if bus != Bus::Alert {
        s.playing.retain(|p| p.bus != bus);
    }
    s.playing.push(Playing { source: source.clone(), bus, volume, looped, at });
    if let Some((started, clips)) = s.capture.as_mut() {
        let now = started.elapsed().as_secs_f64();
        if bus != Bus::Alert {
            for c in clips.iter_mut().filter(|c| c.bus == bus && c.stop.is_none()) {
                c.stop = Some(now);
            }
        }
        let start = at.saturating_duration_since(*started).as_secs_f64();
        clips.push(CapturedClip { source, bus, volume, looped, start, stop: None });
    }
}

fn note_stop(bus: Bus) {
    let Ok(mut s) = capture_state().lock() else { return };
    s.playing.retain(|p| p.bus != bus);
    if let Some((started, clips)) = s.capture.as_mut() {
        let now = started.elapsed().as_secs_f64();
        for c in clips.iter_mut().filter(|c| c.bus == bus && c.stop.is_none()) {
            c.stop = Some(now);
        }
    }
}

/// Start logging every sound played from now on (sounds already playing are included).
pub fn capture_start() {
    let Ok(mut s) = capture_state().lock() else { return };
    let started = std::time::Instant::now();
    let clips = s
        .playing
        .iter()
        .map(|p| CapturedClip {
            source: p.source.clone(),
            bus: p.bus,
            volume: p.volume,
            looped: p.looped,
            // Negative when it started before the recording: the mix starts part-way in.
            start: if p.at <= started { -(started - p.at).as_secs_f64() } else { (p.at - started).as_secs_f64() },
            stop: None,
        })
        .collect();
    s.capture = Some((started, clips));
}

/// Stop logging and return what played during the recording.
pub fn capture_take() -> Vec<CapturedClip> {
    capture_state().lock().ok().and_then(|mut s| s.capture.take()).map(|(_, c)| c).unwrap_or_default()
}

/// Lower the graphic bus (voice-over or an alert is speaking).
pub fn duck(on: bool) {
    send(Cmd::Duck(on));
}

pub fn set_devices(program: &str, alert: &str) {
    send(Cmd::Devices { program: program.to_string(), alert: alert.to_string() });
}

pub fn set_presentation_mix(muted: bool, graphic_gain: f32) {
    send(Cmd::PresentationMix { muted, graphic_gain: graphic_gain.clamp(0.0, 2.0) });
}

/// Output buffer in frames: about 85 ms at 48 kHz. rodio's default (~43 ms) underruns while the
/// map and compositor load the CPU, which is heard as choppy, stuttering music.
const OUTPUT_BUFFER_FRAMES: u32 = 4096;

/// Open `device` with the roomier buffer; if the device refuses that size, with rodio's default.
fn open_device(device: rodio::cpal::Device) -> Result<rodio::MixerDeviceSink, rodio::DeviceSinkError> {
    let roomy = rodio::DeviceSinkBuilder::from_device(device.clone())
        .map(|b| b.with_buffer_size(rodio::cpal::BufferSize::Fixed(OUTPUT_BUFFER_FRAMES)))
        .and_then(|b| b.open_stream());
    roomy.or_else(|e| {
        log::warn!("audio output refused a {OUTPUT_BUFFER_FRAMES}-frame buffer ({e}); using its default");
        rodio::DeviceSinkBuilder::from_device(device).and_then(|b| b.open_stream())
    })
}

fn open_default() -> Option<rodio::MixerDeviceSink> {
    match rodio::cpal::default_host().default_output_device() {
        Some(device) => open_device(device).ok().or_else(|| rodio::DeviceSinkBuilder::open_default_sink().ok()),
        None => rodio::DeviceSinkBuilder::open_default_sink().ok(),
    }
}

/// An opened output: the device, plus the bus every player connects to. The bus runs through a
/// peak limiter before the device, so several buses at once (show bed + scene audio + alert tone)
/// or a volume above 100% never clip, which was heard as harsh, distorted audio.
struct Output {
    _device: rodio::MixerDeviceSink,
    bus: rodio::mixer::Mixer,
}

fn limited(device: rodio::MixerDeviceSink) -> Output {
    use rodio::Source;
    let (channels, rate) = (device.config().channel_count(), device.config().sample_rate());
    let (bus, mix) = rodio::mixer::mixer(channels, rate);
    // A silent source keeps the bus alive between tracks (an empty mixer ends and detaches).
    bus.add(rodio::source::Zero::new(channels, rate));
    device.mixer().add(mix.limit(rodio::source::LimitSettings::broadcast()));
    Output { _device: device, bus }
}

fn open_stream(name: &str) -> Option<Output> {
    open_device_by_name(name).map(limited)
}

fn open_device_by_name(name: &str) -> Option<rodio::MixerDeviceSink> {
    let host = rodio::cpal::default_host();
    let device = if name.is_empty() { None } else { host.output_devices().ok()?.find(|d| d.description().map(|d| d.name() == name).unwrap_or(false)) };
    match device {
        Some(d) => match open_device(d) {
            Ok(s) => Some(s),
            Err(e) => {
                // Output failover: the chosen device is gone or busy; use the default.
                log::warn!("audio device \"{name}\" failed ({e}); using the default output");
                crate::broadcast_ops::log_incident("Audio output failover", &format!("\"{name}\" failed: {e}"));
                open_default()
            }
        },
        None => {
            if !name.is_empty() {
                log::warn!("audio device \"{name}\" not found; using the default output");
                crate::broadcast_ops::log_incident("Audio output failover", &format!("\"{name}\" not found"));
            }
            open_default()
        }
    }
}

/// Decode a track from memory: the whole file is read before playback starts, so the real-time
/// audio callback never waits on the disk (a disk or antivirus stall there is an audible dropout).
fn decode_file(path: &std::path::Path) -> Option<rodio::Decoder<std::io::Cursor<Vec<u8>>>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => {
            log::warn!("audio file missing: {}", path.display());
            return None;
        }
    };
    match rodio::Decoder::try_from(std::io::Cursor::new(bytes)) {
        Ok(source) => Some(source),
        Err(_) => {
            log::warn!("can't decode {}", path.display());
            None
        }
    }
}

fn audio_thread(rx: mpsc::Receiver<Cmd>) {
    let (mut program_name, mut alert_name) = (String::new(), String::new());
    let mut program = open_stream("");
    let mut alert: Option<Output> = None;
    let mut sinks: Vec<(Bus, rodio::Player, f32)> = Vec::new();
    let mut ducked = false;
    let mut presentation_muted = false;
    let mut graphic_gain = 1.0;
    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(Cmd::Devices { program: p, alert: a }) => {
                if p != program_name {
                    program_name = p;
                    program = open_stream(&program_name);
                }
                if a != alert_name {
                    alert_name = a;
                    alert = if alert_name.is_empty() { None } else { open_stream(&alert_name) };
                }
            }
            Ok(Cmd::Play { input, bus, volume, delay_ms, looped }) => {
                let stream = if bus == Bus::Alert { alert.as_ref().or(program.as_ref()) } else { program.as_ref() };
                let Some(stream) = stream else { continue };
                let source: Box<dyn rodio::Source + Send> = match input {
                    AudioInput::File(path) => {
                        let Some(source) = decode_file(&path) else { continue };
                        if looped {
                            use rodio::Source;
                            Box::new(source.repeat_infinite())
                        } else {
                            Box::new(source)
                        }
                    }
                    AudioInput::Wav(bytes) => {
                        let Ok(source) = rodio::Decoder::try_from(std::io::Cursor::new(bytes)) else {
                            log::warn!("can't decode MP4 clip audio");
                            continue;
                        };
                        Box::new(source)
                    }
                };
                if bus != Bus::Alert {
                    sinks.retain(|(b, s, _)| {
                        if *b == bus {
                            s.stop();
                        }
                        *b != bus
                    });
                }
                let sink = rodio::Player::connect_new(&stream.bus);
                sink.set_volume(0.0);
                if delay_ms > 0 {
                    use rodio::Source;
                    sink.append(source.delay(Duration::from_millis(delay_ms)));
                } else {
                    sink.append(source);
                }
                sinks.push((bus, sink, volume.clamp(0.0, 2.0)));
            }
            Ok(Cmd::Stop(bus)) => sinks.retain(|(b, s, _)| {
                if *b == bus {
                    s.stop();
                }
                *b != bus
            }),
            Ok(Cmd::Duck(on)) => ducked = on,
            Ok(Cmd::PresentationMix { muted, graphic_gain: gain }) => {
                presentation_muted = muted;
                graphic_gain = gain;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        sinks.retain(|(_, player, _)| !player.empty());
        let duck_graphics = ducked || sinks.iter().any(|(bus, _, _)| *bus == Bus::Alert);
        // A scene's own soundtrack overrides the show bed while it plays.
        let scene_audio = sinks.iter().any(|(bus, _, _)| *bus == Bus::Graphic);
        for (bus, player, base) in &sinks {
            let volume = match bus {
                Bus::Graphic | Bus::Cue | Bus::Show if presentation_muted => 0.0,
                Bus::Show if scene_audio => 0.0,
                Bus::Show if duck_graphics => base * 0.25,
                Bus::Show => *base,
                Bus::Graphic if duck_graphics => base * graphic_gain * 0.25,
                Bus::Graphic => base * graphic_gain,
                Bus::Alert | Bus::Cue => *base,
            };
            player.set_volume(volume);
        }
    }
}

// ---- Input: meters, loudness, voice-over recording, automatic ducking ----

struct InputState {
    meter: Meter,
    recording: Option<(PathBuf, Vec<i16>, u32, u16)>,
}

fn input_state() -> &'static Arc<Mutex<InputState>> {
    static S: OnceLock<Arc<Mutex<InputState>>> = OnceLock::new();
    S.get_or_init(|| Arc::new(Mutex::new(InputState { meter: Meter::default(), recording: None })))
}

pub fn meter() -> Meter {
    input_state().lock().map(|s| s.meter).unwrap_or_default()
}

/// Start listening to an input device (empty = default) on its own thread. Re-calling with a new
/// name switches devices. `duck_db`: duck the soundtrack when the input is louder than this.
pub fn start_input(name: &str, duck_db: Option<f32>) {
    static CURRENT: OnceLock<Mutex<Option<Sender<()>>>> = OnceLock::new();
    let slot = CURRENT.get_or_init(|| Mutex::new(None));
    if let Ok(mut s) = slot.lock() {
        if let Some(stop) = s.take() {
            let _ = stop.send(());
        }
        let (tx, rx) = mpsc::channel::<()>();
        *s = Some(tx);
        let name = name.to_string();
        let _ = std::thread::Builder::new().name("audio-in".into()).spawn(move || {
            let host = rodio::cpal::default_host();
            let device = if name.is_empty() { host.default_input_device() } else { host.input_devices().ok().and_then(|mut d| d.find(|x| x.description().map(|d| d.name() == name).unwrap_or(false))) };
            let Some(device) = device else {
                log::warn!("audio input \"{name}\" not found");
                return;
            };
            let Ok(config) = device.default_input_config() else { return };
            let rate = config.sample_rate();
            let channels = config.channels();
            let state = input_state().clone();
            let window = Arc::new(Mutex::new(Vec::<f32>::new()));
            let w2 = window.clone();
            let mut ducking = false;
            let mut handle = move |data: &[f32]| {
                let peak = data.iter().fold(0.0f32, |a, x| a.max(x.abs()));
                let peak_db = 20.0 * peak.max(1e-6).log10();
                // Short-term loudness: mean square over ~3 s, -0.691 offset as in BS.1770.
                let short = if let Ok(mut w) = w2.lock() {
                    w.extend(data.iter().map(|x| x * x));
                    let keep = (rate as usize * channels as usize * 3).max(1);
                    if w.len() > keep {
                        let cut = w.len() - keep;
                        w.drain(..cut);
                    }
                    let ms = w.iter().sum::<f32>() / w.len().max(1) as f32;
                    -0.691 + 10.0 * ms.max(1e-10).log10()
                } else {
                    -70.0
                };
                if let Ok(mut s) = state.lock() {
                    s.meter = Meter { peak_db, short_lufs: short, active: true };
                    if let Some((_, buf, _, _)) = s.recording.as_mut() {
                        buf.extend(data.iter().map(|x| (x.clamp(-1.0, 1.0) * 32767.0) as i16));
                    }
                }
                if let Some(th) = duck_db {
                    let loud = peak_db > th;
                    if loud != ducking {
                        ducking = loud;
                        duck(loud);
                    }
                }
            };
            let mut last_error = None::<std::time::Instant>;
            let err = move |e: rodio::cpal::Error| {
                let now = std::time::Instant::now();
                // Buffer under/overruns on the input meter are harmless (a missed meter update)
                // and happen whenever the PC is busy: note them quietly. Real errors still warn.
                let xrun = e.to_string().contains("underrun") || e.to_string().contains("overrun");
                if xrun {
                    log::debug!("audio input: {e}");
                } else if last_error.is_none_or(|at| now.duration_since(at) >= Duration::from_secs(60)) {
                    log::warn!("audio input error: {e}");
                    last_error = Some(now);
                }
            };
            let stream = match config.sample_format() {
                rodio::cpal::SampleFormat::F32 => device.build_input_stream(config.into(), move |d: &[f32], _| handle(d), err, None),
                rodio::cpal::SampleFormat::I16 => device.build_input_stream(config.into(), move |d: &[i16], _| handle(&d.iter().map(|x| *x as f32 / 32768.0).collect::<Vec<_>>()), err, None),
                _ => return,
            };
            let Ok(stream) = stream else { return };
            if stream.play().is_err() {
                return;
            }
            if let Ok(mut s) = input_state().lock() {
                if let Some(r) = s.recording.as_mut() {
                    r.2 = rate;
                    r.3 = channels;
                }
            }
            // Keep the stream alive until told to switch.
            let _ = rx.recv();
            drop(stream);
            let _ = window;
        });
    }
}

/// Begin a voice-over recording into the media library.
pub fn start_recording(rate_hint: u32) -> PathBuf {
    let path = crate::config::LibraryPaths::get().media.join(format!("Voice-over {}.wav", chrono::Local::now().format("%Y-%m-%d %H-%M-%S")));
    if let Ok(mut s) = input_state().lock() {
        s.recording = Some((path.clone(), Vec::new(), rate_hint, 1));
    }
    path
}

pub fn recording() -> bool {
    input_state().lock().map(|s| s.recording.is_some()).unwrap_or(false)
}

/// Finish the recording and write a 16-bit PCM WAV.
pub fn stop_recording(rate: u32, channels: u16) -> anyhow::Result<PathBuf> {
    let (path, samples, r, ch) = input_state().lock().ok().and_then(|mut s| s.recording.take()).ok_or_else(|| anyhow::anyhow!("not recording"))?;
    let (rate, channels) = if r > 0 { (r, ch.max(1)) } else { (rate, channels) };
    std::fs::write(&path, wav_bytes(&samples, channels, rate))?;
    Ok(path)
}

/// A 16-bit PCM WAV file in memory.
pub fn wav_bytes(samples: &[i16], channels: u16, rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decode every bundled track to the end, the way playback does (from memory), and report
    /// formats. A track that fails part-way or has an odd rate/channel layout is a likely cause
    /// of "choppy and disoriented" playback.
    #[test]
    #[ignore = "reads the full music bundle (~800 MB)"]
    fn every_bundled_track_decodes_cleanly() {
        use rodio::Source;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/music");
        let mut files: Vec<PathBuf> = Vec::new();
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if matches!(path.extension().and_then(|e| e.to_str()), Some("ogg" | "mp3" | "wav")) {
                    files.push(path);
                }
            }
        }
        let mut formats = std::collections::BTreeMap::<(u32, u16), usize>::new();
        let mut failed = Vec::new();
        for path in &files {
            let Some(source) = decode_file(path) else {
                failed.push(format!("{}: can't open or decode", path.display()));
                continue;
            };
            let (rate, channels) = (source.sample_rate().get(), source.channels().get());
            *formats.entry((rate, channels)).or_default() += 1;
            let samples = source.count();
            let seconds = samples as f64 / rate as f64 / channels as f64;
            if seconds < 1.0 {
                failed.push(format!("{}: only {seconds:.2} s decoded", path.display()));
            }
        }
        eprintln!("{} tracks; formats (rate, channels): {formats:?}", files.len());
        assert!(failed.is_empty(), "tracks that don't decode cleanly:\n{}", failed.join("\n"));
    }

    /// Two buses at full level plus 200% volume would peak near 4.0 (hard clipping); the limited
    /// bus keeps every sample at or below full scale.
    #[test]
    fn bus_limiter_prevents_clipping() {
        use rodio::Source;
        let (ch, rate) = (std::num::NonZero::new(2).unwrap(), std::num::NonZero::new(48_000).unwrap());
        let (bus, mix) = rodio::mixer::mixer(ch, rate);
        bus.add(rodio::source::SineWave::new(220.0).amplify(2.0).take_duration(Duration::from_secs(1)));
        bus.add(rodio::source::SineWave::new(330.0).amplify(2.0).take_duration(Duration::from_secs(1)));
        let samples: Vec<f32> = mix.limit(rodio::source::LimitSettings::broadcast()).collect();
        // Skip the 3 ms attack, then everything must sit under 0 dBFS.
        let peak = samples.iter().skip(48_000 * 2 / 50).fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak <= 1.0, "peak {peak}");
    }
}
