import Foundation
import NotesCore

// notes-mcp --vault <папка> — MCP-сервер по stdio для нейронок. Логика — в NotesCore/MCPServer.swift.
let args = CommandLine.arguments
guard let flag = args.firstIndex(of: "--vault"), flag + 1 < args.count else {
    FileHandle.standardError.write(Data("usage: notes-mcp --vault <path>\n".utf8))
    exit(2)
}
let root = URL(fileURLWithPath: (args[flag + 1] as NSString).expandingTildeInPath, isDirectory: true)
MCPServer(library: NoteLibrary(root: root)).run()
