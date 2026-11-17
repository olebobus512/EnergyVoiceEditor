# Technology stack

Status: current stack is implemented; candidates are marked.

## In use
- Tauri 2, Vue 3, TypeScript, Vite, CSS.
- Rust (edition 2024): `tauri`, `tauri-plugin-opener`, `tauri-plugin-dialog`, `serde`, `serde_json`, `zip` (archives), `sha2` (checksums).
- Python 3 standard library only. Windows speech through PowerShell (`System.Speech`). Piper through its command line, optional.

## Candidates, not yet added (each needs a reason when it is added)
| Crate/library | Reason | When |
|---|---|---|
| `cpal` | audio device output for a realtime engine | Phase 3b |
| `rubato` | high quality resampling (current resampler is linear) | before concatenative synthesis |
| `symphonia` | decode FLAC/OGG/MP3 for import | when importing non-WAV |
| `hound` | not needed; `audio.rs` already reads/writes WAV | – |
| `ndarray` | only if DSP code needs it; plain slices first | – |
| ONNX Runtime (Python) | neural inference | Phase 7, needs a licensed model |
| NumPy | Python DSP/inference | with the neural backend |

C++ is not used. Dependencies are added only with a feature that needs them.
