import AppKit
import SnaplineCore

/// Shown by Copy Text only when some spots could not be read for sure: the text with those spots
/// highlighted, a palette of symbols, and Copy / Cancel. Nothing reaches the clipboard until Copy.
@MainActor
final class TextReviewPanel: NSObject, NSWindowDelegate {
    private static var current: TextReviewPanel?

    private let panel: NSPanel
    private let textView: NSTextView

    /// `marked`: text from TextExtraction with UncertainMarks around the spots to check.
    static func show(_ marked: String) {
        current?.panel.close()
        let p = TextReviewPanel(marked)
        current = p
        NSApp.activate()
        p.panel.center()
        p.panel.makeKeyAndOrderFront(nil)
        p.panel.makeFirstResponder(p.textView)
    }

    private init(_ marked: String) {
        let (text, ranges) = UncertainMarks.extract(marked)
        panel = NSPanel(contentRect: CGRect(x: 0, y: 0, width: 640, height: 400),
                        styleMask: [.titled, .closable, .resizable, .utilityWindow], backing: .buffered, defer: false)
        textView = NSTextView()
        super.init()

        panel.title = "Check the copied text"
        panel.isFloatingPanel = true
        panel.isReleasedWhenClosed = false
        panel.delegate = self
        panel.minSize = CGSize(width: 520, height: 300)

        let note = NSTextField(wrappingLabelWithString:
            "Some symbols could not be read for sure; they are highlighted. Select a character and click a symbol to "
            + "replace it, or type over it. A \u{FFFD} marks a symbol that was skipped. Then click Copy.")
        note.textColor = .secondaryLabelColor

        textView.isRichText = false
        textView.allowsUndo = true
        textView.font = .systemFont(ofSize: 15)
        textView.string = text
        textView.textContainerInset = NSSize(width: 6, height: 6)
        textView.isVerticallyResizable = true
        textView.autoresizingMask = [.width]
        textView.textContainer?.widthTracksTextView = true
        for range in ranges {
            textView.layoutManager?.addTemporaryAttribute(.backgroundColor,
                                                          value: Brand.violet.withAlphaComponent(0.35), forCharacterRange: range)
        }
        if let first = ranges.first { textView.setSelectedRange(first) }
        let scroll = NSScrollView()
        scroll.documentView = textView
        scroll.hasVerticalScroller = true
        scroll.borderType = .bezelBorder

        // Two rows of symbol buttons; each inserts its symbol over the selection.
        let symbols = SymbolRepair.palette
        let half = (symbols.count + 1) / 2
        let rows = [Array(symbols.prefix(half)), Array(symbols.dropFirst(half))].map { row in
            NSStackView(views: row.map { symbol in
                let b = NSButton(title: String(symbol), target: self, action: #selector(insertSymbol(_:)))
                b.bezelStyle = .smallSquare
                b.font = .systemFont(ofSize: 15)
                b.toolTip = ShortcutWords.convert(String(symbol))
                b.widthAnchor.constraint(equalToConstant: 34).isActive = true
                return b
            })
        }

        let cancel = NSButton(title: "Cancel", target: self, action: #selector(cancelClicked))
        cancel.keyEquivalent = "\u{1b}"
        let copy = NSButton(title: "Copy", target: self, action: #selector(copyClicked))
        copy.keyEquivalent = "\r"
        copy.keyEquivalentModifierMask = [.command]
        copy.toolTip = "Copy (⌘↩)"
        let buttons = NSStackView(views: [NSView(), cancel, copy])

        let stack = NSStackView(views: [note, scroll] + rows + [buttons])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 10
        stack.edgeInsets = NSEdgeInsets(top: 14, left: 14, bottom: 14, right: 14)
        stack.translatesAutoresizingMaskIntoConstraints = false
        let content = NSView()
        content.addSubview(stack)
        panel.contentView = content
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: content.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: content.trailingAnchor),
            stack.topAnchor.constraint(equalTo: content.topAnchor),
            stack.bottomAnchor.constraint(equalTo: content.bottomAnchor),
            scroll.widthAnchor.constraint(equalTo: stack.widthAnchor, constant: -28),
            note.widthAnchor.constraint(equalTo: stack.widthAnchor, constant: -28),
            buttons.widthAnchor.constraint(equalTo: stack.widthAnchor, constant: -28),
            scroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 150),
        ])
    }

    @objc private func insertSymbol(_ sender: NSButton) {
        textView.insertText(sender.title, replacementRange: textView.selectedRange())
        panel.makeFirstResponder(textView)
    }

    @objc private func copyClicked() {
        Actions.finishCopyText(textView.string)
        panel.close()
    }

    @objc private func cancelClicked() { panel.close() }

    func windowWillClose(_ notification: Notification) {
        if Self.current === self { Self.current = nil }
    }
}
