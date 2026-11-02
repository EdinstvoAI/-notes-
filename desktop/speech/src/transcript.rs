//! Расшифровка — как `Transcript.swift` на маке: реплики со временем, спикеры с именами, .srt и
//! заметка «**00:12 Макар:** текст» с шапкой (источник, длительность, модель, слов).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Transcript {
    pub segments: Vec<Segment>,
    #[serde(default)]
    pub names: BTreeMap<usize, String>,
}

impl Transcript {
    pub fn name(&self, speaker: usize) -> String {
        self.names.get(&speaker).cloned().unwrap_or_else(|| format!("спикер {}", speaker + 1))
    }

    pub fn rename(&mut self, speaker: usize, name: &str) {
        let n = name.trim();
        if n.is_empty() { self.names.remove(&speaker); } else { self.names.insert(speaker, n.to_string()); }
    }

    pub fn mark(&mut self, index: usize, speaker: usize) {
        if let Some(s) = self.segments.get_mut(index) {
            s.speaker = Some(speaker);
        }
    }

    pub fn text(&self) -> String {
        self.segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ")
    }

    pub fn words(&self) -> usize {
        self.text().split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).count()
    }

    pub fn clock(seconds: f64) -> String {
        let t = seconds.max(0.0) as u64;
        let (h, m, s) = (t / 3600, t % 3600 / 60, t % 60);
        if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m:02}:{s:02}") }
    }

    pub fn srt(&self) -> String {
        let stamp = |t: f64| {
            let ms = (t * 1000.0).round() as u64;
            format!("{:02}:{:02}:{:02},{:03}", ms / 3_600_000, ms % 3_600_000 / 60_000, ms % 60_000 / 1000, ms % 1000)
        };
        self.segments.iter().enumerate()
            .map(|(i, s)| format!("{}\n{} --> {}\n{}\n", i + 1, stamp(s.start), stamp(s.end), s.text))
            .collect::<Vec<_>>().join("\n")
    }

    pub fn markdown(&self, title: &str) -> String {
        let mut previous = None;
        let lines: Vec<String> = self.segments.iter().map(|s| {
            let mut label = Self::clock(s.start);
            if let Some(sp) = s.speaker {
                if Some(sp) != previous { label.push_str(&format!(" {}:", self.name(sp))); }
                previous = Some(sp);
            }
            format!("**{label}** {}", s.text)
        }).collect();
        format!("# {title}\n\n{}\n", lines.join("\n\n"))
    }

    /// Заметка в «расшифровки/…»: шапка как на маке, `added` — «2026-11-02 14:30».
    pub fn note(&self, title: &str, source: &str, added: &str, duration: f64, model: &str) -> String {
        format!("---\nисточник: {source}\nзагружено: {added}\nдлительность: {}\nмодель: {model}\nслов: {}\n---\n\n{}",
            Self::clock(duration), self.words(), self.markdown(title))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_and_srt_like_mac() {
        let mut t = Transcript { segments: vec![
            Segment { start: 0.0, end: 4.2, text: "Привет.".into(), speaker: Some(0) },
            Segment { start: 4.2, end: 9.0, text: "Начнём.".into(), speaker: Some(0) },
            Segment { start: 3725.0, end: 3730.5, text: "Да.".into(), speaker: Some(1) },
        ], names: BTreeMap::new() };
        t.rename(0, " Макар ");
        assert_eq!(t.markdown("созвон"), "# созвон\n\n**00:00 Макар:** Привет.\n\n**00:04** Начнём.\n\n**1:02:05 спикер 2:** Да.\n");
        assert!(t.srt().starts_with("1\n00:00:00,000 --> 00:00:04,200\nПривет.\n"));
        assert!(t.note("созвон", "zoom.mp4", "2026-11-02 14:30", 3730.0, "gigaam").starts_with("---\nисточник: zoom.mp4\nзагружено: 2026-11-02 14:30\nдлительность: 1:02:10\nмодель: gigaam\nслов: 3\n---\n\n# созвон"));
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(serde_json::from_str::<Transcript>(&json).unwrap(), t);
    }
}
