// Поведение — по образцу NotesCore/Tests/NotesCoreTests/NoteLibraryTests.swift.

use notes_core::{Vault, VaultError};
use std::fs;

fn vault() -> (tempfile::TempDir, Vault) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("проекты/вложенная")).unwrap();
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join(".git/config"), "x").unwrap();
    fs::write(root.join("Входящие.md"), "- [ ] купить хлеб 📅 2026-11-02\n").unwrap();
    fs::write(root.join("проекты/план 10.md"), "# План\n").unwrap();
    fs::write(root.join("проекты/план 2.md"), "- [x] сделано\n").unwrap();
    fs::write(root.join("проекты/вложенная/идея.md"), "").unwrap();
    fs::write(root.join("проекты/фото.png"), [0u8]).unwrap();
    fs::write(root.join("проекты/скрипт.sh"), "").unwrap();
    let vault = Vault::open(root).unwrap();
    (dir, vault)
}

#[test]
fn tree_skips_hidden_and_counts_notes() {
    let (_d, v) = vault();
    let tree = v.tree();
    assert_eq!(tree.folders.len(), 1, ".git hidden");
    assert_eq!(tree.note_count, 4);
    let projects = &tree.folders[0];
    assert_eq!(projects.path, "проекты");
    let titles: Vec<_> = projects.notes.iter().map(|n| n.title.as_str()).collect();
    assert_eq!(titles, ["план 2", "план 10"], "natural order like Finder");
    assert_eq!(projects.files.len(), 1, "png shown, sh skipped");
    assert_eq!(projects.folders[0].notes[0].path, "проекты/вложенная/идея.md");
}

#[test]
fn write_keeps_line_endings() {
    let (d, v) = vault();
    fs::write(d.path().join("crlf.md"), "a\r\nb\r\n").unwrap();
    v.write("crlf.md", "a\nb\nc\n").unwrap();
    assert_eq!(fs::read(d.path().join("crlf.md")).unwrap(), b"a\r\nb\r\nc\r\n");

    v.write("Входящие.md", "x\r\ny\r\n").unwrap();
    assert_eq!(fs::read_to_string(d.path().join("Входящие.md")).unwrap(), "x\ny\n", "LF stays LF");
}

#[test]
fn no_escape_from_vault() {
    let (_d, v) = vault();
    assert!(matches!(v.read("../secret.md"), Err(VaultError::OutsideVault)));
    assert!(matches!(v.write("../x.md", ""), Err(VaultError::OutsideVault)));
    assert!(matches!(v.read("нет.md"), Err(VaultError::NotFound)));
}

#[test]
fn create_note_picks_free_name() {
    let (_d, v) = vault();
    assert_eq!(v.create_note("проекты", "план 2").unwrap(), "проекты/план 2 2.md");
    assert_eq!(v.create_note("", "Новая заметка").unwrap(), "Новая заметка.md");
    assert_eq!(v.create_note("", "Новая заметка").unwrap(), "Новая заметка 2.md");
    assert!(matches!(v.create_note("", "a:b"), Err(VaultError::BadName(_))));
}

#[test]
fn tasks_from_all_notes() {
    let (_d, v) = vault();
    let tasks = v.tasks();
    assert_eq!(tasks.len(), 2);
    let bread = tasks.iter().find(|(p, _)| p == "Входящие.md").unwrap();
    assert_eq!(bread.1.date.as_deref(), Some("2026-11-02"));
    assert!(tasks.iter().any(|(p, t)| p == "проекты/план 2.md" && t.done));
}

#[test]
fn rename_keeps_folder_and_avoids_clash() {
    let (d, v) = vault();
    assert_eq!(v.rename("проекты/план 10.md", "план 2").unwrap(), "проекты/план 2 2.md");
    assert!(d.path().join("проекты/план 2 2.md").exists());
    assert_eq!(v.rename("проекты/план 2 2.md", "  итог ").unwrap(), "проекты/итог.md");
    assert_eq!(v.rename("проекты/итог.md", "итог").unwrap(), "проекты/итог.md", "same name is a no-op");
    assert!(matches!(v.rename("проекты/итог.md", "a/b"), Err(VaultError::BadName(_))));
}

#[test]
fn search_all_words_title_first() {
    let (d, v) = vault();
    fs::write(d.path().join("хлеб.md"), "просто так").unwrap();
    let hits = v.search("ХЛЕБ", 10);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].note.title, "хлеб", "title match ranks higher");
    assert_eq!(hits[1].snippet, "купить хлеб");
    assert!(v.search("хлеб сделано", 10).is_empty(), "all words must match");
    assert!(v.search("  ", 10).is_empty());
}

#[test]
fn duplicate_names_like_mac() {
    let (d, v) = vault();
    assert_eq!(v.duplicate("Входящие.md").unwrap(), "Входящие копия.md");
    assert_eq!(v.duplicate("Входящие.md").unwrap(), "Входящие копия 2.md");
    assert_eq!(fs::read_to_string(d.path().join("Входящие копия.md")).unwrap(), "- [ ] купить хлеб 📅 2026-11-02\n");
    assert_eq!(v.duplicate("проекты/вложенная").unwrap(), "проекты/вложенная копия");
    assert!(d.path().join("проекты/вложенная копия/идея.md").exists(), "folder copied deep");
}

#[test]
fn move_into_folder() {
    let (d, v) = vault();
    assert_eq!(v.move_into("Входящие.md", "проекты").unwrap().as_deref(), Some("проекты/Входящие.md"));
    assert!(!d.path().join("Входящие.md").exists());
    assert_eq!(v.move_into("проекты/Входящие.md", "проекты").unwrap(), None, "same folder");
    assert_eq!(v.move_into("проекты", "проекты/вложенная").unwrap(), None, "not into itself");
    fs::write(d.path().join("идея.md"), "").unwrap();
    assert_eq!(v.move_into("идея.md", "проекты/вложенная").unwrap().as_deref(), Some("проекты/вложенная/идея 2.md"));
    assert_eq!(v.move_into("проекты/вложенная", "").unwrap().as_deref(), Some("вложенная"));
}

#[test]
fn create_folder_and_copy_in() {
    let (d, v) = vault();
    assert_eq!(v.create_folder("", "проекты").unwrap(), "проекты 2");
    let outside = tempfile::tempdir().unwrap();
    let file = outside.path().join("скан.pdf");
    fs::write(&file, "pdf").unwrap();
    assert_eq!(v.copy_in(&file, "проекты").unwrap(), "проекты/скан.pdf");
    assert_eq!(v.copy_in(&file, "проекты").unwrap(), "проекты/скан копия.pdf");
    assert!(d.path().join("проекты/скан копия.pdf").exists());
}

#[test]
fn templates_and_daily() {
    let (d, v) = vault();
    assert!(v.templates().is_empty());
    let daily = v.daily("2026-11-01", "09:00").unwrap();
    assert_eq!(daily, "дневник/2026-11-01.md");
    assert_eq!(fs::read_to_string(d.path().join(&daily)).unwrap(), "# 2026-11-01\n\n");
    fs::write(d.path().join(&daily), "мой текст").unwrap();
    assert_eq!(v.daily("2026-11-01", "10:00").unwrap(), daily);
    assert_eq!(fs::read_to_string(d.path().join(&daily)).unwrap(), "мой текст", "existing daily untouched");

    fs::create_dir_all(d.path().join("шаблоны")).unwrap();
    fs::write(d.path().join("шаблоны/встреча.md"), "# {{название}}\n{{дата}} {{время}}\n").unwrap();
    fs::write(d.path().join("шаблоны/день.md"), "день {{дата}}").unwrap();
    assert_eq!(v.templates(), ["встреча", "день"]);
    let a = v.create_from_template("встреча", "проекты", "2026-11-02", "15:30").unwrap();
    let b = v.create_from_template("встреча", "проекты", "2026-11-02", "15:30").unwrap();
    assert_eq!((a.as_str(), b.as_str()), ("проекты/встреча.md", "проекты/встреча 2.md"));
    assert_eq!(fs::read_to_string(d.path().join(&b)).unwrap(), "# встреча 2\n2026-11-02 15:30\n");
    assert_eq!(fs::read_to_string(d.path().join(v.daily("2026-11-03", "08:00").unwrap())).unwrap(), "день 2026-11-03");
}

#[test]
fn edit_task_lines() {
    let (d, v) = vault();
    let (_, bread) = v.tasks().into_iter().find(|(p, _)| p == "Входящие.md").unwrap();
    v.set_due("Входящие.md", bread.line, &bread.raw, Some("2026-11-05"), Some("15:00"), Some(90)).unwrap();
    assert_eq!(fs::read_to_string(d.path().join("Входящие.md")).unwrap(), "- [ ] купить хлеб 📅 2026-11-05 15:00–16:30\n");
    assert!(matches!(v.set_due("Входящие.md", 0, &bread.raw, None, None, None), Err(VaultError::ChangedOnDisk)), "stale raw");
    let (_, now) = v.tasks().into_iter().find(|(p, _)| p == "Входящие.md").unwrap();
    v.set_done("Входящие.md", now.line, &now.raw, true).unwrap();
    v.append_task("Входящие.md", " позвонить ", Some("2026-11-06"), None, None).unwrap();
    v.append_task("новое.md", "первая", None, None, None).unwrap();
    assert_eq!(fs::read_to_string(d.path().join("Входящие.md")).unwrap(),
        "- [x] купить хлеб 📅 2026-11-05 15:00–16:30\n- [ ] позвонить 📅 2026-11-06\n");
    assert_eq!(fs::read_to_string(d.path().join("новое.md")).unwrap(), "- [ ] первая\n");
}

#[test]
fn rename_and_delete_task() {
    let (d, v) = vault();
    fs::write(d.path().join("Входящие.md"), "- [ ] старое !! 📅 2026-11-02 10:00\n- [ ] второе\n").unwrap();
    let (_, t) = v.tasks().into_iter().find(|(p, t)| p == "Входящие.md" && t.line == 0).unwrap();
    v.set_task_title("Входящие.md", 0, &t.raw, " новое ").unwrap();
    assert_eq!(fs::read_to_string(d.path().join("Входящие.md")).unwrap(), "- [ ] новое !! 📅 2026-11-02 10:00\n- [ ] второе\n");
    v.delete_task("Входящие.md", 0, "- [ ] новое !! 📅 2026-11-02 10:00").unwrap();
    assert_eq!(fs::read_to_string(d.path().join("Входящие.md")).unwrap(), "- [ ] второе\n");
}
