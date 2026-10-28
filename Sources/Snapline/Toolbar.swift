import AppKit

/// Rounded floating bar of icon buttons placed next to the selection.
final class Toolbar: NSVisualEffectView {
    let stack: NSStackView

    init(buttons: [NSView], vertical: Bool) {
        stack = NSStackView(views: buttons)
        stack.orientation = vertical ? .vertical : .horizontal
        stack.spacing = 2
        stack.edgeInsets = NSEdgeInsets(top: 4, left: 4, bottom: 4, right: 4)
        super.init(frame: .zero)
        material = .hudWindow
        state = .active
        wantsLayer = true
        layer?.cornerRadius = 8
        addSubview(stack)
        stack.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor),
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
        setFrameSize(stack.fittingSize)
    }

    @available(*, unavailable) required init?(coder: NSCoder) { fatalError() }

    static func button(_ symbol: String, _ tip: String, _ target: AnyObject, _ action: Selector, tag: Int = 0) -> NSButton {
        let image = NSImage(systemSymbolName: symbol, accessibilityDescription: tip)!
            .withSymbolConfiguration(.init(pointSize: 15, weight: .medium))!
        let b = NSButton(image: image, target: target, action: action)
        b.bezelStyle = .smallSquare
        b.isBordered = false
        b.setButtonType(.pushOnPushOff)
        b.toolTip = tip
        // Spoken label without the shortcut hint: "Upload and copy link", not "… (⌘D)". It is set on the
        // button itself so it survives image changes (the color button swaps its image).
        b.setAccessibilityLabel(tip.components(separatedBy: " (").first ?? tip)
        b.tag = tag
        b.widthAnchor.constraint(equalToConstant: 30).isActive = true
        b.heightAnchor.constraint(equalToConstant: 28).isActive = true
        b.contentTintColor = .labelColor
        return b
    }

    static func colorDot(_ color: NSColor) -> NSImage {
        NSImage(size: NSSize(width: 16, height: 16), flipped: false) { r in
            color.setFill()
            NSBezierPath(ovalIn: r.insetBy(dx: 1, dy: 1)).fill()
            NSColor.white.setStroke()
            NSBezierPath(ovalIn: r.insetBy(dx: 1, dy: 1)).stroke()
            return true
        }
    }
}
