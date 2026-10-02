import Foundation

struct LocalTunnelProfile: Codable, Equatable, Sendable {
    let interfaceAddress: String
    let peerAddress: String
    private static func numeric(_ address: String) -> UInt32? {
        let parts = address.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count == 4 else { return nil }
        var value: UInt32 = 0
        for part in parts {
            guard let byte = UInt8(part), String(byte) == String(part) else { return nil }
            value = (value << 8) | UInt32(byte)
        }
        return value
    }
    var interfaceNumber: UInt32? { Self.numeric(interfaceAddress) }
    var peerNumber: UInt32? { Self.numeric(peerAddress) }
    var valid: Bool {
        guard let a = interfaceNumber, let b = peerNumber else { return false }
        func privateAddress(_ n: UInt32) -> Bool {
            n >> 24 == 10 || n >> 20 == 0xac1 || n >> 16 == 0xc0a8
        }
        return privateAddress(a) && privateAddress(b) && a != b && (a & ~3) == (b & ~3)
            && [UInt32(1), 2].contains(a & 3) && [UInt32(1), 2].contains(b & 3)
    }
}
