//! Twin of `macOS/Sources/SnaplineCore/SelectionGeometry.swift`: geometry for the selection overlay.
//! View rects use a top-left origin (y grows downward). `Rect` itself has no orientation: reading order uses
//! it for normalized OCR boxes with a bottom-left origin.

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    #[must_use]
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub fn min_x(self) -> f64 {
        self.x
    }

    #[must_use]
    pub fn max_x(self) -> f64 {
        self.x + self.width
    }

    #[must_use]
    pub fn mid_x(self) -> f64 {
        self.x + self.width / 2.0
    }

    #[must_use]
    pub fn min_y(self) -> f64 {
        self.y
    }

    #[must_use]
    pub fn max_y(self) -> f64 {
        self.y + self.height
    }

    #[must_use]
    pub fn mid_y(self) -> f64 {
        self.y + self.height / 2.0
    }

    /// The smallest rect holding both.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        rect(
            Point {
                x: self.min_x().min(other.min_x()),
                y: self.min_y().min(other.min_y()),
            },
            Point {
                x: self.max_x().max(other.max_x()),
                y: self.max_y().max(other.max_y()),
            },
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl Handle {
    pub const ALL: [Self; 8] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Right,
        Self::BottomRight,
        Self::Bottom,
        Self::BottomLeft,
        Self::Left,
    ];
}

/// The rectangle spanned by two corner points, in any order.
#[must_use]
pub fn rect(a: Point, b: Point) -> Rect {
    Rect::new(
        a.x.min(b.x),
        a.y.min(b.y),
        (a.x - b.x).abs(),
        (a.y - b.y).abs(),
    )
}

/// OCR bounding box (normalized, bottom-left origin) → view rect in points (top-left origin).
#[must_use]
pub fn view_rect(normalized: Rect, view: Size) -> Rect {
    Rect::new(
        normalized.min_x() * view.width,
        (1.0 - normalized.max_y()) * view.height,
        normalized.width * view.width,
        normalized.height * view.height,
    )
}

/// Part of a word's box that holds `characters`, assuming equal character widths,
/// padded by half a character on each side because real glyph widths vary.
#[must_use]
#[allow(clippy::cast_precision_loss)] // character counts are far below 2^52
pub fn slice(bbox: Rect, characters: std::ops::Range<usize>, count: usize) -> Rect {
    if count == 0 {
        return bbox;
    }
    let unit = bbox.width / count as f64;
    let min_x = bbox
        .min_x()
        .max(bbox.min_x() + characters.start as f64 * unit - unit / 2.0);
    let max_x = bbox
        .max_x()
        .min(bbox.min_x() + characters.end as f64 * unit + unit / 2.0);
    Rect::new(min_x, bbox.min_y(), max_x - min_x, bbox.height)
}

/// `r` moved by (`dx`, `dy`), kept fully inside `bounds`.
#[must_use]
pub fn move_within(r: Rect, dx: f64, dy: f64, bounds: Rect) -> Rect {
    // max then min (not clamp): a rect wider than `bounds` sticks to the far edge, as on macOS.
    let x = (r.x + dx).max(bounds.min_x()).min(bounds.max_x() - r.width);
    let y = (r.y + dy)
        .max(bounds.min_y())
        .min(bounds.max_y() - r.height);
    Rect::new(x, y, r.width, r.height)
}

/// Centre point of a resize handle on `r`.
#[must_use]
pub fn handle_point(h: Handle, r: Rect) -> Point {
    let (x, y) = match h {
        Handle::TopLeft => (r.min_x(), r.min_y()),
        Handle::Top => (r.mid_x(), r.min_y()),
        Handle::TopRight => (r.max_x(), r.min_y()),
        Handle::Right => (r.max_x(), r.mid_y()),
        Handle::BottomRight => (r.max_x(), r.max_y()),
        Handle::Bottom => (r.mid_x(), r.max_y()),
        Handle::BottomLeft => (r.min_x(), r.max_y()),
        Handle::Left => (r.min_x(), r.mid_y()),
    };
    Point { x, y }
}

/// The resize handle within `tolerance` points of `p`, if any.
#[must_use]
pub fn handle_at(p: Point, r: Rect, tolerance: f64) -> Option<Handle> {
    Handle::ALL.into_iter().find(|&h| {
        let c = handle_point(h, r);
        (c.x - p.x).abs() <= tolerance && (c.y - p.y).abs() <= tolerance
    })
}

/// Moves the edges a handle controls to `to`; the opposite edges stay put.
#[must_use]
pub fn resize(r: Rect, h: Handle, to: Point) -> Rect {
    let (mut x1, mut y1, mut x2, mut y2) = (r.min_x(), r.min_y(), r.max_x(), r.max_y());
    match h {
        Handle::TopLeft => (x1, y1) = (to.x, to.y),
        Handle::Top => y1 = to.y,
        Handle::TopRight => (x2, y1) = (to.x, to.y),
        Handle::Right => x2 = to.x,
        Handle::BottomRight => (x2, y2) = (to.x, to.y),
        Handle::Bottom => y2 = to.y,
        Handle::BottomLeft => (x1, y2) = (to.x, to.y),
        Handle::Left => x1 = to.x,
    }
    rect(Point { x: x1, y: y1 }, Point { x: x2, y: y2 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_close(a: Rect, b: Rect) -> bool {
        let e = 0.001;
        (a.x - b.x).abs() < e
            && (a.y - b.y).abs() < e
            && (a.width - b.width).abs() < e
            && (a.height - b.height).abs() < e
    }

    #[test]
    fn rect_from_two_points_in_any_direction() {
        assert_eq!(
            rect(Point { x: 50.0, y: 40.0 }, Point { x: 10.0, y: 100.0 }),
            Rect::new(10.0, 40.0, 40.0, 60.0)
        );
    }

    #[test]
    fn ocr_box_to_top_left_view_rect() {
        // OCR: normalized, origin bottom-left. View: points, origin top-left.
        let r = view_rect(
            Rect::new(0.1, 0.2, 0.5, 0.25),
            Size {
                width: 1000.0,
                height: 800.0,
            },
        );
        assert!(
            is_close(r, Rect::new(100.0, 440.0, 500.0, 200.0)),
            "got {r:?}"
        );
    }

    #[test]
    fn horizontal_slice_of_word_box_with_half_character_padding() {
        // "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE": the key is characters 18..38 of 38.
        let word = Rect::new(0.0, 10.0, 380.0, 20.0);
        let r = slice(word, 18..38, 38);
        assert!(
            is_close(r, Rect::new(175.0, 10.0, 205.0, 20.0)),
            "got {r:?}"
        );
        assert_eq!(slice(word, 0..38, 38), word);
    }

    #[test]
    fn move_is_clamped_inside_bounds() {
        let bounds = Rect::new(0.0, 0.0, 500.0, 500.0);
        assert_eq!(
            move_within(Rect::new(450.0, 10.0, 40.0, 40.0), 30.0, -30.0, bounds),
            Rect::new(460.0, 0.0, 40.0, 40.0)
        );
    }

    #[test]
    fn handle_hit_test() {
        let sel = Rect::new(100.0, 100.0, 200.0, 100.0);
        assert_eq!(
            handle_at(Point { x: 101.0, y: 99.0 }, sel, 6.0),
            Some(Handle::TopLeft)
        );
        assert_eq!(
            handle_at(Point { x: 300.0, y: 150.0 }, sel, 6.0),
            Some(Handle::Right)
        );
        assert_eq!(handle_at(Point { x: 200.0, y: 150.0 }, sel, 6.0), None);
    }

    #[test]
    fn resize_moves_only_the_dragged_edges_and_flips_past_the_opposite_edge() {
        let sel = Rect::new(100.0, 100.0, 200.0, 100.0);
        assert_eq!(
            resize(sel, Handle::BottomRight, Point { x: 350.0, y: 260.0 }),
            Rect::new(100.0, 100.0, 250.0, 160.0)
        );
        // Dragging the left handle past the right edge flips instead of producing a negative width.
        assert_eq!(
            resize(sel, Handle::Left, Point { x: 400.0, y: 0.0 }),
            Rect::new(300.0, 100.0, 100.0, 100.0)
        );
    }
}
