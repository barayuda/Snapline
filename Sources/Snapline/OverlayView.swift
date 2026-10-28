import AppKit
import SnaplineCore

/// Full-screen view on top of the frozen screen image of one display:
/// selection, drawing, auto-hide boxes, keyboard and the two toolbars.
@MainActor
final class OverlayView: NSView, NSTextFieldDelegate {
    private unowned let session: CaptureSession
    let shot: ScreenShot
    let scale: CGFloat

    private(set) var selection: CGRect?
    private var annotations: [Annotation] = []
    private var hideBoxes: [HideBox] = []
    private var scanning = true
    private var scan: Task<Void, Never>?

    private enum Undo { case annotation, toggle(Int) }
    private var undoStack: [Undo] = []

    private enum Drag {
        case none
        case selecting(CGPoint)
        case moving(CGPoint, CGRect)
        case resizing(SelectionGeometry.Handle, CGRect)
        case drawing(CGPoint)
        case movingText(Int, CGPoint)
    }
    private var drag = Drag.none
    private var current: Annotation?
    private var mouseDownPoint = CGPoint.zero
    private var didDrag = false

    private var textField: NSTextField?
    private var fontSize: CGFloat = 22

    private var toolButtons: [NSButton] = []
    private var colorButton: NSButton!
    private var autoHideButton: NSButton!
    private var drawBar: Toolbar!
    private var mainBar: Toolbar!

    init(session: CaptureSession, shot: ScreenShot) {
        self.session = session
        self.shot = shot
        self.scale = CGFloat(shot.image.width) / shot.screen.frame.width
        super.init(frame: CGRect(origin: .zero, size: shot.screen.frame.size))
        buildToolbars()
        addTrackingArea(NSTrackingArea(rect: .zero, options: [.mouseMoved, .activeAlways, .inVisibleRect, .cursorUpdate], owner: self))

        let viewSize = bounds.size
        scan = Task {
            hideBoxes = await Redaction.scan(shot.image, viewSize: viewSize, categories: Settings.autoHideCategories)
            scanning = false
            needsDisplay = true
        }
    }

    @available(*, unavailable) required init?(coder: NSCoder) { fatalError() }

    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    // MARK: - Toolbars

    private func buildToolbars() {
        toolButtons = Tool.allCases.enumerated().map { i, tool in
            Toolbar.button(tool.symbol, tool.title, self, #selector(toolClicked(_:)), tag: i)
        }
        colorButton = Toolbar.button("circle.fill", "Color", self, #selector(colorClicked))
        colorButton.setButtonType(.momentaryPushIn)
        autoHideButton = Toolbar.button("eye.slash", "Auto-hide sensitive data (click a black box to show it)", self, #selector(autoHideClicked))
        let undo = Toolbar.button("arrow.uturn.backward", "Undo (⌘Z)", self, #selector(undoClicked))
        undo.setButtonType(.momentaryPushIn)
        drawBar = Toolbar(buttons: toolButtons + [colorButton, autoHideButton, undo], vertical: true)

        let actions: [(String, String, CaptureSession.Action?)] = [
            ("icloud.and.arrow.up", "Upload and copy link (⌘D)", .upload),
            ("magnifyingglass", "Search similar images", nil),
            ("slider.horizontal.3", "Edit in external app (⌘E)", .edit),
            ("envelope", "Email", .email),
            ("printer", "Print (⌘P)", .print),
            ("doc.on.doc", "Copy image (⌘C)", .copy),
            ("text.viewfinder", "Copy text (⇧⌘C), read on this Mac", .copyText),
            ("square.and.arrow.down", "Save to default folder (⌘S) · ⌥-click or ⌥⌘S: Save As…", .save),
            ("xmark", "Close (Esc)", .cancel),
        ]
        let buttons = actions.enumerated().map { i, a in
            let b = Toolbar.button(a.0, a.1, self, #selector(actionClicked(_:)), tag: i)
            b.setButtonType(.momentaryPushIn)
            return b
        }
        mainActions = actions.map(\.2)
        mainBar = Toolbar(buttons: buttons, vertical: false)

        for bar in [drawBar!, mainBar!] {
            bar.isHidden = true
            addSubview(bar)
        }
        refreshToolbarState()
    }

    private var mainActions: [CaptureSession.Action?] = []

    func refreshToolbarState() {
        for (i, b) in toolButtons.enumerated() {
            b.state = session.tool == Tool.allCases[i] ? .on : .off
            b.contentTintColor = b.state == .on ? Brand.violetLight : .labelColor
        }
        colorButton.image = Toolbar.colorDot(session.color)
        autoHideButton.state = session.autoHideActive ? .on : .off
        autoHideButton.contentTintColor = session.autoHideActive ? Brand.violetLight : .labelColor
        needsDisplay = true
    }

    @objc private func toolClicked(_ sender: NSButton) {
        commitText()
        let tool = Tool.allCases[sender.tag]
        session.tool = session.tool == tool ? nil : tool
    }

    @objc private func colorClicked() { session.showColorPanel() }
    @objc private func undoClicked() { undo() }

    @objc private func autoHideClicked() {
        session.autoHideActive.toggle()
    }

    @objc private func actionClicked(_ sender: NSButton) {
        commitText()
        if var action = mainActions[sender.tag] {
            if case .save = action, NSEvent.modifierFlags.contains(.option) { action = .saveAs }
            session.perform(action)
            return
        }
        let menu = NSMenu()
        for engine in SearchEngine.allCases {
            let item = NSMenuItem(title: engine.title, action: #selector(searchPicked(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = engine.rawValue
            menu.addItem(item)
        }
        menu.popUp(positioning: nil, at: NSPoint(x: 0, y: sender.bounds.height + 4), in: sender)
    }

    @objc private func searchPicked(_ item: NSMenuItem) {
        guard let raw = item.representedObject as? String, let engine = SearchEngine(rawValue: raw) else { return }
        session.perform(.search(engine))
    }

    private func layoutToolbars() {
        let show: Bool
        if case .none = drag, let sel = selection, sel.width > 2, sel.height > 2 { show = true } else { show = false }
        drawBar.isHidden = !show
        mainBar.isHidden = !show
        guard show, let sel = selection else { return }
        let gap: CGFloat = 8

        var m = CGPoint(x: sel.maxX - mainBar.frame.width, y: sel.maxY + gap)
        if m.y + mainBar.frame.height > bounds.maxY {
            m.y = sel.minY - mainBar.frame.height - gap >= 0 ? sel.minY - mainBar.frame.height - gap : sel.maxY - mainBar.frame.height - gap
        }
        m.x = min(max(m.x, 0), bounds.maxX - mainBar.frame.width)
        mainBar.setFrameOrigin(m)

        var d = CGPoint(x: sel.maxX + gap, y: sel.minY)
        if d.x + drawBar.frame.width > bounds.maxX {
            d.x = sel.minX - drawBar.frame.width - gap >= 0 ? sel.minX - drawBar.frame.width - gap : sel.maxX - drawBar.frame.width - gap
        }
        d.y = min(max(d.y, 0), bounds.maxY - drawBar.frame.height)
        drawBar.setFrameOrigin(d)
    }

    // MARK: - Drawing

    private var visibleHideBoxes: [(Int, HideBox)] {
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
            label += scanning ? "   ·   scanning…" : hidden > 0 ? "   ·   hiding \(hidden)" : ""
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

    // MARK: - Selection API used by the session

    func clearSelection() {
        commitText()
        selection = nil
        layoutToolbars()
        needsDisplay = true
    }

    func selectWholeScreen() {
        session.clearSelections(except: self)
        selection = bounds
        layoutToolbars()
        needsDisplay = true
    }

    // MARK: - Mouse

    private func point(_ e: NSEvent) -> CGPoint {
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

    // MARK: - Keyboard

    override func keyDown(with e: NSEvent) {
        let chars = e.charactersIgnoringModifiers?.lowercased() ?? ""
        let flags = e.modifierFlags
        switch Int(e.keyCode) {
        case 53: session.cancel()                                  // Esc
        case 123: nudge(dx: -1, dy: 0, flags)                       // ←
        case 124: nudge(dx: 1, dy: 0, flags)                        // →
        case 125: nudge(dx: 0, dy: 1, flags)                        // ↓
        case 126: nudge(dx: 0, dy: -1, flags)                       // ↑
        case 36, 76: session.perform(.copy)                         // Return copies, like Lightshot's Enter
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

    private func undo() {
        guard let last = undoStack.popLast() else { return NSSound.beep() }
        switch last {
        case .annotation: annotations.removeLast()
        case let .toggle(i): hideBoxes[i].hidden.toggle()
        }
        needsDisplay = true
    }

    // MARK: - Text

    private func textIndex(at p: CGPoint) -> Int? {
        annotations.indices.last { annotations[$0].textBounds?.insetBy(dx: -4, dy: -4).contains(p) == true }
    }

    private func beginText(at p: CGPoint) {
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

    private func resizeTextField() {
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
