// Без консольного окна в релизной сборке Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `--mcp` — MCP-сервер для нейронок по stdio, без окна (как `<notes> --mcp` на маке).
    if std::env::args().any(|a| a == "--mcp") {
        return notes_desktop_lib::mcp::serve();
    }
    notes_desktop_lib::run()
}
