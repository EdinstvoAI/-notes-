import AppKit
import NotesCore
import SwiftUI

// Раздел «синхронизация» (03.11.2026): версии хранилища из git и откат — «как git, но графически».
// Открывается по капсуле синхронизации в шапке. Работает и без GitHub — на одном локальном git.
// Мой экран на согласование: своего кадра у Макара пока нет.
@MainActor @Observable
final class SyncHistory {
    private(set) var commits: [VaultCommit] = []
    private(set) var hasRemote = false
    private(set) var isRepo = true
    private(set) var busy = false
    var selected: String?
    var error: String?

    func load(_ root: URL) {
        Task {
            let result = await Task.detached { () -> ([VaultCommit], Bool, Bool) in
                guard let git = try? GitVault.open(at: root) else { return ([], false, false) }
                return ((try? git.history()) ?? [], git.remoteURL() != nil, true)
            }.value
            (commits, hasRemote, isRepo) = result
            if selected == nil || !commits.contains(where: { $0.id == selected }) { selected = commits.first?.id }
        }
    }

    // Отмена и откат — новым коммитом; файлы меняются на диске, заметки подхватывают слежку за папкой.
    func run(_ root: URL, _ action: @escaping @Sendable (GitVault) throws -> Void) {
        busy = true
        error = nil
        Task {
            let failure = await Task.detached { () -> String? in
                do { try action(try GitVault.open(at: root)); return nil } catch { return error.localizedDescription }
            }.value
            busy = false
            error = failure
            selected = nil
            load(root)
        }
    }
}

struct SyncSidebar: View {
    let model: NotesModel

    var body: some View {
        let history = model.history
        SidebarList(title: "синхронизация") {
            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    SyncGraph(history: history).padding(.bottom, 16)
                    ForEach(Array(history.commits.enumerated()), id: \.element.id) { index, commit in
                        if index == 0 || !Calendar.current.isDate(commit.date, inSameDayAs: history.commits[index - 1].date) {
                            Text(commit.date.formatted(.dateTime.day().month(.wide).locale(.app)))
                                .font(Martian.body.font(11))
                                .foregroundStyle(NotesLayout.gray)
                                .padding(.leading, 10).padding(.top, index == 0 ? 0 : 12).padding(.bottom, 4)
                        }
                        SidebarRow(glyph: .git,
                                   title: commit.date.formatted(.dateTime.hour().minute().locale(.app)) + "  " + commit.message,
                                   selected: history.selected == commit.id) { history.selected = commit.id }
                    }
                }
            }
            .scrollIndicators(.never)
        }
        .task(id: model.section) { history.load(model.library.root) }
    }
}

struct SyncScreen: View {
    @Environment(\.pageWidth) private var pageWidth
    let model: NotesModel
    @State private var confirm: Confirm?
    @State private var open: String?
    @State private var preview: String?

    enum Confirm: Identifiable { case revert(VaultCommit), restore(VaultCommit)
        var id: String { switch self { case .revert(let c): "r" + c.id; case .restore(let c): "s" + c.id } }
    }

    var body: some View {
        let history = model.history
        let root = model.library.root
        ScrollView {
            VStack(alignment: .leading, spacing: 5) {
                if !history.isRepo {
                    heading("истории нет", "включи git в настройках → синхронизация")
                } else if let commit = history.commits.first(where: { $0.id == history.selected }) {
                    let pushed = SyncController.lastPush.map { commit.date <= $0 } ?? false
                    heading(commit.message, commit.date.formatted(.dateTime.day().month(.wide).hour().minute().locale(.app))
                        + " · " + (history.hasRemote ? (pushed ? String(localized: "на github") : String(localized: "ещё не на github"))
                                                     : String(localized: "только на этом mac")))
                    // Как в Time Machine: клик по файлу — он, каким был в этой версии, и «вернуть этот файл».
                    ForEach(commit.files, id: \.path) { file in
                        // Удалённый файл смотрим и возвращаем из версии до удаления.
                        let source = file.kind == .deleted ? older(than: commit.id) ?? commit.id : commit.id
                        Button { toggle(file.path, commit: source, root: root) } label: {
                            SettingRow(title: file.path, subtitle: kind(file.kind)) {
                                if open == file.path {
                                    Button("вернуть этот файл") {
                                        let path = file.path, id = source
                                        history.run(root) { try $0.restoreFile(path, from: id) }
                                    }
                                    .buttonStyle(WhitePillStyle())
                                }
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        if open == file.path {
                            Text(preview ?? String(localized: "в этой версии файла нет"))
                                .font(Martian.body.font(12))
                                .foregroundStyle(Theme.ink.opacity(preview == nil ? 0.4 : 0.8))
                                .textSelection(.enabled)
                                .padding(14)
                                .frame(width: 640, alignment: .leading)
                                .background(RoundedRectangle(cornerRadius: 12).fill(Theme.bg.opacity(0.5)))
                                .transition(.blurReplace)
                        }
                    }
                    if let error = history.error {
                        Text(error).font(Martian.body.font(12)).foregroundStyle(Theme.coral).padding(.top, 12)
                    }
                    HStack(spacing: 8) {
                        Button("отменить эти правки") { confirm = .revert(commit) }
                            .buttonStyle(WhitePillStyle())
                        if commit.id != history.commits.first?.id {
                            Button("вернуть всё к этой версии") { confirm = .restore(commit) }
                                .buttonStyle(WhitePillStyle())
                        }
                    }
                    .disabled(history.busy)
                    .padding(.top, 24)
                    Text("откат — новая версия сверху: его тоже можно отменить")
                        .font(Martian.body.font(11)).foregroundStyle(NotesLayout.gray).padding(.top, 10)
                } else {
                    heading("версий пока нет", "приложение сохраняет версию по расписанию")
                }
            }
            .frame(width: min(832, pageWidth), alignment: .leading)
            .padding(.leading, 58)
            .padding(.top, 36)
            .padding(.bottom, 40)
        }
        .scrollIndicators(.never)
        .onChange(of: history.selected) {
            open = nil
            #if DEBUG
            // -syncPreview YES — сразу открыть первый файл версии, для снимка.
            if UserDefaults.standard.bool(forKey: "syncPreview"), let id = history.selected,
               let file = history.commits.first(where: { $0.id == id })?.files.first {
                toggle(file.path, commit: file.kind == .deleted ? older(than: id) ?? id : id, root: root)
            }
            #endif
        }
        .alert(item: $confirm) { item in
            switch item {
            case .revert(let commit):
                Alert(title: Text("отменить «\(commit.message)»?"),
                      message: Text("уйдут только правки этой версии, всё, что было после, останется"),
                      primaryButton: .destructive(Text("отменить правки")) { history.run(root) { try $0.revert(commit.id) } },
                      secondaryButton: .cancel(Text("не надо")))
            case .restore(let commit):
                Alert(title: Text("вернуть всё к версии от \(commit.date.formatted(.dateTime.day().month(.wide).hour().minute().locale(.app)))?"),
                      message: Text("все заметки станут как тогда; текущее состояние останется в истории"),
                      primaryButton: .destructive(Text("вернуть")) { history.run(root) { try $0.restore(to: commit.id) } },
                      secondaryButton: .cancel(Text("не надо")))
            }
        }
    }

    private func heading(_ title: String, _ subtitle: String) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(LocalizedStringKey(title))
                .font(Martian.heading.font(26))
                .kerning(Theme.tracking(-7, size: 26))
                .foregroundStyle(Theme.ink)
            Text(LocalizedStringKey(subtitle))
                .font(Martian.body.font(13))
                .kerning(Theme.tracking(-7, size: 13))
                .foregroundStyle(NotesLayout.gray)
                .padding(.top, 1)
                .padding(.bottom, 40)
        }
    }

    private func older(than id: String) -> String? {
        let commits = model.history.commits
        return commits.firstIndex { $0.id == id }.flatMap { $0 + 1 < commits.count ? commits[$0 + 1].id : nil }
    }

    private func toggle(_ path: String, commit: String, root: URL) {
        guard open != path else { withAnimation(.smooth) { open = nil }; return }
        let text = (try? GitVault.open(at: root)).flatMap { try? $0.text(of: path, at: commit) }
        withAnimation(.smooth) { preview = text; open = path }
    }

    private func kind(_ kind: FileChange.Kind) -> String {
        switch kind {
        case .added: String(localized: "новый")
        case .modified: String(localized: "изменён")
        case .deleted: String(localized: "удалён")
        }
    }
}

// Граф правок над датами: столбик — день за последние две недели, высота — сколько версий.
// Клик — последняя версия дня; выбранный день коралловый.
private struct SyncGraph: View {
    let history: SyncHistory

    var body: some View {
        let calendar = Calendar.current
        let today = calendar.startOfDay(for: .now)
        let days = (0..<14).reversed().compactMap { calendar.date(byAdding: .day, value: -$0, to: today) }
        let counts = days.map { day in history.commits.filter { calendar.isDate($0.date, inSameDayAs: day) }.count }
        let top = max(counts.max() ?? 1, 1)
        let selected = history.commits.first { $0.id == history.selected }?.date
        HStack(alignment: .bottom, spacing: 4) {
            ForEach(Array(days.enumerated()), id: \.offset) { index, day in
                let active = selected.map { calendar.isDate($0, inSameDayAs: day) } ?? false
                RoundedRectangle(cornerRadius: 2)
                    .fill(active ? Theme.coral : Theme.ink.opacity(counts[index] == 0 ? 0.08 : 0.3))
                    .frame(height: 4 + 44 * CGFloat(counts[index]) / CGFloat(top))
                    .frame(maxWidth: .infinity)
                    .contentShape(Rectangle())
                    .onTapGesture {
                        if let commit = history.commits.first(where: { calendar.isDate($0.date, inSameDayAs: day) }) {
                            history.selected = commit.id
                        }
                    }
                    .help(day.formatted(.dateTime.day().month(.wide).locale(.app)) + " · \(counts[index])")
            }
        }
        .frame(height: 48, alignment: .bottom)
        .padding(.horizontal, 10)
        .animation(.smooth(duration: 0.25), value: history.selected)
    }
}
