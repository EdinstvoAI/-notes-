//! MCP для нейронок (порт `MCPScreen.swift` и `Entry` из MyApp.swift, 03.11.2026): `notes-desktop --mcp`
//! без окна берёт хранилище из настроек приложения и отвечает по stdio. Журнал — `mcp.log` рядом с
//! индексами (`%LOCALAPPDATA%\ai.edinstvo.notes\`): время, инструмент, длина ответа, путь — без текста.

use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;

const ID: &str = "ai.edinstvo.notes";

/// Те же папки, что `app_config_dir` и `app_local_data_dir` у Tauri, но без запущенного приложения:
/// `--mcp` окна не создаёт.
fn dirs() -> (PathBuf, PathBuf) {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let home = env("HOME").or_else(|| env("USERPROFILE")).unwrap_or_default();
    if cfg!(windows) {
        (env("APPDATA").unwrap_or_default().join(ID), env("LOCALAPPDATA").unwrap_or_default().join(ID))
    } else if cfg!(target_os = "macos") {
        let d = home.join("Library/Application Support").join(ID);
        (d.clone(), d)
    } else {
        (env("XDG_CONFIG_HOME").unwrap_or(home.join(".config")).join(ID), env("XDG_DATA_HOME").unwrap_or(home.join(".local/share")).join(ID))
    }
}

pub fn log_file() -> PathBuf {
    dirs().1.join("mcp.log")
}

fn prefs() -> serde_json::Value {
    std::fs::read_to_string(dirs().0.join("prefs.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Запуск `--mcp`: читает stdin до конца и выходит.
pub fn serve() {
    let Some(root) = std::fs::read_to_string(dirs().0.join("vault-path")).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) else {
        eprintln!("<notes>: хранилище не выбрано — открой приложение");
        std::process::exit(1);
    };
    let Ok(vault) = notes_core::Vault::open(&root) else {
        eprintln!("<notes>: папка хранилища не открылась: {root}");
        std::process::exit(1);
    };
    let p = prefs();
    let hidden: HashSet<String> = p["mcpHiddenFolders"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    let server = notes_core::mcp::Server {
        vault: &vault,
        allow_writes: p["mcpReadOnly"].as_bool() != Some(true),
        hidden,
        log: Some(Box::new(append)),
    };
    server.run(std::io::stdin().lock(), std::io::stdout().lock());
}

fn append(tool: &str, target: &str, chars: usize) {
    let file = log_file();
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(file) {
        let _ = writeln!(f, "{now}\t{tool}\t{chars}\t{}", target.replace(['\n', '\r'], " "));
    }
}

#[derive(serde::Serialize)]
pub struct Info {
    /// Путь к exe для команды подключения.
    exe: String,
    /// Последние 50 обращений, свежие сверху: (секунды, инструмент, путь).
    recent: Vec<(u64, String, String)>,
    /// ≈ токенов отдано: знаки / 3.
    tokens: usize,
}

#[tauri::command]
pub fn mcp_info() -> Info {
    let text = std::fs::read_to_string(log_file()).unwrap_or_default();
    let mut chars = 0;
    let mut all = vec![];
    for line in text.lines() {
        let p: Vec<&str> = line.split('\t').collect();
        let Some(time) = p.first().and_then(|t| t.parse::<u64>().ok()) else { continue };
        if p.len() >= 4 {
            chars += p[2].parse::<usize>().unwrap_or(0);
            all.push((time, p[1].to_string(), p[3..].join("\t")));
        } else if p.len() == 3 {
            all.push((time, p[1].to_string(), p[2].to_string()));
        }
    }
    let recent = all.into_iter().rev().take(50).collect();
    Info { exe: std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default(), recent, tokens: chars / 3 }
}

#[tauri::command]
pub fn mcp_clear_log() {
    let _ = std::fs::remove_file(log_file());
}

fn skill_dir(agent: &str, name: &str) -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)?;
    let base = match agent { "claude" => ".claude", "codex" => ".codex", _ => return None };
    // Имя скилла — только из нашего списка, но на всякий случай без путей.
    (!name.contains(['/', '\\', '.'])).then(|| home.join(base).join("skills").join(name))
}

/// Какие скиллы уже лежат у нейронки.
#[tauri::command]
pub fn mcp_skills(agent: String, names: Vec<String>) -> Vec<bool> {
    names.iter().map(|n| skill_dir(&agent, n).is_some_and(|d| d.join("SKILL.md").exists())).collect()
}

/// Положить скилл в `~/.claude/skills/<имя>/SKILL.md` или `~/.codex/skills/…`.
#[tauri::command]
pub fn mcp_install_skill(agent: String, name: String, title: String, about: String, text: String) -> Result<(), String> {
    let dir = skill_dir(&agent, &name).ok_or("нет папки скиллов")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let body = format!("---\nname: {name}\ndescription: {title} — {about}. Работает с заметками <notes> через MCP-сервер notes.\n---\n{text}\n");
    std::fs::write(dir.join("SKILL.md"), body).map_err(|e| e.to_string())
}
