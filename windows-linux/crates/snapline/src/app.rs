//! The tray app: tray icon and menu, capture shortcuts, and one capture at a time.

use crate::actions;
use crate::capture;
use crate::icon::{self, TrayState};
use crate::platform::{self, show_error};
use crate::popup_window::PopupWindow;
use crate::secrets::Keyring;
use crate::settings::{self, Settings, TrayClick};
use crate::upload_job::{self, CachedToken, Job, Outcome};
use crate::window::OverlayWindow;
use chrono::Local;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use snapline_core::naming;
use snapline_overlay::input::{Key, Mods};
use snapline_overlay::popup::{self, Popup, PopupButton, PopupState};
use snapline_overlay::session::{Command, CursorKind, Effect, Redraw, Session};
use snapline_overlay::text::TextFont;
use snapline_overlay::{Point, Rect};
use snapline_upload::http::UreqTransport;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, NamedKey, PhysicalKey};
use winit::window::{CursorIcon, WindowId};

pub enum UserEvent {
    HotKey,
    /// Capture after the tray menu has faded out (sent shortly after a menu click).
    CaptureNow,
    Menu(MenuId),
    Tray(TrayIconEvent),
    UploadProgress(f64),
    UploadFinished(Outcome),
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
    why: MenuId,
    /// Why some shortcuts did not register, shown from the tray menu.
    shortcut_note: Option<String>,
    hotkeys: Option<GlobalHotKeyManager>,
    capture: Option<Capture>,
    started: bool,
    secrets: Option<Keyring>,
    settings_item: MenuId,
    popup: Option<(Popup, PopupWindow)>,
    popup_close_at: Option<Instant>,
    popup_hovered: bool,
    /// Where the pointer last was over the popup, for clicks right after the popup changed under it.
    popup_pointer: Option<Point>,
    icon_reset_at: Option<Instant>,
    /// The last upload, for Try again and the search that follows it.
    last_job: Option<Job>,
    uploading: bool,
    google_token: Option<CachedToken>,
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
            why: MenuId::new("why"),
            shortcut_note: None,
            hotkeys: None,
            capture: None,
            started: false,
            secrets: None,
            settings_item: MenuId::new("upload-settings"),
            popup: None,
            popup_close_at: None,
            popup_hovered: false,
            popup_pointer: None,
            icon_reset_at: None,
            last_job: None,
            uploading: false,
            google_token: None,
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
        match Keyring::open() {
            Ok(store) => self.secrets = Some(store),
            Err(e) => show_error(&format!(
                "Snapline cannot reach Credential Manager, so uploads are off: {e}"
            )),
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
    /// When none work, says so at once; when some fail, the tray menu offers the explanation.
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
        let (working, failed): (Vec<String>, Vec<String>) =
            self.settings.hotkeys.iter().cloned().partition(|name| {
                name.parse::<HotKey>()
                    .is_ok_and(|h| manager.register(h).is_ok())
            });
        self.shortcut_note = settings::shortcut_note(&failed, &working);
        if working.is_empty()
            && let Some(note) = &self.shortcut_note
        {
            show_error(note);
        }
        self.hotkeys = Some(manager);
        working
            .iter()
            .map(|n| settings::shortcut_label(n))
            .collect()
    }

    fn build_tray(&self, labels: &[String]) -> Result<TrayIcon, String> {
        let title = if labels.is_empty() {
            "Take Screenshot".to_owned()
        } else {
            format!("Take Screenshot\t{}", labels.join(", "))
        };
        let menu = Menu::new();
        menu.append(&MenuItem::with_id(self.take.clone(), title, true, None))
            .map_err(|e| e.to_string())?;
        if self.shortcut_note.is_some() && !labels.is_empty() {
            let failed: Vec<String> = self
                .settings
                .hotkeys
                .iter()
                .map(|n| settings::shortcut_label(n))
                .filter(|l| !labels.contains(l))
                .collect();
            let why = format!("Why doesn't {} work?", failed.join(" or "));
            menu.append(&MenuItem::with_id(self.why.clone(), why, true, None))
                .map_err(|e| e.to_string())?;
        }
        menu.append_items(&[
            &MenuItem::with_id(self.settings_item.clone(), "Upload Settings…", true, None),
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id(self.quit.clone(), "Quit Snapline", true, None),
        ])
        .map_err(|e| e.to_string())?;
        let icon = Icon::from_rgba(icon::tray_rgba(32, icon::TrayState::Normal), 32, 32)
            .map_err(|e| e.to_string())?;
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
        let session = Session::new(
            screens.into_iter().map(|s| (s.image, s.scale)).collect(),
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
            w.fit_to_monitor();
            w.window.set_visible(true);
            w.window.set_cursor(CursorIcon::Crosshair);
        }
        cap.windows[cap.session.focused].window.focus_window();
        self.capture = Some(cap);
    }

    fn apply(&mut self, event_loop: &ActiveEventLoop, i: usize, effect: Effect) {
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
        match command {
            Command::Upload | Command::Search(_) => {
                let search = if let Command::Search(engine) = command {
                    Some(engine)
                } else {
                    None
                };
                match image.encode_png() {
                    Ok(png) => {
                        let job = Job {
                            png,
                            file_name: naming::file_name(Local::now()),
                            search,
                        };
                        self.start_upload(job, event_loop);
                    }
                    Err(e) => show_error(&format!("Snapline could not make the PNG: {e}")),
                }
            }
            _ => match actions::run(command, &image, &self.settings) {
                Ok(Some(done)) => self.confirm(done, event_loop),
                Ok(None) => {}
                Err(e) => show_error(&e),
            },
        }
    }

    /// "Screenshot copied" / "Screenshot saved", which fades after a moment.
    fn confirm(&mut self, done: actions::Done, event_loop: &ActiveEventLoop) {
        let (title, detail) = match done {
            actions::Done::Copied { width, height } => {
                ("Screenshot copied", format!("{width} × {height} pixels"))
            }
            actions::Done::Saved(path) => (
                "Screenshot saved",
                path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                ),
            ),
        };
        self.show_popup(
            event_loop,
            PopupState::Message {
                title: title.into(),
                detail,
            },
            Some(2.5),
        );
    }

    fn start_upload(&mut self, job: Job, event_loop: &ActiveEventLoop) {
        if self.uploading {
            let detail = "Wait for the current upload to finish, then try again.".into();
            return self.show_popup(
                event_loop,
                PopupState::Message {
                    title: "Upload in progress".into(),
                    detail,
                },
                Some(2.5),
            );
        }
        let (Some(secrets), Some(path)) = (self.secrets, Settings::path()) else {
            let message = "Snapline cannot reach its settings or Credential Manager.".into();
            return self.show_popup(event_loop, PopupState::Failed { message }, None);
        };
        self.uploading = true;
        self.last_job = Some(job.clone());
        self.show_popup(
            event_loop,
            PopupState::Uploading {
                progress: 0.0,
                file: job.file_name.clone(),
            },
            None,
        );
        self.set_tray(TrayState::Progress(0.0));
        let proxy = self.proxy.clone();
        let token = self.google_token.clone();
        std::thread::spawn(move || {
            let last = std::sync::atomic::AtomicU32::new(0);
            let report = |p: f64| {
                let percent = (p * 100.0) as u32;
                if percent != last.swap(percent, std::sync::atomic::Ordering::Relaxed) {
                    let _ = proxy.send_event(UserEvent::UploadProgress(p));
                }
            };
            let outcome =
                upload_job::run(&job, &path, &secrets, &UreqTransport::new(), token, &report);
            let _ = proxy.send_event(UserEvent::UploadFinished(outcome));
        });
    }

    fn finish_upload(&mut self, outcome: Outcome, event_loop: &ActiveEventLoop) {
        self.uploading = false;
        match outcome {
            Outcome::Done { result, token } => {
                self.google_token = token;
                let link = result.page.to_string();
                copy_text(&link);
                if let Some(engine) = self.last_job.as_ref().and_then(|j| j.search) {
                    let _ = open::that(engine.url(&result.direct).as_str());
                }
                self.show_popup(event_loop, PopupState::Done { link }, Some(4.0));
                self.set_tray(TrayState::Success);
                self.icon_reset_at = Some(Instant::now() + Duration::from_secs(4));
            }
            Outcome::Failed(message) => {
                // A failure may come from a dead sign-in: never reuse its token.
                self.google_token = None;
                self.show_popup(event_loop, PopupState::Failed { message }, None);
                self.set_tray(TrayState::Failure);
                self.icon_reset_at = Some(Instant::now() + Duration::from_secs(6));
            }
        }
    }

    fn set_tray(&self, state: TrayState) {
        if let (Some(tray), Ok(icon)) = (
            &self.tray,
            Icon::from_rgba(icon::tray_rgba(32, state), 32, 32),
        ) {
            let _ = tray.set_icon(Some(icon));
        }
    }

    /// Shows `state` in the popup next to the tray icon, creating or moving its window.
    fn show_popup(
        &mut self,
        event_loop: &ActiveEventLoop,
        state: PopupState,
        close_after: Option<f64>,
    ) {
        let anchor = self.tray.as_ref().and_then(TrayIcon::rect).map(|r| {
            Rect::new(
                r.position.x,
                r.position.y,
                f64::from(r.size.width),
                f64::from(r.size.height),
            )
        });
        let centre = anchor.map_or((0, 0), |a| {
            ((a.x + a.width / 2.0) as i32, (a.y + a.height / 2.0) as i32)
        });
        let monitor = event_loop
            .available_monitors()
            .find(|m| {
                let (p, s) = (m.position(), m.size());
                centre.0 >= p.x
                    && centre.1 >= p.y
                    && i64::from(centre.0) < i64::from(p.x) + i64::from(s.width)
                    && i64::from(centre.1) < i64::from(p.y) + i64::from(s.height)
            })
            .or_else(|| event_loop.primary_monitor());
        let scale = monitor
            .as_ref()
            .map_or(1.0, winit::monitor::MonitorHandle::scale_factor);
        let work = platform::work_area(centre.0, centre.1)
            .or_else(|| {
                monitor.as_ref().map(|m| {
                    let (p, s) = (m.position(), m.size());
                    Rect::new(
                        f64::from(p.x),
                        f64::from(p.y),
                        f64::from(s.width),
                        f64::from(s.height),
                    )
                })
            })
            .unwrap_or(Rect::new(0.0, 0.0, 1280.0, 800.0));
        let model = Popup::new(state);
        let size = model.size(&self.font, scale);
        let at = popup::origin(anchor, size, work, 8.0 * scale);
        let position = PhysicalPosition::new(at.x as i32, at.y as i32);
        let dims = PhysicalSize::new(size.width.ceil() as u32, size.height.ceil() as u32);
        let window = match self.popup.take() {
            Some((_, mut window)) => {
                window.scale = scale;
                if let Err(e) = window.place(position, dims) {
                    eprintln!("Snapline: {e}");
                }
                window
            }
            None => match PopupWindow::open(event_loop, position, dims, scale) {
                Ok(window) => window,
                Err(e) => {
                    return show_error(&format!("Snapline could not show its status popup: {e}"));
                }
            },
        };
        self.popup = Some((model, window));
        // The pointer may still be over the popup: keep it, so a click on the new buttons works at once.
        self.redraw_popup();
        self.popup_close_at =
            close_after.map(|seconds| Instant::now() + Duration::from_secs_f64(seconds));
    }

    fn redraw_popup(&mut self) {
        if let Some((popup, window)) = &mut self.popup
            && let Err(e) = window.present(popup, &self.font)
        {
            eprintln!("Snapline: {e}");
        }
    }

    fn close_popup(&mut self) {
        self.popup = None;
        self.popup_close_at = None;
        self.popup_hovered = false;
        self.popup_pointer = None;
    }

    fn popup_event(&mut self, event_loop: &ActiveEventLoop, event: &WindowEvent) {
        match event {
            WindowEvent::RedrawRequested => self.redraw_popup(),
            WindowEvent::CursorMoved { position, .. } => {
                self.popup_hovered = true;
                let point = Point {
                    x: position.x,
                    y: position.y,
                };
                self.popup_pointer = Some(point);
                if let Some((popup, window)) = &mut self.popup {
                    let hit = popup.hit(&self.font, window.scale, point);
                    if popup.hover != hit {
                        popup.hover = hit;
                        window.window.request_redraw();
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.popup_hovered = false;
                self.popup_pointer = None;
                if let Some((popup, window)) = &mut self.popup {
                    popup.hover = None;
                    window.window.request_redraw();
                }
                // It was due to close while the pointer was on it: close a moment after the pointer leaves.
                if self.popup_close_at.is_some_and(|t| t <= Instant::now()) {
                    self.popup_close_at = Some(Instant::now() + Duration::from_millis(1500));
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } if state.is_pressed() => {
                // Hit-test where the pointer is now: the popup may have changed under a pointer that did not move.
                let pressed = self.popup.as_ref().zip(self.popup_pointer).and_then(
                    |((popup, window), point)| popup.hit(&self.font, window.scale, point),
                );
                if let Some(button) = pressed {
                    self.popup_button(button, event_loop);
                }
            }
            _ => {}
        }
    }

    fn popup_button(&mut self, button: PopupButton, event_loop: &ActiveEventLoop) {
        let link = match self.popup.as_ref().map(|(p, _)| &p.state) {
            Some(PopupState::Done { link }) => Some(link.clone()),
            _ => None,
        };
        match button {
            PopupButton::Copy => {
                if let Some(link) = link {
                    copy_text(&link);
                }
            }
            PopupButton::Open => {
                if let Some(link) = link {
                    let _ = open::that(link);
                }
                self.close_popup();
            }
            PopupButton::Settings => {
                platform::open_upload_settings();
                self.close_popup();
            }
            PopupButton::Retry => {
                if let Some(job) = self.last_job.clone() {
                    self.start_upload(job, event_loop);
                }
            }
            PopupButton::Close => self.close_popup(),
        }
    }
}

fn copy_text(text: &str) {
    if let Err(e) = arboard::Clipboard::new().and_then(|mut c| c.set_text(text.to_owned())) {
        eprintln!("Snapline: could not copy the link: {e}");
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

/// The overlay key for a key press. With Ctrl held on a non-Latin layout (Ctrl+С on Russian), the letter comes
/// from the key's position, so Ctrl shortcuts work on every layout. Latin layouts keep their own letters (AZERTY).
fn key_of(key: &WinitKey, physical: PhysicalKey, ctrl: bool) -> Option<Key> {
    if ctrl
        && let WinitKey::Character(s) = key
        && s.chars().next().is_some_and(|c| !c.is_ascii())
        && let Some(letter) = latin_letter(physical)
    {
        return Some(Key::Char(letter));
    }
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

/// The QWERTY letter printed on a key position, for Ctrl shortcuts.
fn latin_letter(physical: PhysicalKey) -> Option<char> {
    let PhysicalKey::Code(code) = physical else {
        return None;
    };
    let name = format!("{code:?}"); // "KeyC"
    let letter = name.strip_prefix("Key")?;
    let mut chars = letter.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c.to_ascii_lowercase()),
        _ => None,
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        if !self.started {
            self.started = true;
            self.start();
        }
    }

    /// Closes the popup and resets the tray icon on time, and sleeps until the next of those.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if self.popup_close_at.is_some_and(|t| t <= now) && !self.popup_hovered {
            self.close_popup();
        }
        if self.icon_reset_at.is_some_and(|t| t <= now) {
            self.icon_reset_at = None;
            self.set_tray(TrayState::Normal);
        }
        let close = self.popup_close_at.filter(|_| !self.popup_hovered);
        let next = [close, self.icon_reset_at].into_iter().flatten().min();
        event_loop.set_control_flow(next.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::HotKey | UserEvent::CaptureNow => self.open_capture(event_loop),
            UserEvent::Menu(id) if id == self.take => {
                // Windows fades the clicked menu item out; capturing at once would put it in the image.
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    let _ = proxy.send_event(UserEvent::CaptureNow);
                });
            }
            UserEvent::Menu(id) if id == self.quit => event_loop.exit(),
            UserEvent::Menu(id) if id == self.settings_item => platform::open_upload_settings(),
            UserEvent::UploadProgress(p) => {
                self.set_tray(TrayState::Progress(p));
                if let Some((popup, window)) = &mut self.popup
                    && let PopupState::Uploading { progress, .. } = &mut popup.state
                {
                    *progress = p;
                    window.window.request_redraw();
                }
            }
            UserEvent::UploadFinished(outcome) => self.finish_upload(outcome, event_loop),
            UserEvent::Menu(id) if id == self.why => {
                if let Some(note) = &self.shortcut_note {
                    platform::show_info(note);
                }
            }
            UserEvent::Tray(TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            }) if self.settings.tray_click == TrayClick::Capture => self.open_capture(event_loop),
            _ => {}
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self
            .popup
            .as_ref()
            .is_some_and(|(_, w)| w.window.id() == id)
        {
            return self.popup_event(event_loop, &event);
        }
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
                cap.windows[i].fit_to_monitor();
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
                match key_of(&event.logical_key, event.physical_key, cap.mods.ctrl) {
                    Some(key) => cap.session.key(key, cap.mods),
                    None => return,
                }
            }
            WindowEvent::CloseRequested => Effect::command(Command::Cancel),
            _ => return,
        };
        self.apply(event_loop, i, effect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::KeyCode;

    fn char_key(s: &str) -> WinitKey {
        WinitKey::Character(s.into())
    }

    // Review finding: on Russian or Greek layouts Ctrl+C reports "с", so the shortcut must come from the key position.
    #[test]
    fn ctrl_shortcuts_work_on_non_latin_layouts() {
        assert_eq!(
            key_of(&char_key("с"), PhysicalKey::Code(KeyCode::KeyC), true),
            Some(Key::Char('c'))
        );
        assert_eq!(
            key_of(&char_key("ς"), PhysicalKey::Code(KeyCode::KeyW), true),
            Some(Key::Char('w'))
        );
    }

    #[test]
    fn latin_layouts_keep_their_own_letters() {
        // AZERTY: the A key sits where QWERTY has Q; Ctrl+A must still select all.
        assert_eq!(
            key_of(&char_key("a"), PhysicalKey::Code(KeyCode::KeyQ), true),
            Some(Key::Char('a'))
        );
    }

    #[test]
    fn typing_without_ctrl_keeps_the_character() {
        assert_eq!(
            key_of(&char_key("с"), PhysicalKey::Code(KeyCode::KeyC), false),
            Some(Key::Char('с'))
        );
    }
}
