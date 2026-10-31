# Snapline brand

**Idea:** a frame and a spark. Four selection corners say "pick an area"; the spark in the middle is the moment the
shot is taken and marked up. Everything else stays quiet, so the frame does the talking.

![Icon](Assets/icon-1024.png)

## Colour

| Name | Hex | Use |
|---|---|---|
| Violet deep | `#4C1AB8` | Icon gradient start (top left) |
| Electric violet | `#7C3AED` | Primary brand colour; size badge, handle rings, Preferences tint |
| Violet light | `#8B5CF6` | Selected tool, text on dark |
| Cyan | `#22D3EE` | Icon gradient end; selection frame |
| Ink | `#0C0819` | Dark backgrounds, neutral badges |

The gradient always runs violet (top left) → cyan (bottom right). Do not reverse it or add other hues.

## Type

- **Wordmark:** Outfit Bold, tight tracking (−3 at 196 px). Always "Snapline", one word, capital S.
- **Tagline:** Outfit Regular: "Capture. Mark up. Hide what matters."
- **Small labels:** Geist Mono, uppercase, wide tracking.
- **In the app:** the system font (SF Pro), so it feels native.

Outfit and Geist Mono are under the SIL Open Font License; copies and licences are in `scripts/brand/fonts/`.

## Icon

- The app uses a layered Icon Composer document, `Assets/AppIcon.icon`: a full-bleed gradient **background** layer and a
  white **glyph** layer (corners + sparks). macOS 26+ applies its own shape, glass edge and shadow; `actool` (run by
  `macOS/scripts/make-app.sh`) also builds a classic `AppIcon.icns` from it for macOS 15. A plain `.icns` alone gets placed
  inside a grey square on macOS 26+, so keep the layered document.
- Glyph: white corners (stroke 54, round caps) and a four-point spark with a small companion spark.
- `Assets/icon-1024.png` is a flat preview (824 body on the 1024 grid, continuous corners, gloss, rim light) for
  documents and the banner. At 64 px and below it uses heavier corners and a single spark.
- Menu bar: the same corners and spark as an 18-pt template image (`Brand.menuBarImage`), tinted by macOS.

## Regenerate

```bash
swift scripts/brand/make-brand.swift   # writes Assets/AppIcon.icon, icon-1024.png, banner.png
macOS/scripts/make-app.sh              # compiles the icon with actool and bundles it (needs Xcode)
```
