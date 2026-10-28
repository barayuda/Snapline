import AppKit

/// Snapline brand: colours and the menu bar glyph. Palette matches scripts/brand/make-brand.swift.
enum Brand {
    static let tagline = "Capture. Mark up. Hide what matters."

    /// Shown in the About panel; the README "Privacy" section says the same. Keep both true:
    /// check every network call (Uploaders.swift, GoogleAuth.swift) before changing them.
    static let privacyNotice = """
        Private by design: capture, drawing, auto-hide and text recognition run entirely on this Mac. \
        Snapline has no servers and no analytics, and no third party receives your screenshots to read or analyse them. \
        An image leaves your Mac only when you choose Upload, Search or Email, and only to the place you chose.
        """

    static let violet = NSColor(srgbRed: 0x7C / 255, green: 0x3A / 255, blue: 0xED / 255, alpha: 1)
    static let violetLight = NSColor(srgbRed: 0x8B / 255, green: 0x5C / 255, blue: 0xF6 / 255, alpha: 1)
    static let cyan = NSColor(srgbRed: 0x22 / 255, green: 0xD3 / 255, blue: 0xEE / 255, alpha: 1)
    static let ink = NSColor(srgbRed: 0x0C / 255, green: 0x08 / 255, blue: 0x19 / 255, alpha: 1)

    static var version: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "dev"
    }

    /// The icon's corners and spark as an 18-pt template image, so macOS tints it for light and dark menu bars.
    static let menuBarImage: NSImage = {
        let image = NSImage(size: NSSize(width: 18, height: 18), flipped: false) { rect in
            let box = rect.insetBy(dx: 2, dy: 2)
            let arm: CGFloat = 4.5
            let corners = NSBezierPath()
            for (x, y, dx, dy) in [(box.minX, box.maxY, 1.0, -1.0), (box.maxX, box.maxY, -1.0, -1.0),
                                   (box.maxX, box.minY, -1.0, 1.0), (box.minX, box.minY, 1.0, 1.0)] {
                corners.move(to: NSPoint(x: x, y: y + dy * arm))
                corners.line(to: NSPoint(x: x, y: y))
                corners.line(to: NSPoint(x: x + dx * arm, y: y))
            }
            corners.lineWidth = 1.7
            corners.lineCapStyle = .round
            corners.lineJoinStyle = .round
            NSColor.black.setStroke()
            corners.stroke()

            let c = NSPoint(x: rect.midX, y: rect.midY), r: CGFloat = 4, k = r * 0.14
            let spark = NSBezierPath()
            spark.move(to: NSPoint(x: c.x, y: c.y + r))
            spark.curve(to: NSPoint(x: c.x + r, y: c.y), controlPoint1: NSPoint(x: c.x + k, y: c.y + k), controlPoint2: NSPoint(x: c.x + k, y: c.y + k))
            spark.curve(to: NSPoint(x: c.x, y: c.y - r), controlPoint1: NSPoint(x: c.x + k, y: c.y - k), controlPoint2: NSPoint(x: c.x + k, y: c.y - k))
            spark.curve(to: NSPoint(x: c.x - r, y: c.y), controlPoint1: NSPoint(x: c.x - k, y: c.y - k), controlPoint2: NSPoint(x: c.x - k, y: c.y - k))
            spark.curve(to: NSPoint(x: c.x, y: c.y + r), controlPoint1: NSPoint(x: c.x - k, y: c.y + k), controlPoint2: NSPoint(x: c.x - k, y: c.y + k))
            NSColor.black.setFill()
            spark.fill()
            return true
        }
        image.isTemplate = true
        image.accessibilityDescription = "Snapline"
        return image
    }()
}
