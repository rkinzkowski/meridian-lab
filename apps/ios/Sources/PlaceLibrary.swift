import Foundation
import Combine

@MainActor final class PlaceLibrary: ObservableObject {
    struct Archive: Codable { var schemaVersion = 1; var places: [SavedPlace] = []; var recents: [SavedPlace] = []; var historyEnabled = true }
    @Published private(set) var archive = Archive()
    @Published var error: String?
    private let file: URL
    init() {
        file = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("MeridianLab/library.json")
        if FileManager.default.fileExists(atPath: file.path) {
            do {
                let decoded = try JSONDecoder().decode(Archive.self, from: Data(contentsOf: file))
                guard decoded.schemaVersion == 1 else { throw CocoaError(.coderReadCorrupt) }
                archive = decoded
            } catch { self.error = "Saved places could not be read. Your existing file has been preserved." }
        }
    }
    func save(_ place: SavedPlace) { archive.places.append(place); persist() }
    func record(_ place: SavedPlace) { guard archive.historyEnabled else { return }; archive.recents.insert(place, at: 0); archive.recents = Array(archive.recents.prefix(100)); persist() }
    func delete(_ ids: Set<UUID>) { archive.places.removeAll { ids.contains($0.id) }; persist() }
    func clearHistory() { archive.recents.removeAll(); persist() }
    func historyEnabled(_ enabled: Bool) { archive.historyEnabled = enabled; persist() }
    func export() throws -> Data { try JSONEncoder().encode(archive.places) }
    private func persist() {
        do {
            try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
            try JSONEncoder().encode(archive).write(to: file, options: [.atomic, .completeFileProtection])
            var url = file; var values = URLResourceValues(); values.isExcludedFromBackup = true; try url.setResourceValues(values)
        } catch { self.error = "Changes to saved places could not be stored on this iPhone." }
    }
}
