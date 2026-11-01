//! Snapline for Windows and Linux: a tray app that captures the screen, lets you mark it up, and copies or saves it.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window in release builds
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)] // pixel maths on clamped values

mod actions;
mod app;
mod capture;
mod icon;
#[cfg(target_os = "linux")]
mod linux_tray;
#[cfg(target_os = "linux")]
mod message;
mod platform;
mod popup_window;
#[cfg(target_os = "linux")]
mod portal_shortcuts;
mod recognize;
mod secrets;
mod settings;
mod share;
mod shortcuts;
#[cfg(target_os = "linux")]
mod tesseract_ocr;
mod tray;
mod upload_job;
mod upload_settings;
mod window;
#[cfg(windows)]
mod windows_ocr;
mod worker;

use winit::event_loop::{ControlFlow, EventLoop};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version") {
        return println!(
            "Snapline {}\n{}",
            env!("CARGO_PKG_VERSION"),
            recognize::status()
        );
    }
    #[cfg(target_os = "linux")]
    if let Some(text) = message::parse(&args) {
        return message::run_window(text);
    }
    if let Some(problem) = platform::session_problem() {
        return platform::show_error(problem);
    }
    // `--upload-settings` is the Phase 3 name, kept for shortcuts made then.
    if args
        .iter()
        .any(|a| a == "--settings" || a == "--upload-settings")
    {
        return upload_settings::run_window();
    }
    // `--capture` (a desktop shortcut on Wayland): the running Snapline takes it, or this one starts and takes it.
    let capture_at_start = args.iter().any(|a| a == "--capture");
    if capture_at_start && platform::send(platform::Signal::Capture) {
        return;
    }
    if !platform::claim_single_instance() {
        platform::show_error(&format!(
            "Snapline is already running. Look for its icon {}.",
            platform::TRAY_PLACE
        ));
        return;
    }
    #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
    let mut builder = EventLoop::<app::UserEvent>::with_user_event();
    // Wayland sessions too: Snapline places its overlay and popups, which only X11 (XWayland) allows.
    #[cfg(target_os = "linux")]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;
        builder.with_x11();
    }
    let event_loop = match builder.build() {
        Ok(l) => l,
        Err(e) => return platform::show_error(&format!("Snapline could not start: {e}")),
    };
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = app::App::new(event_loop.create_proxy(), capture_at_start);
    if let Err(e) = event_loop.run_app(&mut app) {
        platform::show_error(&format!("Snapline stopped: {e}"));
    }
}
