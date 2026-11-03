import AppKit
import NotesCore
import NotesSpeech
import SwiftUI

// Состояние первого входа. Шаги и ветки — по docs/ONBOARDING.md, номера совпадают с кадрами в Figma.
@Observable
final class OnboardingModel {
    // loading — сцена загрузки, steps — лист с шагами, finished — «все готово» на стекле.
    // locked — повторный запуск с пинкодом или Touch ID: та же загрузка, потом карточка входа;
    // unlocked — вход пройден (основного экрана пока нет, остаются планки с логотипом).
    // unlocked — вход пройден: главный экран заметок (NotesWindow).
    enum Phase { case loading, steps, finished, locked, unlocked }

    enum Step: Int, CaseIterable {
        case welcome = 1, git, github, githubCode, githubDone, models, download, permissions, password
    }

    var phase: Phase = .loading
    var step: Step = .welcome
    // Папка хранилища, которую выбрали или создали на шаге 1.
    var vaultURL: URL?
    // Пользователь согласился сделать git-репозиторий в папке (сам git init — этап 2 в ROADMAP).
    var wantsGit = false
    // Полное имя репозитория на GitHub после входа, например makark/notes.
    var githubRepository: String?
    // Модель распознавания: выбор сразу сохраняется — его же берёт надиктовка.
    var speechModel: SpeechModel = SpeechSettings.model {
        didSet { SpeechSettings.model = speechModel }
    }
    // Доля скачанной модели, 0…1, и когда скачивание началось — для «осталось N мин».
    var downloadProgress: Double = 0
    var downloadStarted: Date?
    var downloadFailed = false
    // Окно ввода пинкода поверх шага 9.
    var pinEntryShown = false
    // Лист входа уже поднят: после загрузки при запуске или сразу, если заблокировали во время работы.
    var lockReady = false
    // Меняется при каждом перезапуске, чтобы SwiftUI пересоздал сцену с нуля.
    var runID = UUID()

    init() {
        vaultURL = VaultStore.restore()
        // Перезапуск сразу после работы пускает без пинкода (LockPolicy), иначе — вход.
        // Первый вход пройден — сразу заметки; с пинкодом или Touch ID — сначала вход (LockPolicy).
        if UserDefaults.standard.bool(forKey: Self.completedKey), vaultURL != nil {
            let needs = Self.lockConfigured
                && LockPolicy.needsUnlockAtLaunch(lastSeenUnlocked: LockMemory().lastSeenUnlocked)
            phase = needs ? .locked : .unlocked
        }
        #if DEBUG
        // Запуск сразу с нужного шага, без загрузки: `open <notes>.app --args -onboardingStep 4`.
        // 10 — финальный экран. Нужно, чтобы снимать каждый шаг для сверки с Figma.
        let start = UserDefaults.standard.integer(forKey: "onboardingStep")
        if start == 10 {
            phase = .finished
        } else if start == 11 {
            phase = .locked
        } else if let step = Step(rawValue: start) {
            phase = .steps
            self.step = step
        }
        // `-testVault` — главный экран на тестовой папке в контейнере приложения: настоящие заметки
        // при проверках не трогаем. `-notesOpen проекты/план приложения.md`, `-notesSearch запрос` — сразу туда.
        if UserDefaults.standard.bool(forKey: "calendarCleanup") {
            FileHandle.standardError.write(Data("удалено тестовых событий: \(CalendarSync.shared.removeTestEvents())\n".utf8))
        }
        if UserDefaults.standard.bool(forKey: "testVault") {
            CalendarSync.shared.enabled = false
            vaultURL = Self.makeTestVault()
            phase = .unlocked
        }
        pinEntryShown = step == .password && phase == .steps && UserDefaults.standard.string(forKey: "pinPreview") != nil
        #endif
    }

    #if DEBUG
    static func day(_ offset: Int) -> String {
        NoteTasks.format(Calendar.current.date(byAdding: .day, value: offset, to: Date())!, withTime: false)
    }

    static func makeTestVault() -> URL {
        let base = AppSupport.folder
            .appendingPathComponent("test-vault", isDirectory: true)
        try? FileManager.default.removeItem(at: base)
        let notes: [String: String] = [
            "проекты/план приложения.md": "Нативное приложение для заметок под Mac. Всё хранится локально в обычных .md файлах, синхронизация — через git.\n\n## Что делаем\n\n— заметки в Markdown\n— автосинхронизация через репозиторий\n— поиск по смыслу\n— надиктовка по хоткею\n— задачи в Apple Calendar\n\nКанбан не делаем. Дизайн — сам.",
            "проекты/идеи.md": "Сделать так, чтобы всё работало офлайн и потом догоняло.",
            "проекты/созвоны.md": "Обсудили, как переносить заметки между устройствами без облака.",
            "личное/книги.md": "Прочитать до конца года.",
            "Созвоны/12 октября.md": "Синхронизация с телефоном: открыть репозиторий в Working Copy.",
            "Книги/список.md": "Норвежский лес", "Идеи для видео/первое.md": "Как я пишу заметки",
            "Входящие.md": "- [ ] Купить хлеб 📅 \(Self.day(3))\n- [ ] Записать видео про заметки\n- [x] Выбрать модель распознавания",
            "проекты/дела.md": "- [ ] Созвониться с командой 📅 \(Self.day(0)) 15:00\n- [ ] Собрать иконку приложения 📅 \(Self.day(2)) 9:00\n- [ ] Дописать план приложения 📅 \(Self.day(0))", "Список покупок.md": "молоко, хлеб",
        ]
        for (path, text) in notes {
            let url = base.appendingPathComponent(path)
            try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? text.write(to: url, atomically: true, encoding: .utf8)
        }
        // Файл для просмотра — картинка, чтобы проверить просмотр Quick Look в листе.
        if let tiff = NSApplication.shared.applicationIconImage?.tiffRepresentation,
           let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) {
            try? FileManager.default.createDirectory(at: base.appendingPathComponent("Идеи для видео"), withIntermediateDirectories: true)
            try? png.write(to: base.appendingPathComponent("Идеи для видео/обложка.png"))
        }
        // -testVaultGit YES — с историей из трёх версий, для снимков раздела «синхронизация».
        if UserDefaults.standard.bool(forKey: "testVaultGit"), let git = try? GitVault.prepare(at: base).vault {
            _ = try? git.commitAll(message: "заметки")
            try? "молоко, хлеб, сыр".write(to: base.appendingPathComponent("Список покупок.md"), atomically: true, encoding: .utf8)
            _ = try? git.commitAll(message: "изменено: Список покупок")
            try? FileManager.default.removeItem(at: base.appendingPathComponent("личное/книги.md"))
            _ = try? git.commitAll(message: "удалено: книги")
        }
        return base
    }
    #endif

    // Есть чем закрываться: пинкод или Touch ID.
    static var lockConfigured: Bool { PinStore().isSet || BiometricLock.isEnabled }

    // Mac заблокировали, он уснул или приложение долго не трогали (AppLock).
    func lock() {
        guard phase == .unlocked else { return }
        pinEntryShown = false
        withAnimation(.smooth(duration: 0.8)) {
            lockReady = true
            phase = .locked
        }
    }

    func unlock() {
        LockMemory().lastSeenUnlocked = Date()
        withAnimation(.smooth(duration: 0.8)) { phase = .unlocked }
    }

    // «забыл пинкод»: хозяин Mac подтвердил себя — паузу снимаем и даём задать новый пинкод.
    func resetPin() {
        Task {
            guard await BiometricLock.verifyOwner() else { return }
            var limiter = LockMemory().limiter
            limiter.reset()
            LockMemory().limiter = limiter
            withAnimation(.smooth(duration: 0.5)) { pinEntryShown = true }
        }
    }

    func go(to step: Step) {
        withAnimation(.smooth(duration: 0.8)) { self.step = step }
    }

    func finish() {
        UserDefaults.standard.set(true, forKey: Self.completedKey)
        withAnimation(.smooth(duration: 1.4)) { phase = .finished }
    }

    // Первый вход пройден. Пока основного экрана нет, только запоминаем; потом по этому флагу
    // при запуске сразу откроются заметки.
    static let completedKey = "onboardingCompleted"

    func restart() {
        phase = .loading
        lockReady = false
        step = .welcome
        vaultURL = nil
        wantsGit = false
        githubRepository = nil
        downloadProgress = 0
        downloadStarted = nil
        downloadFailed = false
        runID = UUID()
    }

    // Только для меню «Отладка»: пролистать шаги без настоящих действий.
    func debugNext() {
        switch phase {
        case .loading: withAnimation(.smooth(duration: 1.3)) { phase = .steps }
        case .finished, .locked, .unlocked: restart()
        case .steps:
            if let next = Step(rawValue: step.rawValue + 1) { go(to: next) } else { finish() }
        }
    }
}
