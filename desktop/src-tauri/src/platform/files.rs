//! Файлы глазами ОС: показать в Проводнике или Finder, удалить в Корзину (вернуть можно).

use std::path::Path;
use std::process::Command;

/// Показать файл выделенным в файловом менеджере.
pub fn reveal(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        // `/select,` и путь — одним аргументом, иначе Проводник открывает «Документы».
        Command::new("explorer").arg(format!("/select,{}", path.display())).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg("-R").arg(path).spawn()?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Command::new("xdg-open").arg(path.parent().unwrap_or(path)).spawn()?;
    }
    Ok(())
}

/// В Корзину Windows или macOS, а не насовсем: как на маке, удалённое можно вернуть.
pub fn trash(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let target = super::macos::call(serde_json::json!({"command": "trash", "text": path.to_string_lossy()}))?;
        let target = target.as_str().filter(|p| !p.is_empty()).ok_or("Корзина не вернула путь")?;
        TRASHED.lock().map_err(|e| e.to_string())?.insert(path.to_path_buf(), target.into());
        return Ok(());
    }
    #[cfg(not(target_os = "macos"))]
    trash::delete(path).map_err(|e| e.to_string())
}

/// Открыть страницу в браузере по умолчанию. Только https://github.com/ — больше приложению некуда.
pub fn open_github(url: &str) -> Result<(), String> {
    if !url.starts_with("https://github.com/") {
        return Err("only github.com".into());
    }
    #[cfg(target_os = "windows")]
    let status = Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(target_os = "macos")]
    let status = Command::new("open").arg(url).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let status = Command::new("xdg-open").arg(url).spawn();
    status.map(|_| ()).map_err(|e| e.to_string())
}

/// Вернуть из Корзины на старое место — для Ctrl+Z после «удалить». На Windows и Linux через список
/// Корзины; на маке — по квитанции FileManager этого запуска, без управления Finder.
#[cfg(target_os = "macos")]
static TRASHED: std::sync::Mutex<std::collections::BTreeMap<std::path::PathBuf, std::path::PathBuf>> = std::sync::Mutex::new(std::collections::BTreeMap::new());

pub fn untrash(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let mut receipts = TRASHED.lock().map_err(|e| e.to_string())?;
        let source = receipts.get(path).ok_or("нет удалённого этим запуском файла")?;
        // За время удаления по старому пути мог появиться новый файл: отмена не затирает его.
        if path.exists() { return Err("на прежнем месте уже есть файл".into()); }
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
        std::fs::rename(source, path).map_err(|e| e.to_string())?;
        receipts.remove(path);
        return Ok(());
    }
    #[cfg(any(target_os = "windows", all(unix, not(target_os = "macos"))))]
    {
        let items = trash::os_limited::list().map_err(|e| e.to_string())?;
        let mut found: Vec<_> = items.into_iter().filter(|i| i.original_path() == path).collect();
        found.sort_by_key(|i| i.time_deleted);
        let last = found.pop().ok_or("в корзине не нашли")?;
        return trash::os_limited::restore_all([last]).map_err(|e| e.to_string());
    }
    #[allow(unreachable_code)]
    {
        let _ = path;
        Err("на этой ОС вернуть из Корзины можно только руками".into())
    }
}

/// «Открыть в программе» — файл в программе по умолчанию (Word, Excel…), как на маке.
pub fn open_default(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let r = Command::new("rundll32").arg("url.dll,FileProtocolHandler").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let r = Command::new("open").arg(path).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let r = Command::new("xdg-open").arg(path).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}
