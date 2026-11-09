//! «Вставлять в поле» — после надиктовки нажать Ctrl+V за человека, как AutoPaste на маке (там ⌘V).
//! На Windows — SendInput; разрешений, как «Универсальный доступ» на маке, не нужно.

pub fn ctrl_v() {
    #[cfg(target_os = "macos")]
    { let _ = super::macos::call(serde_json::json!({"command": "paste"})); }
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        let key = |vk: VIRTUAL_KEY, up: bool| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }, time: 0, dwExtraInfo: 0 } },
        };
        // Хоткей Ctrl+Alt+Space ещё может быть зажат — сначала отпускаем Alt, иначе выйдет Ctrl+Alt+V.
        let inputs = [key(VK_MENU, true), key(VK_CONTROL, false), key(VK_V, false), key(VK_V, true), key(VK_CONTROL, true)];
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}
