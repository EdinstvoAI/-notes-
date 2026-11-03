import AppKit
import EventKit
import NotesCore
import NotesSpeech
import SwiftUI

// Настройки — кадры «настройки — …» (секция «настройки»): заголовок страницы, серая подпись,
// стеклянные строки 640 с кнопкой справа, внизу пояснение. Всё на настоящих данных.
struct SettingsScreen: View {
    @Environment(\.pageWidth) private var pageWidth
    let model: NotesModel

    var body: some View {
        let page = model.settingsPage
        ScrollView {
            VStack(alignment: .leading, spacing: 5) {
                Text(LocalizedStringKey(page.rawValue))
                    .font(Martian.heading.font(26))
                    .kerning(Theme.tracking(-7, size: 26))
                    .foregroundStyle(Theme.ink)
                Text(subtitle(page))
                    .font(Martian.body.font(13))
                    .kerning(Theme.tracking(-7, size: 13))
                    .foregroundStyle(NotesLayout.gray)
                    .padding(.top, 1)
                    .padding(.bottom, 40)
                switch page {
                case .general: GeneralPage()
                case .sync: SyncPage(model: model)
                case .dictation: DictationPage()
                case .search: SearchPage()
                case .lock: LockPage(model: model)
                case .calendar:
                    CalendarPage(sync: model.tasks.calendar, reload: model.tasks.reload)
                    FolderPicks(folders: model.tree.folders.map(\.name), sync: model.tasks.calendar, reload: model.tasks.reload)
                case .templates: TemplatesPage(model: model)
                }
            }
            .frame(width: min(832, pageWidth), alignment: .leading)
            .padding(.leading, 58)
            .padding(.top, 36)
            .padding(.bottom, 40)
        }
        .scrollIndicators(.never)
    }

    private func subtitle(_ page: NotesModel.SettingsPage) -> String {
        switch page {
        case .general: String(localized: "вид, запуск и текст заметок")
        case .sync: String(localized: "где лежат заметки и куда уходят")
        case .dictation: String(localized: "текст сразу в буфере")
        case .search: "заметки, задачи, события, видео"
        case .lock: String(localized: "пинкод и touch id")
        case .calendar: String(localized: "задачи со сроком попадают в календарь")
        case .templates: String(localized: "шаблоны заметок и автоматизация")
        }
    }
}

// Строка настроек: заголовок, подпись, справа своё (кнопка, «включено», текст).
struct SettingRow<Trailing: View>: View {
    let title: String
    var subtitle: String? = nil
    @ViewBuilder var trailing: () -> Trailing

    var body: some View {
        GlassRow(width: 640) {
            VStack(alignment: .leading, spacing: 2) {
                Text(LocalizedStringKey(title))
                    .font(Martian.body.font(13))
                    .kerning(Theme.tracking(-7, size: 13))
                    .foregroundStyle(Theme.ink)
                if let subtitle {
                    Text(LocalizedStringKey(subtitle))
                        .font(Martian.body.font(11))
                        .kerning(Theme.tracking(-7, size: 11))
                        .foregroundStyle(Theme.fg.opacity(0.3))
                        .lineLimit(1)
                }
            }
            Spacer(minLength: 0)
            trailing()
        }
    }
}

private struct Note: View {
    let text: String
    var body: some View {
        Text(LocalizedStringKey(text))
            .font(Martian.body.font(11))
            .kerning(Theme.tracking(-7, size: 11))
            .foregroundStyle(Theme.fg.opacity(0.3))
            .padding(.top, 14)
    }
}

private struct SyncPage: View {
    let model: NotesModel
    @State private var sync = SyncController.shared
    @State private var sending = false
    @State private var isRepo = false
    @State private var relogin = false
    @State private var commit = AppSettings.commitMinutes
    @State private var push = AppSettings.pushMinutes

    var body: some View {
        let vault = model.library.root
        SettingRow(title: "папка хранилища", subtitle: vault.path.replacingOccurrences(of: NSHomeDirectory(), with: "~")) {
            Button("изменить") {
                VaultPicker.choose { url in
                    try? VaultStore.save(url)
                    NotificationCenter.default.post(name: .vaultChosen, object: url)
                }
            }
            .buttonStyle(WhitePillStyle())
        }
        SettingRow(title: "история в git", subtitle: "версии сохраняются по расписанию") {
            if isRepo {
                GrantedLabel(text: "включено")
            } else {
                Button("включить") {
                    Task {
                        _ = try? await Task.detached { try GitVault.prepare(at: vault) }.value
                        sync.start(vault: vault)
                        isRepo = GitVault.isRepository(vault)
                    }
                }
                .buttonStyle(WhitePillStyle())
            }
        }
        SettingRow(title: "github", subtitle: TokenStore().exists ? String(localized: "приватный репозиторий notes") : String(localized: "вход по коду при первом входе")) {
            if TokenStore().exists && !sync.needsLogin {
                GrantedLabel(text: "подключено")
            } else {
                Button("войти заново") { relogin = true }.buttonStyle(WhitePillStyle())
            }
        }
        ChoiceRow(title: "сохранять версию", value: AppSettings.every(commit), choices: AppSettings.commitChoices.map { ($0, AppSettings.every($0)) }) {
            AppSettings.commitMinutes = $0; commit = $0
        }
        ChoiceRow(title: "отправлять на github", value: AppSettings.every(push), choices: AppSettings.pushChoices.map { ($0, AppSettings.every($0)) }) {
            AppSettings.pushMinutes = $0; push = $0
        }
        SettingRow(title: String(localized: "последний коммит"), subtitle: sync.lastCommit.map { NoteEditor.date($0) } ?? String(localized: "с запуска правок не было")) {
            EmptyView()
        }
        SettingRow(title: "последняя отправка",
                   subtitle: SyncController.lastPush.map { NoteEditor.date($0) } ?? String(localized: "ещё не отправляли")) {
            Button(sending ? "отправляем…" : "отправить сейчас") {
                sending = true
                Task { _ = await sync.pushNow(); sending = false }
            }
            .buttonStyle(WhitePillStyle())
            .disabled(sending || !TokenStore().exists || !sync.isRunning)
        }
        // Ошибка — целиком и выделяемой, чтобы её можно было скопировать и прислать.
        if let error = sync.errorText {
            Text(error)
                .font(Martian.body.font(11)).foregroundStyle(Theme.coral)
                .textSelection(.enabled).fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: 640, alignment: .leading).padding(.top, 8)
        }
        Note(text: "правки с двух mac сохранятся обе")
            .onAppear { isRepo = GitVault.isRepository(vault) }
            .sheet(isPresented: $relogin) {
                ReloginSheet(vault: vault) {
                    relogin = false
                    sync.clearError()
                    Task { _ = await sync.pushNow() }
                }
            }
    }
}

private struct DictationPage: View {
    @State private var selected = SpeechSettings.model
    @State private var downloads = ModelDownloads.shared
    @State private var language = AppSettings.language
    @State private var minutes = AppSettings.maxMinutes
    @State private var speakers = AppSettings.speakers
    @State private var paste = AutoPaste.enabled
    @State private var speakersReady = Diarization.isInstalled()

    var body: some View {
        HotKeyRow(width: 640)
        ChoiceRow(title: "язык речи", subtitle: "для whisper",
                  value: AppSettings.languages.first { $0.code == language }?.title ?? language,
                  choices: AppSettings.languages.map { ($0.code, $0.title) }) {
            AppSettings.language = $0; language = $0
        }
        SwitchRow(title: "вставлять в поле", subtitle: AutoPaste.enabled && !AutoPaste.trusted
                    ? String(localized: "включи <notes> в «универсальном доступе»") : String(localized: "в поле, где курсор"), isOn: $paste) {
            AutoPaste.enabled = $0; paste = $0
        }
        ChoiceRow(title: "длина записи", subtitle: "потом распознается сама",
                  value: Self.length(minutes), choices: AppSettings.lengthChoices.map { ($0, Self.length($0)) }) {
            AppSettings.maxMinutes = $0; minutes = $0
        }
        ForEach(SpeechModel.allCases, id: \.self) { model in
            SettingRow(title: model.displayName, subtitle: subtitle(model)) {
                if model == selected {
                    GrantedLabel(text: "выбрана")
                } else if let progress = downloads.progress[model] {
                    Text("\(Int(progress * 100))%").font(Martian.body.font(13)).foregroundStyle(Theme.fg.opacity(0.3))
                } else if ModelStore.standard.isInstalled(model) {
                    Button("выбрать") { SpeechSettings.model = model; selected = model }.buttonStyle(WhitePillStyle())
                } else {
                    Button("скачать") { Task { try? await downloads.download(model) } }.buttonStyle(WhitePillStyle())
                }
            }
        }
        SettingRow(title: String(localized: "спикеры в расшифровках"), subtitle: String(localized: "кто говорит в видео и аудио, \(Diarization.downloadMB) мб") + (speakersReady ? String(localized: " · скачано") : "")) {
            if let progress = downloads.speakers {
                Text("\(Int(progress * 100))%").font(Martian.body.font(13)).foregroundStyle(Theme.fg.opacity(0.3))
            } else if speakersReady {
                Toggle("", isOn: Binding(get: { speakers }, set: { AppSettings.speakers = $0; speakers = $0 }))
                    .toggleStyle(.switch).tint(Theme.coral).labelsHidden()
            } else {
                Button("скачать") {
                    Task { try? await downloads.downloadSpeakers(); speakersReady = Diarization.isInstalled() }
                }.buttonStyle(WhitePillStyle())
            }
        }
        Note(text: "всё распознаётся на этом mac, без интернета")
    }

    private static func length(_ minutes: Int) -> String { minutes == 0 ? String(localized: "без предела") : String(localized: "до \(minutes) мин") }

    private func subtitle(_ model: SpeechModel) -> String {
        let size = model.downloadMB >= 1000 ? String(format: String(localized: "%.1f гб"), Double(model.downloadMB) / 1000) : String(localized: "\(model.downloadMB) мб")
        let kind = model == .gigaAM ? String(localized: "только русский, сразу с пунктуацией") : String(localized: "99 языков")
        return "\(kind), \(size)" + (ModelStore.standard.isInstalled(model) ? String(localized: " · скачано") : "")
    }
}

// Общие: тема, язык, запуск при входе, текст заметок.
private struct GeneralPage: View {
    @State private var theme = AppSettings.theme
    @State private var language = AppSettings.interfaceLanguage
    private let startLanguage = AppSettings.interfaceLanguage
    @State private var login = AppSettings.launchAtLogin
    @State private var size = AppSettings.textSize
    @State private var spell = AppSettings.spellCheck
    @State private var intelligence = Intelligence.enabled

    var body: some View {
        ChoiceRow(title: "тема", value: theme.rawValue, choices: AppSettings.Theme.allCases.map { ($0, $0.rawValue) }) {
            AppSettings.theme = $0; theme = $0
        }
        ChoiceRow(title: "язык", subtitle: language == startLanguage ? nil : String(localized: "после перезапуска"),
                  value: language.rawValue, choices: AppSettings.InterfaceLanguage.allCases.map { ($0, $0.rawValue) }) {
            AppSettings.interfaceLanguage = $0; language = $0
        }
        SwitchRow(title: "открывать при входе в mac", subtitle: "чтобы хоткей надиктовки работал сразу", isOn: $login) {
            AppSettings.launchAtLogin = $0; login = AppSettings.launchAtLogin
        }
        ChoiceRow(title: "размер текста заметок", value: "\(Int(size))", choices: AppSettings.textSizes.map { ($0, "\(Int($0))") }) {
            AppSettings.textSize = $0; size = $0
        }
        SwitchRow(title: "проверка орфографии", subtitle: "подчёркивать слова с ошибками", isOn: $spell) {
            AppSettings.spellCheck = $0; spell = $0
        }
        if let reason = Intelligence.unavailableReason {
            SettingRow(title: "apple intelligence", subtitle: reason) { EmptyView() }
        } else {
            SwitchRow(title: "apple intelligence", subtitle: "кратко для заметок, на этом mac", isOn: $intelligence) {
                Intelligence.enabled = $0; intelligence = $0
            }
        }
        Note(text: "настройки хранятся на этом mac, в заметки не попадают")
    }
}

// Строка с выбором из списка: справа «изменить», под заголовком — что выбрано.
private struct ChoiceRow<Value: Hashable>: View {
    let title: String
    var subtitle: String? = nil
    let value: String
    let choices: [(Value, String)]
    let pick: (Value) -> Void

    var body: some View {
        SettingRow(title: title, subtitle: subtitle.map { "\(value.localized) · \($0)" } ?? value.localized) {
            Menu("изменить") {
                ForEach(choices, id: \.0) { choice in Button(choice.1.localized) { pick(choice.0) } }
            }
            .menuStyle(.button).buttonStyle(WhitePillStyle()).fixedSize()
        }
    }
}

struct SwitchRow: View {
    let title: String
    let subtitle: String
    @Binding var isOn: Bool
    let change: (Bool) -> Void

    var body: some View {
        SettingRow(title: title, subtitle: subtitle) {
            Toggle("", isOn: Binding(get: { isOn }, set: { change($0) }))
                .toggleStyle(.switch).tint(Theme.coral).labelsHidden()
        }
    }
}

// Поиск: один на всё — заметки, задачи, события, видео и расшифровки (ТЗ 30.10.2026).
private struct SearchPage: View {
    var body: some View {
        SettingRow(title: "поиск", subtitle: "⌘K из любого места") {
            GrantedLabel(text: "работает")
        }
        Note(text: "находит заметки, задачи, события календаря, видео и слова внутри расшифровок")
    }
}

private struct LockPage: View {
    let model: NotesModel
    @State private var pinSet = PinStore().isSet
    @State private var touchID = BiometricLock.isEnabled

    var body: some View {
        SettingRow(title: String(localized: "пинкод"), subtitle: pinSet ? String(localized: "4 цифры") : String(localized: "не задан")) {
            Button(pinSet ? "сменить" : "задать") { model.pinEntryShown = true }.buttonStyle(WhitePillStyle())
        }
        SettingRow(title: "touch id", subtitle: "вход по пальцу") {
            if !BiometricLock.isAvailable {
                Text("нет touch id").font(Martian.body.font(13)).foregroundStyle(Theme.fg.opacity(0.3))
            } else if touchID {
                Button("выключить") { BiometricLock.disable(); touchID = false }.buttonStyle(WhitePillStyle())
            } else {
                Button("включить") { Task { touchID = await BiometricLock.enable() } }.buttonStyle(WhitePillStyle())
            }
        }
        SettingRow(title: "когда спрашивать", subtitle: "после блокировки mac или 15 минут без дела") { EmptyView() }
        Note(text: "пинкод закрывает приложение, файлы заметок не шифруются")
            .onChange(of: model.pinEntryShown) { _, shown in if !shown { pinSet = PinStore().isSet } }
    }
}

// Шаблоны — .md в папке «шаблоны» хранилища; автоматизация — заметка на каждый день (ТЗ 30.10.2026).
private struct TemplatesPage: View {
    let model: NotesModel
    @AppStorage(NotesModel.dailyKey) private var daily = false

    var body: some View {
        let count = Templates.list(in: model.library).count
        SettingRow(title: String(localized: "папка шаблонов"), subtitle: count == 0 ? String(localized: "пока пусто, положи сюда .md") : String(localized: "\(count) \(NoteEditor.plural(count, "шаблон", "шаблона", "шаблонов"))")) {
            Button("открыть") {
                let dir = model.library.root.appendingPathComponent(Templates.folder, isDirectory: true)
                try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
                model.reloadTree()
                model.open(.folder(dir))
            }
            .buttonStyle(WhitePillStyle())
        }
        Note(text: "в шаблоне работают {{дата}}, {{время}} и {{название}}")
        GlassRow(width: 640) {
            VStack(alignment: .leading, spacing: 2) {
                Text("заметка на каждый день").font(Martian.body.font(13)).foregroundStyle(Theme.ink)
                Text("в папке «\(Templates.dailyFolder)» по шаблону «\(Templates.dailyTemplate)»")
                    .font(Martian.body.font(11)).foregroundStyle(Theme.fg.opacity(0.3))
            }
            Spacer(minLength: 0)
            Toggle("", isOn: $daily).toggleStyle(.switch).tint(Theme.coral).labelsHidden()
                .onChange(of: daily) { _, on in if on { model.openDaily() } }
        }
        .padding(.top, 20)
    }
}

private struct CalendarPage: View {
    let sync: CalendarSync
    let reload: () -> Void
    @State private var access = false
    @State private var calendarID = ""
    @State private var minutes = 30

    var body: some View {
        SettingRow(title: "доступ к календарю") {
            if access {
                GrantedLabel(text: "разрешено")
            } else {
                Button("разрешить") { Task { access = await sync.requestAccess(); reload() } }.buttonStyle(WhitePillStyle())
            }
        }
        SettingRow(title: "календарь для задач", subtitle: sync.tasksCalendar?.title ?? "—") {
            Menu("изменить") {
                ForEach(sync.calendars, id: \.calendarIdentifier) { calendar in
                    Button(calendar.title) { sync.tasksCalendar = calendar; calendarID = calendar.calendarIdentifier; reload() }
                }
            }
            .menuStyle(.button).buttonStyle(WhitePillStyle()).fixedSize()
            .disabled(!access)
        }
        SettingRow(title: String(localized: "длительность"), subtitle: String(localized: "\(minutes) минут на задачу")) {
            Menu("изменить") {
                ForEach([15, 30, 60, 90], id: \.self) { value in
                    Button(String(localized: "\(value) минут")) { sync.minutes = value; minutes = value }
                }
            }
            .menuStyle(.button).buttonStyle(WhitePillStyle()).fixedSize()
        }
        Note(text: "событие в календаре обновится само")
            .onAppear { access = sync.hasAccess; minutes = sync.minutes }
    }
}

extension Notification.Name {
    static let vaultChosen = Notification.Name("notes.vaultChosen")
}

// Из каких папок тянуть задачи — и в список задач, и в календарь. По строке на папку верхнего уровня.
private struct FolderPicks: View {
    let folders: [String]
    let sync: CalendarSync
    let reload: () -> Void
    @State private var excluded: Set<String> = []

    var body: some View {
        Text("задачи из папок")
            .font(Martian.body.font(15)).foregroundStyle(Theme.ink).padding(.top, 26)
        ForEach(folders, id: \.self) { name in
            SettingRow(title: name) {
                Toggle("", isOn: Binding(get: { !excluded.contains(name) }, set: { on in
                    if on { excluded.remove(name) } else { excluded.insert(name) }
                    sync.excludedFolders = excluded
                    reload()
                }))
                .toggleStyle(.switch).tint(Theme.coral).labelsHidden()
            }
        }
        .onAppear { excluded = sync.excludedFolders }
    }
}

// Повторный вход в GitHub по коду — тот же поток, что на первом входе: код, «открыть github»,
// подтвердил — новый токен в Связке ключей, папка привязана к notes и отправлена.
private struct ReloginSheet: View {
    let vault: URL
    let done: () -> Void
    @State private var login = GitHubLogin()
    @State private var attempt = 0
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 18) {
            Text("войти в github")
                .font(Martian.heading.font(22)).foregroundStyle(Theme.ink)
            Text("вход устарел, введи код на github заново")
                .font(Martian.body.font(13)).foregroundStyle(NotesLayout.gray)
            DeviceCodeCapsule(state: login.state, renew: { attempt += 1 })
                .offset(y: -(207 - Layout.sheetTop))
                .frame(height: 110)
            HStack(spacing: 10) {
                Button("открыть github") {
                    if case .waiting(let code, let url) = login.state { DeviceCode.copy(code); NSWorkspace.shared.open(url) }
                    else { DeviceCode.openGitHub() }
                }
                .buttonStyle(CoralCapsuleStyle())
                Button("отмена") { dismiss() }.buttonStyle(QuietLinkStyle(size: 13, opacity: 0.4))
            }
        }
        .padding(28)
        .frame(width: 600)
        .task(id: attempt) { await login.run(vault: vault) { _ in done() } }
    }
}
