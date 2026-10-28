import AppKit
import Observation
import SwiftUI

/// Upload status in the menu bar popover, with progress on the icon itself.
@MainActor
enum Uploads {
    static func start(png: Data, fileName: String, search: SearchEngine?) {
        let model = UploadModel(png: png, fileName: fileName, search: search)
        StatusPopover.show(NSHostingController(rootView: UploadView(model: model)), kind: .upload)
        model.run()
    }
}

@MainActor
@Observable
final class UploadModel {
    enum Phase {
        case uploading(Double)
        case done(URL)
        case failed(String)
    }

    var phase = Phase.uploading(0)
    var hovering = false
    let png: Data
    let fileName: String
    let search: SearchEngine?

    init(png: Data, fileName: String, search: SearchEngine?) {
        self.png = png
        self.fileName = fileName
        self.search = search
    }

    func run() {
        phase = .uploading(0)
        StatusPopover.setIcon(.progress(0))
        Task {
            do {
                let result = try await Uploaders.current().upload(png, fileName: fileName) { value in
                    Task { @MainActor in
                        guard case .uploading = self.phase else { return }
                        self.phase = .uploading(value)
                        StatusPopover.setIcon(.progress(value))
                    }
                }
                phase = .done(result.pageURL)
                copyLink()
                StatusPopover.setIcon(.success, resetAfter: 4)
                if let search { NSWorkspace.shared.open(search.url(for: result.directURL)) }
                // The link is already on the clipboard; close unless the pointer is over the popover.
                if StatusPopover.isShowingUpload { StatusPopover.closeLater(after: 4) { self.hovering } }
            } catch {
                phase = .failed(error.localizedDescription)
                StatusPopover.setIcon(.failure, resetAfter: 6)
                // If the popover was closed meanwhile, bring the problem back into view.
                if !StatusPopover.isShowingUpload {
                    StatusPopover.show(NSHostingController(rootView: UploadView(model: self)), kind: .upload)
                }
            }
        }
    }

    func copyLink() {
        guard case let .done(url) = phase else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(url.absoluteString, forType: .string)
    }
}

struct UploadView: View {
    @Bindable var model: UploadModel

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            switch model.phase {
            case let .uploading(value):
                Label("Uploading…", systemImage: "icloud.and.arrow.up").font(.headline)
                ProgressView(value: value)
                Text(model.fileName).font(.caption).foregroundStyle(.secondary).lineLimit(1)

            case let .done(url):
                Label("Uploaded. Link copied.", systemImage: "checkmark.circle.fill")
                    .font(.headline).foregroundStyle(.green)
                Text(url.absoluteString)
                    .font(.callout.monospaced()).lineLimit(1).truncationMode(.middle).textSelection(.enabled)
                HStack {
                    Button("Copy") { model.copyLink() }
                    Button("Open") { NSWorkspace.shared.open(url); StatusPopover.close() }
                    Button("Email") { Actions.email(model.png, name: model.fileName, link: url); StatusPopover.close() }
                }

            case let .failed(message):
                Label("Upload failed", systemImage: "exclamationmark.triangle.fill")
                    .font(.headline).foregroundStyle(.red)
                Text(message).font(.callout).fixedSize(horizontal: false, vertical: true)
                HStack {
                    Button("Preferences…") { StatusPopover.close(); PreferencesWindow.show(tab: .upload) }
                    Button("Try again") { model.run() }.keyboardShortcut(.defaultAction)
                }
            }
        }
        .padding(14)
        .frame(width: 340, alignment: .leading)
        .onHover { model.hovering = $0 }
    }
}
