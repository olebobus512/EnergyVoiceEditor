# Synthesis backend API

Status: **design only** for the Rust interface. Python speech engines have a smaller interface today (`list_voices`, `synthesize`).

## Interface (planned, Rust trait + equivalent JSON for out-of-process backends)
```
id, name, version, backendType (classic | neural | speech | external)
supportedLanguages, supportedParameters (pitch, vibrato, dynamics, breathiness, ...)
requirements()                 -> what must be present (runtime, GPU, model)
checkCompatibility(bank)       -> Compatible | Incompatible(reasons)
initialize(config)
synthesize(renderPlan, outputPath, cancelToken, progress) -> result | error
cancel(taskId)
cleanup()
```
The render plan is plain data (notes, phonemes with times, F0 curve, expression curves, bank id, settings). The core never calls a concrete backend directly; it asks the backend manager for the backend that a bank's `supportedBackends` and the track's setting select.

## Backends
| Id | Kind | State |
|---|---|---|
| `eve-classic` | sample-based, Rust | not started |
| `eve-neural` | neural, Python | not started, needs a model |
| `eve-speech` | TTS | engines exist in Python; not behind this interface |
| `eve-external` | third-party over IPC | not started |

`IMPLEMENTED_BACKENDS` (`src-tauri/src/evbank/manifest.rs`) must list exactly the backends that run in this build. It is empty today, which is why every bank shows "no synthesis engine can use this voicebank yet".

Adding a backend must not require changes to the Piano Roll, the Voicebank Manager or the project format: it registers itself and declares what it supports.
