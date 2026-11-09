//! Настройки приложения — вне хранилища: `%APPDATA%\ai.edinstvo.notes\` на Windows,
//! `~/Library/Application Support/ai.edinstvo.notes/` на маке. В хранилище своих файлов не кладём.

use std::fs;
use tauri::{AppHandle, Manager};

pub fn test_root() -> Option<std::path::PathBuf> {
    #[cfg(debug_assertions)]
    { return std::env::var_os("NOTES_TEST_ROOT").map(std::path::PathBuf::from); }
    #[cfg(not(debug_assertions))]
    None
}

pub fn config_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    if let Some(root) = test_root() { return Ok(root.join("config")); }
    app.path().app_config_dir().map_err(|e| e.to_string())
}

pub fn data_dir(app: &AppHandle) -> std::path::PathBuf {
    test_root().map(|p| p.join("data")).unwrap_or_else(|| app.path().app_data_dir().unwrap_or_default())
}

fn file(app: &AppHandle, name: &str) -> Option<std::path::PathBuf> {
    Some(config_dir(app).ok()?.join(name))
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

pub fn pref_num(app: &AppHandle, key: &str) -> Option<f64> {
    serde_json::from_str::<serde_json::Value>(&prefs(app)).ok().and_then(|v| v.get(key)?.as_f64())
}

/// Первый запуск Tauri подхватывает выбранную папку и настройки Swift, не меняя исходные данные.
#[cfg(target_os = "macos")]
pub fn migrate(app: &AppHandle) -> Result<(), String> {
    if test_root().is_some() || file(app, "migration-swift").is_some_and(|p| p.exists()) { return Ok(()); }
    if load(app, "vault-path").is_some() { return Ok(()); }
    let legacy = super::macos::call(serde_json::json!({"command": "legacy"}))?;
    if let Some(vault) = legacy.get("vault").and_then(|v| v.as_str()) {
        if let Some(pin) = legacy.get("pin").and_then(|v| v.as_str()) { super::secrets::save_pin(pin)?; }
        if load(app, "prefs.json").is_none() {
            if let Some(prefs) = legacy.get("prefs") { save_prefs(app, &prefs.to_string()); }
        }
        save_vault_path(app, vault);
    }
    save(app, "migration-swift", "1");
    Ok(())
}
