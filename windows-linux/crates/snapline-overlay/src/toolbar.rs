//! The two toolbars next to the selection, and the colour palette. Layout and hit testing only;
//! `render` draws them.

use crate::annotation::Tool;
use crate::color::PALETTE;
use snapline_core::search::SearchEngine;
use snapline_core::selection_geometry::{Point, Rect, Size};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Tool(Tool),
    Color,
    Undo,
    Copy,
    Save,
    Close,
    /// A palette colour, by index into `PALETTE`.
    Swatch(usize),
    Upload,
    Search,
    /// An entry of the search menu (a text label).
    Engine(SearchEngine),
}

impl Button {
    /// Tooltip text.
    #[must_use]
    pub const fn tip(self) -> &'static str {
        match self {
            Self::Tool(t) => t.title(),
            Self::Color => "Color",
            Self::Undo => "Undo (Ctrl+Z)",
            Self::Copy => "Copy image (Ctrl+C)",
            Self::Save => "Save to folder (Ctrl+S). Shift-click or Ctrl+Shift+S: Save as",
            Self::Close => "Close (Esc)",
            Self::Swatch(_) | Self::Engine(_) => "",
            Self::Upload => "Upload and copy link (Ctrl+D)",
            Self::Search => "Search similar images",
        }
    }
}

pub const DRAW_BAR: [Button; 10] = [
    Button::Tool(Tool::Pencil),
    Button::Tool(Tool::Line),
    Button::Tool(Tool::Arrow),
    Button::Tool(Tool::Rectangle),
    Button::Tool(Tool::Ellipse),
    Button::Tool(Tool::Marker),
    Button::Tool(Tool::Text),
    Button::Tool(Tool::Hide),
    Button::Color,
    Button::Undo,
];

pub const MAIN_BAR: [Button; 5] = [
    Button::Upload,
    Button::Search,
    Button::Copy,
    Button::Save,
    Button::Close,
];

const BUTTON: Size = Size {
    width: 30.0,
    height: 28.0,
};
const PAD: f64 = 4.0;
const SPACING: f64 = 2.0;
const GAP: f64 = 8.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    pub frame: Rect,
    pub buttons: Vec<(Button, Rect)>,
}

fn bar_size(n: usize, horizontal: bool, s: f64) -> Size {
    let n = n as f64;
    let long = 2.0 * PAD
        + n * if horizontal {
            BUTTON.width
        } else {
            BUTTON.height
        }
        + (n - 1.0) * SPACING;
    if horizontal {
        Size {
            width: long * s,
            height: (2.0 * PAD + BUTTON.height) * s,
        }
    } else {
        Size {
            width: (2.0 * PAD + BUTTON.width) * s,
            height: long * s,
        }
    }
}

fn bar(buttons: &[Button], horizontal: bool, origin: Point, s: f64) -> Bar {
    let size = bar_size(buttons.len(), horizontal, s);
    let (bw, bh, pad, sp) = (BUTTON.width * s, BUTTON.height * s, PAD * s, SPACING * s);
    let buttons = buttons
        .iter()
        .enumerate()
        .map(|(k, b)| {
            let k = k as f64;
            let (x, y) = if horizontal {
                (origin.x + pad + k * (bw + sp), origin.y + pad)
            } else {
                (origin.x + pad, origin.y + pad + k * (bh + sp))
            };
            (*b, Rect::new(x, y, bw, bh))
        })
        .collect();
    Bar {
        frame: Rect::new(origin.x, origin.y, size.width, size.height),
        buttons,
    }
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

/// Moves `r` out of `obstacle`: first sideways (left, then right), then up, then down; the first spot
/// that is on screen and clear wins. Keeps `r` where it is when nothing fits.
fn step_aside(r: Rect, obstacle: Rect, gap: f64, bounds: Rect) -> Rect {
    if !overlaps(r, obstacle) {
        return r;
    }
    let candidates = [
        Rect {
            x: obstacle.x - r.width - gap,
            ..r
        },
        Rect {
            x: obstacle.x + obstacle.width + gap,
            ..r
        },
        Rect {
            y: obstacle.y - r.height - gap,
            ..r
        },
        Rect {
            y: obstacle.y + obstacle.height + gap,
            ..r
        },
    ];
    let on_screen = |c: &Rect| {
        c.x >= bounds.x
            && c.y >= bounds.y
            && c.x + c.width <= bounds.x + bounds.width
            && c.y + c.height <= bounds.y + bounds.height
    };
    candidates
        .into_iter()
        .find(|c| on_screen(c) && !overlaps(*c, obstacle))
        .unwrap_or(r)
}

/// The search menu's entries are text, 200 × 28 points.
const MENU_ITEM: Size = Size {
    width: 200.0,
    height: 28.0,
};

fn menu(buttons: &[Button], origin: Point, s: f64) -> Bar {
    let (w, h, pad, sp) = (
        MENU_ITEM.width * s,
        MENU_ITEM.height * s,
        PAD * s,
        SPACING * s,
    );
    let n = buttons.len() as f64;
    let frame = Rect::new(
        origin.x,
        origin.y,
        w + 2.0 * pad,
        2.0 * pad + n * h + (n - 1.0) * sp,
    );
    let buttons = buttons
        .iter()
        .enumerate()
        .map(|(k, b)| {
            (
                *b,
                Rect::new(origin.x + pad, origin.y + pad + k as f64 * (h + sp), w, h),
            )
        })
        .collect();
    Bar { frame, buttons }
}

/// Keeps `v` in `0..=limit`, even when the bar is larger than the screen.
fn fit(v: f64, limit: f64) -> f64 {
    v.min(limit).max(0.0)
}

/// The search menu: under the Search button (above the main bar when there is no room), clear of the other bars.
fn search_menu(
    main: &Bar,
    [main_frame, draw_frame]: [Rect; 2],
    palette: Option<Rect>,
    bounds: Rect,
    s: f64,
) -> Bar {
    let gap = GAP * s;
    let (right, bottom) = (bounds.x + bounds.width, bounds.y + bounds.height);
    let engines = SearchEngine::ALL.map(Button::Engine);
    let probe = menu(&engines, Point::default(), s).frame;
    let search = main
        .buttons
        .iter()
        .find(|(b, _)| *b == Button::Search)
        .map_or(main_frame, |(_, r)| *r);
    let below = main_frame.y + main_frame.height + gap;
    let y = if below + probe.height <= bottom {
        below
    } else {
        main_frame.y - probe.height - gap
    };
    let mut frame = Rect::new(
        fit(search.x - PAD * s, right - probe.width),
        fit(y, bottom - probe.height),
        probe.width,
        probe.height,
    );
    frame = step_aside(
        step_aside(frame, draw_frame, gap, bounds),
        main_frame,
        gap,
        bounds,
    );
    if let Some(palette) = palette {
        frame = step_aside(frame, palette, gap, bounds);
    }
    menu(
        &engines,
        Point {
            x: frame.x,
            y: frame.y,
        },
        s,
    )
}

/// Where the bars go for a selection, in pixels: main bar, draw bar, then the palette when it is open.
#[must_use]
pub fn layout(sel: Rect, bounds: Rect, s: f64, palette_open: bool, search_open: bool) -> Vec<Bar> {
    let gap = GAP * s;
    let (right, bottom) = (bounds.x + bounds.width, bounds.y + bounds.height);
    let (sel_right, sel_bottom) = (sel.x + sel.width, sel.y + sel.height);

    let main = bar_size(MAIN_BAR.len(), true, s);
    let mut main_at = Point {
        x: sel_right - main.width,
        y: sel_bottom + gap,
    };
    if main_at.y + main.height > bottom {
        main_at.y = if sel.y - main.height - gap >= 0.0 {
            sel.y - main.height - gap
        } else {
            sel_bottom - main.height - gap
        };
    }
    main_at.x = fit(main_at.x, right - main.width);
    main_at.y = fit(main_at.y, bottom - main.height);

    let draw = bar_size(DRAW_BAR.len(), false, s);
    let mut draw_at = Point {
        x: sel_right + gap,
        y: sel.y,
    };
    if draw_at.x + draw.width > right {
        draw_at.x = if sel.x - draw.width - gap >= 0.0 {
            sel.x - draw.width - gap
        } else {
            sel_right - draw.width - gap
        };
    }
    draw_at.x = fit(draw_at.x, right - draw.width);
    draw_at.y = fit(draw_at.y, bottom - draw.height);

    // At a corner the draw bar can flip onto the main bar; a click would then hit the bar underneath.
    let draw_frame = Rect::new(draw_at.x, draw_at.y, draw.width, draw.height);
    let main_frame = step_aside(
        Rect::new(main_at.x, main_at.y, main.width, main.height),
        draw_frame,
        gap,
        bounds,
    );
    let main_at = Point {
        x: main_frame.x,
        y: main_frame.y,
    };

    let mut bars = vec![
        bar(&MAIN_BAR, true, main_at, s),
        bar(&DRAW_BAR, false, draw_at, s),
    ];
    if palette_open {
        let swatches: Vec<Button> = (0..PALETTE.len()).map(Button::Swatch).collect();
        let size = bar_size(swatches.len(), true, s);
        let color_y = bars[1]
            .buttons
            .iter()
            .find(|(b, _)| *b == Button::Color)
            .map_or(draw_at.y, |(_, r)| r.y - PAD * s);
        let mut x = draw_at.x - size.width - gap;
        if x < 0.0 {
            x = draw_at.x + draw.width + gap;
        }
        let frame = Rect::new(
            fit(x, right - size.width),
            fit(color_y, bottom - size.height),
            size.width,
            size.height,
        );
        let frame = step_aside(
            step_aside(frame, main_frame, gap, bounds),
            draw_frame,
            gap,
            bounds,
        );
        bars.push(bar(
            &swatches,
            true,
            Point {
                x: frame.x,
                y: frame.y,
            },
            s,
        ));
    }
    if search_open {
        let palette = palette_open.then(|| bars[2].frame);
        let menu = search_menu(&bars[0], [main_frame, draw_frame], palette, bounds, s);
        bars.push(menu);
    }
    bars
}

/// The button under `p`, if any.
#[must_use]
pub fn hit(bars: &[Bar], p: Point) -> Option<Button> {
    bars.iter()
        .flat_map(|b| b.buttons.iter())
        .find(|(_, r)| crate::session::contains(*r, p))
        .map(|(b, _)| *b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapline_core::search::SearchEngine;

    const SCREEN: Rect = Rect::new(0.0, 0.0, 1000.0, 800.0);

    fn inside(r: Rect, outer: Rect) -> bool {
        r.x >= outer.x
            && r.y >= outer.y
            && r.x + r.width <= outer.x + outer.width
            && r.y + r.height <= outer.y + outer.height
    }

    #[test]
    fn main_bar_sits_under_the_selection_right_aligned() {
        let bars = layout(
            Rect::new(100.0, 100.0, 200.0, 100.0),
            SCREEN,
            1.0,
            false,
            false,
        );
        let main = bars[0].frame;
        assert_eq!(main.x + main.width, 300.0);
        assert_eq!(main.y, 208.0);
    }

    #[test]
    fn main_bar_flips_above_at_the_bottom_edge() {
        let bars = layout(
            Rect::new(100.0, 700.0, 200.0, 100.0),
            SCREEN,
            1.0,
            false,
            false,
        );
        let main = bars[0].frame;
        assert_eq!(main.y + main.height, 692.0);
    }

    #[test]
    fn draw_bar_flips_left_at_the_right_edge() {
        let bars = layout(
            Rect::new(900.0, 100.0, 100.0, 100.0),
            SCREEN,
            1.0,
            false,
            false,
        );
        let draw = bars[1].frame;
        assert_eq!(draw.x + draw.width, 892.0);
    }

    // Review Focus 2.
    #[test]
    fn full_screen_selection_keeps_toolbars_on_screen() {
        for scale in [1.0, 1.5, 2.0] {
            for bar in layout(SCREEN, SCREEN, scale, true, true) {
                assert!(
                    inside(bar.frame, SCREEN),
                    "{:?} at scale {scale}",
                    bar.frame
                );
            }
        }
        // A screen smaller than a bar must not panic.
        let tiny = Rect::new(0.0, 0.0, 50.0, 40.0);
        let _ = layout(tiny, tiny, 1.0, true, true);
    }

    #[test]
    fn scale_doubles_the_bars() {
        let one = layout(
            Rect::new(100.0, 100.0, 200.0, 100.0),
            SCREEN,
            1.0,
            false,
            false,
        )[1]
        .frame;
        let two = layout(
            Rect::new(100.0, 100.0, 200.0, 100.0),
            SCREEN,
            2.0,
            false,
            false,
        )[1]
        .frame;
        assert_eq!(two.width, one.width * 2.0);
    }

    #[test]
    fn hit_finds_the_button_under_the_pointer() {
        let bars = layout(
            Rect::new(100.0, 100.0, 200.0, 100.0),
            SCREEN,
            1.0,
            true,
            false,
        );
        let (button, r) = bars[1].buttons[0];
        assert_eq!(button, Button::Tool(Tool::Pencil));
        assert_eq!(
            hit(
                &bars,
                Point {
                    x: r.x + 5.0,
                    y: r.y + 5.0
                }
            ),
            Some(Button::Tool(Tool::Pencil))
        );
        assert_eq!(hit(&bars, Point { x: 150.0, y: 150.0 }), None);
        assert_eq!(bars.len(), 3, "the palette is the third bar when open");
    }

    // A click on one bar must never land on another bar drawn underneath (review finding: corner selections).
    #[test]
    fn bars_never_overlap() {
        for scale in [1.0, 1.5, 2.0] {
            for (x, y, w, h) in [
                (940.0, 700.0, 50.0, 90.0),
                (950.0, 760.0, 50.0, 40.0),
                (0.0, 0.0, 1000.0, 800.0),
                (0.0, 760.0, 60.0, 40.0),
                (900.0, 0.0, 100.0, 60.0),
                (400.0, 300.0, 200.0, 100.0),
                (980.0, 400.0, 20.0, 20.0),
            ] {
                for (palette, search) in
                    [(false, false), (true, false), (false, true), (true, true)]
                {
                    let bars = layout(Rect::new(x, y, w, h), SCREEN, scale, palette, search);
                    for (k, a) in bars.iter().enumerate() {
                        for b in &bars[k + 1..] {
                            assert!(
                                !overlaps(a.frame, b.frame),
                                "{:?} overlaps {:?} for selection {:?} at scale {scale}",
                                a.frame,
                                b.frame,
                                (x, y, w, h)
                            );
                        }
                        assert!(inside(a.frame, SCREEN), "{:?} off screen", a.frame);
                    }
                }
            }
        }
    }

    #[test]
    fn search_menu_sits_under_the_search_button() {
        let bars = layout(
            // Tall enough that the draw bar ends above the menu, so nothing has to step aside.
            Rect::new(300.0, 100.0, 300.0, 400.0),
            SCREEN,
            1.0,
            false,
            true,
        );
        assert_eq!(bars.len(), 3);
        let menu = &bars[2];
        let search = bars[0]
            .buttons
            .iter()
            .find(|(b, _)| *b == Button::Search)
            .unwrap()
            .1;
        assert!(
            menu.frame.y >= bars[0].frame.y + bars[0].frame.height,
            "{:?}",
            menu.frame
        );
        assert!((menu.frame.x - (search.x - 4.0)).abs() < 1.0);
        let engines: Vec<_> = menu.buttons.iter().map(|(b, _)| *b).collect();
        assert_eq!(
            engines,
            [
                Button::Engine(SearchEngine::GoogleLens),
                Button::Engine(SearchEngine::TinEye)
            ]
        );
    }
}
