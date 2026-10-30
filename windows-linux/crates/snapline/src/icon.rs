//! The tray icon: the brand square (violet → cyan) with the white selection corners and spark (BRAND.md).

use snapline_overlay::tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LinearGradient, Paint, PathBuilder, Pixmap, Point,
    SpreadMode, Stroke, Transform,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrayState {
    Normal,
    /// 0…1, drawn as a ring that fills clockwise from the top.
    Progress(f64),
    /// ✓ on a green badge.
    Success,
    /// ! on a red badge.
    Failure,
}

/// Straight (not premultiplied) RGBA, `size`×`size`.
///
/// # Panics
/// If `size` is 0.
#[must_use]
pub fn tray_rgba(size: u32, state: TrayState) -> Vec<u8> {
    let mut pixmap = Pixmap::new(size, size).expect("icon size is not zero");
    let unit = size as f32 / 32.0;
    let grid = Transform::from_scale(unit, unit);

    let mut bg = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    if let Some(shader) = LinearGradient::new(
        Point::from_xy(0.0, 0.0),
        Point::from_xy(32.0, 32.0),
        vec![
            GradientStop::new(0.0, Color::from_rgba8(0x4C, 0x1A, 0xB8, 255)),
            GradientStop::new(1.0, Color::from_rgba8(0x22, 0xD3, 0xEE, 255)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ) {
        bg.shader = shader;
    }
    let mut square = PathBuilder::new();
    let (lo, hi, r) = (1.0, 31.0, 7.0);
    square.move_to(lo + r, lo);
    square.line_to(hi - r, lo);
    square.quad_to(hi, lo, hi, lo + r);
    square.line_to(hi, hi - r);
    square.quad_to(hi, hi, hi - r, hi);
    square.line_to(lo + r, hi);
    square.quad_to(lo, hi, lo, hi - r);
    square.line_to(lo, lo + r);
    square.quad_to(lo, lo, lo + r, lo);
    square.close();
    if let Some(path) = square.finish() {
        pixmap.fill_path(&path, &bg, FillRule::Winding, grid, None);
    }

    let mut white = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    white.set_color_rgba8(255, 255, 255, 255);
    let mut corners = PathBuilder::new();
    for [a, b, c] in [
        [(7.0, 12.0), (7.0, 7.0), (12.0, 7.0)],
        [(20.0, 7.0), (25.0, 7.0), (25.0, 12.0)],
        [(25.0, 20.0), (25.0, 25.0), (20.0, 25.0)],
        [(12.0, 25.0), (7.0, 25.0), (7.0, 20.0)],
    ] {
        corners.move_to(a.0, a.1);
        corners.line_to(b.0, b.1);
        corners.line_to(c.0, c.1);
    }
    if let Some(path) = corners.finish() {
        let stroke = Stroke {
            width: 2.6,
            line_cap: LineCap::Round,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &white, &stroke, grid, None);
    }
    let mut spark = PathBuilder::new();
    spark.move_to(16.0, 10.0);
    spark.quad_to(16.0, 16.0, 22.0, 16.0);
    spark.quad_to(16.0, 16.0, 16.0, 22.0);
    spark.quad_to(16.0, 16.0, 10.0, 16.0);
    spark.quad_to(16.0, 16.0, 16.0, 10.0);
    spark.close();
    if let Some(path) = spark.finish() {
        pixmap.fill_path(&path, &white, FillRule::Winding, grid, None);
    }

    draw_state(&mut pixmap, grid, state);
    pixmap
        .pixels()
        .iter()
        .flat_map(|c| {
            let c = c.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}

/// The upload state on top of the icon: a progress ring, or a ✓ / ! badge.
fn draw_state(pixmap: &mut Pixmap, grid: Transform, state: TrayState) {
    let paint = |r: u8, g: u8, b: u8, a: u8| {
        let mut p = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        p.set_color_rgba8(r, g, b, a);
        p
    };
    let stroke = |width: f32| Stroke {
        width,
        line_cap: LineCap::Round,
        ..Stroke::default()
    };
    match state {
        TrayState::Normal => {}
        TrayState::Progress(done) => {
            // Dim the glyph, then a track and the filled part of the ring.
            if let Some(r) = snapline_overlay::tiny_skia::Rect::from_xywh(1.0, 1.0, 30.0, 30.0) {
                pixmap.fill_rect(r, &paint(0x0C, 0x08, 0x19, 140), grid, None);
            }
            if let Some(ring) = PathBuilder::from_circle(16.0, 16.0, 11.0) {
                pixmap.stroke_path(&ring, &paint(255, 255, 255, 70), &stroke(3.0), grid, None);
            }
            let mut arc = PathBuilder::new();
            let steps = 48_u32;
            let end = (done.clamp(0.0, 1.0) * f64::from(steps)).round() as u32;
            for k in 0..=end {
                let angle = -std::f64::consts::FRAC_PI_2
                    + std::f64::consts::TAU * f64::from(k) / f64::from(steps);
                let (x, y) = (
                    (16.0 + 11.0 * angle.cos()) as f32,
                    (16.0 + 11.0 * angle.sin()) as f32,
                );
                if k == 0 {
                    arc.move_to(x, y);
                } else {
                    arc.line_to(x, y);
                }
            }
            if let Some(path) = arc.finish() {
                pixmap.stroke_path(&path, &paint(255, 255, 255, 255), &stroke(3.0), grid, None);
            }
        }
        TrayState::Success | TrayState::Failure => {
            let (r, g, b) = if state == TrayState::Success {
                (0x34, 0xC7, 0x59)
            } else {
                (0xFF, 0x3B, 0x30)
            };
            if let Some(badge) = PathBuilder::from_circle(24.0, 24.0, 7.5) {
                pixmap.fill_path(&badge, &paint(r, g, b, 255), FillRule::Winding, grid, None);
            }
            let mut mark = PathBuilder::new();
            if state == TrayState::Success {
                mark.move_to(20.5, 24.0);
                mark.line_to(23.0, 26.5);
                mark.line_to(27.5, 21.5);
            } else {
                mark.move_to(24.0, 20.5);
                mark.line_to(24.0, 24.5);
                mark.move_to(24.0, 27.3);
                mark.line_to(24.0, 27.4);
            }
            if let Some(path) = mark.finish() {
                pixmap.stroke_path(&path, &paint(255, 255, 255, 255), &stroke(2.0), grid, None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn tray_icon_is_square_rgba_and_has_the_brand_corners() {
        let rgba = super::tray_rgba(32, super::TrayState::Normal);
        assert_eq!(rgba.len(), 32 * 32 * 4);
        let px = |x: usize, y: usize| &rgba[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4];
        assert_eq!(
            px(0, 0)[3],
            0,
            "corner outside the rounded square is transparent"
        );
        assert_eq!(px(16, 16), [255, 255, 255, 255], "the spark is white");
        let c = px(3, 16);
        assert!(
            c[3] == 255 && c[2] > c[1],
            "the background is the violet-cyan gradient"
        );
    }

    fn px(rgba: &[u8], x: usize, y: usize) -> [u8; 4] {
        rgba[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4]
            .try_into()
            .unwrap()
    }

    #[test]
    fn progress_ring_fills_clockwise_from_the_top() {
        let rgba = super::tray_rgba(32, super::TrayState::Progress(0.5));
        let right = px(&rgba, 27, 16); // on the ring, in the first half
        let left = px(&rgba, 5, 16); // on the ring, not reached yet
        assert!(right[0] > 230 && right[1] > 230, "{right:?}");
        assert!(left[0] < 200, "{left:?}");
    }

    #[test]
    fn success_and_failure_add_a_coloured_badge() {
        let ok = px(&super::tray_rgba(32, super::TrayState::Success), 22, 22);
        assert!(ok[1] > 150 && ok[0] < 120, "green badge: {ok:?}");
        let bad = px(&super::tray_rgba(32, super::TrayState::Failure), 22, 22);
        assert!(bad[0] > 200 && bad[1] < 120, "red badge: {bad:?}");
    }
}
