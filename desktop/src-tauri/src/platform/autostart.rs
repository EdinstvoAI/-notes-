//! «Открывать при входе» — на Windows ключ `Run` текущего пользователя (без прав администратора).
//! На маке — при переезде (SMAppService / LaunchAgent), до тех пор недоступно.

#[cfg(target_os = "windows")]
const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

pub fn enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        return std::process::Command::new("reg").args(["query", KEY, "/v", "notes"]).output().is_ok_and(|o| o.status.success());
    }
    #[allow(unreachable_code)]
    false
}

pub fn set(on: bool) -> Result<(), String> {
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
