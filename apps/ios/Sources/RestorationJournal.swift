import Foundation
import CryptoKit

/// Persists only a hash of the expected device identity, never coordinates or pairing.
/// A pending marker is removed only after an acknowledged native Clear.
struct RestorationJournal {
    struct Marker: Codable { let schemaVersion: Int; let deviceFingerprint: String }
    private let file: URL
    init(file: URL? = nil) {
        self.file = file ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("MeridianLab/restore-required.json")
    }
    static func fingerprint(_ configuration: Data) throws -> String {
        let config = try JSONDecoder().decode(MobileConfiguration.self, from: configuration)
        try config.validate()
        return SHA256.hash(data: Data(config.expectedDeviceId.utf8)).map { String(format: "%02x", $0) }.joined()
    }
    func pending() throws -> Marker? {
        guard FileManager.default.fileExists(atPath: file.path) else { return nil }
        let data = try Data(contentsOf: file)
        guard data.count <= 1024 else { throw journalError() }
        let marker = try JSONDecoder().decode(Marker.self, from: data)
        guard marker.schemaVersion == 1, marker.deviceFingerprint.count == 64 else { throw journalError() }
        return marker
    }
    func validate(configuration: Data) throws {
        if let marker = try pending(), marker.deviceFingerprint != (try Self.fingerprint(configuration)) {
            throw MobileFault(code: "DEVICE_IDENTITY_MISMATCH", message: "Restore is still unconfirmed for the previous device. Restore that device from the trusted desktop before replacing its mobile pairing.")
        }
    }
    func mark(configuration: Data) throws {
        try validate(configuration: configuration)
        let marker = Marker(schemaVersion: 1, deviceFingerprint: try Self.fingerprint(configuration))
        try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        try JSONEncoder().encode(marker).write(to: file, options: [.atomic, .completeFileProtection])
        var url = file; var values = URLResourceValues(); values.isExcludedFromBackup = true
        try url.setResourceValues(values)
    }
    func acknowledgeClear(configuration: Data) throws {
        try validate(configuration: configuration)
        if FileManager.default.fileExists(atPath: file.path) { try FileManager.default.removeItem(at: file) }
    }
    private func journalError() -> MobileFault { MobileFault(code: "RESTORE_STATE_UNAVAILABLE", message: "The last restoration state could not be read. Restore the selected iPhone from the trusted desktop before continuing.") }
}
