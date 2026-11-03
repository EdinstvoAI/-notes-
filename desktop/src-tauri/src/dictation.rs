//! Надиктовка — как `DictationController` на маке: хоткей «нажал — нажал» (первое нажатие — пишем,
//! второе — распознаём), сфера поверх всех окон, текст — в буфер обмена. Всё на этом компьютере.
//! Забыли нажать второй раз — через 5 минут останавливаемся сами; пока распознаём, нажатия не считаем.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

use notes_speech::{GigaAm, Model, ModelStore, SAMPLE_RATE};

/// Фразы короче — случайное нажатие, не распознаём (как на маке).
const MIN_SECONDS: f32 = 0.4;
const MAX_SECONDS: u64 = 5 * 60;

#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum Event {
    Listening,
    /// Громкость 0…1, ~20 раз в секунду — для сферы.
    Level { level: f32, bands: [f32; 4] },
    Recognizing,
    Copied { text: String },
    Error { message: String },
    Idle,
}

#[derive(Default)]
pub struct Dictation {
    recording: Mutex<Option<Recording>>,
    busy: Mutex<bool>,
    model: Mutex<Option<(String, GigaAm)>>,
}

struct Recording {
    // Поток cpal не Send на всех ОС — держим его в своём потоке и останавливаем флагом.
    stop: Arc<std::sync::atomic::AtomicBool>,
    samples: Arc<Mutex<Vec<f32>>>,
    rate: Arc<Mutex<u32>>,
    started: std::time::Instant,
}

/// Выбранная модель (настройки → надиктовка) и язык для Whisper.
pub fn chosen(app: &AppHandle) -> (Model, String) {
    let model = crate::platform::settings::pref_str(app, "speechModel").and_then(|id| Model::from_id(&id)).unwrap_or(Model::GigaAm);
    let lang = crate::platform::settings::pref_str(app, "language").unwrap_or_else(|| "ru".into());
    (model, lang)
}

pub fn store(app: &AppHandle) -> ModelStore {
    ModelStore::new(app.path().app_data_dir().unwrap_or_default().join("models"))
}

fn emit(app: &AppHandle, event: Event) {
    orb(app, &event);
    let _ = app.emit("dictation", event);
}

/// Окно сферы: показываем на время надиктовки, «готово» и ошибку держим полторы секунды.
fn orb(app: &AppHandle, event: &Event) {
    let Some(w) = app.get_webview_window("orb") else { return };
    match event {
        Event::Listening | Event::Recognizing => { let _ = w.show(); let _ = w.set_ignore_cursor_events(true); }
        Event::Copied { .. } | Event::Error { .. } => {
            let _ = w.show();
            let _ = w.set_ignore_cursor_events(true);
            let w2 = w.clone();
            std::thread::spawn(move || { std::thread::sleep(std::time::Duration::from_millis(1600)); let _ = w2.hide(); });
        }
        Event::Idle => { let _ = w.hide(); }
        Event::Level { .. } => {}
    }
}

/// Создать окно сферы: прозрачное, без рамки, поверх всех, не в панели задач, фокус не берёт,
/// клики пропускает. Внизу по центру экрана.
pub fn create_orb(app: &AppHandle) -> tauri::Result<()> {
    let window = tauri::WebviewWindowBuilder::new(app, "orb", tauri::WebviewUrl::App("orb.html".into()))
        .title("<notes> orb")
        .inner_size(240.0, 240.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .resizable(false)
        .shadow(false)
        .visible(false)
        .build()?;
    // Пропускать клики — только после первого показа: у ещё не показанного окна на Linux нет
    // нативной поверхности, и tao падает (01.11.2026). Ставим в `orb()` при каждом показе.
    if let Ok(Some(monitor)) = window.primary_monitor() {
        let (size, scale) = (monitor.size(), monitor.scale_factor());
        let (w, h) = (size.width as f64 / scale, size.height as f64 / scale);
        let _ = window.set_position(tauri::LogicalPosition::new(w / 2.0 - 120.0, h - 240.0 - 60.0));
    }
    Ok(())
}

/// Нажали хоткей (или кнопку микрофона).
pub fn toggle(app: &AppHandle) {
    let state = app.state::<Dictation>();
    if *state.busy.lock().unwrap() {
        return;
    }
    let running = state.recording.lock().unwrap().take();
    match running {
        Some(rec) => finish(app.clone(), rec),
        None => {
            if !store(app).is_installed(chosen(app).0) {
                emit(app, Event::Error { message: "модель не скачана: настройки → надиктовка".into() });
                return;
            }
            match start(app.clone()) {
                Ok(rec) => {
                    *state.recording.lock().unwrap() = Some(rec);
                    emit(app, Event::Listening);
                }
                Err(e) => emit(app, Event::Error { message: e }),
            }
        }
    }
}

fn start(app: AppHandle) -> Result<Recording, String> {
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
    let rate = Arc::new(Mutex::new(SAMPLE_RATE as u32));
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let (s, r, st) = (samples.clone(), rate.clone(), stop.clone());
    std::thread::spawn(move || {
        let run = || -> Result<cpal::Stream, String> {
            let device = cpal::default_host().default_input_device().ok_or("нет микрофона")?;
            let config = device.default_input_config().map_err(|e| e.to_string())?;
            *r.lock().unwrap() = config.sample_rate().0;
            let channels = config.channels() as usize;
            let rate_hz = config.sample_rate().0 as f32;
            let sink = s.clone();
            let level_app = app.clone();
            let mut last_level = std::time::Instant::now();
            let mut push = move |data: &[f32]| {
                let mono: Vec<f32> = data.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32).collect();
                if last_level.elapsed().as_millis() >= 50 {
                    let rms = (mono.iter().map(|x| x * x).sum::<f32>() / mono.len().max(1) as f32).sqrt();
                    emit(&level_app, Event::Level { level: (rms * 8.0).min(1.0), bands: bands(&mono, rate_hz) });
                    last_level = std::time::Instant::now();
                }
                sink.lock().unwrap().extend(mono);
            };
            let err = |e| eprintln!("микрофон: {e}");
            let stream = match config.sample_format() {
                cpal::SampleFormat::F32 => device.build_input_stream(&config.into(), move |d: &[f32], _| push(d), err, None),
                cpal::SampleFormat::I16 => device.build_input_stream(&config.into(),
                    move |d: &[i16], _| push(&d.iter().map(|&x| x as f32 / 32768.0).collect::<Vec<_>>()), err, None),
                other => return Err(format!("формат микрофона {other:?} не поддержан")),
            }
            .map_err(|e| e.to_string())?;
            stream.play().map_err(|e| e.to_string())?;
            Ok(stream)
        };
        match run() {
            Ok(stream) => {
                let _ = ready_tx.send(Ok(()));
                let started = std::time::Instant::now();
                while !st.load(std::sync::atomic::Ordering::SeqCst) {
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    if started.elapsed().as_secs() >= MAX_SECONDS {
                        // Забыли нажать второй раз — останавливаемся сами.
                        let app2 = app.clone();
                        std::thread::spawn(move || toggle(&app2));
                        break;
                    }
                }
                drop(stream);
            }
            Err(e) => { let _ = ready_tx.send(Err(e)); }
        }
    });
    ready_rx.recv().map_err(|e| e.to_string())??;
    Ok(Recording { stop, samples, rate, started: std::time::Instant::now() })
}

fn finish(app: AppHandle, rec: Recording) {
    rec.stop.store(true, std::sync::atomic::Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(60));
    let raw = std::mem::take(&mut *rec.samples.lock().unwrap());
    let rate = *rec.rate.lock().unwrap();
    let _ = rec.started;
    recognize(app, resample(&raw, rate));
}

/// Распознать готовые семплы 16 кГц (из микрофона или из файла самопроверки) и положить в буфер.
pub fn recognize(app: AppHandle, samples: Vec<f32>) {
    if (samples.len() as f32) < MIN_SECONDS * SAMPLE_RATE as f32 {
        emit(&app, Event::Idle);
        return;
    }
    *app.state::<Dictation>().busy.lock().unwrap() = true;
    emit(&app, Event::Recognizing);
    std::thread::spawn(move || {
        let state = app.state::<Dictation>();
        let result = (|| -> Result<String, String> {
            let mut model = state.model.lock().unwrap();
            let (which, lang) = chosen(&app);
            let key = format!("{}:{lang}", which.id());
            // Модель или язык сменили в настройках — перезагружаем.
            if model.as_ref().map(|(k, _)| k != &key).unwrap_or(true) {
                *model = Some((key, notes_speech::load(which, &store(&app).folder(which), &lang)?));
            }
            Ok(model.as_ref().unwrap().1.transcribe(&samples))
        })();
        *state.busy.lock().unwrap() = false;
        match result {
            Ok(text) if !text.is_empty() => {
                use tauri_plugin_clipboard_manager::ClipboardExt;
                let _ = app.clipboard().write_text(text.clone());
                // «Вставлять в поле» (настройки → надиктовка) — Ctrl+V за человека.
                if crate::platform::settings::pref_bool(&app, "paste") {
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    crate::platform::paste::ctrl_v();
                }
                emit(&app, Event::Copied { text });
            }
            Ok(_) => emit(&app, Event::Idle),
            Err(e) => emit(&app, Event::Error { message: e }),
        }
    });
}

/// Линейная передискретизация в 16 кГц — для речи достаточно.
pub fn resample(input: &[f32], rate: u32) -> Vec<f32> {
    if rate as usize == SAMPLE_RATE || input.is_empty() {
        return input.to_vec();
    }
    let ratio = rate as f64 / SAMPLE_RATE as f64;
    let n = (input.len() as f64 / ratio) as usize;
    (0..n)
        .map(|i| {
            let pos = i as f64 * ratio;
            let j = pos as usize;
            let frac = (pos - j as f64) as f32;
            let a = input[j];
            let b = *input.get(j + 1).unwrap_or(&a);
            a + (b - a) * frac
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn resample_48k_to_16k() {
        let input: Vec<f32> = (0..48_000).map(|i| (i as f32 / 48_000.0)).collect();
        let out = super::resample(&input, 48_000);
        assert_eq!(out.len(), 16_000);
        assert!((out[8_000] - 0.5).abs() < 0.001);
    }
}

/// Громкость по четырём полосам частот для сферы — как AudioRecorder на маке: 80–300 Гц, 300–1000,
/// 1–3 кГц, 3–8 кГц, в децибелах (тихая речь видна), у каждой полосы свой потолок, 35 дБ ниже — ноль.
/// Окно Ханна на 1024 отсчёта; считаем только нужные частоты, полное БПФ не нужно.
pub fn bands(samples: &[f32], rate: f32) -> [f32; 4] {
    const N: usize = 1024;
    const EDGES: [f32; 5] = [80.0, 300.0, 1000.0, 3000.0, 8000.0];
    const CEILINGS: [f32; 4] = [-38.0, -45.0, -55.0, -64.0];
    if samples.len() < 64 {
        return [0.0; 4];
    }
    let frame: Vec<f32> = (0..N)
        .map(|i| samples.get(i).copied().unwrap_or(0.0) * (0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (N - 1) as f32).cos()))
        .collect();
    let hz = rate / N as f32;
    let mut out = [0.0f32; 4];
    for b in 0..4 {
        let lo = ((EDGES[b] / hz) as usize).max(1);
        let hi = ((EDGES[b + 1] / hz) as usize).min(N / 2 - 1);
        if hi < lo {
            continue;
        }
        let mut energy = 0.0f32;
        for k in lo..=hi {
            let w = -2.0 * std::f32::consts::PI * k as f32 / N as f32;
            let (mut re, mut im) = (0.0f32, 0.0f32);
            for (i, x) in frame.iter().enumerate() {
                let a = w * i as f32;
                re += x * a.cos();
                im += x * a.sin();
            }
            // Как vDSP на маке: его вещественное БПФ даёт вдвое больший спектр — мощность ×4.
            energy += 4.0 * (re * re + im * im);
        }
        energy /= (hi - lo + 1) as f32;
        let db = 10.0 * (energy / (N * N) as f32 + 1e-12).log10();
        out[b] = ((db - CEILINGS[b] + 35.0) / 35.0).clamp(0.0, 1.0);
    }
    out
}

#[cfg(test)]
mod band_tests {
    #[test]
    fn tone_lights_its_band() {
        let tone: Vec<f32> = (0..1024).map(|i| 0.3 * (2.0 * std::f32::consts::PI * 1500.0 * i as f32 / 16_000.0).sin()).collect();
        let b = super::bands(&tone, 16_000.0);
        assert!(b[2] > 0.8 && b[0] < 0.3, "{b:?}");
        assert_eq!(super::bands(&vec![0.0; 1024], 16_000.0), [0.0; 4]);
    }
}
