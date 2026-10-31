//! The tray app: tray icon and menu, capture shortcuts, and one capture at a time.

use crate::actions;
use crate::capture;
use crate::icon::TrayState;
use crate::platform::{self, show_error};
use crate::popup_window::PopupWindow;
use crate::recognize;
use crate::secrets::Keyring;
use crate::settings::{self, Settings, TrayClick};
use crate::share;
use crate::tray::{self, MenuAction, Tray};
use crate::upload_job::{self, CachedToken, Job, Outcome};
use crate::window::OverlayWindow;
use crate::worker::OnPanic;
use chrono::Local;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use snapline_core::naming;
use snapline_core::sensitive_text::SensitiveCategory;
use snapline_overlay::input::{Key, Mods};
use snapline_overlay::popup::{self, Popup, PopupButton, PopupState};
use snapline_overlay::scan::{self, HideBox};
use snapline_overlay::session::{Command, CursorKind, Effect, Redraw, Session};
use snapline_overlay::text::TextFont;
use snapline_overlay::{Point, Rect};
use snapline_upload::http::UreqTransport;
use std::sync::Mutex;
use std::time::{Duration, Instant};
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
    Menu(MenuAction),
    /// A left click on the tray icon (a screenshot when the setting says so).
    TrayClicked,
    /// The desktop shows no tray icons.
    #[cfg_attr(
        not(target_os = "linux"),
        allow(dead_code, reason = "only the Linux tray reports a missing panel")
    )]
    TrayMissing,
    UploadProgress(f64),
    UploadFinished(Outcome),
    /// Auto-hide finished reading screen `screen` of capture `capture`: its boxes, or why it could not run.
    /// With the boxes comes why a step is missing, when only text or only face detection worked.
    ScanDone {
        capture: u64,
        screen: usize,
        result: Result<(Vec<HideBox>, Option<String>), String>,
    },
    /// Copy Text finished: the text (maybe empty), or a sentence saying why it could not run.
    TextRead(Result<String, String>),
    /// The settings window saved a change.
    SettingsChanged,
    /// The user shared or cancelled: Share's helper window can close.
    ShareClosed,
}

struct Capture {
    /// Tells this capture's scan results from those of a capture already closed.
    id: u64,
    session: Session,
    windows: Vec<OverlayWindow>,
    pointer: Vec<Point>,
    mods: Mods,
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "independent flags of the running app, not a state machine"
)]
pub struct App {
    proxy: EventLoopProxy<UserEvent>,
    settings: Settings,
    font: TextFont,
    tray: Option<Tray>,
    /// The shortcuts that work now, by label.
    labels: Vec<String>,
    told_tray_missing: bool,
    /// Why some shortcuts did not register, shown from the tray menu.
    shortcut_note: Option<String>,
    hotkeys: Option<GlobalHotKeyManager>,
    /// The shortcuts registered now, to drop them when the settings change.
    registered: Vec<HotKey>,
    capture: Option<Capture>,
    started: bool,
    secrets: Option<Keyring>,
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
    captures: u64,
    /// Share's helper window, while Share may still be open.
    share: Option<share::Open>,
}

impl App {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            proxy,
            settings: Settings::default(),
            font: TextFont::system(),
            tray: None,
            labels: Vec::new(),
            told_tray_missing: false,
            shortcut_note: None,
            hotkeys: None,
            registered: Vec::new(),
            capture: None,
            started: false,
            secrets: None,
            popup: None,
            popup_close_at: None,
            popup_hovered: false,
            popup_pointer: None,
            icon_reset_at: None,
            last_job: None,
            uploading: false,
            google_token: None,
            captures: 0,
            share: None,
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
                "Snapline cannot reach {}, so uploads are off: {e}",
                platform::SECRET_STORE
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

        let proxy = self.proxy.clone();
        platform::watch_settings(move || {
            let _ = proxy.send_event(UserEvent::SettingsChanged);
        });

        let labels = self.register_hotkeys();
        self.add_tray(&labels);
    }

    /// Registers every configured shortcut it can, and returns the labels of those that worked.
    /// When none work, says so at once; when some fail, the tray menu offers the explanation.
    /// Registers the shortcuts in `self.settings`, first dropping any registered before (a settings change),
    /// and returns the labels of those that work.
    fn register_hotkeys(&mut self) -> Vec<String> {
        if self.hotkeys.is_none() {
            match GlobalHotKeyManager::new() {
                Ok(m) => self.hotkeys = Some(m),
                Err(e) => {
                    show_error(&format!(
                        "Snapline could not set up its capture shortcuts: {e}"
                    ));
                    return Vec::new();
                }
            }
        }
        let Some(manager) = &self.hotkeys else {
            return Vec::new();
        };
        let _ = manager.unregister_all(&self.registered);
        let mut registered = Vec::new();
        let (working, failed) = settings::split_shortcuts(&self.settings.hotkeys, |name| {
            let Ok(key) = name.parse::<HotKey>() else {
                return false;
            };
            let ok = manager.register(key).is_ok();
            if ok {
                registered.push(key);
            }
            ok
        });
        self.registered = registered;
        self.shortcut_note = settings::shortcut_note(&failed, &working);
        if working.is_empty()
            && let Some(note) = &self.shortcut_note
        {
            show_error(note);
        }
        let labels: Vec<String> = working
            .iter()
            .map(|n| settings::shortcut_label(n))
            .collect();
        self.labels.clone_from(&labels);
        labels
    }

    /// The settings window saved a change: re-read the file, register the shortcuts again and update the tray
    /// menu, so the menu labels and the tray click follow. A file that cannot be read changes nothing.
    fn reload_settings(&mut self) {
        let Some(path) = Settings::path() else { return };
        let (settings, problem) = Settings::load(&path);
        if problem.is_some() {
            return;
        }
        self.settings = settings;
        let labels = self.register_hotkeys();
        let (menu, menu_on_click) = (
            self.menu(&labels),
            self.settings.tray_click == TrayClick::Menu,
        );
        // Change the existing icon: a new one may land in the overflow, away from where the user pinned it.
        match &mut self.tray {
            Some(tray) => {
                if let Err(e) = tray.set_menu(&menu, menu_on_click) {
                    show_error(&format!("Snapline could not update its tray menu: {e}"));
                }
            }
            None => self.add_tray(&labels),
        }
    }

    /// The tray menu for the shortcuts that work (`labels`) and those that failed.
    fn menu(&self, labels: &[String]) -> Vec<tray::Entry> {
        let failed: Vec<String> = if self.shortcut_note.is_some() {
            self.settings
                .hotkeys
                .iter()
                .map(|n| settings::shortcut_label(n))
                .filter(|l| !labels.contains(l))
                .collect()
        } else {
            Vec::new()
        };
        tray::menu(labels, &failed)
    }

    fn add_tray(&mut self, labels: &[String]) {
        let menu_on_click = self.settings.tray_click == TrayClick::Menu;
        match Tray::new(&self.menu(labels), menu_on_click, self.proxy.clone()) {
            Ok(tray) => self.tray = Some(tray),
            Err(e) => show_error(&format!("Snapline could not add its tray icon: {e}")),
        }
    }

    fn open_capture(&mut self, event_loop: &ActiveEventLoop) {
        if self.capture.is_some() {
            return; // the shortcut pressed again during a capture does nothing
        }
        self.share = None; // a new capture closes Share's helper window
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
        self.captures += 1;
        let mut cap = Capture {
            id: self.captures,
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
        self.start_scan(&mut cap);
        for (k, w) in cap.windows.iter_mut().enumerate() {
            if let Err(e) = w.present(&cap.session, k) {
                eprintln!("Snapline: {e}");
            }
            w.fit_to_monitor();
            w.window.set_visible(true);
            w.window.set_cursor(CursorIcon::Crosshair);
        }
        cap.windows[cap.session.focused].window.focus_window();
        #[cfg(target_os = "linux")]
        platform::take_focus(&cap.windows[cap.session.focused].window);
        self.capture = Some(cap);
    }

    /// Reads every screen of `cap` on its own thread, so auto-hide can cover what it finds. The choices are read
    /// from the file each time, so the settings window applies to the next capture.
    fn start_scan(&self, cap: &mut Capture) {
        let auto_hide =
            Settings::path().map_or(self.settings.auto_hide, |p| Settings::load(&p).0.auto_hide);
        let categories = auto_hide.categories();
        if categories.is_empty() {
            return;
        }
        cap.session.begin_scan();
        for (screen, o) in cap.session.overlays.iter().enumerate() {
            let (image, scale, categories) = (o.screen.clone(), o.ui_scale, categories.clone());
            let (proxy, capture) = (self.proxy.clone(), cap.id);
            std::thread::spawn(move || {
                let _guard = OnPanic::new({
                    let proxy = proxy.clone();
                    move || {
                        let _ = proxy.send_event(UserEvent::ScanDone {
                            capture,
                            screen,
                            result: Err(recognize::CRASHED.into()),
                        });
                    }
                });
                let faces = categories.contains(&SensitiveCategory::Face);
                let text = categories.iter().any(|c| *c != SensitiveCategory::Face);
                let result = recognize::system()
                    .recognize(&image, text, faces)
                    .map(|r| (scan::hide_boxes(&r.found, &categories, scale), r.problem));
                let _ = proxy.send_event(UserEvent::ScanDone {
                    capture,
                    screen,
                    result,
                });
            });
        }
    }

    /// Copy Text: reads the exported selection (so hidden areas stay hidden) on another thread.
    fn read_text(&self, image: snapline_overlay::tiny_skia::Pixmap) {
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let _guard = OnPanic::new({
                let proxy = proxy.clone();
                move || {
                    let why = "Snapline hit an internal error while reading the text.";
                    let _ = proxy.send_event(UserEvent::TextRead(Err(why.into())));
                }
            });
            let (w, h) = (f64::from(image.width()), f64::from(image.height()));
            let result = recognize::system()
                .recognize(&image, true, false)
                .map(|r| scan::copy_text(&r.found, w, h));
            let _ = proxy.send_event(UserEvent::TextRead(result));
        });
    }

    /// Share: a temp PNG handed to the Windows Share window; Explorer with the file selected when Share is missing.
    fn share(&mut self, image: &snapline_overlay::tiny_skia::Pixmap, event_loop: &ActiveEventLoop) {
        let name = naming::file_name(Local::now());
        let file = match image
            .encode_png()
            .map_err(|e| e.to_string())
            .and_then(|png| actions::temp_png(&png, &name))
        {
            Ok(file) => file,
            Err(e) => return show_error(&e),
        };
        self.share = None; // one Share at a time
        let proxy = self.proxy.clone();
        let on_closed = move || {
            let _ = proxy.send_event(UserEvent::ShareClosed);
        };
        match share::open(event_loop, &file, &name, on_closed) {
            Ok(open) => self.share = Some(open),
            Err(why) => {
                platform::reveal(&file);
                let title = "Share is not available".into();
                let detail = actions::share_unavailable(&why);
                self.show_popup(event_loop, PopupState::Problem { title, detail }, None);
            }
        }
    }

    fn text_read(&mut self, result: Result<String, String>, event_loop: &ActiveEventLoop) {
        let (state, close_after) = match result {
            Ok(text) if !text.trim().is_empty() => {
                let text = text.trim_end();
                match actions::copy_text(text) {
                    Ok(()) => {
                        let detail = actions::text_copied_detail(text);
                        let title = "Text copied".into();
                        (PopupState::Message { title, detail }, Some(2.5))
                    }
                    Err(e) => {
                        let title = "Text not copied".into();
                        let detail = format!("Snapline could not use the clipboard: {e}");
                        (PopupState::Problem { title, detail }, None)
                    }
                }
            }
            Ok(_) => {
                let title = "No text found".into();
                let detail = "Snapline could not read any text in the selection.".into();
                (PopupState::Problem { title, detail }, Some(4.0))
            }
            Err(detail) => {
                let title = "Text not copied".into();
                (PopupState::Problem { title, detail }, None)
            }
        };
        self.show_popup(event_loop, state, close_after);
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
        if command != Command::Cancel && image.is_none() {
            return platform::beep(); // nothing selected: keep the capture open
        }
        let scale = cap.session.export_scale();
        self.capture = None; // closes the overlay windows before any dialog opens
        let Some(mut image) = image else {
            return;
        };
        // Re-read, so the settings window applies to the next action without a restart.
        let settings =
            Settings::path().map_or_else(|| self.settings.clone(), |p| Settings::load(&p).0);
        // Save at 100 %: everything but Copy Text, which OCR reads better at full size.
        if settings.downscale
            && scale > 1.0
            && command != Command::CopyText
            && let Some(small) = snapline_overlay::render::downscale(&image, 1.0 / scale)
        {
            image = small;
        }
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
            Command::CopyText => self.read_text(image),
            Command::Share => self.share(&image, event_loop),
            _ => match actions::run(command, &image, &settings) {
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
            actions::Done::PrintFromViewer => (
                "Opened for printing",
                "Press Ctrl+P in the image viewer to print.".to_owned(),
            ),
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
            let message = format!(
                "Snapline cannot reach its settings or {}.",
                platform::SECRET_STORE
            );
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
            let _guard = OnPanic::new({
                let proxy = proxy.clone();
                move || {
                    let why = "Snapline hit an internal error while uploading. Try again.";
                    let _ =
                        proxy.send_event(UserEvent::UploadFinished(Outcome::Failed(why.into())));
                }
            });
            let last = std::sync::atomic::AtomicU32::new(0);
            let report = |p: f64| {
                let percent = (p * 100.0) as u32;
                if percent != last.swap(percent, std::sync::atomic::Ordering::Relaxed) {
                    let _ = proxy.send_event(UserEvent::UploadProgress(p));
                }
            };
            let outcome = upload_job::run(
                &job,
                &path,
                &secrets,
                &UreqTransport::with_system_proxy(platform::system_proxy()),
                token,
                &report,
            );
            let _ = proxy.send_event(UserEvent::UploadFinished(outcome));
        });
    }

    fn finish_upload(&mut self, outcome: Outcome, event_loop: &ActiveEventLoop) {
        self.uploading = false;
        match outcome {
            Outcome::Done { result, token } => {
                self.google_token = token;
                let link = result.page.to_string();
                // The popup shows the link with a Copy button, so a clipboard failure is only logged.
                if let Err(e) = actions::copy_text(&link) {
                    eprintln!("Snapline: could not use the clipboard: {e}");
                }
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
        if let Some(tray) = &self.tray {
            tray.set_state(state);
        }
    }

    /// Shows `state` in the popup next to the tray icon, creating or moving its window.
    fn show_popup(
        &mut self,
        event_loop: &ActiveEventLoop,
        state: PopupState,
        close_after: Option<f64>,
    ) {
        let anchor = self.tray.as_ref().and_then(Tray::rect);
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
                if let Some(link) = link
                    && let Err(e) = actions::copy_text(&link)
                {
                    eprintln!("Snapline: could not use the clipboard: {e}");
                }
            }
            PopupButton::Open => {
                if let Some(link) = link {
                    let _ = open::that(link);
                }
                self.close_popup();
            }
            PopupButton::Settings => {
                platform::open_settings();
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
        CursorKind::Pointer => CursorIcon::Pointer,
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
        if self.share.as_ref().is_some_and(|s| s.until <= now) {
            self.share = None;
        }
        if self.icon_reset_at.is_some_and(|t| t <= now) {
            self.icon_reset_at = None;
            self.set_tray(TrayState::Normal);
        }
        let close = self.popup_close_at.filter(|_| !self.popup_hovered);
        let share = self.share.as_ref().map(|s| s.until);
        let next = [close, self.icon_reset_at, share]
            .into_iter()
            .flatten()
            .min();
        event_loop.set_control_flow(next.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::HotKey | UserEvent::CaptureNow => self.open_capture(event_loop),
            UserEvent::Menu(MenuAction::Take) => {
                // The clicked menu fades out; capturing at once would put it in the image.
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    let _ = proxy.send_event(UserEvent::CaptureNow);
                });
            }
            UserEvent::Menu(MenuAction::Quit) => event_loop.exit(),
            UserEvent::Menu(MenuAction::Settings) => platform::open_settings(),
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
            UserEvent::ScanDone {
                capture,
                screen,
                result,
            } => {
                let Some(cap) = self.capture.as_mut().filter(|c| c.id == capture) else {
                    return; // that capture is closed
                };
                let (result, problem) = match result {
                    Ok((boxes, problem)) => (Ok(boxes), problem),
                    Err(why) => (Err(recognize::badge_for(&why).to_owned()), Some(why)),
                };
                if let Some(why) = &problem {
                    eprintln!("Snapline: auto-hide: {why}");
                }
                let partial = result.is_ok() && problem.is_some();
                let effect = cap.session.scan_done(screen, result);
                if partial {
                    // Only text or only faces were checked: say so instead of "hiding N".
                    cap.session.overlays[screen].scan_note = Some(recognize::INCOMPLETE.to_owned());
                }
                let focused = cap.session.focused;
                self.apply(event_loop, focused, effect);
            }
            UserEvent::TextRead(result) => self.text_read(result, event_loop),
            UserEvent::SettingsChanged => self.reload_settings(),
            UserEvent::ShareClosed => self.share = None,
            UserEvent::Menu(MenuAction::Why) => {
                if let Some(note) = &self.shortcut_note {
                    platform::show_info(note);
                }
            }
            UserEvent::TrayClicked if self.settings.tray_click == TrayClick::Capture => {
                self.open_capture(event_loop);
            }
            UserEvent::TrayMissing if !self.told_tray_missing => {
                self.told_tray_missing = true;
                platform::show_info(&tray::no_tray_note(&self.labels));
            }
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
