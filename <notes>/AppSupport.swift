import Foundation

// Своя папка в Application Support. Без песочницы (выключена 01.11.2026 ради автовставки надиктовки)
// Application Support общий для всех программ — поэтому подпапка по идентификатору приложения.
// Модели и расшифровки, скачанные и сделанные в песочнице, при первом запуске переезжают сюда.
enum AppSupport {
    nonisolated static let folder: URL = {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let url = base.appendingPathComponent(Bundle.main.bundleIdentifier ?? "notes", isDirectory: true)
        try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }()

    // Из контейнера песочницы — файлы Application Support (переносом: модели весят гигабайты) и настройки.
    // Своя база не появляется: это то же, что было в контейнере, только в новом месте.
    static func migrate() {
        guard let id = Bundle.main.bundleIdentifier, !UserDefaults.standard.bool(forKey: "migratedFromSandbox") else { return }
        #if DEBUG
        // Проверочные запуски не переносят данные: у Макара в это время может работать сборка из песочницы.
        if UserDefaults.standard.bool(forKey: "testVault") { return }
        #endif
        let home = FileManager.default.homeDirectoryForCurrentUser
        let container = home.appendingPathComponent("Library/Containers/\(id)/Data/Library", isDirectory: true)
        let old = container.appendingPathComponent("Application Support", isDirectory: true)
        for name in (try? FileManager.default.contentsOfDirectory(atPath: old.path)) ?? [] {
            let target = folder.appendingPathComponent(name)
            guard !FileManager.default.fileExists(atPath: target.path) else { continue }
            try? FileManager.default.moveItem(at: old.appendingPathComponent(name), to: target)
        }
        let prefs = container.appendingPathComponent("Preferences/\(id).plist")
        if let values = NSDictionary(contentsOf: prefs) as? [String: Any] {
            for (key, value) in values where UserDefaults.standard.object(forKey: key) == nil {
                UserDefaults.standard.set(value, forKey: key)
            }
        }
        UserDefaults.standard.set(true, forKey: "migratedFromSandbox")
    }
}
