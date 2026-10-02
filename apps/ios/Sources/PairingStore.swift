import Foundation
import Security

actor PairingStore {
    private let service: String
    private let inbox: URL
    private let journal: RestorationJournal
    private let account = "mobile-configuration-v1"
    private var query: [String: Any] { [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: account] }

    init(inbox: URL? = nil, service: String = "MeridianLab.DeveloperPairing", journal: RestorationJournal = RestorationJournal()) {
        self.inbox = inbox ?? FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0].appendingPathComponent("meridian-device.json")
        self.service = service
        self.journal = journal
    }

    /// Automatically consumes the desktop's fixed House Arrest bootstrap filename.
    /// Pairing material is never copied to preferences, diagnostics, or network services.
    func load() throws -> Data {
        // A replacement must not destroy the credentials needed to clear an old
        // session. Finish recovery with the saved configuration before consuming
        // any newly staged setup; leave that file intact for the next connection.
        if try journal.pending() != nil, let saved = try storedConfiguration() {
            try journal.validate(configuration: saved)
            return saved
        }
        if FileManager.default.fileExists(atPath: inbox.path) {
            let values = try inbox.resourceValues(forKeys: [.fileSizeKey, .isRegularFileKey, .isSymbolicLinkKey])
            guard values.isRegularFile == true, values.isSymbolicLink != true, let size = values.fileSize, size > 0, size <= 131072 else {
                throw MobileFault(code: "PAIRING_RECORD_INVALID", message: "The mobile setup file is not valid. Repair setup from your desktop.")
            }
            try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: inbox.path)
            var protected = inbox
            var metadata = URLResourceValues(); metadata.isExcludedFromBackup = true
            try protected.setResourceValues(metadata)
            let data = try Data(contentsOf: inbox)
            let config = try JSONDecoder().decode(MobileConfiguration.self, from: data)
            try config.validate()
            try journal.validate(configuration: data)
            let attributes: [String: Any] = [kSecValueData as String: data, kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly]
            let update = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
            if update == errSecItemNotFound {
                let add = SecItemAdd(query.merging(attributes) { _, value in value } as CFDictionary, nil)
                guard add == errSecSuccess else { throw storageError() }
            } else if update != errSecSuccess { throw storageError() }
            try FileManager.default.removeItem(at: inbox)
            return data
        }
        guard let data = try storedConfiguration() else {
            throw MobileFault(code: "REMOTE_PAIRING_REQUIRED", message: "Connect this iPhone to Meridian Lab on your trusted desktop and prepare the mobile app. Local maps and saved places are available now.")
        }
        try journal.validate(configuration: data)
        return data
    }
    private func storedConfiguration() throws -> Data? {
        var request = query
        request[kSecReturnData as String] = true
        request[kSecMatchLimit as String] = kSecMatchLimitOne
        var value: CFTypeRef?
        let status = SecItemCopyMatching(request as CFDictionary, &value)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = value as? Data else { throw storageError() }
        try JSONDecoder().decode(MobileConfiguration.self, from: data).validate()
        return data
    }
    func forget() throws {
        guard try journal.pending() == nil else {
            throw MobileFault(code: "RESTORE_UNCONFIRMED", message: "Restore real location before removing this iPhone's saved connection.")
        }
        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else { throw storageError() }
    }
    private func storageError() -> MobileFault { MobileFault(code: "PAIRING_STORE_UNAVAILABLE", message: "Unlock your iPhone and retry access to its protected pairing data.") }
}
