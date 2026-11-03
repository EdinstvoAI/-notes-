//! Локальный MCP-сервер — порт `MCPServer.swift` (03.11.2026): нейронка (Claude, Codex, Cursor) работает
//! с хранилищем по stdio, JSON-RPC построчно, без сети. Пишет только новое — заметку или строку задачи;
//! чужой текст не переписывает. Коммит делает приложение, как после любой правки снаружи.

use crate::Vault;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::{BufRead, Write};
use std::path::Path;

pub const MAX_READ: usize = 200_000;
const VERSION: &str = "2025-06-18";
const WRITE_TOOLS: [&str; 2] = ["create_note", "add_task"];

pub struct Server<'a> {
    pub vault: &'a Vault,
    /// «Только чтение» в разделе MCP: инструменты записи не видны и не работают.
    pub allow_writes: bool,
    /// Папки верхнего уровня, закрытые для нейронок: их не видно ни в поиске, ни в списке, ни по пути.
    pub hidden: HashSet<String>,
    /// Журнал: инструмент, путь или запрос, длина ответа — без текста заметок.
    pub log: Option<Box<dyn Fn(&str, &str, usize) + 'a>>,
}

impl Server<'_> {
    /// Читает запросы до конца ввода: нейронка закрыла канал — выходим.
    pub fn run(&self, input: impl BufRead, mut output: impl Write) {
        for line in input.lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            if let Some(reply) = self.handle(&line) {
                let _ = writeln!(output, "{reply}");
                let _ = output.flush();
            }
        }
    }

    /// Одна строка запроса → одна строка ответа; None — уведомление.
    pub fn handle(&self, line: &str) -> Option<String> {
        let Ok(request) = serde_json::from_str::<Value>(line) else {
            return Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "parse error"}}).to_string());
        };
        let id = request.get("id")?.clone();
        let params = request.get("params").cloned().unwrap_or(json!({}));
        let mut reply = json!({"jsonrpc": "2.0", "id": id});
        match request["method"].as_str().unwrap_or("") {
            "initialize" => reply["result"] = json!({
                "protocolVersion": params["protocolVersion"].as_str().unwrap_or(VERSION),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "notes", "version": "1.0.0"},
                "instructions": "Markdown-заметки пользователя. Пути — от корня хранилища, с .md.",
            }),
            "ping" => reply["result"] = json!({}),
            "tools/list" => reply["result"] = json!({"tools": tools().into_iter()
                .filter(|t| self.allow_writes || !WRITE_TOOLS.contains(&t["name"].as_str().unwrap_or(""))).collect::<Vec<_>>()}),
            "tools/call" => {
                let name = params["name"].as_str().unwrap_or("");
                let args = &params["arguments"];
                let target = ["path", "note", "query", "title", "folder"].iter()
                    .find_map(|k| args[k].as_str()).unwrap_or("");
                let (text, error) = match self.call(name, args) {
                    Ok(t) => (t, false),
                    Err(e) => (e, true),
                };
                if let Some(log) = &self.log {
                    log(name, target, if error { 0 } else { text.chars().count() });
                }
                reply["result"] = json!({"content": [{"type": "text", "text": text}]});
                if error {
                    reply["result"]["isError"] = json!(true);
                }
            }
            other => reply["error"] = json!({"code": -32601, "message": format!("unknown method {other}")}),
        }
        Some(reply.to_string())
    }

    fn call(&self, name: &str, args: &Value) -> Result<String, String> {
        if WRITE_TOOLS.contains(&name) && !self.allow_writes {
            return Err("запись выключена: включено «только чтение»".into());
        }
        let s = |k: &str| args[k].as_str().unwrap_or("").to_string();
        let need = |k: &str| -> Result<String, String> {
            let v = s(k);
            if v.trim().is_empty() { Err(format!("нужен параметр {k}")) } else { Ok(v) }
        };
        match name {
            "search_notes" => {
                let limit = args["limit"].as_u64().unwrap_or(20) as usize;
                let hits: Vec<_> = self.vault.search(&need("query")?, 200).into_iter()
                    .filter(|h| self.inside(&h.note.path)).take(limit).collect();
                Ok(if hits.is_empty() { "ничего не нашлось".into() }
                   else { hits.iter().map(|h| format!("{} — {}", h.note.path, h.snippet)).collect::<Vec<_>>().join("\n") })
            }
            "list_notes" => {
                let folder = self.check(&s("folder"), true)?;
                let mut notes = vec![];
                collect(&self.vault.tree(), &mut notes);
                let prefix = if folder.is_empty() { String::new() } else { format!("{folder}/") };
                let mut notes: Vec<_> = notes.into_iter().filter(|n| n.path.starts_with(&prefix) && self.inside(&n.path)).collect();
                notes.sort_by(|a, b| b.modified.cmp(&a.modified));
                Ok(if notes.is_empty() { "заметок нет".into() } else { notes.iter().map(|n| n.path.clone()).collect::<Vec<_>>().join("\n") })
            }
            "read_note" => {
                let path = self.check(&need("path")?, false)?;
                let text = self.vault.read(&path).map_err(|e| e.to_string())?;
                Ok(if text.chars().count() > MAX_READ { text.chars().take(MAX_READ).collect::<String>() + "\n…(обрезано)" } else { text })
            }
            "create_note" => {
                let title = need("title")?.replace('/', "-");
                let folder = self.check(&s("folder"), true)?;
                std::fs::create_dir_all(self.vault.resolve(&folder).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                // «план 2» и т. д.: существующую заметку не перезаписываем никогда.
                let note = self.vault.create_note(&folder, &title).map_err(|e| e.to_string())?;
                self.vault.write(&note, &s("text")).map_err(|e| e.to_string())?;
                Ok(format!("создана {note}"))
            }
            "list_tasks" => {
                let all = args["include_done"].as_bool() == Some(true);
                let tasks: Vec<_> = self.vault.tasks().into_iter().filter(|(p, t)| (all || !t.done) && self.inside(p)).collect();
                Ok(if tasks.is_empty() { "задач нет".into() } else {
                    tasks.iter().map(|(p, t)| {
                        let due = t.date.as_ref().map(|d| format!(" 📅 {d}{}", t.time.as_ref().map(|x| format!(" {x}")).unwrap_or_default())).unwrap_or_default();
                        format!("{} {}{due} — {p}", if t.done { "[x]" } else { "[ ]" }, t.title)
                    }).collect::<Vec<_>>().join("\n")
                })
            }
            "add_task" => {
                let title = need("title")?;
                let note = self.check(&need("note")?, false)?;
                let due = s("due");
                let (date, time) = match parse_due(due.trim()) {
                    Some(d) => d,
                    None if due.trim().is_empty() => (None, None),
                    None => return Err("due: ГГГГ-ММ-ДД или ГГГГ-ММ-ДД ЧЧ:ММ".into()),
                };
                if let Some(dir) = self.vault.resolve(&note).map_err(|e| e.to_string())?.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                }
                self.vault.append_task(&note, &title, date.as_deref(), time.as_deref(), None).map_err(|e| e.to_string())?;
                Ok(format!("задача добавлена в {note}"))
            }
            _ => Err(format!("нет инструмента {name}")),
        }
    }

    /// Путь от корня. Наружу нельзя (`..`, абсолютный, ссылка наружу), скрытые и закрытые папки — тоже;
    /// файл — только `.md`. Возвращает чистый путь через `/`.
    pub fn check(&self, path: &str, directory: bool) -> Result<String, String> {
        let clean = path.replace('\\', "/").trim_matches(|c| c == '/' || c == ' ').to_string();
        let full = self.vault.resolve(&clean).map_err(|_| format!("путь вне хранилища: {path}"))?;
        if Path::new(&clean).is_absolute() || clean.contains(':') {
            return Err(format!("путь вне хранилища: {path}"));
        }
        // Ссылка внутри хранилища может вести наружу — проверяем и настоящий путь.
        if let (Ok(real), Ok(root)) = (full.canonicalize(), self.vault.root().canonicalize()) {
            if !real.starts_with(&root) {
                return Err(format!("путь вне хранилища: {path}"));
            }
        }
        if clean.split('/').any(|p| p.starts_with('.')) {
            return Err(format!("скрытый путь: {path}"));
        }
        let first = clean.split('/').next().unwrap_or("");
        if self.hidden.contains(first) && (clean.contains('/') || directory) {
            return Err(format!("папка закрыта для нейронок: {first}"));
        }
        if !directory && !clean.to_lowercase().ends_with(".md") {
            return Err(format!("нужен путь к .md: {path}"));
        }
        Ok(clean)
    }

    fn inside(&self, path: &str) -> bool {
        self.check(path, false).is_ok()
    }
}

fn collect(f: &crate::vault::Folder, out: &mut Vec<crate::vault::Note>) {
    out.extend(f.notes.iter().cloned());
    for sub in &f.folders {
        collect(sub, out);
    }
}

fn parse_due(text: &str) -> Option<(Option<String>, Option<String>)> {
    let ok_date = |d: &str| d.len() == 10 && d.as_bytes()[4] == b'-' && d.as_bytes()[7] == b'-' && d.chars().filter(|c| c.is_ascii_digit()).count() == 8;
    let ok_time = |t: &str| t.len() == 5 && t.as_bytes()[2] == b':' && t.chars().filter(|c| c.is_ascii_digit()).count() == 4;
    match text.split_once(' ') {
        Some((d, t)) if ok_date(d) && ok_time(t) => Some((Some(d.into()), Some(t.into()))),
        None if ok_date(text) => Some((Some(text.into()), None)),
        _ => None,
    }
}

fn tool(name: &str, about: &str, props: &[(&str, &str)], required: &[&str], flags: &[(&str, &str)]) -> Value {
    let mut p = serde_json::Map::new();
    for (k, d) in props { p.insert((*k).into(), json!({"type": "string", "description": d})); }
    for (k, d) in flags { p.insert((*k).into(), json!({"type": "boolean", "description": d})); }
    json!({"name": name, "description": about, "inputSchema": {"type": "object", "properties": p, "required": required}})
}

pub fn tools() -> Vec<Value> {
    vec![
        tool("search_notes", "Найти заметки по словам: все слова должны встретиться. Ответ — путь и отрывок.", &[("query", "слова для поиска")], &["query"], &[]),
        tool("list_notes", "Список заметок, свежие сверху.", &[("folder", "папка от корня; пусто — всё хранилище")], &[], &[]),
        tool("read_note", "Текст заметки целиком (Markdown).", &[("path", "путь к .md от корня")], &["path"], &[]),
        tool("create_note", "Создать новую заметку. Существующую не перезаписывает — добавит номер к названию.",
             &[("title", "название, оно же имя файла"), ("text", "Markdown"), ("folder", "папка от корня; пусто — корень")], &["title"], &[]),
        tool("list_tasks", "Задачи из всех заметок (строки «- [ ]»).", &[], &[], &[("include_done", "показать и выполненные")]),
        tool("add_task", "Дописать задачу в конец заметки; заметки нет — создаст.",
             &[("title", "текст задачи"), ("note", "путь к .md от корня"), ("due", "ГГГГ-ММ-ДД или ГГГГ-ММ-ДД ЧЧ:ММ")], &["title", "note"], &[]),
    ]
}
