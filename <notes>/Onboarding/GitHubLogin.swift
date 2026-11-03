import Foundation
import NotesCore

// Шаг 4: вход в GitHub по коду, создание приватного notes и первая отправка истории.
// Логика входа и git — в NotesCore (покрыта тестами), здесь только порядок шагов и состояние для экрана.
@Observable
final class GitHubLogin {
    enum State: Equatable {
        case requesting                 // просим у GitHub код
        case waiting(code: String, verification: URL)
        case connecting                 // код подтвердили: создаём notes и отправляем историю
        case expired                    // код протух — нужен новый
        case failed(String)
    }

    private(set) var state: State = .requesting

    // Весь путь от кода до отправки истории. Задача отменяется, когда пользователь уходит с экрана:
    // тогда опрос GitHub прекращается, а device_code пропадает вместе с ней (CONTEXT.md).
    func run(vault: URL?, onConnected: @escaping (String) -> Void) async {
        guard let clientID = GitHubApp.clientID else {
            state = .failed(String(localized: "вход в github ещё не настроен"))
            return
        }
        state = .requesting
        do {
            let flow = GitHubDeviceFlow(clientID: clientID)
            let code = try await flow.requestCode()
            state = .waiting(code: code.userCode, verification: code.verificationURI)

            let grant = try await flow.waitForGrant(code)
            try GitHubSession.save(grant)
            let token = grant.token
            state = .connecting

            let repo = try await GitHubRepositories(token: token).ensureNotes()
            if let vault {
                // libgit2 работает синхронно и с диском — уводим с главного потока, чтобы не дёргался UI.
                try await Task.detached {
                    let git = try GitVault.prepare(at: vault).vault
                    try git.setRemote(repo.cloneURL)
                    // notes уже был (второй Мак) — сначала забираем и сливаем его историю,
                    // иначе GitHub отклонит push: у сервера своя история, у этой папки своя.
                    _ = try git.pull(token: token)
                    try git.push(token: token)
                }.value
            }
            SyncController.markPushed()
            onConnected(repo.fullName)
        } catch is CancellationError {
            return
        } catch GitHubError.expired {
            state = .expired
        } catch GitHubError.denied {
            state = .failed(String(localized: "вход отменён на github"))
        } catch {
            state = .failed(String(describing: error))
        }
    }
}
