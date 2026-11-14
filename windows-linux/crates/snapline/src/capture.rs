//! Freezes every monitor at full pixel resolution.

use snapline_overlay::tiny_skia::{IntSize, Pixmap};

pub struct Screen {
    /// Top-left corner on the virtual desktop, in physical pixels.
    pub x: i32,
    pub y: i32,
    /// Pixels per point on this monitor, from Windows (not from our window, which may not have moved there yet).
    pub scale: f64,
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
                scale: f64::from(m.scale_factor().map_err(|e| e.to_string())?),
                image,
            })
        })
        .collect()
}

/// What to say when the capture failed. In a Wayland session the desktop's screenshot portal takes it, and may ask
/// the user first or have been refused, so say where to allow it.
#[must_use]
pub fn capture_problem(error: &str, wayland: bool) -> String {
    let said = format!("Snapline could not take the screenshot: {error}");
    if wayland {
        format!(
            "{said}. In a Wayland session the desktop must allow it: in GNOME, answer Allow when asked, or turn on \
             Settings → Apps → Snapline → Screenshots; in KDE Plasma, System Settings → Application Permissions."
        )
    } else {
        said
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Wayland session (CI: headless sway with a red background) is captured for real, not black: xcap takes it
    /// through wlroots screencopy or the screenshot portal.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "needs a Wayland session with XWayland: CI runs it under headless sway"]
    fn wayland_capture_is_the_real_screen() {
        let screens = capture_all().unwrap();
        assert!(!screens.is_empty());
        let red = screens.iter().any(|s| {
            s.image
                .pixels()
                .iter()
                .any(|p| p.red() > 200 && p.green() < 50 && p.blue() < 50)
        });
        assert!(red, "the red background was not captured");
    }

    // Review Focus 1 (Phase 6b): GNOME asks first; a refused or failed portal capture must say how to allow it.
    #[test]
    fn a_refused_screenshot_says_how_to_allow_it() {
        let text = capture_problem("Portal request didn't succeed: Cancelled", true);
        assert!(
            text.contains("allow") && text.contains("Screenshots") && text.contains("Cancelled"),
            "{text}"
        );
        assert_eq!(
            capture_problem("no display", false),
            "Snapline could not take the screenshot: no display"
        );
    }
}
