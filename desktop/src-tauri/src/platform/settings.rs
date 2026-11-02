//! Настройки приложения — вне хранилища: `%APPDATA%\ai.edinstvo.notes\` на Windows,
//! `~/Library/Application Support/ai.edinstvo.notes/` на маке. В хранилище своих файлов не кладём.

use std::fs;
use tauri::{AppHandle, Manager};

fn file(app: &AppHandle, name: &str) -> Option<std::path::PathBuf> {
    Some(app.path().app_config_dir().ok()?.join(name))
}

fn save(app: &AppHandle, name: &str, text: &str) {
    if let Some(file) = file(app, name) {
        if let Some(dir) = file.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(file, text);
    }
}

fn load(app: &AppHandle, name: &str) -> Option<String> {
    fs::read_to_string(file(app, name)?).ok()
}

pub fn save_vault_path(app: &AppHandle, path: &str) {
    save(app, "vault-path", path);
}

pub fn load_vault_path(app: &AppHandle) -> Option<String> {
    let path = load(app, "vault-path")?.trim().to_string();
    (!path.is_empty()).then_some(path)
}

/// Закреплённые заметки — пути от корня хранилища, по строке. Настройка приложения, как
/// `pinnedNotes` в UserDefaults на маке: в хранилище не пишем.
pub fn pins(app: &AppHandle) -> Vec<String> {
    load(app, "pins").map(|t| t.lines().filter(|l| !l.is_empty()).map(str::to_string).collect()).unwrap_or_default()
}

pub fn save_pins(app: &AppHandle, pins: &[String]) {
    save(app, "pins", &pins.join("\n"));
}

/// Настройки интерфейса (тема, размер текста, заметка дня…) — JSON целиком; разбирает интерфейс.
pub fn prefs(app: &AppHandle) -> String {
    load(app, "prefs.json").unwrap_or_else(|| "{}".to_string())
}

pub fn save_prefs(app: &AppHandle, json: &str) {
    save(app, "prefs.json", json);
}

/// Состояние входа между запусками: когда последний раз было открыто и разблокировано, ошибки пинкода.
/// Не секрет — секрет (хеш пинкода) в хранилище паролей.
#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct LockFile {
    pub last_seen: Option<i64>,
    pub attempts: notes_core::lock::Attempts,
}

pub fn lock(app: &AppHandle) -> LockFile {
    load(app, "lock.json").and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_lock(app: &AppHandle, lock: &LockFile) {
    if let Ok(t) = serde_json::to_string(lock) {
        save(app, "lock.json", &t);
    }
}

/// Одна настройка из `prefs.json` (её пишет интерфейс).
pub fn pref_bool(app: &AppHandle, key: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(&prefs(app)).ok().and_then(|v| v.get(key)?.as_bool()).unwrap_or(false)
}

pub fn pref_str(app: &AppHandle, key: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(&prefs(app)).ok().and_then(|v| Some(v.get(key)?.as_str()?.to_string()))
}
