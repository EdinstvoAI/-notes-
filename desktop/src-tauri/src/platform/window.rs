//! Стекло под окном.
//!
//! Windows 11: Mica из `tauri.conf.json` (`windowEffects`), окно прозрачное, интерфейс кладёт свои
//! стеклянные слои поверх. macOS 26+: системный Liquid Glass — `tauri-plugin-liquid-glass`
//! (NSGlassEffectView), как у Swift-версии; на старых macOS плагин ничего не делает.

#[cfg(target_os = "macos")]
pub fn apply_glass(window: &tauri::WebviewWindow) {
    use tauri::Manager;
    use tauri_plugin_liquid_glass::{LiquidGlassConfig, LiquidGlassExt};
    let _ = window.app_handle().liquid_glass().set_effect(window, LiquidGlassConfig::default());
}

#[cfg(not(target_os = "macos"))]
pub fn apply_glass(_window: &tauri::WebviewWindow) {}
