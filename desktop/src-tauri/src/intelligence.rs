//! Foundation Models только на Mac, по явному переключателю. Ответ не затирает параллельную правку.
use crate::{platform, with_vault, AppState};
use tauri::State;

#[tauri::command]
pub fn intelligence_state() -> serde_json::Value {
    #[cfg(target_os = "macos")]
    { return platform::macos::call(serde_json::json!({"command": "intelligence_state"})).unwrap_or_else(|e| serde_json::json!({"supported": true, "reason": e})); }
    #[cfg(not(target_os = "macos"))]
    serde_json::json!({"supported": false, "reason": "сейчас недоступна"})
}

#[tauri::command]
pub async fn intelligence_apply(path: String, expected: String, mode: String, app: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    if !platform::settings::pref_bool(&app, "intelligence") { return Err("apple intelligence выключена".into()); }
    if mode != "summary" && mode != "tasks" { return Err("неизвестное действие".into()); }
    with_vault(&state, |v| if v.read(&path).map_err(|e| e.to_string())? == expected { Ok(()) } else { Err("заметка уже изменилась".into()) })?;
    let input = expected.clone();
    let action = mode.clone();
    let value = tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        { platform::macos::call(serde_json::json!({"command": action, "text": input})) }
        #[cfg(not(target_os = "macos"))]
        { let _ = (action, input); Err::<serde_json::Value, String>("сейчас недоступна".into()) }
    }).await.map_err(|e| e.to_string())??;
    let result = if mode == "summary" {
        notes_core::sections::summary(&expected, value.as_str().ok_or("пустой ответ модели")?)
    } else {
        let tasks: Vec<(String, Option<String>)> = value.as_array().ok_or("неверный ответ модели")?.iter().filter_map(|t| Some((t.get("title")?.as_str()?.into(), t.get("date").and_then(|d| d.as_str()).map(str::to_string)))).collect();
        notes_core::sections::tasks(&expected, &tasks)
    };
    with_vault(&state, |v| {
        if v.read(&path).map_err(|e| e.to_string())? != expected { return Err("заметка изменилась во время обработки — повтори".into()); }
        v.write(&path, &result).map_err(|e| e.to_string())
    })?;
    Ok(result)
}
