//! From recognised text and faces to the boxes auto-hide covers, and to the text Copy Text copies.

use snapline_core::selection_geometry::Rect;
use snapline_core::sensitive_text::SensitiveCategory;

/// An area auto-hide found. `hidden` is false after the user clicked it to show it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HideBox {
    pub rect: Rect,
    pub category: SensitiveCategory,
    pub hidden: bool,
}
