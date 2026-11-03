import Foundation
import Testing
@testable import NotesCore

@Suite("сейф паролей")
struct PasswordSafeTests {
    @Test("шифр открывается своим паролем, чужим — нет; текста в файле не видно")
    func roundTrip() throws {
        let items = [SafeItem(kind: .login, title: "почта", username: "makar", secret: "тайна-123")]
        let data = try PasswordSafe.seal(items, password: "мастер")
        #expect(try PasswordSafe.open(data, password: "мастер") == items)
        #expect(throws: PasswordSafe.Failure.wrongPassword) { try PasswordSafe.open(data, password: "не тот") }
        #expect(data.range(of: Data("тайна".utf8)) == nil && data.range(of: Data("makar".utf8)) == nil)
    }

    @Test("испорченный файл — ошибка, а не мусор")
    func damaged() throws {
        var data = try PasswordSafe.seal([], password: "x")
        data[data.count - 1] ^= 1
        #expect(throws: PasswordSafe.Failure.wrongPassword) { try PasswordSafe.open(data, password: "x") }
        #expect(throws: PasswordSafe.Failure.damaged) { try PasswordSafe.open(Data("мусор".utf8), password: "x") }
    }

    // Векторы RFC 6238 (SHA1, ключ «12345678901234567890»), обрезанные до 6 цифр.
    @Test("TOTP по RFC 6238")
    func totp() {
        let secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"
        #expect(TOTP.code(secret: secret, date: Date(timeIntervalSince1970: 59)) == "287082")
        #expect(TOTP.code(secret: secret, date: Date(timeIntervalSince1970: 1_111_111_109)) == "081804")
        #expect(TOTP.code(secret: "otpauth://totp/x?secret=\(secret)&issuer=y", date: Date(timeIntervalSince1970: 59)) == "287082")
        #expect(TOTP.code(secret: "не base32") == nil)
    }
}
