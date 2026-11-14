//! Message boxes on Linux, drawn with egui. rfd's Linux message box needs zenity, which many desktops and the
//! Flatpak sandbox lack, so this program runs again with `--message <text>`, as Settings does: `show` returns at once
//! (the tray app must keep running), `show_and_wait` waits (right before Snapline exits).

/// The arguments that show `text`.
#[must_use]
pub fn args(text: &str) -> [String; 2] {
    ["--message".into(), text.into()]
}

/// The text to show, when this program's arguments (without the program name) ask for a message.
#[must_use]
pub fn parse(args: &[String]) -> Option<String> {
    let at = args.iter().position(|a| a == "--message")?;
    args.get(at + 1).cloned()
}

/// Shows `text` in its own window, without waiting. When no window can open, the text goes to stderr.
pub fn show(text: &str) {
    match std::env::current_exe() {
        Ok(exe) => {
            let mut window = std::process::Command::new(exe);
            window.args(args(text));
            show_using(window, text);
        }
        Err(_) => eprintln!("Snapline: {text}"),
    }
}

/// Shows `text` and waits until it is closed, for messages right before Snapline exits: the Flatpak sandbox and
/// the `AppImage` mount end with the process, and a box left running would vanish with them.
pub fn show_and_wait(text: &str) {
    match std::env::current_exe() {
        Ok(exe) => {
            let mut window = std::process::Command::new(exe);
            window.args(args(text));
            show_and_wait_using(window, text);
        }
        Err(_) => eprintln!("Snapline: {text}"),
    }
}

/// Runs `window` to the end; true when it was shown. Otherwise `text` goes to stderr.
pub fn show_and_wait_using(mut window: std::process::Command, text: &str) -> bool {
    let shown = window.status().is_ok_and(|s| s.success());
    if !shown {
        eprintln!("Snapline: {text}");
    }
    shown
}

/// Starts `window` (the `--message` window) and returns at once: the window is its own process, so it must not
/// hold up the tray app (a note at start blocked the first capture). A thread waits for it and prints `text` when
/// no window could open.
pub fn show_using(mut window: std::process::Command, text: &str) {
    let text = text.to_owned();
    match window.spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                if !child.wait().is_ok_and(|s| s.success()) {
                    eprintln!("Snapline: {text}");
                }
            });
        }
        Err(_) => eprintln!("Snapline: {text}"),
    }
}

/// The `--message` window: the text and OK. Enter and Escape close it too. Exits with 1 when no window opens.
pub fn run_window(text: String) {
    use eframe::egui;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Snapline")
            .with_inner_size([440.0, 170.0])
            .with_icon(std::sync::Arc::new(egui::IconData {
                rgba: crate::icon::tray_rgba(64, crate::icon::TrayState::Normal),
                width: 64,
                height: 64,
            })),
        ..Default::default()
    };
    let shown = eframe::run_native(
        "Snapline",
        options,
        Box::new(|_| Ok(Box::new(Message(text)))),
    );
    // No window (no display): fail, so `show` prints the message instead.
    if shown.is_err() {
        std::process::exit(1);
    }
}

struct Message(String);

impl eframe::App for Message {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        use eframe::egui;
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(110.0)
                .show(ui, |ui| ui.label(&self.0));
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                let ok = ui.button("OK").clicked();
                let key = ui
                    .input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape));
                if ok || key {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_survives_the_command_line() {
        let text = "Snapline could not save to /home/a b: Permission denied";
        assert_eq!(parse(&args(text)).as_deref(), Some(text));
        assert_eq!(parse(&["--message".into()]), None);
        assert_eq!(parse(&["--settings".into()]), None);
    }

    // Phase 6b review: a message right before Snapline exits must be waited for (Flatpak and AppImage end the
    // child with the parent), and when no window opens the text goes to stderr.
    #[test]
    fn a_message_before_exit_waits_until_it_is_closed() {
        let mut slow = std::process::Command::new("sleep");
        slow.arg("1");
        let started = std::time::Instant::now();
        assert!(show_and_wait_using(slow, "Snapline is already running."));
        assert!(started.elapsed() >= std::time::Duration::from_secs(1));
        assert!(!show_and_wait_using(
            std::process::Command::new("false"),
            "no window: printed instead"
        ));
    }

    // Phase 6b check: the "no tray icons" note blocked Snapline's event loop, so a capture waited until it was
    // closed. Showing a message must return at once.
    #[test]
    fn showing_a_message_does_not_wait_for_it() {
        let mut slow = std::process::Command::new("sleep");
        slow.arg("3");
        let started = std::time::Instant::now();
        show_using(slow, "Snapline could not save the screenshot.");
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }
}
