//! Linux (X11) versions of the platform calls. `platform/mod.rs` re-exports them.

use std::ffi::OsStr;
use std::fs::File;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use snapline_overlay::Rect;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, InputFocus};
use x11rb::rust_connection::RustConnection;

/// Said on Wayland: 6a supports X11 sessions only.
pub const WAYLAND: &str = "Snapline for Linux needs an X11 session for now; Wayland support is coming. Log out, \
     choose an X11 or Xorg session at the login screen (for example \"Ubuntu on Xorg\"), and start Snapline again.";
/// Said when there is no desktop at all.
pub const NO_DISPLAY: &str =
    "Snapline needs a desktop session. Start it from your desktop, not from a text console.";

/// Why Snapline cannot run in this session, or `None` when it can.
pub fn session_problem() -> Option<&'static str> {
    session_problem_for(
        std::env::var_os("XDG_SESSION_TYPE").as_deref(),
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
        std::env::var_os("DISPLAY").is_some(),
    )
}

/// `session_type` is `$XDG_SESSION_TYPE`; `wayland` and `x11` say whether `$WAYLAND_DISPLAY` and `$DISPLAY` are set.
pub fn session_problem_for(
    session_type: Option<&OsStr>,
    wayland: bool,
    x11: bool,
) -> Option<&'static str> {
    let kind = session_type.and_then(OsStr::to_str).unwrap_or("");
    if kind == "wayland" || (wayland && kind != "x11") {
        Some(WAYLAND)
    } else if x11 {
        None
    } else {
        Some(NO_DISPLAY)
    }
}

/// The user's runtime folder (`$XDG_RUNTIME_DIR`, private to the user), or the temp folder.
fn runtime_dir() -> PathBuf {
    dirs::runtime_dir().unwrap_or_else(std::env::temp_dir)
}

/// Lock files held for the life of the process.
static HELD: Mutex<Vec<File>> = Mutex::new(Vec::new());

/// `name` is the Windows mutex name (`Local\dev.barayuda.snapline`); its last part names the lock file.
pub fn claim(name: &str) -> bool {
    let file = name.rsplit('\\').next().unwrap_or(name);
    claim_at(&runtime_dir().join(format!("{file}.lock")))
}

/// False when another process holds the lock on `path`. When no lock can be made at all, Snapline runs.
pub fn claim_at(path: &Path) -> bool {
    let Ok(file) = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
    else {
        return true;
    };
    match file.try_lock() {
        Ok(()) => {
            HELD.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(file);
            true
        }
        Err(std::fs::TryLockError::WouldBlock) => false,
        Err(std::fs::TryLockError::Error(_)) => true,
    }
}

const SETTINGS_SOCKET: &str = "dev.barayuda.snapline.settings.sock";

pub fn watch_settings(notify: impl Fn() + Send + 'static) {
    watch_at(&runtime_dir().join(SETTINGS_SOCKET), notify);
}

pub fn announce_settings_changed() {
    announce_at(&runtime_dir().join(SETTINGS_SOCKET));
}

/// Calls `notify` for every datagram sent to `path`. Only the tray app listens, and it holds the claim, so a socket
/// file left behind by a crash is safe to replace.
pub fn watch_at(path: &Path, notify: impl Fn() + Send + 'static) {
    let _ = std::fs::remove_file(path);
    let Ok(socket) = UnixDatagram::bind(path) else {
        eprintln!("Snapline: settings changes will apply after a restart");
        return;
    };
    std::thread::spawn(move || {
        let mut buf = [0u8; 16];
        while socket.recv(&mut buf).is_ok() {
            notify();
        }
    });
}

/// Nothing happens when the tray app is not running.
pub fn announce_at(path: &Path) {
    if let Ok(socket) = UnixDatagram::unbound() {
        let _ = socket.send_to(b"settings", path);
    }
}

/// A connection to the X server and its root window.
fn x11() -> Option<(RustConnection, u32)> {
    let (conn, screen) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen)?.root;
    Some((conn, root))
}

fn atom(conn: &RustConnection, name: &str) -> Option<u32> {
    Some(
        conn.intern_atom(false, name.as_bytes())
            .ok()?
            .reply()
            .ok()?
            .atom,
    )
}

pub fn beep() {
    if let Some((conn, _)) = x11() {
        let _ = conn.bell(0);
        let _ = conn.flush();
    }
}

pub fn cursor_position() -> Option<(i32, i32)> {
    let (conn, root) = x11()?;
    let pointer = conn.query_pointer(root).ok()?.reply().ok()?;
    Some((pointer.root_x.into(), pointer.root_y.into()))
}

/// The part of `monitor` inside `work`, or `None` when they do not overlap.
#[must_use]
pub fn clip(monitor: Rect, work: Rect) -> Option<Rect> {
    let x = monitor.x.max(work.x);
    let y = monitor.y.max(work.y);
    let right = (monitor.x + monitor.width).min(work.x + work.width);
    let bottom = (monitor.y + monitor.height).min(work.y + work.height);
    (right > x && bottom > y).then(|| Rect::new(x, y, right - x, bottom - y))
}

/// `_NET_WORKAREA` of the current desktop: the screen without panels, as the window manager reports it.
fn net_workarea(conn: &RustConnection, root: u32) -> Option<Rect> {
    let current = conn
        .get_property(
            false,
            root,
            atom(conn, "_NET_CURRENT_DESKTOP")?,
            AtomEnum::CARDINAL,
            0,
            1,
        )
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .next()
        .unwrap_or(0);
    let areas: Vec<u32> = conn
        .get_property(
            false,
            root,
            atom(conn, "_NET_WORKAREA")?,
            AtomEnum::CARDINAL,
            0,
            1024,
        )
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .collect();
    let boxes = areas.as_chunks::<4>().0;
    let a = boxes.get(current as usize).or_else(|| boxes.first())?;
    Some(Rect::new(
        f64::from(a[0]),
        f64::from(a[1]),
        f64::from(a[2]),
        f64::from(a[3]),
    ))
}

/// The usable part of the monitor under `(x, y)`, in physical pixels; `None` without a window manager.
pub fn work_area(x: i32, y: i32) -> Option<Rect> {
    let monitor = xcap::Monitor::all().ok()?.into_iter().find_map(|m| {
        let (mx, my) = (m.x().ok()?, m.y().ok()?);
        let (mw, mh) = (m.width().ok()?, m.height().ok()?);
        let inside = x >= mx
            && y >= my
            && i64::from(x) < i64::from(mx) + i64::from(mw)
            && i64::from(y) < i64::from(my) + i64::from(mh);
        inside.then(|| Rect::new(f64::from(mx), f64::from(my), f64::from(mw), f64::from(mh)))
    })?;
    let (conn, root) = x11()?;
    // ponytail: _NET_WORKAREA is one box for all monitors, so a panel on another monitor only can be missed.
    clip(monitor, net_workarea(&conn, root)?)
}

/// The X11 window id of `window`.
fn xid(window: &winit::window::Window) -> Option<u32> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Xlib(h) => u32::try_from(h.window).ok(),
        RawWindowHandle::Xcb(h) => Some(h.window.get()),
        _ => None,
    }
}

/// Gives `window` the keyboard (Escape and the shortcuts must reach the overlay at once).
pub fn take_focus(window: &winit::window::Window) {
    if let (Some((conn, _)), Some(id)) = (x11(), xid(window)) {
        let _ = conn.set_input_focus(InputFocus::PARENT, id, x11rb::CURRENT_TIME);
        let _ = conn.flush();
    }
}

/// Popups are override-redirect on X11 (`popup_window.rs`): no window manager focuses them or lists them.
pub fn never_activate(_window: &winit::window::Window) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("snapline-{name}-{}", std::process::id()))
    }

    #[test]
    fn the_work_area_is_the_monitor_without_the_panels() {
        let monitor = Rect::new(1920.0, 0.0, 2560.0, 1440.0);
        // _NET_WORKAREA spans every monitor: here a 48-pixel panel at the bottom of a 4480 × 1440 desktop.
        let work = Rect::new(0.0, 0.0, 4480.0, 1392.0);
        assert_eq!(
            clip(monitor, work),
            Some(Rect::new(1920.0, 0.0, 2560.0, 1392.0))
        );
        assert_eq!(clip(monitor, Rect::new(0.0, 0.0, 1920.0, 1080.0)), None);
    }

    #[test]
    #[ignore = "needs an X server: CI runs it under xvfb-run"]
    fn x11_answers_for_the_pointer_and_the_bell() {
        assert!(cursor_position().is_some());
        beep();
        // Xvfb has no window manager, so no _NET_WORKAREA: the popup falls back to the whole monitor.
        assert_eq!(work_area(0, 0), None);
    }

    #[test]
    fn only_the_first_claim_wins() {
        let path = scratch("claim.lock");
        assert!(claim_at(&path));
        assert!(!claim_at(&path), "a second Snapline must stop");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn an_announcement_reaches_the_watcher() {
        let path = scratch("settings.sock");
        let (tx, rx) = std::sync::mpsc::channel();
        watch_at(&path, move || {
            let _ = tx.send(());
        });
        announce_at(&path);
        assert!(rx.recv_timeout(std::time::Duration::from_secs(5)).is_ok());
        let _ = std::fs::remove_file(path);
    }

    // Review Focus 5: XWayland sees only X11 apps, so a capture on Wayland would come out black.
    #[test]
    fn wayland_and_console_sessions_are_refused() {
        let wayland = Some(OsStr::new("wayland"));
        let x11 = Some(OsStr::new("x11"));
        assert_eq!(session_problem_for(wayland, true, true), Some(WAYLAND));
        assert_eq!(session_problem_for(None, true, false), Some(WAYLAND));
        assert_eq!(session_problem_for(x11, false, true), None);
        // A nested Wayland compositor inside an X11 session: Snapline still sees the X11 desktop.
        assert_eq!(session_problem_for(x11, true, true), None);
        assert_eq!(session_problem_for(None, false, false), Some(NO_DISPLAY));
        assert_eq!(
            session_problem_for(Some(OsStr::new("tty")), false, false),
            Some(NO_DISPLAY)
        );
    }
}
