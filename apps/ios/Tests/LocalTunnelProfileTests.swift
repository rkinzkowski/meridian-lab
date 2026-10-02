import XCTest
@testable import MeridianLab

final class LocalTunnelProfileTests: XCTestCase {
    func testDeveloperProfileUsesTheTwoPrivateAddressesInOneSlash30() {
        let profile = LocalTunnelProfile(interfaceAddress: "10.83.0.2", peerAddress: "10.83.0.1")

        XCTAssertTrue(profile.valid)
        XCTAssertEqual(profile.interfaceNumber, 0x0a530002)
        XCTAssertEqual(profile.peerNumber, 0x0a530001)
    }

    func testProfileRejectsPublicAndMalformedOrSameAddresses() {
        for profile in [
            LocalTunnelProfile(interfaceAddress: "8.8.8.8", peerAddress: "8.8.8.9"),
            LocalTunnelProfile(interfaceAddress: "10.83.0.2", peerAddress: "10.83.1.1"),
            LocalTunnelProfile(interfaceAddress: "10.83.0.1", peerAddress: "10.83.0.1"),
            LocalTunnelProfile(interfaceAddress: "10.83.0.02", peerAddress: "10.83.0.1"),
        ] {
            XCTAssertFalse(profile.valid)
        }
    }
}
