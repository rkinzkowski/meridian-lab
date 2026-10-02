import SwiftUI
import MapKit
import UniformTypeIdentifiers
import UIKit

@main struct MeridianApp: App {
    @StateObject private var model = MapCoordinator()
    @Environment(\.scenePhase) private var scenePhase
    var body: some Scene {
        WindowGroup { MeridianMapView(model: model)
            .task { await model.inspectRestoration() }
            .onChange(of: scenePhase) { _, phase in
                if phase == .background {
                    // No claim of indefinite background execution. Make a bounded attempt
                    // to clear before iOS suspends this foreground development tool.
                    let task = UIApplication.shared.beginBackgroundTask(withName: "Restore simulated location")
                    Task { await model.restore(); if task != .invalid { UIApplication.shared.endBackgroundTask(task) } }
                }
            }
        }
    }
}

struct MeridianMapView: View {
    @ObservedObject var model: MapCoordinator
    @State private var showLibrary = false
    @State private var showSetup = false
    var body: some View {
        NavigationStack {
            MapReader { reader in
                Map(position: $model.camera) {
                    if let selected = model.selected { Marker(selected.name, coordinate: selected.point.coordinate).tint(.orange) }
                    if let point = model.status.heldPoint { Annotation("Simulated location", coordinate: point.coordinate) { Image(systemName: "location.fill").padding(10).background(.mint, in: Circle()).foregroundStyle(.black).accessibilityLabel("Simulated location") } }
                    if model.geometry.count >= 2 { MapPolyline(coordinates: model.geometry).stroke(.mint, lineWidth: 5) }
                }
                .mapControls { MapCompass(); MapScaleView() }
                .onTapGesture { position in if let coordinate = reader.convert(position, from: .local) { model.select(coordinate) } }
                .safeAreaInset(edge: .top) { search.padding(.horizontal).padding(.top, 8) }
                .safeAreaInset(edge: .bottom) { controls.padding(12).background(.regularMaterial, in: RoundedRectangle(cornerRadius: 24)).padding(12) }
            }
            .navigationTitle("Meridian Lab")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) { Button("Saved", systemImage: "bookmark") { showLibrary = true } }
                ToolbarItem(placement: .topBarTrailing) { Button("Connection", systemImage: "cable.connector") { showSetup = true } }
            }
            .sheet(isPresented: $showLibrary) { LibraryView(library: model.library) { place in model.select(place.point.coordinate, name: place.name); model.focus(place.point.coordinate); showLibrary = false } }
            .sheet(isPresented: $showSetup) { setup }
            .onOpenURL { url in
                if url.scheme == "meridianlab", url.host == "connection" { showSetup = true }
            }
        }
    }
    private var search: some View {
        VStack(spacing: 0) {
            HStack {
                Image(systemName: "magnifyingglass")
                TextField("Search or latitude, longitude", text: $model.query).textInputAutocapitalization(.never).autocorrectionDisabled().submitLabel(.search).onSubmit { Task { await model.search() } }
                if !model.query.isEmpty { Button("Clear", systemImage: "xmark.circle.fill") { model.query = ""; model.completions = [] }.labelStyle(.iconOnly) }
            }.padding(14)
            ForEach(Array(model.completions.enumerated()), id: \.offset) { _, completion in
                Button { Task { await model.search(completion) } } label: {
                    VStack(alignment: .leading) { Text(completion.title); Text(completion.subtitle).font(.caption).foregroundStyle(.secondary) }.frame(maxWidth: .infinity, alignment: .leading).padding(10)
                }.buttonStyle(.plain)
            }
        }.background(.regularMaterial, in: RoundedRectangle(cornerRadius: 18))
    }
    private var controls: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack { Circle().fill(model.status.ready ? .mint : .orange).frame(width: 8, height: 8); Text(model.status.label).font(.caption.weight(.semibold)); Spacer(); if model.busy { ProgressView() } }
            if let fault = model.fault { Text(fault.message).font(.caption).foregroundStyle(.orange).accessibilityLabel("Connection error: \(fault.message)") }
            if let selected = model.selected {
                HStack {
                    VStack(alignment: .leading) { Text(selected.name).font(.headline).lineLimit(1); Text(String(format: "%.5f, %.5f", selected.point.latitude, selected.point.longitude)).font(.caption.monospacedDigit()).foregroundStyle(.secondary) }
                    Spacer()
                    Button("Save", systemImage: "bookmark.badge.plus") { model.library.save(selected) }.labelStyle(.iconOnly)
                }
                HStack {
                    Button("Teleport", systemImage: "location.fill") { Task { await model.teleport() } }.buttonStyle(.borderedProminent).tint(.mint).disabled(model.busy)
                    Button("Route", systemImage: "point.topleft.down.to.point.bottomright.curvepath") { Task { await model.route() } }.buttonStyle(.bordered).disabled(model.busy || model.status.heldPoint == nil)
                    Spacer()
                }
            } else { Text("Select a destination on the map.").font(.subheadline) }
            if model.status.phase != "idle" && model.status.phase != "stopped" {
                Button("Restore", role: .destructive) { Task { await model.restore() } }.disabled(model.busy)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            if let route = model.status.activeRoute {
                ProgressView(value: route.distanceMeters, total: max(1, route.totalMeters))
                HStack { Text(route.finished ? "Route complete · holding endpoint" : String(format: "%.0f of %.0f m", route.distanceMeters, route.totalMeters)).font(.caption); Spacer(); if !route.finished { Button(route.paused ? "Resume" : "Pause") { Task { await model.pauseOrResume() } }.disabled(model.busy) } }
            }
            if model.status.ready {
                HStack {
                    Text("Speed").font(.caption)
                    Slider(value: $model.speed, in: 0.5...35, onEditingChanged: { editing in if !editing { Task { await model.changeSpeed() } } }).accessibilityLabel("Movement speed in metres per second")
                    Text(String(format: "%.1f m/s", model.speed)).font(.caption.monospacedDigit()).frame(width: 65)
                }
                HStack { Text("Move").font(.caption); Spacer(); move("West", "arrow.left", 270); move("North", "arrow.up", 0); move("South", "arrow.down", 180); move("East", "arrow.right", 90) }
            }
        }
    }
    private func move(_ label: String, _ image: String, _ bearing: Double) -> some View { Button(label, systemImage: image) { Task { await model.joystick(bearing) } }.labelStyle(.iconOnly).buttonStyle(.bordered).disabled(model.busy || model.status.heldPoint == nil) }
    private var setup: some View {
        NavigationStack { Form {
            Section("Developer connection") {
                Text(model.status.label)
                Text("Mobile developer simulation is unverified on this iOS build. Setup needs a legitimate pairing record and a reachable local developer endpoint. The desktop path remains available.").font(.footnote)
                Button("Prepare connection") { Task { await model.connect() } }.disabled(model.busy)
                Button("Stop and restore", role: .destructive) { Task { await model.restore() } }
            }
            Section("Integrated local connection") {
                Text("Meridian Lab configures its signed local Network Extension when available. A legitimate pairing record, a reachable local developer endpoint, and Apple provisioning are still required. Meridian Bridge is an optional local-network diagnostic companion.").font(.footnote)
                Link("Open Meridian Bridge diagnostics", destination: URL(string: "meridianbridge://diagnostics")!)
            }
            Section("Diagnostics") { Text(model.diagnostics).font(.caption.monospaced()); ShareLink("Share sanitized diagnostics", item: model.diagnostics) }
        }.navigationTitle("Connection").toolbar { Button("Done") { showSetup = false } } }
    }
}

struct PlacesExport: FileDocument {
    static var readableContentTypes: [UTType] { [.json] }
    var data: Data
    init(data: Data) { self.data = data }
    init(configuration: ReadConfiguration) throws { data = configuration.file.regularFileContents ?? Data() }
    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper { FileWrapper(regularFileWithContents: data) }
}
struct LibraryView: View {
    @ObservedObject var library: PlaceLibrary
    let select: (SavedPlace) -> Void
    @State private var exporting = false
    @State private var export = PlacesExport(data: Data())
    var body: some View {
        NavigationStack { List {
            if let error = library.error { Text(error).foregroundStyle(.orange) }
            Section("Saved places") { ForEach(library.archive.places) { place in Button(place.name) { select(place) } }.onDelete { indices in library.delete(Set(indices.map { library.archive.places[$0].id })) } }
            Section("Recents") { ForEach(library.archive.recents) { place in Button(place.name) { select(place) } } }
            Section("Privacy") {
                Toggle("Keep location history on this iPhone", isOn: Binding(get: { library.archive.historyEnabled }, set: { library.historyEnabled($0) }))
                Button("Clear history", role: .destructive) { library.clearHistory() }
                Button("Export saved places") { do { export = PlacesExport(data: try library.export()); exporting = true } catch { library.error = "Saved places could not be exported." } }
            }
        }.navigationTitle("Your places").fileExporter(isPresented: $exporting, document: export, contentType: .json, defaultFilename: "Meridian-places") { result in if case .failure = result { library.error = "Saved places export did not finish." } } }
    }
}
