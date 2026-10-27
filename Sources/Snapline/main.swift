import AppKit

// Diagnostic: `Snapline --scan image.png` prints what auto-hide would cover, then exits.
if CommandLine.arguments.count == 3, CommandLine.arguments[1] == "--scan" {
    Settings.registerDefaults()
    guard let image = NSImage(contentsOfFile: CommandLine.arguments[2])?.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
        print("Cannot read \(CommandLine.arguments[2])")
        exit(1)
    }
    Task {
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
