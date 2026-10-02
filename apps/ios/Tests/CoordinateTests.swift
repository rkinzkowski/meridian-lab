import XCTest
@testable import MeridianLab

final class CoordinateTests: XCTestCase {
    func testCoordinateBoundsAndNonfiniteInputs() {
        XCTAssertEqual(GeoPoint.parse("90, -180"), GeoPoint(latitude: 90, longitude: -180))
        for text in ["91,0", "0,-181", "nan,0", "inf,0", "1,2,3", "1", ","] { XCTAssertNil(GeoPoint.parse(text), text) }
    }
    func testNetworkAvailabilityDoesNotSatisfyReadiness() {
        for phase in ["idle", "connecting", "stopping", "stopped", "failed"] {
            XCTAssertFalse(MobileStatus(phase: phase, physicalDeviceVerified: false).ready)
        }
    }
    func testMalformedPairingRejected() {
        let config = MobileConfiguration(schemaVersion: 1, expectedDeviceId: "device-a", remoteAddress: "127.0.0.1", remotePort: 1234, pairingRecord: "not-base64")
        XCTAssertThrowsError(try config.validate())
    }
}
