# Changelog

## 1.1.4-beta — 2026-11-25

Tauri beta for macOS and Windows with shared Edinstvo Glass materials.

### Settings and models
- Settings open immediately while platform checks load independently, with timeout and retry.
- Installed dictation models are selected only when their files are present.
- Git, credential storage and biometric availability checks run outside the UI thread.

### Interface and onboarding
- Shared glass materials track moving controls and use the page background for sheets.
- Fix header sizing, tooltips near window edges, folder button size and onboarding transitions.
- Improve the GitHub sign-in card in dark mode and clarify CSV import.

### Windows and macOS
- Use the app header for window dragging on Windows and add branded NSIS installer artwork.
- Include macOS Touch ID, Apple Calendar and the menu bar panel in the shared Tauri app.
- Keep the password safe local and outside the notes repository on both platforms.

### Downloads
- Apple Silicon macOS: `.dmg`.
- Windows x64: `.exe`.
- These are beta builds: macOS is ad-hoc signed without notarization; Windows is unsigned.
- Physical Windows Hello and running the Windows installer remain manual checks.

## 1.0.0 — 2026-11-01

First release. A notes app for macOS 27+: plain Markdown files in a folder, history in git,
everything on your Mac.

### Notes
- Markdown notes in an ordinary folder that opens in any other editor.
- Tabs for open notes, folders, drag and drop, pin, duplicate, move, move to Trash.
- Templates and a daily note.
- ⌘Z for trash, move, duplicate, paste, new notes and task check-off.

### Sync
- The folder is a git repo: hourly commits, so any edit can be undone.
- Optional GitHub sign-in with a device code, no password: history goes to a private repo.
- Pulls changes from another Mac without losing text. On a conflict both versions are kept.

### Search
- One search (⌘K) for notes, tasks, events, videos and words in transcripts.
- Optional semantic search with the built-in Apple model.

### Dictation
- Hold a hotkey, talk, release: the text is on the clipboard.
- Optional paste into the focused text field.
- On-device speech recognition: GigaAM (Russian, with punctuation) or Whisper (99 languages).
  Models are downloaded on demand.
- An animated glass orb that reacts to your voice.

### Tasks and calendar
- `- [ ] task 📅 date` lines in notes become tasks. Dated tasks go to Apple Calendar.
- Day, week and month views: drag to create, move and resize, calendar colors, now line.
- Create, rename, recolor and delete calendars.

### Video and audio
- Drop a video or audio file to transcribe it on this Mac.
- Speakers: who says what, with renaming.
- Save a transcript as a note. Export to .srt. Folders for transcripts.

### Apple Intelligence (optional)
- Note summaries.
- Tasks from a transcript, sent to the calendar.

### Privacy and look
- PIN code and Touch ID. Tokens and PIN live in the Keychain.
- No servers or accounts of our own. The network is used only for GitHub and model downloads.
- Light and dark theme, app icon follows the theme.
- English, Russian and Serbian (Latin).
