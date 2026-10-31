//! The Linux tray icon: a `StatusNotifierItem` over D-Bus, shown by KDE, Xfce, Cinnamon, MATE, `LXQt` and by GNOME with
//! the `AppIndicator` extension. Menu choices and left clicks reach the app as `UserEvent`s.

use crate::app::UserEvent;
use crate::icon::{self, TrayState};
use crate::tray::Entry;
use ksni::blocking::{Handle, TrayMethods};
use winit::event_loop::EventLoopProxy;

/// ksni wants ARGB32 in network byte order; Snapline draws RGBA.
#[must_use]
pub fn argb(rgba: &[u8]) -> Vec<u8> {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .flat_map(|[r, g, b, a]| [*a, *r, *g, *b])
        .collect()
}

fn pixmap(state: TrayState) -> ksni::Icon {
    ksni::Icon {
        width: 32,
        height: 32,
        data: argb(&icon::tray_rgba(32, state)),
    }
}

/// `MENU` is ksni's `MENU_ON_ACTIVATE`, which is a constant: a left click opens the menu (true) or is sent to the
/// app (false). Changing the setting therefore starts the other kind of item.
struct Item<const MENU: bool> {
    menu: Vec<Entry>,
    icon: ksni::Icon,
    proxy: EventLoopProxy<UserEvent>,
}

impl<const MENU: bool> ksni::Tray for Item<MENU> {
    const MENU_ON_ACTIVATE: bool = MENU;

    fn id(&self) -> String {
        "snapline".into()
    }

    fn title(&self) -> String {
        "Snapline".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Snapline".into(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.proxy.send_event(UserEvent::TrayClicked);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        self.menu
            .iter()
            .map(|entry| match entry {
                Entry::Separator => ksni::MenuItem::Separator,
                Entry::Item {
                    label,
                    shortcut,
                    action,
                } => {
                    let action = *action;
                    ksni::menu::StandardItem {
                        label: shortcut
                            .as_ref()
                            .map_or_else(|| label.clone(), |s| format!("{label}    {s}")),
                        activate: Box::new(move |this: &mut Self| {
                            let _ = this.proxy.send_event(UserEvent::Menu(action));
                        }),
                        ..Default::default()
                    }
                    .into()
                }
            })
            .collect()
    }

    fn watcher_offline(&self, _reason: ksni::OfflineReason) -> bool {
        let _ = self.proxy.send_event(UserEvent::TrayMissing);
        true // keep the item: it shows as soon as a panel starts
    }
}

enum Running {
    Menu(Handle<Item<true>>),
    Capture(Handle<Item<false>>),
}

pub struct LinuxTray {
    running: Running,
    proxy: EventLoopProxy<UserEvent>,
}

impl LinuxTray {
    pub fn new(
        menu: &[Entry],
        menu_on_click: bool,
        proxy: EventLoopProxy<UserEvent>,
    ) -> Result<Self, String> {
        let running = spawn(menu, pixmap(TrayState::Normal), menu_on_click, &proxy)?;
        Ok(Self { running, proxy })
    }

    pub fn set_menu(&mut self, menu: &[Entry], menu_on_click: bool) -> Result<(), String> {
        let menu = menu.to_vec();
        match (&self.running, menu_on_click) {
            (Running::Menu(h), true) => drop(h.update(|t| t.menu = menu)),
            (Running::Capture(h), false) => drop(h.update(|t| t.menu = menu)),
            (old, _) => {
                match old {
                    Running::Menu(h) => h.shutdown().wait(),
                    Running::Capture(h) => h.shutdown().wait(),
                }
                self.running = spawn(&menu, pixmap(TrayState::Normal), menu_on_click, &self.proxy)?;
            }
        }
        Ok(())
    }

    pub fn set_state(&self, state: TrayState) {
        let icon = pixmap(state);
        match &self.running {
            Running::Menu(h) => drop(h.update(|t| t.icon = icon)),
            Running::Capture(h) => drop(h.update(|t| t.icon = icon)),
        }
    }
}

/// Starts the item even when no panel is up yet (login): `watcher_offline` then tells the app.
fn spawn(
    menu: &[Entry],
    icon: ksni::Icon,
    menu_on_click: bool,
    proxy: &EventLoopProxy<UserEvent>,
) -> Result<Running, String> {
    let menu = menu.to_vec();
    let proxy = proxy.clone();
    let failed = |e: ksni::Error| format!("the desktop's tray service did not answer ({e})");
    if menu_on_click {
        Item::<true> { menu, icon, proxy }
            .assume_sni_available(true)
            .spawn()
            .map(Running::Menu)
            .map_err(failed)
    } else {
        Item::<false> { menu, icon, proxy }
            .assume_sni_available(true)
            .spawn()
            .map(Running::Capture)
            .map_err(failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argb_puts_alpha_first() {
        assert_eq!(argb(&[1, 2, 3, 4, 5, 6, 7, 8]), [4, 1, 2, 3, 8, 5, 6, 7]);
    }

    /// Starts the item on a session bus with no panel. CI runs it under `dbus-run-session`.
    #[test]
    #[ignore = "needs an X server and a session bus: CI runs it under xvfb-run and dbus-run-session"]
    fn linux_tray_starts_without_a_panel() {
        use winit::platform::x11::EventLoopBuilderExtX11;
        let event_loop = winit::event_loop::EventLoop::<UserEvent>::with_user_event()
            .with_any_thread(true)
            .build()
            .unwrap();
        let menu = crate::tray::menu(&["Ctrl+Shift+1".into()], &[]);
        let mut tray = LinuxTray::new(&menu, true, event_loop.create_proxy()).unwrap();
        tray.set_state(TrayState::Success);
        tray.set_menu(&menu, false).unwrap(); // switches to the other kind of item
        assert!(matches!(tray.running, Running::Capture(_)));
    }
}
