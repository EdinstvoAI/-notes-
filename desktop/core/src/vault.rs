//! Хранилище — обычная папка с `.md`. Как `NoteLibrary` на маке
//! (`NotesCore/Sources/NotesCore/NoteLibrary.swift`): скрытое (`.git`, служебные папки других программ)
//! не показываем и не трогаем, порядок — по имени без учёта регистра.

use serde::Serialize;
use std::cmp::Ordering;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::names::{compare_key, windows_problem};
use crate::tasks::{tasks_in, Task};

/// Файлы, которые показываем просмотром (как `NoteLibrary.viewable`).
pub const VIEWABLE: &[&str] = &[
    "pdf", "doc", "docx", "rtf", "pages", "key", "numbers", "xls", "xlsx", "ppt", "pptx", "txt", "csv",
    "png", "jpg", "jpeg", "heic", "gif", "webp", "tiff", "svg", "mp4", "mov", "m4v", "mp3", "m4a", "wav",
    "aiff", "flac",
];

#[derive(Debug)]
pub enum VaultError {
    Io(io::Error),
    /// Путь выходит за пределы хранилища (`..`, абсолютный путь).
    OutsideVault,
    /// Имя не создать на этой ОС — показать человеку, файл не трогать.
    BadName(&'static str),
    NotFound,
    /// Строку задачи поменяли снаружи с тех пор, как её прочитали.
    ChangedOnDisk,
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultError::Io(e) => write!(f, "{e}"),
            VaultError::OutsideVault => write!(f, "path is outside the vault"),
            VaultError::BadName(why) => write!(f, "bad file name: {why}"),
            VaultError::NotFound => write!(f, "not found"),
            VaultError::ChangedOnDisk => write!(f, "the note changed on disk"),
        }
    }
}

impl std::error::Error for VaultError {}

impl From<io::Error> for VaultError {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::NotFound { VaultError::NotFound } else { VaultError::Io(e) }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Note {
    /// Путь от корня хранилища через `/` — одинаковый на обеих ОС.
    pub path: String,
    pub title: String,
    /// Секунды с 1970 года.
    pub modified: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Entry {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Folder {
    pub path: String,
    pub name: String,
    pub folders: Vec<Folder>,
    pub notes: Vec<Note>,
    pub files: Vec<Entry>,
    /// Заметок внутри вместе с вложенными папками.
    pub note_count: usize,
    /// Файлы, которые на этой ОС не открыть (имя с мака) — показать человеку списком.
    pub unreadable: Vec<Entry>,
}

#[derive(Debug, Clone)]
pub struct Vault {
    root: PathBuf,
}

impl Vault {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, VaultError> {
        let root = root.into();
        if !root.is_dir() {
            return Err(VaultError::NotFound);
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn tree(&self) -> Folder {
        self.folder(&self.root, "")
    }

    fn folder(&self, dir: &Path, rel: &str) -> Folder {
        let mut folders = Vec::new();
        let mut notes = Vec::new();
        let mut files = Vec::new();
        let mut unreadable = Vec::new();
        for item in fs::read_dir(dir).into_iter().flatten().flatten() {
            let name = item.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = join(rel, &name);
            if cfg!(windows) {
                if let Some(_why) = windows_problem(&name) {
                    unreadable.push(Entry { path, name });
                    continue;
                }
            }
            let Ok(meta) = item.metadata() else { continue };
            if meta.is_dir() {
                folders.push(self.folder(&item.path(), &path));
                continue;
            }
            let ext = Path::new(&name).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if ext == "md" {
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs());
                let title = name[..name.len() - 3].to_string();
                notes.push(Note { path, title, modified });
            } else if VIEWABLE.contains(&ext.as_str()) {
                files.push(Entry { path, name });
            }
        }
        folders.sort_by(|a, b| by_name(&a.name, &b.name));
        notes.sort_by(|a, b| by_name(&a.title, &b.title));
        files.sort_by(|a, b| by_name(&a.name, &b.name));
        let note_count = notes.len() + folders.iter().map(|f| f.note_count).sum::<usize>();
        let name = if rel.is_empty() {
            self.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
        } else {
            rel.rsplit('/').next().unwrap_or(rel).to_string()
        };
        Folder { path: rel.to_string(), name, folders, notes, files, note_count, unreadable }
    }

    /// Абсолютный путь внутри хранилища. `..` и абсолютные пути не пускаем.
    pub fn resolve(&self, rel: &str) -> Result<PathBuf, VaultError> {
        let mut out = self.root.clone();
        for part in Path::new(rel).components() {
            match part {
                Component::Normal(p) => out.push(p),
                Component::CurDir => {}
                _ => return Err(VaultError::OutsideVault),
            }
        }
        Ok(out)
    }

    /// Текст заметки. Концы строк — как в файле.
    pub fn read(&self, rel: &str) -> Result<String, VaultError> {
        Ok(fs::read_to_string(self.resolve(rel)?)?)
    }

    /// Записать заметку, сохранив концы строк файла: пришёл с LF — останется LF, даже если
    /// поле ввода на Windows вернуло CRLF. Пишем через временный файл, чтобы сбой не оставил полфайла.
    pub fn write(&self, rel: &str, text: &str) -> Result<(), VaultError> {
        let path = self.resolve(rel)?;
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if let Some(why) = windows_problem(name) {
                return Err(VaultError::BadName(why));
            }
        }
        let crlf = match fs::read(&path) {
            Ok(old) => old.windows(2).any(|w| w == b"\r\n"),
            Err(_) => false,
        };
        let lf = text.replace("\r\n", "\n");
        let body = if crlf { lf.replace('\n', "\r\n") } else { lf };
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("md.tmp-notes");
        fs::write(&tmp, body)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Новая заметка в папке: «Новая заметка», «Новая заметка 2»… Возвращает путь.
    pub fn create_note(&self, folder: &str, title: &str) -> Result<String, VaultError> {
        if let Some(why) = windows_problem(title) {
            return Err(VaultError::BadName(why));
        }
        let dir = self.resolve(folder)?;
        let taken: Vec<String> = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| compare_key(&e.file_name().to_string_lossy()))
            .collect();
        let mut n = 1;
        loop {
            let name = if n == 1 { format!("{title}.md") } else { format!("{title} {n}.md") };
            if !taken.contains(&compare_key(&name)) {
                let rel = join(folder, &name);
                self.write(&rel, "")?;
                return Ok(rel);
            }
            n += 1;
        }
    }

    /// Переименовать заметку заголовком. Имя занято — добавляем номер, как Finder. Возвращает новый путь.
    pub fn rename(&self, rel: &str, title: &str) -> Result<String, VaultError> {
        let title = title.trim();
        if let Some(why) = windows_problem(title) {
            return Err(VaultError::BadName(why));
        }
        let from = self.resolve(rel)?;
        if !from.is_file() {
            return Err(VaultError::NotFound);
        }
        let folder = rel.rsplit_once('/').map_or("", |(f, _)| f);
        let current = rel.rsplit('/').next().unwrap_or(rel);
        let dir = self.resolve(folder)?;
        let taken: Vec<String> = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| compare_key(n) != compare_key(current))
            .map(|n| compare_key(&n))
            .collect();
        let mut n = 1;
        let name = loop {
            let name = if n == 1 { format!("{title}.md") } else { format!("{title} {n}.md") };
            if !taken.contains(&compare_key(&name)) {
                break name;
            }
            n += 1;
        };
        if name == current {
            return Ok(rel.to_string());
        }
        let to = dir.join(&name);
        fs::rename(&from, &to)?;
        Ok(join(folder, &name))
    }

    /// Новая папка: занятое имя — «2», «3». Возвращает путь.
    pub fn create_folder(&self, parent: &str, name: &str) -> Result<String, VaultError> {
        let name = name.trim();
        if let Some(why) = windows_problem(name) {
            return Err(VaultError::BadName(why));
        }
        let dir = self.resolve(parent)?;
        let name = free_name(&dir, name, None, |n| format!(" {n}"), 2);
        fs::create_dir_all(dir.join(&name))?;
        Ok(join(parent, &name))
    }

    /// Дубль рядом: «план копия.md», «план копия 2.md» — как на маке.
    pub fn duplicate(&self, rel: &str) -> Result<String, VaultError> {
        let from = self.resolve(rel)?;
        let folder = parent_of(rel);
        let (base, ext) = split_ext(rel.rsplit('/').next().unwrap_or(rel), from.is_file());
        let dir = self.resolve(folder)?;
        let name = free_name(&dir, &format!("{base} копия"), ext, |n| format!(" {n}"), 1);
        copy_any(&from, &dir.join(&name))?;
        Ok(join(folder, &name))
    }

    /// Перенести заметку или папку в другую папку хранилища. В себя и в свою подпапку — нельзя
    /// (`None`), в ту же папку — ничего не делаем. Занятое имя — «2», «3».
    pub fn move_into(&self, rel: &str, folder: &str) -> Result<Option<String>, VaultError> {
        let from = self.resolve(rel)?;
        if !from.exists() {
            return Err(VaultError::NotFound);
        }
        if parent_of(rel) == folder || folder == rel || folder.starts_with(&format!("{rel}/")) {
            return Ok(None);
        }
        let dir = self.resolve(folder)?;
        let current = rel.rsplit('/').next().unwrap_or(rel);
        let (base, ext) = split_ext(current, from.is_file());
        let name = if dir.join(current).exists() { free_name(&dir, base, ext, |n| format!(" {n}"), 2) } else { current.to_string() };
        fs::rename(&from, dir.join(&name))?;
        Ok(Some(join(folder, &name)))
    }

    /// Скопировать внутрь файл снаружи хранилища (перетащили из Проводника или Finder).
    pub fn copy_in(&self, outside: &Path, folder: &str) -> Result<String, VaultError> {
        let dir = self.resolve(folder)?;
        let file = outside.file_name().and_then(|n| n.to_str()).ok_or(VaultError::NotFound)?.to_string();
        let (base, ext) = split_ext(&file, outside.is_file());
        let name = if dir.join(&file).exists() { free_name(&dir, &format!("{base} копия"), ext, |n| format!(" {n}"), 1) } else { file };
        copy_any(outside, &dir.join(&name))?;
        Ok(join(folder, &name))
    }

    /// Строка задачи `line` в заметке — только если она всё ещё `raw`: файл могли поменять
    /// снаружи, и тогда чужую правку не трогаем (как `changedOnDisk` на маке).
    fn edit_task_line(&self, rel: &str, line: usize, raw: &str, edit: impl FnOnce(&str) -> String) -> Result<(), VaultError> {
        let text = self.read(rel)?;
        let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
        let current = lines.get(line).map(|l| l.trim_end_matches('\r')).ok_or(VaultError::ChangedOnDisk)?;
        if current != raw.trim_end_matches('\r') {
            return Err(VaultError::ChangedOnDisk);
        }
        lines[line] = edit(current);
        self.write(rel, &lines.join("\n"))
    }

    /// Новая дата задачи: старую `📅 …` срезаем, пишем новую. `date: None` — убрать дату.
    pub fn set_due(&self, rel: &str, line: usize, raw: &str, date: Option<&str>, time: Option<&str>, minutes: Option<u32>) -> Result<(), VaultError> {
        self.edit_task_line(rel, line, raw, |l| {
            let head = l.split("📅").next().unwrap_or(l).trim_end();
            format!("{head}{}", crate::tasks::date_text(date, time, minutes))
        })
    }

    /// Новое название задачи — дата, «!!» и отметка остаются как были.
    pub fn set_task_title(&self, rel: &str, line: usize, raw: &str, title: &str) -> Result<(), VaultError> {
        let title = title.trim().to_string();
        self.edit_task_line(rel, line, raw, |l| {
            let head_end = l.find(']').map_or(0, |i| i + 1);
            let tail = l.find("📅").map(|i| format!(" {}", &l[i..])).unwrap_or_default();
            let urgent = if l[head_end..].contains("!!") { " !!" } else { "" };
            format!("{} {title}{urgent}{tail}", &l[..head_end])
        })
    }

    /// Удалить строку задачи из заметки (карточка события → «удалить»).
    pub fn delete_task(&self, rel: &str, line: usize, raw: &str) -> Result<(), VaultError> {
        let text = self.read(rel)?;
        let mut lines: Vec<&str> = text.split('\n').collect();
        if lines.get(line).map(|l| l.trim_end_matches('\r')) != Some(raw.trim_end_matches('\r')) {
            return Err(VaultError::ChangedOnDisk);
        }
        lines.remove(line);
        self.write(rel, &lines.join("\n"))
    }

    /// Отметить задачу (не)сделанной — правим только квадратик.
    pub fn set_done(&self, rel: &str, line: usize, raw: &str, done: bool) -> Result<(), VaultError> {
        self.edit_task_line(rel, line, raw, |l| match l.find('[') {
            Some(i) => format!("{}[{}]{}", &l[..i], if done { 'x' } else { ' ' }, &l[i + 3..]),
            None => l.to_string(),
        })
    }

    /// Дописать задачу в конец заметки (по умолчанию — «Входящие.md», как на маке). Нет файла — создаём.
    pub fn append_task(&self, rel: &str, title: &str, date: Option<&str>, time: Option<&str>, minutes: Option<u32>) -> Result<(), VaultError> {
        let title = title.trim();
        if title.is_empty() {
            return Ok(());
        }
        let mut text = self.read(rel).unwrap_or_default();
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&format!("- [ ] {title}{}\n", crate::tasks::date_text(date, time, minutes)));
        self.write(rel, &text)
    }

    /// Задачи из всех заметок: (путь заметки, задача).
    pub fn tasks(&self) -> Vec<(String, Task)> {
        let mut out = Vec::new();
        self.collect_tasks(&self.tree(), &mut out);
        out
    }

    fn collect_tasks(&self, folder: &Folder, out: &mut Vec<(String, Task)>) {
        for note in &folder.notes {
            if let Ok(text) = self.read(&note.path) {
                out.extend(tasks_in(&text).into_iter().map(|t| (note.path.clone(), t)));
            }
        }
        for child in &folder.folders {
            self.collect_tasks(child, out);
        }
    }
}

fn parent_of(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(f, _)| f)
}

/// «план.md» → («план», Some("md")); у папки расширения нет.
fn split_ext(name: &str, is_file: bool) -> (&str, Option<&str>) {
    match name.rsplit_once('.') {
        Some((base, ext)) if is_file && !base.is_empty() => (base, Some(ext)),
        _ => (name, None),
    }
}

/// Первое свободное имя: `base`, потом `base` + suffix(n) начиная с `start` (1 — сначала без номера).
fn free_name(dir: &Path, base: &str, ext: Option<&str>, suffix: impl Fn(u32) -> String, start: u32) -> String {
    let taken: Vec<String> = fs::read_dir(dir).into_iter().flatten().flatten()
        .map(|e| compare_key(&e.file_name().to_string_lossy())).collect();
    let make = |b: String| match ext { Some(e) => format!("{b}.{e}"), None => b };
    let first = make(base.to_string());
    if !taken.contains(&compare_key(&first)) {
        return first;
    }
    let mut n = start.max(2);
    loop {
        let name = make(format!("{base}{}", suffix(n)));
        if !taken.contains(&compare_key(&name)) {
            return name;
        }
        n += 1;
    }
}

fn copy_any(from: &Path, to: &Path) -> io::Result<()> {
    if from.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_any(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(from, to).map(|_| ())
    }
}

fn join(rel: &str, name: &str) -> String {
    if rel.is_empty() { name.to_string() } else { format!("{rel}/{name}") }
}

/// По имени без учёта регистра и формы Юникода, цифры — по значению («2» раньше «10»), как в Finder.
fn by_name(a: &str, b: &str) -> Ordering {
    natural(&compare_key(a), &compare_key(b))
}

fn natural(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let n: String = std::iter::from_fn(|| x.next_if(|c| c.is_ascii_digit())).collect();
                let m: String = std::iter::from_fn(|| y.next_if(|c| c.is_ascii_digit())).collect();
                let (n, m) = (n.trim_start_matches('0'), m.trim_start_matches('0'));
                match n.len().cmp(&m.len()).then_with(|| n.cmp(m)) {
                    Ordering::Equal => {}
                    o => return o,
                }
            }
            (Some(c), Some(d)) => {
                if c != d {
                    return c.cmp(&d);
                }
                x.next();
                y.next();
            }
        }
    }
}
