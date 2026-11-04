//! План синхронизации задач с календарём — порт `CalendarPlan` (NotesCore/NoteTasks.swift): что создать,
//! обновить и удалить, чтобы события совпали с задачами. Сам EventKit — в приложении, здесь только решение.
//! Связи «задача → событие» живут вне хранилища: в заметки служебное не пишем.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Задача с датой, как её видит календарь. `key` — путь заметки + «|» + название, как `NoteTask.key`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Wanted {
    pub key: String,
    pub title: String,
    /// Начало, мс от 1970 (местное время уже пересчитано вызывающим).
    pub start: f64,
    pub all_day: bool,
    pub minutes: Option<u32>,
}

/// Что сейчас в событии.
#[derive(Debug, Clone, PartialEq)]
pub struct Current {
    pub title: String,
    pub start: f64,
    pub all_day: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Create(Wanted),
    Update(String, Wanted),
    /// Ключ задачи и id события.
    Delete(String, String),
}

/// `links` — ключ задачи → id события; `existing` — id события → что в нём (None — удалили руками).
pub fn actions(tasks: &[Wanted], links: &BTreeMap<String, String>, existing: &BTreeMap<String, Option<Current>>) -> Vec<Action> {
    let mut out = Vec::new();
    let mut wanted = BTreeSet::new();
    for t in tasks {
        wanted.insert(t.key.clone());
        match links.get(&t.key).and_then(|id| existing.get(id).cloned().flatten().map(|c| (id, c))) {
            Some((id, c)) => {
                // Длительность не сравниваем: у события её нет в «что сейчас» — иначе правили бы каждый раз.
                if c.title != t.title || (c.start - t.start).abs() > 500.0 || c.all_day != t.all_day {
                    out.push(Action::Update(id.clone(), t.clone()));
                }
            }
            // Связи нет или событие удалили в Календаре — создаём заново.
            None => out.push(Action::Create(t.clone())),
        }
    }
    for (key, id) in links {
        if !wanted.contains(key) {
            out.push(Action::Delete(key.clone(), id.clone()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(key: &str, start: f64) -> Wanted {
        Wanted { key: key.into(), title: key.into(), start, all_day: false, minutes: None }
    }

    #[test]
    fn creates_updates_and_deletes() {
        let links = BTreeMap::from([("a".to_string(), "e1".to_string()), ("b".to_string(), "e2".to_string()), ("gone".to_string(), "e3".to_string())]);
        let existing = BTreeMap::from([
            ("e1".to_string(), Some(Current { title: "a".into(), start: 1000.0, all_day: false })),
            ("e2".to_string(), Some(Current { title: "b".into(), start: 1000.0, all_day: false })),
            ("e3".to_string(), None),
        ]);
        let got = actions(&[w("a", 1000.0), w("b", 5000.0), w("new", 1.0)], &links, &existing);
        assert_eq!(got, vec![
            Action::Update("e2".into(), w("b", 5000.0)),
            Action::Create(w("new", 1.0)),
            Action::Delete("gone".into(), "e3".into()),
        ]);
    }

    #[test]
    fn recreates_event_removed_by_hand() {
        let links = BTreeMap::from([("a".to_string(), "e1".to_string())]);
        let existing = BTreeMap::from([("e1".to_string(), None)]);
        assert_eq!(actions(&[w("a", 1.0)], &links, &existing), vec![Action::Create(w("a", 1.0))]);
    }
}
