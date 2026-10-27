import AppKit
import SnaplineCore

enum Tool: CaseIterable {
    case pencil, line, arrow, rectangle, ellipse, marker, text, hide

    var symbol: String {
        switch self {
        case .pencil: "pencil"
        case .line: "line.diagonal"
        case .arrow: "arrow.up.right"
        case .rectangle: "rectangle"
        case .ellipse: "circle"
        case .marker: "highlighter"
        case .text: "textformat"
        case .hide: "rectangle.fill"
        }
    }

    var title: String {
        switch self {
        case .pencil: "Pencil (P)"
        case .line: "Line (L)"
        case .arrow: "Arrow (A)"
        case .rectangle: "Rectangle (R)"
        case .ellipse: "Ellipse (E)"
        case .marker: "Marker (M)"
        case .text: "Text (T)"
        case .hide: "Hide area (H)"
        }
    }

    static func forKey(_ c: String) -> Tool? {
        ["p": .pencil, "l": .line, "a": .arrow, "r": .rectangle, "e": .ellipse, "m": .marker, "t": .text, "h": .hide][c]
    }
}

/// One drawn shape, in view points (top-left origin).
struct Annotation {
    enum Shape {
        case path([CGPoint])                 // pencil, marker
        case line(CGPoint, CGPoint)
        case arrow(CGPoint, CGPoint)
        case rectangle(CGRect)
        case ellipse(CGRect)
        case hide(CGRect)
        case text(CGPoint, String, CGFloat)  // origin, text, font size
    }

    var shape: Shape
    var color: NSColor
    var width: CGFloat
    var isMarker = false

    static func textAttributes(size: CGFloat, color: NSColor) -> [NSAttributedString.Key: Any] {
        [.font: NSFont.systemFont(ofSize: size, weight: .semibold), .foregroundColor: color]
    }

    var textBounds: CGRect? {
        guard case let .text(origin, string, size) = shape else { return nil }
        return CGRect(origin: origin, size: (string as NSString).size(withAttributes: Self.textAttributes(size: size, color: color)))
    }

    /// Draws into the current (flipped) graphics context.
    func draw() {
        color.setStroke()
        color.setFill()
        switch shape {
        case let .path(points):
            guard let first = points.first else { return }
            let p = NSBezierPath()
            p.move(to: first)
            points.dropFirst().forEach { p.line(to: $0) }
            if points.count == 1 { p.line(to: first) }
            p.lineCapStyle = isMarker ? .square : .round
            p.lineJoinStyle = .round
            p.lineWidth = isMarker ? width * 5 : width
            if isMarker { color.withAlphaComponent(0.4).setStroke() }
            p.stroke()
        case let .line(a, b):
            stroke { $0.move(to: a); $0.line(to: b) }
        case let .arrow(a, b):
            let angle = atan2(b.y - a.y, b.x - a.x)
            let head = max(12, width * 4)
            let base = CGPoint(x: b.x - cos(angle) * head * 0.8, y: b.y - sin(angle) * head * 0.8)
            stroke { $0.move(to: a); $0.line(to: base) }
            let tri = NSBezierPath()
            tri.move(to: b)
            tri.line(to: CGPoint(x: b.x - head * cos(angle - .pi / 7), y: b.y - head * sin(angle - .pi / 7)))
            tri.line(to: CGPoint(x: b.x - head * cos(angle + .pi / 7), y: b.y - head * sin(angle + .pi / 7)))
            tri.close()
            tri.fill()
        case let .rectangle(r):
            stroke { $0.appendRect(r) }
        case let .ellipse(r):
            stroke { $0.appendOval(in: r) }
        case let .hide(r):
            NSColor.black.setFill()
            r.fill()
        case let .text(origin, string, size):
            (string as NSString).draw(at: origin, withAttributes: Self.textAttributes(size: size, color: color))
        }
    }

    private func stroke(_ build: (NSBezierPath) -> Void) {
        let p = NSBezierPath()
        build(p)
        p.lineWidth = width
        p.lineCapStyle = .round
        p.lineJoinStyle = .round
        p.stroke()
    }

    /// Updates the shape while the mouse is dragged from `start` to `p`.
    mutating func extend(from start: CGPoint, to p: CGPoint) {
        switch shape {
        case var .path(points): points.append(p); shape = .path(points)
        case .line: shape = .line(start, p)
        case .arrow: shape = .arrow(start, p)
        case .rectangle: shape = .rectangle(SelectionGeometry.rect(from: start, to: p))
        case .ellipse: shape = .ellipse(SelectionGeometry.rect(from: start, to: p))
        case .hide: shape = .hide(SelectionGeometry.rect(from: start, to: p))
        case .text: break
        }
    }

    static func begin(_ tool: Tool, at p: CGPoint, color: NSColor, width: CGFloat) -> Annotation? {
        let shape: Shape
        switch tool {
        case .pencil, .marker: shape = .path([p])
        case .line: shape = .line(p, p)
        case .arrow: shape = .arrow(p, p)
        case .rectangle: shape = .rectangle(CGRect(origin: p, size: .zero))
        case .ellipse: shape = .ellipse(CGRect(origin: p, size: .zero))
        case .hide: shape = .hide(CGRect(origin: p, size: .zero))
        case .text: return nil
        }
        return Annotation(shape: shape, color: color, width: width, isMarker: tool == .marker)
    }
}

/// An area found by the auto-hide scan.
struct HideBox: Sendable {
    var rect: CGRect
    let category: SensitiveCategory
    var hidden = true
}
