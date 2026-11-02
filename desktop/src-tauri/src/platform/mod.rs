//! Всё, что зависит от ОС. Общий код зовёт только эти функции; реализация — за `cfg(target_os)`.
//! Мак пока на Swift-версии, поэтому macOS-ветки здесь — заготовки, но проект собирается на обеих ОС
//! с первого дня (docs/WINDOWS.md, «Архитектура»).
//!
//! Дальше сюда приедут: секреты (Credential Manager / Связка ключей), вход по лицу или пальцу
//! (Windows Hello / Touch ID), глобальный хоткей, вставка текста в чужое поле, календарь.

pub mod auth;
pub mod autostart;
pub mod files;
pub mod paste;
pub mod secrets;
pub mod settings;
pub mod window;
