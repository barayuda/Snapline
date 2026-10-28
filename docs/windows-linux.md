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

## Repo layout

```
macOS/                     the macOS app (Swift package, unchanged code)
windows-linux/             Cargo workspace
  crates/snapline-core/    pure logic, no UI, no OS calls: the Rust twin of SnaplineCore
  crates/snapline/         the app (from Phase 2): tray, hotkey, capture, overlay, actions
    src/platform/windows/  Windows-only code (OCR, credentials, capture details)
    src/platform/linux/    Linux-only code (from Phase 6)
fixtures/                  test cases shared by both apps (JSON)
Assets/, scripts/brand/    brand art and generator, shared
site/                      website
```

## What replaces each macOS piece

| macOS | Windows | Linux |
|---|---|---|
| ScreenCaptureKit | `xcap` (Windows.Graphics.Capture / DXGI); per-monitor DPI aware | X11: `xcap`. Wayland: screenshot portal (`ashpd`) |
| Overlay (AppKit window per screen) | `winit` borderless topmost window per monitor showing the frozen capture, drawn with `egui` | same; Wayland cannot place windows, so one fullscreen window per output |
| Drawing (Core Graphics) | `egui` painter on screen; `tiny-skia` to render the final image | same |
| Vision text recognition | Windows.Media.Ocr (`windows` crate), built into Windows 10/11 | Tesseract (bundled language data) |
| Vision face detection | Windows.Media.FaceAnalysis.FaceDetector | to be decided in Phase 6 |
| Carbon hotkey | `global-hotkey` (RegisterHotKey) | X11: `global-hotkey`. Wayland: global shortcuts portal |
| Menu bar item and popovers | `tray-icon` plus small `egui` popup windows near the tray | same; GNOME needs the AppIndicator extension |
| Keychain | Credential Manager via `keyring` | Secret Service via `keyring` |
| URLSession | `reqwest` with rustls | same |
| Google sign-in on 127.0.0.1 | same flow, `std::net::TcpListener` | same |
| Pasteboard | `arboard` | same |
| Save panel | `rfd` | same |
| Launch at login | `HKCU\...\Run` registry value | XDG autostart file |
| Settings (UserDefaults) | TOML file in `%APPDATA%\Snapline\` (`serde`, `toml`) | `~/.config/snapline/` |
| Preferences (SwiftUI) | `egui` window | same |

Crate versions are not pinned here. Add each with `cargo add` so the latest stable release is used, and check for
updates with `cargo outdated` at the start of each phase.

## Phases

Each phase ends with something that works on its own and has its own plan in `docs/plans/`.

1. **Workspace and core logic.** Cargo workspace, CI on Windows and Linux, `snapline-core` with everything from
   `macOS/Sources/SnaplineCore/` except `SymbolRepair` (it depends on the OCR engine, so it moves to Phase 4). Shared
   test cases for sensitive text. Plan: [docs/plans/2026-10-28-rust-core.md](plans/2026-10-28-rust-core.md).
2. **Windows capture and draw.** Tray icon, hotkey, capture every monitor, selection with handles and keyboard
   moves, all draw tools, undo, copy and save. CI uploads the `.exe` as a build artifact.
3. **Uploads.** S3 and Google Drive, credentials in Credential Manager, the upload popup with link, progress and errors.
4. **Text recognition.** Windows OCR, auto-hide with click-to-reveal, Copy Text with reading order and the review
   popup, face detection, `SymbolRepair` ported and its thresholds retuned for Windows OCR.
5. **Everything else, and shipping.** Search similar, email, print, edit in another app, Preferences window, launch at
   login, installer, code signing, website and privacy policy updated for Windows.
6. **Linux X11**, then **Linux Wayland** (portals, Flatpak).

## Risks

- **Phone numbers.** macOS uses Apple's `NSDataDetector`, which Rust has no equal for. The Rust version uses its own
  pattern plus the 7 to 15 digit rule, and the shared test cases must include text that is not a phone number (dates,
  versions, prices, order numbers).
- **Regex features.** Some rules use lookbehind and lookahead, which the `regex` crate does not support. Use
  `fancy-regex` for those rules only.
- **OCR quality.** Windows OCR and Tesseract misread different characters than Vision. Auto-hide needs its own test
  images per engine, and on Linux it will miss more.
- **Multiple monitors with different scaling.** Needs per-monitor DPI awareness from the first Windows build.
- **SmartScreen.** An unsigned installer shows a warning. Code signing is planned for Phase 5.
- **Wayland.** Capture and hotkeys depend on portals that not every desktop has.
