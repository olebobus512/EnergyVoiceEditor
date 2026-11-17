# Project format (`.eveproj`)

## Version 1: implemented (`src/project.ts`, `src-tauri/src/project_io.rs`)
JSON: `version`, `name`, `sampleRate`, `tracks[]`. Track: `id`, `type` (voice, audio, instrument, midi), `name`, `voiceId?`, `color` (#RRGGBB), `height` (compact/normal/large), `volume`, `pan`, `muted`, `solo`, `clips[]`. Clip: `id`, `name`, `path`, `start`, `duration`, `text?`.

Compatibility: files without `color`/`height` get defaults on load. Fields the app does not know are not validated and are kept as they are.

Known weakness: `clip.path` is an absolute path to the generated-audio folder of one machine.

## Version 2: planned
Adds what the master prompt lists: tempo and time signature, notes (pitch, start, duration, lyric, phonemes, per-note expression), pitch and expression curves, automation, markers, loop regions, mixer settings, synthesis settings, **voicebank references** (`voiceId` + version + checksum of the manifest, never the bank's files), dependencies, autosave and recovery file.

Rules for v2: relative paths with the audio next to the project (or a packed project archive); a migration function per version step with a test on a saved v1 file; opening a newer version fails with a clear message instead of guessing; missing voicebanks open the project with a "missing dependency" list, not an error.
