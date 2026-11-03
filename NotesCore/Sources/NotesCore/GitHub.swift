import Foundation

// Сеть подменяется в тестах, поэтому GitHub-клиенты ходят через этот протокол, а не через URLSession напрямую.
public protocol HTTPClient: Sendable {
    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse)
}

extension URLSession: HTTPClient {
    public func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let (data, response) = try await data(for: request)
        guard let http = response as? HTTPURLResponse else { throw URLError(.badServerResponse) }
        return (data, http)
    }
}

public enum GitHubError: Error, Equatable {
    case expired       // код протух (живёт 15 минут, задаёт GitHub)
    case denied        // на GitHub нажали «Cancel»
    case unexpected(String)
}

// Вход по коду (OAuth Device Flow). Работает без своего сервера и без client secret:
// приложение показывает короткий код, человек вводит его на github.com/login/device,
// а приложение тем временем спрашивает GitHub, подтвердили ли вход.
// Веб-аналог — вход на телевизоре по коду с телефона.
public struct GitHubDeviceFlow: Sendable {
    public struct Code: Decodable, Equatable, Sendable {
        public let deviceCode: String
        public let userCode: String
        public let verificationURI: URL
        public let expiresIn: Int
        public let interval: Int

        enum CodingKeys: String, CodingKey {
            case deviceCode = "device_code", userCode = "user_code", verificationURI = "verification_uri"
            case expiresIn = "expires_in", interval
        }
    }

    // Доступ к репозиториям: нужен, чтобы создать приватный notes и отправлять в него историю.
    public static let scope = "repo"

    let clientID: String
    let http: HTTPClient
    let sleep: @Sendable (Duration) async throws -> Void

    public init(clientID: String, http: HTTPClient = URLSession.shared,
                sleep: @escaping @Sendable (Duration) async throws -> Void = { try await Task.sleep(for: $0) }) {
        self.clientID = clientID
        self.http = http
        self.sleep = sleep
    }

    public func requestCode() async throws -> Code {
        let data = try await post("https://github.com/login/device/code",
                                   ["client_id": clientID, "scope": Self.scope])
        return try JSONDecoder().decode(Code.self, from: data)
    }

    // Ждёт подтверждения на GitHub и возвращает токен. Отмена задачи (ушли с экрана) прекращает
    // опрос — device_code нигде не сохраняется, поэтому код, введённый позже, никому не даст токен.
    public func waitForToken(_ code: Code) async throws -> String { try await waitForGrant(code).token }

    // Токен и, если в приложении GitHub включено истечение токенов, ключ продления. Без продления GitHub
    // выкидывал раз в сутки (03.11.2026, «он раз в сутки меня выкидывает»).
    public struct Grant: Equatable, Sendable {
        public let token: String
        public let refresh: String?
        public let expires: Date?
    }

    public func waitForGrant(_ code: Code) async throws -> Grant {
        var interval = code.interval
        while true {
            try await sleep(.seconds(interval))
            try Task.checkCancellation()
            let data = try await post("https://github.com/login/oauth/access_token", [
                "client_id": clientID,
                "device_code": code.deviceCode,
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
            ])
            let reply = try JSONDecoder().decode(TokenReply.self, from: data)
            if let grant = reply.grant { return grant }
            switch reply.error {
            case "authorization_pending": continue
            // GitHub просит опрашивать реже: прибавляем 5 секунд, как сказано в его документации.
            case "slow_down": interval = reply.interval ?? interval + 5
            case "expired_token": throw GitHubError.expired
            case "access_denied": throw GitHubError.denied
            default: throw GitHubError.unexpected(reply.error ?? "пустой ответ")
            }
        }
    }

    // Продлить токен ключом продления; новый ключ продления приходит вместе с токеном.
    public func refresh(_ refreshToken: String) async throws -> Grant {
        let data = try await post("https://github.com/login/oauth/access_token", [
            "client_id": clientID, "grant_type": "refresh_token", "refresh_token": refreshToken,
        ])
        let reply = try JSONDecoder().decode(TokenReply.self, from: data)
        guard let grant = reply.grant else { throw GitHubError.unexpected(reply.error ?? "пустой ответ") }
        return grant
    }

    private struct TokenReply: Decodable {
        let accessToken: String?
        let refreshToken: String?
        let expiresIn: Int?
        let error: String?
        let interval: Int?

        var grant: Grant? {
            accessToken.map { Grant(token: $0, refresh: refreshToken, expires: expiresIn.map { Date().addingTimeInterval(TimeInterval($0)) }) }
        }

        enum CodingKeys: String, CodingKey {
            case accessToken = "access_token", refreshToken = "refresh_token", expiresIn = "expires_in", error, interval
        }
    }

    private func post(_ url: String, _ form: [String: String]) async throws -> Data {
        var request = URLRequest(url: URL(string: url)!)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        var components = URLComponents()
        components.queryItems = form.sorted { $0.key < $1.key }.map { URLQueryItem(name: $0.key, value: $0.value) }
        request.httpBody = components.percentEncodedQuery?.data(using: .utf8)
        let (data, response) = try await http.send(request)
        guard (200..<300).contains(response.statusCode) else {
            throw GitHubError.unexpected("github ответил \(response.statusCode)")
        }
        return data
    }
}

// Вход в GitHub между запусками: токен и ключ продления — в Связке ключей, срок — в настройках.
// Токен истекает через 5 минут или раньше — продлеваем заранее; без ключа продления токен вечный.
public enum GitHubSession {
    static let expiresKey = "githubTokenExpires"

    public static func save(_ grant: GitHubDeviceFlow.Grant) throws {
        try TokenStore().save(grant.token)
        if let refresh = grant.refresh { try TokenStore(account: "refresh").save(refresh) }
        UserDefaults.standard.set(grant.expires, forKey: expiresKey)
    }

    public static func token(clientID: String?, now: Date = Date()) async -> String? {
        guard let token = TokenStore().load() else { return nil }
        guard let expires = UserDefaults.standard.object(forKey: expiresKey) as? Date, expires < now.addingTimeInterval(300),
              let clientID, let refresh = TokenStore(account: "refresh").load(),
              let grant = try? await GitHubDeviceFlow(clientID: clientID).refresh(refresh) else { return token }
        try? save(grant)
        return grant.token
    }
}

// Репозиторий для истории: приватный notes в аккаунте пользователя.
public struct GitHubRepositories: Sendable {
    public struct Repository: Decodable, Equatable, Sendable {
        public let fullName: String
        public let cloneURL: String
        public let isPrivate: Bool

        enum CodingKeys: String, CodingKey {
            case fullName = "full_name", cloneURL = "clone_url", isPrivate = "private"
        }
    }

    public static let name = "notes"

    let token: String
    let http: HTTPClient

    public init(token: String, http: HTTPClient = URLSession.shared) {
        self.token = token
        self.http = http
    }

    // Создаёт приватный notes. Если такой уже есть (GitHub отвечает 422), подключаемся к нему.
    public func ensureNotes() async throws -> Repository {
        var create = request("https://api.github.com/user/repos")
        create.httpMethod = "POST"
        create.httpBody = try JSONSerialization.data(withJSONObject: [
            "name": Self.name, "private": true, "auto_init": false,
            "description": "заметки <notes>",
        ])
        let (data, response) = try await http.send(create)
        switch response.statusCode {
        case 201:
            return try JSONDecoder().decode(Repository.self, from: data)
        case 422:
            let login = try await currentLogin()
            let (existing, existingResponse) = try await http.send(request("https://api.github.com/repos/\(login)/\(Self.name)"))
            guard existingResponse.statusCode == 200 else {
                throw GitHubError.unexpected("не нашли \(login)/\(Self.name): \(existingResponse.statusCode)")
            }
            return try JSONDecoder().decode(Repository.self, from: existing)
        default:
            throw GitHubError.unexpected("github ответил \(response.statusCode)")
        }
    }

    public func currentLogin() async throws -> String {
        let (data, response) = try await http.send(request("https://api.github.com/user"))
        guard response.statusCode == 200 else { throw GitHubError.unexpected("github ответил \(response.statusCode)") }
        struct User: Decodable { let login: String }
        return try JSONDecoder().decode(User.self, from: data).login
    }

    private func request(_ url: String) -> URLRequest {
        var request = URLRequest(url: URL(string: url)!)
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        request.setValue("2022-11-28", forHTTPHeaderField: "X-GitHub-Api-Version")
        return request
    }
}
