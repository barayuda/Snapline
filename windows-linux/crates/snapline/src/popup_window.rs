//! The status popup's window: borderless, always on top, never taking the focus, drawn through softbuffer.

use snapline_overlay::popup::Popup;
use snapline_overlay::text::TextFont;
use snapline_overlay::tiny_skia::Pixmap;
use std::num::NonZeroU32;
use std::rc::Rc;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowLevel};

pub struct PopupWindow {
    pub window: Rc<Window>,
    _context: softbuffer::Context<Rc<Window>>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    frame: Pixmap,
    /// Pixels per point on the monitor the popup is on.
    pub scale: f64,
}

impl PopupWindow {
    pub fn open(
        event_loop: &ActiveEventLoop,
        at: PhysicalPosition<i32>,
        size: PhysicalSize<u32>,
        scale: f64,
    ) -> Result<Self, String> {
        let attributes = Window::default_attributes()
            .with_title("Snapline")
            .with_decorations(false)
            .with_resizable(false)
            .with_active(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_position(at)
            .with_inner_size(size)
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
        crate::platform::never_activate(&window);
        let context = softbuffer::Context::new(window.clone()).map_err(|e| e.to_string())?;
        let surface =
            softbuffer::Surface::new(&context, window.clone()).map_err(|e| e.to_string())?;
        let frame = Pixmap::new(size.width.max(1), size.height.max(1)).ok_or("empty popup")?;
        let mut popup = Self {
            window,
            _context: context,
            surface,
            frame,
            scale,
        };
        popup.place(at, size)?;
        Ok(popup)
    }

    /// Moves and resizes (the height changes with the content).
    pub fn place(
        &mut self,
        at: PhysicalPosition<i32>,
        size: PhysicalSize<u32>,
    ) -> Result<(), String> {
        let (w, h) = (size.width.max(1), size.height.max(1));
        if (w, h) != (self.frame.width(), self.frame.height()) {
            self.frame = Pixmap::new(w, h).ok_or("empty popup")?;
        }
        let (nw, nh) = NonZeroU32::new(w)
            .zip(NonZeroU32::new(h))
            .ok_or("empty popup")?;
        self.surface.resize(nw, nh).map_err(|e| e.to_string())?;
        let _ = self.window.request_inner_size(PhysicalSize::new(w, h));
        self.window.set_outer_position(at);
        Ok(())
    }

    pub fn present(&mut self, popup: &Popup, font: &TextFont) -> Result<(), String> {
        popup.render(&mut self.frame, font, self.scale);
        let mut buffer = self.surface.buffer_mut().map_err(|e| e.to_string())?;
        for (dst, src) in buffer.iter_mut().zip(self.frame.pixels()) {
            // softbuffer has no alpha: the corners outside the rounded box show black.
            let c = src.demultiply();
            *dst = (u32::from(c.red()) << 16) | (u32::from(c.green()) << 8) | u32::from(c.blue());
        }
        buffer.present().map_err(|e| e.to_string())?;
        self.window.set_visible(true);
        Ok(())
    }
}
