# Phase 4: Windows text recognition — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On Windows, every capture is scanned on the PC: sensitive text and faces get solid black boxes before anything can be copied, saved or uploaded, and a click on a box shows it again. Copy Text (Ctrl+Shift+C) reads the selection with Windows OCR and copies it in reading order. A new Auto-hide tab in the settings window picks what to hide.

**Architecture:** Everything that decides *what* to hide and *what* text to copy is pure code in `snapline-overlay` (`scan.rs`), fed by plain `Recognized` values (lines of words with pixel boxes, and face boxes), so it is tested on every OS. The session gains hide boxes, a scanning state, and a held command: Copy, Save or Upload pressed while the scan is still running waits for it, so nothing leaves the PC unhidden. The app crate adds a `Recognizer`: Windows.Media.Ocr and Windows.Media.FaceAnalysis on Windows, and a "not available yet" stand-in elsewhere (Linux gets Tesseract in Phase 6). Scans run on one worker thread per monitor.

**Tech Stack:** Rust 2024 (stable); `windows` 0.62 (WinRT: `Media_Ocr`, `Media_FaceAnalysis`, `Graphics_Imaging`, `Storage_Streams`, `Globalization`, `Win32_System_WinRT`) with blocking `.join()` from windows-future 0.3; everything from Phases 1–3.

**Spec:** [docs/windows-linux.md](../windows-linux.md) (Phase 4), with the macOS app as the behaviour reference: `macOS/Sources/Snapline/Redaction.swift`, `TextExtraction.swift`, `OverlayView+Drawing.swift`, `OverlayView+Mouse.swift`, `OverlayView+Keyboard.swift`, `OverlayView+Toolbars.swift`, `Actions.swift` (`copyText`), and the Auto-hide tab in `Preferences.swift`.

## Decisions made for this phase

Barayuda's choices on 31 October 2026:

| Question | Choice |
|---|---|
| Keyboard-symbol repair (`SymbolRepair`) and the "check the copied text" popup with its symbol palette | **Deferred.** Windows apps rarely show ⌘ ⇧ ⌥ symbols. Copy Text copies the OCR text in reading order. |
| Where auto-hide is set | **A new Auto-hide tab** in the settings window, which becomes "Snapline settings" with Upload and Auto-hide tabs. Phase 5 adds the rest. |

Behaviour copied from macOS:
- **Scanning:** every capture is scanned when auto-hide is on and at least one category is ticked. All four categories are on by default.
- **Badge:** the size badge adds "scanning…" while the scan runs, then "hiding N".
- **Boxes in the selection:** found items show as solid black boxes. A click on one, with no tool selected, shows it with a dashed yellow outline (on screen only). Another click hides it again, and Ctrl+Z undoes a click.
- **Auto-hide button:** the eye button in the draw bar turns auto-hide off and on for this capture.
- **Exports wait for the scan:** Copy, Copy Text, Save, Save As, Upload and Search wait until the scan of the selected screen is done.
- **Box sizes:** text boxes grow by 3 × 2 points. Face boxes grow by 20 % sideways and 30 % up and down, to cover hair and chin.
- **Copy Text:** reads the *flattened* selection, so hidden areas stay hidden. It reports "Text copied" with the first line, or "No text found".

Choices made in this plan:
- **Windows OCR language:** the user's profile languages, falling back to English. Windows has no OCR for some languages, Indonesian among them; English OCR still reads Latin text and digits well, which is what the sensitive-text rules need.
- **Large images:** images larger than `OcrEngine::MaxImageDimension` are scaled down for OCR, and the boxes are scaled back up.
- **No OCR available:** this happens with no OCR language installed, on some Windows N editions, or on non-Windows during development. The capture still works, the badge says "auto-hide unavailable", and Copy Text explains what to install. Held commands are released, so nothing hangs.

Left for later: `SymbolRepair` and the review popup (if needed); Tesseract on Linux (Phase 6); the macOS "Copy Text: write keyboard symbols as words" option (Phase 5, with General preferences).

## Global Constraints

- Workspace `windows-linux/`, edition 2024, stable toolchain. `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` (pedantic) must pass. Crate roots may allow only `cast_possible_truncation`, `cast_sign_loss` and `cast_precision_loss`. Write empty checks as `assert_eq!(v, [])`.
- This Mac cannot compile the Windows target (`ring` needs MSVC). Windows-only code is checked by CI's windows-latest job. Push after each Windows-only task, so a compile error shows up early.
- `snapline-overlay` must not depend on winit, `windows` or any OS API. The OCR results it consumes are plain values.
- Text positions: `sensitive_text::matches` returns **byte** ranges. Convert to character counts before `selection_geometry::slice`. Phase 1 flagged this; Review Focus 1 tests it.
- Never export unhidden: every exporting command goes through `Session::request`, which holds it while the selected screen is scanning.
- Add crates with `cargo add`. This plan's Windows code matches `windows` 0.62.2. If a newer major comes in, fix the call sites, not the behaviour.
- Copy in plain English, sentence case, Windows names ("Settings → Time & language").
- Git: a commit per task, GPG-signed, plain sentence-case messages, no AI co-author or "generated with" lines. Check `git log --format='%h %G?'` shows `G` before pushing.

## Review Focus

1. **Text with accents, non-Latin letters or emoji before a sensitive value** (for example "Café: 4111 1111 1111 1111", or a key after "—"). The box must cover the value, not shift by the extra bytes. Test: `boxes_stay_on_the_value_after_multibyte_text` (Task 3).
2. **Copy, Save or Upload pressed right after the capture opens, before the scan finishes.** The command must wait, and the export must include the boxes. Tests: `commands_wait_for_the_scan`, `export_covers_what_the_scan_found` (Task 1).
3. **No OCR on the PC** (no OCR language, Windows N, a broken OCR service). The capture still works, the badge says so, held commands go ahead, and Copy Text explains what to install. Tests: `a_failed_scan_releases_held_commands_and_says_why` (Task 1), `failed_scan_shows_in_the_badge` (Task 2).
4. **Screens larger than Windows OCR's limit** (8K, or tall portrait monitors). The image must be scaled for OCR and the boxes scaled back exactly. Test: `big_images_are_scaled_for_ocr_and_boxes_scaled_back` (Task 3).
5. **Revealing a box, then undo, auto-hide off, or moving the selection.** Undo hides the box again. With auto-hide off, nothing is covered on screen or in the export. Boxes outside the selection are neither drawn nor exported. Tests: `clicking_a_box_shows_it_and_undo_hides_it_again`, `auto_hide_off_exports_everything` (Task 1).

---

## File structure

```
windows-linux/crates/
├── snapline-overlay/src/
│   ├── scan.rs                  new: Recognized, HideBox, hide_boxes, copy_text, ocr_scale
│   ├── session.rs               hide boxes, scanning state, held command, toggle by click, undo kinds
│   ├── toolbar.rs, icons.rs     Auto-hide (eye) and Copy Text buttons
│   ├── render.rs                black boxes, dashed revealed boxes, scan badge
│   └── popup.rs                 PopupState::Problem (red title) for Copy Text failures
└── snapline/src/
    ├── recognize.rs             new: Recognizer trait, system(); the OCR self-test on Windows
    ├── windows_ocr.rs           new, Windows only: Windows.Media.Ocr + FaceDetector
    ├── settings.rs              [auto_hide] table
    ├── upload_settings.rs       → settings window with Upload and Auto-hide tabs
    ├── app.rs, main.rs, platform.rs, actions.rs   wiring
```

---

### Task 1: Hide boxes in the session

**Files:**
- Create: `windows-linux/crates/snapline-overlay/src/scan.rs` (only `HideBox` in this task; Task 3 fills the rest). Add `pub mod scan;` to `lib.rs`.
- Modify: `windows-linux/crates/snapline-overlay/src/session.rs`

**Interfaces:**
- Produces:
  - `scan::HideBox { rect: Rect, category: SensitiveCategory, hidden: bool }` (`Clone`, `Copy`, `Debug`, `PartialEq`).
  - `Overlay` gains `pub hide_boxes: Vec<HideBox>`, `pub scanning: bool`, `pub scan_note: Option<String>`, and a private `undo: Vec<Undo>`.
  - `Session` gains `pub auto_hide: bool` (default true), `begin_scan(&mut self)`, `scan_done(&mut self, i: usize, result: Result<Vec<HideBox>, String>) -> Effect`, `visible_hide_boxes(&self, i) -> Vec<(usize, HideBox)>`.
  - `Command` gains `CopyText`.
  - `CursorKind` gains `Pointer`.
- Changes: `request` takes `&mut self` and holds an exporting command while the selected screen scans. `undo` undoes the last annotation *or* box click on the focused screen. `export` covers hidden visible boxes.

- [ ] **Step 1: Write the failing tests**

`scan.rs`:

```rust
//! From recognised text and faces to the boxes auto-hide covers, and to the text Copy Text copies.

use snapline_core::selection_geometry::Rect;
use snapline_core::sensitive_text::SensitiveCategory;

/// An area auto-hide found. `hidden` is false after the user clicked it to show it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HideBox {
    pub rect: Rect,
    pub category: SensitiveCategory,
    pub hidden: bool,
}
```

Add to `mod tests` in `session.rs`:

```rust
    use crate::scan::HideBox;
    use snapline_core::sensitive_text::SensitiveCategory;

    fn found(rect: Rect) -> Vec<HideBox> {
        vec![HideBox { rect, category: SensitiveCategory::Payment, hidden: true }]
    }

    fn black_at(image: &Pixmap, x: u32, y: u32) -> bool {
        let c = image.pixel(x, y).unwrap();
        c.red() == 0 && c.green() == 0 && c.blue() == 0
    }

    // Review Focus 2.
    #[test]
    fn commands_wait_for_the_scan() {
        let mut s = selected(1.0);
        s.begin_scan();
        assert_eq!(s.key(Key::Char('c'), ctrl()).command, None, "held while scanning");
        let effect = s.scan_done(0, Ok(found(Rect::new(20.0, 20.0, 30.0, 10.0))));
        assert_eq!(effect.command, Some(Command::Copy), "released when the scan is done");
        assert_eq!(s.scan_done(1, Ok(vec![])).command, None, "released only once");
    }

    // Review Focus 2.
    #[test]
    fn export_covers_what_the_scan_found() {
        let mut s = selected(1.0);
        s.begin_scan();
        s.scan_done(0, Ok(found(Rect::new(20.0, 20.0, 30.0, 10.0))));
        let image = s.export().unwrap(); // selection starts at (10, 10)
        assert!(black_at(&image, 15, 15), "the box is black in the export");
        assert!(!black_at(&image, 60, 40), "the rest is untouched");
    }

    // Review Focus 3.
    #[test]
    fn a_failed_scan_releases_held_commands_and_says_why() {
        let mut s = selected(1.0);
        s.begin_scan();
        s.key(Key::Char('s'), ctrl());
        let effect = s.scan_done(0, Err("auto-hide unavailable".into()));
        assert_eq!(effect.command, Some(Command::Save));
        assert_eq!(s.overlays[0].scan_note.as_deref(), Some("auto-hide unavailable"));
        assert!(!s.overlays[0].scanning);
    }

    // Review Focus 5.
    #[test]
    fn clicking_a_box_shows_it_and_undo_hides_it_again() {
        let mut s = selected(1.0);
        s.begin_scan();
        s.scan_done(0, Ok(found(Rect::new(20.0, 20.0, 30.0, 10.0))));
        drag(&mut s, 0, pt(25.0, 25.0), pt(25.0, 25.0));
        assert!(!s.overlays[0].hide_boxes[0].hidden, "a click shows it");
        assert!(!black_at(&s.export().unwrap(), 15, 15));
        assert_eq!(s.undo(), Effect::one(0));
        assert!(s.overlays[0].hide_boxes[0].hidden, "undo hides it again");
    }

    // Review Focus 5.
    #[test]
    fn auto_hide_off_exports_everything() {
        let mut s = selected(1.0);
        s.begin_scan();
        s.scan_done(0, Ok(found(Rect::new(20.0, 20.0, 30.0, 10.0))));
        s.auto_hide = false;
        assert_eq!(s.visible_hide_boxes(0), []);
        assert!(!black_at(&s.export().unwrap(), 15, 15));
    }

    #[test]
    fn boxes_outside_the_selection_are_ignored() {
        let mut s = selected(1.0);
        s.begin_scan();
        s.scan_done(0, Ok(found(Rect::new(300.0, 200.0, 30.0, 10.0))));
        assert_eq!(s.visible_hide_boxes(0), []);
    }

    #[test]
    fn the_pointer_shows_a_hand_over_a_box() {
        let mut s = selected(1.0);
        s.begin_scan();
        s.scan_done(0, Ok(found(Rect::new(20.0, 20.0, 30.0, 10.0))));
        s.pointer_move(0, pt(25.0, 25.0));
        assert_eq!(s.cursor(0), CursorKind::Pointer);
    }

    #[test]
    fn undo_still_removes_drawings_in_order() {
        let mut s = selected(1.0);
        s.begin_scan();
        s.scan_done(0, Ok(found(Rect::new(20.0, 20.0, 30.0, 10.0))));
        drag(&mut s, 0, pt(25.0, 25.0), pt(25.0, 25.0)); // show the box
        s.tool = Some(Tool::Rectangle);
        drag(&mut s, 0, pt(60.0, 30.0), pt(90.0, 50.0));
        s.undo();
        assert_eq!(s.overlays[0].annotations, [], "the drawing goes first");
        assert!(!s.overlays[0].hide_boxes[0].hidden);
        s.undo();
        assert!(s.overlays[0].hide_boxes[0].hidden, "then the box click");
    }
```

Run `cd windows-linux && cargo test -p snapline-overlay` → fails (no `begin_scan`, `scan_done`, …).

- [ ] **Step 2: Implement**

In `session.rs`:

```rust
/// What `undo` takes back on one screen, newest last.
#[derive(Clone, Copy, Debug)]
enum Undo {
    Annotation,
    /// A click that showed or hid hide box `n`.
    Toggle(usize),
}
```

- `Overlay`: add `pub hide_boxes: Vec<HideBox>`, `pub scanning: bool`, `/// Why there are no boxes, when the scan could not run (shown in the size badge). pub scan_note: Option<String>`, and `undo: Vec<Undo>`. Initialise them empty, false, `None`, empty.
- `Session`: add `pub auto_hide: bool` (true in `new`) and `pending: Option<Command>` (None).
- Every `o.annotations.push(…)` (in `pointer_up` and `commit_text`) also does `o.undo.push(Undo::Annotation)`.
- Add:

```rust
    /// Starts the auto-hide scan on every screen: exporting commands wait until it is done.
    pub fn begin_scan(&mut self) {
        for o in &mut self.overlays {
            o.scanning = true;
        }
    }

    /// The scan of screen `i` finished: its boxes, or why it could not run. Releases a held command once the
    /// screen that holds the selection is done.
    pub fn scan_done(&mut self, i: usize, result: Result<Vec<HideBox>, String>) -> Effect {
        let o = &mut self.overlays[i];
        o.scanning = false;
        match result {
            Ok(boxes) => o.hide_boxes = boxes,
            Err(note) => o.scan_note = Some(note),
        }
        let ready = self.selected().is_none_or(|k| !self.overlays[k].scanning);
        Effect { command: if ready { self.pending.take() } else { None }, ..Effect::all() }
    }

    /// The boxes inside the selection of screen `i`, with their index; none when auto-hide is off.
    #[must_use]
    pub fn visible_hide_boxes(&self, i: usize) -> Vec<(usize, HideBox)> {
        let o = &self.overlays[i];
        let Some(sel) = o.selection.filter(|_| self.auto_hide) else { return Vec::new() };
        o.hide_boxes.iter().copied().enumerate().filter(|(_, b)| intersects(b.rect, sel)).collect()
    }
```

  with `fn intersects(a: Rect, b: Rect) -> bool { a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height }`.
- `request` becomes `pub(crate) fn request(&mut self, c: Command) -> Effect`:

```rust
        let Some(i) = self.selected() else { return Effect::beep() };
        if self.auto_hide && self.overlays[i].scanning {
            self.pending = Some(c); // never export before auto-hide has found what to hide
            return Effect::all();
        }
        Effect::command(c)
```

- `pointer_up`, in the `match o.drag`, add before `_ => {}`: for `Drag::Moving(..)` without a drag, toggle the topmost visible box under the pointer:

```rust
            Drag::Moving(..) if !o.did_drag => {
                let hit = self_visible.iter().rev().find(|(_, b)| contains(b.rect, o.pointer)).map(|(k, _)| *k);
                if let Some(k) = hit {
                    o.hide_boxes[k].hidden = !o.hide_boxes[k].hidden;
                    o.undo.push(Undo::Toggle(k));
                }
            }
```

  Here `self_visible` is `self.visible_hide_boxes(i)`, computed **before** borrowing `o` mutably. Restructure `pointer_up` so that `let visible = self.visible_hide_boxes(i);` comes first.
- `undo`:

```rust
    pub fn undo(&mut self) -> Effect {
        let i = self.focused;
        let o = &mut self.overlays[i];
        match o.undo.pop() {
            Some(Undo::Annotation) => {
                o.annotations.pop();
                Effect::one(i)
            }
            Some(Undo::Toggle(k)) => {
                o.hide_boxes[k].hidden = !o.hide_boxes[k].hidden;
                Effect::one(i)
            }
            None => Effect::beep(),
        }
    }
```

- `export`: cover the hidden visible boxes first, then the drawings (as macOS does):

```rust
    pub fn export(&mut self) -> Option<Pixmap> {
        self.commit_text();
        let i = self.selected()?;
        let covers = self.visible_hide_boxes(i).into_iter().filter(|(_, b)| b.hidden).map(|(_, b)| Annotation {
            shape: Shape::Hide(b.rect),
            color: crate::color::BLACK,
            width: 0.0,
            marker: false,
        });
        let o = &self.overlays[i];
        let all: Vec<Annotation> = covers.chain(o.annotations.iter().cloned()).collect();
        render::render_selection(&o.screen, o.selection?, &all, &self.font)
    }
```

- `cursor`: in the `contains(sel, p)` branch, when `self.tool` is `None` and a visible box contains `p`, return `CursorKind::Pointer`.
- `Command`: add `CopyText`. `CursorKind`: add `Pointer`.

Fix the app crate's matches so it still compiles:
- `actions::run`: `Command::CopyText` → `Ok(None)`.
- `app.rs` `cursor_icon`: `CursorKind::Pointer => CursorIcon::Pointer`.

- [ ] **Step 3: Run the tests**

Run: `cd windows-linux && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: all pass, including every Phase 2–3 overlay test.

- [ ] **Step 4: Commit**

```bash
git add windows-linux
git commit -m "Hold exports until auto-hide has scanned, and let a click show a hidden box"
```

---

### Task 2: Drawing the boxes, the scan badge, and the new buttons

**Files:**
- Modify: `windows-linux/crates/snapline-overlay/src/render.rs`, `toolbar.rs`, `icons.rs`, `session.rs`, `popup.rs`

**Interfaces:**
- Produces:
  - `toolbar::Button` gains `AutoHide` and `CopyText`.
  - `DRAW_BAR` becomes `[8 tools, Color, AutoHide, Undo]`, as on macOS.
  - `MAIN_BAR` becomes `[Upload, Search, Copy, CopyText, Save, Close]`.
  - Ctrl+Shift+C runs `Command::CopyText`; Ctrl+C stays Copy.
  - `popup::PopupState::Problem { title: String, detail: String }`: a red title and no buttons.

Drawing (macOS `OverlayView+Drawing.swift`):
- On screen and in the export, a visible hidden box is solid black.
- On screen only, a visible shown box gets a dashed yellow (#FFCC00) 1-point outline, 4 on and 3 off.
- The size badge adds:
  - "   ·   scanning…" while scanning;
  - "   ·   hiding N" when N visible boxes are hidden;
  - "   ·   " + the scan note when the scan could not run.

  Each only applies when auto-hide is on.

- [ ] **Step 1: Write the failing tests**

`render.rs` tests:

```rust
    use crate::scan::HideBox;
    use snapline_core::sensitive_text::SensitiveCategory;

    fn with_box(hidden: bool) -> Session {
        let mut s = screen_session(1);
        select(&mut s, Rect::new(100.0, 100.0, 150.0, 80.0));
        s.begin_scan();
        let b = HideBox { rect: Rect::new(120.0, 120.0, 40.0, 12.0), category: SensitiveCategory::Payment, hidden };
        s.scan_done(0, Ok(vec![b]));
        s
    }

    #[test]
    fn hidden_boxes_are_black_and_shown_ones_are_outlined() {
        assert_eq!(rgb(&rendered(&with_box(true), 0), 140, 126), (0, 0, 0));
        let shown = rendered(&with_box(false), 0);
        assert_eq!(rgb(&shown, 140, 126), (255, 255, 255), "inside stays visible");
        let (r, g, b) = rgb(&shown, 121, 120);
        assert!(r > 200 && g > 160 && b < 80, "yellow dashed edge: {:?}", (r, g, b));
    }

    // Review Focus 3.
    #[test]
    fn failed_scan_shows_in_the_badge() {
        let mut s = screen_session(1);
        select(&mut s, Rect::new(100.0, 100.0, 150.0, 80.0));
        let plain = rendered(&s, 0);
        s.begin_scan();
        s.scan_done(0, Err("auto-hide unavailable".into()));
        let noted = rendered(&s, 0);
        // The badge sits above the selection and grows to the right with the note.
        let differs = (180..400).any(|x| rgb(&plain, x, 86) != rgb(&noted, x, 86));
        assert!(differs, "the note should widen the badge");
    }
```

`toolbar.rs`: add `Button::AutoHide` and `Button::CopyText` to the button list in `bars_never_overlap`'s assumptions (no change needed). Add:

```rust
    #[test]
    fn the_bars_carry_the_new_buttons() {
        assert_eq!(DRAW_BAR[8..], [Button::Color, Button::AutoHide, Button::Undo]);
        assert_eq!(MAIN_BAR[3], Button::CopyText);
    }
```

`session.rs`:

```rust
    #[test]
    fn ctrl_shift_c_copies_text_and_the_eye_turns_auto_hide_off() {
        let mut s = selected(1.0);
        let shift = Mods { ctrl: true, shift: true, alt: false };
        assert_eq!(s.key(Key::Char('C'), shift).command, Some(Command::CopyText));
        assert_eq!(s.key(Key::Char('c'), ctrl()).command, Some(Command::Copy));
        click_button(&mut s, Button::AutoHide, Mods::default());
        assert!(!s.auto_hide);
    }
```

`popup.rs`:

```rust
    #[test]
    fn problems_have_no_buttons_but_close() {
        let font = TextFont::bundled();
        let p = Popup::new(PopupState::Problem { title: "Text not copied".into(), detail: "No OCR.".into() });
        let kinds: Vec<_> = p.buttons(&font, 1.0).iter().map(|(b, _)| *b).collect();
        assert_eq!(kinds, [PopupButton::Close]);
    }
```

Run → fails.

- [ ] **Step 2: Implement**

- `toolbar.rs`:
  - Add the buttons, with tips `AutoHide => "Auto-hide sensitive data (click a black box to show it)"` and `CopyText => "Copy text (Ctrl+Shift+C), read on this PC"`.
  - Update `DRAW_BAR` (11 entries) and `MAIN_BAR` (6 entries).
- `session.rs`:
  - `press`: `Button::AutoHide` → `self.auto_hide = !self.auto_hide; Effect::all()`. `Button::CopyText` → `self.request(Command::CopyText)`.
  - In `key`'s Ctrl match, change the `'c'` arm to `'c' => self.request(if mods.shift { Command::CopyText } else { Command::Copy }),`.
- `icons.rs` `outline()`:

```rust
        Button::AutoHide => {
            // An eye with a slash.
            pb.push_oval(tiny_skia::Rect::from_xywh(3.0, 7.0, 18.0, 10.0).unwrap_or(tiny_skia::Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap()));
            pb.push_circle(12.0, 12.0, 2.5);
            pb.move_to(4.0, 20.0);
            pb.line_to(20.0, 4.0);
        }
        Button::CopyText => {
            // Viewfinder corners around a T.
            for [a, b, c] in [[(4.0, 9.0), (4.0, 4.0), (9.0, 4.0)], [(15.0, 4.0), (20.0, 4.0), (20.0, 9.0)],
                              [(20.0, 15.0), (20.0, 20.0), (15.0, 20.0)], [(9.0, 20.0), (4.0, 20.0), (4.0, 15.0)]] {
                pb.move_to(a.0, a.1);
                pb.line_to(b.0, b.1);
                pb.line_to(c.0, c.1);
            }
            pb.move_to(8.5, 9.0);
            pb.line_to(15.5, 9.0);
            pb.move_to(12.0, 9.0);
            pb.line_to(12.0, 16.0);
        }
```

  Write the oval without `unwrap_or(... unwrap())`: use `if let Some(r) = tiny_skia::Rect::from_xywh(3.0, 7.0, 18.0, 10.0) { pb.push_oval(r); }`.
- `render.rs`:
  - In `render_screen`, after the annotations and before dimming, draw the visible boxes:

```rust
    for (_, b) in session.visible_hide_boxes(i) {
        if b.hidden {
            fill_rect(out, b.rect, BLACK, 255);
        } else if let Some(path) = sk_rect(b.rect).map(PathBuilder::from_rect) {
            let mut dashed = stroke((1.0 * scale) as f32, LineCap::Butt);
            dashed.dash = tiny_skia::StrokeDash::new(vec![(4.0 * scale) as f32, (3.0 * scale) as f32], 0.0);
            out.stroke_path(&path, &paint(YELLOW, 255), &dashed, Transform::identity(), None);
        }
    }
```

    with `const YELLOW: Rgb = Rgb(0xFF, 0xCC, 0x00);`.
  - Build the badge label with:

```rust
    let mut label = format!("{} × {}", sel.width.round() as i64, sel.height.round() as i64);
    if session.auto_hide {
        if o.scanning {
            label.push_str("   ·   scanning…");
        } else if let Some(note) = &o.scan_note {
            label.push_str("   ·   ");
            label.push_str(note);
        } else {
            let hidden = session.visible_hide_boxes(i).iter().filter(|(_, b)| b.hidden).count();
            if hidden > 0 {
                label.push_str(&format!("   ·   hiding {hidden}"));
            }
        }
    }
```

  - In `draw_bars`, the eye is drawn active (violet light) while `session.auto_hide` is on:
    `let active = matches!(button, Button::Tool(t) if session.tool == Some(*t)) || (*button == Button::AutoHide && session.auto_hide);`
- `popup.rs`:
  - Add the variant `Problem { title: String, detail: String }`.
  - `title()` → `(title, RED)`. `lines()` wraps `detail`. `actions()` → `&[]`.

- [ ] **Step 3: Run the tests and look at it**

Run: `cd windows-linux && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: all pass. If `bars_never_overlap` or `full_screen_selection_keeps_toolbars_on_screen` now fail because the draw bar is taller (11 buttons), fix the layout, not the test, and record the ruling.

Then write a throwaway test (do not commit it) that renders a selection with one hidden box, one shown box, the badge reading "hiding 1", and the eye button hovered. Save it to `/tmp/phase4.png` and look at it.

- [ ] **Step 4: Commit**

```bash
git add windows-linux
git commit -m "Draw auto-hide boxes and the scan state, and add the auto-hide and Copy Text buttons"
```

---

### Task 3: From recognised text to boxes, and to Copy Text

**Files:**
- Modify: `windows-linux/crates/snapline-overlay/src/scan.rs`

**Interfaces:**
- Consumes: `snapline_core::{sensitive_text::{matches, SensitiveCategory}, selection_geometry::{slice, Rect}, reading_order}`.
- Produces:
  - `scan::Word { text: String, rect: Rect }` and `scan::Line { words: Vec<Word> }`.
  - `scan::Recognized { lines: Vec<Line>, faces: Vec<Rect> }` (all `Clone`, `Debug`, `Default`, `PartialEq`). Every rect is in the pixels of the image that was read.
  - `scan::hide_boxes(&Recognized, categories: &[SensitiveCategory], scale: f64) -> Vec<HideBox>`.
  - `scan::copy_text(&Recognized, width: f64, height: f64) -> String`.
  - `scan::ocr_scale(width: u32, height: u32, max_dimension: u32) -> f64` (1.0 or less).
  - `Recognized::scaled(self, factor: f64) -> Recognized` (multiplies every rect).

How a match becomes a box (macOS `Redaction.matchBox`):
- The line's text is its words joined by single spaces, so every word's byte offset is known.
- A match that lies inside one word, like the value in `KEY=value`, is cut out of that word's box with `slice`. `slice` takes **character** positions, so convert the byte offsets first.
- A match that spans several words gets the union of their boxes.
- Every text box then grows by 3 × 2 points (times `scale`).

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: [SensitiveCategory; 3] = [SensitiveCategory::Contact, SensitiveCategory::Payment, SensitiveCategory::Secret];

    /// One line whose words are 10 px per character, 4 px apart, starting at x = 100, y = 50, 20 px tall.
    fn line(text: &str) -> Line {
        let mut x = 100.0;
        let words = text
            .split(' ')
            .map(|w| {
                let width = 10.0 * w.chars().count() as f64;
                let word = Word { text: w.into(), rect: Rect::new(x, 50.0, width, 20.0) };
                x += width + 4.0;
                word
            })
            .collect();
        Line { words }
    }

    fn one(text: &str) -> Recognized {
        Recognized { lines: vec![line(text)], faces: vec![] }
    }

    #[test]
    fn a_card_number_spanning_words_gets_one_box_over_all_of_them() {
        let boxes = hide_boxes(&one("Card 4111 1111 1111 1111"), &TEXT, 1.0);
        assert_eq!(boxes.len(), 1);
        let b = boxes[0];
        assert_eq!(b.category, SensitiveCategory::Payment);
        assert!(b.hidden);
        // "Card" is 40 px + 4 gap: the card starts at x = 144; four 40 px groups with 4 px gaps end at 316.
        assert!((b.rect.x - (144.0 - 3.0)).abs() < 1e-9 && (b.rect.x + b.rect.width - (316.0 + 3.0)).abs() < 1e-9, "{b:?}");
        assert!((b.rect.y - 48.0).abs() < 1e-9 && (b.rect.height - 24.0).abs() < 1e-9);
    }

    #[test]
    fn part_of_a_word_is_cut_out_of_the_word_box() {
        let key = format!("API_KEY={}", ["sk_live_", "4eC39HqLyjWDarjtT1zdp7dc"].concat());
        let boxes = hide_boxes(&one(&key), &TEXT, 1.0);
        assert_eq!(boxes.len(), 1);
        let word = line(&key).words[0].rect;
        assert!(boxes[0].rect.x > word.x + 30.0, "the box starts after \"API_KEY=\": {:?}", boxes[0].rect);
    }

    // Review Focus 1.
    #[test]
    fn boxes_stay_on_the_value_after_multibyte_text() {
        let plain = hide_boxes(&one("Cafe: 4111 1111 1111 1111"), &TEXT, 1.0);
        let accented = hide_boxes(&one("Café: 4111 1111 1111 1111"), &TEXT, 1.0);
        assert_eq!(plain[0].rect, accented[0].rect, "\"é\" is 2 bytes but 1 character");
        let key = format!("—key={}", ["sk_live_", "4eC39HqLyjWDarjtT1zdp7dc"].concat());
        let word = line(&key).words[0].rect;
        let b = hide_boxes(&one(&key), &TEXT, 1.0)[0].rect;
        assert!(b.x > word.x + 30.0 && b.x + b.width <= word.x + word.width + 15.0, "{b:?} in {word:?}");
    }

    #[test]
    fn faces_grow_and_only_when_asked() {
        let found = Recognized { lines: vec![], faces: vec![Rect::new(100.0, 100.0, 50.0, 60.0)] };
        assert_eq!(hide_boxes(&found, &TEXT, 1.0), []);
        let boxes = hide_boxes(&found, &[SensitiveCategory::Face], 1.0);
        assert_eq!(boxes[0].rect, Rect::new(90.0, 82.0, 70.0, 96.0));
        assert_eq!(boxes[0].category, SensitiveCategory::Face);
    }

    #[test]
    fn copy_text_reads_lines_in_order() {
        let found = Recognized {
            lines: vec![
                Line { words: vec![Word { text: "second".into(), rect: Rect::new(10.0, 60.0, 60.0, 20.0) }] },
                Line { words: vec![Word { text: "first".into(), rect: Rect::new(10.0, 20.0, 50.0, 20.0) }] },
            ],
            faces: vec![],
        };
        assert_eq!(copy_text(&found, 200.0, 100.0), "first\nsecond");
        assert_eq!(copy_text(&Recognized::default(), 200.0, 100.0), "");
    }

    // Review Focus 4.
    #[test]
    fn big_images_are_scaled_for_ocr_and_boxes_scaled_back() {
        assert!((ocr_scale(7680, 4320, 10_000) - 1.0).abs() < f64::EPSILON);
        let factor = ocr_scale(15_360, 8640, 10_000);
        assert!((factor - 10_000.0 / 15_360.0).abs() < 1e-12);
        let read = Recognized { lines: vec![line("x")], faces: vec![Rect::new(10.0, 20.0, 30.0, 40.0)] };
        let back = read.scaled(1.0 / factor);
        assert!((back.faces[0].x - 10.0 / factor).abs() < 1e-9);
        assert!((back.lines[0].words[0].rect.width - 10.0 / factor).abs() < 1e-9);
    }
}
```

Run `cargo test -p snapline-overlay scan` → fails.

- [ ] **Step 2: Implement**

Above the tests (keep `HideBox`):

```rust
use snapline_core::reading_order;
use snapline_core::selection_geometry::slice;
use snapline_core::sensitive_text::matches;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Word {
    pub text: String,
    pub rect: Rect,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub words: Vec<Word>,
}

/// What text recognition and face detection found, in the pixels of the image that was read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recognized {
    pub lines: Vec<Line>,
    pub faces: Vec<Rect>,
}

fn grow(r: Rect, dx: f64, dy: f64) -> Rect {
    Rect::new(r.x - dx, r.y - dy, r.width + 2.0 * dx, r.height + 2.0 * dy)
}

fn scale_rect(r: Rect, f: f64) -> Rect {
    Rect::new(r.x * f, r.y * f, r.width * f, r.height * f)
}

impl Line {
    /// The words joined by single spaces, and each word's byte range in that text.
    fn text(&self) -> (String, Vec<std::ops::Range<usize>>) {
        let mut text = String::new();
        let mut ranges = Vec::new();
        for (k, w) in self.words.iter().enumerate() {
            if k > 0 {
                text.push(' ');
            }
            ranges.push(text.len()..text.len() + w.text.len());
            text.push_str(&w.text);
        }
        (text, ranges)
    }
}

impl Recognized {
    /// Every rect multiplied by `factor` (to map OCR on a scaled-down image back to the screen).
    #[must_use]
    pub fn scaled(self, factor: f64) -> Self {
        Self {
            lines: self
                .lines
                .into_iter()
                .map(|l| Line {
                    words: l.words.into_iter().map(|w| Word { rect: scale_rect(w.rect, factor), ..w }).collect(),
                })
                .collect(),
            faces: self.faces.into_iter().map(|r| scale_rect(r, factor)).collect(),
        }
    }
}

/// How much to scale an image so its longer side fits Windows OCR's limit (1.0 when it already fits).
#[must_use]
pub fn ocr_scale(width: u32, height: u32, max_dimension: u32) -> f64 {
    let longer = width.max(height).max(1);
    (f64::from(max_dimension) / f64::from(longer)).min(1.0)
}

/// The boxes auto-hide covers: sensitive text in `categories`, and faces when `Face` is one of them.
/// `scale` is pixels per point, for the padding.
#[must_use]
pub fn hide_boxes(found: &Recognized, categories: &[SensitiveCategory], scale: f64) -> Vec<HideBox> {
    let text_categories: Vec<SensitiveCategory> =
        categories.iter().copied().filter(|c| *c != SensitiveCategory::Face).collect();
    let mut boxes = Vec::new();
    if !text_categories.is_empty() {
        for line in &found.lines {
            let (text, ranges) = line.text();
            for m in matches(&text, &text_categories) {
                let touched: Vec<usize> =
                    (0..ranges.len()).filter(|&k| ranges[k].start < m.range.end && m.range.start < ranges[k].end).collect();
                let rect = match touched.as_slice() {
                    [] => continue,
                    [k] => {
                        let (word, range) = (&line.words[*k], &ranges[*k]);
                        let start = m.range.start.max(range.start);
                        let end = m.range.end.min(range.end);
                        // `slice` counts characters, not bytes.
                        let chars = text[range.start..start].chars().count()..text[range.start..end].chars().count();
                        if chars.start == 0 && chars.end == word.text.chars().count() {
                            word.rect
                        } else {
                            slice(word.rect, chars, word.text.chars().count())
                        }
                    }
                    many => many.iter().map(|&k| line.words[k].rect).reduce(Rect::union).unwrap_or_default(),
                };
                boxes.push(HideBox { rect: grow(rect, 3.0 * scale, 2.0 * scale), category: m.category, hidden: true });
            }
        }
    }
    if categories.contains(&SensitiveCategory::Face) {
        for face in &found.faces {
            // Face boxes are tight around the features; grow them to cover hair and chin.
            boxes.push(HideBox {
                rect: grow(*face, face.width * 0.2, face.height * 0.3),
                category: SensitiveCategory::Face,
                hidden: true,
            });
        }
    }
    boxes
}

/// The recognised text in reading order (lines joined into paragraphs as on macOS), or "".
#[must_use]
pub fn copy_text(found: &Recognized, width: f64, height: f64) -> String {
    // `reading_order` takes Vision-style boxes: normalised, origin at the bottom left.
    let pieces: Vec<(String, Rect)> = found
        .lines
        .iter()
        .filter_map(|line| {
            let r = line.words.iter().map(|w| w.rect).reduce(Rect::union)?;
            let normalized = Rect::new(r.x / width, 1.0 - (r.y + r.height) / height, r.width / width, r.height / height);
            Some((line.text().0, normalized))
        })
        .collect();
    reading_order::text(&pieces)
}
```

If `Rect::union` takes `self, other` by value, `reduce(Rect::union)` works directly. If it doesn't, use `reduce(|a, b| a.union(b))`.

- [ ] **Step 3: Run the tests**

Run: `cd windows-linux && cargo test -p snapline-overlay && cargo clippy -p snapline-overlay --all-targets -- -D warnings`
Expected: pass. In `a_card_number_spanning_words_gets_one_box_over_all_of_them`, the expected edges follow from the test's 10 px per character. Check the arithmetic against `line()`, not the code.

- [ ] **Step 4: Commit**

```bash
git add windows-linux
git commit -m "Turn recognised text and faces into hide boxes and Copy Text"
```

---

### Task 4: Auto-hide settings, and the settings window's two tabs

**Files:**
- Modify: `windows-linux/crates/snapline/src/settings.rs`, `upload_settings.rs`, `main.rs`, `platform.rs`, `app.rs` (menu label and the open call)

**Interfaces:**
- Produces:
  - `settings::AutoHide { enabled, contact, payment, secret, face: bool }` (all true by default; serde default), with `categories(&self) -> Vec<SensitiveCategory>` (empty when off).
  - `Settings` gains `pub auto_hide: AutoHide`, stored as an `[auto_hide]` table.
  - `UploadForm` gains `pub auto_hide: AutoHide` and `save_auto_hide(&mut self, path: &Path)`.
  - The window is titled "Snapline settings" and has an Upload tab and an Auto-hide tab.
  - It opens with `snapline.exe --settings`; `--upload-settings` still works. `platform::open_upload_settings` is renamed `open_settings`.
  - The tray item becomes "Settings…".

The Auto-hide tab copies the macOS one:
- A toggle: "Hide sensitive data automatically on every capture".
- A group "What to hide" with one checkbox per category, using `SensitiveCategory::title()`. The checkboxes are greyed out when the toggle is off.
- The note: "Runs on this PC only. Found items are covered with solid black boxes; click a box in the capture to show it again. This is a safety net, not a guarantee: small, faint or rotated text can be missed, so check before you share."
- Changes save straight away, as on macOS.

- [ ] **Step 1: Write the failing tests**

`settings.rs`:

```rust
    #[test]
    fn auto_hide_defaults_to_everything_and_reads_partial_tables() {
        assert_eq!(Settings::default().auto_hide.categories(), SensitiveCategory::ALL);
        let path = temp_file("auto-hide", "[auto_hide]\nface = false\n");
        let (s, problem) = Settings::load(&path);
        assert!(problem.is_none());
        assert_eq!(s.auto_hide.categories(), [SensitiveCategory::Contact, SensitiveCategory::Payment, SensitiveCategory::Secret]);
        let off = AutoHide { enabled: false, ..AutoHide::default() };
        assert_eq!(off.categories(), []);
    }
```

`upload_settings.rs`:

```rust
    #[test]
    fn auto_hide_changes_save_without_touching_upload_settings() {
        let (path, store) = (temp_path("auto-hide"), MemoryStore::default());
        s3_form().save(&path, &store);
        let mut form = UploadForm::load(&Settings::load(&path).0, &store);
        form.auto_hide.face = false;
        form.save_auto_hide(&path);
        let saved = Settings::load(&path).0;
        assert!(!saved.auto_hide.face && saved.auto_hide.payment);
        assert_eq!(saved.s3_bucket, "shots");
    }
```

Run → fails.

- [ ] **Step 2: Implement**

`settings.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoHide {
    pub enabled: bool,
    pub contact: bool,
    pub payment: bool,
    pub secret: bool,
    pub face: bool,
}

impl Default for AutoHide {
    fn default() -> Self {
        Self { enabled: true, contact: true, payment: true, secret: true, face: true }
    }
}

impl AutoHide {
    /// The categories to hide on every capture (none when auto-hide is off).
    #[must_use]
    pub fn categories(&self) -> Vec<SensitiveCategory> {
        if !self.enabled {
            return Vec::new();
        }
        let on = [self.contact, self.payment, self.secret, self.face];
        SensitiveCategory::ALL.into_iter().zip(on).filter(|(_, on)| *on).map(|(c, _)| c).collect()
    }
}
```

Add `pub auto_hide: AutoHide,` to `Settings`, `auto_hide: AutoHide::default(),` to its `Default`, and `use snapline_core::sensitive_text::SensitiveCategory;`. `SensitiveCategory::ALL` is ordered Contact, Payment, Secret, Face.

`upload_settings.rs`:
- `UploadForm`: add `pub auto_hide: AutoHide` (loaded from `settings.auto_hide`).
- Add the method:

```rust
    /// Saves the auto-hide choices at once, as macOS does; reports only a failure.
    pub fn save_auto_hide(&mut self, path: &Path) {
        let choice = self.auto_hide;
        if let Err(e) = Settings::update(path, |s| s.auto_hide = choice) {
            self.status = Some((e, false));
        }
    }
```

- `SettingsApp`: add `tab: Tab` with `#[derive(Clone, Copy, PartialEq, Eq)] enum Tab { Upload, AutoHide }`. In `ui()`, above the existing content, add a tab row: `ui.selectable_value(&mut self.tab, Tab::Upload, "Upload"); ui.selectable_value(&mut self.tab, Tab::AutoHide, "Auto-hide");`. Move the current Upload content into `fn upload_tab(&mut self, ui)`.
- Add:

```rust
    fn auto_hide_tab(&mut self, ui: &mut eframe::egui::Ui) {
        let before = self.form.auto_hide;
        let a = &mut self.form.auto_hide;
        ui.checkbox(&mut a.enabled, "Hide sensitive data automatically on every capture");
        ui.add_space(6.0);
        ui.label(eframe::egui::RichText::new("What to hide").strong());
        ui.add_enabled_ui(a.enabled, |ui| {
            ui.checkbox(&mut a.contact, SensitiveCategory::Contact.title());
            ui.checkbox(&mut a.payment, SensitiveCategory::Payment.title());
            ui.checkbox(&mut a.secret, SensitiveCategory::Secret.title());
            ui.checkbox(&mut a.face, SensitiveCategory::Face.title());
        });
        ui.add_space(6.0);
        ui.small(
            "Runs on this PC only. Found items are covered with solid black boxes; click a box in the capture to \
             show it again. This is a safety net, not a guarantee: small, faint or rotated text can be missed, so \
             check before you share.",
        );
        if self.form.auto_hide != before {
            self.form.save_auto_hide(&self.path);
        }
    }
```

- Title and app name become "Snapline settings". The mutex name becomes `Local\dev.barayuda.snapline.settings`.

`main.rs`: route `--settings` or `--upload-settings` to `upload_settings::run_window()`. `platform.rs`: rename `open_upload_settings` to `open_settings`, which spawns with `--settings`. `app.rs`: menu label "Settings…", and update the calls.

- [ ] **Step 3: Run the tests and lint**

Run: `cd windows-linux && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add windows-linux
git commit -m "Add an Auto-hide tab to the settings window"
```

---

### Task 5: Windows text recognition and face detection

**Files:**
- Create: `windows-linux/crates/snapline/src/recognize.rs`, `windows-linux/crates/snapline/src/windows_ocr.rs` (`#[cfg(windows)]`)
- Modify: `main.rs` (`mod recognize; #[cfg(windows)] mod windows_ocr;`), `Cargo.toml`

**Interfaces:**
- Consumes: `snapline_overlay::scan::{Recognized, Line, Word, ocr_scale}`, `snapline_overlay::tiny_skia::Pixmap`.
- Produces:
  - `recognize::Recognizer` (`fn recognize(&self, image: &Pixmap, faces: bool) -> Result<Recognized, String>`).
  - `recognize::system() -> Box<dyn Recognizer>`.
  - `recognize::UNAVAILABLE: &str` (the short badge note, "auto-hide unavailable").

- [ ] **Step 1: Add the `windows` crate (Windows only)**

```bash
cd windows-linux
cargo add -p snapline windows@0.62 --target 'cfg(windows)' --features Media_Ocr,Media_FaceAnalysis,Graphics_Imaging,Storage_Streams,Globalization,Foundation_Collections,Win32_System_WinRT
```

- [ ] **Step 2: The trait and the stand-in, test first**

`recognize.rs`:

```rust
//! Text recognition and face detection on this PC: Windows OCR on Windows; elsewhere not yet (Linux: Phase 6).

use snapline_overlay::scan::Recognized;
use snapline_overlay::tiny_skia::Pixmap;

/// Shown in the size badge when auto-hide could not run.
pub const UNAVAILABLE: &str = "auto-hide unavailable";

pub trait Recognizer {
    /// Reads `image`; finds faces too when `faces` is true. The error is a sentence for the user.
    fn recognize(&self, image: &Pixmap, faces: bool) -> Result<Recognized, String>;
}

struct NotYet;

impl Recognizer for NotYet {
    fn recognize(&self, _image: &Pixmap, _faces: bool) -> Result<Recognized, String> {
        Err("Text recognition is not available on this system yet.".into())
    }
}

/// The recognizer for this system.
#[must_use]
pub fn system() -> Box<dyn Recognizer> {
    #[cfg(windows)]
    {
        Box::new(crate::windows_ocr::WindowsOcr)
    }
    #[cfg(not(windows))]
    {
        Box::new(NotYet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapline_overlay::text::TextFont;
    use snapline_overlay::{Point, color::BLACK};

    #[cfg(not(windows))]
    #[test]
    fn other_systems_say_it_is_not_available_yet() {
        let image = Pixmap::new(10, 10).unwrap();
        assert!(system().recognize(&image, true).unwrap_err().contains("not available"));
    }

    /// Runs real Windows OCR on CI. A runner without an OCR language prints why and passes, so CI shows it.
    #[cfg(windows)]
    #[test]
    fn windows_ocr_reads_a_card_number() {
        let mut image = Pixmap::new(900, 120).unwrap();
        image.fill(snapline_overlay::tiny_skia::Color::WHITE);
        TextFont::bundled().draw(&mut image, "Card 4111 1111 1111 1111", Point { x: 20.0, y: 30.0 }, 40.0, BLACK);
        match system().recognize(&image, false) {
            Ok(found) => {
                let text: Vec<String> =
                    found.lines.iter().flat_map(|l| l.words.iter().map(|w| w.text.clone())).collect();
                assert!(text.join(" ").contains("4111"), "{text:?}");
                assert!(found.lines[0].words[0].rect.x < 100.0, "boxes in image pixels");
            }
            Err(e) => eprintln!("skipped: Windows OCR is not available on this runner: {e}"),
        }
    }
}
```

`NotYet` is unused on Windows: mark it `#[cfg(not(windows))]`. Run `cargo test -p snapline recognize` on this Mac → the non-Windows test passes once `system` exists. Write the test first and watch it fail.

- [ ] **Step 3: The Windows implementation**

`windows_ocr.rs`:

```rust
//! Windows.Media.Ocr and Windows.Media.FaceAnalysis, run on the calling (worker) thread.

use crate::recognize::Recognizer;
use snapline_overlay::scan::{Line, Recognized, Word, ocr_scale};
use snapline_overlay::tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};
use snapline_overlay::Rect;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::FaceAnalysis::FaceDetector;
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::core::HSTRING;

pub struct WindowsOcr;

const NO_OCR: &str = "Windows text recognition is not set up on this PC. Add a language that includes \
                      \"Optical character recognition\" in Settings → Time & language → Language & region.";

fn engine() -> Result<OcrEngine, String> {
    OcrEngine::TryCreateFromUserProfileLanguages()
        .or_else(|_| OcrEngine::TryCreateFromLanguage(&Language::CreateLanguage(&HSTRING::from("en-US"))?))
        .map_err(|_| NO_OCR.to_owned())
}

/// A Windows bitmap of `image` (straight BGRA; the screenshot is opaque, so premultiplied = straight).
fn bitmap(image: &Pixmap) -> windows::core::Result<SoftwareBitmap> {
    let mut bgra = image.data().to_vec();
    for px in bgra.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    let writer = DataWriter::new()?;
    writer.WriteBytes(&bgra)?;
    SoftwareBitmap::CreateCopyFromBuffer(&writer.DetachBuffer()?, BitmapPixelFormat::Bgra8, image.width() as i32, image.height() as i32)
}

/// `image` scaled by `factor` (< 1) for OCR's size limit.
fn shrink(image: &Pixmap, factor: f64) -> Option<Pixmap> {
    let (w, h) = ((f64::from(image.width()) * factor) as u32, (f64::from(image.height()) * factor) as u32);
    let mut out = Pixmap::new(w.max(1), h.max(1))?;
    let paint = PixmapPaint { quality: FilterQuality::Bilinear, ..PixmapPaint::default() };
    out.draw_pixmap(0, 0, image.as_ref(), &paint, Transform::from_scale(factor as f32, factor as f32), None);
    Some(out)
}

impl Recognizer for WindowsOcr {
    fn recognize(&self, image: &Pixmap, faces: bool) -> Result<Recognized, String> {
        // SAFETY: plain call; "already initialised" results are fine.
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let failed = |e: windows::core::Error| format!("Windows text recognition failed: {}", e.message());
        let engine = engine()?;
        let max = OcrEngine::MaxImageDimension().map_err(failed)?;
        let factor = ocr_scale(image.width(), image.height(), max);
        let small;
        let source = if factor < 1.0 {
            small = shrink(image, factor).ok_or("The screen is too large to read.")?;
            &small
        } else {
            image
        };
        let bitmap = bitmap(source).map_err(failed)?;
        let result = engine.RecognizeAsync(&bitmap).map_err(failed)?.join().map_err(failed)?;
        let mut found = Recognized::default();
        for line in result.Lines().map_err(failed)? {
            let mut words = Vec::new();
            for word in line.Words().map_err(failed)? {
                let r = word.BoundingRect().map_err(failed)?;
                let text = word.Text().map_err(failed)?.to_string_lossy();
                words.push(Word { text, rect: Rect::new(f64::from(r.X), f64::from(r.Y), f64::from(r.Width), f64::from(r.Height)) });
            }
            found.lines.push(Line { words });
        }
        if faces && FaceDetector::IsSupported().unwrap_or(false) {
            let gray = SoftwareBitmap::Convert(&bitmap, BitmapPixelFormat::Gray8).map_err(failed)?;
            let detector = FaceDetector::CreateAsync().map_err(failed)?.join().map_err(failed)?;
            for face in detector.DetectFacesAsync(&gray).map_err(failed)?.join().map_err(failed)? {
                let b = face.FaceBox().map_err(failed)?;
                found.faces.push(Rect::new(f64::from(b.X), f64::from(b.Y), f64::from(b.Width), f64::from(b.Height)));
            }
        }
        Ok(if factor < 1.0 { found.scaled(1.0 / factor) } else { found })
    }
}
```

Adapt names that differ in `windows` 0.62, such as `to_string_lossy` versus `to_string()`, or iterating an `IVectorView`, without changing behaviour. If `FaceDetector` rejects Gray8 at this size, check `FaceDetector::GetSupportedBitmapPixelFormats` and pick the first supported format. Record a ruling for any change.

- [ ] **Step 4: Lint here, then let CI compile it**

Run: `cd windows-linux && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`. This Mac checks only the non-Windows path.

Commit and push. CI's windows-latest job compiles `windows_ocr.rs` and runs `windows_ocr_reads_a_card_number`. Read that job's log for "skipped:" to learn whether the runner has OCR.

```bash
git add windows-linux
git commit -m "Read text and find faces with Windows OCR"
git push
```

Expected: CI passes on both systems. Fix any Windows compile error before Task 6.

---

### Task 6: Wiring — scan every capture, wait for it, Copy Text

**Files:**
- Modify: `windows-linux/crates/snapline/src/app.rs`, `actions.rs`

**Interfaces:**
- Consumes: Tasks 1–5.
- Produces:
  - `UserEvent::ScanDone { capture: u64, screen: usize, result: Result<Vec<HideBox>, String> }`
  - `UserEvent::TextRead(Result<String, String>)`

Behaviour:
- **Starting the scan:** `open_capture` re-reads `settings.toml` for `auto_hide` (so the settings window applies at once). When there are categories, it calls `session.begin_scan()` and, for each screen, spawns a thread that runs:

  ```
  recognize::system().recognize(&screen, cats has Face)
      .map(|found| scan::hide_boxes(&found, &cats, ui_scale))
  ```

  and sends `ScanDone`. Each capture gets an increasing `capture` id; results for a closed capture are ignored.
- **Scan finished:** `ScanDone` for the open capture → `effect = session.scan_done(screen, result.map_err(|_| recognize::UNAVAILABLE.to_owned()))` → `self.apply(event_loop, session.focused, effect)`. A held command runs as if just pressed.
- **Copy Text:** `Command::CopyText` (in `apply`, after `export`) spawns a thread that runs:

  ```
  recognize::system().recognize(&image, false)
      .map(|found| scan::copy_text(&found, w, h))
  ```

  and sends `TextRead`. Reading the flattened image means hidden boxes stay hidden.
- **Text read:**
  - `Ok(text)` with something in it: put `text.trim_end()` on the clipboard. Show the popup `Message { title: "Text copied", detail: "<n> line(s): <first 80 characters>" }`, closing after 2.5 s.
  - `Ok("")`: show `Problem { title: "No text found", detail: "Snapline could not read any text in the selection." }`, closing after 4 s.
  - `Err(sentence)`: show `Problem { title: "Text not copied", detail: sentence }`, which stays until closed.

- [ ] **Step 1: Write the failing test for the message text**

Put the wording in a pure function in `actions.rs`, so it can be tested:

```rust
/// The confirmation after Copy Text: "2 lines: first line…" (at most 80 characters of text).
#[must_use]
pub fn text_copied_detail(text: &str) -> String
```

Test:

```rust
    #[test]
    fn text_copied_detail_counts_lines_and_shortens() {
        assert_eq!(text_copied_detail("Hello"), "1 line: Hello");
        let long = format!("{}\nsecond", "x".repeat(100));
        let detail = text_copied_detail(&long);
        assert!(detail.starts_with("2 lines: ") && detail.chars().count() <= "2 lines: ".len() + 81, "{detail}");
    }
```

Run → fails. Implement it: count non-empty lines, and take the first 80 characters of the text with line breaks turned into spaces, adding "…" when cut. Run again → passes.

- [ ] **Step 2: Wire it**

Add the events, the capture id, the scan threads, `ScanDone` and `TextRead` handling, and the `CopyText` arm in `apply`, as described above. The worker threads send through `self.proxy.clone()`, as uploads do. Move the screen `Pixmap`s into the threads as clones taken before the `Session` takes ownership: clone each `screen.image` while building the session.

- [ ] **Step 3: Lint, test, push for CI**

Run: `cd windows-linux && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`. Then `cargo run -p snapline` on this Mac:
- A capture's badge reads "auto-hide unavailable", since there's no OCR here.
- Ctrl+C still copies.
- Ctrl+Shift+C shows "Text not copied — Text recognition is not available on this system yet."

Commit and push; CI must pass on Windows.

```bash
git add windows-linux
git commit -m "Scan every capture for auto-hide and copy text with Windows OCR"
git push
```

---

### Task 7: Docs, and the Windows check

**Files:**
- Modify: `docs/windows-linux.md`, `README.md`

- [ ] **Step 1: Docs**

`docs/windows-linux.md`:
- **Decisions:** add "Symbol repair deferred — Barayuda, 31 Oct 2026" and "Auto-hide tab in the settings window — Barayuda, 31 Oct 2026".
- **Phase 4 line:** link this plan. Note that `SymbolRepair` and the review popup moved out of Phase 4.
- **Risks:** add "Windows OCR has no Indonesian; Snapline falls back to English OCR, which reads Latin text and digits."

README "Windows (preview)":
- Change "auto-hide and Copy Text come later" to "keyboard-symbol repair in Copy Text comes later".
- Add:

```markdown
- Auto-hide: every capture is read on your PC, and card numbers, keys, passwords, phone numbers, emails and faces
  get black boxes before you copy, save or upload. Click a box to show it again; the eye button turns auto-hide off
  for this capture. Choose what to hide in the tray menu → **Settings…** → Auto-hide.
- Copy Text: Ctrl+Shift+C or the text button. Needs a Windows language with text recognition (Settings → Time &
  language → Language & region).
```

- [ ] **Step 2: Full check, commit, push**

```bash
cd windows-linux && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cd ../macOS && swift test
cd .. && git add docs README.md && git commit -m "Document auto-hide and Copy Text on Windows" && git push
```

- [ ] **Step 3: Barayuda checks it on Windows**

Pass or fail for each, with a screenshot for any fail:

1. Open a page with a card number (4111 1111 1111 1111), an email, a phone number, and an API key such as `sk_live_…`, plus a photo with a face.
   - Capture it and select all of it.
   - The badge says "scanning…", then "hiding N", and each item has a black box.
2. Press Ctrl+C at once, while "scanning…" still shows. The pasted image (in Paint) has the boxes.
3. Click a box: it shows, with a yellow dashed outline. Ctrl+Z hides it again. With a shown box, Ctrl+S saves the value visible.
4. Click the eye button: the boxes disappear from the screen, and Ctrl+C copies everything.
5. Settings… → Auto-hide: untick "Faces". In the next capture, the face is not boxed. Turn auto-hide off: no scan, and no "scanning…".
6. Ctrl+Shift+C on a paragraph pastes the text in reading order. The popup says "Text copied, N lines". On a photo with no text, it says "No text found".
7. Hidden areas stay hidden in Copy Text: the card number is not in the copied text.
8. On a PC language without OCR (or with the English language pack removed), the badge says "auto-hide unavailable". Ctrl+C still works, and Ctrl+Shift+C explains what to install.
9. With two monitors at different scaling, boxes sit exactly on the text on both.

- [ ] **Step 4: Fix what the Windows check finds**

For each fail:
- If the bug is in `scan.rs` or the session: write a failing test there first, then fix it.
- If it is in `windows_ocr.rs` or the wiring: fix it, and ask Barayuda to re-check that line.
