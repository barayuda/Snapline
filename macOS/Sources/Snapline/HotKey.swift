import AppKit
import Carbon.HIToolbox

/// One global hot key through Carbon `RegisterEventHotKey`, which is still the
/// supported way to get a system-wide shortcut without Accessibility permission.
@MainActor
final class HotKey {
    static let shared = HotKey()

    var onPress: (() -> Void)?
    private var ref: EventHotKeyRef?

    private init() {
        var type = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        InstallEventHandler(GetApplicationEventTarget(), { _, _, _ in
            // Carbon delivers application events on the main thread.
            MainActor.assumeIsolated { HotKey.shared.onPress?() }
            return noErr
        }, 1, &type, nil, nil)
    }

    /// Registers the shortcut stored in UserDefaults. Returns false if macOS refused it (already taken).
    @discardableResult
    func registerFromDefaults() -> Bool {
        let d = UserDefaults.standard
        return register(keyCode: UInt32(d.integer(forKey: Key.hotKeyCode)), modifiers: UInt32(d.integer(forKey: Key.hotKeyModifiers)))
    }

    func register(keyCode: UInt32, modifiers: UInt32) -> Bool {
        unregister()
        let id = EventHotKeyID(signature: OSType(0x534E_4150), id: 1) // "SNAP"
        return RegisterEventHotKey(keyCode, modifiers, id, GetApplicationEventTarget(), 0, &ref) == noErr
    }

    func unregister() {
        if let ref { UnregisterEventHotKey(ref) }
        ref = nil
    }

    // MARK: - Converting a recorded NSEvent

    static func carbonModifiers(_ flags: NSEvent.ModifierFlags) -> UInt32 {
        var m: UInt32 = 0
        if flags.contains(.command) { m |= UInt32(cmdKey) }
        if flags.contains(.shift) { m |= UInt32(shiftKey) }
        if flags.contains(.option) { m |= UInt32(optionKey) }
        if flags.contains(.control) { m |= UInt32(controlKey) }
        return m
    }

    static func display(for event: NSEvent) -> String {
        let f = event.modifierFlags
        var s = ""
        if f.contains(.control) { s += "⌃" }
        if f.contains(.option) { s += "⌥" }
        if f.contains(.shift) { s += "⇧" }
        if f.contains(.command) { s += "⌘" }
        let special: [Int: String] = [
            kVK_Space: "Space", kVK_Return: "↩", kVK_Tab: "⇥", kVK_F1: "F1", kVK_F2: "F2", kVK_F3: "F3", kVK_F4: "F4",
            kVK_F5: "F5", kVK_F6: "F6", kVK_F7: "F7", kVK_F8: "F8", kVK_F9: "F9", kVK_F10: "F10", kVK_F11: "F11", kVK_F12: "F12",
        ]
        let key = special[Int(event.keyCode)] ?? (event.characters(byApplyingModifiers: []) ?? "?").uppercased()
        return s + key
    }
}
