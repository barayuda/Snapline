//! The capture overlay without any window: selection, drawing, text, toolbars and rendering, in screen pixels.
//! The app's windows only feed it input and show the pixmaps it draws. Twin of the macOS `OverlayView`.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)] // pixel maths on clamped values

pub mod annotation;
pub mod color;
pub mod render;
pub mod text;

pub use snapline_core::selection_geometry::{Handle, Point, Rect, Size};
pub use tiny_skia;
