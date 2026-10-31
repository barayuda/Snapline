//! Linux (X11) versions of the platform calls. `platform/mod.rs` re-exports them.

use std::ffi::OsStr;
use std::fs::File;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

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

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("snapline-{name}-{}", std::process::id()))
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
