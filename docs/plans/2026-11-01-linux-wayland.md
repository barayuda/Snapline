# Snapline on Linux Wayland (Phase 6b) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Snapline runs in GNOME and KDE Plasma Wayland sessions: the capture is the real screen (not black), the
capture shortcut works, and everything else behaves as on X11.

**Architecture:** Snapline's windows, clipboard and popups keep running through XWayland (the tested X11 code); winit is
forced onto its X11 backend. Screen capture already goes through the screenshot portal (or wlroots screencopy) inside
`xcap` when the session is Wayland. The capture shortcut comes from the GlobalShortcuts portal where the desktop has it
(KDE Plasma 6, GNOME 48+, Hyprland), and everywhere a `snapline --capture` command, which the user can bind in the
desktop's keyboard settings, tells the running Snapline to capture over the socket Phase 6a added.

**Tech Stack:** Rust 2024, winit 0.30 (X11 backend), xcap 0.9 (portal capture), ashpd 0.13 (`global_shortcuts`,
async-io), zbus 5, sway headless for a CI capture check, OrbStack VM.

**Spec:** [docs/windows-linux.md](../windows-linux.md) (Phase 6b) and the Decisions below.

## Decisions

Barayuda, 1 November 2026:

| Question | Answer |
|---|---|
| Approach | XWayland for windows, popups and clipboard; portals for capture and the shortcut. |
| Shortcut | GlobalShortcuts portal where available; `snapline --capture` everywhere, for a desktop custom shortcut. |
| Desktops | GNOME and KDE Plasma first; wlroots (sway, Hyprland) best effort. |
| PR #1 | After 6b: update the description (approved by Barayuda before posting) and mark it Ready for review. |

Decided while planning:

| Topic | Choice | Why |
|---|---|---|
| No XWayland | Refuse with a message naming XWayland | Every GNOME and KDE session has it; without it nothing can be drawn. |
| `--capture` when Snapline is not running | Start Snapline and capture once it is up | A bound key must always work, also right after login. |
| global-hotkey on Wayland | Not registered | Its X11 grab only fires while an XWayland window has focus, so it would look broken. |
| Fractional scaling | Known limit, documented | XWayland draws at 1× and is scaled; native Wayland is a later phase if needed. |

## Global Constraints

- The same as Phase 6a (`docs/plans/2026-11-01-linux-x11.md` Global Constraints): fmt, clippy pedantic `-D warnings`,
  tests on the Mac and in the VM, CI green on Windows and both Linux runners and the Flatpak jobs; GPG-signed commits, no
  AI attribution, Barayuda named, no Archipelago; never push tags or publish; Windows behaviour unchanged.
- Disk: the Mac has about 8 GB free. Before installing a desktop in the VM, check `df -h /Users`; remove what this phase
  installed when done.
- Nothing is posted to PR #1 until Barayuda approves the exact title and text.

## Review Focus

1. **GNOME asks for screenshot permission.** The first portal capture may show a dialog, or fail when it is refused.
   Snapline must say what happened and how to allow it, not show a black or empty capture. Pinned by
   `a_refused_screenshot_says_how_to_allow_it` (Task 4).
2. **`snapline --capture` with Snapline not running, or a stale socket file.** It must start Snapline and capture, not
   fail silently. Pinned by `capture_starts_snapline_when_none_is_running` (Task 2).
3. **A desktop without the GlobalShortcuts portal** (GNOME 46/47, older Plasma). Snapline must say once how to bind
   `snapline --capture`, with the command for this install (deb, AppImage, Flatpak). Pinned by
   `wayland_without_the_portal_explains_the_command` (Task 3).
4. **Shortcut names the portal cannot read.** Conversion from Snapline's names (`Ctrl+Shift+Digit1`, `PrintScreen`) must
   give XDG names (`CTRL+SHIFT+1`, `Print`), and an unknown key gives no preferred trigger instead of a wrong one.
   Pinned by `shortcut_names_become_portal_triggers` (Task 3).
5. **A Wayland session with no XWayland.** Snapline must say so and stop. Pinned by `wayland_needs_xwayland` (Task 1).

---

### Task 1: Wayland sessions run through XWayland

**Files:** `crates/snapline/src/platform/linux.rs`, `crates/snapline/src/platform/mod.rs`, `crates/snapline/src/main.rs`

**Interfaces:** Produces `platform::is_wayland() -> bool`; `platform::linux::NO_XWAYLAND`; `session_problem_for` keeps
its signature.

- [ ] **Step 1: Failing test.** In `platform/linux.rs` tests, replace `wayland_and_console_sessions_are_refused` with:

```rust
    // Phase 6b: Wayland runs through XWayland; only a Wayland session without XWayland is refused.
    #[test]
    fn wayland_needs_xwayland() {
        let wayland = Some(OsStr::new("wayland"));
        assert_eq!(session_problem_for(wayland, true, true), None);
        assert_eq!(session_problem_for(wayland, true, false), Some(NO_XWAYLAND));
        assert_eq!(session_problem_for(None, true, false), Some(NO_XWAYLAND));
        assert_eq!(session_problem_for(Some(OsStr::new("x11")), false, true), None);
        assert_eq!(session_problem_for(None, false, false), Some(NO_DISPLAY));
        assert_eq!(session_problem_for(Some(OsStr::new("tty")), false, false), Some(NO_DISPLAY));
    }
```

Run: `linux/dev-vm.sh run cargo test -p snapline wayland_needs` — Expected: compile error, `NO_XWAYLAND` not found.

- [ ] **Step 2: Implement.** Replace `WAYLAND` with

```rust
/// Said in a Wayland session that has no XWayland: Snapline draws its windows through XWayland.
pub const NO_XWAYLAND: &str = "Snapline needs XWayland in a Wayland session. Install the xwayland package (GNOME \
     and KDE Plasma start it by themselves), or log in to an X11 session, then start Snapline again.";
```

and in `session_problem_for`: `if wayland && kind != "x11" && !x11 { Some(NO_XWAYLAND) } else if x11 { None } else
{ Some(NO_DISPLAY) }` (a Wayland session type with `DISPLAY` set is fine). Add

```rust
/// A Wayland session (Snapline's own windows still go through XWayland).
pub fn is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var_os("XDG_SESSION_TYPE").is_some_and(|t| t == "wayland")
}
```

re-exported from `platform/mod.rs` (`false` off Linux). In `main.rs`, build the tray app's event loop on X11:

```rust
    let mut builder = EventLoop::<app::UserEvent>::with_user_event();
    // Wayland sessions too: Snapline places its overlay and popups, which only X11 (XWayland) allows.
    #[cfg(target_os = "linux")]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;
        builder.with_x11();
    }
    let event_loop = match builder.build() { … };
```

Update `README.md` "Linux (preview)" first paragraph and `docs/windows-linux.md` Phase 6 line in Task 4, not here.

- [ ] **Step 3: Pass, whole suite, commit** "Linux: run in Wayland sessions through XWayland".

### Task 2: `snapline --capture`

**Files:** `crates/snapline/src/platform/linux.rs`, `crates/snapline/src/platform/mod.rs`, `crates/snapline/src/app.rs`,
`crates/snapline/src/main.rs`

**Interfaces:** Produces `platform::Signal { Settings, Capture }`; `platform::watch(impl Fn(Signal) + Send + 'static)`
(replaces `watch_settings` on Linux; Windows keeps `watch_settings`); `platform::send(Signal) -> bool` (true when a
running Snapline got it); `platform::capture_command() -> String`.

- [ ] **Step 1: Failing tests** in `platform/linux.rs`:

```rust
    #[test]
    fn signals_travel_over_the_socket() {
        let path = scratch("signals.sock");
        let (tx, rx) = std::sync::mpsc::channel();
        watch_at(&path, move |s| {
            let _ = tx.send(s);
        });
        assert!(send_at(&path, Signal::Capture));
        assert!(send_at(&path, Signal::Settings));
        let got: Vec<Signal> = (0..2).map(|_| rx.recv_timeout(Duration::from_secs(5)).unwrap()).collect();
        assert_eq!(got, [Signal::Capture, Signal::Settings]);
        let _ = std::fs::remove_file(path);
    }

    // Review Focus 2: nobody listening (not running, or a socket file left by a crash).
    #[test]
    fn capture_starts_snapline_when_none_is_running() {
        assert!(!send_at(&scratch("missing.sock"), Signal::Capture), "no socket file");
        let stale = scratch("stale.sock");
        drop(std::os::unix::net::UnixDatagram::bind(&stale)); // the file stays, nobody reads it
        assert!(!send_at(&stale, Signal::Capture), "a socket file left by a crash");
        assert_eq!(after_capture_flag(false), Some(Startup::CaptureWhenReady));
        assert_eq!(after_capture_flag(true), None);
        let _ = std::fs::remove_file(stale);
    }

    #[test]
    fn the_capture_command_matches_the_install() {
        assert_eq!(
            command_for(&Install::Flatpak("dev.barayuda.Snapline".into()), "--capture"),
            "flatpak run dev.barayuda.Snapline --capture"
        );
        assert_eq!(command_for(&Install::Plain("/usr/bin/snapline".into()), "--capture"), "snapline --capture");
    }
```

- [ ] **Step 2: Implement.** `Signal` is sent as the bytes `settings` / `capture`; unknown datagrams are ignored.
`watch_at(path, notify: impl Fn(Signal))`, `send_at(path, Signal) -> bool` (`send_to` succeeded),
`after_capture_flag(delivered: bool) -> Option<Startup>` (`Startup::CaptureWhenReady` when nobody got it).
`settings_command_for` becomes `command_for(install, flag)` (the Phase 6a test keeps passing through
`settings_command()` = `command_for(.., "--settings")`); add `capture_command()`. In `main.rs`, before the single-instance
claim: `--capture` → `if platform::send(Signal::Capture) { return; }` else start normally with
`App::new(proxy, capture_at_start = true)`, and `App::start` sends `UserEvent::CaptureNow` once the tray and shortcuts
are set up. `announce_settings_changed` becomes `send(Signal::Settings)`; the app's watcher maps `Settings` →
`UserEvent::SettingsChanged`, `Capture` → `UserEvent::HotKey`. Windows code is unchanged (its `--capture` is not added
in this phase).

- [ ] **Step 3: Pass, whole suite, commit** "Linux: `snapline --capture` asks the running Snapline to capture, or starts it".

### Task 3: The GlobalShortcuts portal

**Files:** Create `crates/snapline/src/portal_shortcuts.rs`; modify `crates/snapline/src/app.rs`,
`crates/snapline/src/settings.rs`, `crates/snapline/src/main.rs`, `crates/snapline/Cargo.toml`

**Interfaces:** Produces `portal_shortcuts::xdg_trigger(&str) -> Option<String>`; `portal_shortcuts::start(
EventLoopProxy<UserEvent>, Option<String>)` (thread; sends `UserEvent::HotKey` on activation and
`UserEvent::NoShortcutPortal(String)` when the portal is missing or refused); `settings::wayland_shortcut_note(&str,
&str) -> String`.

- [ ] **Step 1: Failing tests.**

```rust
    // Review Focus 4.
    #[test]
    fn shortcut_names_become_portal_triggers() {
        assert_eq!(xdg_trigger("Ctrl+Shift+Digit1").as_deref(), Some("CTRL+SHIFT+1"));
        assert_eq!(xdg_trigger("PrintScreen").as_deref(), Some("Print"));
        assert_eq!(xdg_trigger("Alt+KeyQ").as_deref(), Some("ALT+q"));
        assert_eq!(xdg_trigger("Super+F5").as_deref(), Some("LOGO+F5"));
        assert_eq!(xdg_trigger("Ctrl+NoSuchKey"), None);
    }
```

(in `portal_shortcuts.rs`) and in `settings.rs`:

```rust
    // Review Focus 3.
    #[test]
    fn wayland_without_the_portal_explains_the_command() {
        let note = wayland_shortcut_note("the portal is not available", "flatpak run dev.barayuda.Snapline --capture");
        assert!(note.contains("keyboard settings") && note.contains("flatpak run dev.barayuda.Snapline --capture"), "{note}");
    }
```

- [ ] **Step 2: Implement.** `ashpd = { version = "0.13.13", default-features = false, features = ["global_shortcuts",
"async-io"] }` (Linux). `start` runs on a thread with `zbus::block_on`: `GlobalShortcuts::new()`, `create_session`,
`bind_shortcuts(&session, &[NewShortcut::new("capture", "Take a screenshot").preferred_trigger(trigger)], None,
Default::default())` then `.response()`, then forward `receive_activated()` items with id `capture` as
`UserEvent::HotKey`. Any error → `UserEvent::NoShortcutPortal(error text)`. On Wayland, `App::register_hotkeys`
registers nothing with global-hotkey, starts the portal with the first configured shortcut, and the labels shown in the
tray come from the portal only after `bind` succeeds. `NoShortcutPortal(why)` sets `shortcut_note =
wayland_shortcut_note(&why, &platform::capture_command())`, shown once (like the tray note) and from the tray's "Why"
item. `xdg_trigger`: modifiers `Ctrl→CTRL`, `Alt→ALT`, `Shift→SHIFT`, `Super→LOGO`; keys `DigitN→N`, `KeyX→x`,
`F1…F24` as is, `PrintScreen→Print`, `Space→space`, `Enter→Return`, `Escape→Escape`; anything else → `None`.

- [ ] **Step 3: Pass, whole suite, commit** "Linux: capture shortcut through the GlobalShortcuts portal; the command when there is none".

### Task 4: Capture on Wayland, checked; docs

**Files:** `crates/snapline/src/capture.rs`, `crates/snapline/src/app.rs`, `.github/workflows/windows-linux.yml`,
`README.md`, `docs/windows-linux.md`, `site/index.html`, `site/privacy/index.html`

- [ ] **Step 1: Failing test** (`capture.rs`):

```rust
    // Review Focus 1: GNOME asks first; a refused or failed portal capture must say how to allow it.
    #[test]
    fn a_refused_screenshot_says_how_to_allow_it() {
        let text = capture_problem("Portal request didn't succeed: Cancelled", true);
        assert!(text.contains("allow") && text.contains("Screenshots"), "{text}");
        assert!(capture_problem("no display", false).contains("no display"));
    }
```

`capture_problem(error, wayland) -> String`: on Wayland, "Snapline could not take the screenshot: {error}. In a
Wayland session the desktop must allow it: in GNOME, Settings → Apps → Snapline → Screenshots (or answer Allow when
asked); in KDE, System Settings → Application Permissions." elsewhere "Snapline could not take the screenshot:
{error}". `app.rs` shows it where `capture_all` fails today.

- [ ] **Step 2: CI wlroots check.** Add to the Linux test job an ignored test `wayland_capture_is_the_real_screen`
(`capture.rs`): capture all screens and assert some pixel is red (`#ff0000` from the background CI sets). CI step:
`sudo apt-get install -y sway swaybg xwayland`, then
`WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 sway -c <(printf 'output * bg #ff0000 solid_color\nxwayland enable\n')`
in the background, wait for `$XDG_RUNTIME_DIR/wayland-*`, export `WAYLAND_DISPLAY`, `XDG_SESSION_TYPE=wayland` and the
`DISPLAY` sway gives XWayland (`:0` by default with xwayland enable, or read it from `swaymsg -t get_outputs`), and run
`cargo test -p snapline -- --ignored wayland_capture`. Expected: pass (wlroots screencopy through xcap).

- [ ] **Step 3: VM GNOME check (portal).** If `df -h /Users` shows ≥ 6 GB free: install `gnome-shell
xdg-desktop-portal-gnome xwayland` in the VM, start `dbus-run-session -- gnome-shell --headless --wayland
--virtual-monitor 1600x900`, run `--version`, `--capture` (logs), and the ignored capture test against it. Ledger what
the portal did (allowed, dialog, refused). Remove the packages afterwards (`apt-get autoremove`, `fstrim`).

- [ ] **Step 4: Docs.** README "Linux (preview)": X11 and Wayland (GNOME, KDE Plasma); the shortcut on Wayland
(portal, or bind `snapline --capture`); GNOME may ask to allow screenshots; fractional scaling looks soft (XWayland).
`docs/windows-linux.md`: Decisions rows for 6b; Phases item 6 says 6b is done with a link to this plan; Risks: the
Wayland line updated. Site: "Linux X11 desktops" → "Linux desktops (X11 and Wayland: GNOME, KDE Plasma)".

- [ ] **Step 5: Whole suite, commit** "Linux: capture on Wayland through the portal; docs for Wayland".

### Task 5: Review and PR

- [ ] Final whole-branch review of 6b (Opus) with this plan's Review Focus; one fix pass with RED→GREEN tests.
- [ ] Draft the PR #1 title and description for phases 1–6b (same structure as the 6a text; Phase 6b section,
  Evidence, a Linux Wayland checklist for Barayuda's devices). Show it; post only after approval; then
  `gh pr ready 1`, and check assignee and labels.
