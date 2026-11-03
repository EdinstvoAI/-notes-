//! Скрытый буфер обмена для сейфа (на маке — `ConcealedType`/`TransientType` и только этот Mac).
//! Windows: вместе с текстом кладём форматы, по которым журнал буфера (Win+V), облачный буфер и
//! менеджеры буфера текст не сохраняют. Через 60 с стираем, если в буфере всё ещё наше.

const CLEAR_AFTER: std::time::Duration = std::time::Duration::from_secs(60);

#[cfg(windows)]
pub fn concealed(_app: &tauri::AppHandle, text: &str) -> Result<(), String> {
    use windows::core::w;
    use windows::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, GetClipboardSequenceNumber, OpenClipboard, RegisterClipboardFormatW, SetClipboardData};
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    const CF_UNICODETEXT: u32 = 13;

    unsafe fn put(format: u32, bytes: &[u8]) -> windows::core::Result<()> {
        let mem: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1))?;
        let ptr = GlobalLock(mem) as *mut u8;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        let _ = GlobalUnlock(mem);
        SetClipboardData(format, HANDLE(mem.0))?;
        Ok(())
    }

    let wide: Vec<u8> = text.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
    let sequence = unsafe {
        OpenClipboard(None).map_err(|e| e.to_string())?;
        let result = (|| -> windows::core::Result<()> {
            EmptyClipboard()?;
            put(CF_UNICODETEXT, &wide)?;
            put(RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing")), &[0])?;
            put(RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory")), &0u32.to_le_bytes())?;
            put(RegisterClipboardFormatW(w!("CanUploadToCloudClipboard")), &0u32.to_le_bytes())?;
            Ok(())
        })();
        let _ = CloseClipboard();
        result.map_err(|e| e.to_string())?;
        GetClipboardSequenceNumber()
    };
    std::thread::spawn(move || {
        std::thread::sleep(CLEAR_AFTER);
        unsafe {
            if GetClipboardSequenceNumber() == sequence && OpenClipboard(None).is_ok() {
                let _ = EmptyClipboard();
                let _ = CloseClipboard();
            }
        }
    });
    Ok(())
}

#[cfg(not(windows))]
pub fn concealed(app: &tauri::AppHandle, text: &str) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().write_text(text.to_string()).map_err(|e| e.to_string())?;
    let (app, text) = (app.clone(), text.to_string());
    std::thread::spawn(move || {
        std::thread::sleep(CLEAR_AFTER);
        if app.clipboard().read_text().ok().as_deref() == Some(&text) {
            let _ = app.clipboard().write_text(String::new());
        }
    });
    Ok(())
}

/// Экран заблокирован: на заблокированном рабочем столе ввод недоступен — `OpenInputDesktop` не открывается.
#[cfg(windows)]
pub fn screen_locked() -> bool {
    use windows::Win32::System::StationsAndDesktops::{CloseDesktop, OpenInputDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_SWITCHDESKTOP};
    unsafe {
        match OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_SWITCHDESKTOP) {
            Ok(d) => { let _ = CloseDesktop(d); false }
            Err(_) => true,
        }
    }
}

#[cfg(not(windows))]
pub fn screen_locked() -> bool {
    false
}
