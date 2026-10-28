import AppKit
import SwiftUI
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
        case .copyText: copyText(image)
        case .save: save(png, name: Naming.fileName(for: date))
        case .saveAs: saveAs(png, name: Naming.fileName(for: date))
        case .print: print(image)
        case .edit: edit(png, name: Naming.fileName(for: date))
        case .email: email(png, name: Naming.fileName(for: date), link: nil)
        case .upload: Uploads.start(png: png, fileName: Naming.fileName(for: date), search: nil)
        case let .search(engine): Uploads.start(png: png, fileName: Naming.fileName(for: date), search: engine)
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

    /// ⇧⌘C: recognise the text in the (already flattened) image, so hidden areas stay hidden.
    /// When some spots could not be read for sure, show the review panel first.
    static func copyText(_ image: CGImage) {
        Task {
            let marked = await TextExtraction.text(in: image)
            guard !UncertainMarks.strip(marked).trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                return StatusPopover.message("No text found", "Snapline could not read any text in the selection.",
                                             symbol: "text.magnifyingglass", tint: .secondary)
            }
            if UncertainMarks.contains(marked) {
                TextReviewController.show(marked)
            } else {
                finishCopyText(marked)
            }
        }
    }

    /// Puts the final text on the clipboard, as words if Preferences asks for it.
    static func finishCopyText(_ text: String) {
        let plain = UncertainMarks.strip(text)
        let out = UserDefaults.standard.bool(forKey: Key.copyShortcutsAsWords) ? ShortcutWords.convert(plain) : plain
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(out, forType: .string)
        let lines = out.split(separator: "\n").count
        StatusPopover.message("Text copied", "\(lines) line\(lines == 1 ? "" : "s"): \(out.prefix(80))")
    }

    /// ⌘S: straight into the folder from Preferences (Desktop by default), never overwriting.
    static func save(_ png: Data, name: String) {
        let dir = Settings.saveDirectory
        do {
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            let url = Naming.uniqueFileURL(in: dir, name: name) { FileManager.default.fileExists(atPath: $0.path) }
            try png.write(to: url)
            StatusPopover.message("Screenshot saved", "\(url.lastPathComponent) in \(FileManager.default.displayName(atPath: dir.path))")
        } catch {
            NSApp.activate()
            let alert = NSAlert(error: error)
            alert.informativeText = "Could not save to \(dir.path). Choose another folder in Snapline Preferences → General."
            alert.runModal()
        }
    }

    /// ⌥⌘S: pick a place once; the dialog starts in the default folder.
    static func saveAs(_ png: Data, name: String) {
        NSApp.activate()
        let panel = NSSavePanel()
        panel.nameFieldStringValue = name
        panel.allowedContentTypes = [.png]
        panel.canCreateDirectories = true
        panel.directoryURL = Settings.saveDirectory
        guard panel.runModal() == .OK, let url = panel.url else { return }
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
