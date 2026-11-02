//! Расшифровки видео и аудио — как `VideoModel` на маке: файл → распознавание на этом компьютере →
//! расшифровка в данных приложения (не в хранилище); в хранилище — только когда человек нажал
//! «сохранить как заметку» (`расшифровки/название.md`).

use notes_speech::{media, GigaAm, Model, Transcript};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub source: String,
    /// «2026-11-02 14:30»
    pub added: String,
    pub duration: f64,
    /// null — ещё распознаём.
    pub words: Option<usize>,
    #[serde(default)]
    pub error: Option<String>,
}

fn dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("transcripts")
}

pub fn list(app: &AppHandle) -> Vec<Item> {
    let mut items: Vec<Item> = std::fs::read_to_string(dir(app).join("index.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    items.sort_by(|a, b| b.added.cmp(&a.added));
    items
}

fn save_list(app: &AppHandle, items: &[Item]) {
    let _ = std::fs::create_dir_all(dir(app));
    let _ = std::fs::write(dir(app).join("index.json"), serde_json::to_string(items).unwrap_or_default());
}

fn update(app: &AppHandle, item: Item) {
    let mut items = list(app);
    items.retain(|i| i.id != item.id);
    items.push(item);
    save_list(app, &items);
}

pub fn get(app: &AppHandle, id: &str) -> Option<Transcript> {
    serde_json::from_str(&std::fs::read_to_string(dir(app).join(format!("{id}.json"))).ok()?).ok()
}

pub fn put(app: &AppHandle, id: &str, t: &Transcript) -> Result<(), String> {
    std::fs::create_dir_all(dir(app)).map_err(|e| e.to_string())?;
    std::fs::write(dir(app).join(format!("{id}.json")), serde_json::to_string(t).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

pub fn delete(app: &AppHandle, id: &str) {
    let mut items = list(app);
    items.retain(|i| i.id != id);
    save_list(app, &items);
    let _ = std::fs::remove_file(dir(app).join(format!("{id}.json")));
}

/// Добавить файл: сразу строка «распознаём…», дальше — в фоне; итог — событием `video`.
pub fn add(app: AppHandle, path: String, added: String) -> Result<Item, String> {
    let p = PathBuf::from(&path);
    let id = format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    let item = Item {
        id: id.clone(),
        title: p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "расшифровка".into()),
        source: p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
        added,
        duration: 0.0,
        words: None,
        error: None,
    };
    update(&app, item.clone());
    let mut working = item.clone();
    std::thread::spawn(move || {
        let result = (|| -> Result<Transcript, String> {
            let samples = media::decode(&p)?;
            working.duration = samples.len() as f64 / notes_speech::SAMPLE_RATE as f64;
            let store = crate::dictation::store(&app);
            if !store.is_installed(Model::GigaAm) {
                return Err("модель не скачана: настройки → надиктовка".into());
            }
            let model = GigaAm::load(&store.folder(Model::GigaAm))?;
            Ok(Transcript { segments: model.segments(&samples), ..Default::default() })
        })();
        match result {
            Ok(t) => {
                working.words = Some(t.words());
                let _ = put(&app, &working.id, &t);
            }
            Err(e) => working.error = Some(e),
        }
        update(&app, working.clone());
        let _ = app.emit("video", working);
    });
    Ok(item)
}
