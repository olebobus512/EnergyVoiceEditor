# Speech synthesis

Status: **implemented through Python engines** (no EVBANK use, no Rust backend interface yet).

## Engines (`engines/python-runtime/engines/`)
| Engine | What it is | Needs | Pitch | Notes |
|---|---|---|---|---|
| `sapi` | Windows system voices through PowerShell `System.Speech` | Windows | best effort via SSML, falls back to no pitch and logs it | speed uses SAPI's −10…10 rate scale (1/3×…3×), coarse steps; the voice's own sample rate |
| `piper` | Piper neural voices through its command line | `pip install piper-tts` and `.onnx` models in `<app data>/voices/piper/` | none | speed = `length_scale = 1/speed`; flag spelling is detected from `--help` |
| `debug` | generated tones | – | yes | not speech; offered only when no real voice exists |

Voice ids are `sapi:<name>`, `piper:<model>`, and the legacy `debug-a`/`debug-b`.

## Input / output
In: text, voice id, speed 0.5–2.0, pitch −12…12 semitones. Out: WAV file path, sample rate, duration. Errors come back as `{code, message}`. Not yet available: language selection, intonation, pauses, emotion parameters, timing information, cancel.

## Verification status
Unit tests use fakes for PowerShell and Piper. **Not yet run against real Windows voices or a real Piper install.**

## Planned
Expose speech through the common backend interface (`eve-speech`), allow `speech` EVBANKs to carry models, return word timings. Keep it independent of the Piano Roll.
