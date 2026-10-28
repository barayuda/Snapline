import AppKit
import SwiftUI

/// Everything Snapline shows after a capture drops down from its menu bar icon: upload status, the Copy Text
/// review and short messages. One popover at a time; the icon itself shows upload progress and results.
@MainActor
enum StatusPopover {
    enum Kind { case message, upload, review }

    static weak var button: NSStatusBarButton?
    private static var popover: NSPopover?
    private static var kind: Kind?
    private static var autoClose: Task<Void, Never>?
    private static var iconReset: Task<Void, Never>?

    /// Shows `content` under the icon. `review` stays until closed in code; the others close on an outside click.
    static func show(_ content: NSViewController, kind: Kind, closeAfter seconds: Double? = nil, keepOpen: (() -> Bool)? = nil) {
        guard let button else { return }
        close()
        let p = NSPopover()
        p.contentViewController = content
        p.behavior = kind == .review ? .applicationDefined : .transient
        p.animates = true
        if kind == .review { NSApp.activate() }
        p.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
        popover = p
        Self.kind = kind
        if let seconds { closeLater(after: seconds, keepOpen: keepOpen) }
    }

    /// Closes after `seconds`, but waits while `keepOpen` (e.g. the pointer is over the popover) is true.
    static func closeLater(after seconds: Double, keepOpen: (() -> Bool)? = nil) {
        autoClose?.cancel()
        let shown = popover
        autoClose = Task {
            try? await Task.sleep(for: .seconds(seconds))
            while keepOpen?() == true { try? await Task.sleep(for: .milliseconds(500)) }
            guard !Task.isCancelled, popover === shown else { return }
            close()
        }
    }

    static func close() {
        autoClose?.cancel()
        popover?.performClose(nil)
        popover = nil
        kind = nil
    }

    static var isShowingReview: Bool { kind == .review && popover?.isShown == true }
    static var isShowingUpload: Bool { kind == .upload && popover?.isShown == true }

    /// A short message (e.g. "Saved to Desktop") that closes by itself. Never covers the Copy Text review.
    static func message(_ title: String, _ detail: String, symbol: String = "checkmark.circle.fill", tint: Color = .green) {
        guard !isShowingReview else { return }
        let view = HStack(alignment: .top, spacing: 10) {
            Image(systemName: symbol).font(.title2).foregroundStyle(tint)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.headline)
                Text(detail).font(.callout).foregroundStyle(.secondary).lineLimit(3)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(14)
        .frame(width: 300, alignment: .leading)
        show(NSHostingController(rootView: view), kind: .message, closeAfter: 2.5)
    }

    // MARK: - Icon

    /// Progress ring, ✓ or !, then back to normal. Failure tints the icon red.
    static func setIcon(_ state: Brand.IconState, resetAfter seconds: Double? = nil) {
        iconReset?.cancel()
        button?.image = Brand.menuBarImage(state)
        button?.contentTintColor = state == .failure ? .systemRed : nil
        guard let seconds else { return }
        iconReset = Task {
            try? await Task.sleep(for: .seconds(seconds))
            guard !Task.isCancelled else { return }
            button?.image = Brand.menuBarImage
            button?.contentTintColor = nil
        }
    }
}
