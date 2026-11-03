import Foundation

// Локальный MCP-сервер: нейронка (Claude, Codex и другие) работает с хранилищем через stdio,
// «чтобы нейронка могла обращаться к нам» (03.11.2026). Без сети и зависимостей: JSON-RPC построчно.
// Пишет только новое — заметку или строку задачи; чужой текст не переписывает. Коммит делает приложение,
// как после любой правки снаружи.
public struct MCPServer {
    public let library: NoteLibrary
    // «Только чтение» в разделе MCP: инструменты записи не видны и не работают.
    public let allowWrites: Bool
    // Журнал обращений для раздела MCP: инструмент, путь или запрос и длина ответа (для оценки токенов),
    // без текста заметок.
    public let log: (@Sendable (String, String, Int) -> Void)?

    // Папки, закрытые для нейронок (раздел MCP): их заметок не видно ни в поиске, ни в списке, ни по пути.
    public let hiddenFolders: Set<String>

    public init(library: NoteLibrary, allowWrites: Bool = true, hiddenFolders: Set<String> = [],
                log: (@Sendable (String, String, Int) -> Void)? = nil) {
        self.library = library
        self.hiddenFolders = hiddenFolders
        self.allowWrites = allowWrites
        self.log = log
    }

    static let writeTools: Set<String> = ["create_note", "add_task"]
    // Больше — не отдаём: нейронке хватит, а случайно не утянет гигантский файл.
    static let maxRead = 200_000

    static let version = "2025-06-18"

    // Читает запросы со stdin до конца ввода. Нейронка закрыла канал — процесс выходит.
    public func run() {
        setvbuf(stdout, nil, _IOLBF, 0)
        while let line = readLine() {
            guard !line.trimmingCharacters(in: .whitespaces).isEmpty else { continue }
            if let reply = handle(line) { print(reply) }
        }
    }

    // Одна строка запроса → одна строка ответа; nil — уведомление, отвечать не надо.
    public func handle(_ line: String) -> String? {
        guard let data = line.data(using: .utf8),
              let request = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else {
            return Self.encode(["jsonrpc": "2.0", "id": NSNull(),
                                "error": ["code": -32700, "message": "parse error"]])
        }
        guard let id = request["id"] else { return nil }
        let method = request["method"] as? String ?? ""
        let params = request["params"] as? [String: Any] ?? [:]
        var reply: [String: Any] = ["jsonrpc": "2.0", "id": id]
        switch method {
        case "initialize":
            reply["result"] = [
                "protocolVersion": params["protocolVersion"] as? String ?? Self.version,
                "capabilities": ["tools": [String: Any]()],
                "serverInfo": ["name": "notes", "version": "1.0.0"],
                "instructions": "Markdown-заметки пользователя. Пути — от корня хранилища, с .md.",
            ]
        case "ping":
            reply["result"] = [String: Any]()
        case "tools/list":
            reply["result"] = ["tools": Self.tools.filter { allowWrites || !Self.writeTools.contains($0["name"] as? String ?? "") }]
        case "tools/call":
            let name = params["name"] as? String ?? ""
            let args = params["arguments"] as? [String: Any] ?? [:]
            let target = (args["path"] ?? args["note"] ?? args["query"] ?? args["title"] ?? args["folder"] ?? "") as? String ?? ""
            do {
                let text = try call(name, args)
                log?(name, target, text.count)
                reply["result"] = ["content": [["type": "text", "text": text]]]
            } catch {
                log?(name, target, 0)
                reply["result"] = ["content": [["type": "text", "text": "\(error)"]], "isError": true]
            }
        default:
            reply["error"] = ["code": -32601, "message": "unknown method \(method)"]
        }
        return Self.encode(reply)
    }

    struct Failure: Error, CustomStringConvertible {
        let description: String
        init(_ text: String) { description = text }
    }

    func call(_ name: String, _ args: [String: Any]) throws -> String {
        if Self.writeTools.contains(name) && !allowWrites { throw Failure("запись выключена: включено «только чтение»") }
        switch name {
        case "search_notes":
            let query = try Self.string(args, "query")
            let hits = library.search(query, limit: 200).filter { inside($0.note.url) }.prefix(args["limit"] as? Int ?? 20)
            if hits.isEmpty { return "ничего не нашлось" }
            return hits.map { "\(relative($0.note.url)) — \($0.snippet)" }.joined(separator: "\n")
        case "list_notes":
            let folder = try url(args["folder"] as? String ?? "", directory: true)
            let notes = library.allNotes(in: NoteLibrary(root: folder).tree())
                .filter { inside($0.url) }
                .sorted { $0.modified > $1.modified }
            if notes.isEmpty { return "заметок нет" }
            return notes.map { relative($0.url) }.joined(separator: "\n")
        case "read_note":
            let text = try library.read(url(try Self.string(args, "path")))
            return text.count > Self.maxRead ? String(text.prefix(Self.maxRead)) + "\n…(обрезано)" : text
        case "create_note":
            let title = try Self.string(args, "title")
            let folder = try url(args["folder"] as? String ?? "", directory: true)
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            // «без названия 2» и т. д.: существующую заметку не перезаписываем никогда.
            let note = try library.createNote(in: folder, title: title.replacingOccurrences(of: "/", with: "-"))
            try library.write(args["text"] as? String ?? "", to: note)
            return "создана \(relative(note))"
        case "list_tasks":
            let open = args["include_done"] as? Bool != true
            let tasks = NoteTasks.all(in: library).filter { (!open || !$0.done) && inside($0.note) }
            if tasks.isEmpty { return "задач нет" }
            return tasks.map { task in
                let due = task.due.map { " 📅 " + NoteTasks.format($0, withTime: task.hasTime) } ?? ""
                return "\(task.done ? "[x]" : "[ ]") \(task.title)\(due) — \(relative(task.note))"
            }.joined(separator: "\n")
        case "add_task":
            let title = try Self.string(args, "title")
            let note = try url(try Self.string(args, "note"))
            var due: Date?, hasTime = false
            if let text = args["due"] as? String, !text.isEmpty {
                guard let parsed = Self.date(text) else { throw Failure("due: ГГГГ-ММ-ДД или ГГГГ-ММ-ДД ЧЧ:ММ") }
                (due, hasTime) = parsed
            }
            try FileManager.default.createDirectory(at: note.deletingLastPathComponent(), withIntermediateDirectories: true)
            try NoteTasks.append(title, due: due, hasTime: hasTime, to: note, library: library)
            return "задача добавлена в \(relative(note))"
        default:
            throw Failure("нет инструмента \(name)")
        }
    }

    // Путь от корня хранилища. Выйти за корень («../», абсолютный путь) нельзя: сервер видит только заметки.
    func url(_ path: String, directory: Bool = false) throws -> URL {
        let clean = path.trimmingCharacters(in: CharacterSet(charactersIn: "/ "))
        let url = (clean.isEmpty ? library.root : library.root.appendingPathComponent(clean, isDirectory: directory))
            .standardizedFileURL
        // Ссылка внутри хранилища может вести наружу — проверяем и настоящий путь, после ссылок.
        let realRoot = library.root.resolvingSymlinksInPath().path
        for candidate in [url.path, url.resolvingSymlinksInPath().path] {
            let base = candidate == url.path ? library.root.path : realRoot
            guard candidate == base || candidate.hasPrefix(base + "/") else { throw Failure("путь вне хранилища: \(path)") }
        }
        guard !url.pathComponents.contains(where: { $0.hasPrefix(".") }) else { throw Failure("скрытый путь: \(path)") }
        let first = relative(url).split(separator: "/").first.map(String.init) ?? ""
        guard !hiddenFolders.contains(first) else { throw Failure("папка закрыта для нейронок: \(first)") }
        guard directory || url.pathExtension.lowercased() == "md" else { throw Failure("нужен путь к .md: \(path)") }
        return url
    }

    // Поиск, список и задачи идут по дереву, а оно видит и ссылки наружу — отсеиваем той же проверкой.
    func inside(_ note: URL) -> Bool { (try? url(relative(note))) != nil }

    func relative(_ url: URL) -> String {
        String(url.standardizedFileURL.path.dropFirst(library.root.path.count + 1).drop { $0 == "/" })
    }

    static func string(_ args: [String: Any], _ key: String) throws -> String {
        guard let value = args[key] as? String, !value.trimmingCharacters(in: .whitespaces).isEmpty else {
            throw Failure("нужен параметр \(key)")
        }
        return value
    }

    static func date(_ text: String) -> (Date, Bool)? {
        let format = DateFormatter()
        format.locale = Locale(identifier: "en_US_POSIX")
        format.dateFormat = "yyyy-MM-dd HH:mm"
        if let date = format.date(from: text) { return (date, true) }
        format.dateFormat = "yyyy-MM-dd"
        return format.date(from: text).map { ($0, false) }
    }

    static func encode(_ object: [String: Any]) -> String {
        let data = (try? JSONSerialization.data(withJSONObject: object, options: [.withoutEscapingSlashes])) ?? Data()
        return String(decoding: data, as: UTF8.self)
    }

    static func tool(_ name: String, _ about: String, _ properties: [String: String], required: [String] = [],
                     flags: [String: String] = [:]) -> [String: Any] {
        var props: [String: Any] = properties.mapValues { ["type": "string", "description": $0] }
        for (key, about) in flags { props[key] = ["type": "boolean", "description": about] }
        return ["name": name, "description": about,
                "inputSchema": ["type": "object", "properties": props, "required": required]]
    }

    static let tools: [[String: Any]] = [
        tool("search_notes", "Найти заметки по словам: все слова должны встретиться. Ответ — путь и отрывок.",
             ["query": "слова для поиска"], required: ["query"]),
        tool("list_notes", "Список заметок, свежие сверху.", ["folder": "папка от корня; пусто — всё хранилище"]),
        tool("read_note", "Текст заметки целиком (Markdown).", ["path": "путь к .md от корня"], required: ["path"]),
        tool("create_note", "Создать новую заметку. Существующую не перезаписывает — добавит номер к названию.",
             ["title": "название, оно же имя файла", "text": "Markdown", "folder": "папка от корня; пусто — корень"],
             required: ["title"]),
        tool("list_tasks", "Задачи из всех заметок (строки «- [ ]»).", [:],
             flags: ["include_done": "показать и выполненные"]),
        tool("add_task", "Дописать задачу в конец заметки; заметки нет — создаст.",
             ["title": "текст задачи", "note": "путь к .md от корня", "due": "ГГГГ-ММ-ДД или ГГГГ-ММ-ДД ЧЧ:ММ"],
             required: ["title", "note"]),
    ]
}
