import EventKit
import NotesCore
import SwiftUI

// Главный экран — секция «Заметки» в Figma (16:7138): кадры «заметка» (7:34), «все заметки» (79:11295)
// и поиск поверх (79:12603). Окно 1200×760, все координаты — из кадров.
enum NotesLayout {
    static let window = CGSize(width: 1200, height: 760)
    // 870, а не 820 (03.11.2026): восемь разделов значками и кнопка синхронизации справа на 820 не влезали.
    static let minWidth: CGFloat = 870
    static let header: CGFloat = 92
    static let gray = Color(red: 134 / 255, green: 134 / 255, blue: 139 / 255)
    static let line = Color(light: Color(red: 227 / 255, green: 227 / 255, blue: 232 / 255), dark: .white.opacity(0.1))
}

struct NotesWindow: View {
    @Bindable var model: NotesModel
    @State private var arrived = false

    var body: some View {
        ZStack(alignment: .topLeading) {
            screen
                // Под поиском экран размыт и чуть затемнён — как в кадре, а не залит серым.
                .blur(radius: model.searchShown || model.pinEntryShown ? 8 : 0)
                .allowsHitTesting(!model.searchShown && !model.pinEntryShown)

            if model.searchShown {
                SearchOverlay(model: model)
                    .transition(.opacity.combined(with: .blurReplace))
            }
            if model.pinEntryShown {
                ZStack {
                    Theme.bg.opacity(0.45)
                    PinEntrySheet { _ in model.pinEntryShown = false }
                }
                .transition(.opacity.combined(with: .blurReplace))
            }
        }
        // Окно тянется; сжимается до 820 (31.10.2026, «окно не сжимается»): разделы берут ширину листа,
        // шапка прячет подписи в значки.
        .frame(minWidth: NotesLayout.minWidth, maxWidth: .infinity,
               minHeight: NotesLayout.window.height, maxHeight: .infinity, alignment: .topLeading)
        .background(Theme.bg)
        .animation(.smooth(duration: 0.3), value: model.searchShown)
        .onDisappear { model.flush() }
    }

    private var screen: some View {
        GeometryReader { geo in
        let w = geo.size.width, h = geo.size.height
        ZStack(alignment: .topLeading) {
            // Шапка из планок, как на первом входе, только 92 px и во всю ширину.
            // Планки — около 60 слоёв с размытием. Раньше они перерисовывались при каждом изменении экрана
            // (буква в заметке, наведение) — отсюда лаги. Теперь один раз на ширину окна, слой через Metal.
            StaticSlats(width: w).equatable()
                .frame(height: NotesLayout.header)

            TopBar(model: model)
                .modifier(Arrival(shown: arrived, delay: 0.05))

            // Кадр 152:32854: один стеклянный лист на панель и раздел; путь — капсулой над панелью,
            // строки панели с 161, раздел — с 283.
            GlassSheet(radius: 18)
                .frame(width: w - 34, height: h - 92)
                .offset(x: 17, y: 92)
                .modifier(Arrival(shown: arrived, delay: 0.15))

            Group {
                SectionCrumb(model: model)
                    .offset(x: 10, y: 15)
                // Смена раздела: панель уходит в размытие и проявляется новой, вместе с разделом справа.
                SectionSidebar(model: model)
                    .id(model.section)
                    .transition(.blurReplace)
                    .frame(width: 247, height: h - 146, alignment: .topLeading)
                    .offset(y: 54)
            }
            .animation(.smooth(duration: 0.45), value: model.section)
            .offset(x: 17, y: 92)
            .modifier(Arrival(shown: arrived, delay: 0.2))

            // Обрезаем до сдвига: после .offset обрезка шла по старому месту, и правый край листа пропадал.
            SectionContent(model: model)
                .environment(\.pageWidth, w - 300 - 116)
                .frame(width: w - 300, height: h - 92, alignment: .topLeading)
                .clipped()
                .offset(x: 283, y: 92)
                .modifier(Arrival(shown: arrived, delay: 0.25))
        }
        }
        .onAppear { arrived = true }
    }
}

// Первый показ окна: шапка, панель и лист проявляются по очереди из размытия, чуть снизу (ТЗ 30.10.2026).
private struct Arrival: ViewModifier {
    let shown: Bool
    let delay: Double

    func body(content: Content) -> some View {
        content
            .opacity(shown ? 1 : 0)
            .blur(radius: shown ? 0 : 12)
            .offset(y: shown ? 0 : 14)
            .animation(.smooth(duration: 0.7).delay(delay), value: shown)
    }
}

// Ширина страницы раздела: лист минус поля 58 с каждой стороны. Тексты не шире 832, как в кадрах;
// календарь тянется на всю ширину.
extension EnvironmentValues {
    @Entry var pageWidth: CGFloat = 832
}

// Стеклянный лист: #F9F9F9 20 % и светлый кант, как лист шагов первого входа.
// Листы доходят до низа окна — скруглены только сверху (ТЗ 30.10.2026); нижний кант уходит за край окна.
struct GlassSheet: View {
    var radius: CGFloat

    var body: some View {
        let shape = UnevenRoundedRectangle(topLeadingRadius: radius, topTrailingRadius: radius)
        shape
            .fill(Theme.sheet.opacity(0.2))
            .overlay(shape.strokeBorder(Theme.edge.opacity(0.9), lineWidth: 1))
            .shadow(color: .black.opacity(0.07), radius: 10, y: -2)
            .padding(.bottom, -1)
    }
}

// Капсула в шапке: #F9F9F9 20 % со стеклом, скругление 18.
private struct HeaderGroup<Content: View>: View {
    let width: CGFloat
    @ViewBuilder var content: () -> Content

    var body: some View {
        ZStack(alignment: .leading) {
            RoundedRectangle(cornerRadius: 18)
                .fill(Theme.sheet.opacity(0.2))
                .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 18))
                .overlay(RoundedRectangle(cornerRadius: 18).strokeBorder(Theme.edge.opacity(0.6), lineWidth: 1))
            content()
        }
        .frame(width: width, height: 48)
    }
}

private struct TopBar: View {
    let model: NotesModel
    @State private var sync = SyncController.shared
    @State private var hovered: NotesModel.Section?
    @State private var fullSections: CGFloat = 518
    @State private var syncText: CGFloat = 0
    @State private var labels: [String: CGFloat] = [:]
    @State private var syncHovered = false
    @Namespace private var hover

    // Разделы в шапке: все со словами → гибрид (значки, слово только у выбранного) → только значки.
    enum Fit { case full, hybrid, icons }
    private static let titles = ["заметки", "задачи", "календарь", "видео", "пароли", "синхронизация", "mcp", "настройки"]

    var body: some View {
        GeometryReader { geo in
        // Ширину разделов со словами меряем (невидимая копия ниже), а не берём 540 из кадра: у Макара
        // подписи вылезали под синхронизацию.
        // Порядок сжатия: сначала текст синхронизации (остаётся значок), потом разделы — значками.
        // Значок синхронизации не пропадает никогда («почему синхронизация пропадает», 31.10.2026).
        // Гибрид — только если влезает подпись любого раздела, даже самая длинная: иначе при переключении
        // раздела шапка то влезала бы, то нет («только когда можем показать текст каждого раздела», 31.10.2026).
        let longest = labels.values.max() ?? 80
        let fit: Fit = geo.size.width >= 449 + fullSections + 22 + 16 + 48 + 17 ? .full
            : geo.size.width >= 449 + Self.hybridWidth(longest) + 16 + 48 + 17 ? .hybrid : .icons
        let compact = fit != .full
        // Группы — в ряд (HStack): синхронизация встаёт сразу за разделами. На отступах .offset она съезжала
        // под разделы — TimelineView тянется на всю ширину и сдвигал её.
        HStack(alignment: .top, spacing: 0) {
            HeaderGroup(width: 417) {
                Button { model.section = .notes } label: {
                    Text("<notes>")
                        .font(Martian.logo.font(15))
                        .kerning(Theme.tracking(-5, size: 15))
                        .foregroundStyle(Theme.fg)
                }
                .buttonStyle(.plain)
                .offset(x: 15)
                Button { model.searchShown = true } label: {
                    HStack(spacing: 6) {
                        DotIcon(glyph: .search)
                        Text("поиск")
                            .font(Martian.body.font(13))
                            .kerning(Theme.tracking(-7, size: 13))
                            .foregroundStyle(NotesLayout.gray)
                        Spacer(minLength: 0)
                        Keycap(text: "⌘K")
                    }
                    .padding(.leading, 10)
                    .padding(.trailing, 10)
                    .frame(width: 256, height: 30)
                    .background(Capsule().fill(Theme.bg.opacity(0.5)))
                    .contentShape(Capsule())
                }
                .buttonStyle(.plain)
                .keyboardShortcut("k", modifiers: .command)
                .background {
                    // Настройки ушли из шапки (кадр 152:32854) — открываются ⌘,.
                    Button("") { model.flush(); model.section = .settings }
                        .keyboardShortcut(",", modifiers: .command)
                        .opacity(0).allowsHitTesting(false)
                }
                .offset(x: 146)
            }
            .padding(.leading, 17)
            .padding(.trailing, 15)

            // Узкое окно — значки, название — подсказкой при наведении; широкое — со словами, как в кадрах.
            // Подсветка наведения одна на группу и переезжает между кнопками, не вылезая за них.
            let sections: CGFloat = switch fit {
            case .full: fullSections + 22
            case .hybrid: Self.hybridWidth(labels[selectedTitle])
            case .icons: CGFloat(Self.titles.count) * 34 + CGFloat(Self.titles.count - 1) * 6 + 22
            }
            // Капсула по ширине самих кнопок: посчитанная ширина расходилась с ними, и шестерёнка
            // налезала на край (снимок Макара, 03.11.2026). Счёт `sections` — только для выбора вида.
            sectionButtons(fit)
                .fixedSize()
                .padding(.horizontal, 11)
                .frame(height: 48)
                .background {
                    RoundedRectangle(cornerRadius: 18)
                        .fill(Theme.sheet.opacity(0.2))
                        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 18))
                        .overlay(RoundedRectangle(cornerRadius: 18).strokeBorder(Theme.edge.opacity(0.6), lineWidth: 1))
                }
                .animation(.smooth(duration: 0.25), value: hovered)
                .padding(.trailing, 16)

            let left = 449 + sections + 16
            // По ширине текста, как в кадре. Не влезает целиком — остаётся одна иконка, без обрезанного
            // и размытого текста («не помещается — просто текст убери», 31.10.2026).
            let natural = 16 + 16 + 6 + syncText + 16
            let room = geo.size.width - left - 17
            let fits = natural <= room && !compact
            let width: CGFloat = fits ? natural : 48
            // Синхронизация прижата к правому краю, между ней и разделами — свободное место (31.10.2026).
            Spacer(minLength: 0)
            Group {
                HeaderGroup(width: width) {
                    HStack(spacing: 6) {
                        DotIcon(glyph: .git)
                        TimelineView(.periodic(from: .now, by: 30)) { context in
                            Text(sync.status(now: context.date))
                                .font(Martian.body.font(11))
                                .kerning(Theme.tracking(-7, size: 11))
                                .foregroundStyle(Theme.fg)
                                .fixedSize()
                                .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { syncText = $0 }
                        }
                        .fixedSize()
                        .opacity(fits ? 1 : 0)
                    }
                    .opacity(0.4)
                    .padding(.leading, 16)
                    // Ширина строки — ровно капсула: длинный текст раньше раздувал её, и капсула
                    // вылезала по центру поверх разделов.
                    .frame(width: width, height: 48, alignment: .leading)
                    .clipped()
                    .animation(.smooth(duration: 0.25), value: fits)
                    .contentShape(Rectangle())
                    .onHover { syncHovered = $0 }
                    .hoverHint(sync.errorText ?? sync.status(now: .now), when: (!fits || sync.errorText != nil) && syncHovered)
                }
                .padding(.trailing, 17)
                .transition(.opacity)
            }
        }
        .padding(.top, 15)
        .background(alignment: .topLeading) {
            sectionButtons(.full)
                .fixedSize()
                .hidden()
                .allowsHitTesting(false)
                .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { fullSections = $0 }
            ForEach(Self.titles, id: \.self) { title in
                Self.label(title).fixedSize().hidden()
                    .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { labels[title] = $0 }
            }
        }
        .animation(.smooth(duration: 0.35), value: fit)
        .animation(.smooth(duration: 0.35), value: model.section)
        }
    }

    // Кнопка со словом: значок 16 + зазор 8 + поля 8 по краям; группа — 3 значка по 34, зазоры 6 и поля 22.
    private static func hybridWidth(_ label: CGFloat?) -> CGFloat {
        // Значки всех разделов, кроме выбранного: с вкладкой «заметки» их стало пять, а счёт остался на четыре —
        // гибрид включался, когда подпись уже не влезала (01.11.2026).
        let others = CGFloat(titles.count - 1)
        return others * 34 + max(34, (label ?? 0) + 40) + others * 6 + 22
    }

    // Без `default`: с ним вкладка «заметки» осталась без подписи в счёте ширины — капсула гибрида
    // была уже кнопок, и шестерёнка вылезала за правый край (снимок Макара, 01.11.2026).
    private var selectedTitle: String {
        switch model.section {
        case .notes: "заметки"
        case .tasks: "задачи"
        case .calendar: "календарь"
        case .video: "видео"
        case .settings: "настройки"
        case .sync: "синхронизация"
        case .mcp: "mcp"
        case .safe: "пароли"
        }
    }

    private static func label(_ title: String) -> some View {
        Text(LocalizedStringKey(title))
            .font(Martian.body.font(13))
            .kerning(Theme.tracking(-7, size: 13))
            .foregroundStyle(Theme.ink)
    }

    private func sectionButtons(_ fit: Fit) -> some View {
        // Заметки — и вкладкой, и по логотипу: «верни вкладку заметки» (31.10.2026). Настройки — тоже в шапке, и по ⌘,.
        HStack(spacing: fit == .full ? 10 : 6) {
            section("заметки", .notes, .notes, fit)
            section("задачи", .taskDone, .tasks, fit)
            section("календарь", .calendar, .calendar, fit)
            section("видео", .video, .video, fit)
            // Синхронизация — своим разделом, а не кликом по статусу (03.11.2026, «статус не кликабельный»).
            section("пароли", .key, .safe, fit)
            section("синхронизация", .git, .sync, fit)
            section("mcp", .spark, .mcp, fit)
            section("настройки", .settings, .settings, fit)
        }
    }

    // Раздел в шапке: выбранный — ярко на белом 10 %, остальные — 30 %, как в кадрах.
    private func section(_ title: String, _ glyph: DotGlyph, _ target: NotesModel.Section, _ fit: Fit) -> some View {
        let active = model.section == target
        let compact = fit == .icons || (fit == .hybrid && !active)
        return Button {
            model.flush()
            model.section = target
        } label: {
            HStack(spacing: 8) {
                DotIcon(glyph: glyph)
                if !compact {
                    Self.label(title)
                        .fixedSize()
                        .transition(.blurReplace)
                }
            }
                .padding(.horizontal, compact ? 0 : 8)
                .frame(minWidth: 34, minHeight: 26)
                .background {
                    if hovered == target && !active {
                        RoundedRectangle(cornerRadius: 8).fill(Theme.bg.opacity(0.06))
                            .matchedGeometryEffect(id: "hover", in: hover)
                    }
                }
                .background(RoundedRectangle(cornerRadius: 8).fill(Theme.bg.opacity(active ? 0.1 : 0)))
                .opacity(active || hovered == target ? 1 : 0.3)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .hoverHint(title, when: compact && hovered == target)
        .onHover { inside in
            if inside { hovered = target } else if hovered == target { hovered = nil }
        }
    }
}

struct NotesSidebar: View {
    let model: NotesModel

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {

            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    // Закреплённые — над деревом.
                    let pins = model.pinned
                    if !pins.isEmpty {
                        ForEach(pins, id: \.self) { url in
                            SidebarRow(glyph: .file, title: url.deletingPathExtension().lastPathComponent,
                                       selected: model.selection == .note(url)) { model.open(.note(url)) }
                                .contextMenu { NoteMenu(model: model, url: url) }
                        }
                        Rectangle().fill(NotesLayout.line).frame(height: 1).padding(.vertical, 6)
                    }
                    FolderRows(model: model, folder: model.tree, depth: 0)
                }
                .contextMenu { NoteMenu(model: model, url: model.library.root, folder: true) }
                // Мимо папок — в корень хранилища.
                .dropDestination(for: URL.self) { urls, _ in model.drop(urls, into: model.library.root) }
                .padding(.horizontal, 10)
                .padding(.top, 15)
                .padding(.bottom, 20)
            }
            .scrollIndicators(.never)
        }
    }
}

// Папки и заметки одного уровня; раскрытая папка показывает свои строки со сдвигом 18.
private struct FolderRows: View {
    let model: NotesModel
    let folder: NoteLibrary.Folder
    let depth: Int
    @State private var target: URL?

    var body: some View {
        ForEach(folder.folders) { child in
            SidebarRow(glyph: .folder, title: child.name, depth: depth,
                       selected: model.selection == .folder(child.url), targeted: target == child.url) {
                model.toggle(child.url)
                model.open(.folder(child.url))
            }
            .contextMenu { NoteMenu(model: model, url: child.url, folder: true) }
            .draggable(child.url)
            .dropDestination(for: URL.self) { urls, _ in model.drop(urls, into: child.url) } isTargeted: { inside in
                if inside { target = child.url } else if target == child.url { target = nil }
            }
            if model.expanded.contains(child.url) {
                FolderRows(model: model, folder: child, depth: depth + 1)
            }
        }
        ForEach(folder.notes) { note in
            SidebarRow(glyph: .file, title: note.title, depth: depth,
                       selected: model.selection == .note(note.url)) {
                model.open(.note(note.url))
            }
            .contextMenu { NoteMenu(model: model, url: note.url) }
            .draggable(note.url)
        }
        ForEach(folder.files) { file in
            SidebarRow(glyph: .file, title: file.url.lastPathComponent, depth: depth,
                       selected: model.selection == .file(file.url)) {
                model.open(.file(file.url))
            }
            .opacity(0.75)
            .contextMenu { NoteMenu(model: model, url: file.url) }
            .draggable(file.url)
        }
    }
}

// Строка боковой панели (компонент «сайдбар/строка»): 227×26, скругление 8, выбранная — чёрный 7 %.
struct SidebarRow: View {
    let glyph: DotGlyph
    let title: String
    var depth = 0
    var count: Int? = nil
    let selected: Bool
    // Сюда сейчас тянут файл — коралловая подсветка.
    var targeted = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                DotIcon(glyph: glyph)
                // Имя файла — то, что ввёл человек: регистр не трогаем.
                Text(LocalizedStringKey(title))
                    .font(Martian.body.font(13))
                    .kerning(Theme.tracking(-7, size: 13))
                    .foregroundStyle(Theme.ink)
                    .lineLimit(1)
                Spacer(minLength: 0)
                if let count {
                    Text("\(count)")
                        .font(Martian.body.font(11))
                        .kerning(Theme.tracking(-7, size: 11))
                        .foregroundStyle(NotesLayout.gray)
                }
            }
            .padding(.leading, 8 + CGFloat(depth) * 18)
            .padding(.trailing, 8)
            .frame(height: 26)
            .background(RoundedRectangle(cornerRadius: 8).fill(Theme.fg.opacity(selected ? 0.07 : 0)))
            .background(RoundedRectangle(cornerRadius: 8).fill(Theme.coral.opacity(targeted ? 0.18 : 0)))
            .animation(.smooth(duration: 0.2), value: targeted)
            .contentShape(RoundedRectangle(cornerRadius: 8))
        }
        .buttonStyle(.plain)
    }
}

// Лист раздела: сверху капсула с путём и две кнопки (надиктовка, новая заметка), под ними — сам раздел.
struct SectionContent: View {
    let model: NotesModel

    var body: some View {
        ZStack(alignment: .topLeading) {
            Group {
                switch model.section {
                case .notes:
                    if let note = model.openNote {
                        NoteEditor(model: model, url: note).id(note)
                    } else if case .file(let url) = model.selection {
                        FilePreview(url: url).id(url)
                    } else {
                        FolderOverview(model: model, folder: model.currentFolder)
                    }
                case .tasks: TasksScreen(model: model.tasks)
                case .calendar:
                    // Календарь — во всю ширину листа (24 слева — колонкам дней нужна ширина, справа — по краю верхних кнопок, 15), а не по колонке текста:
                    // «на таком размере окна можно календарь растянуть» (31.10.2026).
                    CalendarScreen(tasks: model.tasks, uploads: model.video.items)
                        .transformEnvironment(\.pageWidth) { $0 += 116 - 24 - 15 }
                case .video: VideoScreen(model: model)
                case .settings: SettingsScreen(model: model)
                case .sync: SyncScreen(model: model)
                case .mcp: MCPScreen(model: model)
                case .safe: SafeScreen(model: model)
                }
            }
            // Раздел, заметка или папка сменились — старое уходит в размытие, новое из него проявляется,
            // как шаги первого входа. Раньше текст менялся рывком, хотя окно двигалось плавно.
            .id(contentID)
            .transition(.blurReplace)
            .padding(.top, 51)


            if model.section == .notes, !model.tabs.isEmpty {
                NoteTabs(model: model)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.leading, 56).padding(.trailing, 100)
                    .offset(y: 15)
                    .transition(.opacity)
            }

            // Кнопки справа сверху — свои в каждом разделе, в настройках никаких (ТЗ 30.10.2026).
            HStack(spacing: 8) {
                switch model.section {
                case .notes:
                    if Intelligence.isReady, case .note = model.selection {
                        IconButton(glyph: .spark, help: model.summarizing ? String(localized: "сжимаю…") : String(localized: "кратко")) {
                            Task { await model.summarizeNote() }
                        }
                        .disabled(model.summarizing)
                    }
                    IconButton(glyph: .microphone, help: "надиктовать") { DictationController.shared.toggleFromApp() }
                    // Шаблоны — отдельной кнопкой рядом с «новой заметкой», а не только правым кликом (01.11.2026).
                    Menu { TemplateItems(model: model, folder: nil) } label: { IconButtonLabel(glyph: .file) }
                        .menuStyle(.button).menuIndicator(.hidden).buttonStyle(.plain).fixedSize()
                        .help("новая заметка из шаблона")
                    IconButton(glyph: .newNote, help: "новая заметка") { model.newNote() }
                        .keyboardShortcut("n", modifiers: .command)
                        .contextMenu { TemplateItems(model: model, folder: nil) }
                case .tasks:
                    IconButton(glyph: .microphone, help: "надиктовать") { DictationController.shared.toggleFromApp() }
                    IconButton(glyph: .plus, help: "новая задача") { model.tasks.focusNew += 1 }
                        .keyboardShortcut("n", modifiers: .command)
                case .calendar:
                    IconButton(glyph: .microphone, help: "надиктовать") { DictationController.shared.toggleFromApp() }
                case .video:
                    IconButton(glyph: .plus, help: "добавить видео или аудио") { model.video.choose() }
                        .keyboardShortcut("n", modifiers: .command)
                case .settings, .sync, .mcp, .safe:
                    EmptyView()
                }
            }
            .frame(maxWidth: .infinity, alignment: .trailing)
            .padding(.trailing, 15)
            .offset(y: 109 - 92)
        }
        .animation(.smooth(duration: 0.45), value: contentID)
    }

    private var contentID: String {
        switch model.section {
        case .notes: "notes-\(model.selection)"
        case .video: "video-\(model.video.open?.uuidString ?? "")"
        default: "\(model.section)"
        }
    }

    static func crumb(_ model: NotesModel) -> String {
        switch model.section {
        case .notes: model.breadcrumb
        case .tasks: String(localized: "задачи / ") + model.tasks.filter.rawValue.localized
        case .calendar: String(localized: "календарь")
        case .video: model.video.crumb
        case .settings: String(localized: "настройки / ") + model.settingsPage.rawValue.localized
        case .sync: String(localized: "синхронизация")
        case .mcp: "mcp"
        case .safe: model.safe.unlocked ? String(localized: "пароли") + " / " + model.safe.kind.title.localized : String(localized: "пароли")
        }
    }
}

// Путь раздела капсулой над боковой панелью; в заметках клик — ко всем заметкам.
private struct SectionCrumb: View {
    let model: NotesModel

    var body: some View {
        Button { if model.section == .notes { model.open(.folder(model.library.root)) } } label: {
            Text(SectionContent.crumb(model))
                .contentTransition(.interpolate)
                .font(Martian.body.font(13))
                .kerning(Theme.tracking(-7, size: 13))
                .foregroundStyle(NotesLayout.gray)
                .lineLimit(1)
                .padding(.horizontal, 15)
                .frame(maxWidth: 227, minHeight: 36, alignment: .leading)
                .fixedSize()
                .background(Capsule().fill(Theme.bg.opacity(0.2)))
        }
        .buttonStyle(.plain)
        .animation(.smooth(duration: 0.45), value: SectionContent.crumb(model))
    }
}

// Календари в боковой панели: клик — показать или скрыть на экране, цвет — как в Календаре Apple.
// При наведении — «задачи сюда» и корзина; новый календарь вписывается прямо в список, Enter — создать.
private struct CalendarList: View {
    let sync: CalendarSync
    @State private var hovered: String?
    @State private var adding = false
    @State private var name = ""
    // Календарь, который сейчас переименовывают прямо в строке.
    @State private var renaming: String?
    @FocusState private var focused: Bool

    // Цвета на выбор — как в Календаре Apple.
    private static let colors: [(String, NSColor)] = [("красный", .systemRed), ("оранжевый", .systemOrange),
        ("жёлтый", .systemYellow), ("зелёный", .systemGreen), ("голубой", .systemTeal), ("синий", .systemBlue),
        ("фиолетовый", .systemPurple), ("коричневый", .systemBrown)]

    var body: some View {
        let _ = sync.revision
        let main = sync.store.defaultCalendarForNewEvents?.calendarIdentifier
        ForEach(sync.calendars, id: \.calendarIdentifier) { calendar in
            let id = calendar.calendarIdentifier
            let shown = !sync.hidden.contains(id)
            let color = Color(cgColor: calendar.cgColor)
            HStack(spacing: 8) {
                RoundedRectangle(cornerRadius: 4)
                    .strokeBorder(color, lineWidth: 1.5)
                    .background(RoundedRectangle(cornerRadius: 4).fill(shown ? color : .clear))
                    .overlay { if shown { Image(systemName: "checkmark").font(.system(size: 8, weight: .heavy)).foregroundStyle(.white) } }
                    .frame(width: 14, height: 14)
                if renaming == id {
                    TextField("название", text: $name)
                        .textFieldStyle(.plain).font(Martian.body.font(13))
                        .focused($focused)
                        .onSubmit { rename(calendar) }
                        .onExitCommand { renaming = nil }
                        .onAppear { focused = true }
                } else {
                    Text(calendar.title)
                        .font(Martian.body.font(13)).kerning(Theme.tracking(-7, size: 13))
                        .foregroundStyle(Theme.ink.opacity(shown ? 1 : 0.4)).lineLimit(1)
                }
                Spacer(minLength: 0)
                if id == sync.tasksCalendar?.calendarIdentifier {
                    Text("задачи").font(Martian.body.font(10)).foregroundStyle(NotesLayout.gray)
                } else if hovered == id {
                    Button("задачи сюда") { sync.tasksCalendar = calendar }
                        .buttonStyle(QuietLinkStyle(size: 10, opacity: 0.4))
                }
                if hovered == id, id != main {
                    Button { CalendarRemoval.confirm(calendar, sync: sync) } label: {
                        Image(systemName: "trash").font(.system(size: 11)).foregroundStyle(NotesLayout.gray)
                    }.buttonStyle(.plain).help("удалить календарь")
                }
            }
            .padding(.horizontal, 8)
            .frame(height: 28)
            .background(RoundedRectangle(cornerRadius: 8).fill(Theme.fg.opacity(hovered == id ? 0.05 : 0)))
            .contentShape(RoundedRectangle(cornerRadius: 8))
            .onTapGesture { withAnimation(.smooth(duration: 0.2)) { sync.toggleShown(calendar) } }
            .onHover { hovered = $0 ? id : (hovered == id ? nil : hovered) }
            // Настроить календарь — правым кликом: имя, цвет, куда идут задачи, удалить (ТЗ 30.10.2026).
            .contextMenu {
                Button("переименовать") { name = calendar.title; renaming = id }
                Menu("цвет") {
                    ForEach(Self.colors, id: \.0) { title, color in
                        Button(title) { sync.recolor(calendar, color) }
                    }
                }
                if id != sync.tasksCalendar?.calendarIdentifier {
                    Button("задачи сюда") { sync.tasksCalendar = calendar }
                }
                if id != main {
                    Divider()
                    Button("удалить…") { CalendarRemoval.confirm(calendar, sync: sync) }
                }
            }
        }
        if adding {
            TextField("название", text: $name)
                .textFieldStyle(.plain).font(Martian.body.font(13))
                .focused($focused)
                .padding(.horizontal, 8).frame(height: 28)
                .background(RoundedRectangle(cornerRadius: 8).strokeBorder(Theme.coral.opacity(0.5)))
                .onSubmit(create)
                .onExitCommand { adding = false; name = "" }
                .onAppear { focused = true }
        } else {
            SidebarRow(glyph: .plus, title: "новый календарь", selected: false) { adding = true }
                .opacity(0.5)
        }
    }

    private func rename(_ calendar: EKCalendar) {
        let title = name.trimmingCharacters(in: .whitespaces)
        renaming = nil; name = ""
        guard !title.isEmpty, title != calendar.title else { return }
        do { try sync.rename(calendar, title) } catch { NSAlert(error: error).runModal() }
    }

    private func create() {
        let title = name.trimmingCharacters(in: .whitespaces)
        adding = false; name = ""
        guard !title.isEmpty else { return }
        do { try sync.createCalendar(title) } catch { NSAlert(error: error).runModal() }
    }
}

// Папки расшифровок под фильтрами: клик — показать папку, бросить строку видео — переложить,
// правый клик — удалить папку (расшифровки остаются). «новая папка» — имя прямо в строке.
private struct VideoFolders: View {
    let video: VideoModel
    @State private var adding = false
    @State private var name = ""
    @State private var targeted: String?
    @FocusState private var focused: Bool

    var body: some View {
        ForEach(video.folders, id: \.self) { folder in
            SidebarRow(glyph: .folder, title: folder, count: video.count(folder: folder), selected: video.folderFilter == folder) {
                video.filter = .all
                video.folderFilter = folder
                video.open = nil
            }
            .background(RoundedRectangle(cornerRadius: 8).strokeBorder(Theme.coral.opacity(targeted == folder ? 0.6 : 0)))
            .dropDestination(for: String.self) { ids, _ in
                ids.compactMap(UUID.init(uuidString:)).forEach { video.move($0, to: folder) }
                return true
            } isTargeted: { targeted = $0 ? folder : (targeted == folder ? nil : targeted) }
            .contextMenu { Button("удалить папку") { video.removeFolder(folder) } }
        }
        if adding {
            TextField("название папки", text: $name)
                .textFieldStyle(.plain).font(Martian.body.font(13))
                .focused($focused)
                .padding(.horizontal, 8).frame(height: 28)
                .background(RoundedRectangle(cornerRadius: 8).strokeBorder(Theme.coral.opacity(0.5)))
                .onSubmit { video.createFolder(name); adding = false; name = "" }
                .onExitCommand { adding = false; name = "" }
                .onAppear { focused = true }
        } else {
            SidebarRow(glyph: .plus, title: "новая папка", selected: false) { adding = true }
                .opacity(0.5)
        }
    }
}

// Боковая панель своего раздела: дерево заметок, фильтры задач, календари, видео, страницы настроек.
struct SectionSidebar: View {
    let model: NotesModel

    var body: some View {
        switch model.section {
        case .notes: NotesSidebar(model: model)
        case .tasks:
            SidebarList(title: "задачи") {
                ForEach(TasksModel.Filter.allCases, id: \.self) { filter in
                    SidebarRow(glyph: filter == .inCalendar ? .calendar : filter == .done ? .taskDone : .task,
                               title: filter.rawValue, count: model.tasks.count(filter),
                               selected: model.tasks.filter == filter) { model.tasks.filter = filter }
                }
            }
        case .calendar:
            SidebarList(title: "календари") {
                CalendarList(sync: model.tasks.calendar)
            }
        case .video:
            SidebarList(title: "видео") {
                ForEach(VideoModel.Filter.allCases, id: \.self) { filter in
                    SidebarRow(glyph: filter == .done ? .taskDone : .video, title: filter.rawValue,
                               count: model.video.count(filter),
                               selected: model.video.filter == filter && model.video.folderFilter == nil) {
                        model.video.filter = filter
                        model.video.folderFilter = nil
                        model.video.open = nil
                    }
                }
                VideoFolders(video: model.video)
            }
        case .sync:
            SyncSidebar(model: model)
        case .mcp:
            MCPSidebar(model: model)
        case .safe:
            SafeSidebar(model: model)
        case .settings:
            SidebarList(title: "настройки") {
                ForEach(NotesModel.SettingsPage.allCases, id: \.self) { page in
                    SidebarRow(glyph: page.glyph, title: page.rawValue, selected: model.settingsPage == page) {
                        model.settingsPage = page
                    }
                }
            }
        }
    }
}

struct SidebarList<Rows: View>: View {
    let title: String
    @ViewBuilder var rows: () -> Rows

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Заголовка над строками в кадре 152:32854 нет — раздел виден по пути над панелью.
            VStack(alignment: .leading, spacing: 2) { rows() }
                .padding(.horizontal, 10)
                .padding(.top, 15)
            Spacer(minLength: 0)
        }
    }
}

// Круглая кнопка 32×32 с иконкой (компонент «кнопка/иконка»): белый 45 %, мягкая тень.
// Иконка в круглой кнопке 32×32 — без действия, чтобы ставить в свои Button.
struct IconButtonLabel: View {
    let glyph: DotGlyph
    var body: some View {
        DotIcon(glyph: glyph)
            .frame(width: 32, height: 32)
            .background(Circle().fill(Theme.bg.opacity(0.45)))
            .shadow(color: .black.opacity(0.06), radius: 4, y: 1)
            .contentShape(Circle())
    }
}

struct IconButton: View {
    let glyph: DotGlyph
    let help: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            DotIcon(glyph: glyph)
                .frame(width: 32, height: 32)
                .background(Circle().fill(Theme.bg.opacity(0.45)))
                .shadow(color: .black.opacity(0.06), radius: 4, y: 1)
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .help(LocalizedStringKey(help))
    }
}

struct StaticSlats: View, Equatable {
    let width: CGFloat
    var body: some View {
        GlassSlats(slatWidth: Layout.finalSlatWidth).drawingGroup()
    }
}

// Подсказка под значком сразу при наведении — системная всплывает через секунду и её «не видно» (01.11.2026).
extension View {
    func hoverHint(_ text: String, when shown: Bool) -> some View {
        overlay(alignment: .bottom) {
            if shown {
                Text(LocalizedStringKey(text))
                    .font(Martian.body.font(11))
                    .kerning(Theme.tracking(-7, size: 11))
                    .foregroundStyle(Theme.fg)
                    .fixedSize()
                    .padding(.horizontal, 10).padding(.vertical, 6)
                    .background(Capsule().fill(.regularMaterial))
                    .offset(y: 34)
                    .allowsHitTesting(false)
                    .transition(.opacity)
            }
        }
        .animation(.smooth(duration: 0.15), value: shown)
    }
}

