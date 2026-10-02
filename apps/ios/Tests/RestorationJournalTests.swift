import XCTest
@testable import MeridianLab

final class RestorationJournalTests: XCTestCase {
    private func configuration(device: String) throws -> Data {
        let plist: [String: Any] = ["private_key": Data(repeating: 0, count: 32), "public_key": Data(repeating: 0, count: 32), "identifier": "test-only"]
        let pairing = try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0)
        return try JSONEncoder().encode(MobileConfiguration(schemaVersion: 1, expectedDeviceId: device, remoteAddress: "127.0.0.1", remotePort: 1234, pairingRecord: pairing.base64EncodedString()))
    }
    func testPendingRestorationSurvivesReopeningAndCannotBeClearedForAnotherDevice() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = directory.appendingPathComponent("restore.json")
        let first = try configuration(device: "test-device-a")
        let other = try configuration(device: "test-device-b")
        try RestorationJournal(file: file).mark(configuration: first)
        let reopened = RestorationJournal(file: file)
        XCTAssertNotNil(try reopened.pending())
        XCTAssertThrowsError(try reopened.acknowledgeClear(configuration: other))
        XCTAssertNotNil(try reopened.pending())
        let text = try String(contentsOf: file, encoding: .utf8)
        XCTAssertFalse(text.contains("test-device-a"))
        XCTAssertFalse(text.contains("pairingRecord"))
        try reopened.acknowledgeClear(configuration: first)
        XCTAssertNil(try reopened.pending())
    }
    func testCorruptJournalCannotSilentlyBecomeIdle() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = directory.appendingPathComponent("restore.json")
        try Data("invalid".utf8).write(to: file)
        XCTAssertThrowsError(try RestorationJournal(file: file).pending())
        XCTAssertTrue(FileManager.default.fileExists(atPath: file.path))
    }
}
