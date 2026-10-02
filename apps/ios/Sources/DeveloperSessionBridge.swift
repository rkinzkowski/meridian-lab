import Foundation

actor DeveloperSessionBridge {
    private var handle: UInt64 = 0
    private var configuration: Data?
    private let journal = RestorationJournal()
    private let localConnection = LocalConnection()
    private var stopping: Task<MobileStatus, Error>?
    private var operationGeneration: UInt64 = 0

    func pendingRestoration() throws -> Bool { try journal.pending() != nil }

    func prepare(configuration: Data) async throws -> MobileStatus {
        guard stopping == nil else { throw MobileFault(code: "SESSION_BUSY", message: "Wait for Restore to finish before preparing another session.") }
        let generation = operationGeneration
        try journal.validate(configuration: configuration)
        self.configuration = configuration
        guard handle == 0 else { return try await call(["method": "prepare"]) }
        try await localConnection.start(configuration: configuration)
        if try journal.pending() != nil { _ = try await recoverClearOnly(configuration) }
        guard generation == operationGeneration, stopping == nil else { await localConnection.stop(); throw MobileFault(code: "CANCELED", message: "Connection preparation was canceled by Restore.") }
        handle = configuration.withUnsafeBytes { buffer in meridian_session_create(buffer.bindMemory(to: UInt8.self).baseAddress, buffer.count) }
        guard handle != 0 else { throw MobileFault(code: "PAIRING_RECORD_INVALID", message: "Mobile pairing or the local endpoint is invalid. Repair mobile setup from your desktop.") }
        return try await call(["method": "prepare"])
    }
    func request(_ data: Data) async throws -> MobileStatus {
        let currentHandle = handle
        guard currentHandle != 0 else { throw MobileFault(code: "SESSION_NOT_READY", message: "Prepare the developer connection first.") }
        struct Method: Decodable { let method: String }
        let method = try JSONDecoder().decode(Method.self, from: data).method
        if ["set", "route", "joystick"].contains(method) {
            guard stopping == nil else { throw MobileFault(code: "SESSION_BUSY", message: "Restore is in progress.") }
            guard let configuration else { throw unconfirmed() }
            // Persist before dispatch: process termination or a lost reply cannot
            // erase the fact that the device may now be simulating a location.
            try journal.mark(configuration: configuration)
        }
        // Awaiting a detached call keeps this actor reentrant: Stop can cancel an
        // in-progress Prepare through the Rust watch channel instead of queuing behind it.
        let status = try await Task.detached(priority: .userInitiated) {
            let raw = data.withUnsafeBytes { bytes in meridian_session_call(currentHandle, bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count) }
            guard let raw else { throw MobileFault(code: "INTERNAL_ERROR", message: "The native session could not return a response.") }
            defer { meridian_string_free(raw) }
            let reply = try JSONDecoder().decode(RustReply.self, from: Data(String(cString: raw).utf8))
            guard reply.ok, let status = reply.result else { throw reply.error ?? MobileFault(code: "INTERNAL_ERROR", message: "The native developer operation failed.") }
            return status
        }.value
        // Status snapshots may predate an in-flight Set. Only a reply to an
        // explicit clearing operation is allowed to remove the durable marker.
        if ["clear", "stop", "restoreOnly"].contains(method), status.clearConfirmed, !status.restoreRequired, let configuration { try journal.acknowledgeClear(configuration: configuration) }
        return status
    }
    private func call(_ value: [String: String]) async throws -> MobileStatus { try await request(JSONEncoder().encode(value)) }
    func stop() async throws -> MobileStatus {
        if let stopping { return try await stopping.value }
        operationGeneration &+= 1
        let task = Task { try await self.performStop() }
        stopping = task
        defer { stopping = nil }
        let result = try await task.value
        if !result.restoreRequired, try journal.pending() == nil { await localConnection.stop() }
        return result
    }
    private func performStop() async throws -> MobileStatus {
        guard handle != 0 else {
            guard try journal.pending() != nil else { return .idle }
            return try await recoverClearOnly(try await recoveryConfiguration())
        }
        let id = handle
        let stopped: MobileStatus
        do { stopped = try await call(["method": "stop"]) }
        catch {
            // Keep a timed-out live handle: its owned worker may still be clearing.
            // Reconnect only after the old worker reports its terminal failed state.
            guard let terminal = try? await call(["method": "status"]), terminal.phase == "failed" else { throw unconfirmed() }
            if terminal.restoreRequired, let configuration { try journal.mark(configuration: configuration) }
            release(id)
            guard try journal.pending() != nil else { throw error }
            return try await recoverClearOnly(try await recoveryConfiguration())
        }
        release(id)
        if try journal.pending() != nil { return try await recoverClearOnly(try await recoveryConfiguration()) }
        return stopped
    }
    private func recoveryConfiguration() async throws -> Data {
        let data: Data
        if let configuration { data = configuration } else { data = try await PairingStore().load() }
        try journal.validate(configuration: data)
        configuration = data
        return data
    }
    private func recoverClearOnly(_ configuration: Data) async throws -> MobileStatus {
        try journal.validate(configuration: configuration)
        guard handle == 0 else { throw unconfirmed() }
        self.configuration = configuration
        try await localConnection.start(configuration: configuration)
        handle = configuration.withUnsafeBytes { bytes in meridian_session_create(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count) }
        guard handle != 0 else { throw unconfirmed() }
        let id = handle
        do {
            let restored = try await call(["method": "restoreOnly"])
            guard restored.clearConfirmed, !restored.restoreRequired else { throw unconfirmed() }
            release(id)
            return restored
        } catch {
            if let state = try? await call(["method": "status"]), ["failed", "stopped"].contains(state.phase) { release(id) }
            throw unconfirmed()
        }
    }
    private func release(_ id: UInt64) { meridian_session_destroy(id); if handle == id { handle = 0 } }
    private func unconfirmed() -> MobileFault {
        MobileFault(code: "RESTORE_UNCONFIRMED", message: "Clear has not been confirmed. Reconnect this same paired iPhone and retry Restore. Recovery only clears location; it does not replay a previous point or route.")
    }
}
