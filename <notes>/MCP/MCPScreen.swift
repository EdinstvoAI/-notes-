import AppKit
import NotesCore
import SwiftUI

// Раздел MCP в шапке (03.11.2026): подключение нейронок, «только чтение» и журнал обращений.
// Журнал — вне хранилища, в Application Support: только инструмент и путь, без текста заметок.
nonisolated enum MCPLog {
    static let url = AppSupport.folder.appendingPathComponent("mcp.log")

    static var readOnly: Bool {
        get { UserDefaults.standard.bool(forKey: "mcpReadOnly") }
        set { UserDefaults.standard.set(newValue, forKey: "mcpReadOnly") }
    }

    static var hiddenFolders: Set<String> {
        get { Set(UserDefaults.standard.stringArray(forKey: "mcpHiddenFolders") ?? []) }
        set { UserDefaults.standard.set(Array(newValue).sorted(), forKey: "mcpHiddenFolders") }
    }

    enum Agent: String, CaseIterable { case claude = "Claude Code", codex = "Codex", cursor = "Cursor" }

    // Как подключить: Claude и Codex — команда в терминал, Cursor — кусок ~/.cursor/mcp.json.
    static func connect(_ agent: Agent) -> String {
        let app = Bundle.main.executablePath ?? ""
        switch agent {
        case .claude: return "claude mcp add -s user notes -- '\(app)' --mcp"
        case .codex: return "codex mcp add notes -- '\(app)' --mcp"
        case .cursor: return #"{ "mcpServers": { "notes": { "command": "\#(app)", "args": ["--mcp"] } } }"#
        }
    }

    // Готовые промты (03.11.2026, «выбор промта»): что нейронке делать с заметками.
    static let prompts: [(title: String, text: String)] = [
        ("помощник по заметкам", """
        У тебя есть MCP-сервер notes — мои Markdown-заметки. Прежде чем отвечать, ищи в них (search_notes) \
        и читай найденное (read_note). Ссылайся на путь заметки. Новое записывай отдельной заметкой (create_note), \
        старые не переписывай.
        """),
        ("разбор входящих", """
        Прочитай «Входящие.md» через MCP notes. Разложи мысли: дела — задачами (add_task, со сроком, если он есть), \
        идеи — отдельными заметками в подходящие папки (create_note). В конце — короткий список, что куда ушло.
        """),
        ("планировщик задач", """
        Через MCP notes возьми открытые задачи (list_tasks). Сгруппируй: сегодня, неделя, без срока. \
        Предложи план на день — три главных дела. Новые задачи добавляй через add_task в «Входящие.md».
        """),
    ]

    // Скилл для Claude Code: ~/.claude/skills/notes/SKILL.md — по кнопке, вне хранилища.
    static func installSkill(_ prompt: String) throws {
        let folder = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/skills/notes")
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        try """
        ---
        name: notes
        description: Работа с моими заметками <notes> через MCP-сервер notes — поиск, чтение, новые заметки и задачи.
        ---

        \(prompt)

        """.write(to: folder.appendingPathComponent("SKILL.md"), atomically: true, encoding: .utf8)
    }

    static func append(_ tool: String, _ target: String, to url: URL) {
        let line = "\(Int(Date().timeIntervalSince1970))\t\(tool)\t\(target.replacingOccurrences(of: "\n", with: " "))\n"
        if let handle = try? FileHandle(forWritingTo: url) {
            handle.seekToEndOfFile(); handle.write(Data(line.utf8)); try? handle.close()
        } else {
            try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? line.write(to: url, atomically: true, encoding: .utf8)
        }
    }

    // Последние обращения, свежие сверху.
    static func recent(_ limit: Int = 50) -> [(date: Date, tool: String, target: String)] {
        let text = (try? String(contentsOf: url, encoding: .utf8)) ?? ""
        return text.split(separator: "\n").suffix(limit).reversed().compactMap { line in
            let parts = line.split(separator: "\t", maxSplits: 2, omittingEmptySubsequences: false).map(String.init)
            guard parts.count == 3, let time = TimeInterval(parts[0]) else { return nil }
            return (Date(timeIntervalSince1970: time), parts[1], parts[2])
        }
    }
}

extension MCPScreen {
    func copyButton(_ key: String, _ text: String) -> some View {
        Button(copied == key ? "скопировано" : "скопировать") {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
            copied = key
        }
        .buttonStyle(WhitePillStyle())
    }
}

struct MCPSidebar: View {
    let model: NotesModel
    var body: some View {
        SidebarList(title: "mcp") {
            SidebarRow(glyph: .spark, title: "подключение", selected: true) {}
        }
    }
}

struct MCPScreen: View {
    @Environment(\.pageWidth) private var pageWidth
    let model: NotesModel
    @State private var readOnly = MCPLog.readOnly
    @State private var copied: String?
    @State private var agent = MCPLog.Agent.claude
    @State private var prompt = 0
    @State private var hidden = MCPLog.hiddenFolders
    @State private var recent = MCPLog.recent()

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 5) {
                Text("mcp")
                    .font(Martian.heading.font(26))
                    .kerning(Theme.tracking(-7, size: 26))
                    .foregroundStyle(Theme.ink)
                Text("claude, codex и другие ищут и пишут заметки — на этом mac, без сети")
                    .font(Martian.body.font(13))
                    .kerning(Theme.tracking(-7, size: 13))
                    .foregroundStyle(NotesLayout.gray)
                    .padding(.top, 1).padding(.bottom, 40)
                SettingRow(title: "нейронка", subtitle: agent.rawValue) {
                    Menu("изменить") {
                        ForEach(MCPLog.Agent.allCases, id: \.self) { item in Button(item.rawValue) { agent = item; copied = nil } }
                    }
                    .menuStyle(.button).buttonStyle(WhitePillStyle()).fixedSize()
                }
                SettingRow(title: "подключить", subtitle: agent == .cursor ? String(localized: "вставь в ~/.cursor/mcp.json")
                                                                            : String(localized: "команда в терминал, один раз")) {
                    copyButton("connect", MCPLog.connect(agent))
                }
                SettingRow(title: "промт", subtitle: MCPLog.prompts[prompt].title.localized) {
                    HStack(spacing: 8) {
                        Menu("изменить") {
                            ForEach(MCPLog.prompts.indices, id: \.self) { i in Button(MCPLog.prompts[i].title.localized) { prompt = i; copied = nil } }
                        }
                        .menuStyle(.button).buttonStyle(WhitePillStyle()).fixedSize()
                        if agent == .claude {
                            Button(copied == "skill" ? "установлен" : "установить скилл") {
                                copied = (try? MCPLog.installSkill(MCPLog.prompts[prompt].text)) != nil ? "skill" : nil
                            }
                            .buttonStyle(WhitePillStyle())
                        } else {
                            copyButton("prompt", MCPLog.prompts[prompt].text)
                        }
                    }
                }
                SettingRow(title: "закрытые папки", subtitle: hidden.isEmpty ? String(localized: "нет — видно всё хранилище")
                                                                           : hidden.sorted().joined(separator: ", ")) {
                    Menu("выбрать") {
                        ForEach(model.tree.folders.map(\.name), id: \.self) { name in
                            Toggle(name, isOn: Binding(get: { hidden.contains(name) }, set: { on in
                                if on { hidden.insert(name) } else { hidden.remove(name) }
                                MCPLog.hiddenFolders = hidden
                            }))
                        }
                    }
                    .menuStyle(.button).buttonStyle(WhitePillStyle()).fixedSize()
                }
                SwitchRow(title: "только чтение", subtitle: "нейронка ищет и читает, но не создаёт заметки и задачи",
                          isOn: $readOnly) { MCPLog.readOnly = $0; readOnly = $0 }
                SettingRow(title: "что доступно", subtitle: "только .md внутри хранилища: без .git, скрытых папок, ссылок наружу и закрытых папок") {
                    EmptyView()
                }
                Text("последние обращения")
                    .font(Martian.body.font(13)).foregroundStyle(Theme.ink)
                    .padding(.top, 32).padding(.bottom, 8)
                if recent.isEmpty {
                    Text("пока не было").font(Martian.body.font(12)).foregroundStyle(NotesLayout.gray)
                }
                ForEach(Array(recent.enumerated()), id: \.offset) { _, item in
                    HStack(spacing: 12) {
                        Text(item.date.formatted(.dateTime.day().month(.abbreviated).hour().minute().locale(.app)))
                            .foregroundStyle(NotesLayout.gray).frame(width: 110, alignment: .leading)
                        Text(item.tool).foregroundStyle(Theme.ink).frame(width: 110, alignment: .leading)
                        Text(item.target).foregroundStyle(Theme.ink.opacity(0.7)).lineLimit(1)
                    }
                    .font(Martian.body.font(12))
                }
            }
            .frame(width: min(832, pageWidth), alignment: .leading)
            .padding(.leading, 58).padding(.top, 36).padding(.bottom, 40)
        }
        .scrollIndicators(.never)
        .task {
            #if DEBUG
            if let name = UserDefaults.standard.string(forKey: "mcpAgent"), let item = MCPLog.Agent(rawValue: name) { agent = item }
            #endif
            // Журнал пишет другой процесс (сам сервер) — перечитываем, пока раздел открыт.
            while !Task.isCancelled {
                recent = MCPLog.recent()
                try? await Task.sleep(for: .seconds(3))
            }
        }
    }
}
