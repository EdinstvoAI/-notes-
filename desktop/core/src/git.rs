//! История хранилища в git — как `GitVault.swift` на маке: репозиторий в папке заметок, ветка `main`,
//! автор `notes <notes@local>` в конфиге самого репозитория (глобальный конфиг git не трогаем),
//! `core.autocrlf=false`, чтобы Windows не превращал LF в CRLF и мак не видел «правки» во всех файлах.
//! Коммит — всё изменённое, новое и удалённое; пустых коммитов не делаем.

use std::path::Path;

use git2::build::CheckoutBuilder;
use git2::{Cred, FetchOptions, IndexAddOption, IndexEntry, PushOptions, RemoteCallbacks, Repository, RepositoryInitOptions, Signature, Status, StatusOptions};
use serde::Serialize;

pub const BRANCH: &str = "main";
pub const AUTHOR: &str = "notes";
pub const EMAIL: &str = "notes@local";
pub const INITIAL: &str = "начало истории";

/// Что не попадает в историю: служебное ОС, корзина, тяжёлые медиа. Мак пишет то же, плюс мусор Windows.
pub const GITIGNORE: &str = "# создано <notes>\n.DS_Store\nThumbs.db\ndesktop.ini\n.trash/\n*.mp4\n*.mov\n*.m4a\n*.mp3\n*.wav\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    pub path: String,
    pub kind: Kind,
}

pub struct History {
    repo: Repository,
}

pub fn is_repository(folder: &Path) -> bool {
    folder.join(".git").exists()
}

impl History {
    /// Открыть репозиторий в папке или создать новый с первым коммитом. `true` — создан сейчас.
    /// Папка уже была репозиторием — его историю и `.gitignore` не трогаем, только `autocrlf`.
    pub fn prepare(folder: &Path) -> Result<(Self, bool), git2::Error> {
        if is_repository(folder) {
            let repo = Repository::open(folder)?;
            repo.config()?.set_bool("core.autocrlf", false)?;
            return Ok((Self { repo }, false));
        }
        let mut options = RepositoryInitOptions::new();
        options.initial_head(BRANCH).mkpath(true);
        let repo = Repository::init_opts(folder, &options)?;
        {
            let mut config = repo.config()?;
            config.set_str("user.name", AUTHOR)?;
            config.set_str("user.email", EMAIL)?;
            config.set_bool("core.autocrlf", false)?;
        }
        let ignore = folder.join(".gitignore");
        if !ignore.exists() {
            std::fs::write(&ignore, GITIGNORE).map_err(|e| git2::Error::from_str(&e.to_string()))?;
        }
        let history = Self { repo };
        history.commit_all(INITIAL)?;
        Ok((history, true))
    }

    pub fn open(folder: &Path) -> Result<Self, git2::Error> {
        Ok(Self { repo: Repository::open(folder)? })
    }

    /// Новые, изменённые и удалённые файлы — без игнорируемых. Для подписи коммита.
    pub fn changes(&self) -> Result<Vec<Change>, git2::Error> {
        let mut options = StatusOptions::new();
        options.include_untracked(true).recurse_untracked_dirs(true).include_ignored(false);
        let mut out: Vec<Change> = self
            .repo
            .statuses(Some(&mut options))?
            .iter()
            .filter_map(|e| {
                let s = e.status();
                let kind = if s.intersects(Status::WT_NEW | Status::INDEX_NEW) {
                    Kind::Added
                } else if s.intersects(Status::WT_DELETED | Status::INDEX_DELETED) {
                    Kind::Deleted
                } else if s.intersects(Status::WT_MODIFIED | Status::INDEX_MODIFIED | Status::WT_RENAMED | Status::INDEX_RENAMED | Status::WT_TYPECHANGE) {
                    Kind::Modified
                } else {
                    return None;
                };
                Some(Change { path: e.path()?.to_string(), kind })
            })
            .collect();
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    /// Как `git add -A && git commit`. `false` — менять было нечего.
    pub fn commit_all(&self, message: &str) -> Result<bool, git2::Error> {
        let mut index = self.repo.index()?;
        index.add_all(["*"], IndexAddOption::DEFAULT, None)?;
        index.update_all(["*"], None)?;
        index.write()?;
        let tree_id = index.write_tree()?;
        let parent = self.repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        if parent.as_ref().is_some_and(|p| p.tree_id() == tree_id) {
            return Ok(false);
        }
        let tree = self.repo.find_tree(tree_id)?;
        let me = Signature::now(AUTHOR, EMAIL)?;
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        self.repo.commit(Some("HEAD"), &me, &me, message, &tree, &parents)?;
        Ok(true)
    }

    /// Закоммитить, если есть что, с подписью по изменениям. Возвращает подпись коммита.
    pub fn commit_now(&self) -> Result<Option<String>, git2::Error> {
        let changes = self.changes()?;
        if changes.is_empty() {
            return Ok(None);
        }
        let message = message(&changes);
        Ok(self.commit_all(&message)?.then_some(message))
    }

    /// Время последнего коммита, секунды с 1970.
    pub fn last_commit(&self) -> Option<i64> {
        Some(self.repo.head().ok()?.peel_to_commit().ok()?.time().seconds())
    }

    pub fn commit_count(&self) -> usize {
        let Ok(mut walk) = self.repo.revwalk() else { return 0 };
        if walk.push_head().is_err() {
            return 0;
        }
        walk.count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Pull {
    UpToDate,
    FastForward,
    /// conflicts — заметки, которые правили на обоих компьютерах по-разному; рядом — копия
    /// «имя (конфликт дата).md» с другого компьютера.
    Merged { conflicts: Vec<String> },
}

/// Токен GitHub уходит паролем по HTTPS и живёт только на время вызова: в URL репозитория его не
/// пишем, иначе он оказался бы в `.git/config` внутри хранилища.
fn callbacks(token: Option<&str>) -> RemoteCallbacks<'_> {
    let mut cb = RemoteCallbacks::new();
    if let Some(token) = token {
        cb.credentials(move |_, _, _| Cred::userpass_plaintext("x-access-token", token));
    }
    cb
}

impl History {
    pub fn set_remote(&self, url: &str) -> Result<(), git2::Error> {
        if self.repo.find_remote("origin").is_ok() {
            self.repo.remote_set_url("origin", url)
        } else {
            self.repo.remote("origin", url).map(|_| ())
        }
    }

    pub fn remote_url(&self) -> Option<String> {
        self.repo.find_remote("origin").ok()?.url().map(str::to_string)
    }

    /// Есть ли коммиты, которых нет на сервере.
    pub fn has_unpushed(&self) -> bool {
        let Some(local) = self.repo.head().ok().and_then(|h| h.target()) else { return false };
        match self.repo.refname_to_id(&format!("refs/remotes/origin/{BRANCH}")) {
            Ok(remote) => remote != local,
            Err(_) => true,
        }
    }

    pub fn push(&self, token: Option<&str>) -> Result<(), git2::Error> {
        let mut remote = self.repo.find_remote("origin")?;
        let mut options = PushOptions::new();
        options.remote_callbacks(callbacks(token));
        let spec = format!("refs/heads/{BRANCH}:refs/heads/{BRANCH}");
        remote.push(&[spec.as_str()], Some(&mut options))?;
        // Копия ветки сервера — как после `git push`: по ней видно, что отправлено.
        if let Some(id) = self.repo.head()?.target() {
            self.repo.reference(&format!("refs/remotes/origin/{BRANCH}"), id, true, "push")?;
        }
        Ok(())
    }

    pub fn fetch(&self, token: Option<&str>) -> Result<(), git2::Error> {
        let mut remote = self.repo.find_remote("origin")?;
        let mut options = FetchOptions::new();
        options.remote_callbacks(callbacks(token));
        remote.fetch(&[format!("refs/heads/{BRANCH}:refs/remotes/origin/{BRANCH}")], Some(&mut options), None)
    }

    /// Забрать правки с сервера и влить в текущую ветку. Главное правило, как на маке: текст не
    /// теряется никогда. Одну заметку поменяли в двух местах — в файле остаётся здешняя версия, а
    /// версия с другого компьютера ложится рядом копией. Удаление против правки — побеждает правка.
    /// `date` — `2026-11-01`, для имени копии. Перед вызовом всё здешнее коммитим сами.
    pub fn pull(&self, token: Option<&str>, date: &str) -> Result<Pull, git2::Error> {
        self.commit_now()?;
        self.fetch(token)?;
        let Ok(their_id) = self.repo.refname_to_id(&format!("refs/remotes/origin/{BRANCH}")) else { return Ok(Pull::UpToDate) };
        let theirs = self.repo.find_annotated_commit(their_id)?;
        let (analysis, _) = self.repo.merge_analysis(&[&theirs])?;
        if analysis.is_up_to_date() {
            return Ok(Pull::UpToDate);
        }
        if analysis.is_fast_forward() || analysis.is_unborn() {
            let refname = format!("refs/heads/{BRANCH}");
            match self.repo.find_reference(&refname) {
                Ok(mut r) => { r.set_target(their_id, "pull: fast-forward")?; }
                Err(_) => { self.repo.reference(&refname, their_id, true, "pull")?; }
            }
            self.repo.set_head(&refname)?;
            self.repo.checkout_head(Some(CheckoutBuilder::new().safe()))?;
            return Ok(Pull::FastForward);
        }
        let ours = self.repo.head()?.peel_to_commit()?;
        let theirs_commit = self.repo.find_commit(their_id)?;
        let mut index = self.repo.merge_commits(&ours, &theirs_commit, None)?;
        let conflicts = resolve_conflicts(&mut index, date)?;
        let tree_id = index.write_tree_to(&self.repo)?;
        let tree = self.repo.find_tree(tree_id)?;
        let me = Signature::now(AUTHOR, EMAIL)?;
        let message = if conflicts.is_empty() {
            "слияние с другого компьютера".to_string()
        } else {
            let names: Vec<&str> = conflicts.iter().map(|p| p.rsplit('/').next().unwrap_or(p)).collect();
            format!("слияние с другого компьютера, конфликты: {}", names.join(", "))
        };
        self.repo.commit(Some("HEAD"), &me, &me, &message, &tree, &[&ours, &theirs_commit])?;
        // Обновить файлы в папке под новый коммит; всё своё уже закоммичено выше.
        self.repo.checkout_head(Some(CheckoutBuilder::new().force()))?;
        Ok(Pull::Merged { conflicts })
    }
}

fn resolve_conflicts(index: &mut git2::Index, date: &str) -> Result<Vec<String>, git2::Error> {
    if !index.has_conflicts() {
        return Ok(Vec::new());
    }
    let found: Vec<(Option<IndexEntry>, Option<IndexEntry>, String)> = index
        .conflicts()?
        .filter_map(Result::ok)
        .filter_map(|c| {
            let any = c.our.as_ref().or(c.their.as_ref()).or(c.ancestor.as_ref())?;
            let path = String::from_utf8_lossy(&any.path).into_owned();
            Some((c.our, c.their, path))
        })
        .collect();
    let mut copies = Vec::new();
    for (ours, theirs, path) in found {
        index.conflict_remove(std::path::Path::new(&path))?;
        let stage0 = |mut e: IndexEntry, at: &str, index: &mut git2::Index| -> Result<(), git2::Error> {
            e.flags &= !0x3000; // из конфликтной стадии в обычную
            e.path = at.as_bytes().to_vec();
            index.add(&e)
        };
        match (ours, theirs) {
            (Some(o), Some(t)) => {
                stage0(o, &path, index)?;
                let copy = conflict_copy_path(&path, date, index);
                stage0(t, &copy, index)?;
                copies.push(path);
            }
            (Some(o), None) => stage0(o, &path, index)?,
            (None, Some(t)) => stage0(t, &path, index)?,
            (None, None) => {}
        }
    }
    Ok(copies)
}

/// «папка/план.md» → «папка/план (конфликт 2026-10-19).md», при повторе — «… 2», «… 3».
pub fn conflict_copy_path(path: &str, date: &str, index: &git2::Index) -> String {
    let (stem, ext) = match path.rsplit_once('.') {
        Some((s, e)) if !s.ends_with('/') && !s.is_empty() => (s, Some(e)),
        _ => (path, None),
    };
    let mut n = 1;
    loop {
        let suffix = if n == 1 { String::new() } else { format!(" {n}") };
        let candidate = match ext {
            Some(e) => format!("{stem} (конфликт {date}{suffix}).{e}"),
            None => format!("{stem} (конфликт {date}{suffix})"),
        };
        if index.get_path(std::path::Path::new(&candidate), 0).is_none() {
            return candidate;
        }
        n += 1;
    }
}

/// Подпись автокоммита, как `CommitMessage` на маке: «новая: идеи.md», «правки: план.md, идеи.md»,
/// «удалены: 5 файлов», «правки: план.md, +1 новая, −2 удалены».
pub fn message(changes: &[Change]) -> String {
    let names = |k: Kind| -> Vec<&str> {
        changes.iter().filter(|c| c.kind == k).map(|c| c.path.rsplit('/').next().unwrap_or(&c.path)).collect()
    };
    let (added, modified, deleted) = (names(Kind::Added), names(Kind::Modified), names(Kind::Deleted));
    for (list, one, many) in [(&modified, "правка", "правки"), (&added, "новая", "новые"), (&deleted, "удалена", "удалены")] {
        if !list.is_empty() && list.len() == changes.len() {
            let prefix = if list.len() == 1 { one } else { many };
            return if list.len() <= 3 {
                format!("{prefix}: {}", list.join(", "))
            } else {
                format!("{prefix}: {} {}", list.len(), plural(list.len(), "файл", "файла", "файлов"))
            };
        }
    }
    if changes.is_empty() {
        return "правка".into();
    }
    let mut parts = Vec::new();
    if !modified.is_empty() {
        parts.push(if modified.len() <= 2 { modified.join(", ") } else { format!("{} {}", modified.len(), plural(modified.len(), "файл", "файла", "файлов")) });
    }
    if !added.is_empty() {
        parts.push(format!("+{} {}", added.len(), plural(added.len(), "новая", "новые", "новых")));
    }
    if !deleted.is_empty() {
        parts.push(format!("−{} {}", deleted.len(), plural(deleted.len(), "удалена", "удалены", "удалено")));
    }
    format!("правки: {}", parts.join(", "))
}

pub fn plural(n: usize, one: &'static str, few: &'static str, many: &'static str) -> &'static str {
    if (11..=14).contains(&(n % 100)) {
        return many;
    }
    match n % 10 {
        1 => one,
        2..=4 => few,
        _ => many,
    }
}

/// Версия для раздела «синхронизация» (порт `GitVault+History.swift`, 03.11.2026). Откат — новый коммит
/// поверх: история не переписывается, на GitHub уходит обычным push.
#[derive(Debug, Clone, Serialize)]
pub struct Version {
    pub id: String,
    pub message: String,
    /// Секунды с 1970.
    pub date: i64,
    pub parents: Vec<String>,
    pub files: Vec<Change>,
}

impl History {
    /// Последние коммиты, свежие сверху, с файлами, которые в них поменялись.
    pub fn history(&self, limit: usize) -> Result<Vec<Version>, git2::Error> {
        let mut walk = self.repo.revwalk()?;
        walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
        if walk.push_head().is_err() {
            return Ok(vec![]);
        }
        let mut out = vec![];
        for oid in walk.take(limit) {
            let c = self.repo.find_commit(oid?)?;
            out.push(Version {
                id: c.id().to_string(),
                message: c.message().unwrap_or("").trim().to_string(),
                date: c.time().seconds(),
                parents: c.parent_ids().map(|p| p.to_string()).collect(),
                files: self.files(&c)?,
            });
        }
        Ok(out)
    }

    fn files(&self, c: &git2::Commit) -> Result<Vec<Change>, git2::Error> {
        let parent = if c.parent_count() > 0 { Some(c.parent(0)?.tree()?) } else { None };
        let diff = self.repo.diff_tree_to_tree(parent.as_ref(), Some(&c.tree()?), None)?;
        Ok(diff.deltas().filter_map(|d| {
            let (kind, file) = match d.status() {
                git2::Delta::Added => (Kind::Added, d.new_file()),
                git2::Delta::Deleted => (Kind::Deleted, d.old_file()),
                _ => (Kind::Modified, d.new_file()),
            };
            file.path().map(|p| Change { path: p.to_string_lossy().replace('\\', "/"), kind })
        }).collect())
    }

    /// Отменить одну версию: её правки уходят, всё, что после, остаётся. Ту же строку меняли позже —
    /// конфликт: ничего не трогаем.
    pub fn revert(&self, id: &str) -> Result<(), git2::Error> {
        self.commit_all("перед откатом")?;
        let c = self.repo.find_commit(git2::Oid::from_str(id)?)?;
        let mut opts = git2::RevertOptions::new();
        if c.parent_count() > 1 {
            opts.mainline(1);
        }
        self.repo.revert(&c, Some(&mut opts))?;
        let conflicted = self.repo.index()?.has_conflicts();
        self.repo.cleanup_state()?;
        if conflicted {
            self.repo.checkout_head(Some(CheckoutBuilder::new().force()))?;
            self.repo.index()?.read(true)?;
            return Err(git2::Error::from_str("эти правки потом меняли ещё раз — отменить отдельно нельзя, откати к версии"));
        }
        self.commit_all(&format!("отменено: {}", c.message().unwrap_or("").trim()))?;
        Ok(())
    }

    /// Всё хранилище как в версии, новым коммитом сверху. Несохранённое — сначала в историю.
    pub fn restore(&self, id: &str, stamp: &str) -> Result<(), git2::Error> {
        self.commit_all("перед откатом")?;
        let c = self.repo.find_commit(git2::Oid::from_str(id)?)?;
        let target = c.tree()?;
        let head = self.repo.head()?.peel_to_tree()?;
        // Файлы, которых в версии не было, checkout_tree сам не удаляет — убираем по диффу.
        let diff = self.repo.diff_tree_to_tree(Some(&head), Some(&target), None)?;
        let root = self.repo.workdir().unwrap().to_path_buf();
        self.repo.checkout_tree(target.as_object(), Some(CheckoutBuilder::new().force()))?;
        for d in diff.deltas().filter(|d| d.status() == git2::Delta::Deleted) {
            if let Some(p) = d.old_file().path() {
                let _ = std::fs::remove_file(root.join(p));
            }
        }
        self.commit_all(&format!("откат к версии от {stamp}"))?;
        Ok(())
    }

    /// Файл, каким он был в версии, байт в байт (LF/CRLF как тогда). None — тогда его не было.
    pub fn file_at(&self, path: &str, id: &str) -> Result<Option<Vec<u8>>, git2::Error> {
        let tree = self.repo.find_commit(git2::Oid::from_str(id)?)?.tree()?;
        let Ok(entry) = tree.get_path(Path::new(path)) else { return Ok(None) };
        Ok(Some(self.repo.find_blob(entry.id())?.content().to_vec()))
    }

    /// Вернуть один файл как в версии; остальное не трогаем. Не было тогда — файл уходит.
    pub fn restore_file(&self, path: &str, id: &str) -> Result<(), git2::Error> {
        self.commit_all("перед откатом")?;
        let file = self.repo.workdir().unwrap().join(path);
        match self.file_at(path, id)? {
            Some(bytes) => {
                if let Some(dir) = file.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| git2::Error::from_str(&e.to_string()))?;
                }
                std::fs::write(&file, bytes).map_err(|e| git2::Error::from_str(&e.to_string()))?;
            }
            None => { let _ = std::fs::remove_file(&file); }
        }
        let name = Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        self.commit_all(&format!("возвращено: {name}"))?;
        Ok(())
    }
}
