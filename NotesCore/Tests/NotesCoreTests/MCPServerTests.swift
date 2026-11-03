import Foundation
import Testing
@testable import NotesCore

@Suite("MCP-сервер")
struct MCPServerTests {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("mcp-\(UUID().uuidString)")

    func server() throws -> MCPServer {
        try FileManager.default.createDirectory(at: root.appendingPathComponent("проекты"), withIntermediateDirectories: true)
        try "# План\nкупить хлеб".write(to: root.appendingPathComponent("проекты/план.md"), atomically: true, encoding: .utf8)
        return MCPServer(library: NoteLibrary(root: root))
    }

    func call(_ s: MCPServer, _ name: String, _ args: [String: Any]) -> (String, Bool) {
        let req = MCPServer.encode(["jsonrpc": "2.0", "id": 1, "method": "tools/call",
                                    "params": ["name": name, "arguments": args]])
        let reply = try! JSONSerialization.jsonObject(with: Data(s.handle(req)!.utf8)) as! [String: Any]
        let result = reply["result"] as! [String: Any]
        let text = ((result["content"] as! [[String: Any]])[0]["text"] as! String)
        return (text, result["isError"] as? Bool ?? false)
    }

    @Test("рукопожатие и список инструментов; на уведомление не отвечает")
    func handshake() throws {
        let s = try server()
        #expect(s.handle(#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#)!.contains("serverInfo"))
        #expect(s.handle(#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#) == nil)
        #expect(s.handle(#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)!.contains("search_notes"))
    }

    @Test("поиск, чтение, новая заметка не перезаписывает, задача дописывается")
    func tools() throws {
        let s = try server()
        #expect(call(s, "search_notes", ["query": "хлеб"]).0.contains("проекты/план.md"))
        #expect(call(s, "read_note", ["path": "проекты/план.md"]).0 == "# План\nкупить хлеб")
        #expect(call(s, "create_note", ["title": "план", "folder": "проекты", "text": "новое"]).0.contains("план 2.md"))
        #expect(try String(contentsOf: root.appendingPathComponent("проекты/план.md"), encoding: .utf8) == "# План\nкупить хлеб")
        _ = call(s, "add_task", ["title": "позвонить", "note": "проекты/план.md", "due": "2026-11-04 15:00"])
        #expect(call(s, "list_tasks", [:]).0.contains("[ ] позвонить 📅 2026-11-04 15:00"))
    }

    // Нейронка не должна выйти из хранилища: ../ или .git — ошибка, а не чтение чужого файла.
    @Test("за пределы хранилища и в скрытые папки — нельзя")
    func sandbox() throws {
        let s = try server()
        #expect(call(s, "read_note", ["path": "../секрет.md"]).1)
        #expect(call(s, "read_note", ["path": ".git/config.md"]).1)
        #expect(call(s, "read_note", ["path": ".safe/safe.md"]).1)   // сейф паролей нейронке не виден
        #expect(call(s, "read_note", ["path": "проекты/план.txt"]).1)
    }

    // Ссылка в хранилище на файл снаружи: раньше путь «внутри» проходил проверку и сервер читал чужое.
    @Test("ссылка наружу не читается")
    func symlink() throws {
        let s = try server()
        let outside = FileManager.default.temporaryDirectory.appendingPathComponent("secret-\(UUID().uuidString).md")
        try "секрет".write(to: outside, atomically: true, encoding: .utf8)
        try FileManager.default.createSymbolicLink(at: root.appendingPathComponent("ссылка.md"), withDestinationURL: outside)
        #expect(call(s, "read_note", ["path": "ссылка.md"]).1)
        #expect(!call(s, "search_notes", ["query": "секрет"]).0.contains("секрет"))
    }

    @Test("только чтение: записи нет ни в списке, ни в вызове; журнал без текста")
    func readOnly() throws {
        final class Box: @unchecked Sendable { var lines: [String] = [] }
        let box = Box()
        let base = try server()
        let s = MCPServer(library: base.library, allowWrites: false) { box.lines.append("\($0) \($1)") }
        #expect(!s.handle(#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)!.contains("create_note"))
        #expect(call(s, "create_note", ["title": "x", "text": "тайна"]).1)
        #expect(!FileManager.default.fileExists(atPath: root.appendingPathComponent("x.md").path))
        _ = call(s, "read_note", ["path": "проекты/план.md"])
        #expect(box.lines.last == "read_note проекты/план.md")
        #expect(!box.lines.joined().contains("тайна"))
    }

    @Test("закрытая папка не видна ни поиском, ни по пути, и туда не пишется")
    func hidden() throws {
        let base = try server()
        let s = MCPServer(library: base.library, hiddenFolders: ["проекты"])
        #expect(call(s, "search_notes", ["query": "хлеб"]).0 == "ничего не нашлось")
        #expect(call(s, "read_note", ["path": "проекты/план.md"]).1)
        #expect(call(s, "create_note", ["title": "x", "folder": "проекты"]).1)
        #expect(!call(s, "list_notes", [:]).0.contains("проекты"))
    }
}
