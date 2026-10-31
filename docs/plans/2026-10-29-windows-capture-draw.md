# Phase 2: Windows capture and draw — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Windows build of Snapline that lives in the tray, captures every monitor on a shortcut, lets you select an area, draw on it with all eight tools, undo, and copy or save the result as PNG.

**Architecture:** Everything the overlay does (selection, drawing, text, toolbars, rendering) lives in a new crate, `snapline-overlay`. It has no windows and no OS calls, works in screen pixels, and is tested on Windows and Linux CI. The app crate `snapline` is thin glue: winit windows, one per monitor, show pixmaps that `snapline-overlay` draws with tiny-skia through softbuffer, plus the tray, the global shortcuts, screen capture, the clipboard and the save dialog. The screen and the saved image are drawn by the same code, so they cannot drift apart.

**Tech Stack:** Rust 2024 (stable), tiny-skia, ab_glyph, winit 0.30, softbuffer, xcap, tray-icon, global-hotkey, arboard, rfd, serde + toml, dirs, chrono, windows-sys.

**Spec:** [docs/windows-linux.md](../windows-linux.md) (Phase 2), with the macOS app as the reference for behaviour: `macOS/Sources/Snapline/OverlayView*.swift`, `Annotations.swift`, `CaptureSession.swift`, `Actions.swift`.

## Decisions made for this phase

Barayuda's choices on 29 October 2026, which this plan follows:

| Question | Choice |
|---|---|
| How the overlay is drawn | tiny-skia only, shown through softbuffer. No egui in the overlay. Preferences (Phase 5) will be a separate egui window. |
| Default capture shortcut | **Both** PrtScn and Ctrl+Shift+1 |
| Choosing a drawing colour | A built-in palette of 8 colours next to the toolbar, like Lightshot |

Behaviour that follows macOS but uses Windows keys: Ctrl instead of ⌘, and Save As is Ctrl+Shift+S (⌥⌘S on macOS). Clicking the tray icon opens the menu by default, matching the macOS default since PR #2; `tray_click = "capture"` in the settings file switches it.

Left for later phases, on purpose: auto-hide and its toggle button (Phase 4), Copy Text (4), upload, search, email, print, edit (3 and 5), the "Screenshot saved" confirmation popup (it reuses Phase 3's popup), the Preferences window and the downscale option (5).

## Global Constraints

- Workspace: `windows-linux/`, edition 2024, the toolchain in `rust-toolchain.toml` (stable).
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` with the workspace's `clippy::pedantic` must pass. Each crate root allows only `clippy::cast_possible_truncation`, `clippy::cast_sign_loss` and `clippy::cast_precision_loss` (pixel maths on values that are clamped first). Fix every other lint.
- Add crates with `cargo add` so the latest stable release is used. This plan's code matches winit 0.30 (0.31 is still beta, do not use it), softbuffer 0.4, tiny-skia 0.12, ab_glyph 0.2, xcap 0.9, tray-icon 0.26, global-hotkey 0.9, arboard 3, rfd 0.17, toml 1, dirs 7, windows-sys 0.61. If a newer major comes in, fix the call sites, not the behaviour.
- Use tiny-skia only through `snapline_overlay::tiny_skia` in the app crate, so both crates use one version.
- `snapline-overlay` must not depend on winit, softbuffer, xcap, tray-icon or any OS API. It must build and test on Linux.
- The app crate `snapline` is built and tested on Windows CI only (Linux is Phase 6). It must also build on macOS, so it can be tried locally; Windows-only calls go behind `#[cfg(windows)]` in `src/platform.rs`.
- All coordinates in `snapline-overlay` are pixels of one monitor's capture, top-left origin, as `snapline_core::selection_geometry::{Point, Rect}` (f64). UI sizes from macOS are points; multiply them by the monitor's `ui_scale` (winit's scale factor).
- Fonts: text uses Segoe UI Semibold on Windows (`C:\Windows\Fonts\seguisb.ttf`) and falls back to the bundled `scripts/brand/fonts/Outfit-Bold.ttf` (OFL). Tests always use the bundled font.
- Git: commit after each task, GPG-signed (never `--no-gpg-sign`), plain sentence-case messages like the existing history, no AI co-author or "generated with" lines. Check `git log --format='%h %G?'` shows `G` before pushing.
- Copy in the UI is plain English, sentence case, and names keys the Windows way (Ctrl, Shift, PrtScn, Esc).

## Review Focus

The inputs most likely to hurt a real user that no single feature test covers. Each has a test in the task named.

1. **Monitors with different scaling (100 % next to 150 %).** Line widths, text and toolbars must scale with the monitor; the saved image must be exactly the selected pixels. Test: `width_and_export_follow_the_monitor_scale` (Task 4).
2. **Selecting the whole screen, or an area at a screen edge.** The toolbars must stay fully on screen (they move inside the selection when there is no room outside). Test: `full_screen_selection_keeps_toolbars_on_screen` (Task 6).
3. **A broken or hand-edited `settings.toml`.** Snapline must start with defaults, say which file is wrong, and leave the file untouched. Test: `broken_settings_file_gives_defaults_and_a_reason` (Task 8).
4. **Typing text and then pressing Ctrl+C, Enter or Esc without clicking away.** The text must be in the copied image, and Esc must finish the text, not throw the capture away. Tests: `ctrl_c_while_typing_keeps_the_text`, `escape_while_typing_finishes_text_instead_of_closing` (Task 5).
5. **Keyboards that type with AltGr (German, Polish, French…).** Windows reports AltGr as Ctrl+Alt; `@`, `€` or `{` typed into a text box must appear as text, not run a Ctrl shortcut. Test: `altgr_characters_are_typed_not_shortcuts` (Task 5).

---

## File structure

```
windows-linux/crates/
├── snapline-core/                 (Phase 1, unchanged)
├── snapline-overlay/              new: the overlay without windows, tested on every OS
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                 module list, re-exports (tiny_skia, geometry types)
│       ├── color.rs               Rgb, brand colours, the 8-colour palette
│       ├── annotation.rs          Tool, Shape, Annotation: what a drawing is and how a drag shapes it
│       ├── text.rs                TextFont: measure and draw text into a pixmap
│       ├── render.rs              draw annotations, flatten the selection for export, draw the whole screen
│       ├── input.rs               Key, Mods: keyboard input in the overlay's own terms
│       ├── session.rs             Session + Overlay: the state machine for pointer, keys, text, undo, cursor
│       ├── toolbar.rs             Button, Bar, layout and hit testing of the two toolbars and the palette
│       └── icons.rs               the toolbar icons as tiny-skia paths
└── snapline/                      new: the Windows app (also builds on macOS for local tries)
    ├── Cargo.toml
    └── src/
        ├── main.rs                single instance, event loop
        ├── app.rs                 ApplicationHandler: tray, shortcuts, capture lifecycle, event → session
        ├── capture.rs             xcap: freeze every monitor into a Pixmap
        ├── window.rs              OverlayWindow: one borderless topmost winit window + softbuffer surface
        ├── settings.rs            Settings from %APPDATA%\Snapline\settings.toml
        ├── actions.rs             copy, save, save as
        ├── icon.rs                the tray icon, drawn with tiny-skia
        └── platform.rs            Windows-only calls: single-instance mutex, beep, cursor position
```

---

### Task 1: The overlay crate and the annotation model

**Files:**
- Create: `windows-linux/crates/snapline-overlay/Cargo.toml`
- Create: `windows-linux/crates/snapline-overlay/src/lib.rs`
- Create: `windows-linux/crates/snapline-overlay/src/color.rs`
- Create: `windows-linux/crates/snapline-overlay/src/annotation.rs`

**Interfaces:**
- Consumes: `snapline_core::selection_geometry::{Point, Rect, rect}`.
- Produces: `Rgb(u8, u8, u8)`, `color::{VIOLET, VIOLET_LIGHT, CYAN, INK, PALETTE: [Rgb; 8]}`; `Tool` (`ALL`, `for_key(char) -> Option<Tool>`, `title() -> &str`); `Shape`; `Annotation { shape, color, width: f32, marker: bool }` with `begin(tool, p, color, width) -> Option<Annotation>`, `extend(&mut self, start, p)`, `is_path() -> bool`.

- [ ] **Step 1: Create the crate**

`windows-linux/crates/snapline-overlay/Cargo.toml`:

```toml
[package]
name = "snapline-overlay"
version = "0.1.0"
edition.workspace = true
license-file.workspace = true
publish.workspace = true

[lints]
workspace = true

[dependencies]
snapline-core = { path = "../snapline-core" }
```

`src/lib.rs`:

```rust
//! The capture overlay without any window: selection, drawing, text, toolbars and rendering, in screen pixels.
//! The app's windows only feed it input and show the pixmaps it draws. Twin of the macOS `OverlayView`.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)] // pixel maths on clamped values

pub mod annotation;
pub mod color;

pub use snapline_core::selection_geometry::{Handle, Point, Rect, Size};
```

`src/color.rs`:

```rust
//! Colours: the brand (BRAND.md) and the drawing palette.

/// An opaque colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

pub const VIOLET: Rgb = Rgb(0x7C, 0x3A, 0xED);
pub const VIOLET_LIGHT: Rgb = Rgb(0x8B, 0x5C, 0xF6);
pub const CYAN: Rgb = Rgb(0x22, 0xD3, 0xEE);
pub const INK: Rgb = Rgb(0x0C, 0x08, 0x19);
pub const WHITE: Rgb = Rgb(0xFF, 0xFF, 0xFF);
pub const BLACK: Rgb = Rgb(0, 0, 0);

/// The drawing colours in the palette. The first is the default, the same red as macOS.
pub const PALETTE: [Rgb; 8] = [
    Rgb(0xFF, 0x3B, 0x30), // red
    Rgb(0xFF, 0x95, 0x00), // orange
    Rgb(0xFF, 0xCC, 0x00), // yellow
    Rgb(0x34, 0xC7, 0x59), // green
    Rgb(0x00, 0x7A, 0xFF), // blue
    VIOLET,
    BLACK,
    WHITE,
];
```

- [ ] **Step 2: Write the failing tests**

`src/annotation.rs` (tests first; the types come in Step 4):

```rust
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
        assert_eq!(a.shape, Shape::Path(vec![pt(0.0, 0.0), pt(1.0, 1.0), pt(2.0, 2.0)]));
        assert!(a.is_path() && !a.marker);
        assert!(Annotation::begin(Tool::Marker, pt(0.0, 0.0), RED, 3.0).unwrap().marker);
    }

    #[test]
    fn text_tool_does_not_begin_a_shape() {
        assert!(Annotation::begin(Tool::Text, pt(0.0, 0.0), RED, 3.0).is_none());
    }
}
```

Add `pub mod annotation;` is already in `lib.rs`. Put `use crate::color::Rgb; use snapline_core::selection_geometry::{self as geo, Point, Rect};` at the top of the file.

- [ ] **Step 3: Run the tests to see them fail**

Run: `cd windows-linux && cargo test -p snapline-overlay`
Expected: compile errors, `Tool`, `Annotation` and `Shape` not found.

- [ ] **Step 4: Write the model**

Above the tests in `src/annotation.rs`:

```rust
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
    Text { origin: Point, text: String, size: f32 },
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
        Some(Self { shape, color, width, marker: tool == Tool::Marker })
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
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cd windows-linux && cargo test -p snapline-overlay && cargo clippy -p snapline-overlay --all-targets -- -D warnings && cargo fmt --check`
Expected: 5 tests pass, no clippy warnings.

- [ ] **Step 6: Commit**

```bash
git add windows-linux/crates/snapline-overlay windows-linux/Cargo.lock
git commit -m "Add the overlay crate with the drawing tools and shapes"
```

---

### Task 2: Text — measure and draw with a real font

**Files:**
- Create: `windows-linux/crates/snapline-overlay/src/text.rs`
- Modify: `windows-linux/crates/snapline-overlay/src/lib.rs` (add `pub mod text;` and `pub use tiny_skia;`)
- Modify: `windows-linux/crates/snapline-overlay/Cargo.toml` (via `cargo add`)

**Interfaces:**
- Consumes: `Rgb`, `Point`, `Size`.
- Produces: `TextFont` (`Clone`) with `bundled() -> TextFont`, `system() -> TextFont`, `measure(&self, text, size: f32) -> Size` (width of the text, height of one line), `draw(&self, pixmap: &mut Pixmap, text, origin: Point /*top-left*/, size: f32, color: Rgb)`. `snapline_overlay::tiny_skia` re-export.

- [ ] **Step 1: Add the crates**

Run: `cd windows-linux && cargo add -p snapline-overlay tiny-skia ab_glyph`

In `lib.rs` add `pub mod text;` and `pub use tiny_skia;`.

- [ ] **Step 2: Write the failing tests**

`src/text.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{PALETTE, WHITE};
    use tiny_skia::Color;

    fn white(w: u32, h: u32) -> Pixmap {
        let mut p = Pixmap::new(w, h).unwrap();
        p.fill(Color::WHITE);
        p
    }

    #[test]
    fn longer_text_is_wider_and_lines_have_height() {
        let f = TextFont::bundled();
        let short = f.measure("Hi", 20.0);
        let long = f.measure("Hi there", 20.0);
        assert!(long.width > short.width && short.width > 0.0);
        assert!(short.height > 20.0 && short.height < 40.0);
        assert!(f.measure("", 20.0).width.abs() < f64::EPSILON);
    }

    #[test]
    fn draws_inside_its_measured_box_only() {
        let f = TextFont::bundled();
        let mut p = white(200, 60);
        f.draw(&mut p, "Hi", Point { x: 10.0, y: 10.0 }, 30.0, PALETTE[0]);
        let m = f.measure("Hi", 30.0);
        let reddish = |x: u32, y: u32| {
            let c = p.pixel(x, y).unwrap();
            c.red() > 200 && c.green() < 120
        };
        let inside = (10..(10 + m.width as u32)).any(|x| (10..(10 + m.height as u32)).any(|y| reddish(x, y)));
        assert!(inside, "no text pixels inside the measured box");
        let right = 10 + m.width as u32 + 3;
        assert!((right..200).all(|x| (0..60).all(|y| p.pixel(x, y).unwrap().red() == 255
            && p.pixel(x, y).unwrap().green() == 255)), "text drawn past its width");
    }

    #[test]
    fn text_at_the_edges_does_not_panic() {
        let f = TextFont::bundled();
        let mut p = white(50, 20);
        f.draw(&mut p, "Edge", Point { x: -8.0, y: -8.0 }, 24.0, WHITE);
        f.draw(&mut p, "Edge", Point { x: 45.0, y: 15.0 }, 24.0, WHITE);
    }
}
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cd windows-linux && cargo test -p snapline-overlay text`
Expected: compile errors, `TextFont` not found.

- [ ] **Step 4: Write `TextFont`**

Above the tests:

```rust
//! Text for the text tool and the overlay's labels: measure and draw with one font into a tiny-skia pixmap.

use crate::color::Rgb;
use ab_glyph::{Font as _, FontArc, PxScale, ScaleFont as _};
use snapline_core::selection_geometry::{Point, Size};
use tiny_skia::{Pixmap, PremultipliedColorU8};

/// Outfit Bold from the brand fonts (OFL). The fallback, and the font every test uses.
static BUNDLED: &[u8] = include_bytes!("../../../../scripts/brand/fonts/Outfit-Bold.ttf");

#[derive(Clone)]
pub struct TextFont(FontArc);

impl TextFont {
    /// # Panics
    /// Never: the bundled font is checked by the tests.
    #[must_use]
    pub fn bundled() -> Self {
        Self(FontArc::try_from_slice(BUNDLED).expect("bundled font is valid"))
    }

    /// Segoe UI Semibold on Windows, so text looks native; the bundled font elsewhere or when it is missing.
    #[must_use]
    pub fn system() -> Self {
        #[cfg(windows)]
        {
            if let Some(font) = std::env::var_os("WINDIR")
                .and_then(|dir| std::fs::read(std::path::Path::new(&dir).join("Fonts").join("seguisb.ttf")).ok())
                .and_then(|bytes| FontArc::try_from_vec(bytes).ok())
            {
                return Self(font);
            }
        }
        Self::bundled()
    }

    /// Width of `text` and height of one line, in pixels.
    #[must_use]
    pub fn measure(&self, text: &str, size: f32) -> Size {
        let f = self.0.as_scaled(PxScale::from(size));
        let mut width = 0.0;
        let mut prev = None;
        for c in text.chars() {
            let id = f.glyph_id(c);
            if let Some(p) = prev {
                width += f.kern(p, id);
            }
            width += f.h_advance(id);
            prev = Some(id);
        }
        Size { width: f64::from(width), height: f64::from(f.ascent() - f.descent()) }
    }

    /// Draws `text` with the top-left corner of its line at `origin`, blending into `pixmap`.
    /// Anything outside the pixmap is skipped.
    pub fn draw(&self, pixmap: &mut Pixmap, text: &str, origin: Point, size: f32, color: Rgb) {
        let scale = PxScale::from(size);
        let f = self.0.as_scaled(scale);
        let baseline = origin.y as f32 + f.ascent();
        let (w, h) = (pixmap.width() as i32, pixmap.height() as i32);
        let pixels = pixmap.pixels_mut();
        let mut x = origin.x as f32;
        let mut prev = None;
        for c in text.chars() {
            let id = f.glyph_id(c);
            if let Some(p) = prev {
                x += f.kern(p, id);
            }
            let glyph = id.with_scale_and_position(scale, ab_glyph::point(x, baseline));
            x += f.h_advance(id);
            prev = Some(id);
            let Some(outline) = self.0.outline_glyph(glyph) else { continue };
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                let (px, py) = (bounds.min.x as i32 + gx as i32, bounds.min.y as i32 + gy as i32);
                if px < 0 || py < 0 || px >= w || py >= h {
                    return;
                }
                let i = (py * w + px) as usize;
                pixels[i] = blend(pixels[i], color, coverage.min(1.0));
            });
        }
    }
}

/// `color` at `alpha` over a premultiplied pixel (source-over).
fn blend(dst: PremultipliedColorU8, c: Rgb, alpha: f32) -> PremultipliedColorU8 {
    let mix = |s: u8, d: u8| (f32::from(s) * alpha + f32::from(d) * (1.0 - alpha)).round() as u8;
    PremultipliedColorU8::from_rgba(mix(c.0, dst.red()), mix(c.1, dst.green()), mix(c.2, dst.blue()), mix(255, dst.alpha()))
        .unwrap_or(dst)
}
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cd windows-linux && cargo test -p snapline-overlay && cargo clippy -p snapline-overlay --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add windows-linux
git commit -m "Measure and draw text in the overlay with the brand font"
```

---

### Task 3: Draw annotations and flatten the selection

**Files:**
- Create: `windows-linux/crates/snapline-overlay/src/render.rs`
- Modify: `windows-linux/crates/snapline-overlay/src/lib.rs` (add `pub mod render;`)

**Interfaces:**
- Consumes: `Annotation`, `Shape`, `TextFont`, `Rgb`, `Rect`, `Point`.
- Produces: `render::draw_annotation(pixmap, &Annotation, &TextFont, shift: Point)`, `render::render_selection(screen: &Pixmap, selection: Rect, annotations: &[Annotation], font: &TextFont) -> Option<Pixmap>`. Private helpers later tasks extend in the same file: `paint(Rgb, alpha: u8) -> Paint`, `sk_rect(Rect) -> Option<tiny_skia::Rect>`, `fill_rect(pixmap, Rect, Rgb, alpha)`, `stroke(width, cap) -> Stroke`.

- [ ] **Step 1: Write the failing tests**

`src/render.rs`:

```rust
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
        let out = render_selection(&white(100, 80), Rect::new(10.0, 20.0, 30.0, 40.0), &[], &TextFont::bundled()).unwrap();
        assert_eq!((out.width(), out.height()), (30, 40));
    }

    #[test]
    fn empty_or_offscreen_selection_gives_nothing() {
        let f = TextFont::bundled();
        assert!(render_selection(&white(100, 80), Rect::new(10.0, 10.0, 0.0, 5.0), &[], &f).is_none());
        assert!(render_selection(&white(100, 80), Rect::new(200.0, 200.0, 10.0, 10.0), &[], &f).is_none());
    }

    #[test]
    fn selection_hanging_off_the_screen_is_cut_to_the_screen() {
        let out = render_selection(&white(100, 80), Rect::new(90.0, 70.0, 30.0, 30.0), &[], &TextFont::bundled()).unwrap();
        assert_eq!((out.width(), out.height()), (10, 10));
    }

    #[test]
    fn hide_box_is_solid_black_in_the_export() {
        let hide = drawn(Tool::Hide, pt(15.0, 25.0), pt(25.0, 35.0));
        let out = render_selection(&white(100, 80), Rect::new(10.0, 20.0, 30.0, 40.0), &[hide], &TextFont::bundled()).unwrap();
        assert_eq!(rgb(&out, 10, 10), (0, 0, 0));
        assert_eq!(rgb(&out, 1, 1), (255, 255, 255));
    }

    #[test]
    fn marker_is_see_through() {
        let marker = drawn(Tool::Marker, pt(10.0, 40.0), pt(90.0, 40.0));
        let out = render_selection(&white(100, 80), Rect::new(0.0, 0.0, 100.0, 80.0), &[marker], &TextFont::bundled()).unwrap();
        let (r, g, _) = rgb(&out, 50, 40);
        assert_eq!(r, 255);
        assert!((140..=165).contains(&g), "marker should be 40 % opaque, green was {g}");
    }

    #[test]
    fn arrow_head_is_filled() {
        let arrow = drawn(Tool::Arrow, pt(10.0, 40.0), pt(90.0, 40.0));
        let out = render_selection(&white(100, 80), Rect::new(0.0, 0.0, 100.0, 80.0), &[arrow], &TextFont::bundled()).unwrap();
        // 8 px back from the tip and 2 px off the shaft: inside the head, outside the 3 px shaft.
        let (r, g, _) = rgb(&out, 82, 42);
        assert!(r > 200 && g < 100);
    }

    #[test]
    fn pencil_click_leaves_a_dot() {
        let dot = Annotation::begin(Tool::Pencil, pt(50.0, 40.0), RED, 6.0).unwrap();
        let out = render_selection(&white(100, 80), Rect::new(0.0, 0.0, 100.0, 80.0), &[dot], &TextFont::bundled()).unwrap();
        assert_eq!(rgb(&out, 50, 40), (255, 0x3B, 0x30));
    }

    #[test]
    fn text_moves_with_the_selection() {
        let text = Annotation {
            shape: Shape::Text { origin: pt(20.0, 30.0), text: "H".into(), size: 20.0 },
            color: RED,
            width: 0.0,
            marker: false,
        };
        let out = render_selection(&white(100, 80), Rect::new(10.0, 20.0, 60.0, 50.0), &[text], &TextFont::bundled()).unwrap();
        let red_at = |x, y| { let (r, g, _) = rgb(&out, x, y); r > 200 && g < 120 };
        assert!((10..30).any(|x| (10..35).any(|y| red_at(x, y))));
        assert!(!(40..60).any(|x| (0..50).any(|y| red_at(x, y))));
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cd windows-linux && cargo test -p snapline-overlay render`
Expected: compile errors, `render_selection` not found.

- [ ] **Step 3: Write the drawing code**

Above the tests. Sizes follow `Annotation.draw()` in macOS `Annotations.swift`:

```rust
//! Drawing with tiny-skia. The same code draws the overlay on screen and the exported image.

use crate::annotation::{Annotation, Shape};
use crate::color::{BLACK, Rgb};
use crate::text::TextFont;
use snapline_core::selection_geometry::{Point, Rect};
use std::f64::consts::PI;
use tiny_skia::{FillRule, IntRect, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

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
    Stroke { width, line_cap: cap, line_join: LineJoin::Round, ..Stroke::default() }
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
        pixmap.stroke_path(&path, &paint(c, 255), &stroke(width, LineCap::Round), t, None);
    }
}

/// Draws one annotation. `shift` is subtracted from every point (the export crops to the selection).
pub fn draw_annotation(pixmap: &mut Pixmap, a: &Annotation, font: &TextFont, shift: Point) {
    let t = Transform::from_translate(-shift.x as f32, -shift.y as f32);
    match &a.shape {
        Shape::Path(points) => {
            let (width, cap, alpha) =
                if a.marker { (a.width * 5.0, LineCap::Square, 102) } else { (a.width, LineCap::Round, 255) };
            if let [only] = points.as_slice() {
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
                if k == 0 { pb.move_to(p.x as f32, p.y as f32) } else { pb.line_to(p.x as f32, p.y as f32) }
            }
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &paint(a.color, alpha), &stroke(width, cap), t, None);
            }
        }
        Shape::Line(p, q) => line(pixmap, *p, *q, a.color, a.width, t),
        Shape::Arrow(from, to) => {
            let angle = (to.y - from.y).atan2(to.x - from.x);
            let head = f64::from(a.width * 4.0).max(12.0);
            let base = Point { x: to.x - angle.cos() * head * 0.8, y: to.y - angle.sin() * head * 0.8 };
            line(pixmap, *from, base, a.color, a.width, t);
            let wing = |turn: f64| (to.x - head * (angle + turn).cos(), to.y - head * (angle + turn).sin());
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
        Shape::Rectangle(r) => {
            if let Some(path) = sk_rect(*r).map(PathBuilder::from_rect) {
                pixmap.stroke_path(&path, &paint(a.color, 255), &stroke(a.width, LineCap::Round), t, None);
            }
        }
        Shape::Ellipse(r) => {
            if let Some(path) = sk_rect(*r).and_then(PathBuilder::from_oval) {
                pixmap.stroke_path(&path, &paint(a.color, 255), &stroke(a.width, LineCap::Round), t, None);
            }
        }
        Shape::Hide(r) => {
            fill_rect(pixmap, Rect::new(r.x - shift.x, r.y - shift.y, r.width, r.height), BLACK, 255);
        }
        Shape::Text { origin, text, size } => {
            font.draw(pixmap, text, Point { x: origin.x - shift.x, y: origin.y - shift.y }, *size, a.color);
        }
    }
}

/// The selection flattened into one image: the frozen screen, then every annotation, cut to the screen.
/// `None` when nothing of the selection is on the screen.
#[must_use]
pub fn render_selection(screen: &Pixmap, selection: Rect, annotations: &[Annotation], font: &TextFont) -> Option<Pixmap> {
    let x0 = selection.x.round().max(0.0);
    let y0 = selection.y.round().max(0.0);
    let x1 = (selection.x + selection.width).round().min(f64::from(screen.width()));
    let y1 = (selection.y + selection.height).round().min(f64::from(screen.height()));
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
```

In `lib.rs` add `pub mod render;`.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cd windows-linux && cargo test -p snapline-overlay && cargo clippy -p snapline-overlay --all-targets -- -D warnings`
Expected: all pass. If `marker_is_see_through` is off by a few levels, check the alpha is 102 (40 %), as on macOS.

- [ ] **Step 5: Commit**

```bash
git add windows-linux
git commit -m "Draw annotations and flatten the selection into an image"
```

---

### Task 4: The session — select, move, resize, draw, undo, scroll, cursor, export

**Files:**
- Create: `windows-linux/crates/snapline-overlay/src/input.rs`
- Create: `windows-linux/crates/snapline-overlay/src/session.rs`
- Modify: `windows-linux/crates/snapline-overlay/src/lib.rs` (add `pub mod input; pub mod session;`)

**Interfaces:**
- Consumes: `Annotation`, `Tool`, `Shape`, `TextFont`, `render::render_selection`, `selection_geometry::{handle_at, move_within, resize, rect}`.
- Produces (later tasks rely on these exact names):
  - `input::Key { Escape, Enter, Backspace, Left, Right, Up, Down, Char(char) }`, `input::Mods { ctrl, shift, alt }` (`Default`, `Copy`).
  - `session::Command { Copy, Save, SaveAs, Cancel }` (`Copy`, `Eq`).
  - `session::Redraw { None, One(usize), All }`, `session::Effect { redraw, command: Option<Command>, beep: bool }` with `Effect::NONE`, `one(i)`, `all()`, `command(c)`, `beep()`.
  - `session::CursorKind { Crosshair, Arrow, Move, Text, ResizeHorizontal, ResizeVertical, ResizeDiagonalDown, ResizeDiagonalUp }`.
  - `session::TextEdit { origin: Point, text: String, size: f32 }`.
  - `session::Overlay { pub screen: Pixmap, pub ui_scale: f64, pub selection: Option<Rect>, pub annotations: Vec<Annotation>, pub current: Option<Annotation>, pub editing: Option<TextEdit>, pub pointer: Point, .. }` with `bounds() -> Rect`, `is_dragging() -> bool`.
  - `session::Session { pub overlays: Vec<Overlay>, pub tool: Option<Tool>, pub color: Rgb, pub width: f32 /*points*/, pub font_size: f32 /*points*/, pub focused: usize, pub palette_open: bool, pub font: TextFont }` with `new(screens: Vec<(Pixmap, f64)>, font: TextFont)`, `pointer_down(i, p, mods) -> Effect`, `pointer_move(i, p) -> Effect`, `pointer_up(i, p) -> Effect`, `scroll(i, p, lines: f64) -> Effect`, `undo() -> Effect`, `cursor(i) -> CursorKind`, `selected() -> Option<usize>`, `export(&mut self) -> Option<Pixmap>`, and crate-private `commit_text()`, `text_index(i, p)`, `request(Command) -> Effect`, `clear_selections_except(i)`.

- [ ] **Step 1: Write the input types**

`src/input.rs`:

```rust
//! Keyboard input in the overlay's own terms, so the overlay never sees winit types.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Escape,
    Enter,
    Backspace,
    Left,
    Right,
    Up,
    Down,
    /// A typed character, already shifted ("A", "@", " ").
    Char(char),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}
```

- [ ] **Step 2: Write the failing tests**

`src/session.rs`:

```rust
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use tiny_skia::Color;

    pub(crate) fn pt(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    /// `n` white 400×300 screens at the given UI scale.
    pub(crate) fn session(n: usize, scale: f64) -> Session {
        let screens = (0..n)
            .map(|_| {
                let mut p = Pixmap::new(400, 300).unwrap();
                p.fill(Color::WHITE);
                (p, scale)
            })
            .collect();
        Session::new(screens, TextFont::bundled())
    }

    /// Drags on screen `i` from `a` to `b`.
    pub(crate) fn drag(s: &mut Session, i: usize, a: Point, b: Point) {
        s.pointer_down(i, a, Mods::default());
        s.pointer_move(i, b);
        s.pointer_up(i, b);
    }

    pub(crate) fn selected(scale: f64) -> Session {
        let mut s = session(2, scale);
        drag(&mut s, 0, pt(10.0, 10.0), pt(110.0, 60.0));
        s
    }

    #[test]
    fn dragging_selects_an_area() {
        let s = selected(1.0);
        assert_eq!(s.overlays[0].selection, Some(Rect::new(10.0, 10.0, 100.0, 50.0)));
    }

    #[test]
    fn a_click_without_a_drag_clears_the_selection() {
        let mut s = selected(1.0);
        drag(&mut s, 0, pt(300.0, 200.0), pt(300.0, 200.0));
        assert_eq!(s.overlays[0].selection, None);
    }

    #[test]
    fn selecting_on_another_screen_clears_the_first() {
        let mut s = selected(1.0);
        drag(&mut s, 1, pt(10.0, 10.0), pt(50.0, 50.0));
        assert_eq!(s.overlays[0].selection, None);
        assert!(s.overlays[1].selection.is_some());
        assert_eq!(s.selected(), Some(1));
    }

    #[test]
    fn dragging_inside_moves_the_selection_but_not_off_screen() {
        let mut s = selected(1.0);
        drag(&mut s, 0, pt(50.0, 30.0), pt(-500.0, 30.0));
        assert_eq!(s.overlays[0].selection, Some(Rect::new(0.0, 10.0, 100.0, 50.0)));
    }

    #[test]
    fn dragging_a_handle_resizes() {
        let mut s = selected(1.0);
        drag(&mut s, 0, pt(110.0, 35.0), pt(210.0, 35.0)); // the right-edge handle
        assert_eq!(s.overlays[0].selection.unwrap().width, 200.0);
    }

    #[test]
    fn drawing_adds_an_annotation_and_undo_removes_it() {
        let mut s = selected(1.0);
        s.tool = Some(Tool::Rectangle);
        drag(&mut s, 0, pt(20.0, 20.0), pt(60.0, 50.0));
        assert_eq!(s.overlays[0].annotations.len(), 1);
        assert_eq!(s.undo(), Effect::one(0));
        assert!(s.overlays[0].annotations.is_empty());
        assert!(s.undo().beep, "nothing left to undo");
    }

    #[test]
    fn a_click_draws_only_with_the_pencil() {
        let mut s = selected(1.0);
        s.tool = Some(Tool::Rectangle);
        drag(&mut s, 0, pt(20.0, 20.0), pt(20.0, 20.0));
        assert!(s.overlays[0].annotations.is_empty());
        s.tool = Some(Tool::Pencil);
        drag(&mut s, 0, pt(20.0, 20.0), pt(20.0, 20.0));
        assert_eq!(s.overlays[0].annotations.len(), 1);
    }

    #[test]
    fn with_a_tool_a_click_outside_keeps_the_selection() {
        let mut s = selected(1.0);
        s.tool = Some(Tool::Pencil);
        drag(&mut s, 0, pt(300.0, 200.0), pt(320.0, 220.0));
        assert_eq!(s.overlays[0].selection, Some(Rect::new(10.0, 10.0, 100.0, 50.0)));
        assert!(s.overlays[0].annotations.is_empty());
    }

    // Review Focus 1: monitors with different scaling.
    #[test]
    fn width_and_export_follow_the_monitor_scale() {
        let mut s = selected(1.5);
        s.tool = Some(Tool::Line);
        s.width = 4.0;
        drag(&mut s, 0, pt(20.0, 20.0), pt(60.0, 50.0));
        assert!((s.overlays[0].annotations[0].width - 6.0).abs() < f32::EPSILON);
        let image = s.export().unwrap();
        assert_eq!((image.width(), image.height()), (100, 50), "export is the selected pixels, whatever the scale");
    }

    #[test]
    fn scrolling_changes_the_width_within_limits() {
        let mut s = selected(1.0);
        for _ in 0..30 {
            s.scroll(0, pt(50.0, 30.0), 1.0);
        }
        assert!((s.width - 20.0).abs() < f32::EPSILON);
        for _ in 0..30 {
            s.scroll(0, pt(50.0, 30.0), -1.0);
        }
        assert!((s.width - 1.0).abs() < f32::EPSILON);
        assert_eq!(s.scroll(0, pt(50.0, 30.0), 0.0), Effect::NONE);
    }

    #[test]
    fn cursor_tells_what_a_drag_would_do() {
        let mut s = selected(1.0);
        s.pointer_move(0, pt(110.0, 35.0));
        assert_eq!(s.cursor(0), CursorKind::ResizeHorizontal);
        s.pointer_move(0, pt(10.0, 10.0));
        assert_eq!(s.cursor(0), CursorKind::ResizeDiagonalDown);
        s.pointer_move(0, pt(50.0, 30.0));
        assert_eq!(s.cursor(0), CursorKind::Move);
        s.pointer_move(0, pt(300.0, 200.0));
        assert_eq!(s.cursor(0), CursorKind::Crosshair);
    }

    #[test]
    fn export_needs_a_selection() {
        assert!(session(1, 1.0).export().is_none());
    }
}
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cd windows-linux && cargo test -p snapline-overlay session`
Expected: compile errors, `Session` not found.

- [ ] **Step 4: Write the session**

Above the tests in `src/session.rs`. The logic mirrors `OverlayView+Mouse.swift`; macOS point sizes are multiplied by `ui_scale`.

```rust
//! One capture: an overlay per monitor plus the shared tool, colour and width.
//! Pointer, keyboard and text editing all go through `Session`, which answers with an `Effect`.

use crate::annotation::{Annotation, Shape, Tool};
use crate::color::{PALETTE, Rgb};
use crate::input::Mods;
use crate::render;
use crate::text::TextFont;
use snapline_core::selection_geometry::{self as geo, Handle, Point, Rect};
use tiny_skia::Pixmap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Copy,
    Save,
    SaveAs,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Redraw {
    None,
    One(usize),
    All,
}

/// What the app must do after an input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effect {
    pub redraw: Redraw,
    pub command: Option<Command>,
    pub beep: bool,
}

impl Effect {
    pub const NONE: Self = Self { redraw: Redraw::None, command: None, beep: false };

    #[must_use]
    pub const fn one(i: usize) -> Self {
        Self { redraw: Redraw::One(i), ..Self::NONE }
    }

    #[must_use]
    pub const fn all() -> Self {
        Self { redraw: Redraw::All, ..Self::NONE }
    }

    #[must_use]
    pub const fn command(c: Command) -> Self {
        Self { command: Some(c), ..Self::NONE }
    }

    #[must_use]
    pub const fn beep() -> Self {
        Self { beep: true, ..Self::NONE }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorKind {
    Crosshair,
    Arrow,
    Move,
    Text,
    ResizeHorizontal,
    ResizeVertical,
    /// ↖↘
    ResizeDiagonalDown,
    /// ↗↙
    ResizeDiagonalUp,
}

/// The text box being typed into.
#[derive(Clone, Debug, PartialEq)]
pub struct TextEdit {
    pub origin: Point,
    pub text: String,
    pub size: f32,
}

#[derive(Clone, Copy, Debug)]
enum Drag {
    None,
    Selecting(Point),
    Moving(Point, Rect),
    Resizing(Handle, Rect),
    Drawing(Point),
    MovingText(usize, Point),
}

/// What a mouse-down starts, decided before anything changes.
enum Down {
    Select,
    Resize(Handle, Rect),
    Move(Rect),
    Draw(Tool),
    Text,
    MoveText(usize),
    Nothing,
}

/// One monitor: its frozen image and everything drawn on it, in that image's pixels.
pub struct Overlay {
    pub screen: Pixmap,
    /// Pixels per point on this monitor (winit's scale factor).
    pub ui_scale: f64,
    pub selection: Option<Rect>,
    pub annotations: Vec<Annotation>,
    /// The shape being dragged out.
    pub current: Option<Annotation>,
    pub editing: Option<TextEdit>,
    /// Last pointer position.
    pub pointer: Point,
    drag: Drag,
    down: Point,
    did_drag: bool,
}

impl Overlay {
    fn new(screen: Pixmap, ui_scale: f64) -> Self {
        Self {
            screen,
            ui_scale,
            selection: None,
            annotations: Vec::new(),
            current: None,
            editing: None,
            pointer: Point::default(),
            drag: Drag::None,
            down: Point::default(),
            did_drag: false,
        }
    }

    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(0.0, 0.0, f64::from(self.screen.width()), f64::from(self.screen.height()))
    }

    #[must_use]
    pub fn is_dragging(&self) -> bool {
        !matches!(self.drag, Drag::None)
    }

    fn clamp(&self, p: Point) -> Point {
        let b = self.bounds();
        Point { x: p.x.clamp(0.0, b.width), y: p.y.clamp(0.0, b.height) }
    }

    /// A selection big enough to work in (macOS: wider than 2 points).
    fn usable_selection(&self) -> Option<Rect> {
        self.selection.filter(|s| s.width > 2.0 * self.ui_scale)
    }
}

pub(crate) fn contains(r: Rect, p: Point) -> bool {
    p.x >= r.x && p.x <= r.x + r.width && p.y >= r.y && p.y <= r.y + r.height
}

pub struct Session {
    pub overlays: Vec<Overlay>,
    pub tool: Option<Tool>,
    pub color: Rgb,
    /// Stroke width in points; each monitor multiplies it by its `ui_scale`.
    pub width: f32,
    /// Font size for new text, in points.
    pub font_size: f32,
    /// The monitor that gets the keyboard: the one last clicked, or the one under the pointer at the start.
    pub focused: usize,
    pub palette_open: bool,
    pub font: TextFont,
}

impl Session {
    /// One overlay per `(frozen screen, ui_scale)`.
    #[must_use]
    pub fn new(screens: Vec<(Pixmap, f64)>, font: TextFont) -> Self {
        Self {
            overlays: screens.into_iter().map(|(p, s)| Overlay::new(p, s)).collect(),
            tool: None,
            color: PALETTE[0],
            width: 3.0,
            font_size: 22.0,
            focused: 0,
            palette_open: false,
            font,
        }
    }

    /// The overlay whose selection would be exported.
    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        self.overlays.iter().position(|o| o.selection.is_some_and(|s| s.width >= 1.0 && s.height >= 1.0))
    }

    /// The finished image of the selection, with any text being typed included. `None` without a selection.
    pub fn export(&mut self) -> Option<Pixmap> {
        self.commit_text();
        let i = self.selected()?;
        let o = &self.overlays[i];
        render::render_selection(&o.screen, o.selection?, &o.annotations, &self.font)
    }

    /// A command that needs a selection: beep without one.
    pub(crate) fn request(&self, c: Command) -> Effect {
        if self.selected().is_some() { Effect::command(c) } else { Effect::beep() }
    }

    pub(crate) fn clear_selections_except(&mut self, keep: usize) {
        for (k, o) in self.overlays.iter_mut().enumerate() {
            if k != keep {
                o.selection = None;
            }
        }
    }

    pub fn pointer_down(&mut self, i: usize, p: Point, mods: Mods) -> Effect {
        let _ = mods; // Task 6 uses it for Shift-click on Save
        self.focused = i;
        let p = self.overlays[i].clamp(p);
        self.commit_text();
        let s = self.overlays[i].ui_scale;
        let action = match self.overlays[i].usable_selection() {
            Some(sel) => {
                if self.tool.is_none()
                    && let Some(h) = geo::handle_at(p, sel, 6.0 * s)
                {
                    Down::Resize(h, sel)
                } else if contains(sel, p) {
                    match self.tool {
                        None => Down::Move(sel),
                        Some(Tool::Text) => self.text_index(i, p).map_or(Down::Text, Down::MoveText),
                        Some(tool) => Down::Draw(tool),
                    }
                } else if self.tool.is_none() {
                    Down::Select
                } else {
                    Down::Nothing
                }
            }
            None => Down::Select,
        };
        if matches!(action, Down::Select) {
            self.clear_selections_except(i);
        }
        let (color, width, size) = (self.color, self.width * s as f32, self.font_size * s as f32);
        let o = &mut self.overlays[i];
        o.down = p;
        o.did_drag = false;
        o.drag = match action {
            Down::Select => {
                o.selection = Some(Rect::new(p.x, p.y, 0.0, 0.0));
                Drag::Selecting(p)
            }
            Down::Resize(h, sel) => Drag::Resizing(h, sel),
            Down::Move(sel) => Drag::Moving(p, sel),
            Down::Draw(tool) => {
                o.current = Annotation::begin(tool, p, color, width);
                Drag::Drawing(p)
            }
            Down::MoveText(t) => Drag::MovingText(t, p),
            Down::Text => {
                let origin = Point { x: p.x - 2.0 * s, y: p.y - f64::from(size) * 0.6 };
                o.editing = Some(TextEdit { origin, text: String::new(), size });
                Drag::None
            }
            Down::Nothing => Drag::None,
        };
        Effect::all()
    }

    pub fn pointer_move(&mut self, i: usize, p: Point) -> Effect {
        let o = &mut self.overlays[i];
        let p = o.clamp(p);
        let bounds = o.bounds();
        o.pointer = p;
        if (p.x - o.down.x).hypot(p.y - o.down.y) > 2.0 * o.ui_scale {
            o.did_drag = true;
        }
        match o.drag {
            Drag::None => return Effect::NONE,
            Drag::Selecting(start) => o.selection = Some(geo::rect(start, p)),
            Drag::Moving(start, original) => {
                o.selection = Some(geo::move_within(original, p.x - start.x, p.y - start.y, bounds));
            }
            Drag::Resizing(h, original) => o.selection = Some(geo::resize(original, h, p)),
            Drag::Drawing(start) => {
                if let Some(c) = &mut o.current {
                    c.extend(start, p);
                }
            }
            Drag::MovingText(t, last) => {
                if let Shape::Text { origin, .. } = &mut o.annotations[t].shape {
                    origin.x += p.x - last.x;
                    origin.y += p.y - last.y;
                }
                o.drag = Drag::MovingText(t, p);
            }
        }
        Effect::one(i)
    }

    pub fn pointer_up(&mut self, i: usize, p: Point) -> Effect {
        let o = &mut self.overlays[i];
        o.pointer = o.clamp(p);
        match o.drag {
            Drag::Selecting(_) if !o.did_drag => o.selection = None,
            Drag::Drawing(_) => {
                if let Some(c) = o.current.take()
                    && (o.did_drag || c.is_path())
                {
                    o.annotations.push(c);
                }
            }
            _ => {}
        }
        o.drag = Drag::None;
        Effect::all()
    }

    /// Wheel: font size while typing or over text with the text tool, otherwise the stroke width.
    pub fn scroll(&mut self, i: usize, p: Point, lines: f64) -> Effect {
        if lines.abs() < f64::EPSILON {
            return Effect::NONE;
        }
        let step = lines.signum() as f32;
        let s = self.overlays[i].ui_scale as f32;
        let (min, max) = (8.0 * s, 120.0 * s);
        if let Some(e) = self.overlays[i].editing.as_mut() {
            e.size = (e.size + step * 2.0 * s).clamp(min, max);
            self.font_size = e.size / s;
            return Effect::one(i);
        }
        if self.tool == Some(Tool::Text)
            && let Some(t) = self.text_index(i, p)
        {
            if let Shape::Text { size, .. } = &mut self.overlays[i].annotations[t].shape {
                *size = (*size + step * 2.0 * s).clamp(min, max);
            }
            return Effect::one(i);
        }
        self.width = (self.width + step).clamp(1.0, 20.0);
        Effect::NONE
    }

    pub fn undo(&mut self) -> Effect {
        let i = self.focused;
        if self.overlays[i].annotations.pop().is_some() { Effect::one(i) } else { Effect::beep() }
    }

    /// The text annotation under `p`, topmost first.
    pub(crate) fn text_index(&self, i: usize, p: Point) -> Option<usize> {
        let o = &self.overlays[i];
        let pad = 4.0 * o.ui_scale;
        o.annotations.iter().rposition(|a| match &a.shape {
            Shape::Text { origin, text, size } => {
                let m = self.font.measure(text, *size);
                contains(Rect::new(origin.x - pad, origin.y - pad, m.width + 2.0 * pad, m.height + 2.0 * pad), p)
            }
            _ => false,
        })
    }

    /// Turns any text box into a text annotation (empty boxes vanish).
    pub(crate) fn commit_text(&mut self) {
        let color = self.color;
        for o in &mut self.overlays {
            if let Some(e) = o.editing.take() {
                let text = e.text.trim();
                if !text.is_empty() {
                    let shape = Shape::Text { origin: e.origin, text: text.to_owned(), size: e.size };
                    o.annotations.push(Annotation { shape, color, width: 0.0, marker: false });
                }
            }
        }
    }

    #[must_use]
    pub fn cursor(&self, i: usize) -> CursorKind {
        let o = &self.overlays[i];
        let p = o.pointer;
        if o.editing.is_some() {
            return CursorKind::Text;
        }
        let Some(sel) = o.usable_selection() else { return CursorKind::Crosshair };
        if self.tool.is_none()
            && let Some(h) = geo::handle_at(p, sel, 6.0 * o.ui_scale)
        {
            return match h {
                Handle::Left | Handle::Right => CursorKind::ResizeHorizontal,
                Handle::Top | Handle::Bottom => CursorKind::ResizeVertical,
                Handle::TopLeft | Handle::BottomRight => CursorKind::ResizeDiagonalDown,
                Handle::TopRight | Handle::BottomLeft => CursorKind::ResizeDiagonalUp,
            };
        }
        if contains(sel, p) {
            return match self.tool {
                None => CursorKind::Move,
                Some(Tool::Text) if self.text_index(i, p).is_some() => CursorKind::Move,
                Some(Tool::Text) => CursorKind::Text,
                Some(_) => CursorKind::Crosshair,
            };
        }
        if self.tool.is_none() { CursorKind::Crosshair } else { CursorKind::Arrow }
    }
}
```

In `lib.rs` add `pub mod input;` and `pub mod session;`.

- [ ] **Step 5: Run the tests to see them pass**

Run: `cd windows-linux && cargo test -p snapline-overlay && cargo clippy -p snapline-overlay --all-targets -- -D warnings`
Expected: all pass. `dragging_a_handle_resizes` relies on `handle_at` with a 6 px tolerance at the edge midpoint (110, 35).

- [ ] **Step 6: Commit**

```bash
git add windows-linux
git commit -m "Add the capture session: select, move, resize, draw and undo"
```

---

### Task 5: Keyboard and typing text

**Files:**
- Modify: `windows-linux/crates/snapline-overlay/src/session.rs` (add `key`, `nudge`, `select_whole_screen`, tests)

**Interfaces:**
- Consumes: everything from Task 4.
- Produces: `Session::key(&mut self, key: Key, mods: Mods) -> Effect`, `Session::select_whole_screen() -> Effect`.

Keys, from `OverlayView+Keyboard.swift` with Windows modifiers:

| Key | Does |
|---|---|
| Esc | finish the text being typed; otherwise close the capture |
| Enter | finish the text being typed; otherwise copy (like Lightshot) |
| Ctrl+C / Ctrl+S / Ctrl+Shift+S | copy / save / save as (finish typed text first) |
| Ctrl+Z / Ctrl+A | undo / select the whole screen under focus |
| Arrows | move the selection 1 px; Shift: 10 px; Alt: resize |
| + or =, - | stroke width up, down (1–20) |
| P L A R E M T H | pick that tool, again to drop it; V drops the tool |

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `session.rs`:

```rust
    use crate::input::Key;

    fn ctrl() -> Mods {
        Mods { ctrl: true, ..Mods::default() }
    }

    fn typing(s: &mut Session, text: &str) {
        for c in text.chars() {
            s.key(Key::Char(c), Mods::default());
        }
    }

    /// A selection with the text tool and a text box open inside it.
    fn text_box() -> Session {
        let mut s = selected(1.0);
        s.tool = Some(Tool::Text);
        s.pointer_down(0, pt(30.0, 30.0), Mods::default());
        s.pointer_up(0, pt(30.0, 30.0));
        assert!(s.overlays[0].editing.is_some());
        s
    }

    fn texts(s: &Session) -> Vec<String> {
        s.overlays[0]
            .annotations
            .iter()
            .filter_map(|a| match &a.shape {
                Shape::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn escape_closes_and_enter_copies_only_with_a_selection() {
        let mut s = session(1, 1.0);
        assert_eq!(s.key(Key::Escape, Mods::default()).command, Some(Command::Cancel));
        assert!(s.key(Key::Enter, Mods::default()).beep);
        let mut s = selected(1.0);
        assert_eq!(s.key(Key::Enter, Mods::default()).command, Some(Command::Copy));
    }

    #[test]
    fn ctrl_shortcuts() {
        let mut s = selected(1.0);
        assert_eq!(s.key(Key::Char('c'), ctrl()).command, Some(Command::Copy));
        assert_eq!(s.key(Key::Char('s'), ctrl()).command, Some(Command::Save));
        let shift = Mods { ctrl: true, shift: true, alt: false };
        assert_eq!(s.key(Key::Char('S'), shift).command, Some(Command::SaveAs));
    }

    #[test]
    fn tool_letters_toggle_and_v_drops_the_tool() {
        let mut s = selected(1.0);
        s.key(Key::Char('r'), Mods::default());
        assert_eq!(s.tool, Some(Tool::Rectangle));
        s.key(Key::Char('r'), Mods::default());
        assert_eq!(s.tool, None);
        s.key(Key::Char('p'), Mods::default());
        s.key(Key::Char('v'), Mods::default());
        assert_eq!(s.tool, None);
    }

    #[test]
    fn arrows_move_shift_moves_ten_alt_resizes() {
        let mut s = selected(1.0);
        s.key(Key::Right, Mods::default());
        s.key(Key::Down, Mods { shift: true, ..Mods::default() });
        assert_eq!(s.overlays[0].selection, Some(Rect::new(11.0, 20.0, 100.0, 50.0)));
        s.key(Key::Right, Mods { alt: true, ..Mods::default() });
        assert_eq!(s.overlays[0].selection.unwrap().width, 101.0);
    }

    #[test]
    fn ctrl_a_selects_the_whole_focused_screen() {
        let mut s = selected(1.0);
        s.focused = 1;
        s.key(Key::Char('a'), ctrl());
        assert_eq!(s.overlays[1].selection, Some(Rect::new(0.0, 0.0, 400.0, 300.0)));
        assert_eq!(s.overlays[0].selection, None);
    }

    #[test]
    fn typing_then_enter_adds_text() {
        let mut s = text_box();
        typing(&mut s, "Hix");
        s.key(Key::Backspace, Mods::default());
        typing(&mut s, "!");
        s.key(Key::Enter, Mods::default());
        assert_eq!(texts(&s), ["Hi!"]);
        assert!(s.overlays[0].editing.is_none());
    }

    #[test]
    fn letters_while_typing_are_text_not_tools() {
        let mut s = text_box();
        typing(&mut s, "rv");
        assert_eq!(s.tool, Some(Tool::Text));
        assert_eq!(s.overlays[0].editing.as_ref().unwrap().text, "rv");
    }

    #[test]
    fn an_empty_text_box_leaves_nothing() {
        let mut s = text_box();
        typing(&mut s, "   ");
        s.key(Key::Enter, Mods::default());
        assert!(texts(&s).is_empty());
    }

    // Review Focus 4.
    #[test]
    fn ctrl_c_while_typing_keeps_the_text() {
        let mut s = text_box();
        typing(&mut s, "Hi");
        assert_eq!(s.key(Key::Char('c'), ctrl()).command, Some(Command::Copy));
        assert_eq!(texts(&s), ["Hi"]);
    }

    // Review Focus 4.
    #[test]
    fn escape_while_typing_finishes_text_instead_of_closing() {
        let mut s = text_box();
        typing(&mut s, "Hi");
        assert_eq!(s.key(Key::Escape, Mods::default()).command, None);
        assert_eq!(texts(&s), ["Hi"]);
        assert_eq!(s.key(Key::Escape, Mods::default()).command, Some(Command::Cancel));
    }

    // Review Focus 5: AltGr arrives as Ctrl+Alt on Windows.
    #[test]
    fn altgr_characters_are_typed_not_shortcuts() {
        let mut s = text_box();
        let altgr = Mods { ctrl: true, alt: true, shift: false };
        for c in ['@', '€', '{', 'c'] {
            assert_eq!(s.key(Key::Char(c), altgr).command, None);
        }
        assert_eq!(s.overlays[0].editing.as_ref().unwrap().text, "@€{c");
    }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cd windows-linux && cargo test -p snapline-overlay session`
Expected: compile errors, no method `key`.

- [ ] **Step 3: Write the keyboard handling**

In `session.rs`, add `use crate::input::Key;` at the top, then add to `impl Session`:

```rust
    /// Ctrl without Alt is a shortcut. Ctrl+Alt is AltGr on Windows, which types characters.
    fn is_shortcut(mods: Mods) -> bool {
        mods.ctrl && !mods.alt
    }

    pub fn key(&mut self, key: Key, mods: Mods) -> Effect {
        if let Some(i) = self.overlays.iter().position(|o| o.editing.is_some()) {
            match key {
                Key::Enter | Key::Escape => {
                    self.commit_text();
                    return Effect::one(i);
                }
                Key::Backspace => {
                    if let Some(e) = self.overlays[i].editing.as_mut() {
                        e.text.pop();
                    }
                    return Effect::one(i);
                }
                Key::Char(c) if !Self::is_shortcut(mods) && !c.is_control() => {
                    if let Some(e) = self.overlays[i].editing.as_mut() {
                        e.text.push(c);
                    }
                    return Effect::one(i);
                }
                // A shortcut finishes the text first, so Ctrl+C copies what was typed.
                _ if Self::is_shortcut(mods) => self.commit_text(),
                _ => return Effect::NONE,
            }
        }
        match key {
            Key::Escape => Effect::command(Command::Cancel),
            Key::Enter => self.request(Command::Copy),
            Key::Left => self.nudge(-1.0, 0.0, mods),
            Key::Right => self.nudge(1.0, 0.0, mods),
            Key::Up => self.nudge(0.0, -1.0, mods),
            Key::Down => self.nudge(0.0, 1.0, mods),
            Key::Char(c) if Self::is_shortcut(mods) => match c.to_ascii_lowercase() {
                'c' => self.request(Command::Copy),
                's' => self.request(if mods.shift { Command::SaveAs } else { Command::Save }),
                'z' => self.undo(),
                'a' => self.select_whole_screen(),
                _ => Effect::NONE,
            },
            Key::Char('+' | '=') => {
                self.width = (self.width + 1.0).min(20.0);
                Effect::NONE
            }
            Key::Char('-') => {
                self.width = (self.width - 1.0).max(1.0);
                Effect::NONE
            }
            Key::Char('v' | 'V') => {
                self.tool = None;
                Effect::all()
            }
            Key::Char(c) => match Tool::for_key(c) {
                Some(t) => {
                    self.tool = if self.tool == Some(t) { None } else { Some(t) };
                    Effect::all()
                }
                None => Effect::NONE,
            },
            Key::Backspace => Effect::NONE,
        }
    }

    /// Arrows move the selection; Alt resizes it; Shift uses 10-point steps.
    fn nudge(&mut self, dx: f64, dy: f64, mods: Mods) -> Effect {
        let i = self.focused;
        let o = &mut self.overlays[i];
        let Some(mut sel) = o.selection else { return Effect::NONE };
        let step = if mods.shift { 10.0 } else { 1.0 } * o.ui_scale;
        let b = o.bounds();
        if mods.alt {
            sel.width = (sel.width + dx * step).max(1.0).min(b.width - sel.x);
            sel.height = (sel.height + dy * step).max(1.0).min(b.height - sel.y);
        } else {
            sel = geo::move_within(sel, dx * step, dy * step, b);
        }
        o.selection = Some(sel);
        Effect::one(i)
    }

    pub fn select_whole_screen(&mut self) -> Effect {
        let i = self.focused;
        self.clear_selections_except(i);
        self.overlays[i].selection = Some(self.overlays[i].bounds());
        Effect::all()
    }
```

Note on `arrows_move_shift_moves_ten_alt_resizes`: the selection starts on screen 0 and `focused` is 0 after the drag in `selected()`.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cd windows-linux && cargo test -p snapline-overlay && cargo clippy -p snapline-overlay --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add windows-linux
git commit -m "Handle the overlay keyboard shortcuts and typing text"
```

---

### Task 6: Toolbars and the colour palette

**Files:**
- Create: `windows-linux/crates/snapline-overlay/src/toolbar.rs`
- Modify: `windows-linux/crates/snapline-overlay/src/session.rs` (`hover` field, `bars`, `press`, `hover`, toolbar check in `pointer_down`, `pointer_move` and `cursor`)
- Modify: `windows-linux/crates/snapline-overlay/src/lib.rs` (add `pub mod toolbar;`)

**Interfaces:**
- Consumes: `Tool`, `PALETTE`, `Rect`, `Point`, `Size`, Task 4's `Session` and `Effect`.
- Produces: `toolbar::Button { Tool(Tool), Color, Undo, Copy, Save, Close, Swatch(usize) }` with `tip() -> &'static str`; `toolbar::DRAW_BAR: [Button; 10]`, `toolbar::MAIN_BAR: [Button; 3]`; `toolbar::Bar { frame: Rect, buttons: Vec<(Button, Rect)> }`; `toolbar::layout(sel, bounds, scale, palette_open) -> Vec<Bar>` (order: main bar, draw bar, then the palette when open); `toolbar::hit(&[Bar], Point) -> Option<Button>`; `Overlay.hover: Option<Button>`; `Session::bars(i) -> Vec<Bar>`.

Layout mirrors `layoutToolbars()` in `OverlayView+Toolbars.swift`: buttons 30×28 points, 4-point padding, 2-point spacing, 8-point gap from the selection. The main bar sits under the selection, right-aligned; the draw bar sits to its right, top-aligned. Each flips to the other side, then inside the selection, when there is no room.

- [ ] **Step 1: Write the failing tests**

`src/toolbar.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect::new(0.0, 0.0, 1000.0, 800.0);

    fn inside(r: Rect, outer: Rect) -> bool {
        r.x >= outer.x && r.y >= outer.y && r.x + r.width <= outer.x + outer.width && r.y + r.height <= outer.y + outer.height
    }

    #[test]
    fn main_bar_sits_under_the_selection_right_aligned() {
        let bars = layout(Rect::new(100.0, 100.0, 200.0, 100.0), SCREEN, 1.0, false);
        let main = bars[0].frame;
        assert_eq!(main.x + main.width, 300.0);
        assert_eq!(main.y, 208.0);
    }

    #[test]
    fn main_bar_flips_above_at_the_bottom_edge() {
        let bars = layout(Rect::new(100.0, 700.0, 200.0, 100.0), SCREEN, 1.0, false);
        let main = bars[0].frame;
        assert_eq!(main.y + main.height, 692.0);
    }

    #[test]
    fn draw_bar_flips_left_at_the_right_edge() {
        let bars = layout(Rect::new(900.0, 100.0, 100.0, 100.0), SCREEN, 1.0, false);
        let draw = bars[1].frame;
        assert_eq!(draw.x + draw.width, 892.0);
    }

    // Review Focus 2.
    #[test]
    fn full_screen_selection_keeps_toolbars_on_screen() {
        for scale in [1.0, 1.5, 2.0] {
            for bar in layout(SCREEN, SCREEN, scale, true) {
                assert!(inside(bar.frame, SCREEN), "{:?} at scale {scale}", bar.frame);
            }
        }
        // A screen smaller than a bar must not panic.
        let tiny = Rect::new(0.0, 0.0, 50.0, 40.0);
        let _ = layout(tiny, tiny, 1.0, true);
    }

    #[test]
    fn scale_doubles_the_bars() {
        let one = layout(Rect::new(100.0, 100.0, 200.0, 100.0), SCREEN, 1.0, false)[1].frame;
        let two = layout(Rect::new(100.0, 100.0, 200.0, 100.0), SCREEN, 2.0, false)[1].frame;
        assert_eq!(two.width, one.width * 2.0);
    }

    #[test]
    fn hit_finds_the_button_under_the_pointer() {
        let bars = layout(Rect::new(100.0, 100.0, 200.0, 100.0), SCREEN, 1.0, true);
        let (button, r) = bars[1].buttons[0];
        assert_eq!(button, Button::Tool(Tool::Pencil));
        assert_eq!(hit(&bars, Point { x: r.x + 5.0, y: r.y + 5.0 }), Some(Button::Tool(Tool::Pencil)));
        assert_eq!(hit(&bars, Point { x: 150.0, y: 150.0 }), None);
        assert_eq!(bars.len(), 3, "the palette is the third bar when open");
    }
}
```

Add to `mod tests` in `session.rs`:

```rust
    use crate::toolbar::{self, Button};

    fn click_button(s: &mut Session, button: Button, mods: Mods) -> Effect {
        let bars = s.bars(0);
        let r = bars.iter().flat_map(|b| b.buttons.iter()).find(|(b, _)| *b == button).unwrap().1;
        let p = pt(r.x + r.width / 2.0, r.y + r.height / 2.0);
        let effect = s.pointer_down(0, p, mods);
        s.pointer_up(0, p);
        effect
    }

    #[test]
    fn toolbar_buttons_work_and_do_not_touch_the_selection() {
        let mut s = selected(1.0);
        let before = s.overlays[0].selection;
        click_button(&mut s, Button::Tool(Tool::Pencil), Mods::default());
        assert_eq!(s.tool, Some(Tool::Pencil));
        assert_eq!(s.overlays[0].selection, before);
        assert_eq!(click_button(&mut s, Button::Copy, Mods::default()).command, Some(Command::Copy));
        let shift = Mods { shift: true, ..Mods::default() };
        assert_eq!(click_button(&mut s, Button::Save, shift).command, Some(Command::SaveAs));
        assert_eq!(click_button(&mut s, Button::Close, Mods::default()).command, Some(Command::Cancel));
    }

    #[test]
    fn the_palette_opens_and_picks_a_colour() {
        let mut s = selected(1.0);
        click_button(&mut s, Button::Color, Mods::default());
        assert!(s.palette_open);
        click_button(&mut s, Button::Swatch(4), Mods::default());
        assert_eq!(s.color, crate::color::PALETTE[4]);
        assert!(!s.palette_open);
    }

    #[test]
    fn toolbars_hide_while_dragging() {
        let mut s = selected(1.0);
        assert_eq!(s.bars(0).len(), 2);
        s.pointer_down(0, pt(50.0, 30.0), Mods::default());
        assert!(s.bars(0).is_empty());
        s.pointer_up(0, pt(50.0, 30.0));
        assert_eq!(s.bars(0)[0].buttons.len(), toolbar::MAIN_BAR.len());
    }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cd windows-linux && cargo test -p snapline-overlay`
Expected: compile errors, `toolbar` items and `Session::bars` not found.

- [ ] **Step 3: Write the toolbar**

Above the tests in `src/toolbar.rs`:

```rust
//! The two toolbars next to the selection, and the colour palette. Layout and hit testing only;
//! `render` draws them.

use crate::annotation::Tool;
use crate::color::PALETTE;
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
            Self::Swatch(_) => "",
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

pub const MAIN_BAR: [Button; 3] = [Button::Copy, Button::Save, Button::Close];

const BUTTON: Size = Size { width: 30.0, height: 28.0 };
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
    let long = 2.0 * PAD + n * if horizontal { BUTTON.width } else { BUTTON.height } + (n - 1.0) * SPACING;
    if horizontal {
        Size { width: long * s, height: (2.0 * PAD + BUTTON.height) * s }
    } else {
        Size { width: (2.0 * PAD + BUTTON.width) * s, height: long * s }
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
    Bar { frame: Rect::new(origin.x, origin.y, size.width, size.height), buttons }
}

/// Keeps `v` in `0..=limit`, even when the bar is larger than the screen.
fn fit(v: f64, limit: f64) -> f64 {
    v.min(limit).max(0.0)
}

/// Where the bars go for a selection, in pixels: main bar, draw bar, then the palette when it is open.
#[must_use]
pub fn layout(sel: Rect, bounds: Rect, s: f64, palette_open: bool) -> Vec<Bar> {
    let gap = GAP * s;
    let (right, bottom) = (bounds.x + bounds.width, bounds.y + bounds.height);
    let (sel_right, sel_bottom) = (sel.x + sel.width, sel.y + sel.height);

    let main = bar_size(MAIN_BAR.len(), true, s);
    let mut m = Point { x: sel_right - main.width, y: sel_bottom + gap };
    if m.y + main.height > bottom {
        m.y = if sel.y - main.height - gap >= 0.0 { sel.y - main.height - gap } else { sel_bottom - main.height - gap };
    }
    m.x = fit(m.x, right - main.width);
    m.y = fit(m.y, bottom - main.height);

    let draw = bar_size(DRAW_BAR.len(), false, s);
    let mut d = Point { x: sel_right + gap, y: sel.y };
    if d.x + draw.width > right {
        d.x = if sel.x - draw.width - gap >= 0.0 { sel.x - draw.width - gap } else { sel_right - draw.width - gap };
    }
    d.x = fit(d.x, right - draw.width);
    d.y = fit(d.y, bottom - draw.height);

    let mut bars = vec![bar(&MAIN_BAR, true, m, s), bar(&DRAW_BAR, false, d, s)];
    if palette_open {
        let swatches: Vec<Button> = (0..PALETTE.len()).map(Button::Swatch).collect();
        let size = bar_size(swatches.len(), true, s);
        let color_y = bars[1].buttons.iter().find(|(b, _)| *b == Button::Color).map_or(d.y, |(_, r)| r.y - PAD * s);
        let mut x = d.x - size.width - gap;
        if x < 0.0 {
            x = d.x + draw.width + gap;
        }
        let p = Point { x: fit(x, right - size.width), y: fit(color_y, bottom - size.height) };
        bars.push(bar(&swatches, true, p, s));
    }
    bars
}

/// The button under `p`, if any.
#[must_use]
pub fn hit(bars: &[Bar], p: Point) -> Option<Button> {
    bars.iter().flat_map(|b| b.buttons.iter()).find(|(_, r)| crate::session::contains(*r, p)).map(|(b, _)| *b)
}
```

- [ ] **Step 4: Connect the toolbar to the session**

In `session.rs`:

1. `use crate::toolbar::{self, Bar, Button};`
2. Add `pub hover: Option<Button>,` to `Overlay` and `hover: None,` in `Overlay::new`.
3. Add to `impl Session`:

```rust
    /// The toolbars of overlay `i`: shown for a usable selection while nothing is being dragged.
    #[must_use]
    pub fn bars(&self, i: usize) -> Vec<Bar> {
        let o = &self.overlays[i];
        match o.selection {
            Some(sel) if sel.width > 2.0 * o.ui_scale && sel.height > 2.0 * o.ui_scale && !o.is_dragging() => {
                toolbar::layout(sel, o.bounds(), o.ui_scale, self.palette_open)
            }
            _ => Vec::new(),
        }
    }

    fn press(&mut self, button: Button, mods: Mods) -> Effect {
        if !matches!(button, Button::Color | Button::Swatch(_)) {
            self.palette_open = false;
        }
        match button {
            Button::Tool(t) => {
                self.commit_text();
                self.tool = if self.tool == Some(t) { None } else { Some(t) };
                Effect::all()
            }
            Button::Color => {
                self.palette_open = !self.palette_open;
                Effect::all()
            }
            Button::Swatch(k) => {
                self.color = PALETTE[k];
                self.palette_open = false;
                Effect::all()
            }
            Button::Undo => self.undo(),
            Button::Copy => self.request(Command::Copy),
            Button::Save => self.request(if mods.shift { Command::SaveAs } else { Command::Save }),
            Button::Close => Effect::command(Command::Cancel),
        }
    }

    /// Redraws only when the pointer moves onto or off a button (for the tooltip and highlight).
    fn hover(&mut self, i: usize, p: Point) -> Effect {
        let hit = toolbar::hit(&self.bars(i), p);
        let o = &mut self.overlays[i];
        if o.hover == hit {
            Effect::NONE
        } else {
            o.hover = hit;
            Effect::one(i)
        }
    }
```

4. In `pointer_down`, replace `let _ = mods; // Task 6 …` and the clamp line with:

```rust
        self.focused = i;
        let p = self.overlays[i].clamp(p);
        if let Some(button) = toolbar::hit(&self.bars(i), p) {
            return self.press(button, mods);
        }
        self.palette_open = false;
```

5. In `pointer_move`, change `Drag::None => return Effect::NONE,` to `Drag::None => return self.hover(i, p),`.
6. In `cursor`, after the `editing` check, add:

```rust
        if toolbar::hit(&self.bars(i), p).is_some() {
            return CursorKind::Arrow;
        }
```

In `lib.rs` add `pub mod toolbar;`.

- [ ] **Step 5: Run the tests to see them pass**

Run: `cd windows-linux && cargo test -p snapline-overlay && cargo clippy -p snapline-overlay --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add windows-linux
git commit -m "Lay out the overlay toolbars and the colour palette"
```

---

### Task 7: Icons and the on-screen overlay

**Files:**
- Create: `windows-linux/crates/snapline-overlay/src/icons.rs`
- Modify: `windows-linux/crates/snapline-overlay/src/render.rs` (add `render_screen` and helpers)
- Modify: `windows-linux/crates/snapline-overlay/src/lib.rs` (add `pub mod icons;`)

**Interfaces:**
- Consumes: `Session`, `Overlay`, `toolbar::{Bar, Button}`, `TextFont`, colours, Task 3 helpers.
- Produces: `icons::draw(pixmap, Button, frame: Rect, ink: Rgb, current: Rgb)`; `render::render_screen(out: &mut Pixmap, session: &Session, i: usize)` (`out` must be the size of `session.overlays[i].screen`).

What the screen shows, from `OverlayView+Drawing.swift` (point sizes × `ui_scale`):
1. The frozen screen, the annotations, the shape being drawn, the text being typed with a 2-point caret.
2. Black at 45 % over everything outside the selection.
3. No selection: on the focused monitor only, a centred badge "Drag to select an area   ·   Ctrl+A whole screen   ·   Esc to cancel".
4. Selection frame: violet 3-point line at 55 % just outside, a cyan 1.5-point line on the edge.
5. With no tool: eight white handles, radius 4.5, violet 2-point ring.
6. A violet badge with the size in pixels ("640 × 480") above the top-left corner (or inside at the top).
7. The bars: ink background at 92 %, corner radius 8; a light highlight under the hovered button; the active tool's icon in violet light; the tooltip of the hovered button as a badge under the bar (above it when there is no room).

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `render.rs`:

```rust
    use crate::input::Mods;
    use crate::session::Session;

    fn screen_session(n: usize) -> Session {
        Session::new((0..n).map(|_| (white(400, 300), 1.0)).collect(), TextFont::bundled())
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
        assert!((130..=150).contains(&r), "outside should be dimmed to about 140, was {r}");
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
        assert_eq!(rgb(&rendered(&s, 0), 250, 140), (255, 255, 255), "right handle");
        s.tool = Some(crate::annotation::Tool::Pencil);
        assert_ne!(rgb(&rendered(&s, 0), 250, 140), (255, 255, 255));
    }

    #[test]
    fn toolbars_are_drawn_next_to_the_selection() {
        let mut s = screen_session(1);
        select(&mut s, Rect::new(100.0, 100.0, 150.0, 80.0));
        let main = s.bars(0)[0].frame;
        // Top edge, middle: inside the bar's padding, clear of the rounded corners and the icons.
        let (r, g, b) = rgb(&rendered(&s, 0), (main.x + main.width / 2.0) as u32, (main.y + 2.0) as u32);
        assert!(r < 60 && g < 60 && b < 60, "bar background should be dark ink");
    }

    #[test]
    fn rendering_at_the_edges_and_at_scale_two_does_not_panic() {
        let mut s = Session::new(vec![(white(400, 300), 2.0)], TextFont::bundled());
        s.select_whole_screen();
        s.palette_open = true;
        let mut out = Pixmap::new(400, 300).unwrap();
        render_screen(&mut out, &s, 0);
    }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cd windows-linux && cargo test -p snapline-overlay render`
Expected: compile errors, `render_screen` not found.

- [ ] **Step 3: Write the icons**

`src/icons.rs`. Icons are drawn on a 24-unit grid, 2 units wide with round ends, scaled into the button:

```rust
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
            pixmap.stroke_path(&path, &paint(ink, alpha), &stroke(width, LineCap::Round), t, None);
        }
    };
    let dot = |pixmap: &mut Pixmap, color: Rgb, r: f32| {
        if let Some(path) = PathBuilder::from_circle(12.0, 12.0, r) {
            pixmap.fill_path(&path, &paint(color, 255), FillRule::Winding, t, None);
            pixmap.stroke_path(&path, &paint(WHITE, 255), &stroke(1.5, LineCap::Round), t, None);
        }
    };
    let mut pb = PathBuilder::new();
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
            pb.move_to(6.0, 16.0);
            pb.line_to(18.0, 6.0);
            line(pixmap, pb, 6.0, 150);
            let mut base = PathBuilder::new();
            base.move_to(4.0, 21.0);
            base.line_to(20.0, 21.0);
            return line(pixmap, base, 2.0, 255);
        }
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
    }
    line(pixmap, pb, 2.0, 255);
}
```

- [ ] **Step 4: Write `render_screen`**

Add to `render.rs` (imports: `crate::color::{CYAN, INK, VIOLET, VIOLET_LIGHT, WHITE}`, `crate::icons`, `crate::session::Session`, `crate::toolbar::Bar`, `snapline_core::selection_geometry::{self as geo, Handle}`):

```rust
const HINT: &str = "Drag to select an area   ·   Ctrl+A whole screen   ·   Esc to cancel";

fn rounded(r: Rect, radius: f32) -> Option<tiny_skia::Path> {
    let (x, y, w, h) = (r.x as f32, r.y as f32, r.width as f32, r.height as f32);
    let k = radius.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + k, y);
    pb.line_to(x + w - k, y);
    pb.quad_to(x + w, y, x + w, y + k);
    pb.line_to(x + w, y + h - k);
    pb.quad_to(x + w, y + h, x + w - k, y + h);
    pb.line_to(x + k, y + h);
    pb.quad_to(x, y + h, x, y + h - k);
    pb.line_to(x, y + k);
    pb.quad_to(x, y, x + k, y);
    pb.close();
    pb.finish()
}

fn fill_rounded(pixmap: &mut Pixmap, r: Rect, radius: f32, c: Rgb, alpha: u8) {
    if let Some(path) = rounded(r, radius) {
        pixmap.fill_path(&path, &paint(c, alpha), FillRule::Winding, Transform::identity(), None);
    }
}

fn stroke_rect(pixmap: &mut Pixmap, r: Rect, c: Rgb, alpha: u8, width: f32) {
    if let Some(path) = sk_rect(r).map(PathBuilder::from_rect) {
        pixmap.stroke_path(&path, &paint(c, alpha), &stroke(width, LineCap::Butt), Transform::identity(), None);
    }
}

fn grow(r: Rect, by: f64) -> Rect {
    Rect::new(r.x - by, r.y - by, r.width + 2.0 * by, r.height + 2.0 * by)
}

/// A pill-shaped label. `at` is its top-left corner, or its centre when `centered`.
fn badge(pixmap: &mut Pixmap, font: &TextFont, text: &str, at: Point, centered: bool, fill: Rgb, s: f64) {
    let size = (12.0 * s) as f32;
    let m = font.measure(text, size);
    let mut r = Rect::new(at.x, at.y, m.width + 16.0 * s, m.height + 6.0 * s);
    if centered {
        r.x -= r.width / 2.0;
        r.y -= r.height / 2.0;
    }
    fill_rounded(pixmap, r, (r.height / 2.0) as f32, fill, 225);
    font.draw(pixmap, text, Point { x: r.x + 8.0 * s, y: r.y + 3.0 * s }, size, WHITE);
}

fn draw_bars(out: &mut Pixmap, session: &Session, i: usize, bars: &[Bar]) {
    let o = &session.overlays[i];
    let s = o.ui_scale;
    for bar in bars {
        fill_rounded(out, bar.frame, (8.0 * s) as f32, INK, 235);
        for (button, r) in &bar.buttons {
            if o.hover == Some(*button) {
                fill_rounded(out, *r, (5.0 * s) as f32, WHITE, 40);
            }
            let active = matches!(button, crate::toolbar::Button::Tool(t) if session.tool == Some(*t));
            icons::draw(out, *button, *r, if active { VIOLET_LIGHT } else { WHITE }, session.color);
        }
    }
    let Some(hover) = o.hover else { return };
    let tip = hover.tip();
    let Some(bar) = bars.iter().find(|b| b.buttons.iter().any(|(b, _)| *b == hover)) else { return };
    if tip.is_empty() {
        return;
    }
    let below = bar.frame.y + bar.frame.height + 4.0 * s;
    let height = 24.0 * s;
    let y = if below + height <= o.bounds().height { below } else { bar.frame.y - height - 4.0 * s };
    badge(out, &session.font, tip, Point { x: bar.frame.x, y: y.max(0.0) }, false, INK, s);
}

/// Draws what monitor `i` shows during a capture into `out` (same size as that monitor's screen).
pub fn render_screen(out: &mut Pixmap, session: &Session, i: usize) {
    let o = &session.overlays[i];
    let s = o.ui_scale;
    out.data_mut().copy_from_slice(o.screen.data());
    let origin = Point::default();
    for a in o.annotations.iter().chain(o.current.as_ref()) {
        draw_annotation(out, a, &session.font, origin);
    }
    if let Some(e) = &o.editing {
        session.font.draw(out, &e.text, e.origin, e.size, session.color);
        let w = session.font.measure(&e.text, e.size).width;
        fill_rect(out, Rect::new(e.origin.x + w + s, e.origin.y, 2.0 * s, f64::from(e.size) * 1.2), session.color, 255);
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
        out.fill_path(&path, &paint(BLACK, 115), FillRule::EvenOdd, Transform::identity(), None);
    }

    let Some(sel) = o.selection else {
        if i == session.focused {
            let centre = Point { x: bounds.width / 2.0, y: bounds.height / 2.0 };
            badge(out, &session.font, HINT, centre, true, INK, s);
        }
        return;
    };
    stroke_rect(out, grow(sel, 2.0 * s), VIOLET, 140, (3.0 * s) as f32);
    stroke_rect(out, grow(sel, 0.75 * s), CYAN, 255, (1.5 * s) as f32);
    if session.tool.is_none() && sel.width > 2.0 * s {
        for h in Handle::ALL {
            let c = geo::handle_point(h, sel);
            if let Some(path) = PathBuilder::from_circle(c.x as f32, c.y as f32, (4.5 * s) as f32) {
                out.fill_path(&path, &paint(WHITE, 255), FillRule::Winding, Transform::identity(), None);
                out.stroke_path(&path, &paint(VIOLET, 255), &stroke((2.0 * s) as f32, LineCap::Round), Transform::identity(), None);
            }
        }
    }
    let label = format!("{} × {}", sel.width.round() as i64, sel.height.round() as i64);
    badge(out, &session.font, &label, Point { x: sel.x, y: (sel.y - 26.0 * s).max(2.0 * s) }, false, VIOLET, s);
    draw_bars(out, session, i, &session.bars(i));
}
```

In `lib.rs` add `pub mod icons;`.

- [ ] **Step 5: Run the tests to see them pass**

Run: `cd windows-linux && cargo test -p snapline-overlay && cargo clippy -p snapline-overlay --all-targets -- -D warnings`
Expected: all pass. If `handles_show_only_without_a_tool` fails by a pixel, check `handle_point` for `Right` is `(sel.max_x, sel.mid_y)` = (250, 140).

- [ ] **Step 6: Look at it**

Add a throwaway test (do not commit it) that writes `rendered(&s, 0).save_png("/tmp/overlay.png")` with a selection, a rectangle, a text and the palette open, and open the PNG. Check that the bars, icons, badge and tooltip look right. Delete the test.

- [ ] **Step 7: Commit**

```bash
git add windows-linux
git commit -m "Draw the overlay on screen: dimming, frame, handles, size and toolbars"
```

---

### Task 8: The app crate — settings and actions

**Files:**
- Create: `windows-linux/crates/snapline/Cargo.toml`
- Create: `windows-linux/crates/snapline/src/main.rs` (a stub for now: `fn main() {}` plus `mod settings; mod actions;`)
- Create: `windows-linux/crates/snapline/src/settings.rs`
- Create: `windows-linux/crates/snapline/src/actions.rs`

**Interfaces:**
- Consumes: `snapline_core::naming::{file_name, unique_file_path}`, `snapline_overlay::{session::Command, tiny_skia::Pixmap}`.
- Produces: `settings::{Settings { save_directory: PathBuf, hotkeys: Vec<String>, tray_click: TrayClick }, TrayClick { Menu, Capture }, shortcut_label(&str) -> String}`, `Settings::path() -> Option<PathBuf>`, `Settings::load(&Path) -> (Settings, Option<String>)`; `actions::run(Command, &Pixmap, &Settings) -> Result<(), String>`, `actions::save(png: &[u8], dir, name) -> io::Result<PathBuf>`.

Settings file, `%APPDATA%\Snapline\settings.toml`. Snapline never writes it in Phase 2 (there is no Preferences window yet); a missing file means defaults:

```toml
save_directory = 'C:\Users\you\Desktop'
hotkeys = ["PrintScreen", "Ctrl+Shift+Digit1"]   # global-hotkey names
tray_click = "menu"                              # or "capture"
```

- [ ] **Step 1: Create the crate and add dependencies**

`windows-linux/crates/snapline/Cargo.toml`:

```toml
[package]
name = "snapline"
version = "0.1.0"
edition.workspace = true
license-file.workspace = true
publish.workspace = true

[lints]
workspace = true

[dependencies]
snapline-core = { path = "../snapline-core" }
snapline-overlay = { path = "../snapline-overlay" }
```

Run:

```bash
cd windows-linux
cargo add -p snapline serde --features derive
cargo add -p snapline toml dirs chrono arboard rfd winit softbuffer xcap tray-icon global-hotkey
cargo add -p snapline windows-sys --target 'cfg(windows)' --features Win32_Foundation,Win32_Security,Win32_System_Threading,Win32_UI_WindowsAndMessaging
```

Check that `winit` came in as 0.30.x. If `cargo add` picked 0.31 (beta), run `cargo add -p snapline winit@0.30`.

- [ ] **Step 2: Write the failing tests**

`src/settings.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, contents: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("snapline-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.toml");
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn defaults_listen_to_prtscn_and_ctrl_shift_1() {
        let s = Settings::default();
        assert_eq!(s.hotkeys, ["PrintScreen", "Ctrl+Shift+Digit1"]);
        assert_eq!(s.tray_click, TrayClick::Menu);
        for key in &s.hotkeys {
            assert!(key.parse::<global_hotkey::hotkey::HotKey>().is_ok(), "{key} must parse");
        }
    }

    #[test]
    fn missing_file_gives_defaults_without_complaint() {
        let (s, problem) = Settings::load(Path::new("/no/such/snapline/settings.toml"));
        assert_eq!(s, Settings::default());
        assert!(problem.is_none());
    }

    #[test]
    fn a_partial_file_keeps_defaults_for_the_rest() {
        let path = temp_file("partial", "tray_click = \"capture\"\n");
        let (s, problem) = Settings::load(&path);
        assert!(problem.is_none());
        assert_eq!(s.tray_click, TrayClick::Capture);
        assert_eq!(s.hotkeys, Settings::default().hotkeys);
    }

    // Review Focus 3.
    #[test]
    fn broken_settings_file_gives_defaults_and_a_reason() {
        let text = "hotkeys = [\"PrintScreen\"\n";
        let path = temp_file("broken", text);
        let (s, problem) = Settings::load(&path);
        assert_eq!(s, Settings::default());
        assert!(problem.unwrap().contains("settings.toml"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "the file must be left as it was");
    }

    #[test]
    fn shortcut_labels_read_the_windows_way() {
        assert_eq!(shortcut_label("PrintScreen"), "PrtScn");
        assert_eq!(shortcut_label("Ctrl+Shift+Digit1"), "Ctrl+Shift+1");
        assert_eq!(shortcut_label("Alt+KeyQ"), "Alt+Q");
    }
}
```

`src/actions.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("snapline-save-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let first = save(b"one", &dir, "Shot.png").unwrap();
        let second = save(b"two", &dir, "Shot.png").unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(&first).unwrap(), b"one");
        assert_eq!(std::fs::read(&second).unwrap(), b"two");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
```

`src/main.rs` (stub until Task 9):

```rust
mod actions;
mod settings;

fn main() {}
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cd windows-linux && cargo test -p snapline`
Expected: compile errors, `Settings` and `save` not found.

- [ ] **Step 4: Write settings and actions**

Above the tests in `src/settings.rs`:

```rust
//! Settings from `settings.toml` in the user's config folder (`%APPDATA%\Snapline\` on Windows).
//! Phase 2 only reads it; the Preferences window (Phase 5) will write it.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrayClick {
    /// Clicking the tray icon opens the menu (the macOS default too).
    #[default]
    Menu,
    /// Clicking the tray icon takes a screenshot, like Lightshot.
    Capture,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Where Ctrl+S saves.
    pub save_directory: PathBuf,
    /// Capture shortcuts, as global-hotkey names ("PrintScreen", "Ctrl+Shift+Digit1").
    pub hotkeys: Vec<String>,
    pub tray_click: TrayClick,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            save_directory: dirs::desktop_dir().or_else(dirs::home_dir).unwrap_or_default(),
            hotkeys: vec!["PrintScreen".into(), "Ctrl+Shift+Digit1".into()],
            tray_click: TrayClick::Menu,
        }
    }
}

impl Settings {
    #[must_use]
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("Snapline").join("settings.toml"))
    }

    /// Reads `path`. A missing file gives the defaults. A file that cannot be read gives the defaults and
    /// the reason, and is left untouched so the user can fix it.
    #[must_use]
    pub fn load(path: &Path) -> (Self, Option<String>) {
        match std::fs::read_to_string(path) {
            Ok(text) => match toml::from_str(&text) {
                Ok(settings) => (settings, None),
                Err(e) => (Self::default(), Some(format!("{}:\n{e}", path.display()))),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Self::default(), None),
            Err(e) => (Self::default(), Some(format!("{}: {e}", path.display()))),
        }
    }
}

/// How a shortcut name shows in the tray menu: "Ctrl+Shift+Digit1" → "Ctrl+Shift+1", "PrintScreen" → "PrtScn".
#[must_use]
pub fn shortcut_label(name: &str) -> String {
    name.replace("PrintScreen", "PrtScn").replace("Digit", "").replace("Key", "")
}
```

Above the tests in `src/actions.rs`:

```rust
//! What happens to the finished screenshot: copy, save, save as.

use crate::settings::Settings;
use chrono::Local;
use snapline_core::naming;
use snapline_overlay::session::Command;
use snapline_overlay::tiny_skia::Pixmap;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// Runs `command` on the image. The error is a sentence to show the user.
pub fn run(command: Command, image: &Pixmap, settings: &Settings) -> Result<(), String> {
    let name = naming::file_name(Local::now());
    match command {
        Command::Copy => copy(image).map_err(|e| format!("Snapline could not copy the screenshot: {e}")),
        Command::Save => {
            let png = png(image)?;
            save(&png, &settings.save_directory, &name)
                .map(|_| ())
                .map_err(|e| format!("Snapline could not save to {}: {e}", settings.save_directory.display()))
        }
        Command::SaveAs => {
            let png = png(image)?;
            let Some(path) = rfd::FileDialog::new()
                .set_directory(&settings.save_directory)
                .set_file_name(&name)
                .add_filter("PNG image", &["png"])
                .save_file()
            else {
                return Ok(()); // cancelled
            };
            std::fs::write(&path, png).map_err(|e| format!("Snapline could not save {}: {e}", path.display()))
        }
        Command::Cancel => Ok(()),
    }
}

fn png(image: &Pixmap) -> Result<Vec<u8>, String> {
    image.encode_png().map_err(|e| format!("Snapline could not make the PNG: {e}"))
}

/// The screenshot is opaque, so tiny-skia's premultiplied bytes are already plain RGBA.
fn copy(image: &Pixmap) -> Result<(), arboard::Error> {
    arboard::Clipboard::new()?.set_image(arboard::ImageData {
        width: image.width() as usize,
        height: image.height() as usize,
        bytes: Cow::Borrowed(image.data()),
    })
}

/// Writes `png` into `dir` as `name`, adding " 2", " 3"… instead of overwriting.
pub fn save(png: &[u8], dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = naming::unique_file_path(dir, name, Path::exists);
    std::fs::write(&path, png)?;
    Ok(path)
}
```

Add `#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]` at the top of `main.rs` with the same comment as the overlay crate. Until Task 9 uses them, add `#[allow(dead_code)]` on the two modules in `main.rs`; Task 9 removes it.

- [ ] **Step 5: Run the tests to see them pass**

Run: `cd windows-linux && cargo test -p snapline && cargo clippy -p snapline --all-targets -- -D warnings`
Expected: all pass (on this Mac too: none of this is Windows-only).

- [ ] **Step 6: Commit**

```bash
git add windows-linux
git commit -m "Add the Windows app crate with settings, copy and save"
```

---

### Task 9: The running app — tray, shortcuts, capture and overlay windows

**Files:**
- Modify: `windows-linux/crates/snapline/src/main.rs`
- Create: `windows-linux/crates/snapline/src/app.rs`
- Create: `windows-linux/crates/snapline/src/capture.rs`
- Create: `windows-linux/crates/snapline/src/window.rs`
- Create: `windows-linux/crates/snapline/src/icon.rs`
- Create: `windows-linux/crates/snapline/src/platform.rs`

**Interfaces:**
- Consumes: everything above.
- Produces: the `snapline` binary.

This is glue around OS APIs; there is nothing pure left to unit-test except the tray icon. It is checked by clippy on Windows CI, by running it on this Mac (Step 7), and by Barayuda on Windows (Task 10).

- [ ] **Step 1: Write the failing test for the tray icon**

`src/icon.rs`:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn tray_icon_is_square_rgba_and_has_the_brand_corners() {
        let rgba = super::tray_rgba(32);
        assert_eq!(rgba.len(), 32 * 32 * 4);
        let px = |x: usize, y: usize| &rgba[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4];
        assert_eq!(px(0, 0)[3], 0, "corner outside the rounded square is transparent");
        assert_eq!(px(16, 16), [255, 255, 255, 255], "the spark is white");
        let c = px(3, 16);
        assert!(c[3] == 255 && c[2] > c[1], "the background is the violet-cyan gradient");
    }
}
```

Run: `cd windows-linux && cargo test -p snapline icon` → fails, `tray_rgba` not found.

- [ ] **Step 2: Draw the tray icon**

Above the test in `src/icon.rs`:

```rust
//! The tray icon: the brand square (violet → cyan) with the white selection corners and spark (BRAND.md).

use snapline_overlay::tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LinearGradient, Paint, PathBuilder, Pixmap, Point, SpreadMode, Stroke,
    Transform,
};

/// Straight (not premultiplied) RGBA, `size`×`size`.
///
/// # Panics
/// If `size` is 0.
#[must_use]
pub fn tray_rgba(size: u32) -> Vec<u8> {
    let mut p = Pixmap::new(size, size).expect("icon size is not zero");
    let k = size as f32 / 32.0;
    let t = Transform::from_scale(k, k);

    let mut bg = Paint::default();
    bg.anti_alias = true;
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
        p.fill_path(&path, &bg, FillRule::Winding, t, None);
    }

    let mut white = Paint::default();
    white.set_color_rgba8(255, 255, 255, 255);
    white.anti_alias = true;
    let mut corners = PathBuilder::new();
    for [a, b, c] in [[(7.0, 12.0), (7.0, 7.0), (12.0, 7.0)], [(20.0, 7.0), (25.0, 7.0), (25.0, 12.0)],
        [(25.0, 20.0), (25.0, 25.0), (20.0, 25.0)], [(12.0, 25.0), (7.0, 25.0), (7.0, 20.0)]]
    {
        corners.move_to(a.0, a.1);
        corners.line_to(b.0, b.1);
        corners.line_to(c.0, c.1);
    }
    if let Some(path) = corners.finish() {
        let stroke = Stroke { width: 2.6, line_cap: LineCap::Round, ..Stroke::default() };
        p.stroke_path(&path, &white, &stroke, t, None);
    }
    let mut spark = PathBuilder::new();
    spark.move_to(16.0, 10.0);
    spark.quad_to(16.0, 16.0, 22.0, 16.0);
    spark.quad_to(16.0, 16.0, 16.0, 22.0);
    spark.quad_to(16.0, 16.0, 10.0, 16.0);
    spark.quad_to(16.0, 16.0, 16.0, 10.0);
    spark.close();
    if let Some(path) = spark.finish() {
        p.fill_path(&path, &white, FillRule::Winding, t, None);
    }

    p.pixels()
        .iter()
        .flat_map(|c| {
            let c = c.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}
```

Run: `cargo test -p snapline icon` → passes.

- [ ] **Step 3: Platform calls**

`src/platform.rs`:

```rust
//! The few Windows calls Snapline makes directly. Other systems get harmless stand-ins until Phase 6.

/// False when another Snapline is already running. The mutex lives as long as the process (never closed on purpose).
#[cfg(windows)]
pub fn claim_single_instance() -> bool {
    use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    let name: Vec<u16> = "Local\\dev.barayuda.snapline\0".encode_utf16().collect();
    // SAFETY: `name` is a NUL-terminated UTF-16 string that outlives the call; null attributes are allowed.
    unsafe {
        CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        GetLastError() != ERROR_ALREADY_EXISTS
    }
}

#[cfg(not(windows))]
pub fn claim_single_instance() -> bool {
    true // ponytail: no check off Windows yet; Linux gets one in Phase 6
}

#[cfg(windows)]
pub fn beep() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_OK, MessageBeep};
    // SAFETY: plain call with a constant.
    unsafe {
        MessageBeep(MB_OK);
    }
}

#[cfg(not(windows))]
pub fn beep() {}

/// The pointer position in physical screen pixels.
#[cfg(windows)]
pub fn cursor_position() -> Option<(i32, i32)> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: `p` is a valid, writable POINT.
    (unsafe { GetCursorPos(&raw mut p) } != 0).then_some((p.x, p.y))
}

#[cfg(not(windows))]
pub fn cursor_position() -> Option<(i32, i32)> {
    None
}

/// A message box (blocks until closed).
pub fn show_error(message: &str) {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Snapline")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}
```

- [ ] **Step 4: Capture and overlay windows**

`src/capture.rs`:

```rust
//! Freezes every monitor at full pixel resolution.

use snapline_overlay::tiny_skia::{IntSize, Pixmap};

pub struct Screen {
    /// Top-left corner on the virtual desktop, in physical pixels.
    pub x: i32,
    pub y: i32,
    pub image: Pixmap,
}

pub fn capture_all() -> Result<Vec<Screen>, String> {
    let monitors = xcap::Monitor::all().map_err(|e| e.to_string())?;
    monitors
        .iter()
        .map(|m| {
            let image = m.capture_image().map_err(|e| e.to_string())?;
            let (w, h) = image.dimensions();
            let mut data = image.into_raw();
            for px in data.chunks_exact_mut(4) {
                px[3] = 255; // some drivers leave alpha at 0; the screen is opaque
            }
            let size = IntSize::from_wh(w, h).ok_or("a monitor reported an empty image")?;
            let image = Pixmap::from_vec(data, size).ok_or("a monitor image had the wrong size")?;
            Ok(Screen { x: m.x().map_err(|e| e.to_string())?, y: m.y().map_err(|e| e.to_string())?, image })
        })
        .collect()
}
```

If the xcap version's `x()`/`y()` return plain `i32` instead of `Result`, drop the `map_err`.

`src/window.rs`:

```rust
//! One borderless, always-on-top window per monitor showing the overlay through softbuffer.

use crate::capture::Screen;
use snapline_overlay::render;
use snapline_overlay::session::Session;
use snapline_overlay::tiny_skia::Pixmap;
use std::num::NonZeroU32;
use std::rc::Rc;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowLevel};

pub struct OverlayWindow {
    pub window: Rc<Window>,
    _context: softbuffer::Context<Rc<Window>>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    frame: Pixmap,
}

impl OverlayWindow {
    /// Opens a hidden window covering `screen`. Show it after the first `present`.
    pub fn open(event_loop: &ActiveEventLoop, screen: &Screen) -> Result<Self, String> {
        let (w, h) = (screen.image.width(), screen.image.height());
        let attributes = Window::default_attributes()
            .with_title("Snapline")
            .with_decorations(false)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_position(PhysicalPosition::new(screen.x, screen.y))
            .with_inner_size(PhysicalSize::new(w, h))
            .with_visible(false);
        #[cfg(windows)]
        let attributes = {
            use winit::platform::windows::WindowAttributesExtWindows;
            attributes.with_skip_taskbar(true)
        };
        let window = Rc::new(event_loop.create_window(attributes).map_err(|e| e.to_string())?);
        let context = softbuffer::Context::new(window.clone()).map_err(|e| e.to_string())?;
        let mut surface = softbuffer::Surface::new(&context, window.clone()).map_err(|e| e.to_string())?;
        let (nw, nh) = NonZeroU32::new(w).zip(NonZeroU32::new(h)).ok_or("empty monitor")?;
        surface.resize(nw, nh).map_err(|e| e.to_string())?;
        let frame = Pixmap::new(w, h).ok_or("empty monitor")?;
        Ok(Self { window, _context: context, surface, frame })
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        PhysicalSize::new(self.frame.width(), self.frame.height())
    }

    /// Draws overlay `i` and puts it on screen.
    pub fn present(&mut self, session: &Session, i: usize) -> Result<(), String> {
        render::render_screen(&mut self.frame, session, i);
        let mut buffer = self.surface.buffer_mut().map_err(|e| e.to_string())?;
        // ponytail: full-frame copy per redraw (~10 ms at 4K); redraw only the dirty area if it feels slow.
        for (dst, src) in buffer.iter_mut().zip(self.frame.pixels()) {
            *dst = (u32::from(src.red()) << 16) | (u32::from(src.green()) << 8) | u32::from(src.blue());
        }
        buffer.present().map_err(|e| e.to_string())
    }
}
```

- [ ] **Step 5: The application**

`src/app.rs`:

```rust
//! The tray app: tray icon and menu, capture shortcuts, and one capture at a time.

use crate::capture;
use crate::icon;
use crate::platform::{self, show_error};
use crate::settings::{self, Settings, TrayClick};
use crate::window::OverlayWindow;
use crate::actions;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use snapline_overlay::input::{Key, Mods};
use snapline_overlay::session::{Command, CursorKind, Effect, Redraw, Session};
use snapline_overlay::text::TextFont;
use snapline_overlay::{Point, Rect};
use std::sync::Mutex;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};
use winit::application::ApplicationHandler;
use winit::event::{MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::{CursorIcon, WindowId};

pub enum UserEvent {
    HotKey,
    Menu(MenuId),
    Tray(TrayIconEvent),
}

struct Capture {
    session: Session,
    windows: Vec<OverlayWindow>,
    pointer: Vec<Point>,
    mods: Mods,
}

pub struct App {
    proxy: EventLoopProxy<UserEvent>,
    settings: Settings,
    font: TextFont,
    tray: Option<TrayIcon>,
    take: MenuId,
    quit: MenuId,
    hotkeys: Option<GlobalHotKeyManager>,
    capture: Option<Capture>,
    started: bool,
}

impl App {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            proxy,
            settings: Settings::default(),
            font: TextFont::system(),
            tray: None,
            take: MenuId::new("take"),
            quit: MenuId::new("quit"),
            hotkeys: None,
            capture: None,
            started: false,
        }
    }

    /// Settings, event forwarding, shortcuts and the tray. Runs once, when the event loop is up.
    fn start(&mut self) {
        if let Some(path) = Settings::path() {
            let (settings, problem) = Settings::load(&path);
            self.settings = settings;
            if let Some(problem) = problem {
                show_error(&format!("Snapline is using its default settings because this file could not be read:\n\n{problem}"));
            }
        }
        let proxy = Mutex::new(self.proxy.clone());
        GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
            if e.state == HotKeyState::Pressed
                && let Ok(p) = proxy.lock()
            {
                let _ = p.send_event(UserEvent::HotKey);
            }
        }));
        let proxy = Mutex::new(self.proxy.clone());
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if let Ok(p) = proxy.lock() {
                let _ = p.send_event(UserEvent::Menu(e.id));
            }
        }));
        let proxy = Mutex::new(self.proxy.clone());
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let Ok(p) = proxy.lock() {
                let _ = p.send_event(UserEvent::Tray(e));
            }
        }));

        let labels = self.register_hotkeys();
        match self.build_tray(&labels) {
            Ok(tray) => self.tray = Some(tray),
            Err(e) => show_error(&format!("Snapline could not add its tray icon: {e}")),
        }
    }

    /// Registers every configured shortcut it can, and returns the labels of those that worked.
    fn register_hotkeys(&mut self) -> Vec<String> {
        let manager = match GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                show_error(&format!("Snapline could not set up its capture shortcuts: {e}"));
                return Vec::new();
            }
        };
        let working: Vec<String> = self
            .settings
            .hotkeys
            .iter()
            .filter(|name| name.parse::<HotKey>().is_ok_and(|h| manager.register(h).is_ok()))
            .map(|name| settings::shortcut_label(name))
            .collect();
        if working.is_empty() {
            let wanted: Vec<String> = self.settings.hotkeys.iter().map(|n| settings::shortcut_label(n)).collect();
            let file = Settings::path().map_or_else(String::new, |p| format!(" in {}", p.display()));
            show_error(&format!(
                "Snapline could not use its capture shortcuts ({}). Another app may be using them. On Windows 11, \
                 PrtScn often opens the Snipping Tool: turn that off in Settings → Accessibility → Keyboard. \
                 You can still take a screenshot from the tray icon, or set other keys under hotkeys{file}.",
                wanted.join(", ")
            ));
        }
        self.hotkeys = Some(manager);
        working
    }

    fn build_tray(&self, labels: &[String]) -> Result<TrayIcon, String> {
        let title = if labels.is_empty() { "Take Screenshot".to_owned() } else { format!("Take Screenshot\t{}", labels.join(", ")) };
        let menu = Menu::new();
        menu.append_items(&[
            &MenuItem::with_id(self.take.clone(), title, true, None),
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id(self.quit.clone(), "Quit Snapline", true, None),
        ])
        .map_err(|e| e.to_string())?;
        let icon = Icon::from_rgba(icon::tray_rgba(32), 32, 32).map_err(|e| e.to_string())?;
        TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Snapline")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(self.settings.tray_click == TrayClick::Menu)
            .build()
            .map_err(|e| e.to_string())
    }

    fn open_capture(&mut self, event_loop: &ActiveEventLoop) {
        if self.capture.is_some() {
            return; // the shortcut pressed again during a capture does nothing
        }
        let screens = match capture::capture_all() {
            Ok(s) if !s.is_empty() => s,
            Ok(_) => return,
            Err(e) => return show_error(&format!("Snapline could not capture the screen: {e}")),
        };
        let frames: Vec<Rect> = screens
            .iter()
            .map(|s| Rect::new(f64::from(s.x), f64::from(s.y), f64::from(s.image.width()), f64::from(s.image.height())))
            .collect();
        let mut windows = Vec::new();
        for screen in &screens {
            match OverlayWindow::open(event_loop, screen) {
                Ok(w) => windows.push(w),
                Err(e) => return show_error(&format!("Snapline could not open its capture window: {e}")),
            }
        }
        let scales: Vec<f64> = windows.iter().map(|w| w.window.scale_factor()).collect();
        let session = Session::new(screens.into_iter().map(|s| s.image).zip(scales).collect(), self.font.clone());
        let n = windows.len();
        let mut cap = Capture { session, windows, pointer: vec![Point::default(); n], mods: Mods::default() };
        cap.session.focused = platform::cursor_position()
            .and_then(|(x, y)| {
                let (x, y) = (f64::from(x), f64::from(y));
                frames.iter().position(|f| x >= f.x && x < f.x + f.width && y >= f.y && y < f.y + f.height)
            })
            .unwrap_or(0);
        for (k, w) in cap.windows.iter_mut().enumerate() {
            if let Err(e) = w.present(&cap.session, k) {
                eprintln!("Snapline: {e}");
            }
            w.window.set_visible(true);
            w.window.set_cursor(CursorIcon::Crosshair);
        }
        cap.windows[cap.session.focused].window.focus_window();
        self.capture = Some(cap);
    }

    fn apply(&mut self, i: usize, effect: Effect) {
        let Some(cap) = self.capture.as_mut() else { return };
        if effect.beep {
            platform::beep();
        }
        match effect.redraw {
            Redraw::None => {}
            Redraw::One(k) => cap.windows[k].window.request_redraw(),
            Redraw::All => cap.windows.iter().for_each(|w| w.window.request_redraw()),
        }
        cap.windows[i].window.set_cursor(cursor_icon(cap.session.cursor(i)));
        let Some(command) = effect.command else { return };
        let image = cap.session.export();
        self.capture = None; // closes the overlay windows before any dialog opens
        if command == Command::Cancel {
            return;
        }
        let Some(image) = image else { return platform::beep() };
        if let Err(e) = actions::run(command, &image, &self.settings) {
            show_error(&e);
        }
    }
}

fn cursor_icon(kind: CursorKind) -> CursorIcon {
    match kind {
        CursorKind::Crosshair => CursorIcon::Crosshair,
        CursorKind::Arrow => CursorIcon::Default,
        CursorKind::Move => CursorIcon::Move,
        CursorKind::Text => CursorIcon::Text,
        CursorKind::ResizeHorizontal => CursorIcon::EwResize,
        CursorKind::ResizeVertical => CursorIcon::NsResize,
        CursorKind::ResizeDiagonalDown => CursorIcon::NwseResize,
        CursorKind::ResizeDiagonalUp => CursorIcon::NeswResize,
    }
}

fn key_of(key: &WinitKey) -> Option<Key> {
    Some(match key {
        WinitKey::Named(NamedKey::Escape) => Key::Escape,
        WinitKey::Named(NamedKey::Enter) => Key::Enter,
        WinitKey::Named(NamedKey::Backspace) => Key::Backspace,
        WinitKey::Named(NamedKey::ArrowLeft) => Key::Left,
        WinitKey::Named(NamedKey::ArrowRight) => Key::Right,
        WinitKey::Named(NamedKey::ArrowUp) => Key::Up,
        WinitKey::Named(NamedKey::ArrowDown) => Key::Down,
        WinitKey::Named(NamedKey::Space) => Key::Char(' '),
        WinitKey::Character(s) => Key::Char(s.chars().next()?),
        _ => return None,
    })
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        if !self.started {
            self.started = true;
            self.start();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::HotKey => self.open_capture(event_loop),
            UserEvent::Menu(id) if id == self.take => self.open_capture(event_loop),
            UserEvent::Menu(id) if id == self.quit => event_loop.exit(),
            UserEvent::Tray(TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            }) if self.settings.tray_click == TrayClick::Capture => self.open_capture(event_loop),
            _ => {}
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(cap) = self.capture.as_mut() else { return };
        let Some(i) = cap.windows.iter().position(|w| w.window.id() == id) else { return };
        let effect = match event {
            WindowEvent::RedrawRequested => {
                if let Err(e) = cap.windows[i].present(&cap.session, i) {
                    eprintln!("Snapline: {e}");
                }
                return;
            }
            // Keep the window exactly the monitor's pixels when Windows moves it between scales.
            WindowEvent::ScaleFactorChanged { mut inner_size_writer, .. } => {
                let _ = inner_size_writer.request_inner_size(cap.windows[i].size());
                return;
            }
            WindowEvent::ModifiersChanged(m) => {
                let state = m.state();
                cap.mods = Mods { ctrl: state.control_key(), shift: state.shift_key(), alt: state.alt_key() };
                return;
            }
            WindowEvent::CursorMoved { position, .. } => {
                let p = Point { x: position.x, y: position.y };
                cap.pointer[i] = p;
                cap.session.pointer_move(i, p)
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                if state.is_pressed() {
                    cap.session.pointer_down(i, cap.pointer[i], cap.mods)
                } else {
                    cap.session.pointer_up(i, cap.pointer[i])
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y),
                    MouseScrollDelta::PixelDelta(p) => p.y,
                };
                cap.session.scroll(i, cap.pointer[i], lines)
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => match key_of(&event.logical_key) {
                Some(key) => cap.session.key(key, cap.mods),
                None => return,
            },
            WindowEvent::CloseRequested => Effect::command(Command::Cancel),
            _ => return,
        };
        self.apply(i, effect);
    }
}
```

`src/main.rs`:

```rust
//! Snapline for Windows: a tray app that captures the screen, lets you mark it up, and copies or saves it.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window in release builds
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)] // pixel maths on clamped values

mod actions;
mod app;
mod capture;
mod icon;
mod platform;
mod settings;
mod window;

use winit::event_loop::{ControlFlow, EventLoop};

fn main() {
    if !platform::claim_single_instance() {
        platform::show_error("Snapline is already running. Look for its icon next to the clock.");
        return;
    }
    let event_loop = match EventLoop::<app::UserEvent>::with_user_event().build() {
        Ok(l) => l,
        Err(e) => return platform::show_error(&format!("Snapline could not start: {e}")),
    };
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = app::App::new(event_loop.create_proxy());
    if let Err(e) = event_loop.run_app(&mut app) {
        platform::show_error(&format!("Snapline stopped: {e}"));
    }
}
```

Remove the `#[allow(dead_code)]` lines added in Task 8.

- [ ] **Step 6: Build and lint**

Run: `cd windows-linux && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: clean. Fix API differences against the installed crate versions (see Global Constraints) without changing behaviour.

- [ ] **Step 7: Try it on this Mac**

Run: `cd windows-linux && cargo run -p snapline`
Expected: a Snapline icon in the menu bar with "Take Screenshot" and "Quit Snapline". The terminal needs Screen Recording permission for the capture. Choose Take Screenshot (and try Ctrl+Shift+1): the screen freezes, drag a selection, draw with each tool, type text, pick a colour, undo, then Ctrl+C and paste into Preview; then Ctrl+S and check the Desktop. Write down anything that differs from the macOS app; macOS-only window quirks are not bugs for this phase, but drawing or selection bugs are. Quit from the menu.

- [ ] **Step 8: Commit**

```bash
git add windows-linux
git commit -m "Run the Windows app: tray, capture shortcuts and overlay windows"
```

---

### Task 10: CI build, docs, and the Windows check

**Files:**
- Modify: `.github/workflows/windows-linux.yml`
- Modify: `docs/windows-linux.md`
- Modify: `README.md`

**Interfaces:** none.

- [ ] **Step 1: CI builds the app on Windows and keeps Linux to the shared crates**

Replace the last three steps of `.github/workflows/windows-linux.yml` with:

```yaml
      - run: cargo fmt --check
      # The app is Windows-only until Phase 6; Linux checks the shared crates.
      - name: Lint and test (Linux)
        if: runner.os == 'Linux'
        run: |
          cargo clippy --workspace --exclude snapline --all-targets -- -D warnings
          cargo test --workspace --exclude snapline
      - name: Lint and test (Windows)
        if: runner.os == 'Windows'
        run: |
          cargo clippy --workspace --all-targets -- -D warnings
          cargo test --workspace
      - name: Build snapline.exe
        if: runner.os == 'Windows'
        run: cargo build --release -p snapline
      - uses: actions/upload-artifact@v7
        if: runner.os == 'Windows'
        with:
          name: snapline-windows
          path: windows-linux/target/release/snapline.exe
          if-no-files-found: error
```

Check `actions/upload-artifact`'s latest major with `gh api repos/actions/upload-artifact/releases/latest -q .tag_name` and use it.

- [ ] **Step 2: Update the design doc**

In `docs/windows-linux.md`:
- Repo layout: add `crates/snapline-overlay/   the overlay without windows: selection, drawing, toolbars, rendering (tiny-skia)` and say the app's Windows code is in `src/platform.rs` for now.
- Platform table: Overlay → "`winit` borderless topmost window per monitor showing the frozen capture through `softbuffer`"; Drawing → "`tiny-skia` for both the screen and the saved image (one renderer)"; Menu bar item and popovers → "`tray-icon`; popups drawn like the overlay (Phase 3)"; Preferences → "a separate `egui` window (Phase 5)"; Carbon hotkey → "`global-hotkey`; PrtScn and Ctrl+Shift+1 by default".
- Decisions table: add "Overlay drawing | tiny-skia only, no egui in the overlay | One renderer for screen and export; pixel tests on CI; Barayuda, 29 Oct 2026" and "Default shortcut | PrtScn and Ctrl+Shift+1 | Lightshot habit plus the macOS keys; Barayuda, 29 Oct 2026".
- Phase 2 line: link this plan.
- Risks: add "Windows 11 may give PrtScn to the Snipping Tool; Snapline then keeps Ctrl+Shift+1 and explains how to free PrtScn."

- [ ] **Step 3: README**

Add a short "Windows (preview)" section after the macOS install section:

```markdown
## Windows (preview)

The Windows version is in progress (see [docs/windows-linux.md](docs/windows-linux.md)). It can capture, select,
draw, copy and save; uploads, auto-hide and Copy Text come later.

To try it: open the latest successful **windows-linux** run under Actions, download **snapline-windows**, unzip and
run `snapline.exe`. It is not signed yet, so Windows SmartScreen asks first (More info → Run anyway).

- Take a screenshot: PrtScn or Ctrl+Shift+1, or the tray icon's menu.
- In the capture: the same keys as on macOS with Ctrl for ⌘. Ctrl+Shift+S is Save As.
- Settings: `%APPDATA%\Snapline\settings.toml` (`save_directory`, `hotkeys`, `tray_click = "menu"` or `"capture"`).
```

Also update the project layout section to list `windows-linux/crates/snapline-overlay/` and `windows-linux/crates/snapline/`.

- [ ] **Step 4: Full local check**

Run: `cd windows-linux && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` and `cd ../macOS && swift test`
Expected: all clean (the Swift tests are unaffected but share `fixtures/`).

- [ ] **Step 5: Commit and push, then check CI**

```bash
git add .github docs README.md
git commit -m "Build snapline.exe on CI and document the Windows preview"
git log --format='%h %G?' origin/feat/windows-linux-rust..HEAD   # every line must end in G
git push
```

Expected: both CI jobs pass and the Windows job has a `snapline-windows` artifact.

- [ ] **Step 6: Barayuda checks it on Windows**

Hand Barayuda this list with the artifact link. Each line is pass or fail, with a screenshot for any fail:

1. Run `snapline.exe`. A Snapline icon appears next to the clock; running it again says it is already running.
2. Click the tray icon: the menu shows "Take Screenshot  PrtScn, Ctrl+Shift+1" and "Quit Snapline".
3. Press PrtScn. If the Snipping Tool opens instead, turn off Settings → Accessibility → Keyboard → "Use the Print screen key to open screen capture", restart Snapline, try again. Ctrl+Shift+1 must work either way.
4. The screen freezes and dims on every monitor, with the hint in the middle of the monitor under the pointer. The tray menu is not in the frozen image.
5. Drag a selection: size badge in pixels, handles, two toolbars beside it. Move it, resize it from each handle, nudge it with the arrows (Shift for 10, Alt to resize).
6. Each tool: pencil, line, arrow, rectangle, ellipse, marker (see-through), text (type, Backspace, Enter; scroll to resize; drag to move), hide (black box). Mouse wheel changes the line width. Ctrl+Z undoes.
7. Colour button: a palette of 8 colours; picking one changes new drawings.
8. Type text including an AltGr character if your keyboard has one, then press Ctrl+C without pressing Enter: paste into Paint, the text is there.
9. Ctrl+S: a PNG on the Desktop, full resolution; again: a second file, not overwritten. Ctrl+Shift+S: a save dialog.
10. With two monitors at different scaling (e.g. 100 % and 150 %): select on each; lines, text and toolbars are the right size on both; Ctrl+A selects only the monitor under the pointer; the saved image has the full pixels of that area.
11. Esc closes the capture; the shortcut works again afterwards. Quit from the tray menu removes the icon.

- [ ] **Step 7: Fix what the Windows check finds**

For each fail: write a failing test in `snapline-overlay` when the bug is in overlay logic, fix it, and commit; for window or OS bugs, fix in the app crate and ask Barayuda to re-check that line.
