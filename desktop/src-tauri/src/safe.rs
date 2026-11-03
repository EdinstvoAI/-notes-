//! Раздел «пароли» — порт `SafeScreen.swift`/`SafeKeys.swift` (03.11.2026). Ключи сейфа живут в памяти,
//! пока он открыт. На этом компьютере они запоминаются в Диспетчере учётных данных Windows, а достать их
//! можно только после Windows Hello (на маке — Secure Enclave и Touch ID). Сами файлы сейфа — в хранилище,
//! в том же формате, что на маке: один сейф открывается на обоих.

use crate::{platform, with_vault, AppState};
use notes_core::safe::{self, Item, Keys};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

struct Open {
    root: PathBuf,
    keys: Keys,
    items: Vec<Item>,
}

static OPEN: Mutex<Option<Open>> = Mutex::new(None);

fn root(state: &State<AppState>) -> Result<PathBuf, String> {
    with_vault(state, |v| Ok(v.root().to_path_buf()))
}

/// Запись в Диспетчере учётных данных — своя на каждое хранилище.
fn entry(root: &std::path::Path) -> Option<keyring::Entry> {
    keyring::Entry::new("ai.edinstvo.notes", &format!("safe-{}", safe::vault_tag(&root.display().to_string()))).ok()
}

fn stored(root: &std::path::Path) -> Option<Keys> {
    serde_json::from_str(&entry(root)?.get_password().ok()?).ok()
}

fn remember(root: &std::path::Path, keys: &Keys) -> Result<(), String> {
    let e = entry(root).ok_or("нет хранилища паролей windows")?;
    e.set_password(&serde_json::to_string(keys).unwrap()).map_err(|e| e.to_string())
}

fn open_with(root: PathBuf, keys: Keys) -> Result<(), safe::Failure> {
    let (items, split) = safe::open_vault(&root, &keys)?;
    if split {
        safe::save_vault(&root, &keys, &items)?;
    }
    *OPEN.lock().unwrap() = Some(Open { root, keys, items });
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
    Ok(SafeState { exists: safe::exists(&root, safe::FILE) || safe::exists(&root, safe::CODES_FILE),
        remembered: stored(&root).is_some(), unlocked, hello: platform::auth::available() })
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
    let ok = tauri::async_runtime::spawn_blocking(move || platform::auth::verify(&window, "Открыть сейф паролей")).await.unwrap_or(false);
    if !ok {
        return Ok(false);
    }
    let keys = stored(&root).ok_or("ключа на этом компьютере нет")?;
    open_with(root.clone(), keys).map_err(|_| {
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
        safe::save_vault(&o.root, &o.keys, &o.items).map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn safe_delete(id: String) -> Result<(), String> {
    with_open(|o| {
        o.items.retain(|i| i.id != id);
        safe::save_vault(&o.root, &o.keys, &o.items).map_err(|e| e.to_string())
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
        safe::save_vault(&o.root, &o.keys, &o.items).map(|_| n).map_err(|e| e.to_string())
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
