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

    /// What the menu bar icon shows besides its normal look.
    enum IconState: Equatable {
        case normal
        case progress(Double)   // 0...1, drawn as a ring around the spark
        case success            // ✓ in place of the spark
        case failure            // ! in place of the spark (tinted red by the caller)
    }

    static let menuBarImage = menuBarImage(.normal)

    /// The icon as an 18-pt template image, so macOS tints it for light and dark menu bars.
    static func menuBarImage(_ state: IconState) -> NSImage {
        let image = NSImage(size: NSSize(width: 18, height: 18), flipped: false) { rect in
            NSColor.black.setStroke()
            NSColor.black.setFill()
            let c = NSPoint(x: rect.midX, y: rect.midY)

            if case let .progress(value) = state {
                // Faint full track plus the done part, clockwise from the top.
                let track = NSBezierPath(ovalIn: rect.insetBy(dx: 1.6, dy: 1.6))
                track.lineWidth = 1.6
                NSColor.black.withAlphaComponent(0.3).setStroke()
                track.stroke()
                let arc = NSBezierPath()
                arc.appendArc(withCenter: c, radius: rect.width / 2 - 1.6, startAngle: 90,
                              endAngle: 90 - 360 * CGFloat(min(max(value, 0.02), 1)), clockwise: true)
                arc.lineWidth = 1.8
                arc.lineCapStyle = .round
                NSColor.black.setStroke()
                arc.stroke()
                spark(at: c, radius: 3.6).fill()
                return true
            }

            corners(in: rect.insetBy(dx: 2, dy: 2)).stroke()
            switch state {
            case .success:
                let check = NSBezierPath()
                check.move(to: NSPoint(x: c.x - 3, y: c.y))
                check.line(to: NSPoint(x: c.x - 0.8, y: c.y - 2.3))
                check.line(to: NSPoint(x: c.x + 3.2, y: c.y + 2.6))
                check.lineWidth = 1.8
                check.lineCapStyle = .round
                check.lineJoinStyle = .round
                check.stroke()
            case .failure:
                let bar = NSBezierPath(roundedRect: NSRect(x: c.x - 0.9, y: c.y - 0.6, width: 1.8, height: 4.6), xRadius: 0.9, yRadius: 0.9)
                bar.fill()
                NSBezierPath(ovalIn: NSRect(x: c.x - 1, y: c.y - 3.6, width: 2, height: 2)).fill()
            default:
                spark(at: c, radius: 4).fill()
            }
            return true
        }
        image.isTemplate = true
        image.accessibilityDescription = switch state {
        case .normal: "Snapline"
        case let .progress(v): "Snapline, uploading \(Int(v * 100)) percent"
        case .success: "Snapline, upload done"
        case .failure: "Snapline, upload failed"
        }
        return image
    }

    private static func corners(in box: NSRect) -> NSBezierPath {
        let arm: CGFloat = 4.5
        let p = NSBezierPath()
        for (x, y, dx, dy) in [(box.minX, box.maxY, 1.0, -1.0), (box.maxX, box.maxY, -1.0, -1.0),
                               (box.maxX, box.minY, -1.0, 1.0), (box.minX, box.minY, 1.0, 1.0)] {
            p.move(to: NSPoint(x: x, y: y + dy * arm))
            p.line(to: NSPoint(x: x, y: y))
            p.line(to: NSPoint(x: x + dx * arm, y: y))
        }
        p.lineWidth = 1.7
        p.lineCapStyle = .round
        p.lineJoinStyle = .round
        return p
    }

    private static func spark(at c: NSPoint, radius r: CGFloat) -> NSBezierPath {
        let k = r * 0.14
        let p = NSBezierPath()
        p.move(to: NSPoint(x: c.x, y: c.y + r))
        p.curve(to: NSPoint(x: c.x + r, y: c.y), controlPoint1: NSPoint(x: c.x + k, y: c.y + k), controlPoint2: NSPoint(x: c.x + k, y: c.y + k))
        p.curve(to: NSPoint(x: c.x, y: c.y - r), controlPoint1: NSPoint(x: c.x + k, y: c.y - k), controlPoint2: NSPoint(x: c.x + k, y: c.y - k))
        p.curve(to: NSPoint(x: c.x - r, y: c.y), controlPoint1: NSPoint(x: c.x - k, y: c.y - k), controlPoint2: NSPoint(x: c.x - k, y: c.y - k))
        p.curve(to: NSPoint(x: c.x, y: c.y + r), controlPoint1: NSPoint(x: c.x - k, y: c.y + k), controlPoint2: NSPoint(x: c.x - k, y: c.y + k))
        return p
    }
}
