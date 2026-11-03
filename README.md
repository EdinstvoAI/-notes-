<div align="center">

<img src="%3Cnotes%3E/Icon/icon-dark.png" width="112" alt="<notes>">

<h1>&lt;notes&gt;</h1>

<p><b>Your notes are files. Your history is git. Your voice stays on your Mac.</b></p>

<p>
A local-first notes app for macOS: plain Markdown in a folder you own,<br>
automatic version history, on-device dictation and tasks that live in your calendar.
</p>

<p>
<a href="https://github.com/EdinstvoAI/-notes-/releases/latest"><img src="https://img.shields.io/github/v/release/EdinstvoAI/-notes-?style=for-the-badge&labelColor=000000&color=1a1a1a&label=release" alt="Latest release"></a>
<img src="https://img.shields.io/badge/macOS-27%2B-1a1a1a?style=for-the-badge&labelColor=000000&logo=apple&logoColor=white" alt="macOS 27+">
<img src="https://img.shields.io/badge/Windows-beta-1a1a1a?style=for-the-badge&labelColor=000000&logo=windows11&logoColor=white" alt="Windows beta">
<a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-1a1a1a?style=for-the-badge&labelColor=000000" alt="MIT license"></a>
</p>

<p>
<a href="https://github.com/EdinstvoAI/-notes-/releases/latest"><b>Download for macOS</b></a>
&nbsp;·&nbsp;
<a href="https://github.com/EdinstvoAI/-notes-/releases/tag/win-v1.0.0-beta"><b>Windows beta</b></a>
&nbsp;·&nbsp;
<a href="#features">Features</a>
&nbsp;·&nbsp;
<a href="#build-from-source">Build</a>
&nbsp;·&nbsp;
<a href="CHANGELOG.md">Changelog</a>
</p>

</div>

<br>

## Principles

> **Files are the source of truth.** Every note is an ordinary `.md` file. Open the folder in any
> editor, today or in ten years.
>
> **Local-first.** No accounts, no servers of ours. The network is touched only for your own GitHub
> and one-time model downloads.
>
> **Small by default.** The app ships light. Speech models are downloaded on demand, only the ones
> you turn on.

## Features

### ✎ Notes
Markdown in a regular folder, with tabs, folders, drag and drop, pinning and templates.
A daily note opens on launch. ⌘Z undoes everything, from a typo to a note sent to Trash.

### ⟲ Version history
The vault is a git repository. Edits are committed automatically, so any note can be rolled back.
Sign in to GitHub with a short device code (no password) and the history goes to your private repo.
Two Macs on one vault? Changes merge, and on a conflict both versions are kept.

### ⌕ One search
⌘K finds notes, tasks, calendar events, videos and words spoken in transcripts, all in one list.

### ◉ Dictation, anywhere in macOS
Hold a hotkey, talk, release. The text is on your clipboard or typed into the focused field.
Recognition runs on your Mac: **GigaAM** for Russian with punctuation, **Whisper** for 99 languages.
A glass orb listens with you and reacts to your voice.

### ☐ Tasks that reach your calendar
Write `- [ ] call Anna 📅 2026-11-03 15:00` in any note and it becomes a task in Apple Calendar.
Day, week and month views: drag to create, move and resize.

### ▶ Video and audio transcripts
Drop a recording to transcribe it on-device, with speakers you can rename.
Save it as a note or export to `.srt`.

### ✦ Apple Intelligence <sub>optional</sub>
One-tap note summaries and tasks pulled out of a transcript, computed on-device.

### ⚿ Privacy
PIN code and Touch ID. Tokens and the PIN live in the Keychain, never in files.

<sub>Light and dark themes · English, Russian, Serbian (Latin)</sub>

## Platforms

| Platform | Status |
|---|---|
| macOS 27+ | ✅ Available, [download](https://github.com/EdinstvoAI/-notes-/releases/latest) |
| Windows 10/11 | 🧪 Beta, [download](https://github.com/EdinstvoAI/-notes-/releases/tag/win-v1.0.0-beta). Same vault format: one folder works on both |

## Install

1. Download **`notes.dmg`** from the [latest release](https://github.com/EdinstvoAI/-notes-/releases/latest).
2. Open it and drag **`<notes>`** into **Applications**.
3. This build is not notarized yet, so macOS asks once. Open
   **System Settings → Privacy & Security**, scroll down and click **Open Anyway**.

### Windows beta

1. Download the **`.exe` installer** from the [beta release](https://github.com/EdinstvoAI/-notes-/releases/tag/win-v1.0.0-beta).
2. The installer is not signed yet. If SmartScreen stops it, click **More info → Run anyway**.
3. Point it at the same folder you use on your Mac, or a new one.

The beta covers notes, git sync, search (also by meaning), tasks, calendar, dictation, transcripts and
Windows Hello. Apple Intelligence and Apple Calendar stay on macOS. Bugs → [issues](https://github.com/EdinstvoAI/-notes-/issues).

## How your vault looks

```text
my-notes/                    any folder, also a git repo
├── inbox.md                 quick tasks land here
├── projects/
│   └── launch.md            - [ ] ship 1.0 📅 2026-11-01 15:00
├── templates/               {{date}}, {{time}} and {{title}} are filled in
│   └── daily.md
└── transcripts/             transcripts saved as notes
```

<sub>Folder and file names above are examples. The app currently creates its own folders with Russian names.</sub>

Nothing else goes into this folder. The search index, caches and models live in
`~/Library/Application Support` and are rebuilt from your files at any time.

## Build from source

**Requirements:** macOS 27+ and the latest Xcode.

```bash
git clone https://github.com/EdinstvoAI/-notes-.git notes && cd notes
./Scripts/fetch-sherpa-onnx.sh   # speech runtime, ~100 MB, checksums verified
open "<notes>.xcodeproj"         # scheme <notes> → My Mac → ⌘R
```

```bash
cd NotesCore && swift test       # core tests
Scripts/make-dmg.sh              # installer → build/notes.dmg
```

**Windows** (Tauri 2, Rust + TypeScript), on the `Dev-Windows` branch:

```bash
cd desktop && npm ci
npx tauri build --bundles nsis   # installer → desktop/target/release/bundle/nsis
```

<details>
<summary><b>Project layout</b></summary>

<br>

| Path | What lives there |
|---|---|
| `<notes>/` | The macOS app: SwiftUI screens, dictation, calendar, Metal shaders |
| `NotesCore/` | Notes, tasks, git sync, search, templates, with tests |
| `NotesSpeech/` | On-device speech recognition and speaker detection |
| `Scripts/` | Speech runtime download, DMG installer |
| `desktop/` | Windows app on Tauri, in `Dev-Windows`; macOS moves here later |
| `docs/` | Architecture, decisions, process |

</details>

## Built with

[SwiftUI](https://developer.apple.com/xcode/swiftui/) ·
[libgit2](https://libgit2.org) ·
[sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) + GigaAM ·
[WhisperKit](https://github.com/argmaxinc/WhisperKit) ·
Foundation Models · EventKit · Metal ·
[Martian Grotesk](https://github.com/evilmartians/grotesk)

## License

Released under the [MIT License](LICENSE). Martian Grotesk is used under the SIL Open Font License 1.1.

<br>

<div align="center">
<sub>Designed and built by <a href="https://github.com/sharyzadizain">Makar K</a> at <a href="https://github.com/EdinstvoAI">Edinstvo AI</a></sub>
</div>
