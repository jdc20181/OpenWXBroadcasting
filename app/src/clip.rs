use std::{fs, io::BufReader, path::{Path, PathBuf}, process::Command};

use anyhow::{ensure, Context, Result};
use image::{AnimationDecoder, ImageDecoder};

const MAX_MS: u32 = 60_000;
const MAX_BYTES: usize = 96 * 1024 * 1024;
const MAX_FRAMES: usize = 900;

pub struct Clip {
    frames: Vec<(u32, Vec<u8>)>,
    pub audio: Option<std::sync::Arc<[u8]>>,
}

impl Clip {
    pub fn load(name: &str) -> Result<Self> {
        let path = Path::new(name);
        ensure!(path.file_name().is_some_and(|file| file == path.as_os_str()), "Invalid media library filename");
        Self::load_path(&crate::config::LibraryPaths::get().media.join(path))
    }

    fn load_path(path: &Path) -> Result<Self> {
        ensure!(fs::metadata(path)?.len() <= 128 * 1024 * 1024, "Clip file exceeds 128 MB");
        let mut clip = Self { frames: Vec::new(), audio: None };
        let mut total = 0;
        match path.extension().and_then(|ext| ext.to_str()).unwrap_or_default().to_ascii_lowercase().as_str() {
            "gif" => {
                let mut decoder = image::codecs::gif::GifDecoder::new(BufReader::new(fs::File::open(&path)?))?;
                let mut limits = image::Limits::default();
                limits.max_image_width = Some(4096);
                limits.max_image_height = Some(4096);
                decoder.set_limits(limits)?;
                let mut end = 0u32;
                for frame in decoder.into_frames() {
                    let frame = frame?;
                    let (num, denom) = frame.delay().numer_denom_ms();
                    end = end.saturating_add(num.div_ceil(denom.max(1)).max(20));
                    let image = image::DynamicImage::ImageRgba8(frame.into_buffer()).thumbnail(960, 540);
                    let mut bytes = std::io::Cursor::new(Vec::new());
                    image.write_to(&mut bytes, image::ImageFormat::Png)?;
                    clip.push(end, bytes.into_inner(), &mut total)?;
                }
            }
            "mp4" => {
                let parent = crate::config::tile_cache_dir().join("show-clips");
                fs::create_dir_all(&parent)?;
                let dir = parent.join(crate::gfx::model::new_id());
                fs::create_dir(&dir)?;
                let temp = ClipTemp(dir);
                let result = ffmpeg(path)
                    .args(["-an", "-sn", "-vf", "fps=15,scale=960:540:force_original_aspect_ratio=decrease:force_divisible_by=2",
                        "-frames:v", "901", "-q:v", "4"])
                    .arg(temp.0.join("%04d.jpg"))
                    .output().context("MP4 playback requires FFmpeg on PATH")?;
                ensure!(result.status.success(), "FFmpeg could not decode this MP4");
                for i in 1..=MAX_FRAMES + 1 {
                    let path = temp.0.join(format!("{i:04}.jpg"));
                    if !path.is_file() { break; }
                    clip.push((i as u32 * 1000).div_ceil(15), fs::read(path)?, &mut total)?;
                }
                let audio = temp.0.join("audio.wav");
                // A silent MP4 has no audio stream. Keep the video playable in that case.
                if ffmpeg(path).args(["-map", "0:a:0", "-vn", "-sn", "-t", "60", "-ar", "48000", "-ac", "2", "-c:a", "pcm_s16le"])
                    .arg(&audio).output().is_ok_and(|result| result.status.success())
                {
                    clip.audio = fs::read(audio).ok().map(std::sync::Arc::from);
                }
            }
            _ => anyhow::bail!("Choose a GIF or MP4 from the media library"),
        }
        ensure!(!clip.frames.is_empty(), "Clip contains no video frames");
        Ok(clip)
    }

    fn push(&mut self, end: u32, bytes: Vec<u8>, total: &mut usize) -> Result<()> {
        *total += bytes.len();
        ensure!(end <= MAX_MS && self.frames.len() < MAX_FRAMES && *total <= MAX_BYTES,
            "Clip exceeds the 60-second or decoded size limit");
        self.frames.push((end, bytes));
        Ok(())
    }

    pub fn seconds(&self) -> f32 { self.frames.last().map_or(0, |frame| frame.0) as f32 / 1000.0 }

    pub fn frame_index(&self, seconds: f32) -> usize {
        self.frames.partition_point(|(end, _)| *end as f32 <= seconds * 1000.0).min(self.frames.len() - 1)
    }

    pub fn draw(&self, index: usize, target: &mut tiny_skia::Pixmap) -> Result<()> {
        let pixels = image::load_from_memory(&self.frames[index].1)?.to_rgba8();
        let (width, height) = pixels.dimensions();
        let mut data = pixels.into_raw();
        for pixel in data.chunks_exact_mut(4) {
            let alpha = u16::from(pixel[3]);
            for channel in &mut pixel[..3] { *channel = (u16::from(*channel) * alpha / 255) as u8; }
        }
        let size = tiny_skia::IntSize::from_wh(width, height).context("Invalid clip size")?;
        let frame = tiny_skia::Pixmap::from_vec(data, size).context("Invalid clip frame")?;
        let scale = (target.width() as f32 / width as f32).min(target.height() as f32 / height as f32);
        let x = (target.width() as f32 - width as f32 * scale) / 2.0;
        let y = (target.height() as f32 - height as f32 * scale) / 2.0;
        let paint = tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..Default::default() };
        target.fill(tiny_skia::Color::BLACK);
        target.draw_pixmap(0, 0, frame.as_ref(), &paint, tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, x, y), None);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gif_clip_preserves_frame_timing_and_holds_the_last_frame() {
        let path = std::env::temp_dir().join(format!("{}.gif", crate::gfx::model::new_id()));
        let mut encoder = gif::Encoder::new(fs::File::create(&path).unwrap(), 2, 2, &[]).unwrap();
        for (mut pixels, delay) in [(vec![255, 0, 0, 255].repeat(4), 10), (vec![0, 255, 0, 255].repeat(4), 25)] {
            let mut frame = gif::Frame::from_rgba_speed(2, 2, &mut pixels, 10);
            frame.delay = delay;
            encoder.write_frame(&frame).unwrap();
        }
        drop(encoder);
        let clip = Clip::load_path(&path);
        let _ = fs::remove_file(path);
        let clip = clip.unwrap();
        assert!((clip.seconds() - 0.35).abs() < 0.001);
        assert_eq!(clip.frame_index(0.05), 0);
        assert_eq!(clip.frame_index(0.2), 1);
        assert_eq!(clip.frame_index(1.0), 1);
        let mut target = tiny_skia::Pixmap::new(2, 2).unwrap();
        clip.draw(1, &mut target).unwrap();
        assert_eq!(&target.data()[..4], &[0, 255, 0, 255]);
    }
}

fn ffmpeg(path: &Path) -> Command {
    let mut command = Command::new("ffmpeg");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command.args(["-nostdin", "-v", "error", "-protocol_whitelist", "file,pipe", "-f", "mov", "-i"]).arg(path);
    command
}

struct ClipTemp(PathBuf);
impl Drop for ClipTemp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
