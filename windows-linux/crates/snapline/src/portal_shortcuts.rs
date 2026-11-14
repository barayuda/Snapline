//! The capture shortcut in Wayland sessions, through the `GlobalShortcuts` portal (KDE Plasma 6, GNOME 48+, Hyprland).
//! The desktop asks the user to confirm it once and lets them change it; Snapline only suggests a key.

use ashpd::desktop::CreateSessionOptions;
use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;

/// Snapline's shortcut name (`Ctrl+Shift+Digit1`, as global-hotkey writes it) as an XDG trigger (`CTRL+SHIFT+1`);
/// `None` for a key the portal names differently, so no wrong key is suggested.
#[must_use]
pub fn xdg_trigger(name: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for part in name.split('+') {
        let xdg = match part {
            "Ctrl" | "Control" => "CTRL".to_owned(),
            "Alt" => "ALT".to_owned(),
            "Shift" => "SHIFT".to_owned(),
            "Super" | "Meta" | "Cmd" => "LOGO".to_owned(),
            "PrintScreen" => "Print".to_owned(),
            "Space" => "space".to_owned(),
            "Enter" => "Return".to_owned(),
            "Escape" => "Escape".to_owned(),
            key if key.len() == 6 && key.starts_with("Digit") => key[5..].to_owned(),
            key if key.len() == 4 && key.starts_with("Key") => key[3..].to_ascii_lowercase(),
            key if key.starts_with('F')
                && key[1..].parse::<u8>().is_ok_and(|n| (1..=24).contains(&n)) =>
            {
                key.to_owned()
            }
            _ => return None,
        };
        parts.push(xdg);
    }
    Some(parts.join("+"))
}

/// Asks the portal for the capture shortcut, suggesting `trigger`, and calls `pressed` each time it is pressed.
/// When there is no portal, or it refuses, calls `missing` with the reason.
pub fn start(
    trigger: Option<String>,
    pressed: impl Fn() + Send + 'static,
    missing: impl FnOnce(String) + Send + 'static,
) {
    std::thread::spawn(move || {
        if let Err(why) = zbus::block_on(listen(trigger.as_deref(), &pressed)) {
            missing(why);
        }
    });
}

async fn listen(trigger: Option<&str>, pressed: &impl Fn()) -> Result<(), String> {
    let text = |e: ashpd::Error| e.to_string();
    let portal = GlobalShortcuts::new().await.map_err(text)?;
    let session = portal
        .create_session(CreateSessionOptions::default())
        .await
        .map_err(text)?;
    let capture = NewShortcut::new("capture", "Take a screenshot").preferred_trigger(trigger);
    portal
        .bind_shortcuts(&session, &[capture], None, BindShortcutsOptions::default())
        .await
        .map_err(text)?
        .response()
        .map_err(text)?;
    let mut activations = portal.receive_activated().await.map_err(text)?;
    while let Some(activated) = activations.next().await {
        if activated.shortcut_id() == "capture" {
            pressed();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Review Focus 4.
    #[test]
    fn shortcut_names_become_portal_triggers() {
        assert_eq!(
            xdg_trigger("Ctrl+Shift+Digit1").as_deref(),
            Some("CTRL+SHIFT+1")
        );
        assert_eq!(xdg_trigger("PrintScreen").as_deref(), Some("Print"));
        assert_eq!(xdg_trigger("Alt+KeyQ").as_deref(), Some("ALT+q"));
        assert_eq!(xdg_trigger("Super+F5").as_deref(), Some("LOGO+F5"));
        assert_eq!(xdg_trigger("Ctrl+NoSuchKey"), None);
    }

    /// A session bus with no portal (CI runs it under `dbus-run-session`): the reason comes back, nothing hangs.
    #[test]
    #[ignore = "needs a session bus without xdg-desktop-portal: CI runs it under dbus-run-session"]
    fn linux_no_portal_reports_why() {
        let (tx, rx) = std::sync::mpsc::channel();
        start(
            Some("CTRL+SHIFT+1".into()),
            || {},
            move |why| {
                let _ = tx.send(why);
            },
        );
        let why = rx.recv_timeout(std::time::Duration::from_secs(30)).unwrap();
        assert_ne!(why, "", "the reason is shown to the user");
    }
}
