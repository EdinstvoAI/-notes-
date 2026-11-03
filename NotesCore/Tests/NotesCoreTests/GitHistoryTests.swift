import Foundation
import Testing
@testable import NotesCore

@Suite("история и откат")
struct GitHistoryTests {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("hist-\(UUID().uuidString)")

    func write(_ name: String, _ text: String) throws {
        try text.write(to: root.appendingPathComponent(name), atomically: true, encoding: .utf8)
    }
    func read(_ name: String) -> String? { try? String(contentsOf: root.appendingPathComponent(name), encoding: .utf8) }

    func vault() throws -> GitVault {
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let vault = try GitVault.prepare(at: root).vault
        try write("а.md", "первое"); _ = try vault.commitAll(message: "а")
        try write("б.md", "второе"); _ = try vault.commitAll(message: "б")
        try write("а.md", "первое правка"); _ = try vault.commitAll(message: "правка а")
        return vault
    }

    @Test("история свежая сверху, с файлами")
    func history() throws {
        let h = try vault().history()
        #expect(h.prefix(3).map(\.message) == ["правка а", "б", "а"])
        #expect(h[1].files == [FileChange(path: "б.md", kind: .added)])
    }

    // «Дропнуть коммит»: уходит только он, более поздняя правка остаётся.
    @Test("отмена одного коммита не трогает остальные")
    func revert() throws {
        let v = try vault()
        try v.revert(v.history()[1].id)
        #expect(read("б.md") == nil)
        #expect(read("а.md") == "первое правка")
        #expect(try v.history()[0].message == "отменено: б")
    }

    // Откат — новым коммитом сверху: его самого можно отменить, а несохранённое не теряется.
    @Test("откат к версии: файлы как тогда, несохранённое — в истории")
    func restore() throws {
        let v = try vault()
        try write("в.md", "не сохранено")
        try v.restore(to: v.history()[2].id)
        #expect(read("а.md") == "первое")
        #expect(read("б.md") == nil)
        #expect(read("в.md") == nil)
        let h = try v.history()
        #expect(h[1].message == "перед откатом")
        try v.revert(h[0].id)
        #expect(read("в.md") == "не сохранено")
    }

    @Test("конфликт при отмене — ошибка, файлы не тронуты")
    func conflict() throws {
        let v = try vault()
        let first = try v.history()[2].id   // «а»: потом правили ту же строку
        #expect(throws: GitError.self) { try v.revert(first) }
        #expect(read("а.md") == "первое правка")
    }
}
