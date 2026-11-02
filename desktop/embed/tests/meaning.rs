// Настоящая модель из NOTES_TEST_E5 (папка с model.safetensors, tokenizer.json, config.json). Без неё — пропуск.

use notes_embed::{Embedder, Index};

#[test]
fn finds_by_meaning_not_words() {
    let Ok(dir) = std::env::var("NOTES_TEST_E5") else { eprintln!("skip: NOTES_TEST_E5 not set"); return };
    let e = Embedder::load(std::path::Path::new(&dir)).unwrap();
    let notes = vec![
        ("счёт.md", "счёт клиенту", "Выставить счёт студии за макеты лендинга, 40 тысяч, оплата до пятницы."),
        ("рецепт.md", "борщ", "Свёкла, капуста, картошка, варить два часа."),
        ("спорт.md", "тренировки", "Понедельник — ноги, среда — спина, пятница — бег."),
    ];
    let rows = || notes.iter().map(|(p, t, x)| (p.to_string(), 1, t.to_string(), x.to_string()));
    let mut index = Index::default();
    assert_eq!(index.update(&e, rows()).unwrap(), 3);
    let hits = index.search(&e.query("сколько мне заплатят за дизайн").unwrap(), 3);
    println!("{hits:?}");
    assert_eq!(hits[0].0, "счёт.md", "no shared words, found by meaning");
    assert_eq!(index.update(&e, rows()).unwrap(), 0, "unchanged notes are not re-embedded");
}
