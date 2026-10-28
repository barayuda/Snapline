import AppKit

enum SearchEngine: String, CaseIterable {
    case googleLens, tinEye

    var title: String {
        switch self {
        case .googleLens: "Search with Google Lens"
        case .tinEye: "Search with TinEye"
        }
    }

    func url(for image: URL) -> URL {
        let encoded = image.absoluteString.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? ""
        switch self {
        case .googleLens: return URL(string: "https://lens.google.com/uploadbyurl?url=\(encoded)")!
        case .tinEye: return URL(string: "https://tineye.com/search?url=\(encoded)")!
        }
    }
}

/// One capture: an overlay window per display plus the shared tool, color and width.
@MainActor
final class CaptureSession: NSObject {
    enum Action {
        case copy, copyText, save, saveAs, upload, print, edit, email, cancel
        case search(SearchEngine)
    }

    private var windows: [NSWindow] = []
    private var views: [OverlayView] = []
    private let onEnd: () -> Void

    var tool: Tool? { didSet { views.forEach { $0.refreshToolbarState() } } }
    var color = NSColor.systemRed { didSet { views.forEach { $0.colorDidChange() } } }
    var width: CGFloat = 3
    var autoHideActive = true { didSet { views.forEach { $0.refreshToolbarState() } } }

    /// Above the menu bar and Dock, but below pop-up menus so the search menu is visible.
    static let overlayLevel = NSWindow.Level(rawValue: NSWindow.Level.popUpMenu.rawValue - 1)

    init(shots: [ScreenShot], onEnd: @escaping () -> Void) {
        self.onEnd = onEnd
        super.init()
        for shot in shots {
            let window = OverlayWindow(screen: shot.screen)
            let base = NSView(frame: CGRect(origin: .zero, size: shot.screen.frame.size))
            base.wantsLayer = true
            base.layer?.contents = shot.image
            base.layer?.contentsGravity = .resize
            let view = OverlayView(session: self, shot: shot)
            view.autoresizingMask = [.width, .height]
            base.addSubview(view)
            window.contentView = base
            windows.append(window)
            views.append(view)
        }
    }

    func show() {
        NSApp.activate()
        windows.forEach { $0.orderFrontRegardless() }
        // Keyboard goes to the screen under the pointer; fall back to the main screen, then the first one.
        let mouse = NSEvent.mouseLocation
        let index = windows.firstIndex { NSMouseInRect(mouse, $0.frame, false) }
            ?? windows.firstIndex { $0.screen == NSScreen.main }
            ?? 0
        if windows.indices.contains(index) {
            windows[index].makeKeyAndOrderFront(nil)
            windows[index].makeFirstResponder(views[index])
        }
        NSCursor.crosshair.set()
    }

    func clearSelections(except keep: OverlayView) {
        views.filter { $0 !== keep }.forEach { $0.clearSelection() }
    }

    func cancel() { close() }

    private func close() {
        NSColorPanel.shared.orderOut(nil)
        windows.forEach { $0.orderOut(nil) }
        windows.removeAll()
        views.removeAll()
        NSCursor.arrow.set()
        onEnd()
    }

    func perform(_ action: Action) {
        if case .cancel = action { return close() }
        guard let view = views.first(where: { ($0.selection?.width ?? 0) >= 1 }), let image = view.renderSelection() else {
            return NSSound.beep()
        }
        close()
        Actions.run(action, image: image)
    }

    // MARK: - Color panel

    func showColorPanel() {
        let panel = NSColorPanel.shared
        panel.level = NSWindow.Level(rawValue: Self.overlayLevel.rawValue + 1)
        panel.color = color
        panel.showsAlpha = false
        panel.setTarget(self)
        panel.setAction(#selector(colorChanged(_:)))
        panel.orderFrontRegardless()
    }

    @objc private func colorChanged(_ panel: NSColorPanel) {
        color = panel.color
    }
}

/// A non-activating panel: it takes keyboard input (Esc, ⌘C…) as soon as it appears, even when
/// macOS declines to activate Snapline, e.g. after a click on the menu bar icon.
final class OverlayWindow: NSPanel {
    init(screen: NSScreen) {
        super.init(contentRect: screen.frame, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        becomesKeyOnlyIfNeeded = false
        hidesOnDeactivate = false
        level = CaptureSession.overlayLevel
        isOpaque = true
        hasShadow = false
        isReleasedWhenClosed = false
        acceptsMouseMovedEvents = true
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        setFrame(screen.frame, display: false)
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }
}
