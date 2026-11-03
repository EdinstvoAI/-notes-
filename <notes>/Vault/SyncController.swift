import Foundation
import NotesCore
import os

// Фоновая синхронизация хранилища. Раз в час (и сразу при запуске):
// 1) коммитим всё, что накопилось — кто бы ни правил: мы, другой редактор или Finder;
// 2) забираем правки с другого Мака (pull); конфликты не теряют текст — см. GitVault+Pull;
// 3) отправляем на GitHub, если прошли сутки с прошлой отправки.
// Логика — в NotesCore.SyncEngine.
@Observable
final class SyncController {
    static let shared = SyncController()

    // Коммит по расписанию из настроек (по умолчанию раз в час), а не на каждую паузу в правках:
    // история читается по часам, а не по словам.
    static var commitInterval: Duration { .seconds(AppSettings.commitMinutes * 60) }

    private static let lastPushKey = "lastPush"
    private let log = Logger(subsystem: "notes", category: "sync")
    private var engine: SyncEngine?
    // Когда последний раз прошёл цикл синхронизации — для «синхронизировано 2 мин назад» в шапке.
    private(set) var lastSync: Date?
    // Последний коммит и последняя ошибка — раньше ошибки писались только в системный журнал,
    // и что история не уходит на GitHub, было не видно (30.10.2026).
    private(set) var lastCommit: Date?
    private(set) var lastError: String?
    // libgit2 на отозванный токен отвечает «too many redirects or authentication replays».
    var needsLogin: Bool { lastError.map { $0.contains("authentication") || $0.contains("401") } ?? false }
    var errorText: String? { needsLogin ? String(localized: "вход в github устарел") : lastError }
    func clearError() { lastError = nil }
    private var vault: URL?
    private var loop: Task<Void, Never>?

    // Запускаем, только если папка уже git-репозиторий: без «инициализировать» историю не ведём.
    func start(vault: URL) {
        guard engine == nil, GitVault.isRepository(vault) else { return }
        self.vault = vault
        let engine = SyncEngine(folder: vault)
        self.engine = engine
        loop = Task { [weak self] in
            while !Task.isCancelled {
                await self?.tick()
                try? await Task.sleep(for: Self.commitInterval)
            }
        }
        log.info("синхронизация запущена: \(vault.path, privacy: .private)")
    }

    private func tick() async {
        guard let engine else { return }
        lastError = nil
        await connectIfNeeded()
        do {
            if let message = try await engine.commitNow() {
                log.info("коммит: \(message, privacy: .private)")
                lastCommit = Date()
            }
        } catch {
            log.error("не закоммитили: \(String(describing: error), privacy: .public)")
            lastError = String(localized: "коммит: \(error.localizedDescription)")
        }
        await pull()
        await pushIfDue()
        lastSync = Date()
    }

    var isRunning: Bool { engine != nil }

    // Вход в GitHub есть, а папка к нему не привязана (сменили папку или пересоздали приложение) —
    // привязываем к своему notes, забираем его историю и отправляем свою. Раньше история так и
    // оставалась только на Mac.
    private func connectIfNeeded() async {
        guard let vault, let token = await Self.token() else { return }
        let hasRemote = await Task.detached { (try? GitVault.open(at: vault))?.remoteURL() != nil }.value
        guard !hasRemote else { return }
        do {
            let repo = try await GitHubRepositories(token: token).ensureNotes()
            try await Task.detached {
                let git = try GitVault.open(at: vault)
                try git.setRemote(repo.cloneURL)
                _ = try git.pull(token: token)
                try git.push(token: token)
            }.value
            Self.markPushed()
            log.info("папка привязана к \(repo.fullName, privacy: .public)")
        } catch {
            lastError = String(localized: "привязать к github: \(error.localizedDescription)")
        }
    }
    static var lastPush: Date? { UserDefaults.standard.object(forKey: lastPushKey) as? Date }

    // «отправить сейчас» в настройках: коммит накопившегося и отправка, не дожидаясь суток.
    func pushNow() async -> Bool {
        guard let engine, let token = await Self.token() else { return false }
        _ = try? await engine.commitNow()
        await connectIfNeeded()
        var sent = false
        do {
            sent = try await engine.pushIfDue(token: token, lastPush: .distantPast)
            if sent { Self.markPushed() }
        } catch {
            lastError = String(localized: "отправить на github: \(error.localizedDescription)")
        }
        lastSync = Date()
        return sent
    }

    // Подпись в шапке главного экрана.
    func status(now: Date) -> String {
        guard engine != nil else { return String(localized: "без git") }
        if lastError != nil { return String(localized: "не отправилось") }
        guard let lastSync else { return String(localized: "синхронизируем…") }
        let minutes = Int(now.timeIntervalSince(lastSync) / 60)
        switch minutes {
        case ..<1: return String(localized: "синхронизировано только что")
        case ..<60: return String(localized: "синхронизировано \(minutes) мин назад")
        default: return String(localized: "синхронизировано \(minutes / 60) ч назад")
        }
    }

    // Первую отправку делает вход в GitHub — с неё и отсчитываем сутки.
    static func markPushed(_ date: Date = Date()) {
        UserDefaults.standard.set(date, forKey: lastPushKey)
    }

    // Чтение токена может ждать окна macOS «разрешить доступ к Связке ключей» — не на главном потоке,
    // иначе всё приложение замирает, пока окно не закрыли.
    private static func token() async -> String? {
        await Task.detached { await GitHubSession.token(clientID: GitHubApp.clientID) }.value
    }

    // Без входа в GitHub забирать неоткуда — пропускаем.
    private func pull() async {
        guard let engine, let token = await Self.token() else { return }
        do {
            switch try await engine.pull(token: token) {
            case .merged(let conflicts) where !conflicts.isEmpty:
                log.info("слили с другого мака, конфликтов: \(conflicts.count), обе версии сохранены")
            default:
                break
            }
        } catch {
            log.error("не забрали с github: \(String(describing: error), privacy: .public)")
            lastError = String(localized: "забрать с github: \(error.localizedDescription)")
        }
    }

    private func pushIfDue() async {
        // Без входа в GitHub токена нет — история остаётся на этом Маке.
        guard let engine, let token = await Self.token() else { return }
        let lastPush = UserDefaults.standard.object(forKey: Self.lastPushKey) as? Date
        do {
            if try await engine.pushIfDue(token: token, lastPush: lastPush, every: TimeInterval(AppSettings.pushMinutes * 60)) {
                Self.markPushed()
                log.info("история отправлена на github")
            }
        } catch {
            log.error("не отправили на github: \(String(describing: error), privacy: .public)")
            lastError = String(localized: "отправить на github: \(error.localizedDescription)")
        }
    }
}
