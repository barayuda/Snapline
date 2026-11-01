//! The tray icon and its menu. `tray-icon` draws it on Windows (and on macOS, for trying the app); on Linux a
//! `StatusNotifierItem` does (`linux_tray`), which panels show without GTK and which reports left clicks.

use crate::app::UserEvent;
use crate::icon::TrayState;
use snapline_overlay::Rect;
use winit::event_loop::EventLoopProxy;

/// What a tray menu item does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    Take,
    Why,
    Settings,
    Quit,
}

/// One line of the tray menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Item {
        label: String,
        /// Shown beside the label: the shortcuts that do the same.
        shortcut: Option<String>,
        action: MenuAction,
    },
    Separator,
}

/// "Take Screenshot" with the shortcuts that work; "Why doesn't … work?" when some failed but not all (when none
/// work, the app has already said why); then Settings and Quit.
#[must_use]
pub fn menu(working: &[String], failed: &[String]) -> Vec<Entry> {
    let item = |label: String, action| Entry::Item {
        label,
        shortcut: None,
        action,
    };
    let mut menu = vec![Entry::Item {
        label: "Take Screenshot".into(),
        shortcut: (!working.is_empty()).then(|| working.join(", ")),
        action: MenuAction::Take,
    }];
    if !failed.is_empty() && !working.is_empty() {
        let why = format!("Why doesn't {} work?", failed.join(" or "));
        menu.push(item(why, MenuAction::Why));
    }
    menu.extend([
        item("Settings…".into(), MenuAction::Settings),
        Entry::Separator,
        item("Quit Snapline".into(), MenuAction::Quit),
    ]);
    menu
}

/// Said once when the desktop shows no tray icons. `settings` is the command that opens the settings; `capture`, in
/// a Wayland session, the command to bind for a screenshot (the desktop owns the key there).
#[must_use]
pub fn no_tray_note(working: &[String], settings: &str, capture: Option<&str>) -> String {
    let take = match (capture, working.is_empty()) {
        (Some(capture), _) => format!(
            "Your desktop's capture shortcut still works; if it has none, bind `{capture}` in its keyboard settings."
        ),
        (None, true) => {
            format!("No capture shortcut works either. Run `{settings}` to choose one.")
        }
        (None, false) => format!("{} still takes a screenshot.", working.join(" or ")),
    };
    format!(
        "Your desktop shows no tray icons, so Snapline's icon is hidden. {take}\n\nOn GNOME, install the \
         \"AppIndicator and KStatusNotifierItem Support\" extension to see the icon. Settings open with \
         `{settings}`."
    )
}

/// The point whose monitor gets a popup: the middle of the tray icon, else the pointer (Linux gives no icon
/// position), else the top-left corner of the desktop.
#[must_use]
pub fn popup_monitor_point(tray: Option<Rect>, pointer: Option<(i32, i32)>) -> (i32, i32) {
    tray.map(|a| ((a.x + a.width / 2.0) as i32, (a.y + a.height / 2.0) as i32))
        .or(pointer)
        .unwrap_or((0, 0))
}

/// The tray icon. Its menu choices arrive as `UserEvent::Menu`, a left click as `UserEvent::TrayClicked`.
pub struct Tray(Backend);

#[cfg(not(target_os = "linux"))]
type Backend = tray_icon::TrayIcon;
#[cfg(target_os = "linux")]
type Backend = crate::linux_tray::LinuxTray;

impl Tray {
    /// `menu_on_click`: a left click opens the menu (true) or takes a screenshot (false).
    ///
    /// # Errors
    /// When the system has no tray or refuses the icon; the error is for the user.
    pub fn new(
        menu: &[Entry],
        menu_on_click: bool,
        proxy: EventLoopProxy<UserEvent>,
    ) -> Result<Self, String> {
        #[cfg(not(target_os = "linux"))]
        {
            windows_backend::new(menu, menu_on_click, proxy).map(Self)
        }
        #[cfg(target_os = "linux")]
        {
            crate::linux_tray::LinuxTray::new(menu, menu_on_click, proxy).map(Self)
        }
    }

    /// Replaces the menu in place: a new icon may land in the overflow, away from where the user pinned it.
    ///
    /// # Errors
    /// When the menu cannot be built.
    pub fn set_menu(&mut self, menu: &[Entry], menu_on_click: bool) -> Result<(), String> {
        #[cfg(not(target_os = "linux"))]
        {
            self.0
                .set_menu(Some(Box::new(windows_backend::build(menu)?)));
            self.0.set_show_menu_on_left_click(menu_on_click);
            Ok(())
        }
        #[cfg(target_os = "linux")]
        {
            self.0.set_menu(menu, menu_on_click)
        }
    }

    pub fn set_state(&self, state: TrayState) {
        #[cfg(not(target_os = "linux"))]
        if let Ok(icon) = windows_backend::icon(state) {
            let _ = self.0.set_icon(Some(icon));
        }
        #[cfg(target_os = "linux")]
        self.0.set_state(state);
    }

    /// Where the icon is, in physical pixels, when the system says (Linux does not).
    #[must_use]
    #[cfg_attr(
        target_os = "linux",
        allow(clippy::unused_self, reason = "Linux never says where the icon is")
    )]
    pub fn rect(&self) -> Option<Rect> {
        #[cfg(not(target_os = "linux"))]
        {
            self.0.rect().map(|r| {
                Rect::new(
                    r.position.x,
                    r.position.y,
                    f64::from(r.size.width),
                    f64::from(r.size.height),
                )
            })
        }
        #[cfg(target_os = "linux")]
        {
            None
        }
    }
}

/// `tray-icon`, moved here unchanged from `app.rs`.
#[cfg(not(target_os = "linux"))]
mod windows_backend {
    use super::{Entry, MenuAction};
    use crate::app::UserEvent;
    use crate::icon::{self, TrayState};
    use std::sync::Mutex;
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{
        Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    };
    use winit::event_loop::EventLoopProxy;

    const IDS: [(&str, MenuAction); 4] = [
        ("take", MenuAction::Take),
        ("why", MenuAction::Why),
        ("settings", MenuAction::Settings),
        ("quit", MenuAction::Quit),
    ];

    pub fn new(
        menu: &[Entry],
        menu_on_click: bool,
        proxy: EventLoopProxy<UserEvent>,
    ) -> Result<TrayIcon, String> {
        let p = Mutex::new(proxy.clone());
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            let action = IDS.iter().find(|(id, _)| *id == e.id.0).map(|(_, a)| *a);
            if let (Some(action), Ok(p)) = (action, p.lock()) {
                let _ = p.send_event(UserEvent::Menu(action));
            }
        }));
        let p = Mutex::new(proxy);
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = e
                && let Ok(p) = p.lock()
            {
                let _ = p.send_event(UserEvent::TrayClicked);
            }
        }));
        TrayIconBuilder::new()
            .with_icon(icon(TrayState::Normal)?)
            .with_tooltip("Snapline")
            .with_menu(Box::new(build(menu)?))
            .with_menu_on_left_click(menu_on_click)
            .build()
            .map_err(|e| e.to_string())
    }

    pub fn build(entries: &[Entry]) -> Result<Menu, String> {
        let menu = Menu::new();
        for entry in entries {
            match entry {
                Entry::Separator => menu.append(&PredefinedMenuItem::separator()),
                Entry::Item {
                    label,
                    shortcut,
                    action,
                } => {
                    let id = IDS
                        .iter()
                        .find(|(_, a)| a == action)
                        .map_or("take", |(id, _)| *id);
                    let text = shortcut
                        .as_ref()
                        .map_or_else(|| label.clone(), |s| format!("{label}\t{s}"));
                    menu.append(&MenuItem::with_id(id, text, true, None))
                }
            }
            .map_err(|e| e.to_string())?;
        }
        Ok(menu)
    }

    pub fn icon(state: TrayState) -> Result<Icon, String> {
        Icon::from_rgba(icon::tray_rgba(32, state), 32, 32).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_lists_working_shortcuts_and_asks_about_failed_ones() {
        let all = menu(&["PrtScn".into(), "Ctrl+Shift+1".into()], &[]);
        assert_eq!(
            all[0],
            Entry::Item {
                label: "Take Screenshot".into(),
                shortcut: Some("PrtScn, Ctrl+Shift+1".into()),
                action: MenuAction::Take
            }
        );
        assert_eq!(all.len(), 4, "Take, Settings, separator, Quit: {all:?}");
        let some = menu(&["Ctrl+Shift+1".into()], &["PrtScn".into()]);
        assert!(
            matches!(&some[1], Entry::Item { label, action: MenuAction::Why, .. } if label == "Why doesn't PrtScn work?")
        );
        // None work: the app already said why, and the menu has nothing to ask.
        let none = menu(&[], &["PrtScn".into()]);
        assert!(matches!(&none[0], Entry::Item { shortcut: None, .. }));
        assert!(!none.iter().any(|e| matches!(
            e,
            Entry::Item {
                action: MenuAction::Why,
                ..
            }
        )));
    }

    // Final review: Linux gives no tray position, and (0, 0) put popups on the leftmost monitor. Without one, the
    // pointer picks the monitor (the popup then sits in its bottom-right corner).
    #[test]
    fn the_pointer_picks_the_popup_monitor_when_the_tray_has_no_position() {
        let tray = Rect::new(1800.0, 1050.0, 24.0, 24.0);
        assert_eq!(
            popup_monitor_point(Some(tray), Some((10, 10))),
            (1812, 1062)
        );
        assert_eq!(popup_monitor_point(None, Some((2500, 300))), (2500, 300));
        assert_eq!(popup_monitor_point(None, None), (0, 0));
    }

    // Review Focus 3: GNOME without the AppIndicator extension, i3.
    #[test]
    fn no_tray_note_says_how_to_get_the_icon() {
        let settings = "flatpak run dev.barayuda.Snapline --settings";
        let note = no_tray_note(&["Ctrl+Shift+1".into()], settings, None);
        assert!(
            note.contains("Ctrl+Shift+1")
                && note.contains("AppIndicator")
                && note.contains(settings),
            "{note}"
        );
        let note = no_tray_note(&[], settings, None);
        assert!(note.contains("choose one"), "{note}");
        // Phase 6b: on Wayland the desktop owns the key, and the capture command is what to bind.
        let capture = "snapline --capture";
        let note = no_tray_note(&[], settings, Some(capture));
        assert!(
            note.contains(capture) && !note.contains("No capture shortcut works"),
            "{note}"
        );
    }
}
