import AppKit
import Carbon.HIToolbox
import SnaplineCore

extension OverlayView {
    // MARK: - Mouse

    func point(_ e: NSEvent) -> CGPoint {
        let p = convert(e.locationInWindow, from: nil)
        return CGPoint(x: min(max(p.x, 0), bounds.maxX), y: min(max(p.y, 0), bounds.maxY))
    }

    override func mouseDown(with e: NSEvent) {
        window?.makeKey()
        window?.makeFirstResponder(self)
        commitText()
        let p = point(e)
        mouseDownPoint = p
        didDrag = false

        if let sel = selection, sel.width > 2 {
            if session.tool == nil, let h = SelectionGeometry.handle(at: p, in: sel, tolerance: 6) {
                drag = .resizing(h, sel)
            } else if sel.contains(p) {
                switch session.tool {
                case nil:
                    drag = .moving(p, sel)
                case .text?:
                    if let i = textIndex(at: p) { drag = .movingText(i, p) } else { beginText(at: p) }
                case let tool?:
                    current = Annotation.begin(tool, at: p, color: session.color, width: session.width)
                    drag = .drawing(p)
                }
            } else if session.tool == nil {
                startSelection(at: p)
            }
        } else {
            startSelection(at: p)
        }
        layoutToolbars()
        needsDisplay = true
    }

    private func startSelection(at p: CGPoint) {
        session.clearSelections(except: self)
        selection = CGRect(origin: p, size: .zero)
        drag = .selecting(p)
    }

    override func mouseDragged(with e: NSEvent) {
        let p = point(e)
        if hypot(p.x - mouseDownPoint.x, p.y - mouseDownPoint.y) > 2 { didDrag = true }
        switch drag {
        case let .selecting(start):
            selection = SelectionGeometry.rect(from: start, to: p)
        case let .moving(start, original):
            selection = SelectionGeometry.move(original, by: CGVector(dx: p.x - start.x, dy: p.y - start.y), within: bounds)
        case let .resizing(handle, original):
            selection = SelectionGeometry.resize(original, handle: handle, to: p)
        case let .drawing(start):
            current?.extend(from: start, to: p)
        case let .movingText(i, last):
            if case let .text(origin, string, size) = annotations[i].shape {
                annotations[i].shape = .text(CGPoint(x: origin.x + p.x - last.x, y: origin.y + p.y - last.y), string, size)
            }
            drag = .movingText(i, p)
        case .none:
            break
        }
        layoutToolbars()
        needsDisplay = true
    }

    override func mouseUp(with e: NSEvent) {
        let p = point(e)
        switch drag {
        case .selecting:
            if !didDrag { selection = nil }
        case .moving:
            if !didDrag, let (i, _) = visibleHideBoxes.last(where: { $0.1.rect.contains(p) }) {
                hideBoxes[i].hidden.toggle()
                undoStack.append(.toggle(i))
            }
        case .drawing:
            if let c = current, didDrag || isPath(c) {
                annotations.append(c)
                undoStack.append(.annotation)
            }
            current = nil
        default:
            break
        }
        drag = .none
        layoutToolbars()
        updateCursor(at: p)
        needsDisplay = true
    }

    private func isPath(_ a: Annotation) -> Bool {
        if case .path = a.shape { return true }
        return false
    }

    override func mouseMoved(with e: NSEvent) { updateCursor(at: point(e)) }
    override func cursorUpdate(with event: NSEvent) { updateCursor(at: point(event)) }

    private func updateCursor(at p: CGPoint) {
        guard textField == nil else { return }
        guard let sel = selection, sel.width > 2 else { return NSCursor.crosshair.set() }
        if session.tool == nil, let h = SelectionGeometry.handle(at: p, in: sel, tolerance: 6) {
            switch h {
            case .left, .right: NSCursor.resizeLeftRight.set()
            case .top, .bottom: NSCursor.resizeUpDown.set()
            default: NSCursor.crosshair.set()
            }
        } else if sel.contains(p) {
            switch session.tool {
            case nil: (visibleHideBoxes.contains { $0.1.rect.contains(p) } ? NSCursor.pointingHand : NSCursor.openHand).set()
            case .text?: (textIndex(at: p) != nil ? NSCursor.openHand : NSCursor.iBeam).set()
            default: NSCursor.crosshair.set()
            }
        } else {
            (session.tool == nil ? NSCursor.crosshair : NSCursor.arrow).set()
        }
    }

    override func scrollWheel(with e: NSEvent) {
        let step: CGFloat = e.scrollingDeltaY > 0 ? 1 : e.scrollingDeltaY < 0 ? -1 : 0
        guard step != 0 else { return }
        if let f = textField {
            fontSize = min(max(fontSize + step * 2, 8), 120)
            f.font = NSFont.systemFont(ofSize: fontSize, weight: .semibold)
            resizeTextField()
        } else if session.tool == .text, let i = textIndex(at: point(e)), case let .text(o, s, size) = annotations[i].shape {
            annotations[i].shape = .text(o, s, min(max(size + step * 2, 8), 120))
            needsDisplay = true
        } else {
            session.width = min(max(session.width + step, 1), 20)
        }
    }
}
