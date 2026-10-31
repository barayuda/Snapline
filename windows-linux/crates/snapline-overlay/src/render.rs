//! Drawing with tiny-skia. The same code draws the overlay on screen and the exported image.

use crate::annotation::{Annotation, Shape};
use crate::color::{BLACK, CYAN, INK, Rgb, VIOLET, VIOLET_LIGHT, WHITE};
use crate::icons;
use crate::session::Session;
use crate::text::TextFont;
use crate::toolbar::{Bar, Button};
use snapline_core::selection_geometry::{self as geo, Handle, Point, Rect};
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

const HINT: &str = "Drag to select an area   ·   Ctrl+A whole screen   ·   Esc to cancel";

fn rounded(frame: Rect, radius: f32) -> Option<tiny_skia::Path> {
    let (left, top) = (frame.x as f32, frame.y as f32);
    let (right, bottom) = (left + frame.width as f32, top + frame.height as f32);
    let k = radius
        .min(frame.width as f32 / 2.0)
        .min(frame.height as f32 / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(left + k, top);
    pb.line_to(right - k, top);
    pb.quad_to(right, top, right, top + k);
    pb.line_to(right, bottom - k);
    pb.quad_to(right, bottom, right - k, bottom);
    pb.line_to(left + k, bottom);
    pb.quad_to(left, bottom, left, bottom - k);
    pb.line_to(left, top + k);
    pb.quad_to(left, top, left + k, top);
    pb.close();
    pb.finish()
}

pub(crate) fn fill_rounded(pixmap: &mut Pixmap, r: Rect, radius: f32, c: Rgb, alpha: u8) {
    if let Some(path) = rounded(r, radius) {
        pixmap.fill_path(
            &path,
            &paint(c, alpha),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn stroke_rect(pixmap: &mut Pixmap, r: Rect, c: Rgb, alpha: u8, width: f32) {
    if let Some(path) = sk_rect(r).map(PathBuilder::from_rect) {
        pixmap.stroke_path(
            &path,
            &paint(c, alpha),
            &stroke(width, LineCap::Butt),
            Transform::identity(),
            None,
        );
    }
}

fn grow(r: Rect, by: f64) -> Rect {
    Rect::new(r.x - by, r.y - by, r.width + 2.0 * by, r.height + 2.0 * by)
}

/// A pill-shaped label. `at` is its top-left corner, or its centre when `centered`.
fn badge(
    pixmap: &mut Pixmap,
    font: &TextFont,
    text: &str,
    at: Point,
    centered: bool,
    fill: Rgb,
    scale: f64,
) {
    let size = (12.0 * scale) as f32;
    let m = font.measure(text, size);
    let mut r = Rect::new(at.x, at.y, m.width + 16.0 * scale, m.height + 6.0 * scale);
    if centered {
        r.x -= r.width / 2.0;
        r.y -= r.height / 2.0;
    }
    fill_rounded(pixmap, r, (r.height / 2.0) as f32, fill, 225);
    font.draw(
        pixmap,
        text,
        Point {
            x: r.x + 8.0 * scale,
            y: r.y + 3.0 * scale,
        },
        size,
        WHITE,
    );
}

fn draw_bars(out: &mut Pixmap, session: &Session, i: usize, bars: &[Bar]) {
    let o = &session.overlays[i];
    let scale = o.ui_scale;
    for bar in bars {
        fill_rounded(out, bar.frame, (8.0 * scale) as f32, INK, 235);
        for (button, r) in &bar.buttons {
            if o.hover == Some(*button) {
                fill_rounded(out, *r, (5.0 * scale) as f32, WHITE, 40);
            }
            if let Button::Engine(engine) = button {
                let size = (13.0 * scale) as f32;
                let h = session.font.measure(engine.title(), size).height;
                let at = Point {
                    x: r.x + 8.0 * scale,
                    y: r.y + (r.height - h) / 2.0,
                };
                session.font.draw(out, engine.title(), at, size, WHITE);
                continue;
            }
            let active = matches!(button, Button::Tool(t) if session.tool == Some(*t));
            icons::draw(
                out,
                *button,
                *r,
                if active { VIOLET_LIGHT } else { WHITE },
                session.color,
            );
        }
    }
    let Some(hover) = o.hover else { return };
    let tip = hover.tip();
    let Some((bar, button)) = bars.iter().find_map(|b| {
        b.buttons
            .iter()
            .find(|(b, _)| *b == hover)
            .map(|(_, r)| (b, *r))
    }) else {
        return;
    };
    if tip.is_empty() {
        return;
    }
    let size = session.font.measure(tip, (12.0 * scale) as f32);
    let (tip_w, tip_h) = (size.width + 16.0 * scale, size.height + 6.0 * scale);
    let gap = 4.0 * scale;
    let bounds = o.bounds();
    // Horizontal bar: under the button (above when there is no room). Vertical bar: beside the button.
    let at = if bar.frame.width > bar.frame.height {
        let below = bar.frame.y + bar.frame.height + gap;
        let y = if below + tip_h <= bounds.height {
            below
        } else {
            bar.frame.y - tip_h - gap
        };
        Point {
            x: button.x.min(bounds.width - tip_w),
            y,
        }
    } else {
        let right = bar.frame.x + bar.frame.width + gap;
        let x = if right + tip_w <= bounds.width {
            right
        } else {
            bar.frame.x - tip_w - gap
        };
        Point {
            x,
            y: button.y + button.height / 2.0 - tip_h / 2.0,
        }
    };
    let at = Point {
        x: at.x.max(0.0),
        y: at.y.max(0.0),
    };
    badge(out, &session.font, tip, at, false, INK, scale);
}

/// Draws what monitor `i` shows during a capture into `out` (same size as that monitor's screen).
pub fn render_screen(out: &mut Pixmap, session: &Session, i: usize) {
    let o = &session.overlays[i];
    let scale = o.ui_scale;
    out.data_mut().copy_from_slice(o.screen.data());
    let origin = Point::default();
    for a in o.annotations.iter().chain(o.current.as_ref()) {
        draw_annotation(out, a, &session.font, origin);
    }
    if let Some(e) = &o.editing {
        session
            .font
            .draw(out, &e.text, e.origin, e.size, session.color);
        let w = session.font.measure(&e.text, e.size).width;
        let caret = Rect::new(
            e.origin.x + w + scale,
            e.origin.y,
            2.0 * scale,
            f64::from(e.size) * 1.2,
        );
        fill_rect(out, caret, session.color, 255);
    }

    let bounds = o.bounds();
    let mut dim = PathBuilder::new();
    if let Some(r) = sk_rect(bounds) {
        dim.push_rect(r);
    }
    if let Some(r) = o.selection.and_then(sk_rect) {
        dim.push_rect(r);
    }
    if let Some(path) = dim.finish() {
        out.fill_path(
            &path,
            &paint(BLACK, 115),
            FillRule::EvenOdd,
            Transform::identity(),
            None,
        );
    }

    let Some(sel) = o.selection else {
        if i == session.focused {
            let centre = Point {
                x: bounds.width / 2.0,
                y: bounds.height / 2.0,
            };
            badge(out, &session.font, HINT, centre, true, INK, scale);
        }
        return;
    };
    stroke_rect(
        out,
        grow(sel, 2.0 * scale),
        VIOLET,
        140,
        (3.0 * scale) as f32,
    );
    stroke_rect(
        out,
        grow(sel, 0.75 * scale),
        CYAN,
        255,
        (1.5 * scale) as f32,
    );
    if session.tool.is_none() && sel.width > 2.0 * scale {
        for h in Handle::ALL {
            let c = geo::handle_point(h, sel);
            if let Some(path) =
                PathBuilder::from_circle(c.x as f32, c.y as f32, (4.5 * scale) as f32)
            {
                let id = Transform::identity();
                out.fill_path(&path, &paint(WHITE, 255), FillRule::Winding, id, None);
                out.stroke_path(
                    &path,
                    &paint(VIOLET, 255),
                    &stroke((2.0 * scale) as f32, LineCap::Round),
                    id,
                    None,
                );
            }
        }
    }
    let label = format!(
        "{} × {}",
        sel.width.round() as i64,
        sel.height.round() as i64
    );
    let at = Point {
        x: sel.x,
        y: (sel.y - 26.0 * scale).max(2.0 * scale),
    };
    badge(out, &session.font, &label, at, false, VIOLET, scale);
    draw_bars(out, session, i, &session.bars(i));
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

    use crate::input::Mods;
    use crate::session::Session;

    fn screen_session(n: usize) -> Session {
        Session::new(
            (0..n).map(|_| (white(400, 300), 1.0)).collect(),
            TextFont::bundled(),
        )
    }

    fn select(s: &mut Session, r: Rect) {
        let (a, b) = (pt(r.x, r.y), pt(r.x + r.width, r.y + r.height));
        s.pointer_down(0, a, Mods::default());
        s.pointer_move(0, b);
        s.pointer_up(0, b);
    }

    fn rendered(s: &Session, i: usize) -> Pixmap {
        let mut out = Pixmap::new(400, 300).unwrap();
        render_screen(&mut out, s, i);
        out
    }

    #[test]
    fn outside_the_selection_is_dimmed_inside_is_not() {
        let mut s = screen_session(1);
        select(&mut s, Rect::new(100.0, 100.0, 150.0, 80.0));
        let out = rendered(&s, 0);
        let (r, _, _) = rgb(&out, 30, 250);
        assert!(
            (130..=150).contains(&r),
            "outside should be dimmed to about 140, was {r}"
        );
        assert_eq!(rgb(&out, 175, 150), (255, 255, 255));
    }

    #[test]
    fn hint_shows_on_the_focused_screen_only() {
        let s = screen_session(2);
        let differs = |out: &Pixmap| (150..250).any(|x| rgb(out, x, 150) != rgb(out, 5, 5));
        assert!(differs(&rendered(&s, 0)));
        assert!(!differs(&rendered(&s, 1)));
    }

    #[test]
    fn handles_show_only_without_a_tool() {
        let mut s = screen_session(1);
        select(&mut s, Rect::new(100.0, 100.0, 150.0, 80.0));
        assert_eq!(
            rgb(&rendered(&s, 0), 250, 140),
            (255, 255, 255),
            "right handle"
        );
        s.tool = Some(crate::annotation::Tool::Pencil);
        assert_ne!(rgb(&rendered(&s, 0), 250, 140), (255, 255, 255));
    }

    #[test]
    fn toolbars_are_drawn_next_to_the_selection() {
        let mut s = screen_session(1);
        select(&mut s, Rect::new(100.0, 100.0, 150.0, 80.0));
        let main = s.bars(0)[0].frame;
        // Top edge, middle: inside the bar's padding, clear of the rounded corners and the icons.
        let (r, g, b) = rgb(
            &rendered(&s, 0),
            (main.x + main.width / 2.0) as u32,
            (main.y + 2.0) as u32,
        );
        assert!(
            r < 60 && g < 60 && b < 60,
            "bar background should be dark ink"
        );
    }

    #[test]
    fn rendering_at_the_edges_and_at_scale_two_does_not_panic() {
        let mut s = Session::new(vec![(white(400, 300), 2.0)], TextFont::bundled());
        s.select_whole_screen();
        s.palette_open = true;
        let mut out = Pixmap::new(400, 300).unwrap();
        render_screen(&mut out, &s, 0);
    }

    #[test]
    fn tooltip_sits_beside_the_hovered_tool() {
        let mut s = screen_session(1);
        select(&mut s, Rect::new(60.0, 40.0, 150.0, 80.0));
        let bars = s.bars(0);
        let draw = bars[1].frame;
        let line = bars[1].buttons[1].1;
        s.pointer_move(0, pt(line.x + 5.0, line.y + 5.0));
        let out = rendered(&s, 0);
        let beside = (draw.x + draw.width + 8.0) as u32;
        let (r, g, b) = rgb(&out, beside, (line.y + line.height / 2.0) as u32);
        assert!(
            r < 60 && g < 60 && b < 60,
            "the tooltip should be right of the bar, on the button's row"
        );
    }

    #[test]
    fn search_menu_shows_its_labels() {
        let mut s = screen_session(1);
        select(&mut s, Rect::new(60.0, 40.0, 150.0, 80.0));
        s.search_open = true;
        let menu = s.bars(0)[2].buttons[0].1;
        let out = rendered(&s, 0);
        let row = (menu.y + menu.height / 2.0) as u32;
        let light = (menu.x as u32..(menu.x + menu.width) as u32)
            .filter(|&x| rgb(&out, x, row).0 > 200)
            .count();
        assert!(light > 5, "the label text should be drawn");
    }
}
