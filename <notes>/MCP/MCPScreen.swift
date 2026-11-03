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

    // Скиллы (03.11.2026, «скиллы отдельно, и их всякие разные»; «на трёх языках, сами под твою нейросеть»):
    // готовые инструкции, что нейросети делать с заметками. Язык — как у интерфейса. Нейросети ищем сами
    // по их папкам в домашней: Claude Code и Codex получают SKILL.md, Cursor — текст в буфер.
    struct Skill {
        let name: String
        let glyph: DotGlyph
        let ru, en, sr: (title: String, about: String, text: String)
        var local: (title: String, about: String, text: String) {
            switch Locale.current.language.languageCode?.identifier { case "en": en; case "sr": sr; default: ru }
        }
    }

    static let skills: [Skill] = [
        Skill(name: "notes-assistant", glyph: .search,
              ru: ("помощник по заметкам", "отвечает по твоим заметкам и ссылается на них",
                   "Перед ответом ищи в MCP notes (search_notes) и читай найденное (read_note). Ссылайся на путь заметки. Новое — отдельной заметкой (create_note), старые не переписывай."),
              en: ("notes assistant", "answers from your notes and cites them",
                   "Before answering, search MCP notes (search_notes) and read the hits (read_note). Cite the note path. Save new things as a separate note (create_note); never rewrite existing ones."),
              sr: ("pomoćnik za beleške", "odgovara iz tvojih beleški i navodi ih",
                   "Pre odgovora pretraži MCP notes (search_notes) i pročitaj pronađeno (read_note). Navedi putanju beleške. Novo sačuvaj kao posebnu belešku (create_note), postojeće ne menjaj.")),
        Skill(name: "notes-inbox", glyph: .notes,
              ru: ("разбор входящих", "раскладывает «Входящие» на задачи и заметки",
                   "Прочитай «Входящие.md» через MCP notes. Дела — задачами (add_task, со сроком, если он есть), идеи — заметками в подходящие папки (create_note). В конце — короткий список, что куда ушло."),
              en: ("inbox triage", "turns your inbox into tasks and notes",
                   "Read «Входящие.md» via MCP notes. To-dos become tasks (add_task, with a due date if given), ideas become notes in fitting folders (create_note). Finish with a short list of what went where."),
              sr: ("sređivanje prijema", "pretvara prijem u zadatke i beleške",
                   "Pročitaj «Входящие.md» preko MCP notes. Obaveze — zadaci (add_task, sa rokom ako postoji), ideje — beleške u odgovarajuće fascikle (create_note). Na kraju kratka lista šta je gde otišlo.")),
        Skill(name: "notes-planner", glyph: .task,
              ru: ("план на день", "собирает открытые задачи в три главных дела",
                   "Возьми открытые задачи через MCP notes (list_tasks). Сгруппируй: сегодня, неделя, без срока. Предложи план на день — три главных дела. Новые задачи — через add_task во «Входящие.md»."),
              en: ("daily plan", "picks three key tasks from what is open",
                   "Get open tasks via MCP notes (list_tasks). Group them: today, this week, no date. Propose a plan for the day with three key tasks. Add new tasks with add_task to «Входящие.md»."),
              sr: ("plan za dan", "bira tri glavna zadatka od otvorenih",
                   "Uzmi otvorene zadatke preko MCP notes (list_tasks). Grupiši: danas, ove nedelje, bez roka. Predloži plan za dan — tri glavna zadatka. Nove zadatke dodaj preko add_task u «Входящие.md».")),
        Skill(name: "notes-weekly", glyph: .calendar,
              ru: ("итоги недели", "что сделано и что осталось — новой заметкой",
                   "Через MCP notes посмотри заметки за 7 дней (list_notes) и задачи (list_tasks с include_done). Итоги: сделано, осталось, застряло. Сохрани заметкой «Итоги недели ГГГГ-ММ-ДД» (create_note)."),
              en: ("weekly review", "what got done and what is left, as a note",
                   "Via MCP notes, look at notes from the last 7 days (list_notes) and tasks (list_tasks with include_done). Summarize: done, left, stuck. Save as a note «Weekly review YYYY-MM-DD» (create_note)."),
              sr: ("pregled nedelje", "šta je urađeno i šta je ostalo, kao beleška",
                   "Preko MCP notes pogledaj beleške iz poslednjih 7 dana (list_notes) i zadatke (list_tasks sa include_done). Rezime: urađeno, ostalo, zapelo. Sačuvaj kao belešku «Pregled nedelje GGGG-MM-DD» (create_note).")),
        Skill(name: "notes-meeting", glyph: .microphone,
              ru: ("конспект встречи", "из сырого текста — решения и задачи",
                   "Из сырого текста встречи выдели решения, вопросы и задачи со сроками. Сохрани конспект заметкой (create_note) в «Работа», задачи — через add_task в эту же заметку."),
              en: ("meeting notes", "decisions and tasks from a raw transcript",
                   "From the raw meeting text, extract decisions, open questions and tasks with dates. Save the summary as a note (create_note) in «Работа», and the tasks with add_task into the same note."),
              sr: ("zapisnik sa sastanka", "odluke i zadaci iz sirovog teksta",
                   "Iz sirovog teksta sastanka izdvoji odluke, pitanja i zadatke sa rokovima. Sačuvaj zapisnik kao belešku (create_note) u «Работа», a zadatke preko add_task u istu belešku.")),
        Skill(name: "notes-links", glyph: .spark,
              ru: ("связи между заметками", "находит заметки на ту же тему",
                   "Прочитай заметку, которую я назову (read_note). Найди через search_notes заметки на ту же тему. Перечисли их с путями и одной строкой — чем связаны. Заметки не меняй."),
              en: ("related notes", "finds notes on the same topic",
                   "Read the note I name (read_note). Use search_notes to find notes on the same topic. List them with paths and one line on how they relate. Do not change any notes."),
              sr: ("povezane beleške", "pronalazi beleške na istu temu",
                   "Pročitaj belešku koju navedem (read_note). Preko search_notes pronađi beleške na istu temu. Navedi ih sa putanjama i jednim redom — kako su povezane. Beleške ne menjaj.")),
    ]

    // Какие нейросети стоят на этом Mac — по их папкам в домашней.
    static var installedAgents: [Agent] {
        let home = FileManager.default.homeDirectoryForCurrentUser
        let folders: [(Agent, String)] = [(.claude, ".claude"), (.codex, ".codex"), (.cursor, ".cursor")]
        return folders.filter { FileManager.default.fileExists(atPath: home.appendingPathComponent($0.1).path) }.map(\.0)
    }

    static func skillFolder(_ agent: Agent, _ skill: Skill) -> URL? {
        let home = FileManager.default.homeDirectoryForCurrentUser
        switch agent {
        case .claude: return home.appendingPathComponent(".claude/skills/\(skill.name)")
        case .codex: return home.appendingPathComponent(".codex/skills/\(skill.name)")
        case .cursor: return nil
        }
    }

    static func installed(_ skill: Skill) -> Bool {
        let agents = installedAgents.filter { $0 != .cursor }
        return !agents.isEmpty && agents.allSatisfy { agent in
            skillFolder(agent, skill).map { FileManager.default.fileExists(atPath: $0.appendingPathComponent("SKILL.md").path) } ?? false
        }
    }

    // Ставит во все найденные нейросети, кроме Cursor (у него нет папки скиллов).
    static func install(_ skill: Skill) throws {
        let local = skill.local
        for agent in installedAgents {
            guard let folder = skillFolder(agent, skill) else { continue }
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            try """
            ---
            name: \(skill.name)
            description: \(local.title) — \(local.about). MCP notes (<notes>).
            ---

            \(local.text)

            """.write(to: folder.appendingPathComponent("SKILL.md"), atomically: true, encoding: .utf8)
        }
    }

    static func append(_ tool: String, _ target: String, chars: Int, to url: URL) {
        let line = "\(Int(Date().timeIntervalSince1970))\t\(tool)\t\(chars)\t\(target.replacingOccurrences(of: "\n", with: " "))\n"
        if let handle = try? FileHandle(forWritingTo: url) {
            handle.seekToEndOfFile(); handle.write(Data(line.utf8)); try? handle.close()
        } else {
            try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? line.write(to: url, atomically: true, encoding: .utf8)
        }
    }

    // Последние обращения, свежие сверху.
    static func recent(_ limit: Int = 50) -> [(date: Date, tool: String, target: String)] {
        entries().suffix(limit).reversed().map { ($0.date, $0.tool, $0.target) }
    }

    // Старые строки без длины (до 03.11.2026) читаются с нулём.
    static func entries() -> [(date: Date, tool: String, chars: Int, target: String)] {
        let text = (try? String(contentsOf: url, encoding: .utf8)) ?? ""
        return text.split(separator: "\n").compactMap { line in
            let parts = line.split(separator: "\t", omittingEmptySubsequences: false).map(String.init)
            guard parts.count >= 3, let time = TimeInterval(parts[0]) else { return nil }
            if parts.count >= 4, let chars = Int(parts[2]) {
                return (Date(timeIntervalSince1970: time), parts[1], chars, parts[3...].joined(separator: "\t"))
            }
            return (Date(timeIntervalSince1970: time), parts[1], 0, parts[2...].joined(separator: "\t"))
        }
    }

    // Примерно, сколько токенов ушло нейросети: знаков ответа / 3 (русский текст плотнее английского).
    static func tokens() -> Int { entries().reduce(0) { $0 + $1.chars } / 3 }

    static func clear() { try? FileManager.default.removeItem(at: url) }
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
            SidebarRow(glyph: .spark, title: "подключение", selected: !model.mcpSkills) { model.mcpSkills = false }
            SidebarRow(glyph: .file, title: "скиллы", count: MCPLog.skills.count, selected: model.mcpSkills) { model.mcpSkills = true }
        }
    }
}

struct MCPScreen: View {
    @Environment(\.pageWidth) private var pageWidth
    let model: NotesModel
    @State private var readOnly = MCPLog.readOnly
    @State private var copied: String?
    @State private var agent = MCPLog.Agent.claude
    @State private var hidden = MCPLog.hiddenFolders
    @State private var recent = MCPLog.recent()
    @State private var tokens = MCPLog.tokens()

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 5) {
                Text(model.mcpSkills ? "скиллы" : "mcp")
                    .font(Martian.heading.font(26))
                    .kerning(Theme.tracking(-7, size: 26))
                    .foregroundStyle(Theme.ink)
                Text(model.mcpSkills ? "готовые сценарии для нейросети: что делать с твоими заметками"
                                     : "локальный сервер: claude code, codex и cursor работают с заметками на этом mac, без сети")
                    .font(Martian.body.font(13))
                    .kerning(Theme.tracking(-7, size: 13))
                    .foregroundStyle(NotesLayout.gray)
                    .padding(.top, 1).padding(.bottom, 40)
                if model.mcpSkills { skills } else { agentRow; connection }
            }
            .frame(width: min(832, pageWidth), alignment: .leading)
            .padding(.leading, 58).padding(.top, 36).padding(.bottom, 40)
        }
        .scrollIndicators(.never)
        .task {
            #if DEBUG
            if let name = UserDefaults.standard.string(forKey: "mcpAgent"), let item = MCPLog.Agent(rawValue: name) { agent = item }
            if UserDefaults.standard.bool(forKey: "mcpSkills") { model.mcpSkills = true }
            #endif
            // Журнал пишет другой процесс (сам сервер) — перечитываем, пока раздел открыт.
            while !Task.isCancelled {
                recent = MCPLog.recent()
                tokens = MCPLog.tokens()
                try? await Task.sleep(for: .seconds(3))
            }
        }
    }

    private var agentRow: some View {
        SettingRow(title: "нейросеть", subtitle: agent.rawValue) {
            Menu("изменить") {
                ForEach(MCPLog.Agent.allCases, id: \.self) { item in Button(item.rawValue) { agent = item; copied = nil } }
            }
            .menuStyle(.button).buttonStyle(WhitePillStyle()).fixedSize()
        }
    }

    @ViewBuilder private var connection: some View {
        SettingRow(title: "подключить", subtitle: agent == .cursor ? String(localized: "вставь в ~/.cursor/mcp.json")
                                                                    : String(localized: "команда в терминал, один раз")) {
            copyButton("connect", MCPLog.connect(agent))
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
        SwitchRow(title: "только чтение", subtitle: "нейросеть ищет и читает, но не создаёт заметки и задачи",
                  isOn: $readOnly) { MCPLog.readOnly = $0; readOnly = $0 }
        HStack {
            Text("последние обращения").font(Martian.body.font(13)).foregroundStyle(Theme.ink)
            Text("≈ \(tokens.formatted()) токенов отдано нейросети").font(Martian.body.font(12)).foregroundStyle(NotesLayout.gray)
            Spacer()
            if !recent.isEmpty {
                Button("очистить") { MCPLog.clear(); recent = []; tokens = 0 }.buttonStyle(QuietLinkStyle(size: 12, opacity: 0.6))
            }
        }
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

    // Скиллы карточками; ставятся сразу во все найденные нейросети.
    private var skills: some View {
        let agents = MCPLog.installedAgents
        return VStack(alignment: .leading, spacing: 12) {
            Text(agents.isEmpty ? String(localized: "нейросетей на этом mac не нашли — поставь claude code, codex или cursor")
                                : String(localized: "найдено: ") + agents.map(\.rawValue).joined(separator: ", "))
                .font(Martian.body.font(12)).foregroundStyle(NotesLayout.gray)
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 250), spacing: 12)], spacing: 12) {
                ForEach(MCPLog.skills, id: \.name) { skill in SkillCard(skill: skill, cursorOnly: agents == [.cursor]) }
            }
        }
        .frame(width: min(640, pageWidth))
    }
}

private struct SkillCard: View {
    let skill: MCPLog.Skill
    let cursorOnly: Bool
    @State private var done = false
    @State private var hovered = false

    var body: some View {
        let local = skill.local
        VStack(alignment: .leading, spacing: 8) {
            DotIcon(glyph: skill.glyph, size: 20, color: Theme.coral)
            Text(local.title).font(Martian.heading.font(15)).kerning(Theme.tracking(-7, size: 15)).foregroundStyle(Theme.ink)
            Text(local.about).font(Martian.body.font(12)).foregroundStyle(NotesLayout.gray)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 4)
            Button(done || MCPLog.installed(skill) ? (cursorOnly ? "скопировано" : "установлен") : (cursorOnly ? "скопировать" : "установить")) {
                if cursorOnly {
                    NSPasteboard.general.clearContents(); NSPasteboard.general.setString(local.text, forType: .string); done = true
                } else {
                    withAnimation(.smooth) { done = (try? MCPLog.install(skill)) != nil }
                }
            }
            .buttonStyle(WhitePillStyle())
        }
        .padding(16)
        .frame(maxWidth: .infinity, minHeight: 170, alignment: .topLeading)
        .background(RoundedRectangle(cornerRadius: 16).fill(Theme.fg.opacity(hovered ? 0.07 : 0.04)))
        .scaleEffect(hovered ? 1.01 : 1)
        .animation(.smooth(duration: 0.25), value: hovered)
        .onHover { hovered = $0 }
    }
}
