# Implementation roadmap

Update: the current piano roll stores editable notes and lyrics. The Rust WAV renderer mixes those notes with audio clips using a basic formant-based vowel guide voice. This does not implement phonemes, voicebank singing, neural singing, or expression curves; the guide voice only follows the final vowel in each lyric. Key previews use a separate browser synthesizer.

Follows the phases of the master prompt. Honest state as of this document.

| Phase | Goal | State |
|---|---|---|
| 1 Codebase audit | CODEBASE_AUDIT.md | **done** |
| 2 Architecture + docs | this folder | **done** (documents for unbuilt parts say so) |
| 3 Core infrastructure | bridge, IPC, audio engine, project serialization, EVBANK parser and loader | **EVBANK parser/loader/store/packer done and tested.** Realtime audio engine and new project format: not started |
| 4 Basic synthesis | phonemizer API, first language, Piano Roll + lyrics, concatenative synthesis, WAV export | not started |
| 5 Voicebank ecosystem | packaging, validation, manager, install, preview, compatibility | **mostly done early**: packing, validation, manager UI, install, uninstall, export, preview, compatibility status. "Update available" and in-app download are not done |
| 6 Expression | pitch curves, vibrato, dynamics, automation, timing | not started |
| 7 Neural synthesis | Python backend with a real, licensed model | not started. No model is bundled. Do not claim neural synthesis until one runs |
| 8 Speech synthesis | separate TTS pipeline | partly: Windows and Piper speech engines work through the worker. Not yet exposed through backend/EVBANK |
| 9 Plugins | Python plugin API, isolation | not started |
| 10 Optimisation | measured | not started |

## Next steps, in order
1. **Split `App.vue`** (wiring, menus, workspace state) so the Piano Roll has room.
2. **Audio core, offline first:** resampler (`rubato`), PSOLA/WSOLA pitch and time scaling with tests on generated signals. This gives concatenative synthesis a foundation and can be tested without a UI.
3. **Notes in the project model (format v2) + Piano Roll** (create/move/resize/select, snap, tempo).
4. **Phonemizer API + one language** (English rules or a Japanese kana table are the smallest honest start), lyrics on notes, phoneme list.
5. **`eve-classic` backend**: notes + phonemes + sample bank → audio, using `ExampleTone.evbank` for tests; add `eve-classic` to `IMPLEMENTED_BACKENDS` only then.
6. Realtime playback (`cpal`) so preview does not render the whole mix.
7. Expression curves, vibrato.
8. Neural backend, once a model with a suitable licence is chosen.
9. Plugins.

Each step ships with tests and with documentation changes in the same change.

## Built-in instruments (17 November 2026)
The piano-roll selector persists a per-track instrument: vowel guide, piano, electric guitar, bass, organ or synthesizer. Rust uses additive synthesis for the instrument timbres in both timeline playback and WAV export. These are synthesized approximations, not sampled instruments. Individual piano-key audition still uses the browser guide voice. Instrument plugins, Python preset APIs, a downloadable voice catalogue and a PTY terminal remain unimplemented.

## Interactive terminal
Added an xterm.js terminal panel and portable-pty Rust session (Windows ConPTY). Switch OUTPUT to TERMINAL and use Restart Shell to start a session. Keyboard data, ANSI output and resize events use Tauri IPC. The shell runs with the user permissions; no commands run automatically. Interactive agent compatibility has not yet been tested in the desktop window. Plugin APIs and voice downloads remain pending.

## Extensions delivery (17 November 2026)
Implemented v1 data instrument plugins, explicit trusted Python preset execution, a pinned Piper voice download catalogue, local ONNX/config import and common Rust key audition. See EXTENSIONS.md for the API, limits and test instructions. These implementations supersede the earlier entries saying plugins/catalogue were pending. VST/native plugins, EVBANK singing and neural singing are still not implemented.
