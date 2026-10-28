import AppKit

/// Small floating panel shown while uploading: progress, then the link with Copy / Open / Email.
@MainActor
final class UploadPanel: NSObject, NSWindowDelegate {
    private static var openPanels: [UploadPanel] = []

    private let panel: NSPanel
    private let status = NSTextField(labelWithString: "Uploading…")
    private let progress = NSProgressIndicator()
    private let link = NSTextField(string: "")
    private let buttons: NSStackView
    private let png: Data
    private let fileName: String
    private var result: UploadResult?

    static func start(png: Data, fileName: String, search: SearchEngine?) {
        let p = UploadPanel(png: png, fileName: fileName)
        openPanels.append(p)
        Task { await p.upload(search: search) }
    }

    private init(png: Data, fileName: String) {
        self.png = png
        self.fileName = fileName
        panel = NSPanel(contentRect: CGRect(x: 0, y: 0, width: 420, height: 104),
                        styleMask: [.titled, .closable, .nonactivatingPanel, .utilityWindow], backing: .buffered, defer: false)
        let copy = NSButton(title: "Copy", target: nil, action: #selector(copyLink))
        let open = NSButton(title: "Open", target: nil, action: #selector(openLink))
        let email = NSButton(title: "Email", target: nil, action: #selector(emailLink))
        buttons = NSStackView(views: [copy, open, email])
        super.init()
        [copy, open, email].forEach { $0.target = self }

        panel.title = "Snapline upload"
        panel.isFloatingPanel = true
        panel.level = .floating
        panel.isReleasedWhenClosed = false
        panel.delegate = self

        progress.style = .bar
        progress.isIndeterminate = false
        progress.minValue = 0
        progress.maxValue = 1
        link.isEditable = false
        link.isSelectable = true
        link.isHidden = true
        buttons.isHidden = true

        let stack = NSStackView(views: [status, progress, link, buttons])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.edgeInsets = NSEdgeInsets(top: 12, left: 14, bottom: 12, right: 14)
        stack.translatesAutoresizingMaskIntoConstraints = false
        panel.contentView = NSView()
        panel.contentView!.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: panel.contentView!.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: panel.contentView!.trailingAnchor),
            stack.topAnchor.constraint(equalTo: panel.contentView!.topAnchor),
            stack.bottomAnchor.constraint(equalTo: panel.contentView!.bottomAnchor),
            progress.widthAnchor.constraint(equalTo: stack.widthAnchor, constant: -28),
            link.widthAnchor.constraint(equalTo: stack.widthAnchor, constant: -28),
        ])

        if let screen = NSScreen.main?.visibleFrame {
            panel.setFrameTopLeftPoint(CGPoint(x: screen.maxX - panel.frame.width - 16, y: screen.maxY - 16))
        }
        panel.orderFrontRegardless()
    }

    private func upload(search: SearchEngine?) async {
        do {
            let uploader = try Uploaders.current()
            let result = try await uploader.upload(png, fileName: fileName) { [weak self] value in
                Task { @MainActor in self?.progress.doubleValue = value }
            }
            self.result = result
            copyLink()
            status.stringValue = "Uploaded. Link copied to the clipboard."
            progress.isHidden = true
            link.stringValue = result.pageURL.absoluteString
            link.isHidden = false
            buttons.isHidden = false
            Actions.notify("Screenshot uploaded", result.pageURL.absoluteString)
            if let search { NSWorkspace.shared.open(search.url(for: result.directURL)) }
        } catch {
            status.stringValue = "Upload failed: \(error.localizedDescription)"
            progress.isHidden = true
            Actions.notify("Upload failed", error.localizedDescription)
        }
    }

    @objc private func copyLink() {
        guard let url = result?.pageURL else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(url.absoluteString, forType: .string)
    }

    @objc private func openLink() {
        if let url = result?.pageURL { NSWorkspace.shared.open(url) }
    }

    @objc private func emailLink() {
        Actions.email(png, name: fileName, link: result?.pageURL)
    }

    func windowWillClose(_ notification: Notification) {
        Self.openPanels.removeAll { $0 === self }
    }
}
