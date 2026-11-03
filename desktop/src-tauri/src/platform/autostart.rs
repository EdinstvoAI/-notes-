//! «Открывать при входе» — на Windows ключ `Run` текущего пользователя (без прав администратора).
//! На маке — LaunchAgent в `~/Library/LaunchAgents` (свой файл, только для этого пользователя).

#[cfg(target_os = "windows")]
const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

#[cfg(target_os = "macos")]
fn agent() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join("Library/LaunchAgents/ai.edinstvo.notes.plist"))
}

pub fn enabled() -> bool {
    #[cfg(target_os = "macos")]
    {
        return agent().is_some_and(|p| p.exists());
    }
    #[cfg(target_os = "windows")]
    {
        return std::process::Command::new("reg").args(["query", KEY, "/v", "notes"]).output().is_ok_and(|o| o.status.success());
    }
    #[allow(unreachable_code)]
    false
}

pub fn set(on: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let path = agent().ok_or("нет HOME")?;
        if !on {
            let _ = std::fs::remove_file(&path);
            return Ok(());
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let esc = exe.display().to_string().replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
        let plist = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n<key>Label</key><string>ai.edinstvo.notes</string>\n<key>ProgramArguments</key><array><string>{esc}</string></array>\n<key>RunAtLoad</key><true/>\n</dict></plist>\n");
        if let Some(dir) = path.parent() { std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; }
        return std::fs::write(&path, plist).map_err(|e| e.to_string());
    }
    #[cfg(target_os = "windows")]
    {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut cmd = std::process::Command::new("reg");
        if on {
            cmd.args(["add", KEY, "/v", "notes", "/t", "REG_SZ", "/d", &format!("\"{}\"", exe.display()), "/f"]);
        } else {
            cmd.args(["delete", KEY, "/v", "notes", "/f"]);
        }
        let out = cmd.output().map_err(|e| e.to_string())?;
        return if out.status.success() || !on { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).into_owned()) };
    }
    #[allow(unreachable_code)]
    {
        let _ = on;
        Err("на этой ОС пока нет".into())
    }
}
