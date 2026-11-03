import CommonCrypto
import CryptoKit
import Foundation

// Сейф паролей (03.11.2026, «свой сейф»): пароли, коды 2FA (TOTP) и секреты в одном зашифрованном файле
// `.safe/safe.enc` внутри хранилища. Ключ — только из мастер-пароля (PBKDF2-SHA256), Связку ключей не
// используем: Макар ей не доверяет. Файл уходит через git на другой Mac и GitHub уже шифром.
// Забыт мастер-пароль — сейф не открыть никому, восстановления нет.
public struct SafeItem: Codable, Identifiable, Equatable, Sendable {
    public enum Kind: String, Codable, CaseIterable, Sendable { case login, totp, secret }

    public var id = UUID()
    public var kind: Kind
    public var title: String
    public var username = ""
    public var secret = ""       // пароль, секрет TOTP в base32 или сам секрет
    public var url = ""
    public var note = ""
    public var modified = Date()

    public init(kind: Kind, title: String, username: String = "", secret: String = "", url: String = "", note: String = "") {
        self.kind = kind
        self.title = title
        self.username = username
        self.secret = secret
        self.url = url
        self.note = note
    }
}

public enum PasswordSafe {
    public static let file = ".safe/safe.enc"
    // Коды 2FA — отдельным файлом и своим ключом (03.11.2026, «изолировать»): утёк один ключ — второй файл цел.
    public static let codesFile = ".safe/codes.enc"
    static let iterations: UInt32 = 600_000
    static let magic = Data("NSAFE1".utf8)

    public enum Failure: Error, Equatable { case wrongPassword, damaged }

    // Формат: «NSAFE1» + соль 16 байт + запечатанный AES-GCM (nonce + шифр + тег). Соль новая на каждую запись.
    public static func seal(_ items: [SafeItem], password: String) throws -> Data {
        var salt = Data(count: 16)
        _ = salt.withUnsafeMutableBytes { SecRandomCopyBytes(kSecRandomDefault, 16, $0.baseAddress!) }
        let json = try JSONEncoder().encode(items)
        let box = try AES.GCM.seal(json, using: key(password, salt: salt))
        return magic + salt + box.combined!
    }

    public static func open(_ data: Data, password: String) throws -> [SafeItem] {
        guard data.count > magic.count + 16 + 28, data.prefix(magic.count) == magic else { throw Failure.damaged }
        let salt = data.dropFirst(magic.count).prefix(16)
        let body = data.dropFirst(magic.count + 16)
        guard let box = try? AES.GCM.SealedBox(combined: body) else { throw Failure.damaged }
        // Неверный пароль и подмена файла неотличимы: тег GCM не сходится.
        guard let json = try? AES.GCM.open(box, using: key(password, salt: Data(salt))) else { throw Failure.wrongPassword }
        return try JSONDecoder().decode([SafeItem].self, from: json)
    }

    public static func load(vault: URL, password: String, file: String = file) throws -> [SafeItem]? {
        let url = vault.appendingPathComponent(file)
        guard let data = try? Data(contentsOf: url) else { return nil }
        return try open(data, password: password)
    }

    public static func save(_ items: [SafeItem], vault: URL, password: String, file: String = file) throws {
        let url = vault.appendingPathComponent(file)
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try seal(items, password: password).write(to: url, options: .atomic)
    }

    public static func exists(vault: URL, file: String = file) -> Bool {
        FileManager.default.fileExists(atPath: vault.appendingPathComponent(file).path)
    }

    static func key(_ password: String, salt: Data) -> SymmetricKey {
        var key = Data(count: 32)
        let pass = Array(password.utf8)
        _ = key.withUnsafeMutableBytes { out in
            salt.withUnsafeBytes { s in
                CCKeyDerivationPBKDF(CCPBKDFAlgorithm(kCCPBKDF2), pass.map { Int8(bitPattern: $0) }, pass.count,
                                     s.bindMemory(to: UInt8.self).baseAddress, salt.count,
                                     CCPseudoRandomAlgorithm(kCCPRFHmacAlgSHA256), iterations,
                                     out.bindMemory(to: UInt8.self).baseAddress, 32)
            }
        }
        return SymmetricKey(data: key)
    }

    // Импорт из «Паролей» Apple: Файл → «Экспортировать все пароли» даёт CSV
    // Title,URL,Username,Password,Notes,OTPAuth. Пароль — записью «пароли», код 2FA — отдельной записью «коды».
    public static func importPasswordsCSV(_ text: String) -> [SafeItem] {
        let rows = csv(text)
        guard let header = rows.first?.map({ $0.lowercased() }) else { return [] }
        func column(_ name: String) -> Int? { header.firstIndex(of: name) }
        let (t, u, n, p, note, otp) = (column("title"), column("url"), column("username"), column("password"), column("notes"), column("otpauth"))
        var items: [SafeItem] = []
        for row in rows.dropFirst() {
            func value(_ i: Int?) -> String { i.flatMap { $0 < row.count ? row[$0] : nil } ?? "" }
            let title = value(t).isEmpty ? value(u) : value(t)
            if !value(p).isEmpty {
                items.append(SafeItem(kind: .login, title: title, username: value(n), secret: value(p), url: value(u), note: value(note)))
            }
            if !value(otp).isEmpty {
                items.append(SafeItem(kind: .totp, title: title, username: value(n), secret: value(otp)))
            }
        }
        return items
    }

    // CSV с кавычками, запятыми и переносами внутри полей.
    static func csv(_ text: String) -> [[String]] {
        var rows: [[String]] = [], row: [String] = [], field = "", quoted = false
        var chars = Array(text.replacingOccurrences(of: "\r\n", with: "\n"))[...]
        while let c = chars.popFirst() {
            if quoted {
                if c == "\"" { if chars.first == "\"" { field.append("\""); chars.removeFirst() } else { quoted = false } }
                else { field.append(c) }
            } else if c == "\"" { quoted = true }
            else if c == "," { row.append(field); field = "" }
            else if c == "\n" { row.append(field); rows.append(row); row = []; field = "" }
            else { field.append(c) }
        }
        if !field.isEmpty || !row.isEmpty { row.append(field); rows.append(row) }
        return rows
    }

    // Слабый мастер-пароль не запрещаем, а предупреждаем: файл лежит на GitHub, его можно перебирать.
    public static func isWeak(_ password: String) -> Bool {
        let kinds = [password.contains(where: \.isLowercase), password.contains(where: \.isUppercase),
                     password.contains(where: \.isNumber), password.contains(where: { !$0.isLetter && !$0.isNumber })]
        return password.count < 12 || kinds.filter { $0 }.count < 3
    }

    // Новый пароль: 20 знаков без похожих (0/O, 1/l), с цифрами и знаками.
    public static func generate(length: Int = 20) -> String {
        let chars = Array("abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789-_!@#%")
        return String((0..<length).map { _ in chars[Int.random(in: 0..<chars.count)] })
    }
}

// Коды 2FA по RFC 6238: HMAC-SHA1, 6 цифр, 30 секунд — как у Google Authenticator.
public enum TOTP {
    public static func code(secret: String, date: Date = Date(), period: TimeInterval = 30, digits: Int = 6) -> String? {
        guard let key = base32(secret), !key.isEmpty else { return nil }
        var counter = UInt64(date.timeIntervalSince1970 / period).bigEndian
        let message = Data(bytes: &counter, count: 8)
        let mac = Data(HMAC<Insecure.SHA1>.authenticationCode(for: message, using: SymmetricKey(data: key)))
        let offset = Int(mac[mac.count - 1] & 0x0f)
        let value = mac[offset..<offset + 4].reduce(0) { ($0 << 8) | UInt32($1) } & 0x7fff_ffff
        let mod = (0..<digits).reduce(UInt32(1)) { r, _ in r * 10 }
        return String(format: "%0\(digits)d", value % mod)
    }

    // Сколько секунд осталось до смены кода.
    public static func remaining(date: Date = Date(), period: TimeInterval = 30) -> Int {
        Int(period - date.timeIntervalSince1970.truncatingRemainder(dividingBy: period))
    }

    // Секрет из otpauth://…?secret=… или просто base32 с пробелами.
    public static func base32(_ text: String) -> Data? {
        var raw = text
        if let range = text.range(of: "secret=") {
            raw = String(text[range.upperBound...].prefix { $0 != "&" })
        }
        let alphabet = Array("ABCDEFGHIJKLMNOPQRSTUVWXYZ234567")
        var bits = 0, value = 0
        var out = Data()
        for char in raw.uppercased() where char != " " && char != "=" && char != "-" {
            guard let index = alphabet.firstIndex(of: char) else { return nil }
            value = (value << 5) | index
            bits += 5
            if bits >= 8 { out.append(UInt8((value >> (bits - 8)) & 0xff)); bits -= 8 }
        }
        return out
    }
}
