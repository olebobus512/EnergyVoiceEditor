# Codebase audit (Phase 1)

Date of audit: 8 November 2026. Method: reading the whole repository and running its test suites. Nothing here comes from running the app on Windows (see "What was not verified").

## 1. Structure

```
EnergyVoiceEditor/
├── AGENTS.md                  working rules for assistants
├── EnergyVoiceEditor/         the application
│   ├── src/                   Vue 3 + TypeScript frontend (≈6k lines incl. 900 lines of translations)
│   ├── src-tauri/             Rust core (Tauri 2, edition 2024)
│   ├── engines/python-runtime Python worker + speech engines
│   ├── Documentation/         this folder
│   └── tools/                 development tools (example voice bank builder)
```

## 2. Technology in use

| Layer | Technology | Notes |
|---|---|---|
| Shell | Tauri 2, WebView2 on Windows | plugins: opener, dialog |
| Frontend | **Vue 3**, TypeScript, Vite, plain CSS | not React (see ADR-001 in SYSTEM_ARCHITECTURE.md) |
| Core | Rust 2024; crates: tauri, serde, serde_json, zip, sha2 | no audio device crate, no resampling crate |
| ML/TTS | Python 3 worker, stdlib only | engines: Windows SAPI via PowerShell, Piper (CLI, optional), debug tones |
| Storage | JSON files (`settings.json`, `.eveproj`) | the project description mentions SQLite; not used |

## 3. What exists

**Frontend** (`src/`)
- Track timeline (`Arrangement.vue`): tracks with per-track colour and height, reorder by drag, clips draggable between tracks, Ctrl+wheel zoom, ruler seeking.
- Project model (`project.ts`): version-1 JSON, undo/redo (snapshots, 100 steps), clipboard, solo/mute/volume/pan.
- Voice mode: text → speech through the Python worker, clip placed on a voice track.
- Music mode: **only an instrument keyboard/fretboard** (`InstrumentPanel.vue`, `instruments.ts`). There is no Piano Roll note editor, no notes, no lyrics, no phonemes.
- Menus, right-click menus (`ContextMenu.vue`), shortcut table, 4 languages, theme presets, settings dialog, console panel with a PowerShell session.
- Voicebank Manager (`VoicebankManager.vue`): new in this iteration.

**Rust core** (`src-tauri/src/`)
- `audio.rs`: WAV parse/write (PCM 16/24/32, float), mixing to stereo with pan, linear-interpolation resampling. `render_mix` renders the timeline to a WAV; the **webview plays it with an `<audio>` element**. There is no Rust playback engine and no audio device access.
- `worker.rs`: Python worker over JSON lines on stdin/stdout, bounded shutdown.
- `commands.rs`, `settings.rs`, `project_io.rs`, `shell.rs`.
- `evbank/` + `voicebank_commands.rs`: EVBANK loader, store and commands (new, tested).

**Python** (`engines/python-runtime/`): `main.py` protocol, three engines, 26 unit tests.

## 4. Gaps against the master prompt

| Required | State |
|---|---|
| Piano Roll, notes, lyrics, phoneme editor, expression curves, MIDI | not started |
| Phonemizer API and language modules | not started |
| Concatenative synthesis, DSP (pitch/time stretch), quality resampling | not started (linear resampling only) |
| Neural singing backend | not started; no model |
| Realtime Audio Engine (cpal), mixer metering, effects | not started |
| EVBANK format, loader, manager, packer | **done in this iteration** (no backend can use a bank yet) |
| Synthesis backend interface | not started in Rust. Python has a small engine interface used for speech |
| Python plugin system | not started |
| Project format with notes/voicebanks/dependencies | v1 has tracks and clips only |
| Documentation | this folder |

## 5. Technical debt and risks

1. `App.vue` is about 1,450 lines and mixes wiring, menus and workspace state. Splitting it should happen before the Piano Roll is added.
2. Playback = render whole mix, then `<audio>`: seeking and editing during playback are coarse, latency is high. Acceptable for dialogue, not for a vocal editor.
3. Linear resampling is audible on pitch/rate conversion; needs a proper resampler (rubato) before sample-based synthesis.
4. SAPI speed has coarse integer steps; pitch through SSML is best-effort.
5. Project files store absolute clip paths into the app data folder; moving a project to another machine breaks clips. Needs relative paths/packaging (PROJECT_FORMAT.md).
6. No continuous integration; tests run by hand.
7. The `.vscode` and build artefacts (`dist`, `target`, `.exe`) were committed in the archive that was supplied.

## 6. What was not verified

Everything about the real Windows window: close button, WebView2 rendering, real audio playback, SAPI voices, Piper, `revealItemInDir`, native file dialogs. UI behaviour was checked in a Linux WebKit render with a mocked Tauri API.

## 7. Conclusion

The base (Tauri shell, project model, timeline, worker protocol) is sound and worth keeping. The vocal-editor half of the master prompt does not exist yet. The order in IMPLEMENTATION_ROADMAP.md follows from the dependencies: formats first (done), then the audio core and DSP, then notes/phonemes/lyrics, then synthesis.
