//! One borderless, always-on-top window per monitor showing the overlay through softbuffer.

use crate::capture::Screen;
use snapline_overlay::render;
use snapline_overlay::session::Session;
use snapline_overlay::tiny_skia::Pixmap;
use std::num::NonZeroU32;
use std::rc::Rc;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowLevel};

pub struct OverlayWindow {
    pub window: Rc<Window>,
    _context: softbuffer::Context<Rc<Window>>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    frame: Pixmap,
    /// Where the monitor starts on the virtual desktop, in physical pixels.
    origin: PhysicalPosition<i32>,
}

impl OverlayWindow {
    /// Opens a hidden window covering `screen`. Show it after the first `present`.
    pub fn open(event_loop: &ActiveEventLoop, screen: &Screen) -> Result<Self, String> {
        let (w, h) = (screen.image.width(), screen.image.height());
        let attributes = Window::default_attributes()
            .with_title("Snapline")
            .with_decorations(false)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_position(PhysicalPosition::new(screen.x, screen.y))
            .with_inner_size(PhysicalSize::new(w, h))
            .with_visible(false);
        #[cfg(windows)]
        let attributes = {
            use winit::platform::windows::WindowAttributesExtWindows;
            attributes.with_skip_taskbar(true)
        };
        let window = Rc::new(
            event_loop
                .create_window(attributes)
                .map_err(|e| e.to_string())?,
        );
        let context = softbuffer::Context::new(window.clone()).map_err(|e| e.to_string())?;
        let mut surface =
            softbuffer::Surface::new(&context, window.clone()).map_err(|e| e.to_string())?;
        let (nw, nh) = NonZeroU32::new(w)
            .zip(NonZeroU32::new(h))
            .ok_or("empty monitor")?;
        surface.resize(nw, nh).map_err(|e| e.to_string())?;
        let frame = Pixmap::new(w, h).ok_or("empty monitor")?;
        let origin = PhysicalPosition::new(screen.x, screen.y);
        let overlay = Self {
            window,
            _context: context,
            surface,
            frame,
            origin,
        };
        // Creating the window on a monitor with another scale makes Windows resize and move it; put it back.
        overlay.fit_to_monitor();
        Ok(overlay)
    }

    /// Puts the window exactly over its monitor again (after Windows rescaled or moved it).
    pub fn fit_to_monitor(&self) {
        let _ = self.window.request_inner_size(self.size());
        self.window.set_outer_position(self.origin);
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        PhysicalSize::new(self.frame.width(), self.frame.height())
    }

    /// Draws overlay `i` and puts it on screen.
    pub fn present(&mut self, session: &Session, i: usize) -> Result<(), String> {
        render::render_screen(&mut self.frame, session, i);
        let mut buffer = self.surface.buffer_mut().map_err(|e| e.to_string())?;
        // ponytail: full-frame copy per redraw (~10 ms at 4K); redraw only the dirty area if it feels slow.
        for (dst, src) in buffer.iter_mut().zip(self.frame.pixels()) {
            *dst = (u32::from(src.red()) << 16)
                | (u32::from(src.green()) << 8)
                | u32::from(src.blue());
        }
        buffer.present().map_err(|e| e.to_string())
    }
}
