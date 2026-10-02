import Foundation
import NetworkExtension

actor LocalConnection {
    private var manager: NETunnelProviderManager?
    private var generation: UInt64 = 0
    private var starting: (id: UUID, profile: LocalTunnelProfile, task: Task<Void, Error>)?

    func start(configuration: Data) async throws {
        let config = try JSONDecoder().decode(MobileConfiguration.self, from: configuration)
        try config.validate()
        guard let profile = config.localTunnel else { return } // Legacy Clear-only recovery.
        if let starting {
            guard starting.profile == profile else { throw fault("Wait for the current local connection to finish.") }
            return try await starting.task.value
        }
        let id = UUID(), current = generation
        let task = Task { try await self.prepare(profile, generation: current) }
        starting = (id, profile, task)
        defer { if starting?.id == id { starting = nil } }
        try await task.value
    }

    private func prepare(_ profile: LocalTunnelProfile, generation current: UInt64) async throws {
        do {
            let bundle = (Bundle.main.bundleIdentifier ?? "dev.meridianlab.ios") + ".local-tunnel"
            let all: [NETunnelProviderManager] = try await withCheckedThrowingContinuation { continuation in
                NETunnelProviderManager.loadAllFromPreferences { value, error in
                    if let error { continuation.resume(throwing: error) }
                    else { continuation.resume(returning: value ?? []) }
                }
            }
            try check(current)
            let matches = all.filter { ($0.protocolConfiguration as? NETunnelProviderProtocol)?.providerBundleIdentifier == bundle }
            guard matches.count <= 1 else { throw fault("More than one Meridian connection profile exists. Remove the duplicate Meridian profile in iOS Settings and retry.") }
            let selected = matches.first ?? NETunnelProviderManager()
            let old = selected.protocolConfiguration as? NETunnelProviderProtocol
            let same = (old?.providerConfiguration?["interfaceAddress"] as? String) == profile.interfaceAddress
                && (old?.providerConfiguration?["peerAddress"] as? String) == profile.peerAddress
                && old?.includeAllNetworks == false
            manager = selected
            if !same || !selected.isEnabled || selected.isOnDemandEnabled {
                guard ![NEVPNStatus.connected, .connecting, .reasserting].contains(selected.connection.status) else {
                    throw fault("Stop the existing Meridian local connection before changing its profile.")
                }
                let proto = NETunnelProviderProtocol()
                proto.providerBundleIdentifier = bundle
                proto.serverAddress = profile.peerAddress
                proto.disconnectOnSleep = false
                proto.includeAllNetworks = false
                proto.providerConfiguration = ["interfaceAddress": profile.interfaceAddress, "peerAddress": profile.peerAddress]
                selected.protocolConfiguration = proto
                selected.localizedDescription = "Meridian local developer connection"
                selected.isEnabled = true
                selected.isOnDemandEnabled = false
                selected.onDemandRules = nil
                // iOS presents its normal VPN approval. Only this app's profile is edited.
                try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
                    selected.saveToPreferences { error in
                        if let error { continuation.resume(throwing: error) } else { continuation.resume() }
                    }
                }
                try check(current)
                try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
                    selected.loadFromPreferences { error in
                        if let error { continuation.resume(throwing: error) } else { continuation.resume() }
                    }
                }
            }
            try check(current)
            if ![NEVPNStatus.connected, .connecting, .reasserting].contains(selected.connection.status) {
                try selected.connection.startVPNTunnel()
            }
            let deadline = ContinuousClock.now.advanced(by: .seconds(20))
            while selected.connection.status != .connected {
                try check(current)
                guard ContinuousClock.now < deadline else {
                    throw fault("The local connection did not start. Check the signed Network Extension and its iOS approval.")
                }
                try await Task.sleep(for: .milliseconds(200))
            }
            try check(current)
            // The interface alone cannot establish Ready. Rust still verifies
            // Lockdown/RSD identity and opens the real developer service.
        } catch {
            if generation == current { manager?.connection.stopVPNTunnel(); manager = nil }
            throw error
        }
    }
    func requireConnected() throws {
        guard manager?.connection.status == .connected else { throw fault("The local connection stopped. Tap Restore to reconnect this iPhone and clear any previous simulation.") }
    }
    func stop() {
        generation &+= 1
        starting?.task.cancel()
        starting = nil
        manager?.connection.stopVPNTunnel()
        manager = nil
    }
    private func check(_ current: UInt64) throws {
        try Task.checkCancellation()
        guard generation == current else { throw CancellationError() }
    }
    private func fault(_ message: String) -> MobileFault { MobileFault(code: "LOCAL_TUNNEL_UNAVAILABLE", message: message) }
}
