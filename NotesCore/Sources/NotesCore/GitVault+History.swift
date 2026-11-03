import Foundation
import libgit2

// История для раздела «синхронизация» (03.11.2026, «изменения туда-сюда откатывать, как git, но графически»).
// Работает и без GitHub: только локальный git в папке. Откат — новый коммит поверх, история не
// переписывается, поэтому отправка на GitHub идёт обычным push, без force.
public struct VaultCommit: Identifiable, Equatable, Sendable {
    public let id: String
    public let message: String
    public let date: Date
    public let files: [FileChange]
}

extension GitVault {
    // Последние коммиты, свежие сверху, с файлами, которые в них поменялись.
    public func history(limit: Int = 100) throws -> [VaultCommit] {
        var walk: OpaquePointer?
        try check(git_revwalk_new(&walk, repo))
        defer { git_revwalk_free(walk) }
        git_revwalk_sorting(walk, GIT_SORT_TOPOLOGICAL.rawValue | GIT_SORT_TIME.rawValue)
        guard git_revwalk_push_head(walk) == 0 else { return [] }   // пустой репозиторий

        var result: [VaultCommit] = []
        var oid = git_oid()
        while result.count < limit, git_revwalk_next(&oid, walk) == 0 {
            var commit: OpaquePointer?
            try check(git_commit_lookup(&commit, repo, &oid))
            defer { git_commit_free(commit) }
            result.append(VaultCommit(id: Self.hex(oid),
                                      message: String(cString: git_commit_message(commit)).trimmingCharacters(in: .whitespacesAndNewlines),
                                      date: Date(timeIntervalSince1970: TimeInterval(git_commit_time(commit))),
                                      files: try files(of: commit)))
        }
        return result
    }

    // Отменить один коммит: его правки уходят, всё, что было после, остаётся («дропнуть коммит»).
    // Если позже ту же строку правили ещё раз — конфликт: ничего не трогаем и говорим об этом.
    public func revert(_ id: String) throws {
        _ = try commitAll(message: "перед откатом")
        let commit = try lookup(id)
        defer { git_commit_free(commit) }
        var options = git_revert_options()
        try check(git_revert_options_init(&options, UInt32(GIT_REVERT_OPTIONS_VERSION)))
        if git_commit_parentcount(commit) > 1 { options.mainline = 1 }
        try check(git_revert(repo, commit, &options))
        defer { git_repository_state_cleanup(repo) }

        var index: OpaquePointer?
        try check(git_repository_index(&index, repo))
        defer { git_index_free(index) }
        if git_index_has_conflicts(index) == 1 {
            try reset()
            throw GitError(code: -1, message: "эти правки потом меняли ещё раз — отменить отдельно нельзя, откати к версии")
        }
        _ = try commitAll(message: "отменено: \(String(cString: git_commit_message(commit)).trimmingCharacters(in: .whitespacesAndNewlines))")
    }

    // Вернуть всё хранилище к версии: файлы как тогда, новым коммитом сверху. Несохранённое — сначала в историю,
    // чтобы откат можно было отменить.
    public func restore(to id: String) throws {
        _ = try commitAll(message: "перед откатом")
        let commit = try lookup(id)
        defer { git_commit_free(commit) }
        var tree: OpaquePointer?
        try check(git_commit_tree(&tree, commit))
        defer { git_tree_free(tree) }
        var options = git_checkout_options()
        try check(git_checkout_options_init(&options, UInt32(GIT_CHECKOUT_OPTIONS_VERSION)))
        options.checkout_strategy = GIT_CHECKOUT_FORCE.rawValue
        try check(git_checkout_tree(repo, tree, &options))
        _ = try commitAll(message: "откат к версии от \(Self.stamp(Date(timeIntervalSince1970: TimeInterval(git_commit_time(commit)))))")
    }

    private func reset() throws {
        var options = git_checkout_options()
        try check(git_checkout_options_init(&options, UInt32(GIT_CHECKOUT_OPTIONS_VERSION)))
        options.checkout_strategy = GIT_CHECKOUT_FORCE.rawValue
        try check(git_checkout_head(repo, &options))
    }

    private func lookup(_ id: String) throws -> OpaquePointer? {
        var oid = git_oid()
        try check(git_oid_fromstr(&oid, id))
        var commit: OpaquePointer?
        try check(git_commit_lookup(&commit, repo, &oid))
        return commit
    }

    private func files(of commit: OpaquePointer?) throws -> [FileChange] {
        var tree: OpaquePointer?, parentTree: OpaquePointer?
        try check(git_commit_tree(&tree, commit))
        defer { git_tree_free(tree) }
        if git_commit_parentcount(commit) > 0 {
            var parent: OpaquePointer?
            try check(git_commit_parent(&parent, commit, 0))
            defer { git_commit_free(parent) }
            try check(git_commit_tree(&parentTree, parent))
        }
        defer { if let parentTree { git_tree_free(parentTree) } }
        var diff: OpaquePointer?
        try check(git_diff_tree_to_tree(&diff, repo, parentTree, tree, nil))
        defer { git_diff_free(diff) }
        return (0..<git_diff_num_deltas(diff)).compactMap { i in
            guard let delta = git_diff_get_delta(diff, i)?.pointee else { return nil }
            let kind: FileChange.Kind = switch delta.status {
            case GIT_DELTA_ADDED: .added
            case GIT_DELTA_DELETED: .deleted
            default: .modified
            }
            let path = delta.status == GIT_DELTA_DELETED ? delta.old_file.path : delta.new_file.path
            return path.map { FileChange(path: String(cString: $0), kind: kind) }
        }
    }

    static func hex(_ oid: git_oid) -> String {
        var oid = oid
        return String(cString: git_oid_tostr_s(&oid))
    }

    static func stamp(_ date: Date) -> String {
        let format = DateFormatter()
        format.locale = Locale(identifier: "ru_RU")
        format.dateFormat = "d MMMM HH:mm"
        return format.string(from: date)
    }
}
