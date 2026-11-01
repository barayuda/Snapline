//! The Windows Share window for a capture; on Linux a new email with the capture attached (Email on macOS).
//! Windows' Share needs a window of ours, and the overlay is closed by then, so `open` makes a small one and keeps
//! it while Share is up.

use std::path::Path;
use std::time::{Duration, Instant};
use winit::event_loop::ActiveEventLoop;
#[cfg(windows)]
use winit::window::Window;

/// How long the helper window stays at most, in case Share never reports that it closed.
const KEEP: Duration = Duration::from_mins(2);

/// Share, open for one file. Dropping it closes the helper window and stops answering Share.
pub struct Open {
    pub until: Instant,
    #[cfg(windows)]
    _sharing: windows_share::Sharing,
    #[cfg(windows)]
    _window: Window,
}

/// Opens Share for `file`. `on_closed` runs (on another thread) when the user shares or cancels.
///
/// # Errors
/// When Windows cannot show Share here (Windows Server, an old build); the error is for the user.
pub fn open(
    event_loop: &ActiveEventLoop,
    file: &Path,
    title: &str,
    on_closed: impl Fn() + Clone + Send + 'static,
) -> Result<Open, String> {
    #[cfg(windows)]
    {
        use winit::platform::windows::WindowAttributesExtWindows;
        let attributes = Window::default_attributes()
            .with_title("Snapline")
            .with_decorations(false)
            .with_skip_taskbar(true)
            .with_inner_size(winit::dpi::PhysicalSize::new(1, 1));
        let window = event_loop
            .create_window(attributes)
            .map_err(|e| e.to_string())?;
        crate::platform::hide_from_alt_tab(&window);
        window.focus_window();
        let sharing = windows_share::show(&window, file, title, on_closed)?;
        Ok(Open {
            until: Instant::now() + KEEP,
            _sharing: sharing,
            _window: window,
        })
    }
    #[cfg(target_os = "linux")]
    {
        let _ = (event_loop, on_closed, KEEP);
        crate::platform::email(file, title)?;
        Ok(Open {
            until: Instant::now(), // nothing to keep open
        })
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = (event_loop, file, title, on_closed, KEEP);
        Err("Share is not available on this system yet".into())
    }
}

#[cfg(windows)]
mod windows_share {
    use std::path::Path;
    use windows::ApplicationModel::DataTransfer::{
        DataPackage, DataRequestedEventArgs, DataTransferManager, ShareCompletedEventArgs,
    };
    use windows::Foundation::TypedEventHandler;
    use windows::Storage::{IStorageItem, StorageFile};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::IDataTransferManagerInterop;
    use windows::core::{AgileReference, HSTRING, IInspectable, Interface, Ref};
    use windows_collections::IIterable;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::Window;

    /// Answers Share's requests for one window until dropped.
    pub struct Sharing {
        manager: DataTransferManager,
        token: i64,
    }

    impl Drop for Sharing {
        fn drop(&mut self) {
            let _ = self.manager.RemoveDataRequested(self.token);
        }
    }

    fn hwnd(window: &Window) -> Result<HWND, String> {
        let handle = window.window_handle().map_err(|e| e.to_string())?;
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            return Err("not a Windows window".into());
        };
        Ok(HWND(win32.hwnd.get() as *mut core::ffi::c_void))
    }

    pub fn show(
        window: &Window,
        file: &Path,
        title: &str,
        on_closed: impl Fn() + Clone + Send + 'static,
    ) -> Result<Sharing, String> {
        let fail = |e: windows::core::Error| e.message();
        let hwnd = hwnd(window)?;
        // The file is looked up now, not in the handler, so Share's request never waits on disk.
        let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(file))
            .and_then(|op| op.join())
            .map_err(fail)?;
        let file = AgileReference::new(&file).map_err(fail)?;
        let title = HSTRING::from(title);
        let handler = TypedEventHandler::new(move |_, args: Ref<DataRequestedEventArgs>| {
            let data = args.ok()?.Request()?.Data()?;
            data.Properties()?.SetTitle(&title)?;
            let item: IStorageItem = file.resolve()?.cast()?;
            data.SetStorageItemsReadOnly(&IIterable::from(vec![Some(item)]))?;
            // Shared or cancelled: the helper window can go, so the focus returns where it was.
            let done = on_closed.clone();
            data.ShareCompleted(&TypedEventHandler::new(
                move |_: Ref<DataPackage>, _: Ref<ShareCompletedEventArgs>| {
                    done();
                    Ok(())
                },
            ))?;
            let gone = on_closed.clone();
            data.ShareCanceled(&TypedEventHandler::new(
                move |_: Ref<DataPackage>, _: Ref<IInspectable>| {
                    gone();
                    Ok(())
                },
            ))?;
            Ok(())
        });
        let interop = windows::core::factory::<DataTransferManager, IDataTransferManagerInterop>()
            .map_err(fail)?;
        // SAFETY: `hwnd` is our own live top-level window.
        let manager: DataTransferManager = unsafe { interop.GetForWindow(hwnd) }.map_err(fail)?;
        let token = manager.DataRequested(&handler).map_err(fail)?;
        let sharing = Sharing { manager, token };
        // SAFETY: as above.
        unsafe { interop.ShowShareUIForWindow(hwnd) }.map_err(fail)?;
        Ok(sharing)
    }
}
