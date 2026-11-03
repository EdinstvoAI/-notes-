import CryptoKit
import Foundation
import LocalAuthentication
import NotesCore

// Ключи сейфа на этом Mac (03.11.2026, «Touch ID — чтобы было безопасно, без подписки Apple»).
// Ключи зашифрованы ключом из Secure Enclave, а им можно воспользоваться только после Touch ID или
// пароля Mac — это проверяет сам чип, не приложение. Файл с шифром — вне хранилища, на GitHub не уходит;
// на другом Mac он бесполезен. Платной программы Apple не нужно: CryptoKit, без Связки ключей.
nonisolated struct SafeKeys: Codable {
    var safe: String
    var codes: String

    // Для другого Mac — оба ключа одной строкой через пробел. Свой мастер-пароль — тот же для обоих файлов.
    var transfer: String { safe == codes ? safe : safe + " " + codes }

    static func from(_ text: String) -> [SafeKeys] {
        let parts = text.split(separator: " ").map(String.init)
        var options = [SafeKeys(safe: text, codes: text)]
        if parts.count == 2 { options.append(SafeKeys(safe: parts[0], codes: parts[1])) }
        return options
    }

    private struct Sealed: Codable { let enclave: Data; let ephemeral: Data; let box: Data }

    private static func url(_ vault: URL) -> URL {
        let name = SHA256.hash(data: Data(vault.path.utf8)).prefix(8).map { String(format: "%02x", $0) }.joined()
        return AppSupport.folder.appendingPathComponent("safe-\(name).key")
    }

    // Сейф живёт только на этом Mac (03.11.2026, «на GitHub вообще не должны уходить ни коды, ни пароли»):
    // папка в Application Support, не в хранилище. Сейф прошлой беты переезжает сюда из `.safe` хранилища.
    static func home(_ vault: URL) -> URL {
        let name = SHA256.hash(data: Data(vault.path.utf8)).prefix(8).map { String(format: "%02x", $0) }.joined()
        let home = AppSupport.folder.appendingPathComponent("safe-\(name)")
        let old = vault.appendingPathComponent(".safe"), new = home.appendingPathComponent(".safe")
        let files = FileManager.default
        if files.fileExists(atPath: old.path), !files.fileExists(atPath: new.path) {
            try? files.createDirectory(at: home, withIntermediateDirectories: true)
            try? files.moveItem(at: old, to: new)
        }
        return home
    }

    static func stored(_ vault: URL) -> Bool { FileManager.default.fileExists(atPath: url(vault).path) }

    static func forget(_ vault: URL) { try? FileManager.default.removeItem(at: url(vault)) }

    // Запомнить на этом Mac. Ключ чипа создаётся с условием «только после Touch ID или пароля Mac».
    static func store(_ keys: SafeKeys, vault: URL) throws {
        guard SecureEnclave.isAvailable else { throw CocoaError(.featureUnsupported) }
        var error: Unmanaged<CFError>?
        guard let access = SecAccessControlCreateWithFlags(nil, kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
                                                           [.privateKeyUsage, .userPresence], &error) else {
            throw error!.takeRetainedValue() as Error
        }
        let enclave = try SecureEnclave.P256.KeyAgreement.PrivateKey(accessControl: access)
        let ephemeral = P256.KeyAgreement.PrivateKey()
        let box = try AES.GCM.seal(try JSONEncoder().encode(keys),
                                   using: symmetric(try ephemeral.sharedSecretFromKeyAgreement(with: enclave.publicKey)))
        let sealed = Sealed(enclave: enclave.dataRepresentation, ephemeral: ephemeral.publicKey.rawRepresentation, box: box.combined!)
        try FileManager.default.createDirectory(at: AppSupport.folder, withIntermediateDirectories: true)
        try JSONEncoder().encode(sealed).write(to: url(vault), options: [.atomic, .completeFileProtection])
    }

    // Достать ключи: чип сам спросит Touch ID или пароль Mac.
    static func load(_ vault: URL, reason: String) async throws -> SafeKeys {
        let sealed = try JSONDecoder().decode(Sealed.self, from: Data(contentsOf: url(vault)))
        let context = LAContext()
        context.localizedReason = reason
        _ = try await context.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: reason)
        let enclave = try SecureEnclave.P256.KeyAgreement.PrivateKey(dataRepresentation: sealed.enclave, authenticationContext: context)
        let secret = try enclave.sharedSecretFromKeyAgreement(with: P256.KeyAgreement.PublicKey(rawRepresentation: sealed.ephemeral))
        let json = try AES.GCM.open(AES.GCM.SealedBox(combined: sealed.box), using: symmetric(secret))
        return try JSONDecoder().decode(SafeKeys.self, from: json)
    }

    private static func symmetric(_ secret: SharedSecret) -> SymmetricKey {
        secret.hkdfDerivedSymmetricKey(using: SHA256.self, salt: Data("notes.safe".utf8), sharedInfo: Data(), outputByteCount: 32)
    }
}
