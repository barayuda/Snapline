# Snapline for Windows and Linux: design

Status: draft, 28 October 2026. Branch `feat/windows-linux-rust`. Linux X11 preview: Phase 6a, 1 November 2026.

## Goal

Bring Snapline to Windows, then Linux, with the same features as the macOS app: capture, select, draw, copy, save,
upload (S3, Google Drive), auto-hide sensitive data, Copy Text, search similar images, email, print, edit in another
app, and Preferences. The macOS app stays in Swift and keeps working as it does today.

## Decisions

| Decision | Choice | Why |
|---|---|---|
| Language | Rust (edition 2024, stable toolchain) | One small binary, no runtime to install, good Windows API bindings, mature crates for capture, hotkeys, tray and secrets |
| Order | Windows first, then Linux X11, then Linux Wayland | Each step builds on the one before; Wayland has the most platform limits |
| Repo layout | `macOS/` (Swift) and `windows-linux/` (Rust) in this repo | One issue tracker; Windows and Linux share almost all Rust code, so they share one workspace |
| Testing | GitHub Actions builds and runs unit tests on Windows and Linux; Barayuda tries each build on a Windows PC | This Mac cannot run or click through a Windows app |
| Keeping both apps in step | Shared test cases in `fixtures/`, read by both the Swift and the Rust tests | A detection rule fixed on one platform cannot drift on the other |
| Overlay drawing | tiny-skia only, no egui in the overlay | One renderer for the screen and the saved image, with pixel tests on CI. Barayuda, 29 Oct 2026 |
| Default shortcut | PrtScn and Ctrl+Shift+1 | The Lightshot habit plus the macOS keys. Barayuda, 29 Oct 2026 |
| Drawing colours | A built-in palette of 8 colours | Works the same on Windows and Linux, like Lightshot. Barayuda, 29 Oct 2026 |
| Upload settings before Preferences | An upload settings window in Phase 3 (the Upload tab first), run as a second process | People need somewhere to put keys before Phase 5. Barayuda, 30 Oct 2026 |
| Search similar and confirmations | In Phase 3, with uploads | Search is an upload plus a link; confirmations share the upload popup. Barayuda, 30 Oct 2026 |
| Symbol repair deferred | Copy Text copies the OCR text in reading order; `SymbolRepair` and the "check the copied text" popup wait | Windows apps rarely show ⌘ ⇧ ⌥ symbols. Barayuda, 31 Oct 2026 |
| Auto-hide settings | An Auto-hide tab in the settings window, which becomes "Snapline settings" (Upload and Auto-hide tabs) | Choosing what to hide should not wait for Phase 5. Barayuda, 31 Oct 2026 |
| Phase 5 split | 5a: edit, share, print, the General tab and the Phase 4 minors users notice. 5b: installer, code signing, website and privacy policy | Each half can be checked on Windows before shipping. Barayuda, 31 Oct 2026 |
| Email on Windows | The Windows Share window, as a "Share" button | Share reaches Outlook (new and classic), Mail and Teams; MAPI does not work with the new Outlook. Barayuda, 31 Oct 2026 |
| Windows installer | Inno Setup, per user (no admin), plus a portable zip | Simple to build on CI, no admin prompt, uninstaller and sign-in entry included. Barayuda, 31 Oct 2026 |
| Code signing | Wired into the release workflow, switched on when an Azure Artifact Signing account exists; unsigned until then | Signing costs money and identity checks; releases need not wait for it. Barayuda, 31 Oct 2026 |
| Where builds go | GitHub Releases: a `windows-v<version>` tag builds a draft that Barayuda publishes | One public download page; nothing is published by CI alone. Barayuda, 31 Oct 2026 |
| Phase 5c | Proxy from Windows' manual setting and `HTTPS_PROXY`, certificates from the Windows store (no PAC/WPAD); banner says macOS and Windows; README for both; the recorder, Print/Edit/Share, tray and scan, and release minors | Fix what would hurt the first release. Barayuda, 31 Oct 2026 |
| Linux scope | 6a: X11 only; on Wayland Snapline says it needs an X11 session. 6b: Wayland through portals | XWayland sees only X11 apps, so captures there would be black. Barayuda, 1 Nov 2026 |
| Text recognition on Linux | Tesseract 5, linked into Snapline (`tesseract-rs` with the system library); no face detection yet | No separate program to install; the build downloads nothing. Barayuda, 1 Nov 2026 |
| Linux packages | .deb, AppImage and Flatpak, x86_64 and aarch64, from a `linux-v<version>` tag | Covers Debian/Ubuntu, any distro, and sandboxed installs. Barayuda, 1 Nov 2026 |
| Linux testing | An OrbStack Ubuntu 24.04 VM on the Mac (`windows-linux/linux/dev-vm.sh`); CI on GitHub's Ubuntu runners is the gate | Barayuda, 1 Nov 2026 |
| Tray on Linux | `ksni` (StatusNotifierItem over D-Bus), not `tray-icon` | No GTK, and left clicks are reported. Phase 6a plan |
| Message boxes on Linux | An egui window (`snapline --message`) | rfd's Linux message box needs zenity, which the Flatpak sandbox lacks. Phase 6a plan |
| Share and Print on Linux | Share opens an email with the screenshot attached (`xdg-email`); Print opens the image viewer | Linux has no share sheet and no print dialog without GTK. Phase 6a plan |
| Linux overlay and popups | The overlay is a fullscreen window per monitor; popups are override-redirect | Fullscreen gets the keyboard from the window manager; override-redirect keeps popups out of the taskbar on every window manager. Phase 6a |
| App ID | `dev.barayuda.Snapline` | Matches the macOS bundle ID. Phase 6a plan |

## Repo layout

```
macOS/                     the macOS app (Swift package, unchanged code)
windows-linux/             Cargo workspace
  crates/snapline-core/    pure logic, no UI, no OS calls: the Rust twin of SnaplineCore
  crates/snapline-overlay/ the overlay without windows: selection, drawing, text, toolbars, popup, rendering (tiny-skia)
  crates/snapline-upload/  uploads without UI or storage: S3, Google Drive, Google sign-in (tested on every OS)
  crates/snapline/         the app (from Phase 2): tray, hotkey, capture, overlay windows, actions
    src/platform/          system calls: Windows in mod.rs, Linux in linux.rs
  linux/                   Linux packaging (.deb, AppImage, Flatpak), build packages, the OrbStack dev machine
fixtures/                  test cases shared by both apps (JSON)
Assets/, scripts/brand/    brand art and generator, shared
site/                      website
```

## What replaces each macOS piece

| macOS | Windows | Linux |
|---|---|---|
| ScreenCaptureKit | `xcap` (Windows.Graphics.Capture / DXGI); per-monitor DPI aware | X11: `xcap`. Wayland: screenshot portal (`ashpd`) |
| Overlay (AppKit window per screen) | `winit` borderless topmost window per monitor showing the frozen capture through `softbuffer` | same; Wayland cannot place windows, so one fullscreen window per output |
| Drawing (Core Graphics) | `tiny-skia` for both the screen and the saved image (one renderer) | same |
| Vision text recognition | Windows.Media.Ocr (`windows` crate), built into Windows 10/11 | Tesseract 5, linked (`tesseract-rs`); English data from the package |
| Vision face detection | Windows.Media.FaceAnalysis.FaceDetector | none in Phase 6a; text only |
| Carbon hotkey | `global-hotkey` (RegisterHotKey); PrtScn and Ctrl+Shift+1 by default | X11: `global-hotkey`. Wayland: global shortcuts portal |
| Menu bar item and popovers | `tray-icon`; a borderless popup next to the tray, drawn with tiny-skia | `ksni` (StatusNotifierItem); GNOME needs the AppIndicator extension |
| Keychain | Credential Manager via `keyring-core` + `windows-native-keyring-store` | Secret Service via `keyring-core` + `zbus-secret-service-keyring-store` |
| URLSession | `ureq` (rustls with ring) on a worker thread | same |
| Google sign-in on 127.0.0.1 | same flow, `std::net::TcpListener` | same |
| Pasteboard | `arboard` | same |
| Save panel | `rfd` | same |
| Launch at login | `HKCU\...\Run` registry value | XDG autostart file |
| Settings (UserDefaults) | TOML file in `%APPDATA%\Snapline\` (`serde`, `toml`) | `~/.config/Snapline/settings.toml` |
| Preferences (SwiftUI) | an `egui` window as a second process; its Upload tab arrives in Phase 3, Auto-hide in Phase 4 | same |

Crate versions are not pinned here. Add each with `cargo add` so the latest stable release is used, and check for
updates with `cargo outdated` at the start of each phase.

## Phases

Each phase ends with something that works on its own and has its own plan in `docs/plans/`.

1. **Workspace and core logic.** Cargo workspace, CI on Windows and Linux, `snapline-core` with everything from
   `macOS/Sources/SnaplineCore/` except `SymbolRepair` (it depends on the OCR engine, so it moves to Phase 4). Shared
   test cases for sensitive text. Plan: [docs/plans/2026-10-28-rust-core.md](plans/2026-10-28-rust-core.md).
2. **Windows capture and draw.** Tray icon, hotkey, capture every monitor, selection with handles and keyboard
   moves, all draw tools, undo, copy and save. CI uploads the `.exe` as a build artifact. Plan:
   [docs/plans/2026-10-29-windows-capture-draw.md](plans/2026-10-29-windows-capture-draw.md).
3. **Uploads.** S3 and Google Drive, credentials in Credential Manager, the upload popup with link, progress and errors,
   search similar images, confirmations, and the upload settings window. Plan:
   [docs/plans/2026-10-30-windows-uploads.md](plans/2026-10-30-windows-uploads.md).
4. **Text recognition.** Windows OCR, auto-hide with click-to-reveal, Copy Text with reading order, face detection,
   and the Auto-hide tab. `SymbolRepair` and the review popup moved out of this phase (see Decisions). Plan:
   [docs/plans/2026-10-31-windows-text-recognition.md](plans/2026-10-31-windows-text-recognition.md).
5. **Everything else, and shipping.**
   - 5a: edit in another app, Share (in place of email), print, the General tab (shortcut, tray click, save folder,
     editor, saving at 100 %, launch at sign-in), and the Phase 4 minors users notice. Plan:
     [docs/plans/2026-10-31-windows-more-actions.md](plans/2026-10-31-windows-more-actions.md).
   - 5b: installer, release workflow with optional code signing, website and privacy policy updated for Windows.
     Plan: [docs/plans/2026-10-31-windows-shipping.md](plans/2026-10-31-windows-shipping.md). How to release:
     [docs/releasing-windows.md](releasing-windows.md).
   - 5c: uploads through a proxy and the Windows certificate store, the minors users notice, safer release workflow,
     README and banner for both platforms. Plan: [docs/plans/2026-10-31-windows-polish.md](plans/2026-10-31-windows-polish.md).
6. **Linux.** 6a: X11, with .deb, AppImage and Flatpak. Plan: [docs/plans/2026-11-01-linux-x11.md](plans/2026-11-01-linux-x11.md).
   How to release: [docs/releasing-linux.md](releasing-linux.md). 6b: Wayland (screenshot and global shortcuts portals).

## Risks

- **Phone numbers.** macOS uses Apple's `NSDataDetector`, which Rust has no equal for. The Rust version uses its own
  pattern plus the 7 to 15 digit rule, and the shared test cases must include text that is not a phone number (dates,
  versions, prices, order numbers).
- **Regex features.** Some rules use lookbehind and lookahead, which the `regex` crate does not support. Use
  `fancy-regex` for those rules only.
- **OCR quality.** Windows OCR and Tesseract misread different characters than Vision. Auto-hide needs its own test
  images per engine, and on Linux it will miss more.
  In the 6a check, Tesseract on a whole 1080p screen over a busy line-pattern wallpaper dropped an email line
  that it read fine on the selection alone; reading at 2× found it but took about 7× as long.
- **OCR languages.** Windows OCR has no Indonesian. Snapline falls back to English OCR, which reads Latin text and
  digits.
- **Multiple monitors with different scaling.** Needs per-monitor DPI awareness from the first Windows build.
- **PrtScn on Windows 11.** Windows may give PrtScn to the Snipping Tool. Snapline then keeps Ctrl+Shift+1 and
  explains how to free PrtScn.
- **Networks that inspect TLS with their own root certificate.** ureq trusts the bundled web roots only, so uploads
  fail there. Add the Windows certificate store if users hit it.
- **SmartScreen.** An unsigned installer shows a warning. Code signing is planned for Phase 5.
- **Wayland.** Capture and hotkeys depend on portals that not every desktop has. 6a refuses Wayland sessions with a
  message.
- **Tray on GNOME.** GNOME shows no tray icons without an extension. Snapline says so once and keeps working from its
  shortcuts.
