//! Сейф вне папки заметок. На macOS ключи обёрнуты Secure Enclave и открываются
//! через Touch ID / пароль Mac; на Windows — Credential Manager и Windows Hello.
//! Прежний локальный сейф Swift открывается на месте, формат шифра общий.

use crate::{platform, with_vault, AppState};
use notes_core::safe::{self, Item, Keys};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

struct Open {
    root: PathBuf,
    keys: Keys,
    items: Vec<Item>,
    local: PathBuf,
}

static OPEN: Mutex<Option<Open>> = Mutex::new(None);

fn root(state: &State<AppState>) -> Result<PathBuf, String> {
    with_vault(state, |v| Ok(v.root().to_path_buf()))
}

/// Сейф — только на этом компьютере; путь хранилища даёт устойчивый идентификатор, не место записи.
fn home(vault: &std::path::Path) -> Result<PathBuf, String> {
    let tag = safe::vault_tag(&vault.display().to_string());
    if let Some(root) = platform::settings::test_root() { return Ok(root.join(format!("safe-{tag}"))); }
    #[cfg(target_os = "macos")]
    let base = {
        let support = PathBuf::from(std::env::var_os("HOME").ok_or("нет домашней папки")?).join("Library/Application Support");
        // Открываем существующий сейф Swift на месте: не копируем и не меняем его ключи при переходе.
        let legacy = support.join("makarkabanov").join(format!("safe-{tag}"));
        if legacy.join(safe::FILE).exists() || legacy.join(safe::CODES_FILE).exists() { return Ok(legacy); }
        support.join("ai.edinstvo.notes")
    };
    #[cfg(target_os = "windows")]
    let base = PathBuf::from(std::env::var_os("APPDATA").ok_or("нет папки настроек")?).join("ai.edinstvo.notes");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let base = PathBuf::from(std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"))).join("ai.edinstvo.notes");
    Ok(base.join(format!("safe-{tag}")))
}

#[cfg(target_os = "macos")]
fn key_file(vault: &std::path::Path) -> Option<PathBuf> {
    let home = home(vault).ok()?;
    Some(home.with_extension("key"))
}

/// Запись в Диспетчере учётных данных — своя на каждое хранилище.
#[cfg(not(target_os = "macos"))]
fn entry(root: &std::path::Path) -> Option<keyring::Entry> {
    keyring::Entry::new("ai.edinstvo.notes", &format!("safe-{}", safe::vault_tag(&root.display().to_string()))).ok()
}

#[cfg(not(target_os = "macos"))]
fn stored(root: &std::path::Path) -> Option<Keys> {
    serde_json::from_str(&entry(root)?.get_password().ok()?).ok()
}

#[cfg(not(target_os = "macos"))]
fn remember(root: &std::path::Path, keys: &Keys) -> Result<(), String> {
    let e = entry(root).ok_or("нет хранилища паролей windows")?;
    e.set_password(&serde_json::to_string(keys).unwrap()).map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn remember(root: &std::path::Path, keys: &Keys) -> Result<(), String> {
    let file = key_file(root).ok_or("нет папки ключей")?;
    platform::macos::call(serde_json::json!({"command": "safe_store", "text": file, "keys": keys})).map(|_| ())
}

fn remembered(root: &std::path::Path) -> bool {
    #[cfg(target_os = "macos")]
    { return key_file(root).is_some_and(|p| p.exists()); }
    #[cfg(not(target_os = "macos"))]
    { stored(root).is_some() }
}

fn load_keys(root: &std::path::Path) -> Result<Keys, String> {
    #[cfg(target_os = "macos")]
    {
        let file = key_file(root).ok_or("нет папки ключей")?;
        return serde_json::from_value(platform::macos::call(serde_json::json!({"command": "safe_load", "text": file}))?).map_err(|e| e.to_string());
    }
    #[cfg(not(target_os = "macos"))]
    { stored(root).ok_or_else(|| "ключа на этом компьютере нет".into()) }
}

fn open_with(root: PathBuf, keys: Keys) -> Result<(), safe::Failure> {
    let local = home(&root).map_err(safe::Failure::Io)?;
    // Старую бету читаем без удаления исходников. Новые записи всегда остаются вне git-хранилища.
    if !local.join(safe::FILE).exists() && !local.join(safe::CODES_FILE).exists() &&
       (root.join(safe::FILE).exists() || root.join(safe::CODES_FILE).exists()) {
        let (items, _) = safe::open_vault(&root, &keys)?;
        safe::save_vault(&local, &keys, &items)?;
    }
    let (items, split) = safe::open_vault(&local, &keys)?;
    if split {
        safe::save_vault(&local, &keys, &items)?;
    }
    *OPEN.lock().unwrap() = Some(Open { root, keys, items, local });
    Ok(())
}

#[derive(serde::Serialize)]
pub struct SafeState {
    exists: bool,
    /// Ключи запомнены здесь — открыть можно по Windows Hello.
    remembered: bool,
    unlocked: bool,
    hello: bool,
}

#[tauri::command]
pub fn safe_state(state: State<AppState>) -> Result<SafeState, String> {
    let root = root(&state)?;
    let unlocked = OPEN.lock().unwrap().as_ref().is_some_and(|o| o.root == root);
    let local = home(&root)?;
    Ok(SafeState { exists: safe::exists(&local, safe::FILE) || safe::exists(&local, safe::CODES_FILE) || safe::exists(&root, safe::FILE) || safe::exists(&root, safe::CODES_FILE),
        remembered: remembered(&root), unlocked, hello: platform::auth::available() })
}

/// Новый сейф. Без пароля — два случайных ключа; сначала запоминаем их, потом пишем файлы, иначе ключи
/// пропали бы вместе с сейфом.
#[tauri::command]
pub fn safe_create(password: Option<String>, state: State<AppState>) -> Result<(), String> {
    let root = root(&state)?;
    let keys = match &password {
        Some(p) => Keys { safe: p.clone(), codes: p.clone() },
        None => {
            let keys = Keys::random();
            remember(&root, &keys).map_err(|_| "этот компьютер не сохранит ключ — придумай свой мастер-пароль")?;
            keys
        }
    };
    open_with(root.clone(), keys.clone()).map_err(|e| e.to_string())?;
    if password.is_some() {
        let _ = remember(&root, &keys);
    }
    Ok(())
}

/// Ключ сейфа или мастер-пароль, введённый руками (новый компьютер). После — запоминаем под Hello.
#[tauri::command]
pub fn safe_unlock(text: String, state: State<AppState>) -> Result<(), String> {
    let root = root(&state)?;
    let keys = Keys::from(&text).into_iter().find(|k| open_with(root.clone(), k.clone()).is_ok()).ok_or("ключ не подошёл")?;
    let _ = remember(&root, &keys);
    Ok(())
}

/// Открыть по Windows Hello: проверяет сама система, не приложение.
#[tauri::command]
pub async fn safe_unlock_hello(window: tauri::WebviewWindow, state: State<'_, AppState>) -> Result<bool, String> {
    let root = root(&state)?;
    #[cfg(not(target_os = "macos"))]
    let ok = tauri::async_runtime::spawn_blocking(move || platform::auth::verify(&window, "Открыть сейф паролей")).await.unwrap_or(false);
    #[cfg(not(target_os = "macos"))]
    if !ok {
        return Ok(false);
    }
    #[cfg(target_os = "macos")]
    let _ = window;
    let key_root = root.clone();
    let keys = tauri::async_runtime::spawn_blocking(move || load_keys(&key_root)).await.map_err(|e| e.to_string())??;
    open_with(root.clone(), keys).map_err(|_| {
        #[cfg(not(target_os = "macos"))]
        if let Some(e) = entry(&root) { let _ = e.delete_credential(); }
        "ключ на этом компьютере устарел — введи ключ сейфа".to_string()
    })?;
    Ok(true)
}

#[tauri::command]
pub fn safe_lock() {
    *OPEN.lock().unwrap() = None;
}

fn with_open<T>(f: impl FnOnce(&mut Open) -> Result<T, String>) -> Result<T, String> {
    f(OPEN.lock().unwrap().as_mut().ok_or("сейф закрыт")?)
}

#[tauri::command]
pub fn safe_items() -> Result<Vec<Item>, String> {
    with_open(|o| Ok(o.items.clone()))
}

/// Новая запись — сверху; существующая — заменяется с новой датой.
#[tauri::command]
pub fn safe_put(mut item: Item) -> Result<(), String> {
    with_open(|o| {
        item.modified = safe::now();
        if item.id.is_empty() {
            item.id = safe::uuid();
        }
        match o.items.iter().position(|i| i.id == item.id) {
            Some(i) => o.items[i] = item,
            None => o.items.insert(0, item),
        }
        safe::save_vault(&o.local, &o.keys, &o.items).map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn safe_delete(id: String) -> Result<(), String> {
    with_open(|o| {
        o.items.retain(|i| i.id != id);
        safe::save_vault(&o.local, &o.keys, &o.items).map_err(|e| e.to_string())
    })
}

/// Перенос из CSV (Пароли Apple, Edge, Chrome). Возвращает, сколько записей.
#[tauri::command]
pub fn safe_import(path: String) -> Result<usize, String> {
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let new = safe::import_csv(&text);
    with_open(|o| {
        let n = new.len();
        o.items.splice(0..0, new);
        safe::save_vault(&o.local, &o.keys, &o.items).map(|_| n).map_err(|e| e.to_string())
    })
}

/// Коды 2FA сейчас и сколько секунд до смены.
#[tauri::command]
pub fn safe_codes() -> Result<(std::collections::HashMap<String, String>, u64), String> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    with_open(|o| Ok((o.items.iter().filter(|i| i.kind == safe::Kind::Totp)
        .filter_map(|i| safe::totp(&i.secret, now).map(|c| (i.id.clone(), c))).collect(), 30 - now % 30)))
}

#[tauri::command]
pub fn safe_totp(secret: String) -> Option<String> {
    safe::totp(&secret, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs())
}

#[tauri::command]
pub fn safe_generate() -> String {
    safe::generate(20)
}

#[tauri::command]
pub fn safe_weak(password: String) -> bool {
    safe::is_weak(&password)
}

/// Скопировать пароль, код или ключ для другого компьютера. Через минуту буфер стирается, если в нём
/// всё ещё наше; в журнал буфера Windows (Win+V) и облачный буфер не попадает.
#[tauri::command]
pub fn safe_copy(what: String, id: Option<String>, app: tauri::AppHandle) -> Result<(), String> {
    let text = with_open(|o| {
        if what == "key" {
            return Ok(o.keys.transfer());
        }
        let item = o.items.iter().find(|i| Some(&i.id) == id.as_ref()).ok_or("нет записи")?;
        Ok(if item.kind == safe::Kind::Totp {
            safe::totp(&item.secret, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()).unwrap_or_default()
        } else {
            item.secret.clone()
        })
    })?;
    platform::clipboard::concealed(&app, &text)
}

/// Экран заблокирован (Win+L) — сейф закрывается, как на маке при блокировке экрана.
#[tauri::command]
pub fn screen_locked() -> bool {
    platform::clipboard::screen_locked()
}
