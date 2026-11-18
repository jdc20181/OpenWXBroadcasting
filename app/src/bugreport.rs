//! In-app bug reports, kept locally for now (single operator, same PC as the developer): each
//! report is a description plus up to three screenshots in the app data folder
//! (`<config dir>/bug-reports`):
//!
//! - `BR-YYYYMMDD-HHMMSS.txt`: the description, when it was made, app version and workspace,
//!   and the names of its screenshots;
//! - `BR-YYYYMMDD-HHMMSS_1.png` … `_3.png`: the screenshots;
//! - `index.log`: one line per report (id, time, first line), newest last.
//!
//! A stopgap until the CrowdedDX Desk board accepts images (then reports go there directly).

use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};

/// Screenshots a report can hold.
pub const MAX_SHOTS: usize = 3;

/// The reports folder, created on first use.
pub fn folder() -> PathBuf {
    let dir = crate::config::config_dir().join("bug-reports");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Screenshots taken for a report not saved yet.
fn pending() -> PathBuf {
    let dir = folder().join("pending");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Capture the window in front (the app, when the report is started from it) into a pending
/// PNG and return its path.
pub fn capture_pending() -> Result<PathBuf> {
    let image = capture_foreground()?;
    let path = pending().join(format!("shot-{}.png", chrono::Local::now().format("%Y%m%d-%H%M%S-%3f")));
    image.save(&path).with_context(|| format!("saving {}", path.display()))?;
    Ok(path)
}

/// Remove pending screenshots (after saving or discarding a report).
pub fn clear_pending() {
    if let Ok(entries) = std::fs::read_dir(pending()) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Save a report: the description and up to [`MAX_SHOTS`] images (any format the image crate
/// reads; stored as PNG). Returns the report id and the text file's path.
pub fn save(description: &str, shots: &[PathBuf], workspace: &str) -> Result<(String, PathBuf)> {
    save_in(&folder(), description, shots, workspace, chrono::Local::now())
}

fn save_in(dir: &Path, description: &str, shots: &[PathBuf], workspace: &str, now: chrono::DateTime<chrono::Local>) -> Result<(String, PathBuf)> {
    let description = description.trim();
    ensure!(!description.is_empty(), "Describe the problem first");
    ensure!(shots.len() <= MAX_SHOTS, "A report holds up to {MAX_SHOTS} screenshots");
    // Unique id: the time, with a suffix if two reports land in the same second.
    let base = format!("BR-{}", now.format("%Y%m%d-%H%M%S"));
    let id = (1..)
        .map(|n| if n == 1 { base.clone() } else { format!("{base}-{n}") })
        .find(|id| !dir.join(format!("{id}.txt")).exists())
        .context("no free report id")?;
    let mut names = Vec::new();
    for (i, shot) in shots.iter().enumerate() {
        let name = format!("{id}_{}.png", i + 1);
        let image = image::open(shot).map_err(|_| anyhow::anyhow!("Screenshot {} could not be read. Remove it and add it again.", i + 1))?;
        image.save(dir.join(&name)).with_context(|| format!("saving {name}"))?;
        names.push(name);
    }
    let text = format!(
        "Report: {id}\nCreated: {}\nApp: OpenWXBroadcasting v{} build {}\nWorkspace: {workspace}\nScreenshots: {}\n\nDescription:\n{description}\n",
        now.format("%Y-%m-%d %-I:%M %p"),
        env!("CARGO_PKG_VERSION"),
        crate::config::APP_BUILD,
        if names.is_empty() { "none".to_string() } else { names.join(", ") },
    );
    let path = dir.join(format!("{id}.txt"));
    std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
    let first = description.lines().next().unwrap_or_default();
    let line = format!("{id}\t{}\t{} screenshot(s)\t{first}\n", now.format("%Y-%m-%d %H:%M"), names.len());
    use std::io::Write as _;
    std::fs::OpenOptions::new().create(true).append(true).open(dir.join("index.log"))?.write_all(line.as_bytes())?;
    Ok((id, path))
}

/// The window in front, as the screen shows it (GDI copy of the desktop over the window's
/// visible frame, so GPU-drawn content is included).
#[cfg(windows)]
fn capture_foreground() -> Result<image::RgbaImage> {
    use windows::Win32::{
        Foundation::RECT,
        Graphics::{
            Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS},
            Gdi::{
                BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT,
                DIB_RGB_COLORS, SRCCOPY,
            },
        },
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect},
    };
    unsafe {
        let hwnd = GetForegroundWindow();
        ensure!(!hwnd.is_invalid(), "No window is in front to capture");
        // The visible frame (GetWindowRect includes the invisible resize border).
        let mut r = RECT::default();
        if DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut RECT as *mut _, std::mem::size_of::<RECT>() as u32).is_err() {
            GetWindowRect(hwnd, &mut r)?;
        }
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        ensure!(w > 0 && h > 0 && w <= 16384 && h <= 16384, "The window has no visible area");
        let screen = GetDC(None);
        ensure!(!screen.is_invalid(), "Could not read the screen");
        let memory = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, w, h);
        let previous = SelectObject(memory, bitmap.into());
        let copied = BitBlt(memory, 0, 0, w, h, Some(screen), r.left, r.top, SRCCOPY | CAPTUREBLT);
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                // Negative height: rows top to bottom.
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; w as usize * h as usize * 4];
        let rows = GetDIBits(memory, bitmap, 0, h as u32, Some(pixels.as_mut_ptr() as *mut _), &mut info, DIB_RGB_COLORS);
        SelectObject(memory, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory);
        ReleaseDC(None, screen);
        copied?;
        ensure!(rows == h, "The screen copy came back incomplete");
        // BGRA to RGBA, fully opaque.
        for px in pixels.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }
        image::RgbaImage::from_raw(w as u32, h as u32, pixels).context("screen copy size mismatch")
    }
}

#[cfg(not(windows))]
fn capture_foreground() -> Result<image::RgbaImage> {
    anyhow::bail!("Screen capture is only available on Windows; add an image file instead")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captures whatever window is in front and checks it is not blank (the image is deleted).
    #[test]
    #[ignore = "captures the screen"]
    fn captures_the_front_window() {
        let image = capture_foreground().unwrap();
        assert!(image.width() > 100 && image.height() > 100);
        let first = image.get_pixel(image.width() / 2, image.height() / 2).0;
        assert!(image.pixels().any(|p| p.0 != first), "capture is a single colour");
        eprintln!("captured {}x{}", image.width(), image.height());
    }

    #[test]
    fn saves_text_screenshots_and_index() {
        let dir = std::env::temp_dir().join(format!("cwx-bug-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let shot = dir.join("in.png");
        image::RgbaImage::from_pixel(4, 3, image::Rgba([10, 20, 30, 255])).save(&shot).unwrap();
        let now = chrono::Local::now();
        let (id, path) = save_in(&dir, "Legend cut off\non the radar tab", &[shot.clone(), shot.clone()], "Map Studio", now).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(&format!("Screenshots: {id}_1.png, {id}_2.png")), "{text}");
        assert!(text.contains("Legend cut off\non the radar tab"));
        assert!(dir.join(format!("{id}_2.png")).exists());
        // Same second: a distinct id.
        let (second, _) = save_in(&dir, "Another", &[], "Graphics", now).unwrap();
        assert_ne!(id, second);
        let index = std::fs::read_to_string(dir.join("index.log")).unwrap();
        assert_eq!(index.lines().count(), 2);
        assert!(save_in(&dir, "  ", &[], "Map", now).is_err());
        assert!(save_in(&dir, "x", &[shot.clone(), shot.clone(), shot.clone(), shot], "Map", now).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
