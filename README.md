# Snapline

A native Apple Silicon screenshot tool for the macOS menu bar, modelled on Lightshot. Select an area, draw on it, then copy, save, print, edit, email, upload (S3 or Google Drive) or search for similar images. Sensitive data is hidden automatically on every capture.

- Swift 6.4, AppKit, ScreenCaptureKit, Vision. No third-party dependencies.
- arm64 only, macOS 15 or newer.
- Design notes: `../lightshot-rea/SNAPLINE-SPEC.md`. Reverse-engineering notes on the original: `../lightshot-rea/FINDINGS.md`.

## Build and run

```bash
swift test                     # unit tests (signing, OAuth, matchers, geometry)
scripts/make-app.sh            # builds build/Snapline.app (arm64, ad-hoc signed)
scripts/make-app.sh --install  # same, then copies it to ~/Applications
open build/Snapline.app
```

On the first capture macOS asks for **Screen Recording** permission (System Settings → Privacy & Security → Screen & System Audio Recording). The app is signed ad hoc, so after **each rebuild** macOS may treat it as a new app: switch Snapline off and on again in that list. For the same reason, Keychain may ask once per rebuild to let Snapline read its saved keys; choose "Always Allow".

## Use

| Action | How |
|---|---|
| Capture | Click the menu bar icon, or press the shortcut (default ⇧⌘1, change it in Preferences) |
| Menu | Right-click (or ⌃-click) the menu bar icon |
| Select | Drag; drag inside to move; drag the white handles to resize; ⌘A whole screen |
| Move / resize by keyboard | Arrows move 1 pt, ⇧ + arrows 10 pt, ⌥ + arrows resize |
| Draw tools | Toolbar right of the selection, or keys P pencil, L line, A arrow, R rectangle, E ellipse, M marker, T text, H hide area, V back to select |
| Line width | Scroll wheel, or + / − |
| Text | Text tool, click, type, Return. Scroll while typing to change size. Drag a finished text with the text tool to move it |
| Undo | ⌘Z |
| Copy | ⌘C or Return |
| Save | ⌘S (PNG, `Screenshot at MMM dd HH-mm-ss.png`) |
| Upload | ⌘D. The link is copied to the clipboard |
| Edit in another app | ⌘E (Preview by default; choose another app in Preferences) |
| Print | ⌘P |
| Search similar images | Magnifier button → Google Lens or TinEye (uploads first) |
| Cancel | Esc |

### Auto-hide sensitive data

When the screen freezes, Snapline reads it with Apple's on-device text recognition and covers matches with solid black boxes. Nothing is sent anywhere.

- Finds: email addresses, phone numbers, card numbers (Luhn-checked), IBANs (mod-97 checked), API keys and tokens (AWS, Google, GitHub, Slack, Stripe, JWT, private key headers, `Bearer …`), values after `password=` / `token:` / `api_key=` style labels, long random strings, and faces.
- Click a black box to show that item again (it gets a yellow dashed outline); click again to hide it. ⌘Z undoes this too.
- The eye button on the draw toolbar turns auto-hide off for the current capture. Preferences → Auto-hide turns it off for good or per type.
- The boxes are merged into the pixels of everything you copy, save, print, edit, email or upload.
- **It is a safety net, not a guarantee.** Very small, faint, rotated or stylised text can be missed. Check the image before you share it. Use the Hide tool (H) for anything it missed.

To check what it would hide in an image without capturing the screen:

```bash
.build/debug/Snapline --scan some-image.png
```

## Upload setup

Pick the target in Preferences → Upload. Anything uploaded is public to whoever has the link. Do not upload guest or customer data.

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
Sources/SnaplineCore/   pure logic, unit tested: SigV4 signing, PKCE/OAuth helpers,
                        Drive multipart body, file names, sensitive-text matchers, geometry
Sources/Snapline/       the app: menu bar, hot key, capture, overlay, drawing, actions,
                        uploaders, Google sign-in, auto-hide scan, Preferences
Tests/SnaplineCoreTests/
scripts/make-app.sh     bundle + ad-hoc sign
```
