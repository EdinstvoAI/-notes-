import AppKit
import EventKit
import NotesCore
import NotesSpeech
import SwiftUI

// Состояние главного экрана: что выбрано, текст открытой заметки, поиск. Файлы читает и пишет
// NotesCore.NoteLibrary; здесь — только то, что нужно экрану, и автосохранение.
@Observable final class NotesModel {
    enum Selection: Hashable {
        case folder(URL)   // корень хранилища — тоже папка: «все заметки»
        case note(URL)
        case file(URL)     // PDF, Word, фото, видео — просмотр, не правка
    }

    let library: NoteLibrary
    private(set) var tree: NoteLibrary.Folder
    var selection: Selection
    var expanded: Set<URL> = []
    // Открытые заметки — вкладками сверху листа (ТЗ 30.10.2026). Только в памяти: в хранилище не пишем.
    var tabs: [URL] = []
    // ⌘Z, когда окна ещё нет (самопроверка в фоне); обычно — undoManager окна.
    let fallbackUndo = UndoManager()
    // Сколько раз перечитывали дерево — самопроверка следит, что своё сохранение его не будит.
    private(set) var treeReloads = 0

    // Открытая заметка: текст, как он сейчас в редакторе.
    var text = "" { didSet { if text != savedText { scheduleSave() } } }
    // Apple Intelligence пишет «## Кратко» в открытую заметку.
    private(set) var summarizing = false
    func summarizeNote() async {
        guard case .note(let url) = selection, !summarizing else { return }
        summarizing = true
        defer { summarizing = false }
        let source = NoteSections.remove("Кратко", from: text, upToBlank: true)
        guard let summary = try? await Intelligence.summary(of: source), case .note(url) = selection else { return }
        text = NoteSections.setSummary(summary, in: text)
        NotificationCenter.default.post(name: .editorStyleChanged, object: nil)
    }
    private var savedText = ""
    // Номер последней правки: запись через полсекунды делает только последняя из них.
    private var editGeneration = 0

    // Раздел окна: шапка переключает заметки, задачи, календарь и видео; настройки — по ⌘,.
    enum Section: Hashable { case notes, tasks, calendar, video, settings, sync, mcp, safe }
    var section: Section = .notes
    let tasks: TasksModel
    let video: VideoModel
    let history = SyncHistory()
    let safe = SafeModel.shared
    // Раздел MCP: «подключение» или «скиллы».
    var mcpSkills = false

    enum SettingsPage: String, CaseIterable {
        case general = "общие", sync = "синхронизация", dictation = "надиктовка", search = "поиск", lock = "вход", calendar = "календарь"
        case templates = "шаблоны"
        var glyph: DotGlyph {
            switch self {
            case .general: .settings
            case .sync: .git
            case .dictation: .microphone
            case .search: .search
            case .lock: .check
            case .calendar: .calendar
            case .templates: .file
            }
        }
    }
    var settingsPage: SettingsPage = .general

    // Открыли поиск — берём события календаря за год назад и вперёд один раз, а не на каждую букву.
    var searchShown = false { didSet { if searchShown { loadEvents() } } }
    // Окно пинкода из настроек («задать» / «сменить»).
    var pinEntryShown = false
    var query = "" { didSet { runSearch() } }
    private(set) var results: [Found] = []
    var highlighted = 0

    private var watcher: FolderWatcher?
    private var observer: NSObjectProtocol?
    nonisolated private static let changed = Notification.Name("notes.vaultChanged")

    init(vault: URL) {
        library = NoteLibrary(root: vault)
        tree = library.tree()
        selection = .folder(library.root)
        tasks = TasksModel(library: library)
        video = VideoModel(library: library)
        // Слежка идёт не на главном потоке: оттуда только сигнал, а разбираемся уже на главном.
        watcher = FolderWatcher(folder: vault) { paths in
            NotificationCenter.default.post(name: NotesModel.changed, object: paths)
        }
        observer = NotificationCenter.default.addObserver(forName: Self.changed, object: nil, queue: .main) { [weak self] note in
            let paths = note.object as? [String] ?? []
            MainActor.assumeIsolated { self?.externalChange(paths) }
        }
    }

    // MARK: - Выбор

    var currentFolder: NoteLibrary.Folder {
        switch selection {
        case .folder(let url): find(url, in: tree) ?? tree
        case .note(let url), .file(let url): find(url.deletingLastPathComponent(), in: tree) ?? tree
        }
    }

    var openNote: URL? {
        if case .note(let url) = selection { url } else { nil }
    }

    func open(_ selection: Selection) {
        flush()
        section = .notes
        self.selection = selection
        if case .note(let url) = selection {
            if !tabs.contains(where: { $0.standardizedFileURL == url.standardizedFileURL }) { tabs.append(url) }
            let loaded = (try? library.read(url)) ?? ""
            savedText = loaded
            text = loaded
        }
        // Папки на пути к заметке или файлу раскрыты — видно, где он лежит.
        if case .note(let url) = selection { reveal(url) }
        if case .file(let url) = selection { reveal(url) }
    }

    private func reveal(_ url: URL) {
        var folder = url.deletingLastPathComponent().standardizedFileURL
        while folder.path.count > library.root.path.count {
            expanded.insert(folder)
            folder = folder.deletingLastPathComponent().standardizedFileURL
        }
    }

    func toggle(_ folder: URL) {
        withAnimation(.smooth(duration: 0.3)) {
            if expanded.contains(folder) { expanded.remove(folder) } else { expanded.insert(folder) }
        }
    }

    // «проекты / план приложения»; на корне — «все заметки».
    var breadcrumb: String {
        switch selection {
        case .folder(let url) where url.standardizedFileURL == library.root: String(localized: "все заметки")
        case .folder(let url), .note(let url): library.breadcrumb(for: url).joined(separator: " / ")
        case .file(let url): (library.breadcrumb(for: url).dropLast() + [url.lastPathComponent]).joined(separator: " / ")
        }
    }

    // MARK: - Действия

    func newNote() {
        guard let url = try? library.createNote(in: currentFolder.url) else { return NSSound.beep() }
        undoCreation("Новая заметка", [url])
        reloadTree()
        open(.note(url))
    }

    func newNote(from template: URL, in folder: URL? = nil) {
        guard let url = try? Templates.create(from: template, in: folder ?? currentFolder.url, library: library) else {
            return NSSound.beep()
        }
        undoCreation("Новая заметка", [url])
        reloadTree()
        open(.note(url))
    }

    // Автоматизация: заметка на каждый день (настройки → шаблоны). Включена — при запуске есть и открыта.
    static let dailyKey = "dailyNote"

    func openDaily() {
        guard UserDefaults.standard.bool(forKey: Self.dailyKey),
              let url = try? Templates.daily(in: library) else { return }
        reloadTree()
        open(.note(url))
    }

    func newFolder(named name: String) {
        guard !name.trimmingCharacters(in: .whitespaces).isEmpty,
              let url = try? library.createFolder(in: currentFolder.url, name: name) else { return }
        undoCreation("Новая папка", [url])
        reloadTree()
        open(.folder(url))
    }

    func rename(to title: String) {
        guard let url = openNote else { return }
        flush()
        guard let renamed = try? library.rename(url, to: title), renamed != url else { return }
        reloadTree()
        selection = .note(renamed)
    }

    // MARK: - Сохранение

    // Пишем через полсекунды после последней правки: на каждую букву диск дёргать незачем,
    // а потерять можно максимум полсекунды текста. Git-коммит — раз в час, отдельно (SyncController).
    private func scheduleSave() {
        editGeneration += 1
        let generation = editGeneration
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { [weak self] in
            MainActor.assumeIsolated {
                guard let self, self.editGeneration == generation else { return }
                self.flush()
            }
        }
    }

    func flush() {
        guard let url = openNote, text != savedText else { return }
        do {
            try library.write(text, to: url)
            savedText = text
        } catch {
            NSAlert(error: error).runModal()
        }
    }

    // Файлы поменяли снаружи (другой редактор, pull с другого Mac): обновляем дерево, а открытую
    // заметку перечитываем, только если в ней нет несохранённых правок.
    private func externalChange(_ paths: [String] = []) {
        // Своё автосохранение открытой заметки: на диске ровно то, что мы записали. Раньше каждая пауза
        // в наборе перечитывала всё хранилище, задачи и календарь — набор подлагивал (31.10.2026).
        // Тогда пересобираем задачи только из этой заметки.
        // Временный файл атомарной записи («Входящие.md.sb-…») — тоже своё: без песочницы он виден в событиях (01.11.2026).
        let real = paths.filter { !URL(fileURLWithPath: $0).lastPathComponent.contains(".sb-") }
        if let url = openNote, !real.isEmpty,
           real.allSatisfy({ URL(fileURLWithPath: $0).standardizedFileURL == url.standardizedFileURL }),
           (try? library.read(url)) == savedText {
            tasks.reload(note: url, text: savedText)
            return
        }
        reloadTree()
        tasks.reload()
        guard let url = openNote, text == savedText else { return }
        guard FileManager.default.fileExists(atPath: url.path) else { return open(.folder(library.root)) }
        if let disk = try? library.read(url), disk != savedText {
            savedText = disk
            text = disk
        }
    }

    // Файл удалили или перенесли — его вкладка закрывается.
    func reloadTree() {
        treeReloads += 1
        tree = library.tree()
        tabs.removeAll { !FileManager.default.fileExists(atPath: $0.path) }
        if case .file(let url) = selection, !FileManager.default.fileExists(atPath: url.path) {
            selection = .folder(url.deletingLastPathComponent())
        }
    }

    private func find(_ url: URL, in folder: NoteLibrary.Folder) -> NoteLibrary.Folder? {
        if folder.url.standardizedFileURL == url.standardizedFileURL { return folder }
        for child in folder.folders { if let hit = find(url, in: child) { return hit } }
        return nil
    }

    // MARK: - Поиск

    // Один поиск по всему: заметки, задачи, события, видео и упоминания в расшифровках (ТЗ 30.10.2026).
    // «По смыслу» и «по тексту» убраны по ТЗ; выдача — группами в этом порядке.
    struct Found: Identifiable {
        enum Kind: String { case note = "заметки", task = "задачи", event = "события", video = "видео" }
        enum Target { case note(URL), event(Date), video(UUID) }
        let id: String
        let kind: Kind
        let title: String
        let detail: String
        let snippet: String?
        let target: Target
    }

    private struct EventItem { let title: String, start: Date, place: String, notes: String }
    private var events: [EventItem] = []

    private func loadEvents() {
        let year: TimeInterval = 365 * 24 * 3600
        events = tasks.calendar.events(from: Date().addingTimeInterval(-year), to: Date().addingTimeInterval(year))
            .map { EventItem(title: $0.title ?? "", start: $0.startDate, place: $0.location ?? "", notes: $0.notes ?? "") }
    }

    private func runSearch() {
        let query = SearchQuery(self.query)
        highlighted = 0
        guard !query.isEmpty else { results = []; return }
        var found: [Found] = []
        // Расшифровки, сохранённые заметками, найдутся и здесь — среди заметок.
        for hit in library.search(self.query, limit: 6) {
            found.append(Found(id: "n" + hit.note.url.path, kind: .note, title: hit.note.title, detail: path(hit.note.url),
                               snippet: hit.snippet, target: .note(hit.note.url)))
        }
        for task in tasks.tasks.filter({ query.matches($0.title) }).prefix(5) {
            let when = task.done ? String(localized: "готово") : task.due.map { NoteEditor.short($0) } ?? String(localized: "без даты")
            found.append(Found(id: "t" + task.id, kind: .task, title: task.title,
                               detail: when + " · " + task.note.deletingPathExtension().lastPathComponent,
                               snippet: nil, target: .note(task.note)))
        }
        let now = Date()
        // Ближайшие — сначала: будущие по возрастанию, потом прошедшие от свежих.
        let matched = events.filter { query.matches($0.title, $0.place, $0.notes) }
            .sorted { ($0.start >= now) != ($1.start >= now) ? $0.start >= now : $0.start >= now ? $0.start < $1.start : $0.start > $1.start }
        for (index, event) in matched.prefix(5).enumerated() {
            let note = query.matches(event.title) ? nil : [event.place, event.notes].first { query.matches($0) }
            found.append(Found(id: "e\(index)", kind: .event, title: event.title,
                               detail: NoteEditor.date(event.start), snippet: note, target: .event(event.start)))
        }
        for item in video.items {
            let mention = video.transcript(item.id)?.mention(of: query)
            guard query.matches(item.title) || mention != nil else { continue }
            found.append(Found(id: "v" + item.id.uuidString, kind: .video, title: item.title,
                               detail: mention.map { Transcript.clock($0.start) } ?? "видео",
                               snippet: mention?.text, target: .video(item.id)))
            if found.filter({ $0.kind == .video }).count == 5 { break }
        }
        results = found
    }

    private func path(_ url: URL) -> String {
        let parts = library.breadcrumb(for: url).dropLast()
        return parts.isEmpty ? "без папки" : parts.joined(separator: " / ")
    }

    func openHighlighted() {
        guard results.indices.contains(highlighted) else { return }
        switch results[highlighted].target {
        case .note(let url): open(.note(url))
        case .event(let date): flush(); tasks.calendar.focus = date; section = .calendar
        case .video(let id): flush(); video.open = id; section = .video
        }
        searchShown = false
    }
}

#if DEBUG
extension NotesModel {
    // `--args -testVault YES -notesSelfTest YES`, запуск бинарника из терминала: проходит по действиям
    // экрана на тестовой папке и печатает «ok / FAIL» по каждому. Кликать скриптом macOS не даёт.
    func selfTest() async {
        func check(_ name: String, _ ok: Bool) { FileHandle.standardError.write(Data(((ok ? "ok   " : "FAIL ") + name + "\n").utf8)) }
        check("самопроверка началась", true)
        let fm = FileManager.default

        open(.folder(library.root))
        newNote()
        let created = openNote
        check("новая заметка создана и открыта", created.map { fm.fileExists(atPath: $0.path) } ?? false)

        text = "# Заголовок\nпервая строка"
        try? await Task.sleep(for: .milliseconds(800))
        let disk = created.flatMap { try? String(contentsOf: $0, encoding: .utf8) }
        check("текст сохранился сам через полсекунды", disk == text)

        rename(to: "проверка")
        check("переименование меняет файл", openNote?.lastPathComponent == "проверка.md"
              && openNote.map { fm.fileExists(atPath: $0.path) } == true)

        newFolder(named: "новая")
        check("новая папка создана и открыта", fm.fileExists(atPath: library.root.appendingPathComponent("новая").path)
              && currentFolder.name == "новая")

        query = "первая"
        check("поиск находит новую заметку", results.first?.title == "проверка")

        // Правка снаружи: другой редактор пишет в открытую заметку — экран подхватывает.
        open(.note(library.root.appendingPathComponent("проверка.md")))
        try? "снаружи".write(to: library.root.appendingPathComponent("проверка.md"), atomically: true, encoding: .utf8)
        try? await Task.sleep(for: .seconds(2))
        check("правка снаружи подхватилась", text == "снаружи")

        tasks.reload()
        check("задачи собраны из заметок (6)", tasks.tasks.count == 6)
        check("группы: сегодня 2, неделя 2, без даты 1, готово 1",
              tasks.count(.today) == 2 && tasks.count(.week) == 2 && tasks.count(.noDate) == 1 && tasks.count(.done) == 1)
        if let bread = tasks.tasks.first(where: { $0.title == "Купить хлеб" }) {
            tasks.toggle(bread)
            let disk = (try? String(contentsOf: bread.note, encoding: .utf8)) ?? ""
            check("отметка «готово» записалась в файл", disk.contains("- [x] Купить хлеб"))
        }
        if case .note(let url) = selection {
            togglePin(url); check("заметка закрепилась", isPinned(url))
            togglePin(url)
            duplicate(url)
            let copy = url.deletingLastPathComponent().appendingPathComponent(url.deletingPathExtension().lastPathComponent + " копия.md")
            check("дубликат создан", FileManager.default.fileExists(atPath: copy.path))
            trash(copy); check("дубликат ушёл в Корзину", !FileManager.default.fileExists(atPath: copy.path))
            let into = library.root.appendingPathComponent("перенос", isDirectory: true)
            try? FileManager.default.createDirectory(at: into, withIntermediateDirectories: true)
            drop([url], into: into)
            let moved = into.appendingPathComponent(url.lastPathComponent)
            check("перетащенная заметка переехала и открыта там", FileManager.default.fileExists(atPath: moved.path)
                  && openNote?.standardizedFileURL == moved.standardizedFileURL)
            check("папку в саму себя не кладём", !drop([into], into: into))
            drop([moved], into: url.deletingLastPathComponent())
            trash(into)
            // ⌘Z: удаление возвращает файл, перенос — обратно, дубль — убирает.
            trash(url); undoer.undo()
            check("⌘Z вернул удалённую заметку", fm.fileExists(atPath: url.path))
            duplicate(url); undoer.undo()
            check("⌘Z убрал дубль", !fm.fileExists(atPath: copy.path))
            let box = library.root.appendingPathComponent("ящик", isDirectory: true)
            try? fm.createDirectory(at: box, withIntermediateDirectories: true)
            drop([url], into: box); undoer.undo()
            check("⌘Z вернул перенесённую заметку", fm.fileExists(atPath: url.path)
                  && !fm.fileExists(atPath: box.appendingPathComponent(url.lastPathComponent).path))
            trash(box)
            let other = library.root.appendingPathComponent("Входящие.md")
            open(.note(other)); open(.note(url))
            let before = tabs.count
            closeTab(url)
            check("вкладка закрылась и открылась соседняя", tabs.count == before - 1 && openNote != url && openNote != nil)
        }
        // Своё автосохранение не перечитывает хранилище: дерево не трогаем, задача из набранной строки — есть.
        let own = library.root.appendingPathComponent("Входящие.md")
        open(.note(own))
        try? await Task.sleep(for: .seconds(1.5))
        let reloads = treeReloads
        text += "\n- [ ] Задача из набора"
        flush()
        try? await Task.sleep(for: .seconds(2))
        check("своё сохранение не перечитало хранилище, задача подхватилась",
              treeReloads == reloads && tasks.tasks.contains { $0.title == "Задача из набора" })
        // Скорость набора: длинная заметка, перекраска всей против одного абзаца (как при нажатии клавиши).
        let big = NSTextStorage(string: (0..<20_000).map { $0 % 50 == 0 ? "## Заголовок \($0)" : "строка текста номер \($0)" }.joined(separator: "\n"))
        func ms(_ work: () -> Void) -> Double { let t = Date(); work(); return Date().timeIntervalSince(t) * 1000 }
        let full = ms { MarkdownEditor.Style.apply(to: big) }
        let one = ms { MarkdownEditor.Style.apply(to: big, in: NSRange(location: 300_000, length: 1)) }
        check(String(format: "перекраска при наборе %.2f мс, вся заметка %.0f мс", one, full), one < 2 && one * 20 < full)
        if UserDefaults.standard.bool(forKey: "liveSelfTest") {
            CalendarSync.shared.selfTest(check)
            CalendarSync.shared.removalSelfTest(check)
            tasks.editSelfTest(check)
            // Модели голосов — как по кнопке «скачать» в настройках: настоящая загрузка с проверкой сумм.
            try? await ModelDownloads.shared.downloadSpeakers()
            check("модели спикеров скачаны", Diarization.isInstalled())
            await video.selfTest(check)
            if Intelligence.unavailableReason == nil, let note = library.allNotes(in: tree).first {
                open(.note(note.url))
                let before = text
                await summarizeNote()
                check("кратко в заметке «\(note.url.lastPathComponent)»", text.contains("## Кратко") && text.count > before.count)
                text = before
                flush()
            }
        }
        exit(0)
    }
}
#endif
