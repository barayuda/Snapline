//! The few Windows calls Snapline makes directly. Other systems get harmless stand-ins until Phase 6.

/// False when another Snapline is already running. The mutex lives as long as the process (never closed on purpose).
#[cfg(windows)]
pub fn claim_single_instance() -> bool {
    use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    let name: Vec<u16> = "Local\\dev.barayuda.snapline\0".encode_utf16().collect();
    // SAFETY: `name` is a NUL-terminated UTF-16 string that outlives the call; null attributes are allowed.
    unsafe {
        CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        GetLastError() != ERROR_ALREADY_EXISTS
    }
}

#[cfg(not(windows))]
pub fn claim_single_instance() -> bool {
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
