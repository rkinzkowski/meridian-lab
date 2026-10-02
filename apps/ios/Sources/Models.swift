import Foundation
import CoreLocation

struct GeoPoint: Codable, Equatable, Sendable {
    var latitude: Double
    var longitude: Double
    var coordinate: CLLocationCoordinate2D { .init(latitude: latitude, longitude: longitude) }
    init(_ coordinate: CLLocationCoordinate2D) { latitude = coordinate.latitude; longitude = coordinate.longitude }
    init(latitude: Double, longitude: Double) { self.latitude = latitude; self.longitude = longitude }
    var valid: Bool { latitude.isFinite && longitude.isFinite && abs(latitude) <= 90 && abs(longitude) <= 180 }
    static func parse(_ text: String) -> GeoPoint? {
        let pieces = text.split(separator: ",", omittingEmptySubsequences: false).map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
        guard pieces.count == 2, let latitude = Double(pieces[0]), let longitude = Double(pieces[1]) else { return nil }
        let point = GeoPoint(latitude: latitude, longitude: longitude)
        return point.valid ? point : nil
    }
}

struct MobileFault: Codable, Error, LocalizedError, Sendable {
    let code: String
    let message: String
    var errorDescription: String? { message }
}
struct RouteProgress: Codable, Sendable {
    var id: String
    var name: String
    var distanceMeters: Double
    var totalMeters: Double
    var speedMps: Double
    var paused: Bool
    var finished: Bool
    var loop: Bool
}
struct MobileStatus: Codable, Sendable {
    var phase: String
    var heldPoint: GeoPoint?
    var activeRoute: RouteProgress?
    var error: MobileFault?
    var physicalDeviceVerified: Bool
    var restoreRequired: Bool = false
    var clearConfirmed: Bool = false
    static let idle = MobileStatus(phase: "idle", physicalDeviceVerified: false)
    var ready: Bool { ["ready", "active"].contains(phase) }
    var label: String {
        switch phase {
        case "connecting": return "Preparing developer connection"
        case "ready": return "Developer session ready"
        case "active": return "Location simulation active"
        case "stopping": return "Restoring real location"
        case "stopped": return "Real location restored"
        case "failed": return "Connection needs attention"
        default: return "Developer connection is idle"
        }
    }
}
struct RustReply: Codable, Sendable { let ok: Bool; let result: MobileStatus?; let error: MobileFault? }
struct SavedPlace: Codable, Identifiable, Sendable {
    var id = UUID()
    var name: String
    var point: GeoPoint
    var createdAt = Date()
}
struct MobileConfiguration: Codable, Sendable {
    let schemaVersion: Int
    let expectedDeviceId: String
    let remoteAddress: String
    let remotePort: UInt16
    let pairingRecord: String
    var connectionMode: String?
    var localTunnel: LocalTunnelProfile?
    func validate() throws {
        guard [1, 2].contains(schemaVersion), (8...128).contains(expectedDeviceId.count), remotePort > 0,
              !remoteAddress.isEmpty, let bytes = Data(base64Encoded: pairingRecord), bytes.count > 0, bytes.count <= 49152,
              let plist = try PropertyListSerialization.propertyList(from: bytes, options: [], format: nil) as? [String: Any] else {
            throw MobileFault(code: "PAIRING_RECORD_INVALID", message: "Repair mobile setup from your trusted desktop.")
        }
        if schemaVersion == 2 {
            guard connectionMode == "localCoreDevice",
                  let localTunnel, localTunnel.valid, remoteAddress == localTunnel.peerAddress else {
                throw MobileFault(code: "LOCAL_SETUP_INVALID", message: "The on-device connection profile is invalid.")
            }
        } else if localTunnel != nil || (connectionMode != nil && connectionMode != "remotePairing") {
            throw MobileFault(code: "LOCAL_SETUP_INVALID", message: "Repair this older mobile setup before using the local connection.")
        }
        if connectionMode == "localCoreDevice" {
            func dataField(_ key: String, minimum: Int = 1) -> Bool {
                guard let value = plist[key] as? Data else { return false }
                return value.count >= minimum
            }
            guard remoteAddress == "10.83.0.1", remotePort == 62078,
                  localTunnel?.interfaceAddress == "10.83.0.2",
                  localTunnel?.peerAddress == "10.83.0.1",
                  plist["UDID"] as? String == expectedDeviceId,
                  dataField("DeviceCertificate"), dataField("HostPrivateKey"),
                  dataField("HostCertificate"), dataField("RootPrivateKey"),
                  dataField("RootCertificate"),
                  let hostID = plist["HostID"] as? String, !hostID.isEmpty,
                  let systemBUID = plist["SystemBUID"] as? String, !systemBUID.isEmpty,
                  let wifiMAC = plist["WiFiMACAddress"] as? String, !wifiMAC.isEmpty else {
                throw MobileFault(code: "PAIRING_RECORD_INVALID", message: "This local setup needs the legitimate pairing record for this iPhone.")
            }
        } else {
            guard (plist["private_key"] as? Data)?.count == 32, (plist["public_key"] as? Data)?.count == 32,
                  let identifier = plist["identifier"] as? String, !identifier.isEmpty else {
                throw MobileFault(code: "PAIRING_RECORD_INVALID", message: "The saved RemotePairing record is incomplete.")
            }
        }
    }
}
