import Foundation
import NetworkExtension

/// A local, two-address developer interface. It never forwards packets to an
/// internet server, changes DNS, or installs a default route.
final class PacketTunnelProvider: NEPacketTunnelProvider {
    private let packetQueue = DispatchQueue(label: "dev.meridianlab.local-packets")
    private var running = false
    private var epoch: UInt64 = 0
    override func startTunnel(options: [String: NSObject]?, completionHandler: @escaping (Error?) -> Void) {
        guard let proto = protocolConfiguration as? NETunnelProviderProtocol,
              let config = proto.providerConfiguration,
              let a = config["interfaceAddress"] as? String,
              let b = config["peerAddress"] as? String else {
            completionHandler(NSError(domain: "MeridianLocalTunnel", code: 1)); return
        }
        let profile = LocalTunnelProfile(interfaceAddress: a, peerAddress: b)
        guard profile.valid, let interface = profile.interfaceNumber, let peer = profile.peerNumber else {
            completionHandler(NSError(domain: "MeridianLocalTunnel", code: 2)); return
        }
        let settings = NEPacketTunnelNetworkSettings(tunnelRemoteAddress: b)
        let ipv4 = NEIPv4Settings(addresses: [a], subnetMasks: ["255.255.255.252"])
        ipv4.includedRoutes = [NEIPv4Route(destinationAddress: b, subnetMask: "255.255.255.255")]
        settings.ipv4Settings = ipv4
        settings.mtu = 1500
        packetQueue.async {
            self.running = false
            self.epoch &+= 1
            let epoch = self.epoch
            self.setTunnelNetworkSettings(settings) { error in
                self.packetQueue.async {
                    // Stop may precede the settings callback; an obsolete
                    // callback must never restart the packet loop.
                    guard self.epoch == epoch else {
                        completionHandler(NSError(domain: "MeridianLocalTunnel", code: 3,
                            userInfo: [NSLocalizedDescriptionKey: "Local connection start was canceled."]))
                        return
                    }
                    guard error == nil else { completionHandler(error); return }
                    self.running = true
                    self.readPackets(interface: interface, peer: peer, epoch: epoch)
                    completionHandler(nil)
                }
            }
        }
    }
    private func readPackets(interface: UInt32, peer: UInt32, epoch: UInt64) {
        guard running, self.epoch == epoch else { return }
        packetFlow.readPackets { packets, families in
            self.packetQueue.async {
                guard self.running, self.epoch == epoch else { return }
                var reflected: [Data] = []
                for (packet, family) in zip(packets, families) where family.int32Value == AF_INET {
                    guard packet.count <= 65535 else { continue }
                    var data = packet
                    let count = data.count
                    let accepted = data.withUnsafeMutableBytes { bytes in
                        meridian_reflect_local_tcp(bytes.bindMemory(to: UInt8.self).baseAddress, count, interface, peer)
                    }
                    if accepted { reflected.append(data) }
                }
                if !reflected.isEmpty {
                    self.packetFlow.writePackets(reflected, withProtocols: Array(repeating: NSNumber(value: AF_INET), count: reflected.count))
                }
                self.readPackets(interface: interface, peer: peer, epoch: epoch)
            }
        }
    }
    override func stopTunnel(with reason: NEProviderStopReason, completionHandler: @escaping () -> Void) {
        packetQueue.async {
            self.running = false
            self.epoch &+= 1
            completionHandler()
        }
    }
}
