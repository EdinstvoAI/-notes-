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
    // «граф» в панели — большая доска со схемой версий вместо списка (03.11.2026).
    var board = false
    // Открытая карточка версии на доске.
    var card: String?

    func load(_ root: URL) {
        Task {
            let result = await Task.detached { () -> ([VaultCommit], Bool, Bool) in
                guard let git = try? GitVault.open(at: root) else { return ([], false, false) }
                return ((try? git.history()) ?? [], git.remoteURL() != nil, true)
            }.value
            (commits, hasRemote, isRepo) = result
            if selected == nil || !commits.contains(where: { $0.id == selected }) { selected = commits.first?.id }
            #if DEBUG
            // -syncBoard YES — доска графа, -syncCard YES — с открытой карточкой: для снимков.
            if UserDefaults.standard.bool(forKey: "syncBoard") { board = true }
            #endif
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

// Панель: сверху «граф» (доска на всё окно), ниже — версии по дням линией метро (03.11.2026).
struct SyncSidebar: View {
    let model: NotesModel

    var body: some View {
        let history = model.history
        SidebarList(title: "синхронизация") {
            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    SidebarRow(glyph: .spark, title: "граф", selected: history.board) { history.board.toggle() }
                        .padding(.bottom, 12)
                    // Веток пока нет — одна линия; появятся — встанут рядом своими линиями.
                    ForEach(Array(history.commits.enumerated()), id: \.element.id) { index, commit in
                        let last = index == history.commits.count - 1
                        if index == 0 || !Calendar.current.isDate(commit.date, inSameDayAs: history.commits[index - 1].date) {
                            HStack(spacing: 8) {
                                MetroLine(top: index > 0, bottom: true, stop: nil)
                                Text(commit.date.formatted(.dateTime.day().month(.wide).locale(.app)))
                                    .font(Martian.body.font(11))
                                    .foregroundStyle(NotesLayout.gray)
                            }
                            .frame(height: index == 0 ? 22 : 34, alignment: .bottom)
                        }
                        Button {
                            history.selected = commit.id
                            if history.board { history.card = commit.id }
                        } label: {
                            HStack(spacing: 8) {
                                MetroLine(top: true, bottom: !last, stop: history.selected == commit.id)
                                Text(commit.date.formatted(.dateTime.hour().minute().locale(.app)))
                                    .font(Martian.body.font(11))
                                    .foregroundStyle(NotesLayout.gray)
                                Text(commit.message)
                                    .font(Martian.body.font(13))
                                    .kerning(Theme.tracking(-7, size: 13))
                                    .foregroundStyle(Theme.ink)
                                    .lineLimit(1)
                                Spacer(minLength: 0)
                            }
                            .padding(.trailing, 8)
                            .frame(height: 28)
                            .background(RoundedRectangle(cornerRadius: 8).fill(Theme.fg.opacity(history.selected == commit.id ? 0.07 : 0)))
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
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

    var body: some View {
        let history = model.history
        if !history.isRepo {
            page { heading("истории нет", "включи git в настройках → синхронизация") }
        } else if history.commits.isEmpty {
            page { heading("версий пока нет", "приложение сохраняет версию по расписанию") }
        } else if history.board {
            SyncBoard(model: model)
        } else if let commit = history.commits.first(where: { $0.id == history.selected }) {
            page {
                heading(commit.message, VersionDetails.when(commit, history))
                VersionDetails(model: model, commit: commit, width: 640)
            }
        }
    }

    private func page<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 5) { content() }
                .frame(width: min(832, pageWidth), alignment: .leading)
                .padding(.leading, 58)
                .padding(.top, 36)
                .padding(.bottom, 40)
        }
        .scrollIndicators(.never)
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
}

// Файлы версии и откат — и на странице версии, и в карточке на доске.
// Как в Time Machine: клик по файлу — он, каким был в этой версии, и «вернуть этот файл».
private struct VersionDetails: View {
    let model: NotesModel
    let commit: VaultCommit
    let width: CGFloat
    @State private var confirm: Confirm?
    @State private var open: String?
    @State private var preview: String?

    enum Confirm: Identifiable { case revert, restore
        var id: Self { self }
    }

    static func when(_ commit: VaultCommit, _ history: SyncHistory) -> String {
        let pushed = SyncController.lastPush.map { commit.date <= $0 } ?? false
        return commit.date.formatted(.dateTime.day().month(.wide).hour().minute().locale(.app)) + " · "
            + (history.hasRemote ? (pushed ? String(localized: "на github") : String(localized: "ещё не на github"))
                                 : String(localized: "только на этом mac"))
    }

    var body: some View {
        let history = model.history
        let root = model.library.root
        VStack(alignment: .leading, spacing: 5) {
            ForEach(commit.files, id: \.path) { file in
                // Удалённый файл смотрим и возвращаем из версии до удаления.
                let source = file.kind == .deleted ? older(than: commit.id) ?? commit.id : commit.id
                Button { toggle(file.path, commit: source, root: root) } label: {
                    // Своя строка, а не SettingRow: та всегда 640, в карточке на доске вылезала за край.
                    HStack {
                        VStack(alignment: .leading, spacing: 1) {
                            Text(file.path).font(Martian.body.font(13)).foregroundStyle(Theme.ink).lineLimit(1)
                            Text(kind(file.kind)).font(Martian.body.font(11)).foregroundStyle(NotesLayout.gray)
                        }
                        Spacer(minLength: 8)
                        if open == file.path {
                            Button("вернуть этот файл") {
                                let path = file.path
                                history.run(root) { try $0.restoreFile(path, from: source) }
                            }
                            .buttonStyle(WhitePillStyle())
                        }
                    }
                    .padding(.horizontal, 14)
                    .frame(width: width, height: 50)
                    .background(RoundedRectangle(cornerRadius: 14).fill(Theme.fg.opacity(0.04)))
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                if open == file.path {
                    Text(preview ?? String(localized: "в этой версии файла нет"))
                        .font(Martian.body.font(12))
                        .foregroundStyle(Theme.ink.opacity(preview == nil ? 0.4 : 0.8))
                        .textSelection(.enabled)
                        .padding(14)
                        .frame(width: width, alignment: .leading)
                        .background(RoundedRectangle(cornerRadius: 12).fill(Theme.bg.opacity(0.5)))
                        .transition(.blurReplace)
                }
            }
            if let error = history.error {
                Text(error).font(Martian.body.font(12)).foregroundStyle(Theme.coral).padding(.top, 12)
            }
            HStack(spacing: 8) {
                Button("отменить эти правки") { confirm = .revert }
                    .buttonStyle(WhitePillStyle())
                if commit.id != history.commits.first?.id {
                    Button("вернуть всё к этой версии") { confirm = .restore }
                        .buttonStyle(WhitePillStyle())
                }
            }
            .disabled(history.busy)
            .padding(.top, 20)
            Text("откат — новая версия сверху: его тоже можно отменить")
                .font(Martian.body.font(11)).foregroundStyle(NotesLayout.gray).padding(.top, 8)
        }
        .frame(width: width, alignment: .leading)
        .onChange(of: commit.id) { open = nil }
        .task {
            #if DEBUG
            // -syncPreview YES — сразу открыть первый файл версии, для снимка.
            if UserDefaults.standard.bool(forKey: "syncPreview"), let file = commit.files.first {
                toggle(file.path, commit: file.kind == .deleted ? older(than: commit.id) ?? commit.id : commit.id, root: root)
            }
            #endif
        }
        .alert(item: $confirm) { item in
            switch item {
            case .revert:
                Alert(title: Text("отменить «\(commit.message)»?"),
                      message: Text("уйдут только правки этой версии, всё, что было после, останется"),
                      primaryButton: .destructive(Text("отменить правки")) { history.run(root) { [id = commit.id] in try $0.revert(id) } },
                      secondaryButton: .cancel(Text("не надо")))
            case .restore:
                Alert(title: Text("вернуть всё к версии от \(commit.date.formatted(.dateTime.day().month(.wide).hour().minute().locale(.app)))?"),
                      message: Text("все заметки станут как тогда; текущее состояние останется в истории"),
                      primaryButton: .destructive(Text("вернуть")) { history.run(root) { [id = commit.id] in try $0.restore(to: id) } },
                      secondaryButton: .cancel(Text("не надо")))
            }
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

// Доска графа на всё окно: линия метро сверху вниз, свежие версии сверху. Клик по станции — карточка
// рядом, как у события в календаре. Масштаб — щипком или ⌘+ / ⌘−: шаг между станциями 36…140.
private struct SyncBoard: View {
    let model: NotesModel
    @State private var step: CGFloat = 64
    @State private var pinch: CGFloat = 1

    var body: some View {
        let history = model.history
        let commits = history.commits
        let calendar = Calendar.current
        let gap = min(140, max(36, step * pinch))
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(commits.enumerated()), id: \.element.id) { index, commit in
                        let selected = history.selected == commit.id
                        let newDay = index == 0 || !calendar.isDate(commit.date, inSameDayAs: commits[index - 1].date)
                        HStack(spacing: 14) {
                            Text(newDay ? commit.date.formatted(.dateTime.day().month(.abbreviated).locale(.app)) : "")
                                .font(Martian.body.font(11))
                                .foregroundStyle(NotesLayout.gray)
                                .frame(width: 70, alignment: .trailing)
                            ZStack {
                                VStack(spacing: 0) {
                                    Rectangle().fill(index > 0 ? Theme.ink.opacity(0.25) : .clear)
                                    Rectangle().fill(index < commits.count - 1 ? Theme.ink.opacity(0.25) : .clear)
                                }
                                .frame(width: 3)
                                Circle()
                                    .fill(selected ? Theme.coral : Theme.bg)
                                    .overlay(Circle().strokeBorder(selected ? Theme.coral : Theme.ink.opacity(0.5), lineWidth: 3))
                                    .frame(width: selected ? 22 : 16, height: selected ? 22 : 16)
                                    .popover(isPresented: Binding(get: { history.card == commit.id },
                                                                  set: { if !$0, history.card == commit.id { history.card = nil } }),
                                             arrowEdge: .trailing) {
                                        VStack(alignment: .leading, spacing: 6) {
                                            Text(commit.message).font(Martian.heading.font(17)).foregroundStyle(Theme.ink)
                                            Text(VersionDetails.when(commit, history))
                                                .font(Martian.body.font(12)).foregroundStyle(NotesLayout.gray)
                                                .padding(.bottom, 10)
                                            VersionDetails(model: model, commit: commit, width: 360)
                                        }
                                        .padding(18)
                                    }
                            }
                            .frame(width: 24)
                            VStack(alignment: .leading, spacing: 1) {
                                Text(commit.message)
                                    .font(Martian.body.font(13))
                                    .kerning(Theme.tracking(-7, size: 13))
                                    .foregroundStyle(Theme.ink.opacity(selected ? 1 : 0.75))
                                    .lineLimit(1)
                                Text(commit.date.formatted(.dateTime.hour().minute().locale(.app))
                                     + " · " + String(localized: "\(commit.files.count) файлов"))
                                    .font(Martian.body.font(11))
                                    .foregroundStyle(NotesLayout.gray)
                            }
                            Spacer(minLength: 0)
                        }
                        .frame(height: gap)
                        .contentShape(Rectangle())
                        .onTapGesture { history.selected = commit.id; history.card = commit.id }
                        .id(commit.id)
                    }
                }
                .padding(.leading, 20)
                .padding(.vertical, 30)
                .animation(.smooth(duration: 0.25), value: history.selected)
            }
            .scrollIndicators(.never)
            .onChange(of: history.card) { if let id = history.card { withAnimation(.smooth) { proxy.scrollTo(id, anchor: .center) } } }
        }
        .gesture(MagnifyGesture()
            .onChanged { pinch = $0.magnification }
            .onEnded { _ in step = min(140, max(36, step * pinch)); pinch = 1 })
        .background {
            Button("") { step = min(140, step * 1.25) }.keyboardShortcut("+", modifiers: .command).hidden()
            Button("") { step = max(36, step / 1.25) }.keyboardShortcut("-", modifiers: .command).hidden()
        }
        .task {
            #if DEBUG
            if UserDefaults.standard.bool(forKey: "syncCard") { history.card = history.selected }
            #endif
        }
    }
}
// Кусок линии метро у строки: отрезок сверху и снизу, посередине — станция (выбранная — крупнее, коралловая).
private struct MetroLine: View {
    let top: Bool
    let bottom: Bool
    let stop: Bool?

    var body: some View {
        VStack(spacing: 0) {
            Rectangle().fill(top ? Theme.ink.opacity(0.25) : .clear).frame(width: 2)
            if let stop {
                Circle()
                    .fill(stop ? Theme.coral : Theme.bg)
                    .overlay(Circle().strokeBorder(stop ? Theme.coral : Theme.ink.opacity(0.5), lineWidth: 2))
                    .frame(width: stop ? 12 : 9, height: stop ? 12 : 9)
                    .animation(.smooth(duration: 0.2), value: stop)
            }
            Rectangle().fill(bottom ? Theme.ink.opacity(0.25) : .clear).frame(width: 2)
        }
        .frame(width: 22)
    }
}
