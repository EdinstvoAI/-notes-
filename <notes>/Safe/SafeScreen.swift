import AppKit
import LocalAuthentication
import NotesCore
import SwiftUI
import UniformTypeIdentifiers

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
    // Автозакрытие: 5 минут без дела, сон Mac или блокировка экрана — сейф закрывается, пароль из памяти уходит.
    // -safeIdle 5 — для проверки автозакрытия без ожидания.
    static let idle: TimeInterval = UserDefaults.standard.double(forKey: "safeIdle") > 0 ? UserDefaults.standard.double(forKey: "safeIdle") : 300
    private var lastUse = Date()

    init() {
        let center = NSWorkspace.shared.notificationCenter
        center.addObserver(forName: NSWorkspace.willSleepNotification, object: nil, queue: .main) { _ in
            MainActor.assumeIsolated { SafeModel.shared.lock() }
        }
        DistributedNotificationCenter.default().addObserver(forName: .init("com.apple.screenIsLocked"), object: nil, queue: .main) { _ in
            MainActor.assumeIsolated { SafeModel.shared.lock() }
        }
        Task { [weak self] in
            while true {
                try? await Task.sleep(for: .seconds(min(30, Self.idle)))
                guard let self else { return }
                if self.unlocked, Date().timeIntervalSince(self.lastUse) > Self.idle { self.lock() }
            }
        }
    }

    func touch() { lastUse = Date() }

    func unlock(_ vault: URL, _ text: String) {
        do {
            items = try PasswordSafe.load(vault: vault, password: text) ?? []
            if !PasswordSafe.exists(vault: vault) { try PasswordSafe.save([], vault: vault, password: text) }
            password = text
            error = nil
            touch()
            if !remembered(vault).exists, !UserDefaults.standard.bool(forKey: "testVault") { try? remembered(vault).save(text) }
        } catch {
            self.error = String(localized: "пароль не подошёл")
        }
    }

    // Touch ID или пароль Mac (03.11.2026, «зачем мне мастер-пароль»): мастер-пароль вводится один раз на
    // каждом Mac и ложится в Связку ключей; дальше сейф открывает система. На другом Mac файл тот же —
    // там мастер-пароль спросят один раз, поэтому он остаётся.
    private func remembered(_ vault: URL) -> TokenStore { TokenStore(service: "notes.safe", account: vault.path) }

    func canUseTouchID(_ vault: URL) -> Bool { PasswordSafe.exists(vault: vault) && remembered(vault).exists }

    func unlockWithTouchID(_ vault: URL) {
        let context = LAContext()
        context.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: String(localized: "открыть сейф паролей")) { ok, _ in
            Task { @MainActor in
                guard ok, let text = self.remembered(vault).load() else { return }
                self.unlock(vault, text)
            }
        }
    }

    // Сейф без мастер-пароля: ключ сначала в Связку ключей, и только если он там — создаём файл.
    // Иначе ключ потерялся бы вместе с сейфом.
    func create(_ vault: URL) {
        let key = PasswordSafe.generate(length: 32)
        let testing = UserDefaults.standard.bool(forKey: "testVault")
        guard testing || (try? remembered(vault).save(key)) != nil else {
            error = String(localized: "связка ключей не приняла ключ — придумай свой мастер-пароль"); return
        }
        unlock(vault, key)
    }

    // Ключ (мастер-пароль) — в буфер, чтобы ввести на другом Mac; через минуту стирается.
    func copyKey() { if let password { Self.copy(password) } }

    func lock() { password = nil; items = []; open = nil }

    func save(_ vault: URL) {
        guard let password else { return }
        touch()
        do { try PasswordSafe.save(items, vault: vault, password: password) } catch { self.error = error.localizedDescription }
    }

    func add(_ vault: URL) {
        var item = SafeItem(kind: kind, title: String(localized: "без названия"))
        if kind == .login { item.secret = PasswordSafe.generate() }
        items.insert(item, at: 0)
        open = item.id
        save(vault)
    }

    func insert(_ item: SafeItem, _ vault: URL) {
        items.insert(item, at: 0)
        save(vault)
    }

    func update(_ item: SafeItem, _ vault: URL) {
        guard let index = items.firstIndex(where: { $0.id == item.id }) else { return }
        var item = item
        item.modified = Date()
        items[index] = item
        save(vault)
    }

    // Импорт CSV из «Паролей»: файл выбирает человек; после — напоминаем стереть его, он открытым текстом.
    func importCSV(_ vault: URL) {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.commaSeparatedText]
        guard panel.runModal() == .OK, let url = panel.url,
              let text = try? String(contentsOf: url, encoding: .utf8) else { return }
        let new = PasswordSafe.importPasswordsCSV(text)
        items.insert(contentsOf: new, at: 0)
        save(vault)
        error = String(localized: "перенесено: \(new.count). удали CSV — в нём пароли открытым текстом")
    }

    func delete(_ id: UUID, _ vault: URL) { items.removeAll { $0.id == id }; save(vault) }

    static func copy(_ text: String) {
        shared.touch()
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
        // Закрытый сейф не показывает даже разделы — они появляются после входа.
        SidebarList(title: "пароли") {
            if safe.unlocked {
                ForEach(SafeItem.Kind.allCases, id: \.self) { kind in
                    SidebarRow(glyph: kind == .totp ? .check : kind == .secret ? .spark : .key, title: kind.title,
                               count: safe.items.filter { $0.kind == kind }.count,
                               selected: safe.kind == kind) { safe.kind = kind; safe.open = nil }
                }
                SidebarRow(glyph: .key, title: "ключ для другого mac", selected: false) { safe.copyKey() }
                    .padding(.top, 12)
                SidebarRow(glyph: .settings, title: "закрыть сейф", selected: false) { safe.lock() }
            }
        }
    }
}

struct SafeScreen: View {
    @Environment(\.pageWidth) private var pageWidth
    let model: NotesModel
    @State private var entry = ""
    @State private var repeatEntry = ""
    @State private var ownPassword = false
    @State private var importing = false
    @State private var adding = false

    var body: some View {
        let safe = model.safe
        let vault = model.library.root
        ScrollView {
            VStack(alignment: .leading, spacing: 5) {
                Text(LocalizedStringKey(safe.unlocked ? safe.kind.title : "пароли"))
                    .font(Martian.heading.font(26)).kerning(Theme.tracking(-7, size: 26)).foregroundStyle(Theme.ink)
                Text(safe.unlocked ? "в зашифрованном файле .safe/safe.enc, на github — только шифром"
                                   : safe.canUseTouchID(vault) ? "сейф открывается touch id или паролем mac"
                                   : PasswordSafe.exists(vault: vault) ? "на новом mac — ключ сейфа один раз, дальше touch id или пароль mac"
                                   : "ключ придумает mac и спрячет в связку ключей — открывать будешь touch id")
                    .font(Martian.body.font(13)).kerning(Theme.tracking(-7, size: 13)).foregroundStyle(NotesLayout.gray)
                    .padding(.top, 1).padding(.bottom, 40)
                Group {
                    if safe.unlocked { items(safe, vault) } else { lockScreen(safe, vault) }
                }
                .transition(.opacity.combined(with: .move(edge: .bottom)))
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
                if UserDefaults.standard.bool(forKey: "safeAdd") { adding = true }
                if UserDefaults.standard.bool(forKey: "safeImport") { importing = true }
                if let kind = UserDefaults.standard.string(forKey: "safeKind").flatMap(SafeItem.Kind.init(rawValue:)) { safe.kind = kind }
            }
            #endif
        }
    }

    @ViewBuilder private func lockScreen(_ safe: SafeModel, _ vault: URL) -> some View {
        let creating = !PasswordSafe.exists(vault: vault)
        if safe.canUseTouchID(vault) {
            Button("открыть по touch id") { safe.unlockWithTouchID(vault) }
                .buttonStyle(CoralCapsuleStyle())
                .onAppear { safe.unlockWithTouchID(vault) }
        } else {
            masterPassword(safe, vault, creating)
        }
    }

    // Новый сейф — без мастер-пароля (03.11.2026, «не хочу писать это»): ключ придумывает Mac и кладёт
    // в Связку ключей. Свой пароль — по желанию; для другого Mac ключ копируется из открытого сейфа.
    @ViewBuilder private func masterPassword(_ safe: SafeModel, _ vault: URL, _ creating: Bool) -> some View {
        if creating && !ownPassword {
            Button("создать сейф") { withAnimation(.smooth) { safe.create(vault) } }
                .buttonStyle(CoralCapsuleStyle())
            Button("свой мастер-пароль") { ownPassword = true }
                .buttonStyle(QuietLinkStyle(size: 12, opacity: 0.6)).padding(.top, 10)
            if let error = safe.error { Text(error).font(Martian.body.font(12)).foregroundStyle(Theme.coral).padding(.top, 6) }
        } else {
            passwordFields(safe, vault, creating)
        }
    }

    @ViewBuilder private func passwordFields(_ safe: SafeModel, _ vault: URL, _ creating: Bool) -> some View {
        SecureField(creating ? "придумай мастер-пароль" : "ключ сейфа или мастер-пароль", text: $entry)
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
            withAnimation(.smooth) { safe.unlock(vault, entry) }; entry = ""; repeatEntry = ""
        }
        .buttonStyle(CoralCapsuleStyle())
        .disabled(entry.count < 8 || (creating && entry != repeatEntry))
        .padding(.top, 14)
    }

    @ViewBuilder private func items(_ safe: SafeModel, _ vault: URL) -> some View {
        HStack(spacing: 8) {
            Button("добавить") { withAnimation(.smooth) { adding.toggle(); importing = false } }.buttonStyle(CoralCapsuleStyle())
            Button(importing ? "скрыть перенос" : "перенести из «Паролей»") { withAnimation(.smooth) { importing.toggle(); adding = false } }
                .buttonStyle(OutlinePillStyle())
        }
        .padding(.bottom, 12)
        if adding {
            AddForm(kind: safe.kind) { item in
                withAnimation(.smooth) { if let item { safe.insert(item, vault) }; adding = false }
            }
            .id(safe.kind)
            .transition(.opacity.combined(with: .move(edge: .top)))
        }
        // Пустой сейф сразу показывает, как перенести пароли: так понятнее с первого раза.
        if importing || (safe.items.isEmpty && !adding) { importGuide(safe, vault).transition(.opacity.combined(with: .move(edge: .top))) }
        if let note = safe.error { Text(note).font(Martian.body.font(12)).foregroundStyle(Theme.coral).padding(.bottom, 8) }
        if !adding, !safe.items.isEmpty, !safe.items.contains(where: { $0.kind == safe.kind }) {
            Text("здесь пока пусто — нажми «добавить»").font(Martian.body.font(12)).foregroundStyle(NotesLayout.gray)
        }
        ForEach(safe.items.filter { $0.kind == safe.kind }) { item in
            SafeRow(item: item, expanded: safe.open == item.id,
                    toggle: { withAnimation(.smooth) { safe.open = safe.open == item.id ? nil : item.id } },
                    change: { safe.update($0, vault) }, delete: { withAnimation(.smooth) { safe.delete(item.id, vault) } })
                .transition(.opacity.combined(with: .scale(scale: 0.97, anchor: .top)))
        }
    }

    // Перенос из «Паролей» по шагам (03.11.2026, «не пойму, как добавить из паролей»): у Apple нет API,
    // поэтому — выгрузка CSV и удаление файла после.
    private func importGuide(_ safe: SafeModel, _ vault: URL) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("перенести из «Паролей»").font(Martian.body.font(13)).foregroundStyle(Theme.ink)
            ForEach(Array(Self.steps.enumerated()), id: \.offset) { i, step in
                HStack(alignment: .firstTextBaseline, spacing: 10) {
                    Text("\(i + 1)").font(Martian.heading.font(13)).foregroundStyle(Theme.coral).frame(width: 14)
                    Text(LocalizedStringKey(step)).font(Martian.body.font(12)).foregroundStyle(Theme.ink.opacity(0.8))
                }
            }
            HStack(spacing: 8) {
                Button("открыть «Пароли»") {
                    if let app = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.apple.Passwords") {
                        NSWorkspace.shared.openApplication(at: app, configuration: .init())
                    }
                }
                .buttonStyle(OutlinePillStyle())
                Button("выбрать файл") { withAnimation(.smooth) { safe.importCSV(vault); importing = false } }
                    .buttonStyle(CoralCapsuleStyle())
            }
            .padding(.top, 4)
        }
        .padding(16)
        .frame(width: 640, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 14).fill(Theme.fg.opacity(0.04)))
        .padding(.bottom, 12)
    }

    static let steps = [
        "открой «Пароли» — кнопка ниже",
        "в меню: файл → экспортировать все пароли…, сохрани файл",
        "здесь нажми «выбрать файл» и укажи его",
        "удали этот файл и очисти корзину: в нём пароли открытым текстом",
    ]
}

// Новая запись — формой, а не пустой «без названия» (03.11.2026, «странное название, ввести не могу»).
// Код 2FA: можно вставить ссылку otpauth:// — название и аккаунт возьмём из неё.
private struct AddForm: View {
    let kind: SafeItem.Kind
    let done: (SafeItem?) -> Void
    @State private var item: SafeItem
    @FocusState private var focused: Int?

    init(kind: SafeItem.Kind, done: @escaping (SafeItem?) -> Void) {
        self.kind = kind
        self.done = done
        var item = SafeItem(kind: kind, title: "")
        if kind == .login { item.secret = PasswordSafe.generate() }
        _item = State(initialValue: item)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(LocalizedStringKey(kind == .login ? "новый пароль" : kind == .totp ? "новый код 2fa" : "новый секрет"))
                .font(Martian.body.font(13)).foregroundStyle(Theme.ink)
            if kind == .totp {
                input("секретный ключ или ссылка otpauth:// — из настроек 2fa на сайте", $item.secret)
                    .focused($focused, equals: 0)
                    .onChange(of: item.secret) { fillFromLink() }
            }
            input(kind == .totp ? "сервис, например github" : "название", $item.title)
                .focused($focused, equals: 1)
            if kind != .secret { input(kind == .totp ? "аккаунт" : "логин", $item.username) }
            if kind == .login {
                HStack {
                    input("пароль", $item.secret)
                    Button("новый") { item.secret = PasswordSafe.generate() }.buttonStyle(QuietLinkStyle(size: 12, opacity: 0.6))
                }
                input("сайт", $item.url)
            }
            if kind == .secret { input("секрет", $item.secret) }
            if kind == .totp, let code = TOTP.code(secret: item.secret) {
                Text("код сейчас: \(code)").font(Martian.body.font(12)).foregroundStyle(NotesLayout.gray)
            }
            HStack(spacing: 8) {
                Button("сохранить") { done(item) }
                    .buttonStyle(CoralCapsuleStyle())
                    .disabled(item.title.trimmingCharacters(in: .whitespaces).isEmpty || item.secret.isEmpty
                              || (kind == .totp && TOTP.code(secret: item.secret) == nil))
                Button("отмена") { done(nil) }.buttonStyle(QuietLinkStyle(size: 12, opacity: 0.6))
            }
            .padding(.top, 4)
        }
        .padding(16)
        .frame(width: 640, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 14).fill(Theme.fg.opacity(0.04)))
        .padding(.bottom, 12)
        .onAppear { focused = kind == .totp ? 0 : 1 }
    }

    private func input(_ title: String, _ text: Binding<String>) -> some View {
        TextField(LocalizedStringKey(title), text: text)
            .textFieldStyle(.plain).font(Martian.body.font(13))
            .padding(10)
            .background(RoundedRectangle(cornerRadius: 10).fill(Theme.fg.opacity(0.05)))
    }

    // otpauth://totp/GitHub:makar?secret=…&issuer=GitHub
    private func fillFromLink() {
        guard let url = URLComponents(string: item.secret), url.scheme == "otpauth" else { return }
        let label = url.path.trimmingCharacters(in: CharacterSet(charactersIn: "/")).removingPercentEncoding ?? ""
        let parts = label.split(separator: ":", maxSplits: 1).map(String.init)
        let issuer = url.queryItems?.first { $0.name == "issuer" }?.value ?? (parts.count == 2 ? parts[0] : "")
        if item.title.isEmpty { item.title = issuer }
        if item.username.isEmpty { item.username = parts.last ?? "" }
    }
}

// Заметная кнопка на фоне страницы: тонкая обводка, без заливки.
struct OutlinePillStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Martian.body.font(13))
            .kerning(Theme.tracking(-7, size: 13))
            .foregroundStyle(Theme.ink)
            .padding(.vertical, 10)
            .padding(.horizontal, 15)
            .background(Capsule().fill(Theme.fg.opacity(configuration.isPressed ? 0.12 : 0.06)))
            .overlay(Capsule().strokeBorder(Theme.fg.opacity(0.14)))
            .scaleEffect(configuration.isPressed ? 0.96 : 1)
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
                            .padding(10).background(RoundedRectangle(cornerRadius: 10).fill(Theme.fg.opacity(0.05)))
                    }
                    Button(shown ? "скрыть" : "показать") { shown.toggle() }.buttonStyle(QuietLinkStyle(size: 12, opacity: 0.6))
                    if item.kind == .login {
                        Button("новый") { draft?.secret = PasswordSafe.generate() }.buttonStyle(QuietLinkStyle(size: 12, opacity: 0.6))
                    }
                }
                if item.kind == .login { field("сайт", binding.url) }
                field("заметка", binding.note)
                HStack(spacing: 8) {
                    Button("сохранить") { if let draft { change(draft) }; toggle() }.buttonStyle(CoralCapsuleStyle())
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
            .padding(10).background(RoundedRectangle(cornerRadius: 10).fill(Theme.fg.opacity(0.05)))
    }
}
