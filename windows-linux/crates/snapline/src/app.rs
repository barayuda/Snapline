//! The tray app: tray icon and menu, capture shortcuts, and one capture at a time.

use crate::actions;
use crate::capture;
use crate::icon;
use crate::platform::{self, show_error};
use crate::settings::{self, Settings, TrayClick};
use crate::window::OverlayWindow;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use snapline_overlay::input::{Key, Mods};
use snapline_overlay::session::{Command, CursorKind, Effect, Redraw, Session};
use snapline_overlay::text::TextFont;
use snapline_overlay::{Point, Rect};
use std::sync::Mutex;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};
use winit::application::ApplicationHandler;
use winit::event::{MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::{CursorIcon, WindowId};

pub enum UserEvent {
    HotKey,
    Menu(MenuId),
    Tray(TrayIconEvent),
}

struct Capture {
    session: Session,
    windows: Vec<OverlayWindow>,
    pointer: Vec<Point>,
    mods: Mods,
}

pub struct App {
    proxy: EventLoopProxy<UserEvent>,
    settings: Settings,
    font: TextFont,
    tray: Option<TrayIcon>,
    take: MenuId,
    quit: MenuId,
    hotkeys: Option<GlobalHotKeyManager>,
    capture: Option<Capture>,
    started: bool,
}

impl App {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            proxy,
            settings: Settings::default(),
            font: TextFont::system(),
            tray: None,
            take: MenuId::new("take"),
            quit: MenuId::new("quit"),
            hotkeys: None,
            capture: None,
            started: false,
        }
    }

    /// Settings, event forwarding, shortcuts and the tray. Runs once, when the event loop is up.
    fn start(&mut self) {
        if let Some(path) = Settings::path() {
            let (settings, problem) = Settings::load(&path);
            self.settings = settings;
            if let Some(problem) = problem {
                show_error(&format!(
                    "Snapline is using its default settings because this file could not be read:\n\n{problem}"
                ));
            }
        }
        let proxy = Mutex::new(self.proxy.clone());
        GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
            if e.state == HotKeyState::Pressed
                && let Ok(p) = proxy.lock()
            {
                let _ = p.send_event(UserEvent::HotKey);
            }
        }));
        let proxy = Mutex::new(self.proxy.clone());
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if let Ok(p) = proxy.lock() {
                let _ = p.send_event(UserEvent::Menu(e.id));
            }
        }));
        let proxy = Mutex::new(self.proxy.clone());
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let Ok(p) = proxy.lock() {
                let _ = p.send_event(UserEvent::Tray(e));
            }
        }));

        let labels = self.register_hotkeys();
        match self.build_tray(&labels) {
            Ok(tray) => self.tray = Some(tray),
            Err(e) => show_error(&format!("Snapline could not add its tray icon: {e}")),
        }
    }

    /// Registers every configured shortcut it can, and returns the labels of those that worked.
    fn register_hotkeys(&mut self) -> Vec<String> {
        let manager = match GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                show_error(&format!(
                    "Snapline could not set up its capture shortcuts: {e}"
                ));
                return Vec::new();
            }
        };
        let working: Vec<String> = self
            .settings
            .hotkeys
            .iter()
            .filter(|name| {
                name.parse::<HotKey>()
                    .is_ok_and(|h| manager.register(h).is_ok())
            })
            .map(|name| settings::shortcut_label(name))
            .collect();
        if working.is_empty() {
            let wanted: Vec<String> = self
                .settings
                .hotkeys
                .iter()
                .map(|n| settings::shortcut_label(n))
                .collect();
            let file =
                Settings::path().map_or_else(String::new, |p| format!(" in {}", p.display()));
            show_error(&format!(
                "Snapline could not use its capture shortcuts ({}). Another app may be using them. On Windows 11, \
                 PrtScn often opens the Snipping Tool: turn that off in Settings → Accessibility → Keyboard. \
                 You can still take a screenshot from the tray icon, or set other keys under hotkeys{file}.",
                wanted.join(", ")
            ));
        }
        self.hotkeys = Some(manager);
        working
    }

    fn build_tray(&self, labels: &[String]) -> Result<TrayIcon, String> {
        let title = if labels.is_empty() {
            "Take Screenshot".to_owned()
        } else {
            format!("Take Screenshot\t{}", labels.join(", "))
        };
        let menu = Menu::new();
        menu.append_items(&[
            &MenuItem::with_id(self.take.clone(), title, true, None),
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id(self.quit.clone(), "Quit Snapline", true, None),
        ])
        .map_err(|e| e.to_string())?;
        let icon = Icon::from_rgba(icon::tray_rgba(32), 32, 32).map_err(|e| e.to_string())?;
        TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Snapline")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(self.settings.tray_click == TrayClick::Menu)
            .build()
            .map_err(|e| e.to_string())
    }

    fn open_capture(&mut self, event_loop: &ActiveEventLoop) {
        if self.capture.is_some() {
            return; // the shortcut pressed again during a capture does nothing
        }
        let screens = match capture::capture_all() {
            Ok(s) if !s.is_empty() => s,
            Ok(_) => return,
            Err(e) => return show_error(&format!("Snapline could not capture the screen: {e}")),
        };
        let frames: Vec<Rect> = screens
            .iter()
            .map(|s| {
                Rect::new(
                    f64::from(s.x),
                    f64::from(s.y),
                    f64::from(s.image.width()),
                    f64::from(s.image.height()),
                )
            })
            .collect();
        let mut windows = Vec::new();
        for screen in &screens {
            match OverlayWindow::open(event_loop, screen) {
                Ok(w) => windows.push(w),
                Err(e) => {
                    return show_error(&format!("Snapline could not open its capture window: {e}"));
                }
            }
        }
        let scales: Vec<f64> = windows.iter().map(|w| w.window.scale_factor()).collect();
        let session = Session::new(
            screens.into_iter().map(|s| s.image).zip(scales).collect(),
            self.font.clone(),
        );
        let n = windows.len();
        let mut cap = Capture {
            session,
            windows,
            pointer: vec![Point::default(); n],
            mods: Mods::default(),
        };
        cap.session.focused = platform::cursor_position()
            .and_then(|(x, y)| {
                let (x, y) = (f64::from(x), f64::from(y));
                frames
                    .iter()
                    .position(|f| x >= f.x && x < f.x + f.width && y >= f.y && y < f.y + f.height)
            })
            .unwrap_or(0);
        for (k, w) in cap.windows.iter_mut().enumerate() {
            if let Err(e) = w.present(&cap.session, k) {
                eprintln!("Snapline: {e}");
            }
            w.window.set_visible(true);
            w.window.set_cursor(CursorIcon::Crosshair);
        }
        cap.windows[cap.session.focused].window.focus_window();
        self.capture = Some(cap);
    }

    fn apply(&mut self, i: usize, effect: Effect) {
        let Some(cap) = self.capture.as_mut() else {
            return;
        };
        if effect.beep {
            platform::beep();
        }
        match effect.redraw {
            Redraw::None => {}
            Redraw::One(k) => cap.windows[k].window.request_redraw(),
            Redraw::All => cap.windows.iter().for_each(|w| w.window.request_redraw()),
        }
        cap.windows[i]
            .window
            .set_cursor(cursor_icon(cap.session.cursor(i)));
        let Some(command) = effect.command else {
            return;
        };
        let image = cap.session.export();
        self.capture = None; // closes the overlay windows before any dialog opens
        if command == Command::Cancel {
            return;
        }
        let Some(image) = image else {
            return platform::beep();
        };
        if let Err(e) = actions::run(command, &image, &self.settings) {
            show_error(&e);
        }
    }
}

fn cursor_icon(kind: CursorKind) -> CursorIcon {
    match kind {
        CursorKind::Crosshair => CursorIcon::Crosshair,
        CursorKind::Arrow => CursorIcon::Default,
        CursorKind::Move => CursorIcon::Move,
        CursorKind::Text => CursorIcon::Text,
        CursorKind::ResizeHorizontal => CursorIcon::EwResize,
        CursorKind::ResizeVertical => CursorIcon::NsResize,
        CursorKind::ResizeDiagonalDown => CursorIcon::NwseResize,
        CursorKind::ResizeDiagonalUp => CursorIcon::NeswResize,
    }
}

fn key_of(key: &WinitKey) -> Option<Key> {
    Some(match key {
        WinitKey::Named(NamedKey::Escape) => Key::Escape,
        WinitKey::Named(NamedKey::Enter) => Key::Enter,
        WinitKey::Named(NamedKey::Backspace) => Key::Backspace,
        WinitKey::Named(NamedKey::ArrowLeft) => Key::Left,
        WinitKey::Named(NamedKey::ArrowRight) => Key::Right,
        WinitKey::Named(NamedKey::ArrowUp) => Key::Up,
        WinitKey::Named(NamedKey::ArrowDown) => Key::Down,
        WinitKey::Named(NamedKey::Space) => Key::Char(' '),
        WinitKey::Character(s) => Key::Char(s.chars().next()?),
        _ => return None,
    })
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        if !self.started {
            self.started = true;
            self.start();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::HotKey => self.open_capture(event_loop),
            UserEvent::Menu(id) if id == self.take => self.open_capture(event_loop),
            UserEvent::Menu(id) if id == self.quit => event_loop.exit(),
            UserEvent::Tray(TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            }) if self.settings.tray_click == TrayClick::Capture => self.open_capture(event_loop),
            _ => {}
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(cap) = self.capture.as_mut() else {
            return;
        };
        let Some(i) = cap.windows.iter().position(|w| w.window.id() == id) else {
            return;
        };
        let effect = match event {
            WindowEvent::RedrawRequested => {
                if let Err(e) = cap.windows[i].present(&cap.session, i) {
                    eprintln!("Snapline: {e}");
                }
                return;
            }
            // Keep the window exactly the monitor's pixels when Windows moves it between scales.
            WindowEvent::ScaleFactorChanged {
                mut inner_size_writer,
                ..
            } => {
                let _ = inner_size_writer.request_inner_size(cap.windows[i].size());
                return;
            }
            WindowEvent::ModifiersChanged(m) => {
                let state = m.state();
                cap.mods = Mods {
                    ctrl: state.control_key(),
                    shift: state.shift_key(),
                    alt: state.alt_key(),
                };
                return;
            }
            WindowEvent::CursorMoved { position, .. } => {
                let p = Point {
                    x: position.x,
                    y: position.y,
                };
                cap.pointer[i] = p;
                cap.session.pointer_move(i, p)
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if state.is_pressed() {
                    cap.session.pointer_down(i, cap.pointer[i], cap.mods)
                } else {
                    cap.session.pointer_up(i, cap.pointer[i])
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y),
                    MouseScrollDelta::PixelDelta(p) => p.y,
                };
                cap.session.scroll(i, cap.pointer[i], lines)
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                match key_of(&event.logical_key) {
                    Some(key) => cap.session.key(key, cap.mods),
                    None => return,
                }
            }
            WindowEvent::CloseRequested => Effect::command(Command::Cancel),
            _ => return,
        };
        self.apply(i, effect);
    }
}
