// swift-tools-version: 6.0
import PackageDescription

// Логика приложения без UI: git в папке, вход в GitHub, отправка истории.
// Отдельный пакет, чтобы её можно было тестировать через `swift test` без Xcode.
let package = Package(
    name: "NotesCore",
    platforms: [.macOS(.v15)],
    products: [
        .library(name: "NotesCore", targets: ["NotesCore"]),
        // MCP-сервер для нейронок: `notes-mcp --vault <папка>`, stdio, без сети.
        .executable(name: "notes-mcp", targets: ["notes-mcp"]),
    ],
    dependencies: [
        // libgit2 собирается из исходников (MIT + исключения libgit2), без системных зависимостей.
        .package(url: "https://github.com/ibrahimcetin/libgit2.git", exact: "1.9.2"),
    ],
    targets: [
        .target(name: "NotesCore", dependencies: [.product(name: "libgit2", package: "libgit2")]),
        .executableTarget(name: "notes-mcp", dependencies: ["NotesCore"]),
        .testTarget(name: "NotesCoreTests", dependencies: ["NotesCore"]),
    ],
    swiftLanguageModes: [.v5]
)
