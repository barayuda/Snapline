import AppKit
import Carbon.HIToolbox
import SnaplineCore

extension OverlayView {
    // MARK: - Text

    func textIndex(at p: CGPoint) -> Int? {
        annotations.indices.last { annotations[$0].textBounds?.insetBy(dx: -4, dy: -4).contains(p) == true }
    }

    func beginText(at p: CGPoint) {
        let f = NSTextField(string: "")
        f.font = NSFont.systemFont(ofSize: fontSize, weight: .semibold)
        f.textColor = session.color
        f.isBordered = false
        f.drawsBackground = false
        f.focusRingType = .none
        f.cell?.wraps = false
        f.cell?.isScrollable = true
        f.delegate = self
        f.frame = CGRect(x: p.x - 2, y: p.y - fontSize * 0.6, width: 40, height: fontSize * 1.3)
        addSubview(f)
        textField = f
        window?.makeFirstResponder(f)
        NSCursor.iBeam.set()
    }

    func resizeTextField() {
        guard let f = textField else { return }
        let size = (f.stringValue.isEmpty ? "M" : f.stringValue as NSString)
            .size(withAttributes: Annotation.textAttributes(size: fontSize, color: session.color))
        f.setFrameSize(CGSize(width: size.width + 12, height: size.height + 2))
    }

    func controlTextDidChange(_ obj: Notification) { resizeTextField() }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        if selector == #selector(NSResponder.insertNewline(_:)) || selector == #selector(NSResponder.cancelOperation(_:)) {
            commitText()
            return true
        }
        return false
    }

    func commitText() {
        guard let f = textField else { return }
        textField = nil
        let text = f.stringValue.trimmingCharacters(in: .whitespaces)
        if !text.isEmpty {
            let origin = CGPoint(x: f.frame.minX + 2, y: f.frame.minY + 1)
            annotations.append(Annotation(shape: .text(origin, text, fontSize), color: session.color, width: 0))
            undoStack.append(.annotation)
        }
        f.removeFromSuperview()
        window?.makeFirstResponder(self)
        needsDisplay = true
    }

    func colorDidChange() {
        textField?.textColor = session.color
        refreshToolbarState()
    }
}
