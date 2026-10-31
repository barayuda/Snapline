# Snapline for Windows and Linux: design

Status: draft, 28 October 2026. Branch `feat/windows-linux-rust`.

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

## Repo layout

```
macOS/                     the macOS app (Swift package, unchanged code)
windows-linux/             Cargo workspace
  crates/snapline-core/    pure logic, no UI, no OS calls: the Rust twin of SnaplineCore
  crates/snapline-overlay/ the overlay without windows: selection, drawing, text, toolbars, popup, rendering (tiny-skia)
  crates/snapline-upload/  uploads without UI or storage: S3, Google Drive, Google sign-in (tested on every OS)
  crates/snapline/         the app (from Phase 2): tray, hotkey, capture, overlay windows, actions
    src/platform.rs        Windows-only calls for now; splits into platform/windows and platform/linux when they grow
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
| Vision text recognition | Windows.Media.Ocr (`windows` crate), built into Windows 10/11 | Tesseract (bundled language data) |
| Vision face detection | Windows.Media.FaceAnalysis.FaceDetector | to be decided in Phase 6 |
| Carbon hotkey | `global-hotkey` (RegisterHotKey); PrtScn and Ctrl+Shift+1 by default | X11: `global-hotkey`. Wayland: global shortcuts portal |
| Menu bar item and popovers | `tray-icon`; a borderless popup next to the tray, drawn with tiny-skia | same; GNOME needs the AppIndicator extension |
| Keychain | Credential Manager via `keyring-core` + `windows-native-keyring-store` | Secret Service via `keyring-core` |
| URLSession | `ureq` (rustls with ring) on a worker thread | same |
| Google sign-in on 127.0.0.1 | same flow, `std::net::TcpListener` | same |
| Pasteboard | `arboard` | same |
| Save panel | `rfd` | same |
| Launch at login | `HKCU\...\Run` registry value | XDG autostart file |
| Settings (UserDefaults) | TOML file in `%APPDATA%\Snapline\` (`serde`, `toml`) | `~/.config/snapline/` |
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
6. **Linux X11**, then **Linux Wayland** (portals, Flatpak).

## Risks

- **Phone numbers.** macOS uses Apple's `NSDataDetector`, which Rust has no equal for. The Rust version uses its own
  pattern plus the 7 to 15 digit rule, and the shared test cases must include text that is not a phone number (dates,
  versions, prices, order numbers).
- **Regex features.** Some rules use lookbehind and lookahead, which the `regex` crate does not support. Use
  `fancy-regex` for those rules only.
- **OCR quality.** Windows OCR and Tesseract misread different characters than Vision. Auto-hide needs its own test
  images per engine, and on Linux it will miss more.
- **OCR languages.** Windows OCR has no Indonesian. Snapline falls back to English OCR, which reads Latin text and
  digits.
- **Multiple monitors with different scaling.** Needs per-monitor DPI awareness from the first Windows build.
- **PrtScn on Windows 11.** Windows may give PrtScn to the Snipping Tool. Snapline then keeps Ctrl+Shift+1 and
  explains how to free PrtScn.
- **Networks that inspect TLS with their own root certificate.** ureq trusts the bundled web roots only, so uploads
  fail there. Add the Windows certificate store if users hit it.
- **SmartScreen.** An unsigned installer shows a warning. Code signing is planned for Phase 5.
- **Wayland.** Capture and hotkeys depend on portals that not every desktop has.
