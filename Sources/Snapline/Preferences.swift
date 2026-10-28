import AppKit
import ServiceManagement
import SnaplineCore
import SwiftUI

@MainActor
enum PreferencesWindow {
    private static var window: NSWindow?

    static func show() {
        if window == nil {
            let w = NSWindow(contentViewController: NSHostingController(rootView: PreferencesView()))
            w.title = "Snapline Preferences"
            w.styleMask = [.titled, .closable]
            w.isReleasedWhenClosed = false
            window = w
        }
        NSApp.activate()
        window?.center()
        window?.makeKeyAndOrderFront(nil)
    }
}

struct PreferencesView: View {
    var body: some View {
        VStack(spacing: 14) {
            BrandHeader()
            TabView {
                GeneralTab().tabItem { Label("General", systemImage: "gearshape") }
                UploadTab().tabItem { Label("Upload", systemImage: "icloud.and.arrow.up") }
                AutoHideTab().tabItem { Label("Auto-hide", systemImage: "eye.slash") }
            }
            // A grouped Form has no natural height inside a TabView; without this the tabs collapse to nothing.
            .frame(height: 470)
        }
        .tint(Color(nsColor: Brand.violet))
        .frame(width: 560)
        .padding()
    }
}

private struct BrandHeader: View {
    var body: some View {
        HStack(spacing: 14) {
            Image(nsImage: NSApp.applicationIconImage)
                .resizable()
                .frame(width: 56, height: 56)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text("Snapline").font(.system(size: 22, weight: .bold, design: .rounded))
                Text(Brand.tagline)
                    .font(.callout.weight(.medium))
                    .foregroundStyle(LinearGradient(colors: [Color(nsColor: Brand.violetLight), Color(nsColor: Brand.cyan)],
                                                    startPoint: .leading, endPoint: .trailing))
            }
            Spacer()
            Text("v\(Brand.version)").font(.caption.monospaced()).foregroundStyle(.tertiary)
        }
        .padding(.horizontal, 6)
    }
}

// MARK: - General

private struct GeneralTab: View {
    @AppStorage(Key.hotKeyDisplay) private var hotKeyDisplay = "⇧⌘1"
    @AppStorage(Key.downscaleRetina) private var downscaleRetina = false
    @AppStorage(Key.copyShortcutsAsWords) private var copyShortcutsAsWords = false
    @AppStorage(Key.editorAppPath) private var editorAppPath = "/System/Applications/Preview.app"
    @AppStorage(Key.saveDirectory) private var saveDirectory = Settings.saveDirectory.path
    @State private var recording = false
    @State private var monitor: Any?
    @State private var hotKeyError: String?
    @State private var launchAtLogin = SMAppService.mainApp.status == .enabled
    @State private var loginError: String?

    var body: some View {
        Form {
            LabeledContent("Capture shortcut") {
                HStack {
                    Text(recording ? "Type a shortcut…" : hotKeyDisplay)
                        .frame(minWidth: 110)
                        .padding(.vertical, 3)
                        .background(RoundedRectangle(cornerRadius: 5).stroke(recording ? Color.accentColor : .secondary))
                    Button(recording ? "Cancel" : "Record") { recording ? stopRecording() : startRecording() }
                }
            }
            if let hotKeyError { Text(hotKeyError).foregroundStyle(.red).font(.caption) }

            LabeledContent("Save screenshots to") {
                HStack {
                    Image(nsImage: NSWorkspace.shared.icon(forFile: saveDirectory))
                        .resizable()
                        .frame(width: 16, height: 16)
                        .accessibilityHidden(true)
                    Text(FileManager.default.displayName(atPath: saveDirectory))
                        .help(saveDirectory)
                    Button("Choose…", action: chooseSaveDirectory)
                }
            }

            Toggle("Launch at login", isOn: $launchAtLogin)
                .onChange(of: launchAtLogin) { _, on in setLaunchAtLogin(on) }
            if let loginError { Text(loginError).foregroundStyle(.red).font(.caption) }

            Toggle("Downscale Retina screens (save at 1× size)", isOn: $downscaleRetina)

            Toggle(isOn: $copyShortcutsAsWords) {
                Text("Copy Text: write keyboard symbols as words")
                Text("⇧⌘C becomes Shift+Cmd+C, for apps or forms that show symbols as boxes.")
            }

            LabeledContent("Edit with") {
                HStack {
                    Text(FileManager.default.displayName(atPath: editorAppPath))
                    Button("Choose…", action: chooseEditor)
                }
            }
        }
        .formStyle(.grouped)
    }

    private func startRecording() {
        hotKeyError = nil
        recording = true
        HotKey.shared.unregister()
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            let mods = HotKey.carbonModifiers(event.modifierFlags)
            if event.keyCode == 53 { stopRecording(); return nil }  // Esc cancels
            guard event.modifierFlags.intersection([.command, .control, .option]).isEmpty == false else {
                hotKeyError = "Use at least one of ⌘, ⌃ or ⌥."
                return nil
            }
            if HotKey.shared.register(keyCode: UInt32(event.keyCode), modifiers: mods) {
                let d = UserDefaults.standard
                d.set(Int(event.keyCode), forKey: Key.hotKeyCode)
                d.set(Int(mods), forKey: Key.hotKeyModifiers)
                hotKeyDisplay = HotKey.display(for: event)
                AppDelegate.shared?.refreshMenu()
            } else {
                hotKeyError = "\(HotKey.display(for: event)) is already used by another app."
            }
            stopRecording()
            return nil
        }
    }

    private func stopRecording() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        recording = false
        HotKey.shared.registerFromDefaults()
    }

    private func setLaunchAtLogin(_ on: Bool) {
        do {
            if on { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
            loginError = nil
        } catch {
            loginError = error.localizedDescription
        }
        launchAtLogin = SMAppService.mainApp.status == .enabled
        if on && SMAppService.mainApp.status == .requiresApproval {
            loginError = "Allow Snapline in System Settings → General → Login Items."
        }
    }

    private func chooseSaveDirectory() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.canCreateDirectories = true
        panel.prompt = "Use This Folder"
        panel.directoryURL = URL(fileURLWithPath: saveDirectory)
        if panel.runModal() == .OK, let url = panel.url { saveDirectory = url.path }
    }

    private func chooseEditor() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.application]
        panel.directoryURL = URL(fileURLWithPath: "/Applications")
        if panel.runModal() == .OK, let url = panel.url { editorAppPath = url.path }
    }
}

// MARK: - Upload

private struct UploadTab: View {
    @AppStorage(Key.uploadTarget) private var target = "s3"
    @AppStorage(Key.s3Endpoint) private var endpoint = ""
    @AppStorage(Key.s3Region) private var region = "auto"
    @AppStorage(Key.s3Bucket) private var bucket = ""
    @AppStorage(Key.s3PublicBaseURL) private var publicURL = ""
    @AppStorage(Key.s3Prefix) private var prefix = "snapline"
    @AppStorage(Key.googleClientID) private var clientID = ""
    @AppStorage(Key.googleEmail) private var googleEmail = ""
    @State private var accessKey = Keychain.get(.s3AccessKey) ?? ""
    @State private var secretKey = Keychain.get(.s3SecretKey) ?? ""
    @State private var clientSecret = Keychain.get(.googleClientSecret) ?? ""
    @State private var signedIn = GoogleAuth.shared.isSignedIn
    @State private var busy = false
    @State private var message: String?

    var body: some View {
        Form {
            Picker("Upload to", selection: $target) {
                Text("S3-compatible bucket").tag("s3")
                Text("Google Drive").tag("drive")
            }
            .pickerStyle(.segmented)

            if target == "s3" {
                Section("Bucket") {
                    TextField("Endpoint", text: $endpoint, prompt: Text("https://<account>.r2.cloudflarestorage.com"))
                    TextField("Region", text: $region, prompt: Text("auto, or e.g. ap-southeast-1"))
                    TextField("Bucket", text: $bucket)
                    TextField("Key prefix", text: $prefix)
                    TextField("Public URL", text: $publicURL, prompt: Text("https://shots.example.com"))
                }
                Section("Keys (stored in the Keychain)") {
                    TextField("Access key ID", text: $accessKey)
                        .onChange(of: accessKey) { _, v in Keychain.set(.s3AccessKey, v.trimmingCharacters(in: .whitespaces)) }
                    SecureField("Secret access key", text: $secretKey)
                        .onChange(of: secretKey) { _, v in Keychain.set(.s3SecretKey, v.trimmingCharacters(in: .whitespaces)) }
                }
                Text("Uploads are public to anyone with the link. Do not upload guest or customer data.")
                    .font(.caption).foregroundStyle(.secondary)
            } else {
                Section {
                    TextField("Client ID", text: $clientID, prompt: Text("Paste: …apps.googleusercontent.com"))
                    SecureField("Client secret", text: $clientSecret, prompt: Text("Paste: GOCSPX-…"))
                        .onChange(of: clientSecret) { _, v in Keychain.set(.googleClientSecret, v.trimmingCharacters(in: .whitespaces)) }
                } header: {
                    Text("Google Cloud OAuth client (type: Desktop app)")
                } footer: {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("One-time setup in Google Cloud, about 10 minutes. The secret is stored in your Keychain.")
                        HStack(spacing: 14) {
                            Link("Open Google Cloud Console", destination: URL(string: "https://console.cloud.google.com/auth/clients")!)
                            Link("Step-by-step guide", destination: URL(string: "https://github.com/barayuda/Snapline#google-drive-one-time-setup-about-10-minutes")!)
                        }
                    }
                    .font(.caption)
                    .foregroundStyle(.secondary)
                }
                Section("Account") {
                    if signedIn {
                        LabeledContent("Signed in as", value: googleEmail.isEmpty ? "Google account" : googleEmail)
                        Button("Sign out") { Task { busy = true; await GoogleAuth.shared.signOut(); refresh(); busy = false } }
                    } else {
                        Button("Sign in with Google…") { signIn() }.disabled(busy || !hasClient)
                        if !hasClient {
                            Text("Fill in the Client ID and Client secret above to turn on sign-in.")
                                .font(.caption).foregroundStyle(.secondary)
                        }
                    }
                    if busy { ProgressView().controlSize(.small) }
                }
                Text("Screenshots go to a “Snapline” folder in My Drive and are shared as “anyone with the link can view”. Snapline can only see files it created.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            if let message { Text(message).font(.caption).foregroundStyle(.red) }
        }
        .formStyle(.grouped)
    }

    private var hasClient: Bool {
        !clientID.trimmingCharacters(in: .whitespaces).isEmpty && !clientSecret.trimmingCharacters(in: .whitespaces).isEmpty
    }

    private func signIn() {
        busy = true
        message = nil
        Task {
            do { try await GoogleAuth.shared.signIn() } catch { message = error.localizedDescription }
            refresh()
            busy = false
            PreferencesWindow.show()
        }
    }

    private func refresh() {
        signedIn = GoogleAuth.shared.isSignedIn
        googleEmail = GoogleAuth.shared.email ?? ""
    }
}

// MARK: - Auto-hide

private struct AutoHideTab: View {
    @AppStorage(Key.autoHideEnabled) private var enabled = true

    var body: some View {
        Form {
            Toggle("Hide sensitive data automatically on every capture", isOn: $enabled)
            Section("What to hide") {
                ForEach(SensitiveCategory.allCases, id: \.self) { c in
                    CategoryToggle(category: c).disabled(!enabled)
                }
            }
            Text("Runs on this Mac only. Found items are covered with solid black boxes; click a box in the capture to show it again. "
                 + "This is a safety net, not a guarantee: small, faint or rotated text can be missed, so check before you share.")
                .font(.caption).foregroundStyle(.secondary)
        }
        .formStyle(.grouped)
    }
}

private struct CategoryToggle: View {
    let category: SensitiveCategory
    @AppStorage private var on: Bool

    init(category: SensitiveCategory) {
        self.category = category
        _on = AppStorage(wrappedValue: true, Key.autoHide(category))
    }

    var body: some View { Toggle(category.title, isOn: $on) }
}
