//! Snapline for Windows: a tray app that captures the screen, lets you mark it up, and copies or saves it.
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
mod platform;
mod popup_window;
mod secrets;
mod settings;
mod upload_job;
mod window;

use winit::event_loop::{ControlFlow, EventLoop};

fn main() {
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
