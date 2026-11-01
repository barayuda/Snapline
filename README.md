# Snapline

![Snapline: capture, mark up, hide what matters](Assets/banner.png)

A screenshot tool for the macOS menu bar, the Windows tray and Linux panels, modelled on Lightshot. Select an area, draw on it,
then copy, save, print, edit, share, upload (S3 or Google Drive) or search for similar images. Sensitive data is
hidden automatically on every capture.

- **macOS:** Swift 6.4, AppKit, ScreenCaptureKit, Vision, no third-party dependencies. Apple Silicon, macOS 15 or newer.
- **Windows:** Rust, with Windows' own text recognition and face detection. Windows 10 (1809) or 11, x64.
  [Install it](#windows-preview).
- **Linux:** preview for X11 and Wayland desktops (GNOME, KDE Plasma, Xfce and others; .deb, AppImage, Flatpak), from
  the same Rust code.
  [Install it](#linux-preview).
- Copy the text out of any screenshot, read on your computer.
- Written from scratch: Snapline shares no code or artwork with Lightshot.

## Privacy

Snapline is private by design. It has no servers, no accounts of its own, no analytics and no tracking.

- **Everything runs on your computer.** Capturing, drawing, auto-hide and Copy Text use the system's own frameworks
  on this device: ScreenCaptureKit and Vision on macOS, Windows OCR and face detection on Windows, Tesseract (built
  into Snapline) on Linux. No screenshot and
  no recognised text is sent to Snapline or to any third party to be read, analysed or stored.
- **An image leaves your computer only when you ask it to:**
  - **Upload** sends it to the destination you set up: your own S3 bucket or your own Google Drive.
  - **Search** uploads it there first, then opens Google Lens or TinEye in your browser with that link.
  - **Email** (macOS) and **Share** (Linux) hand the file to your mail app; **Share** (Windows) hands it to the app
    you pick.
- **Secrets stay in the system's keystore** (the macOS Keychain, Windows Credential Manager, or the desktop keyring
  on Linux): bucket keys and the
  Google sign-in token. Google sign-in runs in your browser; Snapline only receives the result on a local address
  (127.0.0.1).

You can check this in the code: all network calls are in `macOS/Sources/Snapline/Uploaders.swift` and
`GoogleAuth.swift` on macOS, and in `windows-linux/crates/snapline-upload/` on Windows and Linux.

## macOS

### Build and run

The macOS app is the Swift package in `macOS/`. Run these from there:

```bash
cd macOS
swift test                     # unit tests (signing, OAuth, matchers, geometry)
scripts/make-app.sh            # builds build/Snapline.app (arm64, ad-hoc signed)
scripts/make-app.sh --install  # same, then replaces /Applications/Snapline.app and starts it
open build/Snapline.app
```

On the first capture macOS asks for **Screen Recording** permission (System Settings → Privacy & Security → Screen &
System Audio Recording). If the build is signed ad hoc (no certificate, the default), macOS treats **each rebuild as a
new app**: it asks again, and Keychain may ask once to let Snapline read its saved keys ("Always Allow"). To avoid that,
see below.

#### Keep permissions across rebuilds

`macOS/scripts/make-app.sh` signs with a stable certificate when it finds one, in this order: the `SNAPLINE_SIGN_IDENTITY`
environment variable, an **Apple Development** certificate, a **Snapline Local Signing** certificate, otherwise ad hoc.
It prints which one it used. Create one of these once:

**Option A: free Apple Development certificate (recommended, needs an Apple Account)**
1. Open **Xcode → Settings → Accounts**, click **+**, and sign in with your Apple Account (a free account is enough).
2. Select the account, click **Manage Certificates…**, then **+** → **Apple Development**.
3. Run `macOS/scripts/make-app.sh --install`. It should print `Signed with: Apple Development: …`.

   If it prints `unable to build chain to self-signed root`, your Keychain is missing Apple's intermediate certificate:
   download **Worldwide Developer Relations - G3** from <https://www.apple.com/certificateauthority/> and double-click
   it to add it to the **login** keychain. `security find-identity -v -p codesigning` should then list it as valid.
   If it prints `errSecInternalComponent`, macOS needs your OK to use the key: run the script once in **Terminal**,
   enter your login password in the dialog and click **Always Allow**.

**Option B: self-signed certificate (no Apple Account)**
1. Open **Keychain Access** → menu **Keychain Access → Certificate Assistant → Create a Certificate…**
2. Name: `Snapline Local Signing`, Identity Type: **Self Signed Root**, Certificate Type: **Code Signing** → Create.
3. In the **login** keychain, double-click the new certificate → **Trust** → **Code Signing: Always Trust**, close
   the window and confirm with your password.
4. Run `macOS/scripts/make-app.sh --install`. It should print `Signed with: Snapline Local Signing`.

After switching, macOS asks for Screen Recording **one last time** (the signature changed). Remove old "Snapline"
entries from the list with **−**, allow the new one, and from then on rebuilds keep the permission.

### Use

| Action | How |
|---|---|
| Capture | Press the shortcut (default ⇧⌘1, change it in Preferences), or choose Take Screenshot from the menu |
| Menu | Click the menu bar icon. To make a click capture instead, like Lightshot, set Preferences → General → Clicking the menu bar icon; the menu is then on right-click (or ⌃-click) |
| Select | Drag; drag inside to move; drag the white handles to resize; ⌘A whole screen |
| Move / resize by keyboard | Arrows move 1 pt, ⇧ + arrows 10 pt, ⌥ + arrows resize |
| Draw tools | Toolbar right of the selection, or keys P pencil, L line, A arrow, R rectangle, E ellipse, M marker, T text, H hide area, V back to select |
| Line width | Scroll wheel, or + / − |
| Text | Text tool, click, type, Return. Scroll while typing to change size. Drag a finished text with the text tool to move it |
| Undo | ⌘Z |
| Copy image | ⌘C or Return |
| Copy text | ⇧⌘C or the text button: reads the text in the selection on your Mac and copies it as plain text, in reading order, with wrapped lines joined back into sentences. Keyboard symbols (⌘ ⇧ ⌥ ⌃ ↩ ⏎ ⎋ ⇥ ⇤ ⌫ ⌦ ⇪ ⇞ ⇟ ↖ ↘ ⏏ ↑ ↓ ← → ✓ ≠ ±), which Apple's text recognition cannot read, are restored by comparing their shapes, and symbols it skipped are put back. If something could not be read for sure, a review popover under the menu bar icon shows it highlighted, with a symbol palette, before anything is copied. Hidden areas are not read |
| Save | ⌘S saves straight to the folder set in Preferences → General (Desktop by default) as `Screenshot at MMM dd HH-mm-ss.png`; never overwrites. ⌥⌘S (or ⌥-click Save) opens a Save As dialog |
| Upload | ⌘D. The menu bar icon shows a progress ring, then ✓ (or a red !). A popover under the icon shows the link, already copied, with Copy / Open / Email; on failure it explains why, with Preferences and Try again |
| Edit in another app | ⌘E (Preview by default; choose another app in Preferences) |
| Print | ⌘P |
| Search similar images | Magnifier button → Google Lens or TinEye (uploads first) |
| Cancel | Esc |

Confirmations such as "Screenshot saved" or "Text copied" appear briefly in a small popover under the menu bar icon. Preferences opens in the same popover (menu → Preferences…, or ⌘,). It stays open while you switch to a browser to copy keys; close it with ✕, Esc or a click on the menu bar icon, or drag it away from the menu bar to keep it as a floating window.

#### Auto-hide sensitive data

When the screen freezes, Snapline reads it with Apple's on-device text recognition and covers matches with solid black boxes. Nothing is sent anywhere.

- Finds: email addresses, phone numbers, card numbers (Luhn-checked), IBANs (mod-97 checked), API keys and tokens (AWS, Google, GitHub, Slack, Stripe, JWT, private key headers, `Bearer …`), values after `password=` / `token:` / `api_key=` style labels, long random strings, and faces.
- Click a black box to show that item again (it gets a yellow dashed outline); click again to hide it. ⌘Z undoes this too.
- The eye button on the draw toolbar turns auto-hide off for the current capture. Preferences → Auto-hide turns it off for good or per type.
- The boxes are merged into the pixels of everything you copy, save, print, edit, email or upload.
- **It is a safety net, not a guarantee.** Very small, faint, rotated or stylised text can be missed. Check the image before you share it. Use the Hide tool (H) for anything it missed.

Copied symbols are standard Unicode, so they paste the same on any computer whose fonts have them (macOS shows all; Windows 10/11
and most Linux desktops show the common ones). For places that show boxes, turn on **Preferences → General → Copy Text: write
keyboard symbols as words** (⇧⌘C becomes Shift+Cmd+C). Text is read in the languages set in System Settings → Language & Region,
with English as a fallback.

To check what auto-hide would cover, or what Copy Text would copy (spots for review shown as ⟦…⟧), in an image without capturing
the screen (from `macOS/`, after `swift build`):

```bash
.build/debug/Snapline --scan some-image.png
.build/debug/Snapline --text some-image.png
```

## Windows (preview)

The Windows version is in progress (see [docs/windows-linux.md](docs/windows-linux.md)). It can capture, select,
draw, copy, save, upload, share, print, edit, hide sensitive data and copy text. Keyboard-symbol repair in Copy Text
comes later.

To install it: download **Snapline-Setup** from the [latest release](https://github.com/barayuda/Snapline/releases/latest)
and run it (no admin rights needed), or use the portable zip. It is not signed yet, so Windows SmartScreen asks first
(More info → Run anyway). Preview builds of every push are under Actions: the latest successful **windows-linux**
run has **snapline-windows** (the exe) and **snapline-windows-dist** (installer, zip and checksums). How releases
are made: [docs/releasing-windows.md](docs/releasing-windows.md).

- Take a screenshot: PrtScn or Ctrl+Shift+1, or the tray icon's menu. Change the shortcut in the tray menu →
  **Settings…** → General. If PrtScn opens the Snipping Tool instead, turn off Settings → Accessibility → Keyboard →
  "Use the Print screen key to open screen capture" and restart Snapline.
- In the capture: the same keys as on macOS with Ctrl for ⌘. Ctrl+Shift+S is Save As.
- Edit: Ctrl+E or the sliders button opens the capture in Paint, or in the app chosen under Settings → General.
- Share: the share button opens the Windows Share window (Outlook, Mail, Teams and others). It replaces Email.
- Print: Ctrl+P or the printer button.
- Upload: Ctrl+D or the Upload button. Set the bucket or Google Drive in the tray menu → **Settings…** → Upload (the same
  fields as on macOS; keys are kept in Windows Credential Manager). The link is copied when the upload finishes.
- Networks with a proxy: uploads use the proxy set in Settings → Network & internet → Proxy → "Use a proxy server"
  (or `HTTPS_PROXY`), and trust the certificates Windows trusts. Automatic proxy setup (a setup script or "Automatically
  detect settings") and proxies that ask for a sign-in are not supported yet.
- Search similar images: the magnifier button, then Google Lens or TinEye.
- Auto-hide: every capture is read on your PC, and card numbers, keys, passwords, phone numbers, emails and faces
  get black boxes before you copy, save or upload. Click a box to show it again; the eye button turns auto-hide off
  for this capture. Choose what to hide in the tray menu → **Settings…** → Auto-hide.
- Copy Text: Ctrl+Shift+C or the text button. Needs a Windows language with text recognition (Settings → Time &
  language → Language & region).
- Settings: the tray menu → **Settings…** has General (shortcut, tray click, save folder, editor, saving at 100 % size,
  launch at sign-in), Upload and Auto-hide tabs. Changes apply at once. They are stored in
  `%APPDATA%\Snapline\settings.toml`.

To build it: `cd windows-linux && cargo run -p snapline` (it also runs on macOS, for trying the overlay).

## Linux (preview)

The Linux version runs on **X11** (Xfce, Cinnamon, MATE, KDE Plasma and GNOME on X11) and **Wayland** (GNOME, KDE
Plasma; wlroots desktops such as sway and Hyprland on a best-effort basis). It does what the Windows version does, with
these differences:

- **On Wayland** Snapline's windows run through XWayland (installed with GNOME and KDE Plasma), and the desktop takes
  the screenshot through its screenshot portal:
  - GNOME may ask once to allow Snapline to take screenshots (Settings → Apps → Snapline → Screenshots).
  - The capture shortcut comes from the desktop: KDE Plasma 6, GNOME 48 or newer and Hyprland ask you to confirm it
    once. Elsewhere (GNOME 46 and 47, for example), add a custom shortcut in the desktop's keyboard settings that runs
    `snapline --capture` (`flatpak run dev.barayuda.Snapline --capture` for the Flatpak, or the AppImage path). It
    also starts Snapline when it is not running.
  - Fractional scaling (125 %, 150 %) and monitors with different scales are not checked yet: the overlay may look
    soft, or not line up with the screen. Please report what you see.

- No face detection yet, so faces are not hidden; text is.
- Share opens a new email with the screenshot attached (`xdg-email`), as Email does on macOS.
- Print opens the screenshot in your image viewer; press Ctrl+P there.
- The tray icon needs a panel that shows tray icons. On GNOME, install the "AppIndicator and KStatusNotifierItem
  Support" extension; without a tray, the shortcuts still work and `snapline --settings` opens the settings.
- Bucket keys and the Google sign-in go to the desktop keyring (GNOME Keyring or KWallet).
- Proxies come from `HTTPS_PROXY` (and `HTTP_PROXY`, `NO_PROXY`); certificates are the system's.
- Settings are stored in `~/.config/Snapline/settings.toml`; Edit, Share and Print write their copy to
  `~/.cache/Snapline/temp`.

To install it, download from the [releases](https://github.com/barayuda/Snapline/releases) (tags `linux-v…`):

- **Ubuntu, Debian, Mint:** `sudo apt install ./snapline_<version>_amd64.deb` (or `arm64`).
- **Other distributions:** the AppImage: make it executable and run it (glibc 2.39 or newer).
- **Flatpak:** `flatpak install --user Snapline-<version>-x86_64.flatpak` (the Flathub remote must be set up for the
  runtime).

Preview builds of every push are under Actions: the latest successful **windows-linux** run has **snapline-linux-X64**
and **snapline-linux-ARM64** (.deb and AppImage) and the Flatpak bundles. How releases are made:
[docs/releasing-linux.md](docs/releasing-linux.md).

To build it: install the packages in `windows-linux/linux/apt-packages.txt` (Ubuntu 24.04), then
`cd windows-linux && cargo run -p snapline`. On a Mac, `windows-linux/linux/dev-vm.sh setup` makes an OrbStack Ubuntu
machine to build and test in.

## Upload setup

Pick the target in Preferences → Upload. Anything uploaded is public to whoever has the link. Do not upload guest or customer data.

On Windows and Linux the same fields are in the tray menu → **Settings…** → Upload, and keys are kept in Windows
Credential Manager or the Linux desktop keyring.

### S3-compatible bucket (AWS S3, Cloudflare R2, MinIO)

| Field | Example |
|---|---|
| Endpoint | `https://<account-id>.r2.cloudflarestorage.com` or `https://s3.ap-southeast-1.amazonaws.com` |
| Region | `auto` for R2, the bucket region for AWS |
| Bucket | `screenshots` |
| Key prefix | `snapline` → objects at `snapline/2026/10/<random>.png` |
| Public URL | The public base URL of the bucket, e.g. an R2 custom domain `https://shots.example.com` |
| Access key / secret | An API token with write access to this bucket only. Stored in the Keychain |

The bucket (or the prefix) must allow public reads, otherwise the link will not open.

The endpoint must use HTTPS, because your keys are sent with every upload. The one exception is a server on this Mac
(`http://localhost:…`, `127.0.0.1`, `::1`), such as a local MinIO or floci for testing. A server elsewhere on your local
network (a NAS, an `*.orb.local` container name) needs HTTPS, and macOS asks once for **Local Network** permission.

To test uploads against a local S3-compatible server (it creates and deletes its own bucket), from `macOS/`:

```bash
SNAPLINE_S3_TEST_ENDPOINT=http://localhost:4566 swift test --filter S3IntegrationTests
```

### Google Drive (one-time setup, about 10 minutes)

Snapline signs in with your own Google Cloud OAuth client, so you create one first:

1. Open <https://console.cloud.google.com/>, create a project (for example "Snapline").
2. **APIs & Services → Library**: enable **Google Drive API**.
3. **Google Auth Platform → Branding** (or "OAuth consent screen"): app name "Snapline", your Gmail as support and developer contact. **Audience**: user type **External**.
4. **Data access → Add or remove scopes**: add `.../auth/drive.file`, `openid` and `.../auth/userinfo.email`. All three are non-sensitive, so Google does not need to review the app.
5. **Clients → Create client**: application type **Desktop app**, name "Snapline". Copy the **client ID** and **client secret**.
6. **Audience → Publish app** (status **In production**). Important: while an External app is in "Testing", Google expires sign-ins after 7 days and you would have to sign in again every week.
7. In Snapline Preferences → Upload → Google Drive, paste the client ID and secret, then click **Sign in with Google…**. Your browser opens Google's normal sign-in page; Snapline never sees your password. Because the app is not verified by Google, you may see "Google hasn't verified this app": click **Advanced → Go to Snapline**. This is your own app, used only by you.

After that, uploads go to a `Snapline` folder in My Drive and are shared as "anyone with the link can view". Snapline can only see files it created. **Sign out** in Preferences also revokes its access at Google.

Image search with Drive uploads uses an undocumented Drive direct-image link, which Google sometimes blocks for search engines. If Lens or TinEye cannot load the image, use S3 for searches.

## Project layout

```
macOS/                        the macOS app (Swift package)
  Sources/SnaplineCore/       pure logic, unit tested: SigV4 signing, PKCE/OAuth helpers,
                              Drive multipart body, file names, sensitive-text matchers, geometry
  Sources/Snapline/           the app: menu bar, hot key, capture, overlay, drawing, actions,
                              uploaders, Google sign-in, auto-hide scan, Preferences
  Tests/SnaplineCoreTests/
  scripts/make-app.sh         bundle + sign
windows-linux/                the Windows and Linux app (Rust workspace)
  crates/snapline-core/       the Rust twin of SnaplineCore
  crates/snapline-overlay/    the capture overlay without windows: selection, drawing, toolbars (tiny-skia)
  crates/snapline-upload/     uploads without UI or storage: S3, Google Drive, Google sign-in
  crates/snapline/            the Windows and Linux app: tray, shortcuts, capture, overlay windows, copy, save, uploads
  linux/                      Linux packaging (.deb, AppImage, Flatpak), build packages, the OrbStack dev machine
fixtures/                     test cases shared by the Swift and Rust tests
docs/                         design notes and plans
scripts/brand/                icon and banner generator, brand fonts (OFL)
Assets/                       AppIcon.icon (layered app icon), icon-1024.png, banner.png (generated; also site/assets/og.png)
site/                         website: home, privacy policy, terms (static, deployed by Netlify)
```

Brand colours, type and icon rules: [BRAND.md](BRAND.md).

## Website

<https://snapline.barayuda.web.id> is the static site in `site/` (plain HTML and CSS, no build step). Google's OAuth
consent screen links to its privacy policy and terms, so update `site/privacy/` when the app's data handling changes.

Netlify deploys only the `website` branch (set as the production branch in Netlify; branch deploys and deploy previews
off), so pushes to `main` never deploy. To publish the current `main`:

```bash
git push origin main:website
```

Never commit to `website` directly. Every push to it deploys.

To preview locally: `python3 -m http.server 8642 --directory site`, then open <http://localhost:8642>.

## Credits

Snapline is inspired by **[Lightshot Screenshot](https://app.prntscr.com/)** by Skillbrains: thank you for the
"select, mark up, share" workflow that so many people rely on. Snapline is an independent project written from scratch
for Apple Silicon. It is not affiliated with or endorsed by Skillbrains, and "Lightshot" is their trademark.

## License

Free for personal and other noncommercial use under the [PolyForm Noncommercial License 1.0.0](LICENSE.md): you may use,
study and change Snapline, but not sell it or use it commercially. Copies must keep both "Required Notice" lines (copyright and the Snapline brand).

The Snapline name, icon and logo are not covered by the license; see [TRADEMARKS.md](TRADEMARKS.md).
The fonts in `scripts/brand/fonts/` keep their own SIL Open Font License (license files next to them).
