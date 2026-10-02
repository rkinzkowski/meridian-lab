import SwiftUI
import MapKit

@MainActor final class MapCoordinator: NSObject, ObservableObject, MKLocalSearchCompleterDelegate {
    @Published var status = MobileStatus.idle
    @Published var selected: SavedPlace?
    @Published var query = "" { didSet { completer.queryFragment = query } }
    @Published var completions: [MKLocalSearchCompletion] = []
    @Published var geometry: [CLLocationCoordinate2D] = []
    @Published var fault: MobileFault?
    @Published var busy = false
    @Published var speed = 1.4
    @Published var camera: MapCameraPosition = .automatic
    let library = PlaceLibrary()
    private let bridge = DeveloperSessionBridge()
    private let pairing = PairingStore()
    private let completer = MKLocalSearchCompleter()
    private var directions: MKDirections?
    private var monitor: Task<Void, Never>?
    private var generation: UInt64 = 0
    private var routeGeneration: UInt64 = 0
    private var searchGeneration: UInt64 = 0
    private var busyOwner: UUID?
    private var routeCalculationOwner: UUID?
    private var restoring = false
    private var operationRevision: UInt64 = 0

    override init() { super.init(); completer.delegate = self; completer.resultTypes = [.address, .pointOfInterest] }
    func completerDidUpdateResults(_ completer: MKLocalSearchCompleter) { completions = Array(completer.results.prefix(6)) }
    func completer(_ completer: MKLocalSearchCompleter, didFailWithError error: Error) { fault = MobileFault(code: "SEARCH_UNAVAILABLE", message: "Map search is unavailable. You can enter latitude, longitude or select a point on the map.") }

    func select(_ coordinate: CLLocationCoordinate2D, name: String = "Selected point") {
        searchGeneration &+= 1
        cancelRouteCalculation()
        geometry = []
        selected = SavedPlace(name: name, point: GeoPoint(coordinate)); completions = []
    }
    func search(_ completion: MKLocalSearchCompletion? = nil) async {
        searchGeneration &+= 1; let current = searchGeneration
        if let point = GeoPoint.parse(query) { select(point.coordinate, name: "Coordinate"); focus(point.coordinate); return }
        let request = completion.map(MKLocalSearch.Request.init(completion:)) ?? MKLocalSearch.Request()
        if completion == nil { request.naturalLanguageQuery = query }
        do {
            let response = try await MKLocalSearch(request: request).start()
            guard current == searchGeneration, let item = response.mapItems.first else { return }
            select(item.placemark.coordinate, name: item.name ?? "Search result"); focus(item.placemark.coordinate)
        } catch { if current == searchGeneration { fault = MobileFault(code: "SEARCH_UNAVAILABLE", message: "No map result is available. Try a different address or enter latitude, longitude.") } }
    }
    func focus(_ coordinate: CLLocationCoordinate2D) { camera = .region(.init(center: coordinate, span: .init(latitudeDelta: 0.02, longitudeDelta: 0.02))) }

    func connect() async {
        guard let owner = beginOperation() else { return }
        defer { endOperation(owner) }
        fault = nil
        generation &+= 1; let current = generation
        status.phase = "connecting"
        do {
            let config = try await pairing.load()
            let ready = try await bridge.prepare(configuration: config)
            guard current == generation else { return }
            status = ready; startMonitor()
        } catch { if current == generation { present(error); status.phase = "failed"; _ = try? await bridge.stop() } }
    }
    func inspectRestoration() async {
        guard !busy, status.phase == "idle" else { return }
        do {
            if try await bridge.pendingRestoration() {
                status.phase = "failed"
                status.restoreRequired = true
                fault = MobileFault(code: "RESTORE_UNCONFIRMED", message: "A previous session ended without a confirmed Clear. Tap Restore to reconnect the same paired device and clear it safely.")
            }
        } catch { status.phase = "failed"; present(error) }
    }
    func teleport() async {
        guard let selected, !busy else { return }
        if !status.ready { await connect(); guard status.ready else { return } }
        guard let owner = beginOperation() else { return }
        defer { endOperation(owner) }
        let current = generation
        do {
            let next = try await send(["method": "set", "point": ["latitude": selected.point.latitude, "longitude": selected.point.longitude]])
            if generation == current { status = next; geometry = []; library.record(selected) }
        } catch { if generation == current { present(error) } }
    }
    func restore() async {
        guard !restoring else { return }
        restoring = true
        generation &+= 1; cancelRouteCalculation(); monitor?.cancel(); monitor = nil
        // Restore supersedes an earlier operation. Its owner token prevents an old
        // completion from removing this operation's progress state.
        let owner = UUID(); busyOwner = owner
        operationRevision &+= 1
        defer { endOperation(owner); restoring = false }
        busy = true; status.phase = "stopping"
        do { status = try await bridge.stop(); fault = nil; geometry = [] }
        catch { present(error); status.phase = "failed" }
    }
    func route() async {
        guard let origin = status.heldPoint, let target = selected, status.ready, !busy else {
            fault = MobileFault(code: "ROUTE_START_REQUIRED", message: "Teleport to the route's starting point, then select its destination."); return
        }
        guard let owner = beginOperation() else { return }
        routeGeneration &+= 1; let current = routeGeneration; let sessionGeneration = generation
        routeCalculationOwner = owner
        defer {
            if routeCalculationOwner == owner { routeCalculationOwner = nil }
            if current == routeGeneration { directions = nil }
            endOperation(owner)
        }
        directions?.cancel()
        let request = MKDirections.Request()
        request.source = MKMapItem(placemark: .init(coordinate: origin.coordinate))
        request.destination = MKMapItem(placemark: .init(coordinate: target.point.coordinate))
        request.transportType = speed <= 3 ? .walking : .automobile
        let operation = MKDirections(request: request); directions = operation
        do {
            let result = try await operation.calculate()
            guard current == routeGeneration, sessionGeneration == generation else { return }
            guard let found = result.routes.first else { throw MobileFault(code: "ROUTE_UNAVAILABLE", message: "No road route is available between these points.") }
            let count = found.polyline.pointCount
            guard count >= 2 && count <= 10000 else { throw MobileFault(code: "ROUTE_TOO_LARGE", message: "Choose a shorter route for this mobile session.") }
            var coordinates = [CLLocationCoordinate2D](repeating: .init(), count: count)
            found.polyline.getCoordinates(&coordinates, range: NSRange(location: 0, length: count))
            let spec: [String: Any] = ["id": UUID().uuidString, "name": target.name, "speedMps": speed, "loop": false, "points": coordinates.map { ["latitude": $0.latitude, "longitude": $0.longitude] }]
            // Selection changes can cancel MapKit calculation. Once dispatched,
            // Rust owns the operation; keep controls busy until its reply or Stop.
            routeCalculationOwner = nil
            let next = try await send(["method": "route", "spec": spec])
            if sessionGeneration == generation {
                status = next
                if current == routeGeneration { geometry = coordinates }
            }
        } catch { if current == routeGeneration, sessionGeneration == generation { present(error) } }
    }
    func pauseOrResume() async { await command(["method": status.activeRoute?.paused == true ? "resume" : "pause"]) }
    func changeSpeed() async { if status.activeRoute != nil { await command(["method": "speed", "speedMps": speed]) } }
    func joystick(_ bearing: Double) async { await command(["method": "joystick", "bearing": bearing, "meters": max(1, speed)]) }
    private func command(_ value: [String: Any]) async {
        guard status.ready, let owner = beginOperation() else { return }
        defer { endOperation(owner) }
        let current = generation
        do { let next = try await send(value); if current == generation { status = next } }
        catch { if current == generation { present(error) } }
    }
    private func beginOperation() -> UUID? {
        guard !busy, !restoring else { return nil }
        let owner = UUID(); busyOwner = owner; busy = true
        operationRevision &+= 1
        return owner
    }
    private func endOperation(_ owner: UUID) {
        guard busyOwner == owner else { return }
        busyOwner = nil; busy = false
    }
    private func cancelRouteCalculation() {
        routeGeneration &+= 1; directions?.cancel(); directions = nil
        if let owner = routeCalculationOwner { endOperation(owner); routeCalculationOwner = nil }
    }
    private func send(_ value: [String: Any]) async throws -> MobileStatus { try await bridge.request(JSONSerialization.data(withJSONObject: value)) }
    private func present(_ error: Error) { fault = (error as? MobileFault) ?? MobileFault(code: "OPERATION_FAILED", message: "The operation could not finish. Check the connection and retry.") }
    private func startMonitor() {
        monitor?.cancel()
        let current = generation
        monitor = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                guard !Task.isCancelled, let self, self.generation == current else { return }
                if self.busy { continue }
                let revision = self.operationRevision
                do {
                    let next = try await self.send(["method": "status"])
                    guard !Task.isCancelled, self.generation == current else { return }
                    if self.busy || self.operationRevision != revision { continue }
                    self.status = next
                    if let error = next.error { self.fault = error }
                    if next.phase == "failed" { return }
                } catch {
                    guard !Task.isCancelled, self.generation == current else { return }
                    if self.busy || self.operationRevision != revision { continue }
                    self.present(error); return
                }
            }
        }
    }
    var diagnostics: String { "Meridian Lab 0.1.0\niOS \(UIDevice.current.systemVersion)\nNative core: idevice 0.1.65\nSession: \(status.phase)\nLast error: \(fault?.code ?? "none")\nNative physical acceptance: UNVERIFIED\nPairing, device identifiers, and coordinates omitted." }
}
