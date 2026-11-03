import NotesCore
import AppKit
import SwiftUI

@main struct MyApp: App {
    @State private var onboarding = OnboardingModel()

    init() {
        AppSupport.migrate()
        FontRegistrar.registerBundledFonts()
        // Смена темы вслед за системой — плавно (ThemeCrossfade).
        DispatchQueue.main.async { AppSettings.applyTheme(); ThemeCrossfade.shared.start(); DockIcon.shared.start() }
        #if DEBUG
        // `--args -themeFlipTest YES` — через 3 с приложение само меняет тему: снять кадр посреди наплыва.
        if UserDefaults.standard.bool(forKey: "themeFlipTest") {
            DispatchQueue.main.asyncAfter(deadline: .now() + 3) {
                NSApp.appearance = NSAppearance(named: NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? .aqua : .darkAqua)
            }
        }
        #endif
        #if DEBUG
        // Снимать тёмную тему, не переключая систему: `--args -appearance dark`.
        switch UserDefaults.standard.string(forKey: "appearance") {
        case "dark": NSApplication.shared.appearance = NSAppearance(named: .darkAqua)
        case "light": NSApplication.shared.appearance = NSAppearance(named: .aqua)
        default: break
        }
        #endif
    }

    var body: some Scene {
        // Обычная полоса заголовка macOS: кнопки окна слева, по центру «<notes>».
        // Сцена 560×440 из макета идёт под ней.
        WindowGroup("<notes>") {
            RootView(onboarding: onboarding)
                // Папка уже выбрана и под git — история ведётся с первой секунды, даже пока открыт онбординг.
                .task { if let vault = onboarding.vaultURL { SyncController.shared.start(vault: vault) } }
                // Закрываем заметки, когда Mac заблокировали или приложение долго не трогали.
                .task { AppLock.shared.start(model: onboarding) }
                // Надиктовка работает с запуска приложения, из любого окна (⌥ space).
                .task {
                    DictationController.shared.start()
                    await DictationController.shared.recoverPending()
                    #if DEBUG
                    if let name = UserDefaults.standard.string(forKey: "orbPreview") {
                        await DictationController.shared.debugShowOrb(name)
                    }
                    if let path = UserDefaults.standard.string(forKey: "dictationTestFile") {
                        await DictationController.shared.debugRun(file: URL(fileURLWithPath: path))
                    }
                    // Снимок шапки на нужной ширине: окно запоминает размер, а мышью скрипт тянуть не может.
                    let width = UserDefaults.standard.double(forKey: "windowWidth")
                    if width > 0 {
                        try? await Task.sleep(for: .seconds(1))
                        for window in NSApp.windows where window.isVisible && window.frame.width > 700 {
                            var frame = window.frame
                            frame.size.width = width
                            window.setFrame(frame, display: true)
                        }
                    }
                    #endif
                }
        }
        // Окно сжимается до 820, но открывается в размер кадра 1200×760.
        .defaultSize(width: 1200, height: 760)
        .windowResizability(.contentSize)
        .commands {
            // Чтобы смотреть анимацию загрузки много раз подряд, не перезапуская приложение.
            // Настройки — по ⌘, как в любом приложении Mac: открываются разделом главного окна.
            CommandGroup(replacing: .appSettings) {
                Button("Настройки…") { NotificationCenter.default.post(name: .openSettings, object: nil) }
                    .keyboardShortcut(",")
            }
            CommandMenu("Отладка") {
                Button("Повторить загрузку") { onboarding.restart() }
                    .keyboardShortcut("r")
                Button("Следующий шаг") { onboarding.debugNext() }
                    .keyboardShortcut("]")
            }
        }
    }
}

// Первый вход и вход по пинкоду — окно 560×440; после входа — заметки 1200×760.
private struct RootView: View {
    let onboarding: OnboardingModel
    @State private var notes: NotesModel?

    var body: some View {
        Group {
            if onboarding.phase == .unlocked, let notes {
                NotesWindow(model: notes)
                    .transition(.opacity)
            } else {
                OnboardingView(model: onboarding)
            }
        }
        .animation(.smooth(duration: 0.6), value: onboarding.phase == .unlocked)
        .onReceive(NotificationCenter.default.publisher(for: .openSettings)) { _ in
            notes?.section = .settings
        }
        // Новую папку хранилища выбрали в настройках — пересоздаём заметки и синхронизацию на ней.
        .onReceive(NotificationCenter.default.publisher(for: .vaultChosen)) { note in
            guard let url = note.object as? URL else { return }
            onboarding.vaultURL = url
            SyncController.shared.start(vault: url)
        }
        // Модель заметок — одна на окно: создаём, когда есть папка хранилища.
        .onChange(of: onboarding.vaultURL, initial: true) { _, vault in
            guard let vault, notes?.library.root != vault.standardizedFileURL else { return }
            // UndoManager держит цель без владения: после смены хранилища ⌘Z позвал бы ушедшую модель
            // и уронил приложение, как 30.10 с редактором (PITFALLS.md). Чистим её действия заранее.
            if let old = notes {
                for undo in NSApp.windows.compactMap(\.undoManager) + [old.fallbackUndo] {
                    undo.removeAllActions(withTarget: old)
                    undo.removeAllActions(withTarget: old.tasks)
                }
            }
            let model = NotesModel(vault: vault)
            model.openDaily()
            #if DEBUG
            // `-notesAllTabs YES` — все заметки вкладками: проверить растворение краёв при прокрутке.
            if UserDefaults.standard.bool(forKey: "notesAllTabs") {
                for note in model.library.allNotes(in: model.tree) { model.open(.note(note.url)) }
            }
            if let path = UserDefaults.standard.string(forKey: "notesOpen") {
                let url = vault.appendingPathComponent(path)
                model.open(path.hasSuffix(".md") ? .note(url) : url.pathExtension.isEmpty ? .folder(url) : .file(url))
            }
            // `-notesSection tasks|calendar|video|settings`, `-settingsPage вход` — сразу в раздел.
            switch UserDefaults.standard.string(forKey: "notesSection") {
            case "tasks": model.section = .tasks
            case "calendar": model.section = .calendar
            case "video": model.section = .video
            case "settings": model.section = .settings
            default: break
            }
            if let page = UserDefaults.standard.string(forKey: "settingsPage").flatMap(NotesModel.SettingsPage.init) {
                model.settingsPage = page
            }
            if let query = UserDefaults.standard.string(forKey: "notesSearch") {
                model.searchShown = true
                model.query = query
            }
            #endif
            notes = model
            #if DEBUG
            if UserDefaults.standard.bool(forKey: "notesSelfTest") { Task { await model.selfTest() } }
            if UserDefaults.standard.bool(forKey: "speakerPreview") { Task { await model.video.speakerPreview() } }
            #endif
        }
    }
}

extension Notification.Name {
    static let openSettings = Notification.Name("notes.openSettings")
}
