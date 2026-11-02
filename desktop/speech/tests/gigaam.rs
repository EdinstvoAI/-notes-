// Настоящее распознавание: модель — из NOTES_TEST_GIGAAM_ARCHIVE (скачанный архив). Без неё тест пропускается:
// модель 176 МБ в репозиторий не кладём.

use notes_speech::{GigaAm, Model, ModelStore};

#[test]
fn installs_and_transcribes_russian() {
    let Ok(archive) = std::env::var("NOTES_TEST_GIGAAM_ARCHIVE") else {
        eprintln!("skip: NOTES_TEST_GIGAAM_ARCHIVE not set");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let store = ModelStore::new(dir.path());
    assert!(!store.is_installed(Model::GigaAm));
    store.install_from_archive(Model::GigaAm, std::path::Path::new(&archive)).unwrap();
    assert!(store.is_installed(Model::GigaAm));

    let wave = sherpa_onnx::Wave::read(store.folder(Model::GigaAm).join("test_wavs/example.wav").to_str().unwrap()).unwrap();
    assert_eq!(wave.sample_rate(), 16_000);
    let model = GigaAm::load(&store.folder(Model::GigaAm)).unwrap();
    let text = model.transcribe(wave.samples());
    println!("распознано: {text}");
    assert!(text.chars().filter(|c| c.is_alphabetic()).count() > 10, "{text}");
    assert!(text.chars().any(|c| ('а'..='я').contains(&c.to_lowercase().next().unwrap())), "russian text: {text}");
    // Тот же файл через декодер медиа и с временем реплик.
    let samples = notes_speech::media::decode(&store.folder(Model::GigaAm).join("test_wavs/example.wav")).unwrap();
    assert_eq!(samples.len(), wave.samples().len());
    let segs = model.segments(&samples);
    assert!(!segs.is_empty() && segs[0].start == 0.0 && segs.last().unwrap().end > 5.0);
}

#[test]
fn bad_archive_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("fake.tar.bz2");
    std::fs::write(&fake, "не модель").unwrap();
    let store = ModelStore::new(dir.path().join("models"));
    std::fs::create_dir_all(&store.root).unwrap();
    assert!(store.install_from_archive(Model::GigaAm, &fake).unwrap_err().contains("контрольная сумма"));
    assert!(!store.is_installed(Model::GigaAm));
}

#[test]
fn speakers_on_real_models() {
    let (Ok(seg), Ok(emb)) = (std::env::var("NOTES_TEST_SEG_ARCHIVE"), std::env::var("NOTES_TEST_EMB")) else { return };
    let dir = tempfile::tempdir().unwrap();
    let s = notes_speech::speakers::Speakers::new(dir.path());
    s.install_from(std::path::Path::new(&seg), std::path::Path::new(&emb)).unwrap();
    assert!(s.is_installed());
    let wav = std::env::var("NOTES_TEST_WAV").unwrap();
    let samples = notes_speech::media::decode(std::path::Path::new(&wav)).unwrap();
    let turns = s.turns(&samples).unwrap();
    println!("turns: {turns:?}");
    assert!(!turns.is_empty());
}
