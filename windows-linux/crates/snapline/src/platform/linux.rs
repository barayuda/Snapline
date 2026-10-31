//! Linux (X11) versions of the platform calls. `platform/mod.rs` re-exports them.

use std::ffi::OsStr;

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

#[cfg(test)]
mod tests {
    use super::*;

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
