//! Где лежат модели и как их скачать — как `ModelStore` на маке. Папка — данные приложения
//! (`%APPDATA%\ai.edinstvo.notes\models` на Windows), не хранилище заметок.

use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Model {
    /// GigaAM v3 с пунктуацией: русский, 176 МБ.
    #[serde(rename = "gigaam-v3-punct")]
    GigaAm,
    /// Whisper small (int8): 99 языков, 610 МБ архив / ~360 МБ на диске.
    #[serde(rename = "whisper-small")]
    WhisperSmall,
}

impl Model {
    pub const ALL: [Model; 2] = [Model::GigaAm, Model::WhisperSmall];

    pub fn id(self) -> &'static str {
        match self {
            Model::GigaAm => "gigaam-v3-punct",
            Model::WhisperSmall => "whisper-small",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.id() == id)
    }

    pub fn download_mb(self) -> u32 {
        match self {
            Model::GigaAm => 176,
            Model::WhisperSmall => 610,
        }
    }

    /// Файл, по которому видно, что модель распакована целиком.
    fn marker(self) -> &'static str {
        match self {
            Model::GigaAm => "tokens.txt",
            Model::WhisperSmall => "small-tokens.txt",
        }
    }

    /// Что брать из архива (у Whisper в нём и fp32 — лишние 250 МБ). Пусто — всё.
    fn keep(self) -> &'static [&'static str] {
        match self {
            Model::GigaAm => &[],
            Model::WhisperSmall => &["small-encoder.int8.onnx", "small-decoder.int8.onnx", "small-tokens.txt", "test_wavs"],
        }
    }

    /// Архив с релиза k2-fsa/sherpa-onnx и его SHA-256 — те же, что на маке (`GigaAMArchive`):
    /// подменённый файл не запустится.
    fn archive(self) -> (&'static str, &'static str, &'static str) {
        if self == Model::WhisperSmall {
            return (
                "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-small.tar.bz2",
                "486a46afbb7ba798507190ffe02fea2dd726049af212e774537efac6afb210a6",
                "sherpa-onnx-whisper-small",
            );
        }
        (
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-transducer-punct-giga-am-v3-russian-2025-12-16.tar.bz2",
            "f9620a0099019c6afcee26525ef9ed3297fa50dd5691c1902af0c948fc1a470b",
            "sherpa-onnx-nemo-transducer-punct-giga-am-v3-russian-2025-12-16",
        )
    }
}

pub struct ModelStore {
    pub root: PathBuf,
}

impl ModelStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn folder(&self, model: Model) -> PathBuf {
        self.root.join(model.id())
    }

    pub fn is_installed(&self, model: Model) -> bool {
        self.folder(model).join(model.marker()).exists()
    }

    pub fn remove(&self, model: Model) -> io::Result<()> {
        let f = self.folder(model);
        if f.exists() { fs::remove_dir_all(f) } else { Ok(()) }
    }

    /// Скачать, проверить SHA-256 и распаковать. `progress` — доля 0…1. Уже скачанную не качаем.
    pub fn download(&self, model: Model, progress: impl Fn(f64)) -> Result<(), String> {
        if self.is_installed(model) {
            progress(1.0);
            return Ok(());
        }
        let (url, _, _) = model.archive();
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let partial = self.root.join(format!("{}.download", model.id()));
        let response = ureq::get(url).call().map_err(|e| e.to_string())?;
        let total: f64 = response.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(f64::from(model.download_mb()) * 1_048_576.0);
        let mut reader = response.into_reader();
        let mut file = fs::File::create(&partial).map_err(|e| e.to_string())?;
        let (mut buf, mut done) = (vec![0u8; 1 << 16], 0usize);
        loop {
            let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            done += n;
            progress((done as f64 / total).min(0.99));
        }
        drop(file);
        let result = self.install_from_archive(model, &partial);
        let _ = fs::remove_file(&partial);
        progress(1.0);
        result
    }

    /// Поставить модель из уже скачанного архива: SHA-256, распаковка во временную папку, переименование.
    pub fn install_from_archive(&self, model: Model, archive: &Path) -> Result<(), String> {
        let (_, sha, inner) = model.archive();
        let mut hasher = Sha256::new();
        io::copy(&mut fs::File::open(archive).map_err(|e| e.to_string())?, &mut hasher).map_err(|e| e.to_string())?;
        let got: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if got != sha {
            return Err("контрольная сумма не совпала — файл повреждён или подменён".into());
        }
        let staging = self.root.join(format!("{}.unpack", model.id()));
        let _ = fs::remove_dir_all(&staging);
        fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
        let file = fs::File::open(archive).map_err(|e| e.to_string())?;
        let mut archive = tar::Archive::new(bzip2::read::BzDecoder::new(file));
        let keep = model.keep();
        for entry in archive.entries().map_err(|e| e.to_string())? {
            let mut entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path().map_err(|e| e.to_string())?.into_owned();
            if keep.is_empty() || path.components().nth(1).is_some_and(|c| keep.contains(&c.as_os_str().to_string_lossy().as_ref())) {
                entry.unpack_in(&staging).map_err(|e| e.to_string())?;
            }
        }
        let target = self.folder(model);
        let _ = fs::remove_dir_all(&target);
        fs::rename(staging.join(inner), &target).map_err(|e| e.to_string())?;
        let _ = fs::remove_dir_all(&staging);
        Ok(())
    }
}
