import CoreGraphics

/// Geometry for the selection overlay. All view rects use a top-left origin (flipped view).
public enum SelectionGeometry {
    public enum Handle: CaseIterable, Sendable {
        case topLeft, top, topRight, right, bottomRight, bottom, bottomLeft, left
    }

    public static func rect(from a: CGPoint, to b: CGPoint) -> CGRect {
        CGRect(x: min(a.x, b.x), y: min(a.y, b.y), width: abs(a.x - b.x), height: abs(a.y - b.y))
    }

    /// Vision bounding box (normalized, bottom-left origin) → view rect in points (top-left origin).
    public static func viewRect(fromNormalized box: CGRect, viewSize: CGSize) -> CGRect {
        CGRect(x: box.minX * viewSize.width,
               y: (1 - box.maxY) * viewSize.height,
               width: box.width * viewSize.width,
               height: box.height * viewSize.height)
    }

    /// Part of a word's box that holds `characters`, assuming equal character widths,
    /// padded by half a character on each side because real glyph widths vary.
    public static func slice(_ box: CGRect, characters: Range<Int>, of count: Int) -> CGRect {
        guard count > 0 else { return box }
        let unit = box.width / CGFloat(count)
        let minX = max(box.minX, box.minX + CGFloat(characters.lowerBound) * unit - unit / 2)
        let maxX = min(box.maxX, box.minX + CGFloat(characters.upperBound) * unit + unit / 2)
        return CGRect(x: minX, y: box.minY, width: maxX - minX, height: box.height)
    }

    public static func move(_ rect: CGRect, by d: CGVector, within bounds: CGRect) -> CGRect {
        var r = rect.offsetBy(dx: d.dx, dy: d.dy)
        r.origin.x = min(max(r.minX, bounds.minX), bounds.maxX - r.width)
        r.origin.y = min(max(r.minY, bounds.minY), bounds.maxY - r.height)
        return r
    }

    public static func handlePoint(_ h: Handle, of r: CGRect) -> CGPoint {
        switch h {
        case .topLeft: CGPoint(x: r.minX, y: r.minY)
        case .top: CGPoint(x: r.midX, y: r.minY)
        case .topRight: CGPoint(x: r.maxX, y: r.minY)
        case .right: CGPoint(x: r.maxX, y: r.midY)
        case .bottomRight: CGPoint(x: r.maxX, y: r.maxY)
        case .bottom: CGPoint(x: r.midX, y: r.maxY)
        case .bottomLeft: CGPoint(x: r.minX, y: r.maxY)
        case .left: CGPoint(x: r.minX, y: r.midY)
        }
    }

    public static func handle(at p: CGPoint, in r: CGRect, tolerance: CGFloat) -> Handle? {
        Handle.allCases.first {
            let h = handlePoint($0, of: r)
            return abs(h.x - p.x) <= tolerance && abs(h.y - p.y) <= tolerance
        }
    }

    /// Moves the edges a handle controls to `p`; the opposite edges stay put.
    public static func resize(_ r: CGRect, handle: Handle, to p: CGPoint) -> CGRect {
        var (x1, y1, x2, y2) = (r.minX, r.minY, r.maxX, r.maxY)
        switch handle {
        case .topLeft: (x1, y1) = (p.x, p.y)
        case .top: y1 = p.y
        case .topRight: (x2, y1) = (p.x, p.y)
        case .right: x2 = p.x
        case .bottomRight: (x2, y2) = (p.x, p.y)
        case .bottom: y2 = p.y
        case .bottomLeft: (x1, y2) = (p.x, p.y)
        case .left: x1 = p.x
        }
        return rect(from: CGPoint(x: x1, y: y1), to: CGPoint(x: x2, y: y2))
    }
}
