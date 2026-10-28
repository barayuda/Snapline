import AppKit
import Carbon.HIToolbox
import Observation
import ServiceManagement
import SnaplineCore
import SwiftUI

enum PreferencesTab: Hashable { case general, upload, autoHide }

/// Which tab Preferences shows; set before opening to jump to a tab.
@MainActor
@Observable
final class PreferencesState {
    static let shared = PreferencesState()
    var tab = PreferencesTab.general
}

/// Preferences, shown in the menu bar popover.
@MainActor
enum Preferences {
    /// Built once and kept: laying the form out took 161 ms the first time and ~17 ms after that,
    /// which made the popover stutter while it animated open.
    private static var controller: NSHostingController<PreferencesView>?

    private static var ready: NSHostingController<PreferencesView> {
        if let controller { return controller }
        let c = NSHostingController(rootView: PreferencesView())
        c.preferredContentSize = c.view.fittingSize
        controller = c
        return c
    }

    /// Build and lay out Preferences ahead of time, so the first open is smooth too.
    static func prepare() { _ = ready }

    static func show(tab: PreferencesTab? = nil) {
        if let tab { PreferencesState.shared.tab = tab }
        guard !StatusPopover.isShowingPreferences else { return }
        StatusPopover.show(ready, kind: .preferences)
    }
}

struct PreferencesView: View {
    @Bindable private var state = PreferencesState.shared

    var body: some View {
        VStack(spacing: 14) {
            BrandHeader()
            TabView(selection: $state.tab) {
                GeneralTab().tabItem { Label("General", systemImage: "gearshape") }.tag(PreferencesTab.general)
                UploadTab().tabItem { Label("Upload", systemImage: "icloud.and.arrow.up") }.tag(PreferencesTab.upload)
                AutoHideTab().tabItem { Label("Auto-hide", systemImage: "eye.slash") }.tag(PreferencesTab.autoHide)
            }
            // A grouped Form has no natural height inside a TabView; without this the tabs collapse to nothing.
            .frame(height: 470)
        }
        .tint(Color(nsColor: Brand.violet))
        .frame(width: 560)
        .padding()
        .onExitCommand { StatusPopover.close() }  // Esc
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
            VStack(alignment: .trailing, spacing: 6) {
                Button { StatusPopover.close() } label: {
                    Image(systemName: "xmark.circle.fill").font(.title2).foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .help("Close (Esc)")
                .accessibilityLabel("Close Preferences")
                Text("v\(Brand.version)").font(.caption.monospaced()).foregroundStyle(.tertiary)
            }
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
    @State private var launchAtLogin = false   // loaded in onAppear: this view is kept alive between opens
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

            // A Binding, not onChange: setLaunchAtLogin writes launchAtLogin back, and onChange would then
            // fire again and undo the registration (e.g. when macOS asks for approval first).
            Toggle("Launch at login", isOn: Binding(get: { launchAtLogin }, set: { setLaunchAtLogin($0) }))
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
        .onAppear { launchAtLogin = SMAppService.mainApp.status == .enabled }
        // Closing Preferences mid-recording must not leave the key monitor running or the hotkey off.
        .onDisappear { if recording { stopRecording() } }
    }

    private func startRecording() {
        hotKeyError = nil
        recording = true
        HotKey.shared.unregister()
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            let mods = HotKey.carbonModifiers(event.modifierFlags)
            if event.keyCode == kVK_Escape { stopRecording(); return nil }  // Esc cancels
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
    // Loaded in onAppear (this view is kept alive between opens, so initial values would go stale).
    @State private var accessKey = ""
    @State private var secretKey = ""
    @State private var clientSecret = ""
    @State private var signedIn = false
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
                        .onChange(of: accessKey) { _, v in save(.s3AccessKey, v) }
                    SecureField("Secret access key", text: $secretKey)
                        .onChange(of: secretKey) { _, v in save(.s3SecretKey, v) }
                }
                Text("Uploads are public to anyone with the link. Do not upload guest or customer data.")
                    .font(.caption).foregroundStyle(.secondary)
            } else {
                Section {
                    TextField("Client ID", text: $clientID, prompt: Text("Paste: …apps.googleusercontent.com"))
                    SecureField("Client secret", text: $clientSecret, prompt: Text("Paste: GOCSPX-…"))
                        .onChange(of: clientSecret) { _, v in save(.googleClientSecret, v) }
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
        .onAppear(perform: load)
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
            Preferences.show()
        }
    }

    private func refresh() {
        signedIn = GoogleAuth.shared.isSignedIn
        googleEmail = GoogleAuth.shared.email ?? ""
    }

    private func load() {
        accessKey = Keychain.get(.s3AccessKey) ?? ""
        secretKey = Keychain.get(.s3SecretKey) ?? ""
        clientSecret = Keychain.get(.googleClientSecret) ?? ""
        refresh()
    }

    private func save(_ secret: Secret, _ value: String) {
        message = Keychain.set(secret, value.trimmingCharacters(in: .whitespaces)) ? nil : "Could not save to the Keychain."
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
