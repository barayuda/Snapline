import AppKit
import Carbon.HIToolbox
import SnaplineCore

extension OverlayView {
    // MARK: - Toolbars

    func buildToolbars() {
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

    func layoutToolbars() {
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
}
