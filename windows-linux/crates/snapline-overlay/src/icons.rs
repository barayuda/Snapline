//! Toolbar icons as tiny-skia paths on a 24-unit grid (simple line icons, made for Snapline).

use crate::annotation::Tool;
use crate::color::{PALETTE, Rgb, WHITE};
use crate::render::{paint, stroke};
use crate::toolbar::Button;
use snapline_core::selection_geometry::Rect;
use tiny_skia::{FillRule, LineCap, PathBuilder, Pixmap, Transform};

/// Draws `button`'s icon centred in `frame`. `ink` is the line colour; `current` is the drawing colour.
pub fn draw(pixmap: &mut Pixmap, button: Button, frame: Rect, ink: Rgb, current: Rgb) {
    let side = frame.width.min(frame.height) * 0.64;
    let k = (side / 24.0) as f32;
    let t = Transform::from_row(
        k,
        0.0,
        0.0,
        k,
        (frame.x + (frame.width - side) / 2.0) as f32,
        (frame.y + (frame.height - side) / 2.0) as f32,
    );
    let line = |pixmap: &mut Pixmap, pb: PathBuilder, width: f32, alpha: u8| {
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(
                &path,
                &paint(ink, alpha),
                &stroke(width, LineCap::Round),
                t,
                None,
            );
        }
    };
    let dot = |pixmap: &mut Pixmap, color: Rgb, r: f32| {
        if let Some(path) = PathBuilder::from_circle(12.0, 12.0, r) {
            pixmap.fill_path(&path, &paint(color, 255), FillRule::Winding, t, None);
            pixmap.stroke_path(
                &path,
                &paint(WHITE, 255),
                &stroke(1.5, LineCap::Round),
                t,
                None,
            );
        }
    };
    match button {
        Button::Swatch(n) => return dot(pixmap, PALETTE[n], 9.0),
        Button::Color => return dot(pixmap, current, 8.0),
        Button::Tool(Tool::Hide) => {
            if let Some(r) = tiny_skia::Rect::from_xywh(4.0, 6.0, 16.0, 12.0) {
                pixmap.fill_rect(r, &paint(ink, 255), t, None);
            }
            return;
        }
        Button::Tool(Tool::Marker) => {
            let mut pb = PathBuilder::new();
            pb.move_to(6.0, 16.0);
            pb.line_to(18.0, 6.0);
            line(pixmap, pb, 6.0, 150);
            let mut base = PathBuilder::new();
            base.move_to(4.0, 21.0);
            base.line_to(20.0, 21.0);
            return line(pixmap, base, 2.0, 255);
        }
        _ => {}
    }
    line(pixmap, outline(button), 2.0, 255);
}

/// The stroked line icons. Buttons drawn another way (filled or coloured) give an empty path.
fn outline(button: Button) -> PathBuilder {
    let mut pb = PathBuilder::new();
    match button {
        Button::Tool(Tool::Pencil) => {
            pb.move_to(4.0, 20.0);
            pb.line_to(8.0, 19.0);
            pb.line_to(19.0, 8.0);
            pb.line_to(16.0, 5.0);
            pb.line_to(5.0, 16.0);
            pb.close();
        }
        Button::Tool(Tool::Line) => {
            pb.move_to(5.0, 19.0);
            pb.line_to(19.0, 5.0);
        }
        Button::Tool(Tool::Arrow) => {
            pb.move_to(5.0, 19.0);
            pb.line_to(19.0, 5.0);
            pb.move_to(10.0, 5.0);
            pb.line_to(19.0, 5.0);
            pb.line_to(19.0, 14.0);
        }
        Button::Tool(Tool::Rectangle) => {
            if let Some(r) = tiny_skia::Rect::from_xywh(4.0, 6.0, 16.0, 12.0) {
                pb.push_rect(r);
            }
        }
        Button::Tool(Tool::Ellipse) => {
            if let Some(r) = tiny_skia::Rect::from_xywh(4.0, 5.0, 16.0, 14.0) {
                pb.push_oval(r);
            }
        }
        Button::Tool(Tool::Text) => {
            pb.move_to(6.0, 5.0);
            pb.line_to(18.0, 5.0);
            pb.move_to(12.0, 5.0);
            pb.line_to(12.0, 19.0);
        }
        Button::Undo => {
            pb.move_to(9.0, 6.0);
            pb.line_to(4.0, 11.0);
            pb.line_to(9.0, 16.0);
            pb.move_to(4.0, 11.0);
            pb.line_to(14.0, 11.0);
            pb.quad_to(20.0, 11.0, 20.0, 17.0);
            pb.line_to(20.0, 19.0);
        }
        Button::Copy => {
            if let Some(r) = tiny_skia::Rect::from_xywh(8.0, 8.0, 12.0, 12.0) {
                pb.push_rect(r);
            }
            pb.move_to(4.0, 16.0);
            pb.line_to(4.0, 4.0);
            pb.line_to(16.0, 4.0);
        }
        Button::Save => {
            pb.move_to(12.0, 4.0);
            pb.line_to(12.0, 15.0);
            pb.move_to(7.0, 10.0);
            pb.line_to(12.0, 15.0);
            pb.line_to(17.0, 10.0);
            pb.move_to(5.0, 19.0);
            pb.line_to(19.0, 19.0);
        }
        Button::Close => {
            pb.move_to(6.0, 6.0);
            pb.line_to(18.0, 18.0);
            pb.move_to(18.0, 6.0);
            pb.line_to(6.0, 18.0);
        }
        Button::Upload => {
            pb.move_to(12.0, 15.0);
            pb.line_to(12.0, 4.0);
            pb.move_to(7.0, 9.0);
            pb.line_to(12.0, 4.0);
            pb.line_to(17.0, 9.0);
            pb.move_to(5.0, 15.0);
            pb.line_to(5.0, 19.0);
            pb.line_to(19.0, 19.0);
            pb.line_to(19.0, 15.0);
        }
        Button::Search => {
            pb.push_circle(10.0, 10.0, 6.0);
            pb.move_to(14.5, 14.5);
            pb.line_to(20.0, 20.0);
        }
        // Drawn another way: filled, coloured, or a text label (search menu entries, by render).
        Button::Tool(Tool::Hide | Tool::Marker)
        | Button::Color
        | Button::Swatch(_)
        | Button::Engine(_) => {}
    }
    pb
}
