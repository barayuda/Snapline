import AppKit
import Carbon.HIToolbox
import SnaplineCore

extension OverlayView {
    // MARK: - Drawing

    var visibleHideBoxes: [(Int, HideBox)] {
        guard session.autoHideActive, let sel = selection else { return [] }
        return hideBoxes.enumerated().filter { $0.element.rect.intersects(sel) }.map { ($0.offset, $0.element) }
    }

    /// Image content without overlay chrome. Used for the screen and for export.
    private func drawContent(export: Bool) {
        if export {
            NSImage(cgImage: shot.image, size: bounds.size)
                .draw(in: bounds, from: .zero, operation: .copy, fraction: 1, respectFlipped: true, hints: nil)
        }
        for (_, box) in visibleHideBoxes {
            if box.hidden {
                NSColor.black.setFill()
                box.rect.fill()
            } else if !export {
                let p = NSBezierPath(rect: box.rect)
                p.setLineDash([4, 3], count: 2, phase: 0)
                NSColor.systemYellow.setStroke()
                p.stroke()
            }
        }
        annotations.forEach { $0.draw() }
        current?.draw()
    }

    override func draw(_ dirtyRect: NSRect) {
        drawContent(export: false)

        let dim = NSBezierPath(rect: bounds)
        if let sel = selection {
            dim.appendRect(sel)
            dim.windingRule = .evenOdd
        }
        NSColor.black.withAlphaComponent(0.45).setFill()
        dim.fill()

        guard let sel = selection else {
            if window?.isKeyWindow == true {
                badge("Drag to select an area   ·   ⌘A whole screen   ·   Esc to cancel",
                      at: CGPoint(x: bounds.midX, y: bounds.midY), centered: true)
            }
            return
        }

        // Brand frame: a cyan hairline with a soft violet glow outside it.
        let glow = NSBezierPath(rect: sel.insetBy(dx: -2, dy: -2))
        glow.lineWidth = 3
        Brand.violet.withAlphaComponent(0.55).setStroke()
        glow.stroke()
        let border = NSBezierPath(rect: sel.insetBy(dx: -0.75, dy: -0.75))
        border.lineWidth = 1.5
        Brand.cyan.setStroke()
        border.stroke()

        if session.tool == nil, sel.width > 2 {
            for h in SelectionGeometry.Handle.allCases {
                let c = SelectionGeometry.handlePoint(h, of: sel)
                let dot = NSBezierPath(ovalIn: CGRect(x: c.x - 4.5, y: c.y - 4.5, width: 9, height: 9))
                NSColor.white.setFill(); dot.fill()
                dot.lineWidth = 2
                Brand.violet.setStroke(); dot.stroke()
            }
        }

        var label = "\(Int((sel.width * outputScale).rounded())) × \(Int((sel.height * outputScale).rounded()))"
        if session.autoHideActive && !Settings.autoHideCategories.isEmpty {
            let hidden = visibleHideBoxes.filter { $0.1.hidden }.count
            label += isScanning ? "   ·   scanning…" : hidden > 0 ? "   ·   hiding \(hidden)" : ""
        }
        badge(label, at: CGPoint(x: sel.minX, y: max(sel.minY - 26, 2)), centered: false, fill: Brand.violet)
    }

    private func badge(_ text: String, at p: CGPoint, centered: Bool, fill: NSColor = Brand.ink) {
        let attrs: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 12, weight: .medium), .foregroundColor: NSColor.white]
        let size = (text as NSString).size(withAttributes: attrs)
        var r = CGRect(x: p.x, y: p.y, width: size.width + 16, height: size.height + 6)
        if centered { r.origin = CGPoint(x: p.x - r.width / 2, y: p.y - r.height / 2) }
        fill.withAlphaComponent(0.88).setFill()
        NSBezierPath(roundedRect: r, xRadius: r.height / 2, yRadius: r.height / 2).fill()
        (text as NSString).draw(at: CGPoint(x: r.minX + 8, y: r.minY + 3), withAttributes: attrs)
    }

    private var outputScale: CGFloat { UserDefaults.standard.bool(forKey: Key.downscaleRetina) ? 1 : scale }

    /// Returns once the auto-hide scan has found everything it will hide.
    func scanFinished() async { await scan?.value }

    /// Flattens image, hide boxes and annotations of the selection into one bitmap.
    func renderSelection() -> CGImage? {
        guard let sel = selection, sel.width >= 1, sel.height >= 1 else { return nil }
        let s = outputScale
        let w = Int((sel.width * s).rounded()), h = Int((sel.height * s).rounded())
        guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: shot.image.colorSpace ?? CGColorSpace(name: CGColorSpace.sRGB)!,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        ctx.interpolationQuality = s == scale ? .none : .high
        ctx.translateBy(x: 0, y: CGFloat(h))
        ctx.scaleBy(x: s, y: -s)
        ctx.translateBy(x: -sel.minX, y: -sel.minY)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: true)
        drawContent(export: true)
        NSGraphicsContext.restoreGraphicsState()
        return ctx.makeImage()
    }
}
