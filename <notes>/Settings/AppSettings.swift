import AppKit
import ServiceManagement

// Подробные настройки (ТЗ 31.10.2026: «прям подробные… чтобы всё досконально»). Всё в UserDefaults
// приложения — в хранилище настройки не пишем. Каждая настройка здесь реально что-то меняет.
enum AppSettings {
    enum Theme: String, CaseIterable {
        case system = "как в системе", light = "светлая", dark = "тёмная"
    }

    static var theme: Theme {
        get { UserDefaults.standard.string(forKey: "theme").flatMap(Theme.init(rawValue:)) ?? .system }
        set { UserDefaults.standard.set(newValue.rawValue, forKey: "theme"); applyTheme() }
    }

    // Наплыв темы (ThemeCrossfade) срабатывает и тут: он следит за effectiveAppearance.
    static func applyTheme() {
        switch theme {
        case .system: NSApp.appearance = nil
        case .light: NSApp.appearance = NSAppearance(named: .aqua)
        case .dark: NSApp.appearance = NSAppearance(named: .darkAqua)
        }
    }

    // Язык интерфейса — свой у приложения, поверх системного (03.11.2026). Пишем AppleLanguages только
    // в настройки приложения; строки берутся при запуске, поэтому применяется после перезапуска.
    enum InterfaceLanguage: String, CaseIterable {
        case system = "как в системе", ru = "Русский", en = "English", sr = "Srpski"

        var code: String? {
            switch self {
            case .system: nil
            case .ru: "ru"
            case .en: "en"
            case .sr: "sr-Latn"
            }
        }
    }

    static var interfaceLanguage: InterfaceLanguage {
        get {
            let own = UserDefaults.standard.persistentDomain(forName: Bundle.main.bundleIdentifier ?? "")?["AppleLanguages"]
            let code = (own as? [String])?.first
            return InterfaceLanguage.allCases.first { $0.code != nil && $0.code == code } ?? .system
        }
        set {
            if let code = newValue.code {
                UserDefaults.standard.set([code], forKey: "AppleLanguages")
            } else {
                UserDefaults.standard.removeObject(forKey: "AppleLanguages")
            }
        }
    }

    // Запуск при входе в Mac — системный список «Объекты входа».
    static var launchAtLogin: Bool {
        get { SMAppService.mainApp.status == .enabled }
        set {
            do {
                if newValue { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
            } catch { NSSound.beep() }
        }
    }

    // Размер текста заметок; заголовки растут вместе с ним.
    static let textSizes: [CGFloat] = [13, 15, 17, 19]
    static var textSize: CGFloat {
        get { UserDefaults.standard.object(forKey: "textSize") as? CGFloat ?? 15 }
        set { UserDefaults.standard.set(newValue, forKey: "textSize"); NotificationCenter.default.post(name: .editorStyleChanged, object: nil) }
    }

    static var spellCheck: Bool {
        get { UserDefaults.standard.object(forKey: "spellCheck") as? Bool ?? false }
        set { UserDefaults.standard.set(newValue, forKey: "spellCheck"); NotificationCenter.default.post(name: .editorStyleChanged, object: nil) }
    }

    // Как часто сохранять версию в git и отправлять на GitHub, в минутах.
    static let commitChoices = [15, 30, 60, 180]
    static var commitMinutes: Int {
        get { UserDefaults.standard.object(forKey: "commitMinutes") as? Int ?? 60 }
        set { UserDefaults.standard.set(newValue, forKey: "commitMinutes") }
    }

    static let pushChoices = [60, 360, 1440, 10080]
    static var pushMinutes: Int {
        get { UserDefaults.standard.object(forKey: "pushMinutes") as? Int ?? 1440 }
        set { UserDefaults.standard.set(newValue, forKey: "pushMinutes") }
    }

    // Язык для Whisper: «auto» — определять по речи. GigaAM всегда русский.
    static let languages: [(code: String, title: String)] = [("auto", "определять сам"), ("ru", "русский"),
        ("en", "английский"), ("uk", "украинский"), ("de", "немецкий"), ("fr", "французский"), ("es", "испанский")]
    static var language: String {
        get { UserDefaults.standard.string(forKey: "speechLanguage") ?? "ru" }
        set { UserDefaults.standard.set(newValue, forKey: "speechLanguage") }
    }

    // Определять спикеров в расшифровках видео и аудио — когда модели голосов скачаны.
    static var speakers: Bool {
        get { UserDefaults.standard.object(forKey: "speakers") as? Bool ?? true }
        set { UserDefaults.standard.set(newValue, forKey: "speakers") }
    }

    // Предел длины одной надиктовки, в минутах.
    // 0 — без предела: обрыв посреди длинной надиктовки выглядел как «сфера сама пропала» (01.11.2026).
    static let lengthChoices = [0, 1, 3, 5, 10, 15, 30, 60]
    static var maxMinutes: Int {
        get { UserDefaults.standard.object(forKey: "dictationMinutes") as? Int ?? 0 }
        set { UserDefaults.standard.set(newValue, forKey: "dictationMinutes") }
    }

    // «90 минут» → «1,5 ч», «1440» → «раз в сутки».
    static func every(_ minutes: Int) -> String {
        switch minutes {
        case 1440: String(localized: "раз в сутки")
        case 10080: String(localized: "раз в неделю")
        case let m where m % 60 == 0: m == 60 ? String(localized: "раз в час") : String(localized: "раз в \(m / 60) ч")
        default: String(localized: "раз в \(minutes) мин")
        }
    }
}

extension Notification.Name {
    static let editorStyleChanged = Notification.Name("notes.editorStyleChanged")
}
