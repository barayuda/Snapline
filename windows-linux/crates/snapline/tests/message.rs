//! Runs the built program, as the tray app does when it shows a message.

// Final review: with no display the message window could not open, yet the program exited 0, so the caller
// thought the message was shown and printed nothing.
#[cfg(target_os = "linux")]
#[test]
fn a_message_that_cannot_open_a_window_fails() {
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_snapline"))
        .args(["--message", "Snapline could not save the screenshot."])
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .status()
        .unwrap();
    assert!(!status.success(), "{status}");
}
