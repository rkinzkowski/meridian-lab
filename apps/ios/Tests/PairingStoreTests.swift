import XCTest
import Security
@testable import MeridianLab

final class PairingStoreTests: XCTestCase {
    private func configuration(_ device: String) throws -> Data {
        let plist: [String: Any] = ["private_key": Data(repeating: 0, count: 32), "public_key": Data(repeating: 0, count: 32), "identifier": "test-only"]
        let pairing = try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0)
        return try JSONEncoder().encode(MobileConfiguration(schemaVersion: 1, expectedDeviceId: device, remoteAddress: "127.0.0.1", remotePort: 1234, pairingRecord: pairing.base64EncodedString()))
    }
    private func fixture() throws -> (PairingStore, RestorationJournal, URL) {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let service = "MeridianLab.Tests.\(UUID().uuidString)"
        let inbox = directory.appendingPathComponent("meridian-device.json")
        let journal = RestorationJournal(file: directory.appendingPathComponent("restore.json"))
        addTeardownBlock {
            SecItemDelete([kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service] as CFDictionary)
            try? FileManager.default.removeItem(at: directory)
        }
        return (PairingStore(inbox: inbox, service: service, journal: journal), journal, inbox)
    }
    func testPendingRecoveryKeepsSavedPairingAndDefersReplacementUntilClear() async throws {
        let (store, journal, inbox) = try fixture()
        let original = try configuration("test-device-original")
        let replacement = try configuration("test-device-replacement")
        try original.write(to: inbox)
        let imported = try await store.load()
        XCTAssertEqual(imported, original)
        try journal.mark(configuration: original)
        try replacement.write(to: inbox)

        let recovery = try await store.load()
        XCTAssertEqual(recovery, original)
        XCTAssertEqual(try Data(contentsOf: inbox), replacement)
        do { try await store.forget(); XCTFail("Pending recovery must retain its credentials") }
        catch { XCTAssertEqual((error as? MobileFault)?.code, "RESTORE_UNCONFIRMED") }

        try journal.acknowledgeClear(configuration: original)
        let updated = try await store.load()
        XCTAssertEqual(updated, replacement)
        XCTAssertFalse(FileManager.default.fileExists(atPath: inbox.path))
        let saved = try await store.load()
        XCTAssertEqual(saved, replacement)
    }
    func testMissingSavedPairingCannotImportAnotherDeviceWhileRecoveryIsPending() async throws {
        let (store, journal, inbox) = try fixture()
        let original = try configuration("test-device-original")
        let replacement = try configuration("test-device-replacement")
        try journal.mark(configuration: original)
        try replacement.write(to: inbox)
        do { _ = try await store.load(); XCTFail("Wrong-device import must fail") }
        catch { XCTAssertEqual((error as? MobileFault)?.code, "DEVICE_IDENTITY_MISMATCH") }
        XCTAssertEqual(try Data(contentsOf: inbox), replacement)
        XCTAssertNotNil(try journal.pending())

        try original.write(to: inbox)
        let recovered = try await store.load()
        XCTAssertEqual(recovered, original)
        XCTAssertNotNil(try journal.pending())
    }
}
