//! The few Windows calls Snapline makes directly. Other systems get harmless stand-ins until Phase 6.

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

#[cfg(not(windows))]
pub fn claim(_name: &str) -> bool {
    true // ponytail: no check off Windows yet; Linux gets one in Phase 6
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

#[cfg(not(windows))]
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

#[cfg(not(windows))]
pub fn cursor_position() -> Option<(i32, i32)> {
    None
}

/// A message box (blocks until closed).
pub fn show_error(message: &str) {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Snapline")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

/// An information box (blocks until closed).
pub fn show_info(message: &str) {
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

#[cfg(not(windows))]
pub fn work_area(_x: i32, _y: i32) -> Option<snapline_overlay::Rect> {
    None // the caller falls back to the monitor's full area
}

/// Opens the upload settings window: this program again, as a second process.
pub fn open_upload_settings() {
    match std::env::current_exe() {
        Ok(exe) => {
            if let Err(e) = std::process::Command::new(exe)
                .arg("--upload-settings")
                .spawn()
            {
                show_error(&format!("Snapline could not open its upload settings: {e}"));
            }
        }
        Err(e) => show_error(&format!("Snapline could not open its upload settings: {e}")),
    }
}

/// Keeps a popup from ever taking the focus or showing in Alt+Tab (`with_active(false)` covers only the first show).
#[cfg(windows)]
pub fn never_activate(window: &winit::window::Window) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return;
    };
    let hwnd = win32.hwnd.get() as windows_sys::Win32::Foundation::HWND;
    // SAFETY: `hwnd` is the live window winit just created; only extended style bits are added.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            style | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as isize,
        );
    }
}

#[cfg(not(windows))]
pub fn never_activate(_window: &winit::window::Window) {}
