# System architecture

Status: **design, partly implemented.** Each component is marked.

## 1. Layers

```
Vue 3 frontend (TypeScript)                     implemented (voice mode, timeline, voicebank UI)
        │  Tauri invoke / events  = "Application Bridge"
Rust core                                       partly implemented
   ├─ Project Manager        project_io.rs      v1 only
   ├─ Voicebank Manager      evbank/            implemented
   ├─ Audio renderer         audio.rs           offline mix to WAV; playback is in the webview
   ├─ Audio engine (cpal)    –                  not started
   ├─ DSP / concatenative    –                  not started
   ├─ Synthesis backend mgr  –                  not started (Python engines exist for speech)
   └─ Python backend manager worker.rs          implemented (JSON lines)
        │  JSON lines on stdin/stdout, audio as WAV files on disk
Python worker                                   speech engines implemented; neural/plugins not started
```

Boundaries:
- **Frontend** shows state and sends commands. No audio processing, no file-format parsing.
- **Rust** owns files, formats, security checks, audio, processes. Anything that must never block the UI runs in `spawn_blocking`.
- **Python** runs models and optional plugins in a separate process. It never touches the audio thread. Large audio goes through files, not JSON.

## 2. Decisions

**ADR-001: Vue 3, not React.** The master prompt names React, but the repository is Vue 3 and the project owner designs the UI in FigCraft for Vue (`Контекст.md`, AGENTS.md). The same prompt says to keep the existing UI architecture and not change the stack without a reason. There is no reason, so Vue stays. Nothing in the architecture depends on the UI framework.

**ADR-002: voice banks are data, never code.** An EVBANK is read and verified by Rust and passed to a backend as data. Executable content (Python) only enters through the plugin system, with isolation (SECURITY.md).

**ADR-003: Rust ⇄ Python IPC = JSON lines + files.** Already in use and tested. Documented in IPC_API.md.

**ADR-004: install is atomic.** Staging folder → full verification → rename (EVBANK_SCHEMA.md §6).

**ADR-005: no stubs presented as features.** A voice bank that no backend can use is shown as such in the UI. `IMPLEMENTED_BACKENDS` in `evbank/manifest.rs` is the single list that says which backends exist.

## 3. Component map to the master prompt's subsystems

| Prompt subsystem | Where | State |
|---|---|---|
| Application Core | `lib.rs`, `commands.rs` | yes |
| Project Manager | `project.ts`, `project_io.rs` | v1 |
| Timeline Engine | `Arrangement.vue`, `project.ts` | yes |
| Piano Roll, Lyrics Editor, Phoneme Editor | – | no |
| Phonemizer | – | no |
| Voicebank Manager / Loader | `evbank/`, `VoicebankManager.vue` | yes |
| Synthesis Backend Manager | – | no |
| Singing / Speech Synthesis Engine | Python `engines/` (speech only) | speech only |
| Expression / Pitch Engine | – | no |
| Audio Engine / Mixer | `audio.rs` (offline) | partial |
| Audio Renderer | `audio.rs::mix`, `render_mix` | partial |
| Plugin Manager | – | no |
| Model Manager | – | no |
| Preset Manager | – | no |
| Export System | `export_audio` (WAV) | partial |
