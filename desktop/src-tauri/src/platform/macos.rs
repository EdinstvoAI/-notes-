//! Узкий C ABI к системным фреймворкам Apple. Вызывать из фонового потока: Touch ID ждёт человека.
use std::ffi::{c_char, CStr, CString};
use serde_json::Value;

unsafe extern "C" {
    fn notes_mac_call(request: *const c_char) -> *mut c_char;
    fn notes_mac_free(response: *mut c_char);
}

pub fn call(request: Value) -> Result<Value, String> {
    let request = CString::new(request.to_string()).map_err(|e| e.to_string())?;
    unsafe {
        let pointer = notes_mac_call(request.as_ptr());
        if pointer.is_null() { return Err("системный API не ответил".into()); }
        let result = serde_json::from_slice::<Value>(CStr::from_ptr(pointer).to_bytes());
        notes_mac_free(pointer);
        let result = result.map_err(|e| e.to_string())?;
        if let Some(error) = result.get("error").and_then(Value::as_str) { return Err(error.into()); }
        result.get("value").cloned().ok_or_else(|| "пустой ответ системы".into())
    }
}
