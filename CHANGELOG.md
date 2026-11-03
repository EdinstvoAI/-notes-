# Changelog

## 1.1.0 — 2026-11-03

### Sync
- New Sync section: click the sync capsule in the header. Every saved version, grouped by day,
  drawn as a metro line.
- Graph view: the whole history on one board. Click a version to see its files, the file as it
  was then, and roll back. Pinch or ⌘+ / ⌘− to zoom.
- Undo one version, restore everything to a version, or bring back a single file. Every rollback
  is a new version on top, so it can be undone too. Works with GitHub or with local git only.

### AI agents
- Built-in MCP server: Claude, Codex and other agents can search and read your notes, create
  notes and add tasks. Local only, no network. Settings → General → copy command.

### Settings
- Interface language: system, Русский, English, Srpski.

### Calendar
- Events look like Apple Calendar: compact titles, start–end time, wider day columns.
- Overlapping events are layered so every title shows; full title on hover.

### Dictation
- The orb blurs its lines like frosted glass.

### Fixes
- Header section buttons no longer spill over the capsule edge.

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
