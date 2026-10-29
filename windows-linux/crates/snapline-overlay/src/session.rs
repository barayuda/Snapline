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
    pub const NONE: Self = Self {
        redraw: Redraw::None,
        command: None,
        beep: false,
    };

    #[must_use]
    pub const fn one(i: usize) -> Self {
        Self {
            redraw: Redraw::One(i),
            ..Self::NONE
        }
    }

    #[must_use]
    pub const fn all() -> Self {
        Self {
            redraw: Redraw::All,
            ..Self::NONE
        }
    }

    #[must_use]
    pub const fn command(c: Command) -> Self {
        Self {
            command: Some(c),
            ..Self::NONE
        }
    }

    #[must_use]
    pub const fn beep() -> Self {
        Self {
            beep: true,
            ..Self::NONE
        }
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
        Rect::new(
            0.0,
            0.0,
            f64::from(self.screen.width()),
            f64::from(self.screen.height()),
        )
    }

    #[must_use]
    pub fn is_dragging(&self) -> bool {
        !matches!(self.drag, Drag::None)
    }

    fn clamp(&self, p: Point) -> Point {
        let b = self.bounds();
        Point {
            x: p.x.clamp(0.0, b.width),
            y: p.y.clamp(0.0, b.height),
        }
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
            overlays: screens
                .into_iter()
                .map(|(p, s)| Overlay::new(p, s))
                .collect(),
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
        self.overlays.iter().position(|o| {
            o.selection
                .is_some_and(|s| s.width >= 1.0 && s.height >= 1.0)
        })
    }

    /// The finished image of the selection, with any text being typed included. `None` without a selection.
    pub fn export(&mut self) -> Option<Pixmap> {
        self.commit_text();
        let i = self.selected()?;
        let o = &self.overlays[i];
        render::render_selection(&o.screen, o.selection?, &o.annotations, &self.font)
    }

    /// A command that needs a selection: beep without one.
    #[allow(dead_code)] // used by the keyboard and toolbar (Tasks 5 and 6)
    pub(crate) fn request(&self, c: Command) -> Effect {
        if self.selected().is_some() {
            Effect::command(c)
        } else {
            Effect::beep()
        }
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
        let scale = self.overlays[i].ui_scale;
        let action = match self.overlays[i].usable_selection() {
            Some(sel) => {
                if self.tool.is_none()
                    && let Some(h) = geo::handle_at(p, sel, 6.0 * scale)
                {
                    Down::Resize(h, sel)
                } else if contains(sel, p) {
                    match self.tool {
                        None => Down::Move(sel),
                        Some(Tool::Text) => {
                            self.text_index(i, p).map_or(Down::Text, Down::MoveText)
                        }
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
        let (color, width, size) = (
            self.color,
            self.width * scale as f32,
            self.font_size * scale as f32,
        );
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
                let origin = Point {
                    x: p.x - 2.0 * scale,
                    y: p.y - f64::from(size) * 0.6,
                };
                o.editing = Some(TextEdit {
                    origin,
                    text: String::new(),
                    size,
                });
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
                o.selection = Some(geo::move_within(
                    original,
                    p.x - start.x,
                    p.y - start.y,
                    bounds,
                ));
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
        if self.overlays[i].annotations.pop().is_some() {
            Effect::one(i)
        } else {
            Effect::beep()
        }
    }

    /// The text annotation under `p`, topmost first.
    pub(crate) fn text_index(&self, i: usize, p: Point) -> Option<usize> {
        let o = &self.overlays[i];
        let pad = 4.0 * o.ui_scale;
        o.annotations.iter().rposition(|a| match &a.shape {
            Shape::Text { origin, text, size } => {
                let m = self.font.measure(text, *size);
                contains(
                    Rect::new(
                        origin.x - pad,
                        origin.y - pad,
                        m.width + 2.0 * pad,
                        m.height + 2.0 * pad,
                    ),
                    p,
                )
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
                    let shape = Shape::Text {
                        origin: e.origin,
                        text: text.to_owned(),
                        size: e.size,
                    };
                    o.annotations.push(Annotation {
                        shape,
                        color,
                        width: 0.0,
                        marker: false,
                    });
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
        let Some(sel) = o.usable_selection() else {
            return CursorKind::Crosshair;
        };
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
        if self.tool.is_none() {
            CursorKind::Crosshair
        } else {
            CursorKind::Arrow
        }
    }
}

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
        assert_eq!(
            s.overlays[0].selection,
            Some(Rect::new(10.0, 10.0, 100.0, 50.0))
        );
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
        assert_eq!(
            s.overlays[0].selection,
            Some(Rect::new(0.0, 10.0, 100.0, 50.0))
        );
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
        assert_eq!(s.overlays[0].annotations, []);
        assert!(s.undo().beep, "nothing left to undo");
    }

    #[test]
    fn a_click_draws_only_with_the_pencil() {
        let mut s = selected(1.0);
        s.tool = Some(Tool::Rectangle);
        drag(&mut s, 0, pt(20.0, 20.0), pt(20.0, 20.0));
        assert_eq!(s.overlays[0].annotations, []);
        s.tool = Some(Tool::Pencil);
        drag(&mut s, 0, pt(20.0, 20.0), pt(20.0, 20.0));
        assert_eq!(s.overlays[0].annotations.len(), 1);
    }

    #[test]
    fn with_a_tool_a_click_outside_keeps_the_selection() {
        let mut s = selected(1.0);
        s.tool = Some(Tool::Pencil);
        drag(&mut s, 0, pt(300.0, 200.0), pt(320.0, 220.0));
        assert_eq!(
            s.overlays[0].selection,
            Some(Rect::new(10.0, 10.0, 100.0, 50.0))
        );
        assert_eq!(s.overlays[0].annotations, []);
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
        assert_eq!(
            (image.width(), image.height()),
            (100, 50),
            "export is the selected pixels, whatever the scale"
        );
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
