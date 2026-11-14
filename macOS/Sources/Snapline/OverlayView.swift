import AppKit
import Carbon.HIToolbox
import SnaplineCore

/// Full-screen view on top of the frozen screen image of one display:
/// selection, drawing, auto-hide boxes, keyboard and the two toolbars.
@MainActor
final class OverlayView: NSView, NSTextFieldDelegate {
    unowned let session: CaptureSession
    let shot: ScreenShot
    let scale: CGFloat

    var selection: CGRect?
    var annotations: [Annotation] = []
    var hideBoxes: [HideBox] = []
    var isScanning = true
    var scan: Task<Void, Never>?

    enum Undo { case annotation, toggle(Int) }
    var undoStack: [Undo] = []

    enum Drag {
        case none
        case selecting(CGPoint)
        case moving(CGPoint, CGRect)
        case resizing(SelectionGeometry.Handle, CGRect)
        case drawing(CGPoint)
        case movingText(Int, CGPoint)
    }
    var drag = Drag.none
    var current: Annotation?
    var mouseDownPoint = CGPoint.zero
    var didDrag = false

    var textField: NSTextField?
    var fontSize: CGFloat = 22

    var toolButtons: [NSButton] = []
    var colorButton: NSButton!
    var autoHideButton: NSButton!
    var drawBar: Toolbar!
    var mainBar: Toolbar!

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
            isScanning = false
            needsDisplay = true
        }
    }

    @available(*, unavailable) required init?(coder: NSCoder) { fatalError() }

    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }


    var mainActions: [CaptureSession.Action?] = []

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

}
