//! Системные API Windows и macOS. Общая логика и интерфейс живут в Rust/Tauri;
//! Foundation Models, Secure Enclave и LocalAuthentication — за C ABI в `macos.rs`.

pub mod auth;
pub mod autostart;
pub mod calendar;
pub mod clipboard;
pub mod files;
pub mod paste;
pub mod secrets;
pub mod settings;
pub mod window;

#[cfg(target_os = "macos")]
pub mod macos;
