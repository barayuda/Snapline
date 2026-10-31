//! What a drawing is, and how a mouse drag shapes it. Twin of macOS `Annotations.swift`.

use crate::color::Rgb;
use snapline_core::selection_geometry::{self as geo, Point, Rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Pencil,
    Line,
    Arrow,
    Rectangle,
    Ellipse,
    Marker,
    Text,
    Hide,
}

impl Tool {
    pub const ALL: [Self; 8] = [
        Self::Pencil,
        Self::Line,
        Self::Arrow,
        Self::Rectangle,
        Self::Ellipse,
        Self::Marker,
        Self::Text,
        Self::Hide,
    ];

    /// The one-letter shortcut, as on macOS.
    #[must_use]
    pub fn for_key(c: char) -> Option<Self> {
        Some(match c.to_ascii_lowercase() {
            'p' => Self::Pencil,
            'l' => Self::Line,
            'a' => Self::Arrow,
            'r' => Self::Rectangle,
            'e' => Self::Ellipse,
            'm' => Self::Marker,
            't' => Self::Text,
            'h' => Self::Hide,
            _ => return None,
        })
    }

    /// Tooltip text with the shortcut.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Pencil => "Pencil (P)",
            Self::Line => "Line (L)",
            Self::Arrow => "Arrow (A)",
            Self::Rectangle => "Rectangle (R)",
            Self::Ellipse => "Ellipse (E)",
            Self::Marker => "Marker (M)",
            Self::Text => "Text (T)",
            Self::Hide => "Hide area (H)",
        }
    }
}

/// One drawn shape, in screen pixels (top-left origin).
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// Pencil and marker.
    Path(Vec<Point>),
    Line(Point, Point),
    Arrow(Point, Point),
    Rectangle(Rect),
    Ellipse(Rect),
    /// A solid black box.
    Hide(Rect),
    /// `origin` is the top-left corner of the text; `size` is the font size in pixels.
    Text {
        origin: Point,
        text: String,
        size: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub shape: Shape,
    pub color: Rgb,
    /// Stroke width in pixels.
    pub width: f32,
    pub marker: bool,
}

impl Annotation {
    /// Starts a shape at the mouse-down point. The text tool makes no shape here (it opens a text box).
    #[must_use]
    pub fn begin(tool: Tool, p: Point, color: Rgb, width: f32) -> Option<Self> {
        let empty = Rect::new(p.x, p.y, 0.0, 0.0);
        let shape = match tool {
            Tool::Pencil | Tool::Marker => Shape::Path(vec![p]),
            Tool::Line => Shape::Line(p, p),
            Tool::Arrow => Shape::Arrow(p, p),
            Tool::Rectangle => Shape::Rectangle(empty),
            Tool::Ellipse => Shape::Ellipse(empty),
            Tool::Hide => Shape::Hide(empty),
            Tool::Text => return None,
        };
        Some(Self {
            shape,
            color,
            width,
            marker: tool == Tool::Marker,
        })
    }

    /// Updates the shape while the mouse is dragged from `start` to `p`.
    pub fn extend(&mut self, start: Point, p: Point) {
        match &mut self.shape {
            Shape::Path(points) => points.push(p),
            Shape::Line(_, end) | Shape::Arrow(_, end) => *end = p,
            Shape::Rectangle(r) | Shape::Ellipse(r) | Shape::Hide(r) => *r = geo::rect(start, p),
            Shape::Text { .. } => {}
        }
    }

    #[must_use]
    pub fn is_path(&self) -> bool {
        matches!(self.shape, Shape::Path(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::PALETTE;

    const RED: Rgb = PALETTE[0];

    fn pt(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    #[test]
    fn tool_keys_match_macos() {
        assert_eq!(Tool::for_key('P'), Some(Tool::Pencil));
        assert_eq!(Tool::for_key('h'), Some(Tool::Hide));
        assert_eq!(Tool::for_key('x'), None);
        // Every title ends with its own shortcut letter, e.g. "Pencil (P)".
        for tool in Tool::ALL {
            let letter = tool.title().chars().rev().nth(1).unwrap();
            assert_eq!(Tool::for_key(letter), Some(tool), "{}", tool.title());
        }
    }

    #[test]
    fn rectangle_follows_the_drag_in_any_direction() {
        let mut a = Annotation::begin(Tool::Rectangle, pt(50.0, 40.0), RED, 3.0).unwrap();
        a.extend(pt(50.0, 40.0), pt(10.0, 100.0));
        assert_eq!(a.shape, Shape::Rectangle(Rect::new(10.0, 40.0, 40.0, 60.0)));
    }

    #[test]
    fn arrow_keeps_its_start() {
        let mut a = Annotation::begin(Tool::Arrow, pt(5.0, 5.0), RED, 3.0).unwrap();
        a.extend(pt(5.0, 5.0), pt(30.0, 9.0));
        assert_eq!(a.shape, Shape::Arrow(pt(5.0, 5.0), pt(30.0, 9.0)));
    }

    #[test]
    fn pencil_collects_points_and_marker_is_flagged() {
        let mut a = Annotation::begin(Tool::Pencil, pt(0.0, 0.0), RED, 3.0).unwrap();
        a.extend(pt(0.0, 0.0), pt(1.0, 1.0));
        a.extend(pt(0.0, 0.0), pt(2.0, 2.0));
        assert_eq!(
            a.shape,
            Shape::Path(vec![pt(0.0, 0.0), pt(1.0, 1.0), pt(2.0, 2.0)])
        );
        assert!(a.is_path() && !a.marker);
        assert!(
            Annotation::begin(Tool::Marker, pt(0.0, 0.0), RED, 3.0)
                .unwrap()
                .marker
        );
    }

    #[test]
    fn text_tool_does_not_begin_a_shape() {
        assert!(Annotation::begin(Tool::Text, pt(0.0, 0.0), RED, 3.0).is_none());
    }
}
