//! Message boxes on Linux, drawn with egui. rfd's Linux message box needs zenity, which many desktops and the
//! Flatpak sandbox lack, so `show` runs this program again with `--message <text>`, as Settings does, and waits.

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

/// Shows `text` and waits until it is closed. When no window can open, the text goes to stderr.
pub fn show(text: &str) {
    let shown = std::env::current_exe()
        .and_then(|exe| std::process::Command::new(exe).args(args(text)).status());
    if !matches!(shown, Ok(s) if s.success()) {
        eprintln!("Snapline: {text}");
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
}
