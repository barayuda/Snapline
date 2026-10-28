import AppKit
import SwiftUI

/// Everything Snapline shows drops down from its menu bar icon: Preferences, upload status, the Copy Text
/// review and short messages. One popover at a time; the icon itself shows upload progress and results.
@MainActor
enum StatusPopover {
    enum Kind { case message, upload, review, preferences }

    static weak var button: NSStatusBarButton?
    private static var popover: NSPopover?
    private static var kind: Kind?
    private static var autoClose: Task<Void, Never>?
    private static var iconReset: Task<Void, Never>?

    /// Review and Preferences stay until closed in code (✕, Esc, Copy/Cancel, or a click on the icon), so they
    /// survive switching to a browser; Preferences can also be dragged off into its own window.
    private static let delegate = PopoverDelegate()

    /// Shows `content` under the icon. Messages and upload status close on an outside click.
    static func show(_ content: NSViewController, kind: Kind, closeAfter seconds: Double? = nil, keepOpen: (() -> Bool)? = nil) {
        guard let button else { return }
        close(animated: false)  // never run a closing and an opening animation at the same time
        let p = NSPopover()
        p.contentViewController = content
        p.contentSize = content.preferredContentSize == .zero ? content.view.fittingSize : content.preferredContentSize
        let stays = kind == .review || kind == .preferences
        p.behavior = stays ? .applicationDefined : .transient
        p.animates = true
        p.delegate = delegate
        if stays { NSApp.activate() }
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

    static func close(animated: Bool = true) {
        autoClose?.cancel()
        if !animated { popover?.animates = false }
        popover?.performClose(nil)
        popover = nil
        kind = nil
    }

    static var isShowingReview: Bool { kind == .review && popover?.isShown == true }
    static var isShowingPreferences: Bool { kind == .preferences && popover?.isShown == true }
    /// A popover the user is working in (review or Preferences); a click on the icon closes it instead of capturing.
    static var isShowingPanel: Bool { isShowingReview || isShowingPreferences }
    static var isShowingUpload: Bool { kind == .upload && popover?.isShown == true }

    /// A short message (e.g. "Saved to Desktop") that closes by itself. Never covers the review or Preferences.
    static func message(_ title: String, _ detail: String, symbol: String = "checkmark.circle.fill", tint: Color = .green) {
        guard !isShowingPanel else { return }
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

/// Lets Preferences be dragged off the menu bar into its own floating window.
@MainActor
private final class PopoverDelegate: NSObject, NSPopoverDelegate {
    func popoverShouldDetach(_ popover: NSPopover) -> Bool { StatusPopover.isShowingPreferences }
}
