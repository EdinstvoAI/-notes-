import AppKit
import NotesCore
import SwiftUI

// Раздел «пароли» (03.11.2026): сейф с мастер-паролем — пароли, коды 2FA, секреты. Мой экран на согласование.
// Мастер-пароль живёт только в памяти, пока сейф открыт; копия в буфере стирается через минуту.
@MainActor @Observable
final class SafeModel {
    // Один сейф на приложение: его открывают и окно, и значок в строке меню.
    static let shared = SafeModel()
    private(set) var items: [SafeItem] = []
    private var password: String?
    var kind: SafeItem.Kind = .login
    var open: UUID?
    var error: String?
    var unlocked: Bool { password != nil }

    func unlock(_ vault: URL, _ text: String) {
        do {
            items = try PasswordSafe.load(vault: vault, password: text) ?? []
            if !PasswordSafe.exists(vault: vault) { try PasswordSafe.save([], vault: vault, password: text) }
            password = text
            error = nil
        } catch {
            self.error = String(localized: "пароль не подошёл")
        }
    }

    func lock() { password = nil; items = []; open = nil }

    func save(_ vault: URL) {
        guard let password else { return }
        do { try PasswordSafe.save(items, vault: vault, password: password) } catch { self.error = error.localizedDescription }
    }

    func add(_ vault: URL) {
        var item = SafeItem(kind: kind, title: String(localized: "без названия"))
        if kind == .login { item.secret = PasswordSafe.generate() }
        items.insert(item, at: 0)
        open = item.id
        save(vault)
    }

    func update(_ item: SafeItem, _ vault: URL) {
        guard let index = items.firstIndex(where: { $0.id == item.id }) else { return }
        var item = item
        item.modified = Date()
        items[index] = item
        save(vault)
    }

    func delete(_ id: UUID, _ vault: URL) { items.removeAll { $0.id == id }; save(vault) }

    static func copy(_ text: String) {
        let board = NSPasteboard.general
        board.clearContents()
        board.setString(text, forType: .string)
        let count = board.changeCount
        Task {
            try? await Task.sleep(for: .seconds(60))
            if board.changeCount == count { board.clearContents() }
        }
    }
}

extension SafeItem.Kind {
    var title: String {
        switch self {
        case .login: "пароли"
        case .totp: "коды 2fa"
        case .secret: "секреты"
        }
    }
}

struct SafeSidebar: View {
    let model: NotesModel

    var body: some View {
        let safe = model.safe
        SidebarList(title: "пароли") {
            ForEach(SafeItem.Kind.allCases, id: \.self) { kind in
                SidebarRow(glyph: kind == .totp ? .check : kind == .secret ? .spark : .file, title: kind.title,
                           count: safe.unlocked ? safe.items.filter { $0.kind == kind }.count : nil,
                           selected: safe.kind == kind) { safe.kind = kind; safe.open = nil }
            }
            if safe.unlocked {
                SidebarRow(glyph: .settings, title: "закрыть сейф", selected: false) { safe.lock() }
                    .padding(.top, 12)
            }
        }
    }
}

struct SafeScreen: View {
    @Environment(\.pageWidth) private var pageWidth
    let model: NotesModel
    @State private var entry = ""
    @State private var repeatEntry = ""

    var body: some View {
        let safe = model.safe
        let vault = model.library.root
        ScrollView {
            VStack(alignment: .leading, spacing: 5) {
                Text(LocalizedStringKey(safe.unlocked ? safe.kind.title : "пароли"))
                    .font(Martian.heading.font(26)).kerning(Theme.tracking(-7, size: 26)).foregroundStyle(Theme.ink)
                Text(safe.unlocked ? "в зашифрованном файле .safe/safe.enc, на github — только шифром"
                                   : "сейф открывается мастер-паролем — его не знает никто, кроме тебя")
                    .font(Martian.body.font(13)).kerning(Theme.tracking(-7, size: 13)).foregroundStyle(NotesLayout.gray)
                    .padding(.top, 1).padding(.bottom, 40)
                if safe.unlocked { items(safe, vault) } else { lockScreen(safe, vault) }
            }
            .frame(width: min(832, pageWidth), alignment: .leading)
            .padding(.leading, 58).padding(.top, 36).padding(.bottom, 40)
        }
        .scrollIndicators(.never)
        .task {
            #if DEBUG
            // -safeTest YES — только с тестовым хранилищем: сейф с тестовым паролем и примерами, для снимка.
            if UserDefaults.standard.bool(forKey: "safeTest"), UserDefaults.standard.bool(forKey: "testVault"), !safe.unlocked {
                safe.unlock(vault, "test")
                if safe.items.isEmpty {
                    safe.kind = .login; safe.add(vault)
                    var mail = SafeItem(kind: .login, title: "почта", username: "makar@example.com", secret: "x"); mail.id = safe.items[0].id; safe.update(mail, vault)
                    var code = SafeItem(kind: .totp, title: "github", secret: "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ")
                    code.username = "makark"
                    safe.kind = .totp; safe.add(vault); code.id = safe.items[0].id; safe.update(code, vault); safe.kind = .login
                }
                if let kind = UserDefaults.standard.string(forKey: "safeKind").flatMap(SafeItem.Kind.init(rawValue:)) { safe.kind = kind }
            }
            #endif
        }
    }

    @ViewBuilder private func lockScreen(_ safe: SafeModel, _ vault: URL) -> some View {
        let creating = !PasswordSafe.exists(vault: vault)
        SecureField(creating ? "придумай мастер-пароль" : "мастер-пароль", text: $entry)
            .textFieldStyle(.plain).font(Martian.body.font(15))
            .padding(14).frame(width: 360)
            .background(RoundedRectangle(cornerRadius: 14).fill(Theme.fg.opacity(0.04)))
            .onSubmit { if !creating { safe.unlock(vault, entry); entry = "" } }
        if creating {
            SecureField("ещё раз", text: $repeatEntry)
                .textFieldStyle(.plain).font(Martian.body.font(15))
                .padding(14).frame(width: 360)
                .background(RoundedRectangle(cornerRadius: 14).fill(Theme.fg.opacity(0.04)))
            Text("забудешь — сейф не откроет никто, даже мы. запиши пароль на бумаге")
                .font(Martian.body.font(12)).foregroundStyle(Theme.coral).padding(.top, 6)
        }
        if let error = safe.error { Text(error).font(Martian.body.font(12)).foregroundStyle(Theme.coral).padding(.top, 6) }
        Button(creating ? "создать сейф" : "открыть") {
            safe.unlock(vault, entry); entry = ""; repeatEntry = ""
        }
        .buttonStyle(WhitePillStyle())
        .disabled(entry.count < 8 || (creating && entry != repeatEntry))
        .padding(.top, 14)
    }

    @ViewBuilder private func items(_ safe: SafeModel, _ vault: URL) -> some View {
        Button("добавить") { safe.add(vault) }.buttonStyle(WhitePillStyle()).padding(.bottom, 12)
        ForEach(safe.items.filter { $0.kind == safe.kind }) { item in
            SafeRow(item: item, expanded: safe.open == item.id,
                    toggle: { withAnimation(.smooth) { safe.open = safe.open == item.id ? nil : item.id } },
                    change: { safe.update($0, vault) }, delete: { safe.delete(item.id, vault) })
        }
    }
}

private struct SafeRow: View {
    let item: SafeItem
    let expanded: Bool
    let toggle: () -> Void
    let change: (SafeItem) -> Void
    let delete: () -> Void
    @State private var draft: SafeItem?
    @State private var shown = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                VStack(alignment: .leading, spacing: 1) {
                    Text(item.title).font(Martian.body.font(13)).foregroundStyle(Theme.ink)
                    Text(item.username.isEmpty ? item.url : item.username)
                        .font(Martian.body.font(11)).foregroundStyle(NotesLayout.gray)
                }
                Spacer(minLength: 8)
                if item.kind == .totp {
                    TimelineView(.periodic(from: .now, by: 1)) { context in
                        HStack(spacing: 8) {
                            Text(TOTP.code(secret: item.secret, date: context.date) ?? "—")
                                .font(Martian.heading.font(17)).monospacedDigit().foregroundStyle(Theme.ink)
                            Text("\(TOTP.remaining(date: context.date))").font(Martian.body.font(11)).foregroundStyle(NotesLayout.gray)
                        }
                    }
                }
                Button("скопировать") {
                    SafeModel.copy(item.kind == .totp ? TOTP.code(secret: item.secret) ?? "" : item.secret)
                }
                .buttonStyle(WhitePillStyle())
            }
            .contentShape(Rectangle())
            .onTapGesture(perform: toggle)
            if expanded, let binding = Binding($draft) {
                field("название", binding.title)
                if item.kind != .secret { field(item.kind == .totp ? "аккаунт" : "логин", binding.username) }
                HStack {
                    if shown { field(label, binding.secret) } else {
                        SecureField(label, text: binding.secret).textFieldStyle(.plain).font(Martian.body.font(13))
                    }
                    Button(shown ? "скрыть" : "показать") { shown.toggle() }.buttonStyle(QuietLinkStyle(size: 12, opacity: 0.6))
                    if item.kind == .login {
                        Button("новый") { draft?.secret = PasswordSafe.generate() }.buttonStyle(QuietLinkStyle(size: 12, opacity: 0.6))
                    }
                }
                if item.kind == .login { field("сайт", binding.url) }
                field("заметка", binding.note)
                HStack(spacing: 8) {
                    Button("сохранить") { if let draft { change(draft) }; toggle() }.buttonStyle(WhitePillStyle())
                    Button("удалить") { delete() }.buttonStyle(QuietLinkStyle(size: 12, opacity: 0.6))
                }
                .padding(.top, 4)
            }
        }
        .padding(14)
        .frame(width: 640, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 14).fill(Theme.fg.opacity(0.04)))
        .onChange(of: expanded, initial: true) { draft = expanded ? item : nil; shown = false }
    }

    private var label: String { item.kind == .totp ? String(localized: "секрет или otpauth://") : String(localized: "пароль") }

    private func field(_ title: String, _ text: Binding<String>) -> some View {
        TextField(LocalizedStringKey(title), text: text).textFieldStyle(.plain).font(Martian.body.font(13))
    }
}
