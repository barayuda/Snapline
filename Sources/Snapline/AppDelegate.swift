import AppKit

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    static private(set) var shared: AppDelegate?

    private var statusItem: NSStatusItem!
    private let menu = NSMenu()
    private var session: CaptureSession?

    func applicationDidFinishLaunching(_ notification: Notification) {
        Self.shared = self
        Settings.registerDefaults()

        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        statusItem.button?.image = Brand.menuBarImage
        StatusPopover.button = statusItem.button
        // Prepare Preferences while nothing else is happening, so opening it never stutters.
        DispatchQueue.main.asyncAfter(deadline: .now() + 1) { Preferences.prepare() }
        statusItem.button?.target = self
        statusItem.button?.action = #selector(statusItemClicked)
        statusItem.button?.sendAction(on: [.leftMouseUp, .rightMouseUp])
        statusItem.button?.setAccessibilityHelp("Click to take a screenshot. Right-click or Control-click for the menu.")
        refreshMenu()

        HotKey.shared.onPress = { [weak self] in self?.startCapture() }
        if !HotKey.shared.registerFromDefaults() {
            let alert = NSAlert()
            alert.messageText = "The capture shortcut is already used by another app."
            alert.informativeText = "Choose another one in Snapline Preferences."
            alert.runModal()
        }
    }

    func refreshMenu() {
        menu.removeAllItems()
        let take = NSMenuItem(title: "Take Screenshot  (\(UserDefaults.standard.string(forKey: Key.hotKeyDisplay) ?? ""))",
                              action: #selector(startCapture), keyEquivalent: "")
        menu.addItem(take)
        menu.addItem(.separator())
        menu.addItem(NSMenuItem(title: "Preferences…", action: #selector(showPreferences), keyEquivalent: ","))
        menu.addItem(NSMenuItem(title: "About Snapline", action: #selector(showAbout), keyEquivalent: ""))
        menu.addItem(.separator())
        menu.addItem(NSMenuItem(title: "Quit Snapline", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q"))
        menu.items.filter { $0.action != #selector(NSApplication.terminate(_:)) }.forEach { $0.target = self }
    }

    /// Left click captures, right click (or ⌃-click) opens the menu, like Lightshot.
    @objc private func statusItemClicked() {
        let event = NSApp.currentEvent
        if event?.type == .rightMouseUp || event?.modifierFlags.contains(.control) == true {
            statusItem.menu = menu
            statusItem.button?.performClick(nil)
            statusItem.menu = nil
        } else if StatusPopover.isShowingPanel {
            StatusPopover.close()   // clicking the icon closes Preferences or the review instead of capturing
        } else {
            startCapture()
        }
    }

    @objc func startCapture() {
        guard session == nil else { return }
        StatusPopover.close()
        Task {
            do {
                let shots = try await Capture.captureAllScreens()
                guard session == nil, !shots.isEmpty else { return }
                let s = CaptureSession(shots: shots) { self.session = nil }
                session = s
                s.show()
            } catch is Capture.PermissionError {
                askForScreenRecording()
            } catch {
                NSApp.activate()
                NSAlert(error: error).runModal()
            }
        }
    }

    /// One dialog at a time. The first time a build lacks the permission, let macOS show its own prompt
    /// (it also adds Snapline to the list in System Settings). macOS shows that prompt only once per
    /// build, so after that, show Snapline's reminder with a button to the right settings page.
    private func askForScreenRecording() {
        let defaults = UserDefaults.standard
        if defaults.string(forKey: Key.screenPermissionAskedForBuild) != Self.buildID {
            defaults.set(Self.buildID, forKey: Key.screenPermissionAskedForBuild)
            CGRequestScreenCaptureAccess()
            return
        }
        let alert = NSAlert()
        alert.messageText = "Snapline needs Screen Recording permission."
        alert.informativeText = "Turn on Snapline in System Settings → Privacy & Security → Screen & System Audio Recording, then try again."
        alert.addButton(withTitle: "Open System Settings")
        alert.addButton(withTitle: "Cancel")
        NSApp.activate()
        if alert.runModal() == .alertFirstButtonReturn {
            NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")!)
        }
    }

    /// Changes with every build (ad-hoc signed builds count as a new app for macOS permissions).
    private static let buildID: String = {
        let exe = Bundle.main.executableURL?.path ?? ""
        let date = (try? FileManager.default.attributesOfItem(atPath: exe)[.modificationDate] as? Date) ?? .distantPast
        return "\(Int(date.timeIntervalSince1970))"
    }()

    @objc private func showPreferences() { Preferences.show() }

    @objc private func showAbout() {
        NSApp.activate()
        let center = NSMutableParagraphStyle()
        center.alignment = .center
        center.paragraphSpacing = 6
        let credits = NSMutableAttributedString(string: Brand.tagline + "\n", attributes: [
            .font: NSFont.systemFont(ofSize: 12, weight: .semibold), .foregroundColor: Brand.violetLight, .paragraphStyle: center,
        ])
        credits.append(NSAttributedString(string: Brand.privacyNotice + "\n\nNative on Apple Silicon. By Barayuda.", attributes: [
            .font: NSFont.systemFont(ofSize: 11), .foregroundColor: NSColor.secondaryLabelColor, .paragraphStyle: center,
        ]))
        NSApp.orderFrontStandardAboutPanel(options: [.credits: credits])
    }
}
