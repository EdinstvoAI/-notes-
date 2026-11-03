import AppKit
import NotesCore
import SwiftUI

// Панель значка в строке меню: сейф паролей под рукой — найти, скопировать пароль или код 2FA,
// плюс «открыть <notes>» и надиктовку. Сейф тот же, что в окне (SafeModel.shared).
struct MenuBarPanel: View {
    let vault: URL?
    @State private var entry = ""
    @State private var query = ""
    @State private var copied: UUID?

    // Значок — точечная иконка заметок из шапки, шаблоном: macOS сама красит под светлую и тёмную строку.
    @MainActor static let icon: NSImage = {
        let image = ImageRenderer(content: DotIcon(glyph: .notes).foregroundStyle(.black)).nsImage ?? NSImage()
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
