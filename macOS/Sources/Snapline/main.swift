import AppKit

// Diagnostics: `Snapline --scan image.png` prints what auto-hide would cover;
// `Snapline --text image.png` prints what Copy Text would copy. Both exit afterwards.
if CommandLine.arguments.count == 3, ["--scan", "--text"].contains(CommandLine.arguments[1]) {
    Settings.registerDefaults()
    guard let image = NSImage(contentsOfFile: CommandLine.arguments[2])?.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
        print("Cannot read \(CommandLine.arguments[2])")
        exit(1)
    }
    Task {
        if CommandLine.arguments[1] == "--text" {
            // Marked spots (the review panel would highlight these) are shown as ⟦…⟧.
            let marked = await TextExtraction.text(in: image)
            print(marked.replacingOccurrences(of: "\u{E000}", with: "⟦").replacingOccurrences(of: "\u{E001}", with: "⟧"))
            exit(0)
        }
        let size = CGSize(width: image.width, height: image.height)
        for box in await Redaction.scan(image, viewSize: size, categories: Settings.autoHideCategories) {
            print(box.category.rawValue, box.rect.integral)
        }
        exit(0)
    }
    RunLoop.main.run()
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory)  // menu bar only, no Dock icon
app.run()
