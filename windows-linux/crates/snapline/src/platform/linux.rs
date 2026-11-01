//! Linux (X11) versions of the platform calls. `platform/mod.rs` re-exports them.

use std::ffi::OsStr;
use std::fs::File;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, PoisonError};

use super::Signal;

use snapline_overlay::Rect;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, InputFocus};
use x11rb::rust_connection::RustConnection;

/// Said in a Wayland session that has no `XWayland`: Snapline draws its windows through `XWayland`.
pub const NO_XWAYLAND: &str = "Snapline needs XWayland in a Wayland session. Install the xwayland package (GNOME \
     and KDE Plasma start it by themselves), or log in to an X11 session, then start Snapline again.";
/// Said when there is no desktop at all.
pub const NO_DISPLAY: &str =
    "Snapline needs a desktop session. Start it from your desktop, not from a text console.";

/// Why Snapline cannot run in this session, or `None` when it can.
pub fn session_problem() -> Option<&'static str> {
    session_problem_for(
        std::env::var_os("XDG_SESSION_TYPE").as_deref(),
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
        std::env::var_os("DISPLAY").is_some(),
    )
}

/// `session_type` is `$XDG_SESSION_TYPE`; `wayland` and `x11` say whether `$WAYLAND_DISPLAY` and `$DISPLAY` are set.
/// A Wayland session works when it has `XWayland` (`$DISPLAY`): the capture goes through the portal, the windows
/// through `XWayland`.
pub fn session_problem_for(
    session_type: Option<&OsStr>,
    wayland: bool,
    x11: bool,
) -> Option<&'static str> {
    let kind = session_type.and_then(OsStr::to_str).unwrap_or("");
    if x11 {
        None // X11, or Wayland with XWayland: Snapline's windows go through X11 either way
    } else if kind == "wayland" || wayland {
        Some(NO_XWAYLAND)
    } else {
        Some(NO_DISPLAY)
    }
}

/// The user's runtime folder (`$XDG_RUNTIME_DIR`, private to the user), or the temp folder.
fn runtime_dir() -> PathBuf {
    runtime_dir_for(
        dirs::runtime_dir().unwrap_or_else(std::env::temp_dir),
        std::env::var_os("FLATPAK_ID").as_deref(),
    )
}

/// Inside Flatpak `base` is private to one `flatpak run`; `app/<id>` under it is shared by every instance of the app.
fn runtime_dir_for(base: PathBuf, flatpak_id: Option<&OsStr>) -> PathBuf {
    match flatpak_id {
        Some(id) => base.join("app").join(id),
        None => base,
    }
}

/// Lock files held for the life of the process.
static HELD: Mutex<Vec<File>> = Mutex::new(Vec::new());

/// `name` is the Windows mutex name (`Local\dev.barayuda.snapline`); its last part names the lock file.
pub fn claim(name: &str) -> bool {
    let file = name.rsplit('\\').next().unwrap_or(name);
    claim_at(&runtime_dir().join(format!("{file}.lock")))
}

/// False when another process holds the lock on `path`. When no lock can be made at all, Snapline runs.
pub fn claim_at(path: &Path) -> bool {
    let Ok(file) = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
    else {
        return true;
    };
    match file.try_lock() {
        Ok(()) => {
            HELD.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(file);
            true
        }
        Err(std::fs::TryLockError::WouldBlock) => false,
        Err(std::fs::TryLockError::Error(_)) => true,
    }
}

/// The bytes a signal travels as over the socket.
impl Signal {
    #[must_use]
    pub fn as_bytes(self) -> &'static [u8] {
        match self {
            Self::Settings => b"settings",
            Self::Capture => b"capture",
        }
    }

    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        [Self::Settings, Self::Capture]
            .into_iter()
            .find(|s| s.as_bytes() == bytes)
    }
}

const SIGNAL_SOCKET: &str = "dev.barayuda.snapline.settings.sock";

pub fn watch(notify: impl Fn(Signal) + Send + 'static) {
    watch_at(&runtime_dir().join(SIGNAL_SOCKET), notify);
}

/// True when a running Snapline got it.
pub fn send(signal: Signal) -> bool {
    send_at(&runtime_dir().join(SIGNAL_SOCKET), signal)
}

pub fn announce_settings_changed() {
    send(Signal::Settings);
}

/// Calls `notify` for every signal sent to `path` (anything else is ignored). Only the tray app listens, and it holds
/// the claim, so a socket file left behind by a crash is safe to replace.
pub fn watch_at(path: &Path, notify: impl Fn(Signal) + Send + 'static) {
    let _ = std::fs::remove_file(path);
    let Ok(socket) = UnixDatagram::bind(path) else {
        eprintln!("Snapline: settings changes will apply after a restart");
        return;
    };
    std::thread::spawn(move || {
        let mut buf = [0u8; 16];
        while let Ok(n) = socket.recv(&mut buf) {
            if let Some(signal) = Signal::from_bytes(&buf[..n]) {
                notify(signal);
            }
        }
    });
}

/// False when nobody is listening: Snapline is not running, or the socket file is left from a crash.
pub fn send_at(path: &Path, signal: Signal) -> bool {
    UnixDatagram::unbound().is_ok_and(|socket| socket.send_to(signal.as_bytes(), path).is_ok())
}

/// A connection to the X server and its root window.
fn x11() -> Option<(RustConnection, u32)> {
    let (conn, screen) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen)?.root;
    Some((conn, root))
}

fn atom(conn: &RustConnection, name: &str) -> Option<u32> {
    Some(
        conn.intern_atom(false, name.as_bytes())
            .ok()?
            .reply()
            .ok()?
            .atom,
    )
}

pub fn beep() {
    if let Some((conn, _)) = x11() {
        let _ = conn.bell(0);
        let _ = conn.flush();
    }
}

pub fn cursor_position() -> Option<(i32, i32)> {
    let (conn, root) = x11()?;
    let pointer = conn.query_pointer(root).ok()?.reply().ok()?;
    Some((pointer.root_x.into(), pointer.root_y.into()))
}

/// The part of `monitor` inside `work`, or `None` when they do not overlap.
#[must_use]
pub fn clip(monitor: Rect, work: Rect) -> Option<Rect> {
    let x = monitor.x.max(work.x);
    let y = monitor.y.max(work.y);
    let right = (monitor.x + monitor.width).min(work.x + work.width);
    let bottom = (monitor.y + monitor.height).min(work.y + work.height);
    (right > x && bottom > y).then(|| Rect::new(x, y, right - x, bottom - y))
}

/// `_NET_WORKAREA` of the current desktop: the screen without panels, as the window manager reports it.
fn net_workarea(conn: &RustConnection, root: u32) -> Option<Rect> {
    let current = conn
        .get_property(
            false,
            root,
            atom(conn, "_NET_CURRENT_DESKTOP")?,
            AtomEnum::CARDINAL,
            0,
            1,
        )
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .next()
        .unwrap_or(0);
    let areas: Vec<u32> = conn
        .get_property(
            false,
            root,
            atom(conn, "_NET_WORKAREA")?,
            AtomEnum::CARDINAL,
            0,
            1024,
        )
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .collect();
    let boxes = areas.as_chunks::<4>().0;
    let a = boxes.get(current as usize).or_else(|| boxes.first())?;
    Some(Rect::new(
        f64::from(a[0]),
        f64::from(a[1]),
        f64::from(a[2]),
        f64::from(a[3]),
    ))
}

/// The usable part of the monitor under `(x, y)`, in physical pixels; `None` without a window manager.
pub fn work_area(x: i32, y: i32) -> Option<Rect> {
    let monitor = xcap::Monitor::all().ok()?.into_iter().find_map(|m| {
        let (mx, my) = (m.x().ok()?, m.y().ok()?);
        let (mw, mh) = (m.width().ok()?, m.height().ok()?);
        let inside = x >= mx
            && y >= my
            && i64::from(x) < i64::from(mx) + i64::from(mw)
            && i64::from(y) < i64::from(my) + i64::from(mh);
        inside.then(|| Rect::new(f64::from(mx), f64::from(my), f64::from(mw), f64::from(mh)))
    })?;
    let (conn, root) = x11()?;
    // ponytail: _NET_WORKAREA is one box for all monitors, so a panel on another monitor only can be missed.
    clip(monitor, net_workarea(&conn, root)?)
}

/// The X11 window id of `window`.
fn xid(window: &winit::window::Window) -> Option<u32> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Xlib(h) => u32::try_from(h.window).ok(),
        RawWindowHandle::Xcb(h) => Some(h.window.get()),
        _ => None,
    }
}

/// Gives `window` the keyboard (Escape and the shortcuts must reach the overlay at once).
pub fn take_focus(window: &winit::window::Window) {
    if let (Some((conn, _)), Some(id)) = (x11(), xid(window)) {
        let _ = conn.set_input_focus(InputFocus::PARENT, id, x11rb::CURRENT_TIME);
        let _ = conn.flush();
    }
}

/// Popups are override-redirect on X11 (`popup_window.rs`): no window manager focuses them or lists them.
pub fn never_activate(_window: &winit::window::Window) {}

fn missing_tool(program: &str, e: &std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::NotFound {
        format!("{program} is missing. Install the xdg-utils package.")
    } else {
        format!("{program} did not start: {e}")
    }
}

fn xdg_open(path: &Path) -> Result<(), String> {
    let mut cmd = Command::new("xdg-open");
    cmd.arg(path);
    super::start(cmd).map_err(|e| missing_tool("xdg-open", &e))
}

/// Linux has no "edit" or "print" verbs: every verb opens `file` in the default app for PNG images.
pub fn shell_verb(_verb: &str, file: &Path) -> Result<(), String> {
    xdg_open(file)
}

/// `file` as a `file://` URI.
pub fn file_uri(file: &Path) -> Result<String, String> {
    url::Url::from_file_path(file)
        .map(String::from)
        .map_err(|()| format!("{} is not a full path", file.display()))
}

/// Shows `file` selected in the file manager (`org.freedesktop.FileManager1`: Nautilus, Dolphin, Nemo, Caja,
/// Thunar); opens its folder when the file manager does not offer that.
pub fn reveal(file: &Path) {
    let selected = file_uri(file).and_then(|uri| {
        let bus = zbus::blocking::Connection::session().map_err(|e| e.to_string())?;
        bus.call_method(
            Some("org.freedesktop.FileManager1"),
            "/org/freedesktop/FileManager1",
            Some("org.freedesktop.FileManager1"),
            "ShowItems",
            &(vec![uri.as_str()], ""),
        )
        .map(drop)
        .map_err(|e| e.to_string())
    });
    if selected.is_err()
        && let Some(dir) = file.parent()
        && let Err(e) = xdg_open(dir)
    {
        eprintln!("Snapline: could not show {}: {e}", file.display());
    }
}

/// A new email with `file` attached. Inside Flatpak, `xdg-email` goes through the Email portal.
pub fn email(file: &Path, subject: &str) -> Result<(), String> {
    let mut cmd = Command::new("xdg-email");
    cmd.args(["--subject", subject, "--attach"]).arg(file);
    super::start(cmd).map_err(|e| missing_tool("xdg-email", &e))
}

/// How Snapline was installed, which decides the command that starts it at login.
#[derive(Debug, PartialEq, Eq)]
pub enum Install {
    Flatpak(String),
    AppImage(PathBuf),
    /// The .deb, or a build run in place.
    Plain(PathBuf),
}

#[must_use]
pub fn install() -> Install {
    if let Some(id) = std::env::var_os("FLATPAK_ID") {
        return Install::Flatpak(id.to_string_lossy().into_owned());
    }
    if let Some(path) = std::env::var_os("APPIMAGE") {
        return Install::AppImage(path.into());
    }
    Install::Plain(std::env::current_exe().unwrap_or_else(|_| "snapline".into()))
}

/// The command that runs Snapline with `flag` (`--settings`, `--capture`), as the user would type it in a terminal.
#[must_use]
pub fn command_for(install: &Install, flag: &str) -> String {
    let program = match install {
        Install::Flatpak(id) => format!("flatpak run {id}"),
        Install::Plain(path) if path == Path::new("/usr/bin/snapline") => "snapline".to_owned(),
        Install::AppImage(path) | Install::Plain(path) => {
            let path = path.to_string_lossy();
            if path.contains(|c: char| c.is_whitespace() || "'\"$`\\&;|()<>*?!#~".contains(c)) {
                format!("'{}'", path.replace('\'', r"'\''"))
            } else {
                path.into_owned()
            }
        }
    };
    format!("{program} {flag}")
}

/// The `Exec=` value. Inside quotes the desktop entry spec wants `"`, `` ` ``, `$` and `\` escaped, and `%` doubled.
#[must_use]
pub fn exec_line(install: &Install) -> String {
    match install {
        Install::Flatpak(id) => format!("flatpak run {id}"),
        Install::AppImage(path) | Install::Plain(path) => {
            let mut out = String::from("\"");
            for c in path.to_string_lossy().chars() {
                match c {
                    '"' | '`' | '$' => {
                        out.push_str("\\\\");
                        out.push(c);
                    }
                    '\\' => out.push_str("\\\\\\\\"),
                    '%' => out.push_str("%%"),
                    _ => out.push(c),
                }
            }
            out.push('"');
            out
        }
    }
}

/// `~/.config/autostart`; `None` inside Flatpak, which has no access there (a sandboxed app that can write autostart
/// entries can run any command at the next login).
#[must_use]
pub fn autostart_dir(install: &Install) -> Option<PathBuf> {
    match install {
        Install::Flatpak(_) => None,
        _ => dirs::config_dir().map(|c| c.join("autostart")),
    }
}

const AUTOSTART_FILE: &str = "dev.barayuda.Snapline.desktop";

/// Writes (`Some(exec)`) or removes (`None`) Snapline's autostart entry in `dir`.
pub fn set_autostart(dir: &Path, exec: Option<&str>) -> Result<(), String> {
    let file = dir.join(AUTOSTART_FILE);
    let done = match exec {
        Some(exec) => std::fs::create_dir_all(dir).and_then(|()| {
            std::fs::write(
                &file,
                format!(
                    "[Desktop Entry]\nType=Application\nName=Snapline\nExec={exec}\nIcon=dev.barayuda.Snapline\n\
                     Terminal=false\nX-GNOME-Autostart-enabled=true\n"
                ),
            )
        }),
        None => match std::fs::remove_file(&file) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        },
    };
    done.map_err(|e| format!("Snapline could not change {}: {e}", file.display()))
}

pub fn launch_at_login() -> bool {
    autostart_dir(&install()).is_some_and(|d| d.join(AUTOSTART_FILE).is_file())
}

pub fn set_launch_at_login(on: bool) -> Result<(), String> {
    set_launch_at_login_for(&install(), on)
}

/// Turning it off in the Flatpak is a no-op: it never wrote an entry.
pub fn set_launch_at_login_for(install: &Install, on: bool) -> Result<(), String> {
    match (install, autostart_dir(install)) {
        (Install::Flatpak(_), _) if !on => Ok(()),
        (Install::Flatpak(_), _) => Err(
            "The Flatpak cannot start itself at sign-in. Add Snapline in your desktop's startup settings \
             (for example Settings → Session and Startup on Xfce)."
                .into(),
        ),
        (_, None) => Err("Snapline could not find your home folder.".into()),
        (_, Some(dir)) => set_autostart(&dir, on.then(|| exec_line(install)).as_deref()),
    }
}

/// Why uploads are off when the keyring cannot be reached, and what to do.
#[must_use]
pub fn keyring_unavailable(why: &str) -> String {
    format!(
        "{why}. Snapline keeps bucket keys and your Google sign-in in the desktop keyring (Secret Service). Install \
         and start GNOME Keyring or KWallet, then restart Snapline."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("snapline-{name}-{}", std::process::id()))
    }

    #[test]
    fn the_work_area_is_the_monitor_without_the_panels() {
        let monitor = Rect::new(1920.0, 0.0, 2560.0, 1440.0);
        // _NET_WORKAREA spans every monitor: here a 48-pixel panel at the bottom of a 4480 × 1440 desktop.
        let work = Rect::new(0.0, 0.0, 4480.0, 1392.0);
        assert_eq!(
            clip(monitor, work),
            Some(Rect::new(1920.0, 0.0, 2560.0, 1392.0))
        );
        assert_eq!(clip(monitor, Rect::new(0.0, 0.0, 1920.0, 1080.0)), None);
    }

    #[test]
    #[ignore = "needs an X server: CI runs it under xvfb-run"]
    fn x11_answers_for_the_pointer_and_the_bell() {
        assert!(cursor_position().is_some());
        beep();
        // Xvfb has no window manager, so no _NET_WORKAREA: the popup falls back to the whole monitor.
        assert_eq!(work_area(0, 0), None);
    }

    // Review Focus 4.
    #[test]
    fn exec_line_quotes_what_desktop_files_need() {
        // Final review: the value is unescaped once as a string before Exec quoting, so `"`, `$` and `` ` `` need
        // `\\` in the file and a backslash needs four (what GLib's key file reader accepts).
        let plain = Install::Plain("/home/a b/Apps/100% \"snap\"$`x\\y/snapline".into());
        assert_eq!(
            exec_line(&plain),
            r#""/home/a b/Apps/100%% \\"snap\\"\\$\\`x\\\\y/snapline""#
        );
        assert_eq!(
            exec_line(&Install::Flatpak("dev.barayuda.Snapline".into())),
            "flatpak run dev.barayuda.Snapline"
        );
        let appimage = Install::AppImage("/home/me/Snapline-0.1.0-x86_64.AppImage".into());
        assert_eq!(
            exec_line(&appimage),
            "\"/home/me/Snapline-0.1.0-x86_64.AppImage\""
        );
    }

    // Review Focus 4.
    #[test]
    fn file_uris_escape_spaces() {
        assert_eq!(
            file_uri(Path::new("/home/a b/Snapline 2026-11-01 at 10.00.00.png")).unwrap(),
            "file:///home/a%20b/Snapline%202026-11-01%20at%2010.00.00.png"
        );
        assert!(file_uri(Path::new("relative.png")).is_err());
    }

    #[test]
    fn launch_at_login_writes_and_removes_one_entry() {
        let dir = scratch("autostart");
        set_autostart(&dir, Some("\"/usr/bin/snapline\"")).unwrap();
        let entry = std::fs::read_to_string(dir.join("dev.barayuda.Snapline.desktop")).unwrap();
        assert!(
            entry.starts_with("[Desktop Entry]\n")
                && entry.contains("\nExec=\"/usr/bin/snapline\"\n"),
            "{entry}"
        );
        set_autostart(&dir, None).unwrap();
        set_autostart(&dir, None).unwrap(); // already off: still fine
        assert!(!dir.join("dev.barayuda.Snapline.desktop").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    // Final review: write access to ~/.config/autostart lets a sandboxed app run any command at the next login, so
    // the Flatpak has none and points the user at the desktop's own startup settings.
    #[test]
    fn the_flatpak_leaves_autostart_to_the_desktop() {
        let flatpak = Install::Flatpak("dev.barayuda.Snapline".into());
        assert_eq!(autostart_dir(&flatpak), None);
        let error = set_launch_at_login_for(&flatpak, true).unwrap_err();
        assert!(error.contains("startup"), "{error}");
        assert!(set_launch_at_login_for(&flatpak, false).is_ok());
    }

    // Review Focus 2: i3 or a bare Xfce has no Secret Service.
    #[test]
    fn a_missing_keyring_says_what_to_install() {
        let text =
            keyring_unavailable("org.freedesktop.secrets was not provided by any .service files");
        assert!(
            text.contains("GNOME Keyring")
                && text.contains("KWallet")
                && text.contains("not provided"),
            "{text}"
        );
    }

    // Final review: inside Flatpak /run/user/<uid> is private to each `flatpak run`, so a second launch did not see
    // the first one's lock; $XDG_RUNTIME_DIR/app/<id> is shared by every instance of the app.
    #[test]
    fn flatpak_instances_share_one_runtime_folder() {
        let base = PathBuf::from("/run/user/1000");
        assert_eq!(
            runtime_dir_for(base.clone(), Some(OsStr::new("dev.barayuda.Snapline"))),
            Path::new("/run/user/1000/app/dev.barayuda.Snapline")
        );
        assert_eq!(runtime_dir_for(base.clone(), None), base);
    }

    // Final review: AppImage and Flatpak users have no `snapline` command.
    #[test]
    fn the_settings_command_matches_the_install() {
        assert_eq!(
            command_for(
                &Install::Flatpak("dev.barayuda.Snapline".into()),
                "--settings"
            ),
            "flatpak run dev.barayuda.Snapline --settings"
        );
        assert_eq!(
            command_for(
                &Install::AppImage("/home/me/Apps/Snapline 0.1.AppImage".into()),
                "--settings"
            ),
            "'/home/me/Apps/Snapline 0.1.AppImage' --settings"
        );
        assert_eq!(
            command_for(&Install::Plain("/usr/bin/snapline".into()), "--settings"),
            "snapline --settings"
        );
    }

    #[test]
    fn only_the_first_claim_wins() {
        let path = scratch("claim.lock");
        assert!(claim_at(&path));
        assert!(!claim_at(&path), "a second Snapline must stop");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn signals_travel_over_the_socket() {
        let path = scratch("signals.sock");
        let (tx, rx) = std::sync::mpsc::channel();
        watch_at(&path, move |s| {
            let _ = tx.send(s);
        });
        assert!(send_at(&path, Signal::Capture));
        assert!(send_at(&path, Signal::Settings));
        let got: Vec<Signal> = (0..2)
            .map(|_| rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap())
            .collect();
        assert_eq!(got, [Signal::Capture, Signal::Settings]);
        let _ = std::fs::remove_file(path);
    }

    // Review Focus 2: nobody listening (not running, or a socket file left by a crash): `--capture` starts Snapline.
    #[test]
    fn capture_starts_snapline_when_none_is_running() {
        assert!(
            !send_at(&scratch("missing.sock"), Signal::Capture),
            "no socket file"
        );
        let stale = scratch("stale.sock");
        drop(UnixDatagram::bind(&stale)); // the file stays, nobody reads it
        assert!(
            !send_at(&stale, Signal::Capture),
            "a socket file left by a crash"
        );
        let _ = std::fs::remove_file(stale);
    }

    #[test]
    fn the_capture_command_matches_the_install() {
        assert_eq!(
            command_for(
                &Install::Flatpak("dev.barayuda.Snapline".into()),
                "--capture"
            ),
            "flatpak run dev.barayuda.Snapline --capture"
        );
        assert_eq!(
            command_for(&Install::Plain("/usr/bin/snapline".into()), "--capture"),
            "snapline --capture"
        );
    }

    // Phase 6b: Wayland runs through XWayland; only a Wayland session without XWayland is refused.
    #[test]
    fn wayland_needs_xwayland() {
        let wayland = Some(OsStr::new("wayland"));
        assert_eq!(session_problem_for(wayland, true, true), None);
        assert_eq!(session_problem_for(wayland, true, false), Some(NO_XWAYLAND));
        assert_eq!(session_problem_for(None, true, false), Some(NO_XWAYLAND));
        assert_eq!(
            session_problem_for(Some(OsStr::new("x11")), false, true),
            None
        );
        assert_eq!(session_problem_for(None, false, false), Some(NO_DISPLAY));
        assert_eq!(
            session_problem_for(Some(OsStr::new("tty")), false, false),
            Some(NO_DISPLAY)
        );
    }
}
