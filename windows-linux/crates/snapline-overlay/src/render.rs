//! Drawing with tiny-skia. The same code draws the overlay on screen and the exported image.

use crate::annotation::{Annotation, Shape};
use crate::color::{BLACK, Rgb};
use crate::text::TextFont;
use snapline_core::selection_geometry::{Point, Rect};
use std::f64::consts::PI;
use tiny_skia::{
    FillRule, IntRect, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform,
};

pub(crate) fn paint(c: Rgb, alpha: u8) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c.0, c.1, c.2, alpha);
    p.anti_alias = true;
    p
}

pub(crate) fn sk_rect(r: Rect) -> Option<tiny_skia::Rect> {
    tiny_skia::Rect::from_xywh(r.x as f32, r.y as f32, r.width as f32, r.height as f32)
}

pub(crate) fn stroke(width: f32, cap: LineCap) -> Stroke {
    Stroke {
        width,
        line_cap: cap,
        line_join: LineJoin::Round,
        ..Stroke::default()
    }
}

pub(crate) fn fill_rect(pixmap: &mut Pixmap, r: Rect, c: Rgb, alpha: u8) {
    if let Some(r) = sk_rect(r) {
        pixmap.fill_rect(r, &paint(c, alpha), Transform::identity(), None);
    }
}

fn line(pixmap: &mut Pixmap, a: Point, b: Point, c: Rgb, width: f32, t: Transform) {
    let mut pb = PathBuilder::new();
    pb.move_to(a.x as f32, a.y as f32);
    pb.line_to(b.x as f32, b.y as f32);
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(
            &path,
            &paint(c, 255),
            &stroke(width, LineCap::Round),
            t,
            None,
        );
    }
}

/// Pencil and marker strokes; a click without a drag leaves a dot.
fn draw_path(pixmap: &mut Pixmap, points: &[Point], a: &Annotation, t: Transform) {
    let (width, cap, alpha) = if a.marker {
        (a.width * 5.0, LineCap::Square, 102)
    } else {
        (a.width, LineCap::Round, 255)
    };
    if let [only] = points {
        // A click without a drag: a dot (a zero-length stroke draws nothing).
        let half = width / 2.0;
        let (x, y) = (only.x as f32, only.y as f32);
        let path = if a.marker {
            tiny_skia::Rect::from_xywh(x - half, y - half, width, width).map(PathBuilder::from_rect)
        } else {
            PathBuilder::from_circle(x, y, half)
        };
        if let Some(path) = path {
            pixmap.fill_path(&path, &paint(a.color, alpha), FillRule::Winding, t, None);
        }
        return;
    }
    let mut pb = PathBuilder::new();
    for (k, p) in points.iter().enumerate() {
        if k == 0 {
            pb.move_to(p.x as f32, p.y as f32);
        } else {
            pb.line_to(p.x as f32, p.y as f32);
        }
    }
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, &paint(a.color, alpha), &stroke(width, cap), t, None);
    }
}

fn draw_arrow(pixmap: &mut Pixmap, from: Point, to: Point, a: &Annotation, t: Transform) {
    let angle = (to.y - from.y).atan2(to.x - from.x);
    let head = f64::from(a.width * 4.0).max(12.0);
    let base = Point {
        x: to.x - angle.cos() * head * 0.8,
        y: to.y - angle.sin() * head * 0.8,
    };
    line(pixmap, from, base, a.color, a.width, t);
    let wing = |turn: f64| {
        (
            to.x - head * (angle + turn).cos(),
            to.y - head * (angle + turn).sin(),
        )
    };
    let (l, r) = (wing(-PI / 7.0), wing(PI / 7.0));
    let mut pb = PathBuilder::new();
    pb.move_to(to.x as f32, to.y as f32);
    pb.line_to(l.0 as f32, l.1 as f32);
    pb.line_to(r.0 as f32, r.1 as f32);
    pb.close();
    if let Some(path) = pb.finish() {
        pixmap.fill_path(&path, &paint(a.color, 255), FillRule::Winding, t, None);
    }
}

/// Draws one annotation. `shift` is subtracted from every point (the export crops to the selection).
pub fn draw_annotation(pixmap: &mut Pixmap, a: &Annotation, font: &TextFont, shift: Point) {
    let t = Transform::from_translate(-shift.x as f32, -shift.y as f32);
    match &a.shape {
        Shape::Path(points) => draw_path(pixmap, points, a, t),
        Shape::Line(p, q) => line(pixmap, *p, *q, a.color, a.width, t),
        Shape::Arrow(from, to) => draw_arrow(pixmap, *from, *to, a, t),
        Shape::Rectangle(r) => {
            if let Some(path) = sk_rect(*r).map(PathBuilder::from_rect) {
                pixmap.stroke_path(
                    &path,
                    &paint(a.color, 255),
                    &stroke(a.width, LineCap::Round),
                    t,
                    None,
                );
            }
        }
        Shape::Ellipse(r) => {
            if let Some(path) = sk_rect(*r).and_then(PathBuilder::from_oval) {
                pixmap.stroke_path(
                    &path,
                    &paint(a.color, 255),
                    &stroke(a.width, LineCap::Round),
                    t,
                    None,
                );
            }
        }
        Shape::Hide(r) => {
            fill_rect(
                pixmap,
                Rect::new(r.x - shift.x, r.y - shift.y, r.width, r.height),
                BLACK,
                255,
            );
        }
        Shape::Text { origin, text, size } => {
            font.draw(
                pixmap,
                text,
                Point {
                    x: origin.x - shift.x,
                    y: origin.y - shift.y,
                },
                *size,
                a.color,
            );
        }
    }
}

/// The selection flattened into one image: the frozen screen, then every annotation, cut to the screen.
/// `None` when nothing of the selection is on the screen.
#[must_use]
pub fn render_selection(
    screen: &Pixmap,
    selection: Rect,
    annotations: &[Annotation],
    font: &TextFont,
) -> Option<Pixmap> {
    let x0 = selection.x.round().max(0.0);
    let y0 = selection.y.round().max(0.0);
    let x1 = (selection.x + selection.width)
        .round()
        .min(f64::from(screen.width()));
    let y1 = (selection.y + selection.height)
        .round()
        .min(f64::from(screen.height()));
    if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
        return None;
    }
    let crop = IntRect::from_xywh(x0 as i32, y0 as i32, (x1 - x0) as u32, (y1 - y0) as u32)?;
    let mut out = screen.clone_rect(crop)?;
    let shift = Point { x: x0, y: y0 };
    for a in annotations {
        draw_annotation(&mut out, a, font, shift);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotation::Tool;
    use crate::color::PALETTE;
    use tiny_skia::Color;

    const RED: Rgb = PALETTE[0];

    pub(crate) fn white(w: u32, h: u32) -> Pixmap {
        let mut p = Pixmap::new(w, h).unwrap();
        p.fill(Color::WHITE);
        p
    }

    pub(crate) fn rgb(p: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let c = p.pixel(x, y).unwrap().demultiply();
        (c.red(), c.green(), c.blue())
    }

    fn pt(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn drawn(tool: Tool, from: Point, to: Point) -> Annotation {
        let mut a = Annotation::begin(tool, from, RED, 3.0).unwrap();
        a.extend(from, to);
        a
    }

    #[test]
    fn export_is_the_selection_size() {
        let out = render_selection(
            &white(100, 80),
            Rect::new(10.0, 20.0, 30.0, 40.0),
            &[],
            &TextFont::bundled(),
        )
        .unwrap();
        assert_eq!((out.width(), out.height()), (30, 40));
    }

    #[test]
    fn empty_or_offscreen_selection_gives_nothing() {
        let f = TextFont::bundled();
        assert!(
            render_selection(&white(100, 80), Rect::new(10.0, 10.0, 0.0, 5.0), &[], &f).is_none()
        );
        assert!(
            render_selection(
                &white(100, 80),
                Rect::new(200.0, 200.0, 10.0, 10.0),
                &[],
                &f
            )
            .is_none()
        );
    }

    #[test]
    fn selection_hanging_off_the_screen_is_cut_to_the_screen() {
        let out = render_selection(
            &white(100, 80),
            Rect::new(90.0, 70.0, 30.0, 30.0),
            &[],
            &TextFont::bundled(),
        )
        .unwrap();
        assert_eq!((out.width(), out.height()), (10, 10));
    }

    #[test]
    fn hide_box_is_solid_black_in_the_export() {
        let hide = drawn(Tool::Hide, pt(15.0, 25.0), pt(25.0, 35.0));
        let out = render_selection(
            &white(100, 80),
            Rect::new(10.0, 20.0, 30.0, 40.0),
            &[hide],
            &TextFont::bundled(),
        )
        .unwrap();
        assert_eq!(rgb(&out, 10, 10), (0, 0, 0));
        assert_eq!(rgb(&out, 1, 1), (255, 255, 255));
    }

    #[test]
    fn marker_is_see_through() {
        let marker = drawn(Tool::Marker, pt(10.0, 40.0), pt(90.0, 40.0));
        let out = render_selection(
            &white(100, 80),
            Rect::new(0.0, 0.0, 100.0, 80.0),
            &[marker],
            &TextFont::bundled(),
        )
        .unwrap();
        let (r, g, _) = rgb(&out, 50, 40);
        assert_eq!(r, 255);
        // 40 % of the red's green (0x3B) over white: 59 × 0.4 + 255 × 0.6 ≈ 177. Fully opaque would be 59.
        assert!(
            (170..=185).contains(&g),
            "marker should be 40 % opaque, green was {g}"
        );
    }

    #[test]
    fn arrow_head_is_filled() {
        let arrow = drawn(Tool::Arrow, pt(10.0, 40.0), pt(90.0, 40.0));
        let out = render_selection(
            &white(100, 80),
            Rect::new(0.0, 0.0, 100.0, 80.0),
            &[arrow],
            &TextFont::bundled(),
        )
        .unwrap();
        // 8 px back from the tip and 2 px off the shaft: inside the head, outside the 3 px shaft.
        let (r, g, _) = rgb(&out, 82, 42);
        assert!(r > 200 && g < 100);
    }

    #[test]
    fn pencil_click_leaves_a_dot() {
        let dot = Annotation::begin(Tool::Pencil, pt(50.0, 40.0), RED, 6.0).unwrap();
        let out = render_selection(
            &white(100, 80),
            Rect::new(0.0, 0.0, 100.0, 80.0),
            &[dot],
            &TextFont::bundled(),
        )
        .unwrap();
        assert_eq!(rgb(&out, 50, 40), (255, 0x3B, 0x30));
    }

    #[test]
    fn text_moves_with_the_selection() {
        let text = Annotation {
            shape: Shape::Text {
                origin: pt(20.0, 30.0),
                text: "H".into(),
                size: 20.0,
            },
            color: RED,
            width: 0.0,
            marker: false,
        };
        let out = render_selection(
            &white(100, 80),
            Rect::new(10.0, 20.0, 60.0, 50.0),
            &[text],
            &TextFont::bundled(),
        )
        .unwrap();
        let red_at = |x, y| {
            let (r, g, _) = rgb(&out, x, y);
            r > 200 && g < 120
        };
        assert!((10..30).any(|x| (10..35).any(|y| red_at(x, y))));
        assert!(!(40..60).any(|x| (0..50).any(|y| red_at(x, y))));
    }
}
