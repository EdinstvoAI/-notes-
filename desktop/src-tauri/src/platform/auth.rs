//! Вход через Windows Hello или LocalAuthentication (Touch ID / пароль Mac).

/// Есть ли на компьютере Windows Hello (камера или сканер пальца и настроенный вход).
pub fn available() -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows::Security::Credentials::UI::{UserConsentVerifier, UserConsentVerifierAvailability};
        return UserConsentVerifier::CheckAvailabilityAsync()
            .and_then(|op| op.get())
            .map(|a| a == UserConsentVerifierAvailability::Available)
            .unwrap_or(false);
    }
    #[cfg(target_os = "macos")]
    { return super::macos::call(serde_json::json!({"command": "auth_available"})).ok().and_then(|v| v.as_bool()).unwrap_or(false); }
    #[allow(unreachable_code)]
    false
}

/// Спросить Windows Hello поверх нашего окна. Обычный вызов без окна у программ не из Store прячет
/// системное окно за приложением — поэтому через `IUserConsentVerifierInterop` с HWND.
pub fn verify(window: &tauri::WebviewWindow, message: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows::core::{factory, HSTRING};
        use windows::Foundation::IAsyncOperation;
        use windows::Security::Credentials::UI::{UserConsentVerificationResult, UserConsentVerifier};
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::WinRT::IUserConsentVerifierInterop;

        let Ok(handle) = window.window_handle() else { return false };
        let RawWindowHandle::Win32(win32) = handle.as_raw() else { return false };
        let hwnd = HWND(win32.hwnd.get() as *mut core::ffi::c_void);
        let run = || -> windows::core::Result<bool> {
            let interop = factory::<UserConsentVerifier, IUserConsentVerifierInterop>()?;
            let op: IAsyncOperation<UserConsentVerificationResult> =
                unsafe { interop.RequestVerificationForWindowAsync(hwnd, &HSTRING::from(message))? };
            Ok(op.get()? == UserConsentVerificationResult::Verified)
        };
        return run().unwrap_or(false);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = window;
        return super::macos::call(serde_json::json!({"command": "auth_verify", "text": message})).ok().and_then(|v| v.as_bool()).unwrap_or(false);
    }
    #[allow(unreachable_code)]
    {
        let _ = (window, message);
        false
    }
}
