//! Freezes every monitor at full pixel resolution.

use snapline_overlay::tiny_skia::{IntSize, Pixmap};

pub struct Screen {
    /// Top-left corner on the virtual desktop, in physical pixels.
    pub x: i32,
    pub y: i32,
    pub image: Pixmap,
}

pub fn capture_all() -> Result<Vec<Screen>, String> {
    let monitors = xcap::Monitor::all().map_err(|e| e.to_string())?;
    monitors
        .iter()
        .map(|m| {
            let image = m.capture_image().map_err(|e| e.to_string())?;
            let (w, h) = image.dimensions();
            let mut data = image.into_raw();
            for px in data.as_chunks_mut::<4>().0 {
                px[3] = 255; // some drivers leave alpha at 0; the screen is opaque
            }
            let size = IntSize::from_wh(w, h).ok_or("a monitor reported an empty image")?;
            let image = Pixmap::from_vec(data, size).ok_or("a monitor image had the wrong size")?;
            Ok(Screen {
                x: m.x().map_err(|e| e.to_string())?,
                y: m.y().map_err(|e| e.to_string())?,
                image,
            })
        })
        .collect()
}
