// По образцу NotesCore/Tests/NotesCoreTests/MCPServerTests.swift.
use notes_core::mcp::Server;
use notes_core::Vault;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;

fn vault() -> (tempfile::TempDir, Vault) {
    let d = tempfile::tempdir().unwrap();
    fs::create_dir_all(d.path().join("проекты")).unwrap();
    fs::write(d.path().join("проекты/план.md"), "# План\nкупить хлеб").unwrap();
    let v = Vault::open(d.path()).unwrap();
    (d, v)
}
fn server(v: &Vault) -> Server<'_> { Server { vault: v, allow_writes: true, hidden: HashSet::new(), log: None } }
fn call(s: &Server, name: &str, args: Value) -> (String, bool) {
    let r: Value = serde_json::from_str(&s.handle(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}}).to_string()).unwrap()).unwrap();
    (r["result"]["content"][0]["text"].as_str().unwrap().to_string(), r["result"]["isError"] == json!(true))
}

#[test]
fn handshake() {
    let (_d, v) = vault();
    let s = server(&v);
    assert!(s.handle(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#).unwrap().contains("serverInfo"));
    assert!(s.handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
    assert!(s.handle(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap().contains("search_notes"));
}

#[test]
fn tools() {
    let (d, v) = vault();
    let s = server(&v);
    assert!(call(&s, "search_notes", json!({"query":"хлеб"})).0.contains("проекты/план.md"));
    assert_eq!(call(&s, "read_note", json!({"path":"проекты/план.md"})).0, "# План\nкупить хлеб");
    assert!(call(&s, "create_note", json!({"title":"план","folder":"проекты","text":"новое"})).0.contains("план 2.md"));
    assert_eq!(fs::read_to_string(d.path().join("проекты/план.md")).unwrap(), "# План\nкупить хлеб");
    call(&s, "add_task", json!({"title":"позвонить","note":"проекты/план.md","due":"2026-11-04 15:00"}));
    assert!(call(&s, "list_tasks", json!({})).0.contains("[ ] позвонить 📅 2026-11-04 15:00"));
}

#[test]
fn sandbox() {
    let (_d, v) = vault();
    let s = server(&v);
    for p in ["../секрет.md", ".git/config.md", ".safe/safe.md", "проекты/план.txt", "C:/Windows/x.md", "проекты\\..\\..\\x.md"] {
        assert!(call(&s, "read_note", json!({"path": p})).1, "{p}");
    }
}

#[cfg(unix)]
#[test]
fn symlink_outside_not_read() {
    let (d, v) = vault();
    let out = tempfile::tempdir().unwrap();
    fs::write(out.path().join("s.md"), "секрет").unwrap();
    std::os::unix::fs::symlink(out.path().join("s.md"), d.path().join("ссылка.md")).unwrap();
    let s = server(&v);
    assert!(call(&s, "read_note", json!({"path":"ссылка.md"})).1);
    assert!(!call(&s, "search_notes", json!({"query":"секрет"})).0.contains("секрет"));
}

#[test]
fn read_only_and_log_without_text() {
    let (d, v) = vault();
    let lines = RefCell::new(Vec::<String>::new());
    let s = Server { vault: &v, allow_writes: false, hidden: HashSet::new(), log: Some(Box::new(|t, p, _| lines.borrow_mut().push(format!("{t} {p}")))) };
    assert!(!s.handle(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap().contains("create_note"));
    assert!(call(&s, "create_note", json!({"title":"x","text":"тайна"})).1);
    assert!(!d.path().join("x.md").exists());
    call(&s, "read_note", json!({"path":"проекты/план.md"}));
    assert_eq!(lines.borrow().last().unwrap(), "read_note проекты/план.md");
    assert!(!lines.borrow().join("").contains("тайна"));
}

#[test]
fn hidden_folder() {
    let (_d, v) = vault();
    let s = Server { vault: &v, allow_writes: true, hidden: HashSet::from(["проекты".to_string()]), log: None };
    assert_eq!(call(&s, "search_notes", json!({"query":"хлеб"})).0, "ничего не нашлось");
    assert!(call(&s, "read_note", json!({"path":"проекты/план.md"})).1);
    assert!(call(&s, "create_note", json!({"title":"x","folder":"проекты"})).1);
    assert!(!call(&s, "list_notes", json!({})).0.contains("проекты"));
}
