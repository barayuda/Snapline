import Foundation
import Security
import SnaplineCore

/// UserDefaults keys. SwiftUI reads the same keys through @AppStorage.
enum Key {
    static let hotKeyCode = "hotKeyCode"
    static let hotKeyModifiers = "hotKeyModifiers"
    static let hotKeyDisplay = "hotKeyDisplay"
    static let downscaleRetina = "downscaleRetina"
    static let editorAppPath = "editorAppPath"
    static let autoHideEnabled = "autoHideEnabled"
    static let uploadTarget = "uploadTarget"          // "s3" or "drive"
    static let s3Endpoint = "s3Endpoint"
    static let s3Region = "s3Region"
    static let s3Bucket = "s3Bucket"
    static let s3PublicBaseURL = "s3PublicBaseURL"
    static let s3Prefix = "s3Prefix"
    static let googleClientID = "googleClientID"
    static let googleEmail = "googleEmail"
    static let driveFolderID = "driveFolderID"
    static let saveDirectory = "saveDirectory"        // folder path for ⌘S
    static let copyShortcutsAsWords = "copyShortcutsAsWords"
    static let screenPermissionAskedForBuild = "screenPermissionAskedForBuild"

    static func autoHide(_ c: SensitiveCategory) -> String { "autoHide.\(c.rawValue)" }
}

/// Keychain item names. Secrets never go to UserDefaults.
enum Secret: String {
    case s3AccessKey, s3SecretKey, googleClientSecret, googleRefreshToken
}

enum Settings {
    static func registerDefaults() {
        var defaults: [String: Any] = [
            Key.hotKeyCode: 18,               // kVK_ANSI_1
            Key.hotKeyModifiers: 256 | 512,   // cmdKey | shiftKey
            Key.hotKeyDisplay: "⇧⌘1",
            Key.downscaleRetina: false,
            Key.copyShortcutsAsWords: false,
            Key.editorAppPath: "/System/Applications/Preview.app",
            Key.autoHideEnabled: true,
            Key.uploadTarget: "s3",
            Key.s3Region: "auto",
            Key.s3Prefix: "snapline",
            Key.saveDirectory: FileManager.default.urls(for: .desktopDirectory, in: .userDomainMask)[0].path,
        ]
        for c in SensitiveCategory.allCases { defaults[Key.autoHide(c)] = true }
        UserDefaults.standard.register(defaults: defaults)
    }

    static var autoHideCategories: Set<SensitiveCategory> {
        guard UserDefaults.standard.bool(forKey: Key.autoHideEnabled) else { return [] }
        return Set(SensitiveCategory.allCases.filter { UserDefaults.standard.bool(forKey: Key.autoHide($0)) })
    }

    static var saveDirectory: URL {
        URL(fileURLWithPath: string(Key.saveDirectory), isDirectory: true)
    }

    static func string(_ key: String) -> String {
        (UserDefaults.standard.string(forKey: key) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

/// Minimal generic-password Keychain wrapper.
enum Keychain {
    private static let service = "dev.barayuda.snapline"

    private static func query(_ secret: Secret) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: service,
         kSecAttrAccount as String: secret.rawValue]
    }

    static func get(_ secret: Secret) -> String? {
        var q = query(secret)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &item) == errSecSuccess, let data = item as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    /// Empty or nil value deletes the item.
    static func set(_ secret: Secret, _ value: String?) {
        SecItemDelete(query(secret) as CFDictionary)
        guard let value, !value.isEmpty else { return }
        var q = query(secret)
        q[kSecValueData as String] = Data(value.utf8)
        q[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
        SecItemAdd(q as CFDictionary, nil)
    }
}
