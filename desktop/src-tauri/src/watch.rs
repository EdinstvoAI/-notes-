//! Правки снаружи — как `FolderWatcher` на маке: другой редактор, синхронизация или git поменяли файлы —
//! интерфейс узнаёт и перечитывает дерево и открытую заметку. Служебное (`.git`, скрытое) не считаем.

use notify::{RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

static WATCHER: Mutex<Option<notify::RecommendedWatcher>> = Mutex::new(None);

/// Следить за папкой хранилища. Пачка событий за полсекунды — одно событие `vault-changed` с путями.
pub fn watch(app: AppHandle, root: PathBuf) {
    let (tx, rx) = std::sync::mpsc::channel::<PathBuf>();
    let Ok(mut w) = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(e) = res {
            for p in e.paths {
                let _ = tx.send(p);
            }
        }
    }) else { return };
    if w.watch(&root, RecursiveMode::Recursive).is_err() {
        return;
    }
    *WATCHER.lock().unwrap() = Some(w);
    std::thread::spawn(move || loop {
        let Ok(first) = rx.recv() else { return };
        let mut paths = vec![first];
        let until = Instant::now() + Duration::from_millis(500);
        while let Ok(p) = rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
            paths.push(p);
        }
        let rel: Vec<String> = paths.iter().filter_map(|p| relative(&root, p)).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        if !rel.is_empty() {
            let _ = app.emit("vault-changed", rel);
        }
    });
}

fn relative(root: &Path, p: &Path) -> Option<String> {
    let rel = p.strip_prefix(root).ok()?;
    if rel.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')) {
        return None;
    }
    Some(rel.to_string_lossy().replace('\\', "/"))
}
