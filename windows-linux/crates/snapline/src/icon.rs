//! The tray icon: the brand square (violet → cyan) with the white selection corners and spark (BRAND.md).

use snapline_overlay::tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LinearGradient, Paint, PathBuilder, Pixmap, Point,
    SpreadMode, Stroke, Transform,
};

/// Straight (not premultiplied) RGBA, `size`×`size`.
///
/// # Panics
/// If `size` is 0.
#[must_use]
pub fn tray_rgba(size: u32) -> Vec<u8> {
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

    pixmap
        .pixels()
        .iter()
        .flat_map(|c| {
            let c = c.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn tray_icon_is_square_rgba_and_has_the_brand_corners() {
        let rgba = super::tray_rgba(32);
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
}
