//! Кто говорит — как `Diarization.swift` на маке: pyannote (где сменился голос) + CAM++ (чей голос),
//! 35 МБ по кнопке, с той же SHA-256. Реплики расшифровки получают номер спикера по пересечению по времени.

use crate::{Transcript, SAMPLE_RATE};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const SEG_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-segmentation-models/sherpa-onnx-pyannote-segmentation-3-0.tar.bz2";
const SEG_SHA: &str = "24615ee884c897d9d2ba09bb4d30da6bb1b15e685065962db5b02e76e4996488";
const SEG_INNER: &str = "sherpa-onnx-pyannote-segmentation-3-0/model.onnx";
const EMB_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx";
const EMB_SHA: &str = "aa3cfc16963a10586a9393f5035d6d6b57e98d358b347f80c2a30bf4f00ceba2";
pub const DOWNLOAD_MB: u32 = 35;

pub struct Speakers {
    pub folder: PathBuf,
}

fn sha(path: &Path) -> Result<String, String> {
    let mut h = Sha256::new();
    std::io::copy(&mut std::fs::File::open(path).map_err(|e| e.to_string())?, &mut h).map_err(|e| e.to_string())?;
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

impl Speakers {
    pub fn new(models_root: &Path) -> Self {
        Self { folder: models_root.join("speakers") }
    }

    pub fn is_installed(&self) -> bool {
        self.folder.join("segmentation.onnx").exists() && self.folder.join("embedding.onnx").exists()
    }

    /// Поставить из скачанных файлов: проверка SHA-256, из архива берём только модель.
    pub fn install_from(&self, segmentation_archive: &Path, embedding: &Path) -> Result<(), String> {
        if sha(segmentation_archive)? != SEG_SHA || sha(embedding)? != EMB_SHA {
            return Err("контрольная сумма не совпала".into());
        }
        std::fs::create_dir_all(&self.folder).map_err(|e| e.to_string())?;
        let mut archive = tar::Archive::new(bzip2::read::BzDecoder::new(std::fs::File::open(segmentation_archive).map_err(|e| e.to_string())?));
        let mut found = false;
        for entry in archive.entries().map_err(|e| e.to_string())? {
            let mut entry = entry.map_err(|e| e.to_string())?;
            if entry.path().ok().is_some_and(|p| p.to_string_lossy() == SEG_INNER) {
                entry.unpack(self.folder.join("segmentation.onnx")).map_err(|e| e.to_string())?;
                found = true;
            }
        }
        if !found {
            return Err("в архиве нет модели".into());
        }
        std::fs::copy(embedding, self.folder.join("embedding.onnx")).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn download(&self) -> Result<(), String> {
        let tmp = self.folder.with_extension("download");
        std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
        let get = |url: &str, to: &Path| -> Result<(), String> {
            let mut r = ureq::get(url).call().map_err(|e| e.to_string())?.into_reader();
            std::io::copy(&mut r, &mut std::fs::File::create(to).map_err(|e| e.to_string())?).map(|_| ()).map_err(|e| e.to_string())
        };
        let (a, b) = (tmp.join("seg.tar.bz2"), tmp.join("emb.onnx"));
        get(SEG_URL, &a)?;
        get(EMB_URL, &b)?;
        let result = self.install_from(&a, &b);
        let _ = std::fs::remove_dir_all(&tmp);
        result
    }

    /// Отрезки «кто говорит»: (начало, конец, спикер).
    pub fn turns(&self, samples: &[f32]) -> Result<Vec<(f64, f64, usize)>, String> {
        let path = |n: &str| Some(self.folder.join(n).to_string_lossy().into_owned());
        let threads = std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).clamp(1, 4)) as i32;
        let mut c = sherpa_onnx::OfflineSpeakerDiarizationConfig::default();
        c.segmentation.pyannote.model = path("segmentation.onnx");
        c.segmentation.num_threads = threads;
        c.segmentation.provider = Some("cpu".into());
        c.embedding.model = path("embedding.onnx");
        c.embedding.num_threads = threads;
        c.embedding.provider = Some("cpu".into());
        c.clustering.num_clusters = -1;
        c.clustering.threshold = 0.5;
        c.min_duration_on = 0.3;
        c.min_duration_off = 0.5;
        let sd = sherpa_onnx::OfflineSpeakerDiarization::create(&c).ok_or("не удалось загрузить модели спикеров")?;
        let _ = SAMPLE_RATE;
        let result = sd.process(samples).ok_or("спикеров не нашли")?;
        Ok(result.sort_by_start_time().iter().map(|s| (s.start as f64, s.end as f64, s.speaker.max(0) as usize)).collect())
    }
}

impl Transcript {
    /// Номер спикера каждой реплики — по наибольшему пересечению по времени; номера — по порядку
    /// появления (первый заговоривший — «спикер 1»). Имена сбрасываются (как `assign` на маке).
    pub fn assign(&mut self, turns: &[(f64, f64, usize)]) {
        let mut order: Vec<usize> = Vec::new();
        for s in &mut self.segments {
            let best = turns.iter().filter_map(|&(a, b, sp)| { let o = s.end.min(b) - s.start.max(a); (o > 0.0).then_some((sp, o)) })
                .max_by(|x, y| x.1.total_cmp(&y.1)).map(|x| x.0)
                .or_else(|| turns.iter().min_by(|x, y| (x.0 - s.start).abs().total_cmp(&(y.0 - s.start).abs())).map(|t| t.2));
            s.speaker = best.map(|raw| order.iter().position(|&r| r == raw).unwrap_or_else(|| { order.push(raw); order.len() - 1 }));
        }
        self.names.clear();
    }
}

#[cfg(test)]
mod tests {
    use crate::{Segment, Transcript};

    #[test]
    fn assign_by_overlap_in_order_of_appearance() {
        let seg = |a, b| Segment { start: a, end: b, text: "x".into(), speaker: None };
        let mut t = Transcript { segments: vec![seg(0.0, 5.0), seg(5.0, 9.0), seg(20.0, 22.0)], ..Default::default() };
        t.assign(&[(0.0, 4.5, 7), (4.5, 10.0, 3), (30.0, 31.0, 7)]);
        assert_eq!(t.segments.iter().map(|s| s.speaker).collect::<Vec<_>>(), vec![Some(0), Some(1), Some(0)], "no overlap → nearest turn start");
    }
}
