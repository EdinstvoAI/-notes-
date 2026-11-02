//! Звук из файла — видео (mp4, mov, m4v, 3gp) или аудио (m4a, mp3, wav, flac, aac) — в 16 кГц моно.
//! Чистый Rust (symphonia): без ffmpeg и системных кодеков, одинаково на Windows и маке.

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Расширения, которые принимает «перетащи файл» (как `VideoModel.types` на маке, без caf/aiff).
pub const TYPES: [&str; 11] = ["mp4", "mov", "m4v", "m4a", "m4b", "mp3", "wav", "aac", "flac", "3gp", "aif"];

pub fn decode(path: &std::path::Path) -> Result<Vec<f32>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|_| "этот файл не открылся: нет звуковой дорожки или формат не поддержан".to_string())?;
    let mut format = probed.format;
    let track = format.default_track().ok_or("в файле нет звука")?;
    let id = track.id;
    let rate = track.codec_params.sample_rate.ok_or("неизвестна частота звука")?;
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).map_err(|e| e.to_string())?;
    let mut mono = Vec::new();
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != id {
            continue;
        }
        let Ok(audio) = decoder.decode(&packet) else { continue };
        let spec = *audio.spec();
        let channels = spec.channels.count().max(1);
        let mut buf = SampleBuffer::<f32>::new(audio.capacity() as u64, spec);
        buf.copy_interleaved_ref(audio);
        mono.extend(buf.samples().chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32));
    }
    if mono.is_empty() {
        return Err("в файле нет звука".into());
    }
    Ok(resample(&mono, rate))
}

/// Линейная передискретизация в 16 кГц — для речи достаточно.
pub fn resample(input: &[f32], rate: u32) -> Vec<f32> {
    let target = crate::SAMPLE_RATE as f64;
    if rate as f64 == target {
        return input.to_vec();
    }
    let ratio = rate as f64 / target;
    (0..(input.len() as f64 / ratio) as usize)
        .map(|i| {
            let pos = i as f64 * ratio;
            let j = pos as usize;
            let a = input[j];
            let b = *input.get(j + 1).unwrap_or(&a);
            a + (b - a) * (pos - j as f64) as f32
        })
        .collect()
}
