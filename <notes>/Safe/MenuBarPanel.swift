import AppKit
import NotesCore
import SwiftUI

// Свой значок вместо MenuBarExtra: тот встаёт левее всех и на забитой строке уходит под вырез с камерой
// («не скрой динамик айленд», 03.11.2026). Место — у часов: macOS берёт его из «NSStatusItem Preferred Position».
@MainActor final class MenuBarItem: NSObject {
    static let shared = MenuBarItem()
    private var item: NSStatusItem?
    private let popover = NSPopover()

    func start() {
        guard item == nil else { return }
        let name = "notes"
        let key = "NSStatusItem Preferred Position \(name)"
        if UserDefaults.standard.object(forKey: key) == nil { UserDefaults.standard.set(260, forKey: key) }
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        item.autosaveName = name
        item.button?.image = MenuBarPanel.icon
        item.button?.target = self
        item.button?.action = #selector(toggle)
        popover.behavior = .transient
        popover.contentViewController = NSHostingController(rootView: MenuBarPanel(vault: VaultStore.restore()))
        self.item = item
    }

    @objc private func toggle() {
        guard let button = item?.button else { return }
        if popover.isShown { popover.performClose(nil) } else {
            popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
            popover.contentViewController?.view.window?.makeKey()
        }
    }
}

// Панель значка в строке меню: сейф паролей под рукой — найти, скопировать пароль или код 2FA,
// плюс «открыть <notes>» и надиктовку. Сейф тот же, что в окне (SafeModel.shared).
struct MenuBarPanel: View {
    let vault: URL?
    @State private var entry = ""
    @State private var query = ""
    @State private var copied: UUID?

    // Значок — точечная иконка заметок из шапки, шаблоном: macOS сама красит под светлую и тёмную строку.
    @MainActor static let icon: NSImage = {
        // «<n>» шрифтом логотипа (03.11.2026, «мб сделать иконку <n>»).
        let renderer = ImageRenderer(content: Text("<n>").font(Martian.logo.font(15)).foregroundStyle(.black))
        renderer.scale = 2
        let image = renderer.nsImage ?? NSImage()
        image.size = NSSize(width: image.size.width / 2, height: image.size.height / 2)
        image.isTemplate = true
        return image
    }()

    var body: some View {
        let safe = SafeModel.shared
        VStack(alignment: .leading, spacing: 10) {
            if let vault {
                if safe.unlocked {
                    TextField("найти пароль", text: $query)
                        .textFieldStyle(.plain).font(Martian.body.font(13))
                        .padding(10).background(RoundedRectangle(cornerRadius: 10).fill(Theme.fg.opacity(0.05)))
                    ScrollView {
                        VStack(spacing: 4) {
                            ForEach(found(safe)) { item in row(item) }
                        }
                    }
                    .frame(maxHeight: 320)
                    Button("закрыть сейф") { safe.lock() }.buttonStyle(QuietLinkStyle(size: 12, opacity: 0.6))
                } else if PasswordSafe.exists(vault: vault) {
                    SecureField("мастер-пароль", text: $entry)
                        .textFieldStyle(.plain).font(Martian.body.font(13))
                        .padding(10).background(RoundedRectangle(cornerRadius: 10).fill(Theme.fg.opacity(0.05)))
                        .onSubmit { safe.unlock(vault, entry); entry = "" }
                    if let error = safe.error { Text(error).font(Martian.body.font(11)).foregroundStyle(Theme.coral) }
                } else {
                    Text("сейфа ещё нет — создай его в разделе «пароли»")
                        .font(Martian.body.font(12)).foregroundStyle(NotesLayout.gray)
                }
            }
            Divider()
            HStack {
                Button("открыть <notes>") { NSApp.activate(); NSApp.windows.first { $0.canBecomeMain }?.makeKeyAndOrderFront(nil) }
                Spacer()
                Button("надиктовать") { DictationController.shared.toggleFromApp() }
            }
            .buttonStyle(QuietLinkStyle(size: 12, opacity: 0.7))
        }
        .padding(14)
        .frame(width: 320)
    }

    private func found(_ safe: SafeModel) -> [SafeItem] {
        let words = query.lowercased()
        return safe.items.filter { words.isEmpty || ($0.title + " " + $0.username + " " + $0.url).lowercased().contains(words) }
    }

    private func row(_ item: SafeItem) -> some View {
        Button {
            SafeModel.copy(item.kind == .totp ? TOTP.code(secret: item.secret) ?? "" : item.secret)
            copied = item.id
        } label: {
            HStack {
                VStack(alignment: .leading, spacing: 1) {
                    Text(item.title).font(Martian.body.font(13)).foregroundStyle(Theme.ink)
                    Text(item.username).font(Martian.body.font(11)).foregroundStyle(NotesLayout.gray)
                }
                Spacer()
                if item.kind == .totp {
                    TimelineView(.periodic(from: .now, by: 1)) { context in
                        Text(TOTP.code(secret: item.secret, date: context.date) ?? "—")
                            .font(Martian.body.font(13)).monospacedDigit().foregroundStyle(Theme.ink)
                    }
                }
                Text(copied == item.id ? "скопировано" : "скопировать")
                    .font(Martian.body.font(11)).foregroundStyle(NotesLayout.gray)
            }
            .padding(8)
            .background(RoundedRectangle(cornerRadius: 8).fill(Theme.fg.opacity(0.04)))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
