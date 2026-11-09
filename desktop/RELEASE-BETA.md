Beta of <notes> on Tauri, for Windows and macOS: one codebase for both. It opens the same vault as the Swift Mac version.
Plain Markdown files in a folder you own, git history, on-device dictation. It opens the same vault as the Mac version.

### What works
- Notes: folders, tabs, search (words and meaning), templates, daily note, drag and drop, Ctrl+Z, file preview.
- Tasks and calendar: day, week, month, drag and resize, export to .ics for Windows Calendar and Outlook.
- macOS: Apple Calendar. Your events and calendars in the app, tasks with a date sync to the calendar.
- Shared Edinstvo Glass materials and smooth animations across sections, calendar, onboarding and theme change.
- History in git and sync with a private GitHub repo (sign in with a code, no password). Conflicts keep both versions.
- Dictation with a hotkey (Ctrl+Alt+Space): GigaAM (Russian) or Whisper, on this computer. Video and audio transcripts with speakers.
- Sync section: versions, undo a version, restore a file or the whole vault, graph board.
- Password safe: passwords, 2FA codes, secrets. Stored locally outside the notes repository. Windows Hello or Touch ID, tray panel.
- MCP server for Claude Code, Codex and Cursor (`--mcp`), with skills.
- PIN and Windows Hello. English, Russian and Serbian.

### Install
**Windows 10/11, 64-bit:** run the `.exe` below. It is not signed yet: SmartScreen → **More info → Run anyway**.

**macOS:** open the `.dmg` and drag <notes> to Applications. Not notarized yet: System Settings → Privacy & Security → **Open Anyway**. If macOS still says the app is damaged, run `xattr -cr /Applications/<notes>.app` in Terminal. The macOS build includes Touch ID, Apple Calendar and a menu bar panel. Apple Intelligence uses the on-device system model when enabled.

This is a beta: please report what breaks.
