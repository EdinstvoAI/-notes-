//! Оболочка Tauri: команды для интерфейса поверх `notes-core`. Здесь нет логики заметок —
//! она в ядре, — и нет кода под ОС: он в `platform`.

pub mod mcp;
mod safe;
mod intelligence;
mod dictation;
mod github;
mod platform;
mod video;
mod watch;

use notes_core::{Folder, Vault};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};

/// Открытое хранилище. Путь к нему — в настройках приложения (вне хранилища).
#[derive(Default)]
struct AppState {
    /// Модель поиска по смыслу — загружается один раз, при первом поиске.
    embedder: Mutex<Option<notes_embed::Embedder>>,
    vault: Mutex<Option<Vault>>,
    /// Ушли с экрана входа — опрос GitHub прекращается.
    login_cancelled: std::sync::atomic::AtomicBool,
}

#[derive(Serialize)]
struct TaskRow {
    note: String,
    #[serde(flatten)]
    task: notes_core::Task,
}

fn with_vault<T>(state: &State<AppState>, f: impl FnOnce(&Vault) -> Result<T, String>) -> Result<T, String> {
    let guard = state.vault.lock().map_err(|e| e.to_string())?;
    let vault = guard.as_ref().ok_or("no vault open")?;
    f(vault)
}

#[tauri::command]
fn open_vault(path: String, state: State<AppState>, app: tauri::AppHandle) -> Result<Folder, String> {
    let vault = Vault::open(&path).map_err(|e| e.to_string())?;
    let tree = vault.tree();
    watch::watch(app.clone(), vault.root().to_path_buf());
    *state.vault.lock().map_err(|e| e.to_string())? = Some(vault);
    platform::settings::save_vault_path(&app, &path);
    Ok(tree)
}

/// «создать новую» на первом входе: папка «заметки» в Документах (занято — «заметки 2»), сразу открыта.
#[tauri::command]
fn create_vault(state: State<AppState>, app: tauri::AppHandle) -> Result<Folder, String> {
    let docs = app.path().document_dir().map_err(|e| e.to_string())?;
    let mut path = docs.join("заметки");
    let mut n = 2;
    while path.exists() {
        path = docs.join(format!("заметки {n}"));
        n += 1;
    }
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    open_vault(path.to_string_lossy().into_owned(), state, app)
}

/// Хранилище с прошлого запуска, если папка ещё на месте.
#[tauri::command]
fn last_vault(state: State<AppState>, app: tauri::AppHandle) -> Option<Folder> {
    let path = platform::settings::load_vault_path(&app)?;
    let vault = Vault::open(path).ok()?;
    let tree = vault.tree();
    watch::watch(app.clone(), vault.root().to_path_buf());
    *state.vault.lock().ok()? = Some(vault);
    Some(tree)
}

#[tauri::command]
fn tree(state: State<AppState>) -> Result<Folder, String> {
    with_vault(&state, |v| Ok(v.tree()))
}

#[tauri::command]
fn read_note(path: String, state: State<AppState>) -> Result<String, String> {
    with_vault(&state, |v| v.read(&path).map_err(|e| e.to_string()))
}

#[tauri::command]
fn write_note(path: String, text: String, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| v.write(&path, &text).map_err(|e| e.to_string()))
}

#[tauri::command]
fn create_note(folder: String, title: String, state: State<AppState>) -> Result<String, String> {
    with_vault(&state, |v| v.create_note(&folder, &title).map_err(|e| e.to_string()))
}

#[tauri::command]
fn rename_note(path: String, title: String, state: State<AppState>) -> Result<String, String> {
    with_vault(&state, |v| v.rename(&path, &title).map_err(|e| e.to_string()))
}

#[tauri::command]
fn search(query: String, state: State<AppState>) -> Result<Vec<notes_core::Hit>, String> {
    with_vault(&state, |v| Ok(v.search(&query, 20)))
}

// ——— Календарь Apple (EventKit) — `platform/calendar.rs`. На Windows отвечает «нет». ———
fn links_file(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(platform::settings::config_dir(&app)?.join("calendar-links.json"))
}

#[tauri::command]
fn cal_state() -> platform::calendar::State { platform::calendar::state() }

#[tauri::command]
async fn cal_request() -> bool { tauri::async_runtime::spawn_blocking(platform::calendar::request).await.unwrap_or(false) }

#[tauri::command]
fn cal_calendars() -> Vec<platform::calendar::CalendarInfo> { platform::calendar::calendars() }

#[tauri::command]
fn cal_events(start: f64, end: f64) -> Vec<platform::calendar::EventInfo> { platform::calendar::events(start, end) }

#[tauri::command]
fn cal_move(id: String, start: f64, end: f64) -> Result<(), String> { platform::calendar::move_event(&id, start, end) }

#[tauri::command]
fn cal_rename(id: String, title: String) -> Result<(), String> { platform::calendar::rename_event(&id, &title) }

#[tauri::command]
fn cal_remove(id: String) -> Result<(), String> { platform::calendar::remove_event(&id) }

#[tauri::command]
fn cal_create_calendar(title: String) -> Result<(), String> { platform::calendar::create_calendar(&title) }

#[tauri::command]
fn cal_rename_calendar(id: String, title: String) -> Result<(), String> { platform::calendar::rename_calendar(&id, &title) }

/// Ключ задачи → id события: экран задач по ним показывает «в календаре».
#[tauri::command]
fn cal_set_color(id: String, color: String) -> Result<(), String> { platform::calendar::set_color(&id, &color) }
#[tauri::command]
fn cal_remove_calendar(id: String) -> Result<(), String> { platform::calendar::remove_calendar(&id) }

#[tauri::command]
fn cal_links(app: tauri::AppHandle) -> Result<std::collections::BTreeMap<String, String>, String> {
    Ok(platform::calendar::linked(&links_file(&app)?))
}

/// Тестовая папка не трогает настоящий календарь — как `CalendarSync.enabled` на маке (26.10.2026).
#[tauri::command]
fn cal_sync(tasks: Vec<notes_core::calplan::Wanted>, calendar: Option<String>, minutes: u32, app: tauri::AppHandle, state: State<AppState>) -> Result<usize, String> {
    let test = with_vault(&state, |v| Ok(v.root().to_string_lossy().contains("test-vault")))?;
    if test || std::env::var_os("NOTES_NO_CALENDAR").is_some() { return Ok(0); }
    platform::calendar::sync(&tasks, calendar.as_deref(), minutes, &links_file(&app)?)
}

#[tauri::command]
fn tasks(state: State<AppState>) -> Result<Vec<TaskRow>, String> {
    with_vault(&state, |v| Ok(v.tasks().into_iter().map(|(note, task)| TaskRow { note, task }).collect()))
}

#[tauri::command]
fn create_folder(parent: String, name: String, state: State<AppState>) -> Result<String, String> {
    with_vault(&state, |v| v.create_folder(&parent, &name).map_err(|e| e.to_string()))
}

#[tauri::command]
fn duplicate(path: String, state: State<AppState>) -> Result<String, String> {
    with_vault(&state, |v| v.duplicate(&path).map_err(|e| e.to_string()))
}

#[tauri::command]
fn move_into(path: String, folder: String, state: State<AppState>) -> Result<Option<String>, String> {
    with_vault(&state, |v| v.move_into(&path, &folder).map_err(|e| e.to_string()))
}

#[tauri::command]
fn copy_in(outside: String, folder: String, state: State<AppState>) -> Result<String, String> {
    with_vault(&state, |v| v.copy_in(std::path::Path::new(&outside), &folder).map_err(|e| e.to_string()))
}

#[tauri::command]
fn trash(path: String, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| platform::files::trash(&v.resolve(&path).map_err(|e| e.to_string())?))
}

#[tauri::command]
fn untrash(path: String, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| platform::files::untrash(&v.resolve(&path).map_err(|e| e.to_string())?))
}

#[tauri::command]
fn open_default(path: String, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| platform::files::open_default(&v.resolve(&path).map_err(|e| e.to_string())?))
}

/// Абсолютный путь файла хранилища — для просмотра через asset-протокол.
#[tauri::command]
fn absolute(path: String, state: State<AppState>) -> Result<String, String> {
    with_vault(&state, |v| Ok(v.resolve(&path).map_err(|e| e.to_string())?.display().to_string()))
}

#[tauri::command]
fn reveal(path: String, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| platform::files::reveal(&v.resolve(&path).map_err(|e| e.to_string())?).map_err(|e| e.to_string()))
}

#[tauri::command]
fn pins(app: tauri::AppHandle) -> Vec<String> {
    platform::settings::pins(&app)
}

#[tauri::command]
fn set_pins(pins: Vec<String>, app: tauri::AppHandle) {
    platform::settings::save_pins(&app, &pins);
}

#[tauri::command]
fn templates(state: State<AppState>) -> Result<Vec<String>, String> {
    with_vault(&state, |v| Ok(v.templates()))
}

#[tauri::command]
fn create_from_template(template: String, folder: String, date: String, time: String, state: State<AppState>) -> Result<String, String> {
    with_vault(&state, |v| v.create_from_template(&template, &folder, &date, &time).map_err(|e| e.to_string()))
}

#[tauri::command]
fn daily(date: String, time: String, state: State<AppState>) -> Result<String, String> {
    with_vault(&state, |v| v.daily(&date, &time).map_err(|e| e.to_string()))
}

#[tauri::command]
fn set_due(path: String, line: usize, raw: String, date: Option<String>, time: Option<String>, minutes: Option<u32>, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| v.set_due(&path, line, &raw, date.as_deref(), time.as_deref(), minutes).map_err(|e| e.to_string()))
}

#[tauri::command]
fn set_done(path: String, line: usize, raw: String, done: bool, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| v.set_done(&path, line, &raw, done).map_err(|e| e.to_string()))
}

#[tauri::command]
fn set_task_title(path: String, line: usize, raw: String, title: String, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| v.set_task_title(&path, line, &raw, &title).map_err(|e| e.to_string()))
}

#[tauri::command]
fn delete_task(path: String, line: usize, raw: String, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| v.delete_task(&path, line, &raw).map_err(|e| e.to_string()))
}

#[tauri::command]
fn append_task(path: String, title: String, date: Option<String>, time: Option<String>, minutes: Option<u32>, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| v.append_task(&path, &title, date.as_deref(), time.as_deref(), minutes).map_err(|e| e.to_string()))
}

/// Задачи с датой — в `.ics` для Календаря Windows и Outlook. Путь выбирает человек в окне сохранения.
#[tauri::command]
fn export_ics(path: String, stamp: String, state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| std::fs::write(&path, notes_core::ics::calendar(&v.tasks(), &stamp)).map_err(|e| e.to_string()))
}

#[tauri::command]
fn ics(stamp: String, state: State<AppState>) -> Result<String, String> {
    with_vault(&state, |v| Ok(notes_core::ics::calendar(&v.tasks(), &stamp)))
}

#[tauri::command]
fn prefs(app: tauri::AppHandle) -> String {
    platform::settings::prefs(&app)
}

#[tauri::command]
fn set_prefs(json: String, app: tauri::AppHandle) -> Result<(), String> {
    serde_json::from_str::<serde_json::Value>(&json).map_err(|e| e.to_string())?;
    platform::settings::save_prefs(&app, &json);
    Ok(())
}

/// Путь открытого хранилища — для строки «папка хранилища» в настройках.
#[tauri::command]
fn vault_path(state: State<AppState>) -> Option<String> {
    state.vault.lock().ok()?.as_ref().map(|v| v.root().display().to_string())
}

#[derive(Serialize)]
struct GitStatus {
    /// Папка — git-репозиторий.
    repo: bool,
    /// Последний коммит, секунды с 1970.
    last_commit: Option<i64>,
    commits: usize,
    /// Несохранённых в истории файлов.
    pending: usize,
}

#[tauri::command]
fn git_status(state: State<AppState>) -> Result<GitStatus, String> {
    with_vault(&state, |v| {
        if !notes_core::git::is_repository(v.root()) {
            return Ok(GitStatus { repo: false, last_commit: None, commits: 0, pending: 0 });
        }
        let h = notes_core::git::History::open(v.root()).map_err(|e| e.to_string())?;
        Ok(GitStatus { repo: true, last_commit: h.last_commit(), commits: h.commit_count(), pending: h.changes().map(|c| c.len()).unwrap_or(0) })
    })
}

/// Включить историю: создать репозиторий (или открыть свой) и первый коммит.
#[tauri::command]
fn git_enable(state: State<AppState>) -> Result<(), String> {
    with_vault(&state, |v| notes_core::git::History::prepare(v.root()).map(|_| ()).map_err(|e| e.to_string()))
}

/// Сохранить версию сейчас. Подпись коммита — или `None`, если менять было нечего.
#[tauri::command]
fn git_commit(state: State<AppState>) -> Result<Option<String>, String> {
    with_vault(&state, |v| {
        if !notes_core::git::is_repository(v.root()) {
            return Ok(None);
        }
        notes_core::git::History::open(v.root()).and_then(|h| h.commit_now()).map_err(|e| e.to_string())
    })
}

/// Раздел «синхронизация»: версии и откат (порт `SyncScreen.swift`, 03.11.2026). Откат — новым коммитом.
fn with_history<T>(state: &State<AppState>, f: impl FnOnce(notes_core::git::History) -> Result<T, notes_core::git2::Error>) -> Result<T, String> {
    with_vault(state, |v| notes_core::git::History::open(v.root()).and_then(f).map_err(|e| e.message().to_string()))
}

#[tauri::command]
fn git_history(state: State<AppState>) -> Result<Vec<notes_core::git::Version>, String> {
    with_history(&state, |h| h.history(100))
}

#[tauri::command]
fn git_revert(id: String, state: State<AppState>) -> Result<(), String> {
    with_history(&state, |h| h.revert(&id))
}

#[tauri::command]
fn git_restore(id: String, stamp: String, state: State<AppState>) -> Result<(), String> {
    with_history(&state, |h| h.restore(&id, &stamp))
}

/// Файл, каким был в версии; None — тогда его не было.
#[tauri::command]
fn git_file_at(path: String, id: String, state: State<AppState>) -> Result<Option<String>, String> {
    with_history(&state, |h| h.file_at(&path, &id)).map(|b| b.map(|b| String::from_utf8_lossy(&b).into_owned()))
}

#[tauri::command]
fn git_restore_file(path: String, id: String, state: State<AppState>) -> Result<(), String> {
    with_history(&state, |h| h.restore_file(&path, &id))
}

/// Самопроверка в настоящем окне, как `-notesSelfTest` на маке: `NOTES_SELFTEST=1` — интерфейс сам
/// проходит сценарий на открытом хранилище и пишет итог в вывод, потом приложение закрывается.
#[tauri::command]
fn selftest() -> bool {
    std::env::var("NOTES_SELFTEST").is_ok_and(|v| v == "1")
}

#[tauri::command]
fn selftest_slow() -> bool {
    std::env::var("NOTES_SELFTEST_SLOW").is_ok_and(|v| v == "1")
}

/// Отладочная сборка: `NOTES_SECTION=tasks` — сразу в раздел, как `-notesSection` у Swift (снимки рядом с эталоном).
#[tauri::command]
fn debug_section() -> Option<String> {
    if cfg!(debug_assertions) { std::env::var("NOTES_SECTION").ok() } else { None }
}

/// Переменная окружения для самопроверки (путь к архиву модели и т. п.). Только в режиме самопроверки.
#[tauri::command]
fn selftest_env(name: String) -> Option<String> {
    if !selftest() || !name.starts_with("NOTES_SELFTEST_") {
        return None;
    }
    std::env::var(name).ok()
}

#[tauri::command]
fn selftest_log(line: String, done: bool, app: tauri::AppHandle) {
    println!("selftest: {line}");
    // В релизной сборке Windows консоли нет — итог ещё и в файл (CI читает его).
    if let Ok(path) = std::env::var("NOTES_SELFTEST_LOG") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "selftest: {line}");
        }
    }
    if done {
        app.exit(0);
    }
}

#[derive(Serialize)]
struct GithubStatus {
    signed_in: bool,
    remote: Option<String>,
    unpushed: bool,
}

#[tauri::command]
fn github_status(state: State<AppState>) -> Result<GithubStatus, String> {
    let signed_in = platform::secrets::token().is_some();
    with_vault(&state, |v| {
        let history = notes_core::git::History::open(v.root()).ok();
        Ok(GithubStatus {
            signed_in,
            remote: history.as_ref().and_then(|h| h.remote_url()),
            unpushed: history.as_ref().is_some_and(|h| h.has_unpushed()),
        })
    })
}

#[tauri::command]
async fn github_code() -> Result<github::Code, String> {
    tauri::async_runtime::spawn_blocking(github::request_code).await.map_err(|e| e.to_string())?
}

/// Ждём подтверждения кода, кладём токен в хранилище паролей ОС, создаём или находим приватный
/// `notes`, подключаем его к истории и отправляем её. Возвращает «логин/notes».
#[tauri::command]
async fn github_finish(code: github::Code, app: tauri::AppHandle) -> Result<String, String> {
    use std::sync::atomic::Ordering;
    let state = app.state::<AppState>();
    state.login_cancelled.store(false, Ordering::SeqCst);
    let root = state.vault.lock().map_err(|e| e.to_string())?.as_ref().ok_or("no vault open")?.root().to_path_buf();
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let grant = github::wait_for_token(&code, || handle.state::<AppState>().login_cancelled.load(Ordering::SeqCst))?;
        github::save_grant(&grant)?;
        let token = grant.token;
        let repo = github::ensure_notes(&token)?;
        let (history, _) = notes_core::git::History::prepare(&root).map_err(|e| e.to_string())?;
        history.set_remote(&repo.clone_url).map_err(|e| e.to_string())?;
        history.pull(Some(&token), &today()).map_err(|e| e.to_string())?;
        history.push(Some(&token)).map_err(|e| e.to_string())?;
        Ok(repo.full_name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn open_github(url: String) -> Result<(), String> {
    platform::files::open_github(&url)
}

#[tauri::command]
fn github_cancel(state: State<AppState>) {
    state.login_cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[tauri::command]
fn github_sign_out() {
    platform::secrets::forget_token();
    platform::secrets::forget_refresh();
}

/// Забрать правки с другого компьютера и отправить свои. Возвращает итог слияния.
#[tauri::command]
async fn github_sync(date: String, app: tauri::AppHandle) -> Result<notes_core::git::Pull, String> {
    let root = app.state::<AppState>().vault.lock().map_err(|e| e.to_string())?.as_ref().ok_or("no vault open")?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let token = github::session_token().ok_or("не вошли в github")?;
        let history = notes_core::git::History::open(&root).map_err(|e| e.to_string())?;
        let pulled = history.pull(Some(&token), &date).map_err(|e| e.to_string())?;
        history.push(Some(&token)).map_err(|e| e.to_string())?;
        Ok(pulled)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn today() -> String {
    // Только для имени копии при конфликте во время входа; дату в общем случае передаёт интерфейс.
    let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0) as i64;
    let (z, era) = (days + 719_468, (days + 719_468).div_euclid(146_097));
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[derive(Serialize)]
struct LockState {
    has_pin: bool,
    /// Спросить сейчас (при запуске — если закрывали больше 5 минут назад).
    needs_unlock: bool,
    /// Windows Hello настроен.
    hello: bool,
    /// Сколько секунд ждать после ошибок.
    wait: Option<i64>,
}

#[tauri::command]
fn lock_state(now: i64, app: tauri::AppHandle) -> LockState {
    let has_pin = platform::secrets::pin().is_some();
    let lock = platform::settings::lock(&app);
    LockState {
        has_pin,
        needs_unlock: has_pin && notes_core::lock::policy::at_launch(lock.last_seen, now),
        hello: has_pin && platform::auth::available(),
        wait: lock.attempts.remaining(now),
    }
}

/// Блокировка экрана/сон переживает перезапуск и не даёт пятиминутную льготу.
#[tauri::command]
fn lock_force(app: tauri::AppHandle) {
    let mut lock = platform::settings::lock(&app);
    lock.last_seen = None;
    platform::settings::save_lock(&app, &lock);
    safe::safe_lock();
}

/// Приложение открыто и разблокировано сейчас — отсчёт «закрыл и сразу открыл» отсюда.
#[tauri::command]
fn lock_seen(now: i64, app: tauri::AppHandle) {
    let mut lock = platform::settings::lock(&app);
    lock.last_seen = Some(now);
    platform::settings::save_lock(&app, &lock);
}

#[tauri::command]
fn lock_needed_on_return(inactive_since: i64, now: i64) -> bool {
    platform::secrets::pin().is_some() && notes_core::lock::policy::on_return(Some(inactive_since), now)
}

#[tauri::command]
fn pin_set(pin: String) -> Result<(), String> {
    if !notes_core::lock::valid(&pin) {
        return Err("нужно 4 цифры".into());
    }
    platform::secrets::save_pin(&notes_core::lock::PinHash::new(&pin).encoded())
}

#[tauri::command]
fn pin_clear(app: tauri::AppHandle) {
    platform::secrets::forget_pin();
    platform::settings::save_lock(&app, &Default::default());
}

#[derive(Serialize)]
struct PinCheck {
    ok: bool,
    wait: Option<i64>,
}

/// Проверка пинкода с паузой после ошибок. Хеш считается 200 000 раундов — в фоне.
#[tauri::command]
async fn pin_check(pin: String, now: i64, app: tauri::AppHandle) -> Result<PinCheck, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut lock = platform::settings::lock(&app);
        if let Some(wait) = lock.attempts.remaining(now) {
            return Ok(PinCheck { ok: false, wait: Some(wait) });
        }
        let hash = platform::secrets::pin().and_then(|e| notes_core::lock::PinHash::decode(&e)).ok_or("пинкод не задан")?;
        let ok = hash.matches(&pin);
        if ok {
            lock.attempts.reset();
            lock.last_seen = Some(now);
        } else {
            lock.attempts.failure(now);
        }
        platform::settings::save_lock(&app, &lock);
        Ok(PinCheck { ok, wait: lock.attempts.remaining(now) })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Windows Hello вместо пинкода. Окно системы — поверх нашего.
#[tauri::command]
async fn hello_verify(window: tauri::WebviewWindow) -> bool {
    tauri::async_runtime::spawn_blocking(move || platform::auth::verify(&window, "Откройте <notes>")).await.unwrap_or(false)
}

/// Кнопка микрофона в приложении — то же, что хоткей.
#[tauri::command]
fn dictation_toggle(app: tauri::AppHandle) {
    dictation::toggle(&app);
}

#[derive(Serialize)]
struct ModelStatus {
    id: &'static str,
    installed: bool,
    download_mb: u32,
}

#[tauri::command]
fn models(app: tauri::AppHandle) -> Vec<ModelStatus> {
    let store = dictation::store(&app);
    notes_speech::Model::ALL.iter().map(|&m| ModelStatus { id: m.id(), installed: store.is_installed(m), download_mb: m.download_mb() }).collect()
}

/// Скачать модель; ход — событием `model-progress` (0…1).
#[tauri::command]
async fn model_download(id: Option<String>, app: tauri::AppHandle) -> Result<(), String> {
    let store = dictation::store(&app);
    let handle = app.clone();
    let model = id.as_deref().and_then(notes_speech::Model::from_id).unwrap_or(notes_speech::Model::GigaAm);
    tauri::async_runtime::spawn_blocking(move || {
        store.download(model, |p| { let _ = handle.emit("model-progress", p); })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn model_remove(id: Option<String>, app: tauri::AppHandle) -> Result<(), String> {
    let model = id.as_deref().and_then(notes_speech::Model::from_id).unwrap_or(notes_speech::Model::GigaAm);
    dictation::store(&app).remove(model).map_err(|e| e.to_string())
}

/// Самопроверка без микрофона, как `-dictationTestFile` на маке: WAV 16 кГц моно проходит весь путь
/// до буфера обмена. `archive` — поставить модель из уже скачанного архива (облако не качает с github).
#[tauri::command]
fn dictation_test(wav: String, archive: Option<String>, app: tauri::AppHandle) -> Result<(), String> {
    if let Some(a) = archive {
        dictation::store(&app).install_from_archive(notes_speech::Model::GigaAm, std::path::Path::new(&a))?;
    }
    let wave = sherpa_onnx_wave(&wav)?;
    dictation::recognize(app, wave);
    Ok(())
}

fn sherpa_onnx_wave(path: &str) -> Result<Vec<f32>, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    // Простой PCM16 моно WAV: данные после чанка «data».
    let at = bytes.windows(4).position(|w| w == b"data").ok_or("не wav")? + 8;
    Ok(bytes[at..].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect())
}

fn e5_folder(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::platform::settings::data_dir(&app).join("models").join("multilingual-e5-small")
}

#[tauri::command]
fn meaning_installed(app: tauri::AppHandle) -> bool {
    notes_embed::is_installed(&e5_folder(&app))
}

/// Скачать модель поиска по смыслу; ход — событием `meaning-progress`.
#[tauri::command]
async fn meaning_download(app: tauri::AppHandle) -> Result<(), String> {
    let folder = e5_folder(&app);
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || notes_embed::download(&folder, |p| { let _ = handle.emit("meaning-progress", p); }))
        .await
        .map_err(|e| e.to_string())?
}

/// Поиск по смыслу: сначала дообновить индекс (только изменённые заметки), потом сравнить.
/// Пусто — модель не скачана.
#[tauri::command]
async fn meaning_search(query: String, app: tauri::AppHandle) -> Result<Vec<(String, f32)>, String> {
    if !notes_embed::is_installed(&e5_folder(&app)) {
        return Ok(Vec::new());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let vault = state.vault.lock().map_err(|e| e.to_string())?.clone().ok_or("no vault open")?;
        let mut embedder = state.embedder.lock().map_err(|e| e.to_string())?;
        if embedder.is_none() {
            *embedder = Some(notes_embed::Embedder::load(&e5_folder(&app))?);
        }
        let e = embedder.as_ref().unwrap();
        let path = notes_embed::index_path(&crate::platform::settings::data_dir(&app), &vault.root().display().to_string());
        let mut index = notes_embed::Index::load(&path);
        let notes = notes_core::search::all_notes(&vault.tree());
        let rows = notes.into_iter().filter_map(|n| Some((n.path.clone(), n.modified, n.title.clone(), vault.read(&n.path).ok()?)));
        if index.update(e, rows)? > 0 {
            let _ = index.save(&path);
        }
        Ok(index.search(&e.query(&query)?, 10))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Самопроверка: положить скачанную заранее модель e5 в папку моделей приложения.
#[tauri::command]
fn selftest_install_e5(from: String, app: tauri::AppHandle) -> Result<(), String> {
    if !selftest() {
        return Err("only in self-test".into());
    }
    let to = e5_folder(&app);
    std::fs::create_dir_all(&to).map_err(|e| e.to_string())?;
    for (name, _) in notes_embed::FILES {
        std::fs::copy(std::path::Path::new(&from).join(name), to.join(name)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn video_list(app: tauri::AppHandle) -> Vec<video::Item> {
    video::list(&app)
}

#[tauri::command]
fn video_add(path: String, added: String, app: tauri::AppHandle) -> Result<video::Item, String> {
    video::add(app, path, added)
}

#[tauri::command]
fn video_get(id: String, app: tauri::AppHandle) -> Option<notes_speech::Transcript> {
    video::get(&app, &id)
}

#[tauri::command]
fn video_rename_speaker(id: String, speaker: usize, name: String, app: tauri::AppHandle) -> Result<(), String> {
    let mut t = video::get(&app, &id).ok_or("нет расшифровки")?;
    t.rename(speaker, &name);
    video::put(&app, &id, &t)
}

/// Реплика `index` — другому спикеру (или новому: `speaker` = следующий номер).
#[tauri::command]
fn video_mark(id: String, index: usize, speaker: usize, app: tauri::AppHandle) -> Result<(), String> {
    let mut t = video::get(&app, &id).ok_or("нет расшифровки")?;
    t.mark(index, speaker);
    video::put(&app, &id, &t)
}

#[tauri::command]
fn speakers_installed(app: tauri::AppHandle) -> bool {
    notes_speech::speakers::Speakers::new(&dictation::store(&app).root).is_installed()
}

#[tauri::command]
async fn speakers_download(app: tauri::AppHandle) -> Result<(), String> {
    let s = notes_speech::speakers::Speakers::new(&dictation::store(&app).root);
    tauri::async_runtime::spawn_blocking(move || s.download()).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn selftest_install_speakers(segmentation: String, embedding: String, app: tauri::AppHandle) -> Result<(), String> {
    if !selftest() {
        return Err("only in self-test".into());
    }
    notes_speech::speakers::Speakers::new(&dictation::store(&app).root)
        .install_from(std::path::Path::new(&segmentation), std::path::Path::new(&embedding))
}

#[tauri::command]
fn video_folder(id: String, folder: Option<String>, app: tauri::AppHandle) {
    video::set_folder(&app, &id, folder);
}

#[tauri::command]
fn video_search(query: String, app: tauri::AppHandle) -> Vec<(String, String, String)> {
    video::search(&app, &query)
}

#[tauri::command]
fn video_delete(id: String, app: tauri::AppHandle) {
    video::delete(&app, &id);
}

/// Сохранить как заметку в «расшифровки/». Возвращает путь заметки.
#[tauri::command]
fn video_save_note(id: String, app: tauri::AppHandle, state: State<AppState>) -> Result<String, String> {
    let item = video::list(&app).into_iter().find(|i| i.id == id).ok_or("нет расшифровки")?;
    let t = video::get(&app, &id).ok_or("нет расшифровки")?;
    with_vault(&state, |v| {
        if let Some(path) = &item.saved_note { if v.read(path).is_ok() { return Ok(path.clone()); } }
        let dir = item.folder.as_ref().map_or("расшифровки".to_string(), |f| format!("расшифровки/{f}"));
        let path = v.create_note(&dir, &item.title).map_err(|e| e.to_string())?;
        let title = path.rsplit('/').next().unwrap_or(&path).trim_end_matches(".md").to_string();
        v.write(&path, &t.note(&title, &item.source, &item.added, item.duration, item.model.as_deref().unwrap_or("gigaam-v3-punct"))).map_err(|e| e.to_string())?;
        video::remember_note(&app, &id, &path);
        Ok(path)
    })
}

#[tauri::command]
fn video_srt(id: String, path: String, app: tauri::AppHandle) -> Result<(), String> {
    std::fs::write(path, video::get(&app, &id).ok_or("нет расшифровки")?.srt()).map_err(|e| e.to_string())
}

#[tauri::command]
fn autostart() -> bool {
    platform::autostart::enabled()
}

#[tauri::command]
fn set_autostart(on: bool) -> Result<(), String> {
    platform::autostart::set(on)
}

/// Сменить хоткей надиктовки. Без Ctrl, Alt или Win не даём (перехватили бы обычный набор), F-клавиши
/// можно — как на маке. Занят другой программой — Windows откажет, остаётся старый.
#[tauri::command]
fn set_hotkey(combo: String, app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let lower = combo.to_lowercase();
    let modifier = ["ctrl", "alt", "super", "cmd"].iter().any(|m| lower.contains(m));
    let fkey = lower.split('+').last().is_some_and(|k| k.starts_with('f') && k[1..].parse::<u8>().is_ok());
    if !modifier && !fkey {
        return Err("нужен ctrl, alt или win".into());
    }
    let gs = app.global_shortcut();
    let old = platform::settings::pref_str(&app, "hotkey").unwrap_or_else(|| DICTATION_SHORTCUT.into());
    let _ = gs.unregister(old.as_str());
    if let Err(e) = gs.register(combo.as_str()) {
        let _ = gs.register(old.as_str());
        return Err(format!("занято другой программой: {e}"));
    }
    Ok(())
}

/// Иконка окна и панели задач — под тему, как иконка в Доке на маке (тёмная тема — тёмная иконка).
#[tauri::command]
fn set_app_icon(dark: bool, window: tauri::WebviewWindow) -> Result<(), String> {
    let bytes: &[u8] = if dark { include_bytes!("../icons/taskbar-dark.png") } else { include_bytes!("../icons/taskbar-light.png") };
    window.set_icon(tauri::image::Image::from_bytes(bytes).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// Значок в трее — «<n>» шрифтом логотипа, как в строке меню на маке. Рисует интерфейс (там шрифт), сюда — PNG.
#[tauri::command]
fn set_tray_icon(png: Vec<u8>, app: tauri::AppHandle) -> Result<(), String> {
    let tray = app.tray_by_id("notes").ok_or("нет значка")?;
    tray.set_icon(Some(tauri::image::Image::from_bytes(&png).map_err(|e| e.to_string())?)).map_err(|e| e.to_string())
}

/// «открыть <notes>» из панели трея.
#[tauri::command]
fn show_main(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Панель трея — порт `MenuBarItem`: окно без рамки над значком, прячется, когда уходит фокус.
fn create_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let panel = tauri::WebviewWindowBuilder::new(app, "tray", tauri::WebviewUrl::App("tray.html".into()))
        .title("<notes>")
        .inner_size(300.0, 380.0)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build()?;
    platform::window::apply_glass(&panel);
    let mut tray = TrayIconBuilder::with_id("notes").tooltip("<notes>");
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.on_tray_icon_event(|tray, event| {
        let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } = event else { return };
        let Some(panel) = tray.app_handle().get_webview_window("tray") else { return };
        if panel.is_visible().unwrap_or(false) {
            let _ = panel.hide();
            return;
        }
        // Над значком: панель трея внизу — окно выше, по центру значка; у края экрана не вылезаем.
        let scale = panel.scale_factor().unwrap_or(1.0);
        let (pos, size) = (rect.position.to_physical::<f64>(scale), rect.size.to_physical::<f64>(scale));
        let (w, h) = (300.0 * scale, 380.0 * scale);
        let mut x = pos.x + size.width / 2.0 - w / 2.0;
        let mut y = if pos.y > h { pos.y - h - 8.0 * scale } else { pos.y + size.height + 8.0 * scale };
        if let Ok(Some(m)) = panel.current_monitor() {
            x = x.clamp(m.position().x as f64, m.position().x as f64 + m.size().width as f64 - w);
            y = y.max(m.position().y as f64);
        }
        let _ = panel.set_position(tauri::PhysicalPosition::new(x, y));
        let _ = panel.show();
        let _ = panel.set_focus();
    })
    .build(app)?;
    Ok(())
}

/// «Создать папку» на первом входе — как на маке: окно сохранения с именем «notes», папка создаётся там.
#[tauri::command]
fn create_vault_at(path: String, state: State<AppState>, app: tauri::AppHandle) -> Result<Folder, String> {
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    open_vault(path, state, app)
}

/// Какая ОС — интерфейс выбирает стекло окна (Mica на Windows, Liquid Glass на маке).
#[tauri::command]
fn os() -> &'static str {
    std::env::consts::OS
}

#[cfg(target_os = "macos")]
pub const DICTATION_SHORTCUT: &str = "alt+space";
#[cfg(not(target_os = "macos"))]
pub const DICTATION_SHORTCUT: &str = "ctrl+alt+space";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_plugin_liquid_glass::init());
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            // Хоткей надиктовки: Ctrl+Alt+Space (на маке ⌥ Space; Alt+Space на Windows — меню окна).
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts([DICTATION_SHORTCUT])
                .expect("valid shortcut")
                .with_handler(|app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        dictation::toggle(app);
                    }
                })
                .build(),
        )
        .manage(dictation::Dictation::default())
        .manage(AppState::default())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            platform::settings::migrate(app.handle()).map_err(std::io::Error::other)?;
            if let Some(window) = app.get_webview_window("main") {
                platform::window::apply_glass(&window);
                // Отладка: `NOTES_SIZE=783x686` — окно ровно как у Swift-эталона, для снимков рядом.
                #[cfg(debug_assertions)]
                if let Some((w, h)) = std::env::var("NOTES_SIZE").ok().and_then(|s| { let (w, h) = s.split_once('x')?; Some((w.parse::<f64>().ok()?, h.parse::<f64>().ok()?)) }) {
                    // Через полсекунды: размер, заданный до показа окна, ломал снимок окна (screencapture -l).
                    let win = window.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        let _ = win.set_min_size(None::<tauri::LogicalSize<f64>>);
                        let _ = win.set_size(tauri::LogicalSize::new(w, h));
                    });
                }
            }
            dictation::create_orb(app.handle())?;
            // Без трея (нет области уведомлений) приложение всё равно работает.
            if let Err(e) = create_tray(app.handle()) {
                eprintln!("tray: {e}");
            }
            // Свой хоткей из настроек — вместо стандартного.
            if let Some(h) = platform::settings::pref_str(app.handle(), "hotkey") {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                let gs = app.global_shortcut();
                if gs.register(h.as_str()).is_ok() {
                    let _ = gs.unregister(DICTATION_SHORTCUT);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_vault, create_vault, create_vault_at, set_app_icon, last_vault, tree, read_note, write_note, create_note, rename_note, search, tasks, os,
            cal_state, cal_request, cal_calendars, cal_events, cal_move, cal_rename, cal_remove, cal_create_calendar, cal_rename_calendar, cal_set_color, cal_remove_calendar, cal_links, cal_sync,
            create_folder, duplicate, move_into, copy_in, trash, untrash, reveal, open_default, absolute, pins, set_pins,
            templates, create_from_template, daily,
            set_due, set_done, set_task_title, delete_task, append_task, export_ics, ics, prefs, set_prefs, vault_path, git_status, git_enable, git_commit, set_tray_icon, show_main, safe::safe_state, safe::safe_create, safe::safe_unlock, safe::safe_unlock_hello, safe::safe_lock, safe::safe_items, safe::safe_put, safe::safe_delete, safe::safe_import, safe::safe_codes, safe::safe_totp, safe::safe_generate, safe::safe_weak, safe::safe_copy, safe::screen_locked, mcp::mcp_info, mcp::mcp_clear_log, mcp::mcp_skills, mcp::mcp_install_skill, git_history, git_revert, git_restore, git_file_at, git_restore_file, selftest, selftest_slow, selftest_log, selftest_env, debug_section,
            github_status, github_code, github_finish, github_cancel, github_sign_out, github_sync, open_github,
            intelligence::intelligence_state, intelligence::intelligence_apply,
            lock_state, lock_force, lock_seen, lock_needed_on_return, pin_set, pin_clear, pin_check, hello_verify,
            dictation_toggle, models, model_download, model_remove, dictation_test,
            meaning_installed, meaning_download, meaning_search, selftest_install_e5,
            video_list, video_add, video_get, video_rename_speaker, video_delete, video_save_note, video_srt,
            video_mark, video_folder, video_search, autostart, set_autostart, set_hotkey, speakers_installed, speakers_download, selftest_install_speakers
        ])
        .run(tauri::generate_context!())
        .expect("error while running <notes>");
}
