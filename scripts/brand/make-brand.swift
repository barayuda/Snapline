// Renders Snapline's brand assets into Assets/:
//   AppIcon.icon (layered icon, compiled by actool in make-app.sh), icon-1024.png (preview), banner.png
// Usage (from the repo root): swift scripts/brand/make-brand.swift
// Everything is drawn with CoreGraphics so the artwork is reproducible and editable here.
import AppKit
import CoreText

// MARK: - Palette (keep in sync with Sources/Snapline/Brand.swift)

func rgb(_ hex: UInt32, _ a: CGFloat = 1) -> CGColor {
    CGColor(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255, alpha: a)
}
let violetDeep = rgb(0x4C1AB8)
let violet = rgb(0x7C3AED)
let cyan = rgb(0x22D3EE)
let ink = rgb(0x0C0819)
let white = rgb(0xFFFFFF)

let space = CGColorSpace(name: CGColorSpace.sRGB)!

func gradient(_ stops: [(CGColor, CGFloat)]) -> CGGradient {
    CGGradient(colorsSpace: space, colors: stops.map(\.0) as CFArray, locations: stops.map(\.1))!
}

func context(_ w: Int, _ h: Int) -> CGContext {
    let c = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0, space: space,
                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    c.setShouldAntialias(true)
    c.interpolationQuality = .high
    return c
}

func writePNG(_ c: CGContext, _ path: String) {
    let rep = NSBitmapImageRep(cgImage: c.makeImage()!)
    try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: path))
}

// MARK: - Shapes (all on a 1024 × 1024 grid, origin bottom-left)

/// macOS app icon shape: straight sides with continuous ("squircle") corners.
/// Uses the widely published cubic approximation of Apple's continuous corner:
/// each corner starts 1.5287 × radius away from the corner point.
func squircle(_ r: CGRect, radius: CGFloat = 185) -> CGPath {
    // Corner points as (distance along the incoming edge, distance along the outgoing edge), in radii.
    let segments: [(CGFloat, CGFloat, CGFloat, CGFloat, CGFloat, CGFloat)] = [
        (1.08849296, 0, 0.86840694, 0.02123576, 0.63149399, 0.07491100),
        (0.37282383, 0.13518517, 0.13518517, 0.37282383, 0.07491100, 0.63149399),
        (0.02123576, 0.86840694, 0, 1.08849296, 0, 1.52866483),
    ]
    let e: CGFloat = 1.52866483
    // Clockwise (y up): corner point, unit vector back along the incoming edge, unit vector along the outgoing edge.
    let corners: [(CGPoint, CGVector, CGVector)] = [
        (CGPoint(x: r.maxX, y: r.maxY), CGVector(dx: -1, dy: 0), CGVector(dx: 0, dy: -1)),
        (CGPoint(x: r.maxX, y: r.minY), CGVector(dx: 0, dy: 1), CGVector(dx: -1, dy: 0)),
        (CGPoint(x: r.minX, y: r.minY), CGVector(dx: 1, dy: 0), CGVector(dx: 0, dy: 1)),
        (CGPoint(x: r.minX, y: r.maxY), CGVector(dx: 0, dy: -1), CGVector(dx: 1, dy: 0)),
    ]
    func pt(_ k: CGPoint, _ u: CGVector, _ w: CGVector, _ a: CGFloat, _ b: CGFloat) -> CGPoint {
        CGPoint(x: k.x + (u.dx * a + w.dx * b) * radius, y: k.y + (u.dy * a + w.dy * b) * radius)
    }
    let p = CGMutablePath()
    let (k0, u0, w0) = corners[3]
    p.move(to: pt(k0, u0, w0, 0, e))
    for (k, u, w) in corners {
        p.addLine(to: pt(k, u, w, e, 0))
        for s in segments {
            p.addCurve(to: pt(k, u, w, s.4, s.5), control1: pt(k, u, w, s.0, s.1), control2: pt(k, u, w, s.2, s.3))
        }
    }
    p.closeSubpath()
    return p
}

/// Four-point spark with concave sides.
func spark(center c: CGPoint, radius r: CGFloat, waist: CGFloat = 0.13) -> CGPath {
    let p = CGMutablePath()
    let tips = [CGPoint(x: c.x, y: c.y + r), CGPoint(x: c.x + r, y: c.y), CGPoint(x: c.x, y: c.y - r), CGPoint(x: c.x - r, y: c.y)]
    let k = r * waist
    let controls = [CGPoint(x: c.x + k, y: c.y + k), CGPoint(x: c.x + k, y: c.y - k),
                    CGPoint(x: c.x - k, y: c.y - k), CGPoint(x: c.x - k, y: c.y + k)]
    p.move(to: tips[0])
    for i in 0..<4 { p.addQuadCurve(to: tips[(i + 1) % 4], control: controls[i]) }
    p.closeSubpath()
    return p
}

/// The four selection corners, as one stroked path.
func corners(inset: CGFloat, arm: CGFloat, in r: CGRect) -> CGPath {
    let p = CGMutablePath()
    let box = r.insetBy(dx: inset, dy: inset)
    for (x, y, dx, dy) in [(box.minX, box.maxY, 1.0, -1.0), (box.maxX, box.maxY, -1.0, -1.0),
                           (box.maxX, box.minY, -1.0, 1.0), (box.minX, box.minY, 1.0, 1.0)] {
        p.move(to: CGPoint(x: x, y: y + dy * arm))
        p.addLine(to: CGPoint(x: x, y: y))
        p.addLine(to: CGPoint(x: x + dx * arm, y: y))
    }
    return p
}

// MARK: - Icon

/// Gradient and glows, filling everything (the caller clips, or macOS masks the layer).
func drawBackground(_ c: CGContext) {
    // Base: deep violet top-left → electric violet → cyan bottom-right.
    c.drawLinearGradient(gradient([(violetDeep, 0), (violet, 0.48), (cyan, 1)]),
                         start: CGPoint(x: 160, y: 900), end: CGPoint(x: 880, y: 110),
                         options: [.drawsBeforeStartLocation, .drawsAfterEndLocation])
    // Depth: a cool glow rising from the bottom-right, a soft light from the top-left, a halo behind the spark.
    let glows: [(color: UInt32, alpha: CGFloat, center: CGPoint, radius: CGFloat)] = [
        (0x67E8F9, 0.55, CGPoint(x: 820, y: 180), 520),
        (0xA78BFA, 0.30, CGPoint(x: 260, y: 860), 560),
        (0xFFFFFF, 0.40, CGPoint(x: 512, y: 512), 230),
    ]
    for g in glows {
        let stops: [(CGColor, CGFloat)] = [(rgb(g.color, g.alpha), 0), (rgb(g.color, 0), 1)]
        c.drawRadialGradient(gradient(stops), startCenter: g.center, startRadius: 0,
                             endCenter: g.center, endRadius: g.radius, options: [])
    }
}

/// White corners and sparks. Small sizes get heavier strokes and a single, larger spark.
func drawGlyph(_ c: CGContext, small: Bool, shadows: Bool) {
    let body = CGRect(x: 100, y: 100, width: 824, height: 824)
    c.saveGState()
    if shadows { c.setShadow(offset: CGSize(width: 0, height: -8), blur: 22, color: rgb(0x1E0757, 0.45)) }
    c.addPath(corners(inset: small ? 120 : 132, arm: small ? 180 : 168, in: body))
    c.setLineWidth(small ? 78 : 54)
    c.setLineCap(.round)
    c.setLineJoin(.round)
    c.setStrokeColor(white)
    c.strokePath()
    c.restoreGState()

    // Spark: white with a faint cyan heart.
    let center = CGPoint(x: 512, y: 512)
    let r: CGFloat = small ? 170 : 150
    c.saveGState()
    c.addPath(spark(center: center, radius: r))
    c.clip()
    let heart: [(CGColor, CGFloat)] = [(rgb(0xE0FBFF), 0), (white, 0.55)]
    c.drawRadialGradient(gradient(heart), startCenter: center, startRadius: 0, endCenter: center, endRadius: r, options: [])
    c.restoreGState()
    if !small {
        // A small companion spark, upper right, for rhythm.
        c.addPath(spark(center: CGPoint(x: 628, y: 628), radius: 44, waist: 0.16))
        c.setFillColor(rgb(0xFFFFFF, 0.92))
        c.fillPath()
    }
}

/// Classic flat rendering: body on the 1024 grid with shadow, gloss and rim light.
/// Used for the README preview and the banner; the app bundle uses the layered .icon below.
func drawIcon(_ c: CGContext, px: Int) {
    let small = px <= 64
    c.saveGState()
    c.scaleBy(x: CGFloat(px) / 1024, y: CGFloat(px) / 1024)

    let shape = squircle(CGRect(x: 100, y: 100, width: 824, height: 824))
    if !small {
        c.saveGState()
        c.setShadow(offset: CGSize(width: 0, height: -12), blur: 28, color: rgb(0x000000, 0.35))
        c.addPath(shape)
        c.setFillColor(violetDeep)
        c.fillPath()
        c.restoreGState()
    }

    c.saveGState()
    c.addPath(shape)
    c.clip()
    drawBackground(c)
    // Gloss: a gentle sheen over the upper half.
    let gloss: [(CGColor, CGFloat)] = [(rgb(0xFFFFFF, 0.20), 0), (rgb(0xFFFFFF, 0.04), 0.55), (rgb(0xFFFFFF, 0), 1)]
    c.drawLinearGradient(gradient(gloss), start: CGPoint(x: 512, y: 924), end: CGPoint(x: 512, y: 470), options: [])
    drawGlyph(c, small: small, shadows: !small)
    c.restoreGState()

    // Rim light: hairline highlight on the edge, brighter at the top.
    if !small {
        c.saveGState()
        c.addPath(shape)
        c.setLineWidth(3)
        c.replacePathWithStrokedPath()
        c.clip()
        let rim: [(CGColor, CGFloat)] = [(rgb(0xFFFFFF, 0.55), 0), (rgb(0xFFFFFF, 0.05), 1)]
        c.drawLinearGradient(gradient(rim), start: CGPoint(x: 512, y: 924), end: CGPoint(x: 512, y: 100), options: [])
        c.restoreGState()
    }
    c.restoreGState()
}

/// Layered Icon Composer document (background + glyph). macOS 26+ applies its own shape, glass
/// and shadow; actool also derives a classic .icns from it for macOS 15.
func writeIconDocument(_ dir: String) {
    let assetsDir = "\(dir)/Assets"
    try? FileManager.default.removeItem(atPath: dir)
    try! FileManager.default.createDirectory(atPath: assetsDir, withIntermediateDirectories: true)
    // Layers are full-bleed: map the 824-pt body of the classic grid onto the whole 1024 canvas.
    func layer(_ name: String, _ draw: (CGContext) -> Void) {
        let c = context(1024, 1024)
        c.scaleBy(x: 1024 / 824, y: 1024 / 824)
        c.translateBy(x: -100, y: -100)
        draw(c)
        writePNG(c, "\(assetsDir)/\(name).png")
    }
    layer("background") { drawBackground($0) }
    layer("glyph") { drawGlyph($0, small: false, shadows: false) }
    let json = """
    {
      "groups" : [
        {
          "layers" : [ { "image-name" : "glyph.png", "name" : "glyph" } ],
          "shadow" : { "kind" : "neutral", "opacity" : 0.5 },
          "translucency" : { "enabled" : false, "value" : 0.5 }
        },
        {
          "layers" : [ { "image-name" : "background.png", "name" : "background" } ],
          "shadow" : { "kind" : "none", "opacity" : 0.5 },
          "translucency" : { "enabled" : false, "value" : 0.5 }
        }
      ],
      "supported-platforms" : { "squares" : [ "macOS" ] }
    }
    """
    try! json.write(toFile: "\(dir)/icon.json", atomically: true, encoding: .utf8)
}

// MARK: - Banner

func registerFonts(_ dir: String) {
    for f in ["Outfit-Bold.ttf", "Outfit-Regular.ttf", "GeistMono-Regular.ttf"] {
        CTFontManagerRegisterFontsForURL(URL(fileURLWithPath: "\(dir)/\(f)") as CFURL, .process, nil)
    }
}

func drawText(_ c: CGContext, _ s: String, font: String, size: CGFloat, color: CGColor, at p: CGPoint, kern: CGFloat = 0) -> CGFloat {
    let attrs: [NSAttributedString.Key: Any] = [.font: CTFontCreateWithName(font as CFString, size, nil), .foregroundColor: color, .kern: kern]
    let line = CTLineCreateWithAttributedString(NSAttributedString(string: s, attributes: attrs))
    c.textPosition = p
    CTLineDraw(line, c)
    return CTLineGetTypographicBounds(line, nil, nil, nil)
}

func drawBanner(_ c: CGContext, w: CGFloat, h: CGFloat) {
    // Ink background with a faint violet → cyan aurora on the left.
    c.setFillColor(ink); c.fill(CGRect(x: 0, y: 0, width: w, height: h))
    c.drawRadialGradient(gradient([(rgb(0x7C3AED, 0.55), 0), (rgb(0x7C3AED, 0), 1)]),
                         startCenter: CGPoint(x: 470, y: h / 2 + 60), startRadius: 0,
                         endCenter: CGPoint(x: 470, y: h / 2 + 60), endRadius: 700, options: [])
    c.drawRadialGradient(gradient([(rgb(0x22D3EE, 0.28), 0), (rgb(0x22D3EE, 0), 1)]),
                         startCenter: CGPoint(x: 700, y: h / 2 - 160), startRadius: 0,
                         endCenter: CGPoint(x: 700, y: h / 2 - 160), endRadius: 600, options: [])

    // Measurement dot grid, fading out to the right: the "selection" field the icon lives on.
    let step: CGFloat = 40
    var y = step
    while y < h - step / 2 {
        var x = step
        while x < w - step / 2 {
            let fade = max(0, 1 - x / (w * 0.95))
            c.setFillColor(rgb(0xC4B5FD, 0.05 + 0.13 * fade))
            c.fillEllipse(in: CGRect(x: x - 1.6, y: y - 1.6, width: 3.2, height: 3.2))
            x += step
        }
        y += step
    }

    // Icon, sitting on the grid.
    let iconSize: CGFloat = 600
    c.saveGState()
    c.translateBy(x: 180, y: (h - iconSize) / 2)
    drawIcon(c, px: Int(iconSize))
    c.restoreGState()

    // Wordmark and tagline, aligned to the grid.
    let textX: CGFloat = 880
    _ = drawText(c, "Snapline", font: "Outfit-Bold", size: 196, color: white, at: CGPoint(x: textX, y: h / 2 - 8), kern: -3)
    _ = drawText(c, "Capture.  Mark up.  Hide what matters.", font: "Outfit-Regular", size: 56,
                 color: rgb(0xD8CCFF, 0.92), at: CGPoint(x: textX + 6, y: h / 2 - 110))

    // Small clinical label, bottom right.
    let label = "NATIVE APPLE SILICON  ·  ON-DEVICE PRIVACY  ·  macOS 15+"
    let labelFont = CTFontCreateWithName("GeistMono-Regular" as CFString, 24, nil)
    let lw = CTLineGetTypographicBounds(CTLineCreateWithAttributedString(NSAttributedString(string: label, attributes: [.font: labelFont, .kern: 3])), nil, nil, nil)
    _ = drawText(c, label, font: "GeistMono-Regular", size: 24, color: rgb(0xFFFFFF, 0.42),
                 at: CGPoint(x: w - 120 - lw, y: 96), kern: 3)
    // Selection tick marks framing the label, echoing the icon's corners.
    c.setStrokeColor(rgb(0x22D3EE, 0.7)); c.setLineWidth(3); c.setLineCap(.round)
    let lx = w - 120 - lw - 28, rx = w - 120 + 22, by: CGFloat = 80, ty: CGFloat = 136
    for (x, dx) in [(lx, 1.0), (rx, -1.0)] {
        c.move(to: CGPoint(x: x + dx * 14, y: ty)); c.addLine(to: CGPoint(x: x, y: ty)); c.addLine(to: CGPoint(x: x, y: ty - 12))
        c.move(to: CGPoint(x: x + dx * 14, y: by)); c.addLine(to: CGPoint(x: x, y: by)); c.addLine(to: CGPoint(x: x, y: by + 12))
    }
    c.strokePath()
}

// MARK: - Main

let root = FileManager.default.currentDirectoryPath
let assets = "\(root)/Assets"
writeIconDocument("\(assets)/AppIcon.icon")
let big = context(1024, 1024)
drawIcon(big, px: 1024)
writePNG(big, "\(assets)/icon-1024.png")

registerFonts("\(root)/scripts/brand/fonts")
let (bw, bh) = (2560, 800)
let banner = context(bw, bh)
drawBanner(banner, w: CGFloat(bw), h: CGFloat(bh))
writePNG(banner, "\(assets)/banner.png")
print("Wrote Assets/AppIcon.icon, Assets/icon-1024.png, Assets/banner.png")
