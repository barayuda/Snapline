import AppKit
import SnaplineCore
import UniformTypeIdentifiers

/// What happens to the flattened screenshot after the overlay closes.
@MainActor
enum Actions {
    static func run(_ action: CaptureSession.Action, image: CGImage) {
        let date = Date()
        guard let png = pngData(image) else { return NSSound.beep() }
        switch action {
        case .copy: copy(image, png)
        case .save: save(png, name: Naming.fileName(for: date))
        case .print: print(image)
        case .edit: edit(png, name: Naming.fileName(for: date))
        case .email: email(png, name: Naming.fileName(for: date), link: nil)
        case .upload: UploadPanel.start(png: png, fileName: Naming.fileName(for: date), search: nil)
        case let .search(engine): UploadPanel.start(png: png, fileName: Naming.fileName(for: date), search: engine)
        case .cancel: break
        }
    }

    static func pngData(_ image: CGImage) -> Data? {
        NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:])
    }

    private static func nsImage(_ image: CGImage) -> NSImage {
        // Point size = pixels / backing scale, so Retina shots paste at the right size.
        let scale = NSScreen.main?.backingScaleFactor ?? 2
        let downscaled = UserDefaults.standard.bool(forKey: Key.downscaleRetina)
        return NSImage(cgImage: image, size: NSSize(width: CGFloat(image.width) / (downscaled ? 1 : scale),
                                                    height: CGFloat(image.height) / (downscaled ? 1 : scale)))
    }

    static func copy(_ image: CGImage, _ png: Data) {
        let pb = NSPasteboard.general
        pb.clearContents()
        let item = NSPasteboardItem()
        item.setData(png, forType: .png)
        if let tiff = nsImage(image).tiffRepresentation { item.setData(tiff, forType: .tiff) }
        pb.writeObjects([item])
    }

    static func save(_ png: Data, name: String) {
        NSApp.activate()
        let panel = NSSavePanel()
        panel.nameFieldStringValue = name
        panel.allowedContentTypes = [.png]
        panel.canCreateDirectories = true
        if let dir = UserDefaults.standard.url(forKey: Key.lastSaveDirectory) {
            panel.directoryURL = dir
        } else {
            panel.directoryURL = FileManager.default.urls(for: .desktopDirectory, in: .userDomainMask).first
        }
        guard panel.runModal() == .OK, let url = panel.url else { return }
        UserDefaults.standard.set(url.deletingLastPathComponent(), forKey: Key.lastSaveDirectory)
        do { try png.write(to: url) } catch { NSAlert(error: error).runModal() }
    }

    static func print(_ image: CGImage) {
        NSApp.activate()
        let img = nsImage(image)
        let view = NSImageView(frame: CGRect(origin: .zero, size: img.size))
        view.image = img
        view.imageScaling = .scaleProportionallyDown
        let info = NSPrintInfo.shared.copy() as! NSPrintInfo
        info.horizontalPagination = .fit
        info.verticalPagination = .fit
        info.isHorizontallyCentered = true
        info.isVerticallyCentered = true
        info.orientation = img.size.width > img.size.height ? .landscape : .portrait
        NSPrintOperation(view: view, printInfo: info).run()
    }

    /// Writes a temp file; macOS removes the temp folder over time.
    static func tempFile(_ png: Data, name: String) -> URL? {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("Snapline", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let url = dir.appendingPathComponent(name)
        return (try? png.write(to: url)) != nil ? url : nil
    }

    static func edit(_ png: Data, name: String) {
        guard let file = tempFile(png, name: name) else { return NSSound.beep() }
        let path = Settings.string(Key.editorAppPath)
        let app = FileManager.default.fileExists(atPath: path) ? URL(fileURLWithPath: path)
            : URL(fileURLWithPath: "/System/Applications/Preview.app")
        NSWorkspace.shared.open([file], withApplicationAt: app, configuration: NSWorkspace.OpenConfiguration())
    }

    static func email(_ png: Data, name: String, link: URL?) {
        guard let file = tempFile(png, name: name) else { return NSSound.beep() }
        var items: [Any] = [file]
        if let link { items.insert("Screenshot: \(link.absoluteString)" as NSString, at: 0) }
        if let mail = NSSharingService(named: .composeEmail), mail.canPerform(withItems: items) {
            mail.subject = "Screenshot"
            mail.perform(withItems: items)
        } else if let link {
            // No Mail account: fall back to the default mail app with the link only.
            let body = "Screenshot: \(link.absoluteString)".addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? ""
            NSWorkspace.shared.open(URL(string: "mailto:?subject=Screenshot&body=\(body)")!)
        } else {
            let alert = NSAlert()
            alert.messageText = "No email account is set up in Mail."
            alert.informativeText = "Upload the screenshot first to email its link, or add an account in the Mail app."
            alert.runModal()
        }
    }
}
