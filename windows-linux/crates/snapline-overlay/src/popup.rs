//! The status popup next to the tray icon: upload progress and result, and short confirmations.
//! Twin of the macOS menu bar popovers (`UploadPopover.swift`, `StatusPopover.message`).

use crate::color::{INK, Rgb, VIOLET, WHITE};
use crate::render::fill_rounded;
use crate::text::TextFont;
use snapline_core::selection_geometry::{Point, Rect, Size};
use tiny_skia::Pixmap;

const WIDTH: f64 = 340.0;
const PAD: f64 = 14.0;
const GAP: f64 = 8.0;
const TITLE: f32 = 15.0;
const BODY: f32 = 13.0;
const BUTTON_H: f64 = 28.0;
const BAR_H: f64 = 6.0;
const CLOSE: f64 = 20.0;
const GREEN: Rgb = Rgb(0x34, 0xC7, 0x59);
const RED: Rgb = Rgb(0xFF, 0x3B, 0x30);
const GREY: Rgb = Rgb(0xB8, 0xB5, 0xC8);

#[derive(Clone, Debug, PartialEq)]
pub enum PopupState {
    Uploading { progress: f64, file: String },
    Done { link: String },
    Failed { message: String },
    Message { title: String, detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupButton {
    Copy,
    Open,
    Settings,
    Retry,
    Close,
}

impl PopupButton {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Open => "Open",
            Self::Settings => "Upload settings…",
            Self::Retry => "Try again",
            Self::Close => "×",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Popup {
    pub state: PopupState,
    pub hover: Option<PopupButton>,
}

/// Splits `text` into lines no wider than `width`, breaking long words.
#[must_use]
pub fn wrap(font: &TextFont, text: &str, size: f32, width: f64) -> Vec<String> {
    let fits = |s: &str| font.measure(s, size).width <= width;
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if fits(&candidate) {
            line = candidate;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        for c in word.chars() {
            line.push(c);
            if !fits(&line) && line.chars().count() > 1 {
                line.pop();
                lines.push(std::mem::take(&mut line));
                line.push(c);
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// `text` with its middle replaced by "…" until it fits `width`.
#[must_use]
pub fn shorten_middle(font: &TextFont, text: &str, size: f32, width: f64) -> String {
    if font.measure(text, size).width <= width {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut keep = chars.len();
    while keep > 2 {
        keep -= 1;
        let head: String = chars[..keep.div_ceil(2)].iter().collect();
        let tail: String = chars[chars.len() - keep / 2..].iter().collect();
        let short = format!("{head}…{tail}");
        if font.measure(&short, size).width <= width {
            return short;
        }
    }
    "…".to_owned()
}

/// Where the popup goes: next to the tray icon (above, below, left or right, whichever fits the work area),
/// or the bottom-right corner of the work area when the icon's place is unknown.
#[must_use]
pub fn origin(anchor: Option<Rect>, size: Size, work: Rect, gap: f64) -> Point {
    let (right, bottom) = (work.x + work.width, work.y + work.height);
    let clamp_x = |x: f64| x.min(right - size.width).max(work.x);
    let clamp_y = |y: f64| y.min(bottom - size.height).max(work.y);
    let fits = |p: &Point| {
        p.x >= work.x && p.y >= work.y && p.x + size.width <= right && p.y + size.height <= bottom
    };
    if let Some(a) = anchor {
        let centre_x = clamp_x(a.x + a.width / 2.0 - size.width / 2.0);
        let centre_y = clamp_y(a.y + a.height / 2.0 - size.height / 2.0);
        let candidates = [
            Point {
                x: centre_x,
                y: a.y - size.height - gap,
            },
            Point {
                x: centre_x,
                y: a.y + a.height + gap,
            },
            Point {
                x: a.x - size.width - gap,
                y: centre_y,
            },
            Point {
                x: a.x + a.width + gap,
                y: centre_y,
            },
        ];
        if let Some(p) = candidates.into_iter().find(fits) {
            return p;
        }
    }
    Point {
        x: right - size.width - gap,
        y: bottom - size.height - gap,
    }
}

impl Popup {
    #[must_use]
    pub fn new(state: PopupState) -> Self {
        Self { state, hover: None }
    }

    fn title(&self) -> (&str, Rgb) {
        match &self.state {
            PopupState::Uploading { .. } => ("Uploading…", WHITE),
            PopupState::Done { .. } => ("Uploaded. Link copied.", GREEN),
            PopupState::Failed { .. } => ("Upload failed", RED),
            PopupState::Message { title, .. } => (title, GREEN),
        }
    }

    fn inner(s: f64) -> f64 {
        (WIDTH - 2.0 * PAD) * s
    }

    fn lines(&self, font: &TextFont, s: f64) -> Vec<String> {
        let (size, width) = (BODY * s as f32, Self::inner(s));
        match &self.state {
            PopupState::Uploading { file, .. } => vec![shorten_middle(font, file, size, width)],
            PopupState::Done { link } => vec![shorten_middle(font, link, size, width)],
            PopupState::Failed { message } => wrap(font, message, size, width),
            PopupState::Message { detail, .. } => wrap(font, detail, size, width),
        }
    }

    fn actions(&self) -> &'static [PopupButton] {
        match self.state {
            PopupState::Done { .. } => &[PopupButton::Copy, PopupButton::Open],
            PopupState::Failed { .. } => &[PopupButton::Settings, PopupButton::Retry],
            _ => &[],
        }
    }

    fn line_height(font: &TextFont, size: f32) -> f64 {
        font.measure("Ag", size).height
    }

    /// The progress bar, when uploading.
    #[must_use]
    pub fn progress_bar(&self, font: &TextFont, s: f64) -> Option<Rect> {
        matches!(self.state, PopupState::Uploading { .. }).then(|| {
            let y = PAD * s + Self::line_height(font, TITLE * s as f32) + GAP * s;
            Rect::new(PAD * s, y, Self::inner(s), BAR_H * s)
        })
    }

    #[must_use]
    pub fn size(&self, font: &TextFont, s: f64) -> Size {
        let mut height = PAD * s + Self::line_height(font, TITLE * s as f32) + GAP * s;
        if self.progress_bar(font, s).is_some() {
            height += (BAR_H + GAP) * s;
        }
        height += self.lines(font, s).len() as f64 * Self::line_height(font, BODY * s as f32);
        if !self.actions().is_empty() {
            height += (GAP + BUTTON_H) * s;
        }
        Size {
            width: WIDTH * s,
            height: height + PAD * s,
        }
    }

    /// Close first, then the action buttons from left to right (right-aligned at the bottom).
    #[must_use]
    pub fn buttons(&self, font: &TextFont, s: f64) -> Vec<(PopupButton, Rect)> {
        let size = self.size(font, s);
        let close = Rect::new(
            size.width - (PAD / 2.0 + CLOSE) * s,
            PAD / 2.0 * s,
            CLOSE * s,
            CLOSE * s,
        );
        let mut buttons = vec![(PopupButton::Close, close)];
        let y = size.height - (PAD + BUTTON_H) * s;
        let mut x = size.width - PAD * s;
        let mut row = Vec::new();
        for b in self.actions().iter().rev() {
            let w = font.measure(b.label(), BODY * s as f32).width + 24.0 * s;
            x -= w;
            row.push((*b, Rect::new(x, y, w, BUTTON_H * s)));
            x -= GAP * s;
        }
        row.reverse();
        buttons.extend(row);
        buttons
    }

    #[must_use]
    pub fn hit(&self, font: &TextFont, s: f64, p: Point) -> Option<PopupButton> {
        self.buttons(font, s)
            .into_iter()
            .find(|(_, r)| crate::session::contains(*r, p))
            .map(|(b, _)| b)
    }

    pub fn render(&self, out: &mut Pixmap, font: &TextFont, s: f64) {
        let size = self.size(font, s);
        out.fill(tiny_skia::Color::TRANSPARENT);
        fill_rounded(
            out,
            Rect::new(0.0, 0.0, size.width, size.height),
            (10.0 * s) as f32,
            INK,
            245,
        );
        let (title, colour) = self.title();
        let mut y = PAD * s;
        font.draw(
            out,
            title,
            Point { x: PAD * s, y },
            TITLE * s as f32,
            colour,
        );
        y += Self::line_height(font, TITLE * s as f32) + GAP * s;
        if let (Some(bar), PopupState::Uploading { progress, .. }) =
            (self.progress_bar(font, s), &self.state)
        {
            fill_rounded(out, bar, (BAR_H / 2.0 * s) as f32, WHITE, 50);
            let filled = Rect {
                width: bar.width * progress.clamp(0.0, 1.0),
                ..bar
            };
            fill_rounded(out, filled, (BAR_H / 2.0 * s) as f32, VIOLET, 255);
            y += (BAR_H + GAP) * s;
        }
        let line_h = Self::line_height(font, BODY * s as f32);
        for line in self.lines(font, s) {
            font.draw(out, &line, Point { x: PAD * s, y }, BODY * s as f32, GREY);
            y += line_h;
        }
        for (button, r) in self.buttons(font, s) {
            let hovered = self.hover == Some(button);
            if button == PopupButton::Close {
                if hovered {
                    fill_rounded(out, r, (r.height / 2.0) as f32, WHITE, 40);
                }
            } else {
                fill_rounded(
                    out,
                    r,
                    (6.0 * s) as f32,
                    WHITE,
                    if hovered { 70 } else { 40 },
                );
            }
            let label_size = BODY * s as f32;
            let m = font.measure(button.label(), label_size);
            let at = Point {
                x: r.x + (r.width - m.width) / 2.0,
                y: r.y + (r.height - m.height) / 2.0,
            };
            font.draw(out, button.label(), at, label_size, WHITE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::Color;

    const WORK: Rect = Rect::new(0.0, 0.0, 1920.0, 1040.0);
    const SIZE: Size = Size {
        width: 340.0,
        height: 120.0,
    };

    fn inside(p: Point, work: Rect) -> bool {
        p.x >= work.x
            && p.y >= work.y
            && p.x + SIZE.width <= work.x + work.width
            && p.y + SIZE.height <= work.y + work.height
    }

    // Review Focus 3.
    #[test]
    fn popup_stays_in_the_work_area_for_every_taskbar_edge() {
        // Taskbar at the bottom: above the tray icon.
        let p = origin(Some(Rect::new(1800.0, 1048.0, 24.0, 24.0)), SIZE, WORK, 8.0);
        assert!(
            inside(p, WORK) && p.y + SIZE.height <= 1040.0 && p.x + SIZE.width >= 1800.0,
            "{p:?}"
        );
        // Taskbar at the top: below it.
        let work = Rect::new(0.0, 40.0, 1920.0, 1040.0);
        let p = origin(Some(Rect::new(1800.0, 8.0, 24.0, 24.0)), SIZE, work, 8.0);
        assert!(inside(p, work) && p.y >= 40.0, "{p:?}");
        // Taskbar on the right: to its left.
        let work = Rect::new(0.0, 0.0, 1860.0, 1080.0);
        let p = origin(Some(Rect::new(1880.0, 1000.0, 24.0, 24.0)), SIZE, work, 8.0);
        assert!(inside(p, work) && p.x + SIZE.width <= 1872.0, "{p:?}");
        // Taskbar on the left: to its right.
        let work = Rect::new(60.0, 0.0, 1860.0, 1080.0);
        let p = origin(Some(Rect::new(16.0, 1000.0, 24.0, 24.0)), SIZE, work, 8.0);
        assert!(inside(p, work) && p.x >= 48.0, "{p:?}");
        // Tray icon unknown (hidden in the overflow): bottom-right corner of the work area.
        let p = origin(None, SIZE, WORK, 8.0);
        assert_eq!(
            p,
            Point {
                x: 1920.0 - 340.0 - 8.0,
                y: 1040.0 - 120.0 - 8.0
            }
        );
    }

    #[test]
    fn long_messages_wrap_within_the_width_and_long_words_break() {
        let font = TextFont::bundled();
        let lines = wrap(
            &font,
            "Snapline could not reach shots.example.com: connection refused by the server",
            13.0,
            200.0,
        );
        assert!(lines.len() > 1);
        assert!(
            lines.iter().all(|l| font.measure(l, 13.0).width <= 200.0),
            "{lines:?}"
        );
        let word = wrap(&font, &"x".repeat(200), 13.0, 100.0);
        assert!(word.len() > 1 && word.iter().all(|l| !l.is_empty()));
    }

    #[test]
    fn links_are_shortened_in_the_middle() {
        let font = TextFont::bundled();
        let link =
            "https://shots.example.com/snapline/2026/10/30/Screenshot-2026-10-30-at-09.15.00.png";
        let short = shorten_middle(&font, link, 13.0, 200.0);
        assert!(
            short.starts_with("https://") && short.ends_with("00.png") && short.contains('…'),
            "{short}"
        );
        assert!(font.measure(&short, 13.0).width <= 200.0);
    }

    #[test]
    fn buttons_depend_on_the_state_and_can_be_hit() {
        let font = TextFont::bundled();
        let done = Popup::new(PopupState::Done {
            link: "https://x.example/a.png".into(),
        });
        let kinds: Vec<_> = done.buttons(&font, 1.0).iter().map(|(b, _)| *b).collect();
        assert_eq!(
            kinds,
            [PopupButton::Close, PopupButton::Copy, PopupButton::Open]
        );
        let failed = Popup::new(PopupState::Failed {
            message: "S3 upload failed (HTTP 403).".into(),
        });
        let retry = failed
            .buttons(&font, 1.0)
            .into_iter()
            .find(|(b, _)| *b == PopupButton::Retry)
            .unwrap()
            .1;
        let centre = Point {
            x: retry.x + retry.width / 2.0,
            y: retry.y + retry.height / 2.0,
        };
        assert_eq!(failed.hit(&font, 1.0, centre), Some(PopupButton::Retry));
        assert_eq!(failed.hit(&font, 1.0, Point { x: 2.0, y: 2.0 }), None);
    }

    #[test]
    fn progress_bar_fills_to_the_progress() {
        let font = TextFont::bundled();
        let popup = Popup::new(PopupState::Uploading {
            progress: 0.5,
            file: "Shot.png".into(),
        });
        let size = popup.size(&font, 1.0);
        let mut out = Pixmap::new(size.width as u32, size.height as u32).unwrap();
        out.fill(Color::BLACK);
        popup.render(&mut out, &font, 1.0);
        let bar = popup.progress_bar(&font, 1.0).unwrap();
        let y = (bar.y + bar.height / 2.0) as u32;
        let at = |f: f64| {
            out.pixel((bar.x + bar.width * f) as u32, y)
                .unwrap()
                .demultiply()
        };
        let (filled, empty) = (at(0.25), at(0.75));
        assert!(
            filled.blue() > 200 && filled.red() > 100,
            "violet fill: {filled:?}"
        );
        assert!(empty.blue() < 120, "track only: {empty:?}");
    }

    #[test]
    fn failed_popups_grow_with_their_message() {
        let font = TextFont::bundled();
        let short = Popup::new(PopupState::Failed {
            message: "Oops.".into(),
        })
        .size(&font, 1.0);
        let long = Popup::new(PopupState::Failed {
            message: "word ".repeat(80),
        })
        .size(&font, 1.0);
        assert!(long.height > short.height && (long.width - 340.0).abs() < f64::EPSILON);
    }
}
