import SwiftUI
import Network

/// This companion measures available local networking. It does not manufacture
/// a VPN or advertise developer readiness from an interface or socket alone.
@MainActor final class BridgeDiagnostics: ObservableObject {
    @Published var pathSummary = "Checking local network"
    @Published var advertisements: [String] = []
    @Published var browseSummary = "Discovery is idle"
    private let monitor = NWPathMonitor()
    private var browser: NWBrowser?
    private let queue = DispatchQueue(label: "dev.meridianlab.bridge.network")
    init() {
        monitor.pathUpdateHandler = { [weak self] path in
            let summary: String
            if path.status != .satisfied { summary = "No usable network path" }
            else if path.usesInterfaceType(.wifi) { summary = "Wi-Fi path is available" }
            else if path.usesInterfaceType(.cellular) { summary = "Cellular path is available; developer connectivity is unverified" }
            else { summary = "A network path is available" }
            Task { @MainActor in self?.pathSummary = summary }
        }
        monitor.start(queue: queue)
    }
    func discover() {
        browser?.cancel(); advertisements = []
        let parameters = NWParameters.tcp
        parameters.includePeerToPeer = true
        let next = NWBrowser(for: .bonjour(type: "_remotepairing._tcp", domain: "local."), using: parameters)
        next.stateUpdateHandler = { [weak self] state in
            let text: String
            switch state {
            case .ready: text = "Browsing developer pairing advertisements"
            case .waiting: text = "Discovery is waiting for local network access"
            case .failed: text = "Discovery failed. Check Local Network permission in Settings."
            case .cancelled: text = "Discovery stopped"
            default: text = "Starting local discovery"
            }
            Task { @MainActor in self?.browseSummary = text }
        }
        next.browseResultsChangedHandler = { [weak self] results, _ in
            let names = results.compactMap { result -> String? in
                if case let .service(name, _, _, _) = result.endpoint { return name }; return nil
            }.sorted()
            Task { @MainActor in self?.advertisements = names }
        }
        browser = next; next.start(queue: queue)
    }
    func stop() { browser?.cancel(); browser = nil }
    deinit { monitor.cancel(); browser?.cancel() }
}

@main struct MeridianBridgeApp: App {
    @StateObject private var diagnostics = BridgeDiagnostics()
    var body: some Scene {
        WindowGroup {
            NavigationStack { Form {
                Section("Local connection") {
                    Label(diagnostics.pathSummary, systemImage: "network")
                    Text("This development companion measures local network conditions. An advertisement or network path does not establish a paired developer session.")
                }
                Section("Developer advertisements") {
                    Text(diagnostics.browseSummary)
                    Button("Inspect local discovery") { diagnostics.discover() }
                    Button("Stop discovery") { diagnostics.stop() }
                    ForEach(diagnostics.advertisements, id: \.self) { name in Text(name).privacySensitive() }
                }
                Section("Current capability") {
                    Text("No packet tunnel is included. The embedded Meridian core can establish a userspace developer tunnel only when iOS permits access to its paired endpoint. Self-device routing and iOS 27 beta compatibility require physical validation.")
                    Text("A Network Extension variant would require legitimate Apple provisioning and an independently verified routing design. This build does not request or emulate unavailable entitlements.")
                    Link("Return to Meridian Lab", destination: URL(string: "meridianlab://connection")!)
                }
            }.navigationTitle("Meridian Bridge") }
        }
    }
}
