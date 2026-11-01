# Phase 5a: Windows edit, share, print, General settings — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Windows capture gets the macOS app's last three actions: Edit in another app (Ctrl+E), Share (the Windows Share window, in place of Email), and Print (Ctrl+P). The settings window gets a General tab with these settings:
- capture shortcut, and PrtScn on or off;
- what a tray click does;
- the save folder and the editor;
- saving at 100 % size;
- launching at sign-in.

Shortcut and tray changes apply without restarting. Phase 4's review left minor issues; the ones users would notice are fixed here: a crashed worker thread, a failed clipboard write, the wrong OCR message, and two processes saving settings at once.

**Architecture:**
- **Overlay crate (pure):** owns the new buttons, commands and keys, and the 100 % downscale.
- **App crate, OS calls:** Edit and Print are Windows shell verbs on a temporary PNG. Share uses WinRT `DataTransferManager` through `IDataTransferManagerInterop` in `share.rs`, Windows only, on a small window kept alive while Share is open. Launch at sign-in is a value under `HKCU\…\Run`.
- **Settings changes:** the settings process signals a named Win32 event after it saves. The tray app waits on that event in a thread, re-reads `settings.toml`, then registers the shortcuts again and rebuilds the tray icon.

**Tech Stack:** Rust 2024 (stable); `windows` 0.62.2 (adds `ApplicationModel_DataTransfer`, `Storage`, `Win32_UI_Shell`, `Win32_UI_WindowsAndMessaging`); `windows-sys` 0.61 (adds `Win32_System_Registry`, `Win32_UI_Shell`); `std::fs::File::lock` (stable since Rust 1.89); everything from Phases 1–4.

**Spec:** [docs/windows-linux.md](../windows-linux.md) (Phase 5, first half). The macOS app is the behaviour reference:
- `macOS/Sources/Snapline/Actions.swift`: `edit`, `email`, `print`, `tempFile`, and the downscale in `nsImage`;
- `OverlayView+Toolbars.swift`: main bar order;
- `OverlayView+Keyboard.swift`: ⌘E and ⌘P;
- `Preferences.swift`: `GeneralTab`.

## Decisions made for this phase

Barayuda's choices on 31 October 2026:

| Question | Choice |
|---|---|
| Phase 5 scope | **Split.** 5a (this plan) covers the actions, the General tab and the important minors. 5b (later) covers the installer, code signing, website and privacy policy. |
| Email on Windows | **Windows Share.** The button is "Share", and it hands the PNG to Outlook (new and classic), Mail, Teams, WhatsApp or Nearby Share. MAPI is out because the new Outlook doesn't support it. |
| Deferred minors | **Only those users notice.** Covered: a crashed upload, scan or Copy Text thread; Copy Text saying "copied" when it wasn't; the OCR message on Windows N; the `settings.toml` race. The rest stay on PR #1's list. |

Behaviour copied from macOS:
- **Main bar order:** Upload, Search, Edit, Share, Print, Copy, Copy Text, Save, Close.
- **Temp files:** Edit, Share and Print write the PNG to a temp folder (`%TEMP%\Snapline\<name>.png`) and leave it there for Windows to clean up.
- **Edit:** opens the chosen editor, or the default one when none is chosen. On Windows the default is the PNG `edit` verb, normally Paint.
- **Save at 100 %:** "Save at 100 % size on high-DPI screens" matches macOS "Downscale Retina screens". When it is on, every export except Copy Text is scaled by `1 / ui_scale`. Copy Text keeps full resolution, because OCR reads it better.

Choices made in this plan:
- **Print:** the shell `print` verb on the PNG opens the Windows Photos print dialog. It already offers fit-to-page, orientation and a printer list, so Snapline needs no print code of its own.
- **Share without a window:** the overlay closes before an action runs, so Share needs a window of its own. Snapline opens a small tool window at the selection's centre, shows Share for it, and closes it when the Share window reports it is done, or after 2 minutes.
  - If Windows refuses (Share is missing on Windows Server, for example), Explorer opens with the PNG selected and a popup explains why.
- **Shortcuts in the settings file:** they stay the Phase 2 `hotkeys` list. The General tab shows it as one recorded shortcut plus an "Also use PrtScn" tick box. A recorded shortcut needs Ctrl, Alt or Win, except F1–F24.
- **Taken shortcuts:** before saving a recorded shortcut, the settings process registers it and unregisters it again. A taken shortcut is refused with a message, and the old one stays.
- **Applying changes:** the named event `Local\dev.barayuda.snapline.settings-changed` (auto-reset) carries the signal, so nothing polls. The tray app also re-reads the save folder and editor on every action, as uploads already do.
- **Launch at sign-in:** the value `Snapline` = `"<path to snapline.exe>"` under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. The tick box reads the registry each time the tab opens, so changes made in Task Manager show up.

Left for 5b or later: installer, code signing, website and privacy policy; "Copy Text: write keyboard symbols as words", which needs `SymbolRepair`; the 9 deferred minors not listed above.

## Global Constraints

- Workspace `windows-linux/`, edition 2024, stable toolchain. Each commit must pass `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` (pedantic).
  - Crate roots may allow only `cast_possible_truncation`, `cast_sign_loss` and `cast_precision_loss`.
  - Write empty checks as `assert_eq!(v, [])`.
- **Gate every commit chain:** `cargo clippy … >/dev/null 2>&1 && cargo test … && git commit …`. Phase 4 pushed a commit that failed lint because the chain used `;`.
- **Windows-only code is checked by CI**, because this Mac cannot compile the Windows target. Before pushing it:
  - check every `windows`/`windows-sys` name and signature against `~/.cargo/registry/src/*/windows-0.62.2` or `windows-sys-0.61.*`;
  - read the file once for unused imports (Phase 4 nearly shipped one);
  - push after each Windows-only task.
- `snapline-overlay` must not depend on winit, `windows` or any OS API.
- Add crates and features with `cargo add`.
- Copy in plain English, sentence case, Windows names ("Settings → Apps → Startup").
- Git: one commit per task, GPG-signed, plain sentence-case messages, no AI co-author or "generated with" lines. Check that `git log --format='%h %G?'` shows `G` before pushing.

## Review Focus

1. **Recording a shortcut.**
   - Cases: no modifier, a key global-hotkey cannot parse, a shortcut another app holds, or the same shortcut as now.
   - Expected: only a parseable, free, modified shortcut (or F-key) is saved; the current one is accepted as is.
   - Tests: `recorded_shortcuts_need_a_modifier_and_must_parse` (Task 5), `the_shortcut_list_round_trips_through_the_tab` (Task 5).
2. **Settings changed while the tray app runs.**
   - Cases: the shortcut changes, PrtScn is turned off, or the tray click changes.
   - Expected: the new shortcut works at once, the old one stops working, and a failed registration shows the note as at start.
   - Tests: `reload_keeps_working_shortcuts_and_reports_taken_ones` (Task 6), plus the Windows check.
3. **Two processes saving `settings.toml` at the same moment**, for example the upload thread saving a Drive folder while the settings window saves auto-hide. Neither change may be lost. Test: `concurrent_updates_keep_both_changes` (Task 1).
4. **A worker thread that panics** (upload, scan or Copy Text). The app must not stay stuck on "Uploading…" or "scanning…", and the user must see why. Test: `the_guard_runs_when_the_thread_panics` (Task 1).
5. **Downscale at 100 %.**
   - Cases: 125 % and 150 % scaling, a 1-pixel-wide selection, Copy Text.
   - Expected: the size is rounded and never 0, the drawings stay in place, and Copy Text is untouched.
   - Test: `downscale_rounds_and_never_reaches_zero` (Task 2).

---

## File structure

```
windows-linux/crates/
├── snapline-overlay/src/
│   ├── session.rs, toolbar.rs, icons.rs   Edit, Share, Print buttons, Ctrl+E / Ctrl+P
│   └── render.rs                          downscale()
└── snapline/src/
    ├── worker.rs        new: OnPanic guard for worker threads
    ├── settings.rs      file lock in update(); editor, downscale, General helpers
    ├── actions.rs       temp_png, edit, print; text_copied_detail stays
    ├── share.rs         new, Windows only: Windows Share
    ├── shortcuts.rs     new: recorded shortcut → global-hotkey name, availability check
    ├── platform.rs      shell verbs, launch at sign-in, settings-changed event
    ├── upload_settings.rs   General tab
    ├── recognize.rs, windows_ocr.rs   OCR unavailable message
    └── app.rs           commands, downscale, guards, reload on settings change
```

---

### Task 1: Worker guards, clipboard errors, OCR message, settings lock

**Files:**
- Create: `windows-linux/crates/snapline/src/worker.rs` (add `mod worker;` to `main.rs`)
- Modify: `settings.rs`, `app.rs`, `recognize.rs`, `windows_ocr.rs`

**Interfaces:**
- Produces:
  - `worker::OnPanic<F: FnOnce()>`, built with `OnPanic::new(f)`. It runs `f` when dropped during a panic and does nothing otherwise, so call sites need no disarm step.
  - `recognize::ocr_unavailable(no_language: bool) -> &'static str`.
  - `Settings::update` holds an exclusive lock on `settings.lock`, next to the file, while it reads and writes.
  - `app::copy_text(text) -> Result<(), String>`.

- [ ] **Step 1: Write the failing tests**

`worker.rs`:

```rust
//! Worker threads report back through the event loop; a panic must still report, or the app waits forever.

/// Runs `f` if the thread panics while this is alive (dropped while unwinding); otherwise does nothing.
pub struct OnPanic<F: FnOnce()>(Option<F>);

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    // Review Focus 4.
    #[test]
    fn the_guard_runs_when_the_thread_panics() {
        let (tx, rx) = channel();
        let tx2 = tx.clone();
        let _ = std::thread::spawn(move || {
            let _guard = OnPanic::new(move || tx2.send("panicked").unwrap());
            panic!("boom");
        })
        .join();
        assert_eq!(rx.try_recv(), Ok("panicked"));
        std::thread::spawn(move || {
            let _guard = OnPanic::new(move || tx.send("panicked").unwrap());
        })
        .join()
        .unwrap();
        assert!(rx.try_recv().is_err(), "a normal end sends nothing");
    }
}
```

`settings.rs`:

```rust
    // Review Focus 3.
    #[test]
    fn concurrent_updates_keep_both_changes() {
        let path = temp_file("lock", "");
        let threads: Vec<_> = (0..8)
            .map(|k| {
                let path = path.clone();
                std::thread::spawn(move || {
                    for _ in 0..10 {
                        Settings::update(&path, |s| {
                            if k % 2 == 0 {
                                s.s3_bucket = format!("bucket-{k}");
                            } else {
                                s.google_email = format!("user-{k}");
                            }
                        })
                        .unwrap();
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        let saved = Settings::load(&path).0;
        assert!(saved.s3_bucket.starts_with("bucket-") && saved.google_email.starts_with("user-"), "{saved:?}");
    }
```

`recognize.rs`:

```rust
    #[test]
    fn the_ocr_message_says_what_to_do() {
        assert!(ocr_unavailable(true).contains("Language & region"));
        assert!(ocr_unavailable(false).contains("Media Feature Pack"));
    }
```

Run `cd windows-linux && cargo test -p snapline` → fails to compile.

Then run the lock test alone with `update` unchanged but `OnPanic` and `ocr_unavailable` stubbed. It should fail or flake: without a lock, one thread's read-modify-write loses the other's change. If it passes 5 runs in a row, raise the counts (16 threads × 50) until it fails without the lock. A test that cannot fail proves nothing.

- [ ] **Step 2: Implement**

`worker.rs`:

```rust
impl<F: FnOnce()> OnPanic<F> {
    pub fn new(f: F) -> Self {
        Self(Some(f))
    }
}

impl<F: FnOnce()> Drop for OnPanic<F> {
    fn drop(&mut self) {
        if std::thread::panicking()
            && let Some(f) = self.0.take()
        {
            f();
        }
    }
}
```

`settings.rs`, inside `update`, before `Self::load`:

```rust
        // Both Snapline processes write this file: hold a lock across read-change-write, or one loses the other's change.
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let lock = std::fs::File::create(path.with_extension("lock")).map_err(|e| e.to_string())?;
        lock.lock().map_err(|e| e.to_string())?;
```

The lock is released when `lock` is dropped at the end of the function. Keep `save` itself unlocked, because it is called inside `update`.

`app.rs`:
- **Guards.** In each worker closure (upload, scan, read text), put a guard first:
  - upload: `let _guard = OnPanic::new({ let p = proxy.clone(); move || { let _ = p.send_event(UserEvent::UploadFinished(Outcome::Failed("Snapline hit an internal error while uploading. Try again.".into()))); } });`
  - scan: send `ScanDone { capture, screen, result: Err("internal error".into()) }`;
  - read text: send `TextRead(Err("Snapline hit an internal error while reading the text.".into()))`.
- **`copy_text`** returns `Result<(), String>`:
  - Upload Done: on `Err`, show `Done` but log the error. The link is still in the popup with a Copy button.
  - Copy Text: on `Err`, show `Problem { title: "Text not copied", detail: "Snapline could not use the clipboard: {e}" }`.
  - Change the log text from "the link" to "the clipboard".

`recognize.rs`:

```rust
/// Why Windows text recognition cannot run: no OCR language (`no_language`), or OCR itself is missing.
#[must_use]
pub fn ocr_unavailable(no_language: bool) -> &'static str {
    if no_language {
        "Windows text recognition is not set up on this PC. Add a language that includes \
         \"Optical character recognition\" in Settings → Time & language → Language & region."
    } else {
        "Windows text recognition is not available on this PC. On Windows N editions, install the \
         Media Feature Pack from Settings → Apps → Optional features."
    }
}
```

Mark `ocr_unavailable` with `#[cfg_attr(not(windows), allow(dead_code, reason = "used by the Windows recognizer"))]`.

`windows_ocr.rs`:
- Delete `NO_OCR`.
- In `engine()`, map errors with `|e| ocr_unavailable(e.code().is_ok()).to_owned()`. windows-core turns a null result into `Error::empty()`, whose code is `S_OK`, and `TryCreate…` returns null exactly when no language fits. Check that `Error::code` and `HRESULT::is_ok` exist in `windows-result` 0.4.1 before relying on them.

- [ ] **Step 3: Run the tests**

Run: `cd windows-linux && cargo clippy --workspace --all-targets -- -D warnings >/dev/null 2>&1 && cargo test --workspace`
Expected: all pass. Run `concurrent_updates_keep_both_changes` 5 times; it passes every time.

- [ ] **Step 4: Commit and push**

```bash
git add windows-linux
git commit -m "Report crashed worker threads, clipboard failures and missing OCR clearly, and lock settings.toml"
git push
```

---

### Task 2: Edit, Share and Print in the overlay, and the 100 % downscale

**Files:**
- Modify: `windows-linux/crates/snapline-overlay/src/session.rs`, `toolbar.rs`, `icons.rs`, `render.rs`

**Interfaces:**
- Produces:
  - `Command::{Edit, Share, Print}`.
  - `Button::{Edit, Share, Print}`.
  - `MAIN_BAR: [Button; 9]` = Upload, Search, Edit, Share, Print, Copy, CopyText, Save, Close.
  - Ctrl+E runs Edit and Ctrl+P runs Print.
  - `render::downscale(image: &Pixmap, factor: f64) -> Option<Pixmap>`, where `factor` is at most 1 and the size is rounded, at least 1 px.
  - `Session::export_scale(&self) -> f64`: the selected screen's `ui_scale` (1.0 without a selection).

- [ ] **Step 1: Write the failing tests**

`toolbar.rs`:

```rust
    #[test]
    fn the_main_bar_matches_macos() {
        assert_eq!(
            MAIN_BAR,
            [
                Button::Upload,
                Button::Search,
                Button::Edit,
                Button::Share,
                Button::Print,
                Button::Copy,
                Button::CopyText,
                Button::Save,
                Button::Close
            ]
        );
    }
```

Update `the_bars_carry_the_new_buttons` from Phase 4: Copy Text is now `MAIN_BAR[6]`.

`session.rs`:

```rust
    #[test]
    fn ctrl_e_edits_and_ctrl_p_prints() {
        let mut s = selected(1.0);
        assert_eq!(s.key(Key::Char('e'), ctrl()).command, Some(Command::Edit));
        assert_eq!(s.key(Key::Char('p'), ctrl()).command, Some(Command::Print));
        assert_eq!(
            click_button(&mut s, Button::Share, Mods::default()).command,
            Some(Command::Share)
        );
    }

    #[test]
    fn edit_share_and_print_wait_for_the_scan_too() {
        let mut s = selected(1.0);
        s.begin_scan();
        assert_eq!(s.key(Key::Char('p'), ctrl()).command, None);
        assert_eq!(s.scan_done(0, Ok(vec![])).command, Some(Command::Print));
    }
```

`render.rs`:

```rust
    // Review Focus 5.
    #[test]
    fn downscale_rounds_and_never_reaches_zero() {
        let image = white(301, 1);
        let half = downscale(&image, 1.0 / 1.5).unwrap();
        assert_eq!((half.width(), half.height()), (201, 1));
        let same = downscale(&image, 1.0).unwrap();
        assert_eq!((same.width(), same.height()), (301, 1));
        let mut marked = white(200, 100);
        fill_rect(&mut marked, Rect::new(100.0, 50.0, 100.0, 50.0), BLACK, 255);
        let small = downscale(&marked, 0.5).unwrap();
        assert_eq!(rgb(&small, 75, 37), (0, 0, 0), "the black quarter stays in its place");
        assert_eq!(rgb(&small, 25, 12), (255, 255, 255));
    }
```

Run `cargo test -p snapline-overlay` → fails.

- [ ] **Step 2: Implement**

- **`toolbar.rs`:** the three buttons, with tips:
  - `Edit => "Edit in another app (Ctrl+E)"`;
  - `Share => "Share"`;
  - `Print => "Print (Ctrl+P)"`.
  
  Then the new `MAIN_BAR`. The main bar grows to 9 buttons. `bars_never_overlap` and `full_screen_selection_keeps_toolbars_on_screen` must still pass; if a corner case fails, fix `layout`, not the test, and record a ruling.
- **`session.rs`:**
  - Add the three `Command`s.
  - In `press`, the three buttons map to `self.request(Command::…)`.
  - In the Ctrl match, add `'e' => self.request(Command::Edit)` and `'p' => self.request(Command::Print)`.
  - Add `export_scale`:

```rust
    /// Pixels per point of the screen whose selection is exported (for saving at 100 %).
    #[must_use]
    pub fn export_scale(&self) -> f64 {
        self.selected().map_or(1.0, |i| self.overlays[i].ui_scale)
    }
```

- **`render.rs`:**

```rust
/// `image` scaled by `factor` (at most 1): sizes are rounded and never 0. For "save at 100 %" on high-DPI screens.
#[must_use]
pub fn downscale(image: &Pixmap, factor: f64) -> Option<Pixmap> {
    let factor = factor.min(1.0);
    let w = ((f64::from(image.width()) * factor).round() as u32).max(1);
    let h = ((f64::from(image.height()) * factor).round() as u32).max(1);
    let mut out = Pixmap::new(w, h)?;
    let paint = tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() };
    let (sx, sy) = (w as f32 / image.width() as f32, h as f32 / image.height() as f32);
    out.draw_pixmap(0, 0, image.as_ref(), &paint, Transform::from_scale(sx, sy), None);
    Some(out)
}
```

- **`icons.rs`**, on the 24-unit grid with plain strokes:
  - Edit: three horizontal sliders, as in macOS `slider.horizontal.3`.
  - Share: a tray with an arrow up out of it.
  - Print: a printer, i.e. the paper top, the body and the output tray.
  
  Put each in its own function, as the Phase 4 icons are, so `outline` stays under 100 lines.
- **App crate:** in `actions::run`, add `Command::Edit | Command::Share | Command::Print` to the `Ok(None)` arm for now. Task 3 fills them in.

- [ ] **Step 3: Run the tests and look at the bar**

Run: `cd windows-linux && cargo clippy --workspace --all-targets -- -D warnings >/dev/null 2>&1 && cargo test --workspace`

Then render a selection with the bars to `/tmp/phase5a.png` in a throwaway test, as Phase 4 did. Look at the three new icons at 1× and 2×, then delete the test.

- [ ] **Step 4: Commit**

```bash
git add windows-linux
git commit -m "Add Edit, Share and Print to the capture, and a 100 % downscale"
```

---

### Task 3: Edit and Print, and saving at 100 %

**Files:**
- Modify: `windows-linux/crates/snapline/src/actions.rs`, `platform.rs`, `settings.rs`, `app.rs`, `Cargo.toml` (windows-sys feature `Win32_UI_Shell`)

**Interfaces:**
- Produces:
  - `Settings.editor: PathBuf` (empty means the default editor) and `Settings.downscale: bool` (default false).
  - `actions::temp_png(png, name) -> Result<PathBuf, String>`, which writes to `std::env::temp_dir().join("Snapline")`.
  - `platform::shell_verb(verb: &str, file: &Path) -> Result<(), String>`. On Windows it calls `ShellExecuteW`, where a result above 32 means success; elsewhere it uses `open::that`.
  - `actions::run` handles `Edit` and `Print`.

Behaviour:
- **Edit:**
  - With `editor` set and the file present, run `Command::new(editor).arg(file).spawn()`.
  - Otherwise try `shell_verb("edit", file)`, then fall back to `shell_verb("open", file)`, since some PCs have no `edit` verb for PNG.
  - Errors read "Snapline could not open the editor: …".
- **Print:** `shell_verb("print", file)`. It returns `Ok(None)` because there is nothing to confirm: Windows shows its own dialog.
- **Downscale:** in `App::apply`, right after `export()`:
  - if `settings.downscale` (read fresh) is on and `export_scale() > 1.0`, replace the image with `render::downscale(&image, 1.0 / scale)`;
  - do this for every command except `CopyText`.

- [ ] **Step 1: Write the failing tests**

`actions.rs`:

```rust
    #[test]
    fn temp_png_writes_into_the_snapline_temp_folder() {
        let path = temp_png(b"png bytes", "Snapline test.png").unwrap();
        assert!(path.starts_with(std::env::temp_dir().join("Snapline")));
        assert_eq!(std::fs::read(&path).unwrap(), b"png bytes");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_missing_editor_falls_back_to_the_default() {
        let settings = Settings { editor: "/no/such/editor.exe".into(), ..Settings::default() };
        assert_eq!(editor_to_use(&settings), None, "a missing editor means the default");
        let here = std::env::current_exe().unwrap();
        let settings = Settings { editor: here.clone(), ..Settings::default() };
        assert_eq!(editor_to_use(&settings), Some(here));
    }
```

`settings.rs`: extend the round-trip test to cover `editor` and `downscale`. Check that an old file without them loads with the defaults.

- [ ] **Step 2: Implement** as described above.

`shell_verb` on Windows:

```rust
pub fn shell_verb(verb: &str, file: &std::path::Path) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let wide = |s: &std::ffi::OsStr| {
        use std::os::windows::ffi::OsStrExt;
        s.encode_wide().chain(Some(0)).collect::<Vec<u16>>()
    };
    let (verb_w, file_w) = (wide(verb.as_ref()), wide(file.as_os_str()));
    // SAFETY: both strings are NUL-terminated and live across the call.
    let code = unsafe {
        ShellExecuteW(std::ptr::null_mut(), verb_w.as_ptr(), file_w.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL)
    } as isize;
    if code > 32 { Ok(()) } else { Err(format!("Windows could not {verb} the file (code {code})")) }
}
```

Check `ShellExecuteW`'s parameter and return types in windows-sys 0.61 before pushing. In 0.61 it returns `HINSTANCE`, a pointer, so `as isize` may need to be `as usize as isize`. Clippy may flag that cast; if it does, use `.addr()` and record a ruling.

- [ ] **Step 3: Lint, test, commit, push**

```bash
cd windows-linux && cargo clippy --workspace --all-targets -- -D warnings >/dev/null 2>&1 && cargo test --workspace && cd .. \
  && git add windows-linux && git commit -m "Edit and print captures with the Windows shell, and save at 100 % on request" && git push
```

---

### Task 4: Windows Share

**Files:**
- Create: `windows-linux/crates/snapline/src/share.rs` (`#[cfg(windows)] mod share;` in `main.rs`)
- Modify: `app.rs`, `actions.rs`, `Cargo.toml` (windows features `ApplicationModel_DataTransfer`, `Storage`, `Win32_UI_Shell`)

**Interfaces:**
- Produces: `share::show(hwnd: isize, file: &Path, title: &str) -> Result<(), String>`. It registers `DataRequested` on `DataTransferManager::GetForWindow(hwnd)` and calls `ShowShareUIForWindow(hwnd)`. Its handler:
  1. takes a deferral;
  2. sets `Properties().SetTitle(title)`;
  3. gets a `StorageFile` with `GetFileFromPathAsync(...).join()`;
  4. calls `SetStorageItemsReadOnly(IIterable::from(vec![Some(file.cast::<IStorageItem>()?)]))`;
  5. completes the deferral.
- `App` owns `share_window: Option<(winit::window::Window, Instant)>`: one small tool window, never activated, that holds Share. It is dropped after 2 minutes in `about_to_wait`, or when the next capture opens.

Behaviour:
- **Share:** write `temp_png`, open the share window, then call `share::show`.
- **On failure** (also on non-Windows):
  1. run `explorer.exe /select,"<file>"`;
  2. show `Problem { title: "Share is not available", detail: "Windows could not open Share here ({e}). The screenshot is selected in Explorer, so you can drag it where you need it." }`.

- [ ] **Step 1: Write the failing test**

Share UI cannot be tested on CI. Test the fallback wording, which is pure, in `actions.rs`:

```rust
    #[test]
    fn share_fallback_says_where_the_file_is() {
        let text = share_unavailable("class not registered");
        assert!(text.contains("Explorer") && text.contains("class not registered"));
    }
```

- [ ] **Step 2: Implement**

Check these names in `windows-0.62.2`; they were verified for this plan:
- `windows::Win32::UI::Shell::IDataTransferManagerInterop`, with `GetForWindow<T>(HWND)` and `ShowShareUIForWindow(HWND)`, both `unsafe`;
- `windows::ApplicationModel::DataTransfer::{DataTransferManager, DataRequestedEventArgs}`, where `DataRequested(&TypedEventHandler<DataTransferManager, DataRequestedEventArgs>)` returns a token;
- `TypedEventHandler::new(|_, args: windows_core::Ref<DataRequestedEventArgs>| …)`;
- `DataRequest::GetDeferral` / `DataRequestDeferral::Complete`;
- `DataPackage::SetStorageItemsReadOnly(&IIterable<IStorageItem>)`;
- `StorageFile::GetFileFromPathAsync(&HSTRING)`;
- `IIterable<T>: From<Vec<Option<T>>>`.

Get the interop with `windows::core::factory::<DataTransferManager, IDataTransferManagerInterop>()`. Remove the `DataRequested` handler with `RemoveDataRequested(token)` when the share window is dropped.

- [ ] **Step 3: Lint, test, read the Windows code once more, push**

Expected: CI compiles `share.rs` on Windows. Whether Share opens is checked on a PC (Task 7).

```bash
git add windows-linux && git commit -m "Share captures through the Windows Share window" && git push
```

---

### Task 5: The General tab

**Files:**
- Create: `windows-linux/crates/snapline/src/shortcuts.rs`
- Modify: `upload_settings.rs`, `settings.rs`, `platform.rs`, `Cargo.toml` (windows-sys `Win32_System_Registry`)

**Interfaces:**
- Produces:
  - `shortcuts::from_keys(ctrl: bool, alt: bool, shift: bool, win: bool, key: &str) -> Option<String>`. It builds a global-hotkey name such as `Ctrl+Shift+1`, but only when that name parses as a `HotKey` and has a modifier (an F-key alone is allowed).
  - `shortcuts::is_free(name) -> bool`: register, then unregister, in this process.
  - `Settings::capture_shortcut(&self) -> Option<&str>`, `Settings::uses_print_screen(&self) -> bool`, and `Settings::set_shortcuts(&mut self, shortcut: Option<String>, print_screen: bool)`.
  - `platform::launch_at_login() -> bool` and `platform::set_launch_at_login(on: bool) -> Result<(), String>`. Windows uses the registry; other systems return false or an error.
  - `platform::announce_settings_changed()`.

The General tab copies the macOS `GeneralTab`. Every change saves at once, then calls `announce_settings_changed()`:
- **Capture shortcut:** the current label, plus a "Record" button that reads the next egui key press with modifiers. Esc cancels. A taken shortcut shows "That shortcut is used by another app." in red.
- **"Also use PrtScn"** tick box.
- **"Clicking the tray icon":** Opens the menu / Takes a screenshot.
- **"Save screenshots to":** the folder name and "Choose…", which opens `rfd::FileDialog::pick_folder`.
- **"Edit with":** the app name ("Paint (default)" when empty), "Choose…" (an `.exe` picker), and "Use default".
- **"Save at 100 % size on high-DPI screens"** tick box.
- **"Launch at sign-in"** tick box, with any error in red underneath.

The tab order becomes General, Upload, Auto-hide, as on macOS.

- [ ] **Step 1: Write the failing tests**

`shortcuts.rs`:

```rust
    // Review Focus 1.
    #[test]
    fn recorded_shortcuts_need_a_modifier_and_must_parse() {
        assert_eq!(from_keys(true, false, true, false, "1"), Some("Ctrl+Shift+1".into()));
        assert_eq!(from_keys(false, false, false, false, "A"), None, "no modifier");
        assert_eq!(from_keys(false, false, false, false, "F9"), Some("F9".into()), "F-keys stand alone");
        assert_eq!(from_keys(true, false, false, false, "NotAKey"), None);
        assert_eq!(from_keys(true, true, false, true, "S"), Some("Ctrl+Alt+Super+S".into()));
    }
```

`settings.rs`:

```rust
    // Review Focus 1.
    #[test]
    fn the_shortcut_list_round_trips_through_the_tab() {
        let mut s = Settings::default();
        assert_eq!((s.capture_shortcut(), s.uses_print_screen()), (Some("Ctrl+Shift+Digit1"), true));
        s.set_shortcuts(Some("Ctrl+Alt+S".into()), false);
        assert_eq!(s.hotkeys, ["Ctrl+Alt+S"]);
        s.set_shortcuts(None, true);
        assert_eq!(s.hotkeys, ["PrintScreen"]);
    }
```

Platform: test the `Run` value's quoting, which is pure:

```rust
    #[test]
    fn the_run_value_quotes_the_path() {
        assert_eq!(run_value(std::path::Path::new(r"C:\Program Files\Snapline\snapline.exe")),
                   r#""C:\Program Files\Snapline\snapline.exe""#);
    }
```

- [ ] **Step 2: Implement**

- **`from_keys`:**
  - Join the modifiers as `Ctrl`, `Alt`, `Shift`, `Super`, in that order, with `+`, then the key.
  - Return `None` when the key isn't an F-key and no modifier is down.
  - Return `None` when the result doesn't `parse::<global_hotkey::hotkey::HotKey>()`.
  - Feed it egui's `Key::name()`; egui uses the names global-hotkey parses ("A", "1", "F5").
- **`is_free`:** `GlobalHotKeyManager::new()`, `register(h)`, then `unregister(h)`. A name that is already in `settings.hotkeys` counts as free, because the tray app is the one holding it.
- **Registry:**
  - `RegSetKeyValueW(HKEY_CURRENT_USER, w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"), w!("Snapline"), REG_SZ, …)` to turn it on;
  - `RegDeleteKeyValueW` to turn it off (a missing value is fine);
  - `RegGetValueW(…, RRF_RT_REG_SZ, …)` to read it. It's on when the value exists and names this exe.
  
  Check every name in windows-sys 0.61 first.
- **`announce_settings_changed`:** `OpenEventW(EVENT_MODIFY_STATE, 0, name)` then `SetEvent`. When nothing is open yet, the tray app isn't running and there is nothing to do.

- [ ] **Step 3: Lint, test, commit, push**

```bash
git add windows-linux && git commit -m "Add the General tab: shortcut, tray click, folders, editor, 100 % saving and launch at sign-in" && git push
```

---

### Task 6: Apply settings changes while the tray app runs

**Files:**
- Modify: `windows-linux/crates/snapline/src/app.rs`, `platform.rs`, `settings.rs`

**Interfaces:**
- Produces:
  - `platform::watch_settings(notify: impl Fn() + Send + 'static)`. On Windows it is a thread that creates the named event and calls `notify` each time it is set; elsewhere it does nothing.
  - `UserEvent::SettingsChanged`.
  - `App::reload_settings(&mut self)`, which:
    1. re-reads `settings.toml`;
    2. unregisters every current shortcut, then registers the new list;
    3. rebuilds the tray icon, so the menu labels and the tray-click behaviour follow;
    4. shows the shortcut note when something is taken.
  - `settings::split_shortcuts(names, try_register) -> (Vec<String>, Vec<String>)`: the registration step of `register_hotkeys`, made pure so it can be tested.

- [ ] **Step 1: Write the failing test**

```rust
    // Review Focus 2.
    #[test]
    fn reload_keeps_working_shortcuts_and_reports_taken_ones() {
        let names = vec!["PrintScreen".to_owned(), "Ctrl+Alt+S".to_owned()];
        let (working, failed) = split_shortcuts(&names, |n| n != "PrintScreen");
        assert_eq!((working, failed.clone()), (vec!["Ctrl+Alt+S".to_owned()], vec!["PrintScreen".to_owned()]));
        assert!(shortcut_note(&failed, &["Ctrl+Alt+S".to_owned()]).unwrap().contains("PrtScn"));
    }
```

- [ ] **Step 2: Implement**

- **`register_hotkeys`:**
  - Keep the `GlobalHotKeyManager` and the registered `HotKey`s.
  - On reload, `unregister_all(&old)` before registering the new list.
  - Use `split_shortcuts` with `|n| n.parse::<HotKey>().is_ok_and(|h| manager.register(h).is_ok())`.
- **Tray:** replace `self.tray` with a newly built tray, the same as at start.
- **Start:** call `platform::watch_settings` with a proxy clone that sends `UserEvent::SettingsChanged`.

- [ ] **Step 3: Lint, test, commit, push**

```bash
git add windows-linux && git commit -m "Apply shortcut and tray changes from the settings window at once" && git push
```

---

### Task 7: Docs, and the Windows check

**Files:**
- Modify: `docs/windows-linux.md`, `README.md`

- [ ] **Step 1: Docs**

`docs/windows-linux.md`:
- Decisions: add rows for "Phase 5 split into 5a and 5b", "Windows Share replaces Email" and "Phase 4 minors users notice, in 5a", each "Barayuda, 31 Oct 2026".
- Phase 5 line: split it into 5a (link to this plan) and 5b (installer, code signing, website, privacy policy).

README, "Windows (preview)" section:
- Add Edit (Ctrl+E), Share and Print (Ctrl+P).
- Rename "Settings…" to describe the General, Upload and Auto-hide tabs.
- Mention "Launch at sign-in".
- Drop the hand-editing of `settings.toml` for shortcuts and tray click.

- [ ] **Step 2: Full check, commit, push**

```bash
cd windows-linux && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cd ../macOS && swift test
cd .. && git add docs README.md && git commit -m "Document edit, share, print and the General tab on Windows" && git push
```

- [ ] **Step 3: Barayuda checks it on Windows**

Pass or fail for each, with a screenshot for any fail:

1. Ctrl+E opens the capture in Paint. Choose another editor in Settings → General, and Ctrl+E opens that one instead. Then "Use default" goes back to Paint.
2. Share opens the Windows Share window, and sending to Outlook or Mail attaches the PNG. Close Share without picking anything: Snapline keeps working, and the next capture opens normally.
3. Ctrl+P opens the print dialog with the capture, fitted to the page.
4. On a 150 % screen, tick "Save at 100 % size" and save a capture of a 300 × 200-point area: the PNG is 300 × 200 pixels. Untick it: 450 × 300.
5. Record Ctrl+Alt+S as the shortcut. It captures at once, without restarting Snapline, and Ctrl+Shift+1 no longer does. Untick PrtScn: PrtScn no longer captures.
6. Record a shortcut another app already uses (for example Win+Shift+S). It is refused with the red note, and the old shortcut still works.
7. Tick "Launch at sign-in", sign out and back in: Snapline is in the tray. Untick it: it's gone from Task Manager → Startup apps.
8. Change "Clicking the tray icon" to "Takes a screenshot": a click captures at once, without restarting.
9. With the settings window open, upload to Drive (which saves the folder ID) while changing an Auto-hide tick box. Both changes are in `settings.toml` afterwards.

- [ ] **Step 4: Fix what the Windows check finds**

For each fail:
- If the logic is pure, write a failing test first, then fix it.
- If the bug is in Windows-only code, fix it and ask Barayuda to check that line again.
