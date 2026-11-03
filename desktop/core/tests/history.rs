// По образцу NotesCore/Tests/NotesCoreTests/GitHistoryTests.swift.
use notes_core::git::{Change, History, Kind};
use std::fs;

fn vault() -> (tempfile::TempDir, History) {
    let d = tempfile::tempdir().unwrap();
    let (h, _) = History::prepare(d.path()).unwrap();
    for (f, t, m) in [("а.md", "первое", "а"), ("б.md", "второе", "б"), ("а.md", "первое правка", "правка а")] {
        fs::write(d.path().join(f), t).unwrap();
        h.commit_all(m).unwrap();
    }
    (d, h)
}
fn read(d: &tempfile::TempDir, f: &str) -> Option<String> { fs::read_to_string(d.path().join(f)).ok() }

#[test]
fn history_newest_first_with_files() {
    let (_d, h) = vault();
    let v = h.history(100).unwrap();
    assert_eq!(v.iter().take(3).map(|c| c.message.as_str()).collect::<Vec<_>>(), ["правка а", "б", "а"]);
    assert_eq!(v[1].files, [Change { path: "б.md".into(), kind: Kind::Added }]);
}

#[test]
fn revert_one_keeps_others() {
    let (d, h) = vault();
    h.revert(&h.history(9).unwrap()[1].id).unwrap();
    assert_eq!(read(&d, "б.md"), None);
    assert_eq!(read(&d, "а.md").as_deref(), Some("первое правка"));
    assert_eq!(h.history(1).unwrap()[0].message, "отменено: б");
}

#[test]
fn restore_keeps_unsaved_in_history() {
    let (d, h) = vault();
    fs::write(d.path().join("в.md"), "не сохранено").unwrap();
    h.restore(&h.history(9).unwrap()[2].id, "3 ноября 10:00").unwrap();
    assert_eq!(read(&d, "а.md").as_deref(), Some("первое"));
    assert_eq!(read(&d, "б.md"), None);
    assert_eq!(read(&d, "в.md"), None);
    let v = h.history(9).unwrap();
    assert_eq!(v[1].message, "перед откатом");
    h.revert(&v[0].id).unwrap();
    assert_eq!(read(&d, "в.md").as_deref(), Some("не сохранено"));
}

#[test]
fn revert_conflict_leaves_files() {
    let (d, h) = vault();
    assert!(h.revert(&h.history(9).unwrap()[2].id).is_err());
    assert_eq!(read(&d, "а.md").as_deref(), Some("первое правка"));
}

#[test]
fn one_file_at_version() {
    let (d, h) = vault();
    let old = h.history(9).unwrap()[2].id.clone();
    assert_eq!(h.file_at("а.md", &old).unwrap().as_deref(), Some("первое".as_bytes()));
    assert_eq!(h.file_at("б.md", &old).unwrap(), None);
    h.restore_file("а.md", &old).unwrap();
    assert_eq!(read(&d, "а.md").as_deref(), Some("первое"));
    assert_eq!(read(&d, "б.md").as_deref(), Some("второе"));
}

#[test]
fn restore_file_keeps_crlf() {
    let (d, h) = vault();
    fs::write(d.path().join("w.md"), "a\r\nb\r\n").unwrap();
    h.commit_all("crlf").unwrap();
    let id = h.history(1).unwrap()[0].id.clone();
    fs::write(d.path().join("w.md"), "x").unwrap();
    h.restore_file("w.md", &id).unwrap();
    assert_eq!(fs::read(d.path().join("w.md")).unwrap(), b"a\r\nb\r\n");
}
