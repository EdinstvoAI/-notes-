//! Распознавание речи на этом компьютере — порт `NotesSpeech` с мака. Модели в сборку не входят:
//! качаются по требованию в папку данных приложения и проверяются по SHA-256 (CLAUDE.md, «лёгкая сборка»).
//! На входе — семплы 16 кГц моно, Float −1…1; на выходе — текст.

pub mod media;
pub mod models;
pub mod speakers;
pub mod transcript;

use std::ops::Range;
use std::path::Path;

pub use models::{Model, ModelStore};
pub use transcript::{Segment, Transcript};

pub const SAMPLE_RATE: usize = 16_000;

/// Куски не длиннее `max_seconds`: режем в самой тихой десятой доле секунды между 70 % и 100 % длины
/// куска, чтобы не рвать слово; обрезки короче 0,3 с отбрасываем — это тишина, модель на них сбоит.
/// GigaAM падает на записи длиннее ~25 с — поэтому по 20 (как `Transcribers.pieces` на маке).
pub fn pieces(samples: &[f32], max_seconds: f64) -> Vec<Range<usize>> {
    let limit = (SAMPLE_RATE as f64 * max_seconds) as usize;
    let (window, minimum) = (SAMPLE_RATE / 10, SAMPLE_RATE * 3 / 10);
    let mut out = Vec::new();
    let mut start = 0;
    while start < samples.len() {
        let mut end = samples.len();
        if end - start > limit {
            let (mut best, mut quietest) = (start + limit, f32::MAX);
            let mut at = start + limit * 7 / 10;
            while at + window <= start + limit {
                let energy: f32 = samples[at..at + window].iter().map(|s| s * s).sum();
                if energy < quietest {
                    quietest = energy;
                    best = at + window / 2;
                }
                at += window / 2;
            }
            end = best;
        }
        if end - start >= minimum {
            out.push(start..end);
        }
        start = end;
    }
    out
}

/// GigaAM v3 с пунктуацией (NeMo transducer) через sherpa-onnx. Загрузка в память — секунды:
/// создаём один раз и держим. Вызовы — по очереди, не параллельно.
pub struct GigaAm {
    recognizer: sherpa_onnx::OfflineRecognizer,
}

impl GigaAm {
    pub fn load(folder: &Path) -> Result<Self, String> {
        let path = |name: &str| Some(folder.join(name).to_string_lossy().into_owned());
        let mut config = sherpa_onnx::OfflineRecognizerConfig::default();
        config.feat_config.sample_rate = SAMPLE_RATE as i32;
        config.feat_config.feature_dim = 64; // GigaAM считает 64 мел-полосы
        config.model_config.transducer.encoder = path("encoder.int8.onnx");
        config.model_config.transducer.decoder = path("decoder.onnx");
        config.model_config.transducer.joiner = path("joiner.onnx");
        config.model_config.tokens = path("tokens.txt");
        config.model_config.model_type = Some("nemo_transducer".into());
        config.model_config.provider = Some("cpu".into());
        config.model_config.num_threads = std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).clamp(1, 4)) as i32;
        config.decoding_method = Some("greedy_search".into());
        let recognizer = sherpa_onnx::OfflineRecognizer::create(&config).ok_or("не удалось загрузить модель gigaam")?;
        Ok(Self { recognizer })
    }

    pub fn transcribe(&self, samples: &[f32]) -> String {
        pieces(samples, 20.0)
            .into_iter()
            .map(|r| self.decode(&samples[r]))
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Расшифровка файла: куски до 20 с со временем начала и конца.
    pub fn segments(&self, samples: &[f32]) -> Vec<Segment> {
        let rate = SAMPLE_RATE as f64;
        pieces(samples, 20.0).into_iter().filter_map(|r| {
            let text = self.decode(&samples[r.clone()]);
            (!text.is_empty()).then(|| Segment { start: r.start as f64 / rate, end: r.end as f64 / rate, text, speaker: None })
        }).collect()
    }

    fn decode(&self, samples: &[f32]) -> String {
        let stream = self.recognizer.create_stream();
        stream.accept_waveform(SAMPLE_RATE as i32, samples);
        self.recognizer.decode(&stream);
        stream.get_result().map(|r| r.text.trim().to_string()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_is_one_piece_silence_dropped() {
        assert_eq!(pieces(&vec![0.1; SAMPLE_RATE * 5], 20.0), vec![0..SAMPLE_RATE * 5]);
        assert!(pieces(&vec![0.1; SAMPLE_RATE / 10], 20.0).is_empty());
    }

    #[test]
    fn long_is_cut_in_quietest_place() {
        let mut s = vec![0.5f32; SAMPLE_RATE * 45];
        // Тишина на 17-й секунде — резать здесь, а не ровно на 20-й.
        for x in &mut s[SAMPLE_RATE * 17..SAMPLE_RATE * 17 + SAMPLE_RATE / 5] {
            *x = 0.0;
        }
        let p = pieces(&s, 20.0);
        assert!(p.len() >= 3);
        assert!(p[0].end > SAMPLE_RATE * 17 && p[0].end < SAMPLE_RATE * 17 + SAMPLE_RATE / 5, "{:?}", p[0]);
        assert!(p.iter().all(|r| r.len() <= SAMPLE_RATE * 20));
        assert_eq!(p.last().unwrap().end, s.len());
    }
}
