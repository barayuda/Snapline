import AppKit
import Carbon.HIToolbox
import SnaplineCore

extension OverlayView {
    // MARK: - Keyboard

    // The hidden Edit menu sends ⌘C, ⌘A and ⌘Z as standard actions before keyDown sees them;
    // in the capture they mean Copy image, Select whole screen and Undo.
    @objc func copy(_ sender: Any?) { session.perform(.copy) }
    override func selectAll(_ sender: Any?) { selectWholeScreen() }
    @objc func undo(_ sender: Any?) { undo() }

    override func keyDown(with e: NSEvent) {
        let chars = e.charactersIgnoringModifiers?.lowercased() ?? ""
        let flags = e.modifierFlags
        switch Int(e.keyCode) {
        case kVK_Escape: session.cancel()
        case kVK_LeftArrow: nudge(dx: -1, dy: 0, flags)
        case kVK_RightArrow: nudge(dx: 1, dy: 0, flags)
        case kVK_DownArrow: nudge(dx: 0, dy: 1, flags)
        case kVK_UpArrow: nudge(dx: 0, dy: -1, flags)
        case kVK_Return, kVK_ANSI_KeypadEnter: session.perform(.copy)   // Return copies, like Lightshot's Enter
        default:
            if flags.contains(.command) {
                switch chars {
                case "c": session.perform(flags.contains(.shift) ? .copyText : .copy)
                case "s": session.perform(flags.contains(.option) ? .saveAs : .save)
                case "d": session.perform(.upload)
                case "p": session.perform(.print)
                case "e": session.perform(.edit)
                case "z": undo()
                case "a": selectWholeScreen()
                default: super.keyDown(with: e)
                }
            } else if chars == "+" || chars == "=" {
                session.width = min(session.width + 1, 20)
            } else if chars == "-" {
                session.width = max(session.width - 1, 1)
            } else if let tool = Tool.forKey(chars) {
                session.tool = session.tool == tool ? nil : tool
            } else if chars == "v" {
                session.tool = nil
            } else {
                super.keyDown(with: e)
            }
        }
    }

    /// Arrows move the selection; ⌥ resizes it; ⇧ uses 10-point steps.
    private func nudge(dx: CGFloat, dy: CGFloat, _ flags: NSEvent.ModifierFlags) {
        guard var sel = selection else { return }
        let step: CGFloat = flags.contains(.shift) ? 10 : 1
        if flags.contains(.option) {
            sel.size.width = min(max(sel.width + dx * step, 1), bounds.maxX - sel.minX)
            sel.size.height = min(max(sel.height + dy * step, 1), bounds.maxY - sel.minY)
        } else {
            sel = SelectionGeometry.move(sel, by: CGVector(dx: dx * step, dy: dy * step), within: bounds)
        }
        selection = sel
        layoutToolbars()
        needsDisplay = true
    }

    func undo() {
        guard let last = undoStack.popLast() else { return NSSound.beep() }
        switch last {
        case .annotation: annotations.removeLast()
        case let .toggle(i): hideBoxes[i].hidden.toggle()
        }
        needsDisplay = true
    }
}
