//! The few system calls Snapline makes directly: Windows here, Linux in `linux.rs` (re-exported below). macOS,
//! where the app runs only for trying the overlay, gets harmless stand-ins.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{
    announce_settings_changed, beep, claim, cursor_position, email, is_wayland, launch_at_login,
    never_activate, reveal, send, session_problem, set_launch_at_login, shell_verb, watch,
    work_area,
};
#[cfg(target_os = "linux")]
pub use linux::{keyring_unavailable as linux_keyring_unavailable, take_focus};

/// A message to the running tray app, from the settings window or from `snapline --capture`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// `settings.toml` changed.
    Settings,
    /// Take a screenshot.
    Capture,
}

/// Calls `notify` for each signal to the tray app. Windows has only `Settings` (a named event).
#[cfg(not(target_os = "linux"))]
pub fn watch(notify: impl Fn(Signal) + Send + 'static) {
    watch_settings(move || notify(Signal::Settings));
}

/// True when a running Snapline got it. Off Linux only `Settings` can be sent.
#[cfg(not(target_os = "linux"))]
pub fn send(signal: Signal) -> bool {
    if signal == Signal::Settings {
        announce_settings_changed();
    }
    false
}

/// The command that runs Snapline with `flag` from a terminal (for when there is no tray, or for a desktop shortcut).
fn command(flag: &str) -> String {
    #[cfg(target_os = "linux")]
    {
        linux::command_for(&linux::install(), flag)
    }
    #[cfg(not(target_os = "linux"))]
    {
        format!("snapline {flag}")
    }
}

/// The command that opens the settings.
#[must_use]
pub fn settings_command() -> String {
    command("--settings")
}

/// The command that takes a screenshot (a running Snapline takes it; otherwise Snapline starts and takes it).
#[must_use]
pub fn capture_command() -> String {
    command("--capture")
}

/// Where Snapline keeps secrets, by the name users know.
pub const SECRET_STORE: &str = if cfg!(windows) {
    "Credential Manager"
} else if cfg!(target_os = "linux") {
    "the keyring"
} else {
    "the Keychain"
};

/// Where the tray icon is.
pub const TRAY_PLACE: &str = if cfg!(windows) {
    "next to the clock"
} else {
    "in the panel"
};

/// A Wayland session; other systems never are.
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn is_wayland() -> bool {
    false
}

/// Why Snapline cannot run in this session; other systems always can.
#[cfg(not(target_os = "linux"))]
pub fn session_problem() -> Option<&'static str> {
    None
}

/// False when another Snapline is already running.
pub fn claim_single_instance() -> bool {
    claim("Local\\dev.barayuda.snapline")
}

/// False when another process already holds the lock `name`. The lock lives as long as the process
/// (never released on purpose).
#[cfg(windows)]
pub fn claim(name: &str) -> bool {
    use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    let name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `name` is a NUL-terminated UTF-16 string that outlives the call; null attributes are allowed.
    unsafe {
        CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        GetLastError() != ERROR_ALREADY_EXISTS
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn claim(_name: &str) -> bool {
    true
}

#[cfg(windows)]
pub fn beep() {
    use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
    use windows_sys::Win32::UI::WindowsAndMessaging::MB_OK;
    // SAFETY: plain call with a constant.
    unsafe {
        MessageBeep(MB_OK);
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn beep() {}

/// The pointer position in physical screen pixels.
#[cfg(windows)]
pub fn cursor_position() -> Option<(i32, i32)> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: `p` is a valid, writable POINT.
    (unsafe { GetCursorPos(&raw mut p) } != 0).then_some((p.x, p.y))
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn cursor_position() -> Option<(i32, i32)> {
    None
}

/// A message box. On Linux it returns at once (the box is its own process); elsewhere it waits until closed.
pub fn show_error(message: &str) {
    #[cfg(target_os = "linux")]
    crate::message::show(message);
    #[cfg(not(target_os = "linux"))]
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Snapline")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

/// A message box that waits until it is closed, for messages right before Snapline exits.
pub fn show_error_and_wait(message: &str) {
    #[cfg(target_os = "linux")]
    crate::message::show_and_wait(message);
    #[cfg(not(target_os = "linux"))]
    show_error(message);
}

/// `send` until it is delivered, `tries` times at most, `wait` apart: a starting Snapline holds the claim a moment
/// before it listens.
pub fn send_retrying(send: impl Fn() -> bool, tries: u32, wait: std::time::Duration) -> bool {
    for attempt in 0..tries {
        if send() {
            return true;
        }
        if attempt + 1 < tries {
            std::thread::sleep(wait);
        }
    }
    false
}

/// An information box. On Linux it returns at once (the box is its own process); elsewhere it waits until closed.
pub fn show_info(message: &str) {
    #[cfg(target_os = "linux")]
    crate::message::show(message);
    #[cfg(not(target_os = "linux"))]
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Info)
        .set_title("Snapline")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

/// The usable part (without the taskbar) of the monitor under `(x, y)`, in physical pixels.
#[cfg(windows)]
pub fn work_area(x: i32, y: i32) -> Option<snapline_overlay::Rect> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
    };
    // SAFETY: plain calls; `info` is a valid MONITORINFO with its size set.
    unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = u32::try_from(std::mem::size_of::<MONITORINFO>()).ok()?;
        if GetMonitorInfoW(monitor, &raw mut info) == 0 {
            return None;
        }
        let r = info.rcWork;
        Some(snapline_overlay::Rect::new(
            f64::from(r.left),
            f64::from(r.top),
            f64::from(r.right - r.left),
            f64::from(r.bottom - r.top),
        ))
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn work_area(_x: i32, _y: i32) -> Option<snapline_overlay::Rect> {
    None // the caller falls back to the monitor's full area
}

/// Runs a shell verb ("edit", "print", "open") on `file`, as Explorer's right-click menu does.
///
/// # Errors
/// When Windows has no app for that verb, or it could not start.
#[cfg(windows)]
pub fn shell_verb(verb: &str, file: &std::path::Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(Some(0)).collect::<Vec<u16>>();
    let (verb_w, file_w) = (wide(verb.as_ref()), wide(file.as_os_str()));
    // SAFETY: both strings are NUL-terminated and outlive the call; the other pointers may be null.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb_w.as_ptr(),
            file_w.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecute reports success as a value above 32, in a pointer-sized result.
    let code = result.addr();
    if code > 32 {
        Ok(())
    } else {
        Err(shell_error(verb, code))
    }
}

/// Elsewhere only "open" works (for trying the app on a Mac).
#[cfg(not(any(windows, target_os = "linux")))]
pub fn shell_verb(verb: &str, file: &std::path::Path) -> Result<(), String> {
    match verb {
        "open" | "edit" => open::that(file).map_err(|e| e.to_string()),
        _ => Err(format!("\"{verb}\" is not available on this system yet")),
    }
}

/// Shows `file` in Explorer, selected (elsewhere: opens its folder).
#[cfg(not(target_os = "linux"))]
pub fn reveal(file: &std::path::Path) {
    #[cfg(windows)]
    let shown = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer")
            .raw_arg(select_arg(file))
            .spawn()
            .map(|_| ())
    };
    #[cfg(not(windows))]
    let shown = file
        .parent()
        .map_or(Ok(()), |dir| open::that(dir).map_err(std::io::Error::other));
    if let Err(e) = shown {
        eprintln!("Snapline: could not show {}: {e}", file.display());
    }
}

/// What to tell the user when `ShellExecuteW` fails with `code` (its result, 32 or less).
#[must_use]
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "used by shell_verb on Windows")
)]
pub fn shell_error(verb: &str, code: usize) -> String {
    match code {
        // SE_ERR_NOASSOC, SE_ERR_ASSOCINCOMPLETE
        27 | 31 => format!(
            "No app on this PC is set up to {verb} PNG images. Pick one in Settings → Apps → Default apps (for .png)."
        ),
        2 | 3 => format!("Windows could not find the screenshot file to {verb}."),
        5 => format!("Windows did not allow Snapline to {verb} the file."),
        _ => format!("Windows could not {verb} the file (error {code})."),
    }
}

/// Explorer's argument to show `file` selected. Explorer wants the path in quotes after the comma, so the
/// argument is passed as is (`raw_arg`), not quoted again as a whole.
#[must_use]
#[cfg_attr(not(windows), allow(dead_code, reason = "used by reveal on Windows"))]
pub fn select_arg(file: &std::path::Path) -> String {
    format!("/select,\"{}\"", file.display())
}

/// Windows' manual proxy for this user (Settings → Network & internet → Proxy → "Use a proxy server"), when it is
/// on. Elsewhere `None`; ureq still reads `HTTPS_PROXY`.
#[must_use]
pub fn system_proxy() -> Option<snapline_upload::proxy::SystemProxy> {
    #[cfg(windows)]
    {
        const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
        let enabled = registry::dword(KEY, "ProxyEnable").is_some_and(|v| v != 0);
        let server = registry::string(KEY, "ProxyServer").unwrap_or_default();
        let overrides = registry::string(KEY, "ProxyOverride").unwrap_or_default();
        snapline_upload::proxy::SystemProxy::from_registry(enabled, &server, &overrides)
    }
    #[cfg(not(windows))]
    None
}

/// Reading values under `HKEY_CURRENT_USER`.
#[cfg(windows)]
mod registry {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// A text value, if it exists.
    pub fn string(key: &str, name: &str) -> Option<String> {
        let (key, name) = (wide(key), wide(name));
        let mut buf = vec![0u16; 4096];
        let mut bytes = u32::try_from(buf.len() * 2).unwrap_or(u32::MAX);
        // SAFETY: the strings are NUL-terminated; `buf` holds `bytes` bytes; all live across the call.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &raw mut bytes,
            )
        };
        // `bytes` counts the closing NUL too.
        (status == ERROR_SUCCESS)
            .then(|| String::from_utf16_lossy(&buf[..(bytes as usize / 2).saturating_sub(1)]))
    }

    /// A number value, if it exists.
    pub fn dword(key: &str, name: &str) -> Option<u32> {
        let (key, name) = (wide(key), wide(name));
        let mut value = 0u32;
        let mut bytes = 4u32;
        // SAFETY: as above; `value` is the 4 bytes `bytes` says.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&raw mut value).cast(),
                &raw mut bytes,
            )
        };
        (status == ERROR_SUCCESS).then_some(value)
    }

    #[cfg(test)]
    mod tests {
        // CI sets this user's proxy first (see windows-linux.yml); never run it on your own PC with a proxy you need.
        #[test]
        #[ignore = "expects the proxy setting CI writes"]
        fn reads_the_windows_proxy_setting() {
            let p = super::super::system_proxy().expect("CI turned the proxy on");
            assert_eq!(p.https.as_deref(), Some("127.0.0.1:3128"));
            assert_eq!(
                p.bypass,
                vec!["<local>".to_owned(), "*.corp.example".to_owned()]
            );
        }
    }
}

/// The value Snapline writes under `HKCU\…\Run`: the exe's path in quotes, as Windows wants for paths with spaces.
#[must_use]
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "used by launch at sign-in on Windows")
)]
pub fn run_value(exe: &std::path::Path) -> String {
    format!("\"{}\"", exe.display())
}

/// Whether Task Manager (or Settings → Apps → Startup) lets the `Run` entry start: its `StartupApproved` value
/// starts with an even byte when on and an odd one (03) when turned off. No value means on.
#[must_use]
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "used by launch at sign-in on Windows")
)]
pub fn startup_approved(value: Option<&[u8]>) -> bool {
    value.and_then(<[u8]>::first).is_none_or(|b| b % 2 == 0)
}

/// Whether Windows starts this Snapline at sign-in: the `Run` value names this exe, and Task Manager has not
/// turned it off.
#[must_use]
#[cfg(not(target_os = "linux"))]
pub fn launch_at_login() -> bool {
    #[cfg(windows)]
    {
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        run_key::get().is_some_and(|v| v.eq_ignore_ascii_case(&run_value(&exe)))
            && startup_approved(run_key::approval().as_deref())
    }
    #[cfg(not(windows))]
    false
}

/// Turns starting at sign-in on or off.
///
/// # Errors
/// When Windows refuses the change; the error is for the user.
#[cfg(not(target_os = "linux"))]
pub fn set_launch_at_login(on: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        if on {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            run_key::set(&run_value(&exe))?;
            // Turning it on here also undoes "Disabled" from Task Manager.
            run_key::clear_approval()
        } else {
            run_key::delete()
        }
    }
    #[cfg(not(windows))]
    {
        let _ = on;
        Err("Launch at sign-in is only available on Windows and Linux for now.".into())
    }
}

/// The value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` that starts Snapline at sign-in.
#[cfg(windows)]
mod run_key {
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_BINARY, RRF_RT_REG_SZ, RegDeleteKeyValueW,
        RegGetValueW, RegSetKeyValueW,
    };

    const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "Snapline";
    /// Where Task Manager keeps its on/off switch for `Run` entries.
    const APPROVED: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    pub fn get() -> Option<String> {
        let (key, name) = (wide(KEY), wide(NAME));
        let mut buf = vec![0u16; 1024];
        let mut bytes = u32::try_from(buf.len() * 2).unwrap_or(u32::MAX);
        // SAFETY: the strings are NUL-terminated; `buf` holds `bytes` bytes; all live across the call.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &raw mut bytes,
            )
        };
        // `bytes` counts the closing NUL too.
        (status == ERROR_SUCCESS)
            .then(|| String::from_utf16_lossy(&buf[..(bytes as usize / 2).saturating_sub(1)]))
    }

    pub fn set(value: &str) -> Result<(), String> {
        let (key, name, data) = (wide(KEY), wide(NAME), wide(value));
        let bytes = u32::try_from(data.len() * 2).map_err(|e| e.to_string())?;
        // SAFETY: the strings are NUL-terminated and live across the call; `data` is `bytes` bytes long.
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                REG_SZ,
                data.as_ptr().cast(),
                bytes,
            )
        };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("Windows did not allow it (error {status})."))
        }
    }

    pub fn delete() -> Result<(), String> {
        delete_value(KEY)
    }

    /// Task Manager's switch for Snapline, when it has one.
    pub fn approval() -> Option<Vec<u8>> {
        let (key, name) = (wide(APPROVED), wide(NAME));
        let mut buf = vec![0u8; 64];
        let mut bytes = u32::try_from(buf.len()).unwrap_or(u32::MAX);
        // SAFETY: the strings are NUL-terminated; `buf` holds `bytes` bytes; all live across the call.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_BINARY,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &raw mut bytes,
            )
        };
        (status == ERROR_SUCCESS).then(|| buf[..bytes as usize].to_vec())
    }

    /// Removes Task Manager's switch, so Windows runs the `Run` entry again.
    pub fn clear_approval() -> Result<(), String> {
        delete_value(APPROVED)
    }

    fn delete_value(key: &str) -> Result<(), String> {
        let (key, name) = (wide(key), wide(NAME));
        // SAFETY: the strings are NUL-terminated and live across the call.
        let status = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
        if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(format!("Windows did not allow it (error {status})."))
        }
    }
}

/// The named event the settings window sets after saving, so the tray app reloads its settings.
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "the tray app waits on it on Windows")
)]
pub const SETTINGS_CHANGED: &str = "Local\\dev.barayuda.snapline.settings-changed";

/// Calls `notify` each time the settings window saves a change (it sets `SETTINGS_CHANGED`). Windows only.
#[cfg(not(target_os = "linux"))]
pub fn watch_settings(notify: impl Fn() + Send + 'static) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
        use windows_sys::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObject};
        let name: Vec<u16> = SETTINGS_CHANGED.encode_utf16().chain(Some(0)).collect();
        // SAFETY: `name` is NUL-terminated. Auto-reset, so each save wakes the thread once; never closed, as it
        // lives as long as the app.
        let event = unsafe { CreateEventW(std::ptr::null(), 0, 0, name.as_ptr()) };
        if event.is_null() {
            eprintln!("Snapline: settings changes will apply after a restart");
            return;
        }
        // A HANDLE is a raw pointer, which is not Send; the thread only waits on it.
        let event = event.expose_provenance();
        std::thread::spawn(move || {
            let event = std::ptr::with_exposed_provenance_mut(event);
            // SAFETY: the event stays open for the life of the app.
            while unsafe { WaitForSingleObject(event, INFINITE) } == WAIT_OBJECT_0 {
                notify();
            }
        });
    }
    #[cfg(not(windows))]
    let _ = notify;
}

/// Tells the tray app that `settings.toml` changed. Nothing happens when the tray app is not running.
#[cfg(not(target_os = "linux"))]
pub fn announce_settings_changed() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{EVENT_MODIFY_STATE, OpenEventW, SetEvent};
        let name: Vec<u16> = SETTINGS_CHANGED.encode_utf16().chain(Some(0)).collect();
        // SAFETY: `name` is NUL-terminated; the handle is closed after use.
        unsafe {
            let event = OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr());
            if !event.is_null() {
                SetEvent(event);
                CloseHandle(event);
            }
        }
    }
}

/// Opens the settings window: this program again, as a second process.
pub fn open_settings() {
    match std::env::current_exe() {
        Ok(exe) => {
            if let Err(e) = std::process::Command::new(exe).arg("--settings").spawn() {
                show_error(&format!("Snapline could not open its settings: {e}"));
            }
        }
        Err(e) => show_error(&format!("Snapline could not open its settings: {e}")),
    }
}

/// Keeps a popup from ever taking the focus or showing in Alt+Tab (`with_active(false)` covers only the first show).
#[cfg(not(target_os = "linux"))]
pub fn never_activate(window: &winit::window::Window) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW};
        add_ex_style(window, WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW);
    }
    #[cfg(not(windows))]
    let _ = window;
}

/// Keeps a helper window out of Alt+Tab and the taskbar (it may still take the focus).
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "Share's helper window is Windows only")
)]
pub fn hide_from_alt_tab(window: &winit::window::Window) {
    #[cfg(windows)]
    add_ex_style(
        window,
        windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_TOOLWINDOW,
    );
    #[cfg(not(windows))]
    let _ = window;
}

#[cfg(windows)]
fn add_ex_style(window: &winit::window::Window, flags: u32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW,
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return;
    };
    let hwnd = win32.hwnd.get() as windows_sys::Win32::Foundation::HWND;
    // SAFETY: `hwnd` is a live window of ours; only extended style bits are added.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            style | isize::try_from(flags).unwrap_or_default(),
        );
    }
}

/// Starts `command` without waiting for it. A thread waits instead, so finished programs do not linger.
///
/// # Errors
/// When the program cannot start (missing, not allowed).
pub fn start(mut command: std::process::Command) -> std::io::Result<()> {
    let mut child = command.spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Where Edit, Share and Print put their copy of the screenshot. On Linux that is the user's cache
/// (`~/.cache/Snapline/temp`): `/tmp/Snapline` would be shared by every user of the PC.
#[must_use]
pub fn temp_dir() -> std::path::PathBuf {
    #[cfg(target_os = "linux")]
    if let Some(cache) = dirs::cache_dir() {
        return cache.join("Snapline").join("temp");
    }
    std::env::temp_dir().join("Snapline")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Phase 6b review: `--capture` right while Snapline starts (claim taken, socket not bound yet) said "already
    // running" and took no screenshot; it now tries again for a moment.
    #[test]
    fn sending_tries_again_while_snapline_starts() {
        let tries = std::cell::Cell::new(0);
        let delivered = send_retrying(
            || {
                tries.set(tries.get() + 1);
                tries.get() == 3
            },
            5,
            std::time::Duration::from_millis(10),
        );
        assert!(delivered && tries.get() == 3);
        assert!(!send_retrying(
            || false,
            3,
            std::time::Duration::from_millis(1)
        ));
    }

    #[test]
    fn the_run_value_quotes_the_path() {
        assert_eq!(
            run_value(std::path::Path::new(
                r"C:\Program Files\Snapline\snapline.exe"
            )),
            r#""C:\Program Files\Snapline\snapline.exe""#
        );
    }

    // Review finding: Task Manager turns Snapline off in StartupApproved, not by removing the Run value.
    #[test]
    fn task_managers_switch_is_read() {
        assert!(startup_approved(None), "no entry: Windows runs it");
        assert!(startup_approved(Some(&[2, 0, 0, 0])));
        assert!(startup_approved(Some(&[6, 0])));
        assert!(
            !startup_approved(Some(&[3, 0, 0, 0, 9, 9])),
            "turned off in Task Manager"
        );
        assert!(!startup_approved(Some(&[7])));
    }

    // Phase 5c: "code 31" told the user nothing.
    #[test]
    fn shell_errors_say_what_to_do() {
        let none = shell_error("print", 31);
        assert!(
            none.contains("Default apps") && !none.contains("31"),
            "{none}"
        );
        assert!(shell_error("open", 2).contains("could not find"));
        assert!(shell_error("open", 99).contains("error 99"));
    }

    // Phase 5c: Explorer did not select a file whose path has spaces.
    #[test]
    fn explorer_gets_the_path_in_quotes() {
        let path = std::path::Path::new(r"C:\Users\A B\AppData\Local\Temp\Snapline\Shot 1.png");
        assert_eq!(
            select_arg(path),
            r#"/select,"C:\Users\A B\AppData\Local\Temp\Snapline\Shot 1.png""#
        );
    }
}
