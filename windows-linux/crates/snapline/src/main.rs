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
mod platform;
mod popup_window;
mod recognize;
mod secrets;
mod settings;
mod share;
mod shortcuts;
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
        return println!("Snapline {}", env!("CARGO_PKG_VERSION"));
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
    if !platform::claim_single_instance() {
        platform::show_error("Snapline is already running. Look for its icon next to the clock.");
        return;
    }
    let event_loop = match EventLoop::<app::UserEvent>::with_user_event().build() {
        Ok(l) => l,
        Err(e) => return platform::show_error(&format!("Snapline could not start: {e}")),
    };
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = app::App::new(event_loop.create_proxy());
    if let Err(e) = event_loop.run_app(&mut app) {
        platform::show_error(&format!("Snapline stopped: {e}"));
    }
}
