//! Поиск по смыслу — модель multilingual-e5-small на этом компьютере (BERT, 384 измерения, понимает русский).
//! Модель качается по требованию с Hugging Face и проверяется по SHA-256; индекс лежит в данных
//! приложения, не в хранилище, и пересобирается из файлов. Это не чат-модель: только «смысл» числами.

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tokenizers::{Tokenizer, TruncationParams};

const BASE: &str = "https://huggingface.co/intfloat/multilingual-e5-small/resolve/main/";
/// Файлы модели и их SHA-256 — подменённый файл не загрузится.
pub const FILES: [(&str, &str); 3] = [
    ("model.safetensors", "1a55775f53449dac10a2bcbc312469fac40b96d53198c407081a831f81c98477"),
    ("tokenizer.json", "0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39"),
    ("config.json", "69137736cab8b8903a07fe8afaafdda25aac55415a12a55d1bffa9f581abf959"),
];
pub const DOWNLOAD_MB: u32 = 471;

fn sha256(path: &Path) -> std::io::Result<String> {
    let mut h = Sha256::new();
    std::io::copy(&mut fs::File::open(path)?, &mut h)?;
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

pub fn is_installed(folder: &Path) -> bool {
    FILES.iter().all(|(f, _)| folder.join(f).exists())
}

/// Скачать модель в `folder`. `progress` — 0…1. Каждый файл — во временный, после проверки — на место.
pub fn download(folder: &Path, progress: impl Fn(f64)) -> Result<(), String> {
    fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let total = f64::from(DOWNLOAD_MB) * 1_048_576.0;
    let mut done = 0usize;
    for (name, sha) in FILES {
        let target = folder.join(name);
        if target.exists() {
            continue;
        }
        let partial = folder.join(format!("{name}.download"));
        let mut reader = ureq::get(&format!("{BASE}{name}")).call().map_err(|e| e.to_string())?.into_reader();
        let mut file = fs::File::create(&partial).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 1 << 16];
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
        if sha256(&partial).map_err(|e| e.to_string())? != sha {
            let _ = fs::remove_file(&partial);
            return Err(format!("{name}: контрольная сумма не совпала"));
        }
        fs::rename(&partial, &target).map_err(|e| e.to_string())?;
    }
    progress(1.0);
    Ok(())
}

pub struct Embedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl Embedder {
    pub fn load(folder: &Path) -> Result<Self, String> {
        for (name, sha) in FILES {
            if sha256(&folder.join(name)).map_err(|e| e.to_string())? != sha {
                return Err(format!("{name}: контрольная сумма не совпала"));
            }
        }
        let device = Device::Cpu;
        let config: Config = serde_json::from_str(&fs::read_to_string(folder.join("config.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[folder.join("model.safetensors")], DType::F32, &device) }.map_err(|e| e.to_string())?;
        let model = BertModel::load(vb, &config).map_err(|e| e.to_string())?;
        let mut tokenizer = Tokenizer::from_file(folder.join("tokenizer.json")).map_err(|e| e.to_string())?;
        tokenizer.with_truncation(Some(TruncationParams { max_length: 512, ..Default::default() })).map_err(|e| e.to_string())?;
        tokenizer.with_padding(Some(Default::default()));
        Ok(Self { model, tokenizer, device })
    }

    /// Векторы единичной длины. e5 ждёт префикс: «query: » для запроса, «passage: » для текста.
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let enc = self.tokenizer.encode_batch(texts.to_vec(), true).map_err(|e| e.to_string())?;
        let ids: Vec<Tensor> = enc.iter().map(|e| Tensor::new(e.get_ids(), &self.device)).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
        let mask: Vec<Tensor> = enc.iter().map(|e| Tensor::new(e.get_attention_mask(), &self.device)).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
        let ids = Tensor::stack(&ids, 0).map_err(|e| e.to_string())?;
        let mask = Tensor::stack(&mask, 0).map_err(|e| e.to_string())?;
        let types = ids.zeros_like().map_err(|e| e.to_string())?;
        let out = self.model.forward(&ids, &types, Some(&mask)).map_err(|e| e.to_string())?;
        // Среднее по токенам без «пустых» (padding), затем нормировка — как в описании e5.
        let m = mask.to_dtype(DType::F32).and_then(|m| m.unsqueeze(2)).map_err(|e| e.to_string())?;
        let summed = out.broadcast_mul(&m).and_then(|x| x.sum(1)).map_err(|e| e.to_string())?;
        let counts = m.sum(1).map_err(|e| e.to_string())?;
        let mean = summed.broadcast_div(&counts).map_err(|e| e.to_string())?;
        let norm = mean.sqr().and_then(|x| x.sum_keepdim(1)).and_then(|x| x.sqrt()).map_err(|e| e.to_string())?;
        mean.broadcast_div(&norm).and_then(|x| x.to_vec2::<f32>()).map_err(|e| e.to_string())
    }

    pub fn query(&self, text: &str) -> Result<Vec<f32>, String> {
        Ok(self.embed(&[format!("query: {text}")])?.remove(0))
    }

    pub fn passages(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        let prefixed: Vec<String> = texts.iter().map(|t| format!("passage: {t}")).collect();
        prefixed.chunks(8).map(|c| self.embed(c)).collect::<Result<Vec<_>, _>>().map(|v| v.concat())
    }
}

/// Заметка режется на куски по абзацам до ~700 знаков: длинная заметка — несколько смыслов.
pub fn chunks(title: &str, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::from(title);
    for para in text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
        if cur.chars().count() + para.chars().count() > 700 && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push('\n');
        }
        cur.push_str(&para.chars().take(1500).collect::<String>());
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

#[derive(Default, Serialize, Deserialize)]
pub struct Index {
    notes: HashMap<String, Entry>,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    modified: u64,
    vectors: Vec<Vec<f32>>,
}

impl Index {
    pub fn load(path: &Path) -> Self {
        fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(d) = path.parent() {
            fs::create_dir_all(d)?;
        }
        fs::write(path, serde_json::to_vec(self)?)
    }

    /// Пересчитать только изменившиеся заметки и выкинуть удалённые. `notes` — (путь, изменено, название, текст).
    pub fn update<'a>(&mut self, e: &Embedder, notes: impl IntoIterator<Item = (String, u64, String, String)>) -> Result<usize, String> {
        let mut seen = std::collections::HashSet::new();
        let mut changed = 0;
        for (path, modified, title, text) in notes {
            seen.insert(path.clone());
            if self.notes.get(&path).is_some_and(|x| x.modified == modified) {
                continue;
            }
            let parts = chunks(&title, &text);
            let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
            self.notes.insert(path, Entry { modified, vectors: e.passages(&refs)? });
            changed += 1;
        }
        self.notes.retain(|p, _| seen.contains(p));
        Ok(changed)
    }

    /// Заметки по близости к запросу: (путь, сходство 0…1), лучшие сверху.
    pub fn search(&self, query: &[f32], limit: usize) -> Vec<(String, f32)> {
        let mut out: Vec<(String, f32)> = self
            .notes
            .iter()
            .filter_map(|(p, e)| {
                let best = e.vectors.iter().map(|v| v.iter().zip(query).map(|(a, b)| a * b).sum::<f32>()).fold(f32::MIN, f32::max);
                (best > f32::MIN).then(|| (p.clone(), best))
            })
            .collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1));
        out.truncate(limit);
        out
    }
}

pub fn index_path(data_dir: &Path, vault_key: &str) -> PathBuf {
    // Своё имя на каждое хранилище: несколько папок — несколько индексов.
    let mut h = Sha256::new();
    h.update(vault_key.as_bytes());
    let id: String = h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect();
    data_dir.join("search").join(format!("{id}.json"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn chunks_by_paragraph() {
        let text = (0..10).map(|i| format!("абзац {i} {}", "слово ".repeat(30))).collect::<Vec<_>>().join("\n\n");
        let parts = super::chunks("план", &text);
        assert!(parts.len() > 1);
        assert!(parts[0].starts_with("план"));
        assert!(parts.iter().all(|p| p.chars().count() <= 1500 + 700));
    }
}
