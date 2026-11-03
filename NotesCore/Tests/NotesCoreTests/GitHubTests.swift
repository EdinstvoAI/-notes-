import Foundation
import Testing
@testable import NotesCore

// Подставной GitHub: отдаёт заранее заготовленные ответы по порядку и запоминает запросы.
private final class StubHTTP: HTTPClient, @unchecked Sendable {
    private var replies: [(Int, String)]
    private(set) var requests: [URLRequest] = []
    private let lock = NSLock()

    init(_ replies: [(Int, String)]) { self.replies = replies }

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let (status, body) = lock.withLock {
            requests.append(request)
            return replies.removeFirst()
        }
        let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil, headerFields: nil)!
        return (Data(body.utf8), response)
    }
}

private final class Sleeps: @unchecked Sendable {
    var values: [Duration] = []
}

private let code = GitHubDeviceFlow.Code(deviceCode: "dev", userCode: "WDJB-MJHT",
                                         verificationURI: URL(string: "https://github.com/login/device")!,
                                         expiresIn: 900, interval: 5)

@Suite("вход в github по коду")
struct GitHubDeviceFlowTests {
    @Test("код разбирается из ответа github")
    func parsesCode() async throws {
        let http = StubHTTP([(200, #"{"device_code":"dev","user_code":"WDJB-MJHT","verification_uri":"https://github.com/login/device","expires_in":900,"interval":5}"#)])
        let flow = GitHubDeviceFlow(clientID: "id", http: http, sleep: { _ in })

        #expect(try await flow.requestCode() == code)
        // Без Accept: application/json github отвечает формой, а не JSON.
        #expect(http.requests.first?.value(forHTTPHeaderField: "Accept") == "application/json")
    }

    @Test("истекающий токен приходит с ключом продления, продление даёт новый")
    func refreshesExpiringToken() async throws {
        let http = StubHTTP([(200, #"{"access_token":"ghu_1","expires_in":28800,"refresh_token":"ghr_1"}"#),
                             (200, #"{"access_token":"ghu_2","expires_in":28800,"refresh_token":"ghr_2"}"#)])
        let flow = GitHubDeviceFlow(clientID: "id", http: http, sleep: { _ in })
        let grant = try await flow.waitForGrant(code)
        #expect(grant.refresh == "ghr_1" && grant.expires != nil)
        let next = try await flow.refresh("ghr_1")
        #expect(next.token == "ghu_2" && next.refresh == "ghr_2")
        let body = String(decoding: http.requests[1].httpBody ?? Data(), as: UTF8.self)
        #expect(body.contains("grant_type=refresh_token") && body.contains("refresh_token=ghr_1"))
    }

    @Test("ждём, пока подтвердят, и получаем токен")
    func waitsForApproval() async throws {
        let http = StubHTTP([(200, #"{"error":"authorization_pending"}"#),
                             (200, #"{"error":"authorization_pending"}"#),
                             (200, #"{"access_token":"gho_123","token_type":"bearer"}"#)])
        let flow = GitHubDeviceFlow(clientID: "id", http: http, sleep: { _ in })

        #expect(try await flow.waitForToken(code) == "gho_123")
        #expect(http.requests.count == 3)
    }

    @Test("slow_down — опрашиваем реже")
    func slowsDown() async throws {
        let sleeps = Sleeps()
        let http = StubHTTP([(200, #"{"error":"slow_down","interval":10}"#),
                             (200, #"{"access_token":"t"}"#)])
        let flow = GitHubDeviceFlow(clientID: "id", http: http, sleep: { sleeps.values.append($0) })

        _ = try await flow.waitForToken(code)

        #expect(sleeps.values == [.seconds(5), .seconds(10)])
    }

    @Test("код протух — понятная ошибка, а не вечное ожидание")
    func reportsExpiredCode() async throws {
        let http = StubHTTP([(200, #"{"error":"expired_token"}"#)])
        let flow = GitHubDeviceFlow(clientID: "id", http: http, sleep: { _ in })

        await #expect(throws: GitHubError.expired) { try await flow.waitForToken(code) }
    }

    @Test("на github нажали cancel")
    func reportsDenied() async throws {
        let http = StubHTTP([(200, #"{"error":"access_denied"}"#)])
        let flow = GitHubDeviceFlow(clientID: "id", http: http, sleep: { _ in })

        await #expect(throws: GitHubError.denied) { try await flow.waitForToken(code) }
    }
}

@Suite("репозиторий notes на github")
struct GitHubRepositoriesTests {
    @Test("создаём приватный notes")
    func createsPrivateNotes() async throws {
        let http = StubHTTP([(201, #"{"full_name":"makark/notes","clone_url":"https://github.com/makark/notes.git","private":true}"#)])

        let repo = try await GitHubRepositories(token: "t", http: http).ensureNotes()

        #expect(repo.fullName == "makark/notes")
        #expect(repo.isPrivate)
        let body = try JSONSerialization.jsonObject(with: http.requests[0].httpBody!) as! [String: Any]
        #expect(body["private"] as? Bool == true)
    }

    @Test("notes уже есть — подключаемся к нему, а не падаем")
    func reusesExistingNotes() async throws {
        let http = StubHTTP([(422, #"{"message":"Repository creation failed."}"#),
                             (200, #"{"login":"makark"}"#),
                             (200, #"{"full_name":"makark/notes","clone_url":"https://github.com/makark/notes.git","private":true}"#)])

        let repo = try await GitHubRepositories(token: "t", http: http).ensureNotes()

        #expect(repo.cloneURL == "https://github.com/makark/notes.git")
        #expect(http.requests[2].url?.path == "/repos/makark/notes")
    }
}
