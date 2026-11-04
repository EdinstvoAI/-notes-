//! Ядро `<notes>` для Tauri: хранилище, заметки, задачи. Без кода под конкретную ОС —
//! всё системное живёт в `src-tauri/src/platform`.
//!
//! Договор с мак-версией (docs/WINDOWS.md, «Формат хранилища»): заметки — обычные `.md`, своих
//! служебных файлов в хранилище нет, концы строк сохраняются такими, какими пришли.

pub mod calplan;
pub mod git;
pub mod ics;
pub mod lock;
pub mod mcp;
pub mod names;
pub mod safe;
pub mod search;
pub mod tasks;
pub mod templates;
pub mod vault;

pub use search::Hit;
pub use tasks::{parse_task, Task};
pub use vault::{Entry, Folder, Note, Vault, VaultError};
pub use git2;
