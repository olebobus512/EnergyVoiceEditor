# Extensions and voice catalogue — implemented API v1

## Using the application

The transport **Speech synthesis** button opens a working speech editor: real SAPI/Piper voice selection, text, speed and capability-dependent pitch, synthesis, listen/stop and undoable insertion of the generated WAV at the playhead. Debug tones are excluded from speech choices.

- Settings → Extensions: install a JSON instrument or choose a Python `.py` preset.
- Settings → Voice catalogue: download the pinned LJ Speech high model or import a local Piper `.onnx` with its adjacent `.onnx.json`.
- Settings → Voicebank Manager: existing EVBANK install/verify/export remains separate. EVBANK singing backends are not implemented.
- Piano roll instrument selector: built-in and installed instruments are selectable per track. Key audition, timeline playback and WAV export use Rust synthesis.
- OUTPUT / TERMINAL: opening the terminal starts PowerShell in ConPTY on Windows. ANSI rendering uses xterm.js; Ctrl+C and resize are sent to the PTY. Hiding the panel preserves the terminal screen during this app session.

## Instrument package

Example: `examples/extensions/soft-bell.json`. Install through Extensions. Version 1 is a **data plugin**, not a native binary or VST. Additive synthesis uses up to 32 harmonic amplitudes, with attack/release in seconds and exponential decay in inverse seconds. Harmonics above the Nyquist margin are omitted.

```json
{
  "id": "soft-bell",
  "name": "Soft Bell",
  "version": 1,
  "harmonics": [1, 0.15, 0.5, 0, 0.2, 0, 0.1],
  "attack": 0.005,
  "release": 0.2,
  "decay": 2.5
}
```

IDs contain lowercase ASCII letters, numbers and hyphens. Manifest size is limited to 16 KB. Amplitudes are 0–1; attack 0.001–2 s, release 0.001–3 s, decay 0–20. Unknown fields or unsupported versions are rejected. Installed manifests are stored in the app-data `instruments/` directory and registered as `plugin:<id>`. Duplicate installation does not overwrite an existing instrument. Projects record this ID; another computer must install the same plugin. Instruments are not embedded in project files. There is no native/VST host, arbitrary UI extension loader or sample-based instrument API in v1.

## Python preset API

Example: `examples/extensions/arpeggio.py` generates 32 notes without dependencies.

Rust starts the selected file with `ENERGY_PYTHON`, or `python` if unset, using UTF-8. No shell interpolation is used. Installation of an instrument never executes Python. The user chooses a script and explicitly marks it trusted before running. Scripts run **with the user's permissions, not in a sandbox**; they can access files, processes and the network. Timeout and output limits are reliability measures, not an isolation boundary.

Input is a single JSON document on stdin:

```json
{"apiVersion": 1, "project": {"version": 1, "tracks": []}, "selectedTrackId": "track-1"}
```

Output must be one JSON document on stdout (send diagnostics to stderr):

```json
{"name": "My preset", "tempo": 120, "notes": [{"midi": 60, "start": 0, "duration": 1, "lyric": "la"}]}
```

Start and duration are in beats. Tempo is optional (30–300 BPM when given). Notes are validated (MIDI 0–127, start 0–4096 beats, duration 0.0625–64 beats, at most 2000 notes). Execution is limited to 30 seconds and each output stream to 1 MB. Invalid JSON and Python failures are shown without applying project edits. A script can create its own files; Undo only covers application project changes.

The UI previews the returned name and note count. Apply creates one clip at the playhead, on the originally selected non-audio track or a new piano track. Preset timing is converted to the project tempo; project tempo is unchanged. Applying is one undo step. If the project changed while the script ran, application is refused until the script is run again. This API creates note presets; it does not expose arbitrary project mutation or realtime audio callbacks. Existing render limits (including 600 seconds of summed note duration) still apply.

## Piper catalogue

Downloads are opt-in HTTPS requests from Rust to Hugging Face. No project text is uploaded. The catalogue pins repository revision `e9ab671f2a0e50a701b34361136ba3e6a3a9eb0c` and SHA-256 for both the ONNX and JSON configuration. Installation stages files **outside** Piper's discovery tree, validates hashes and configuration, then renames the directory into place. Errors and cancellation clean the staging folder. Cancellation is checked between reads; a stalled network read may wait until the HTTP timeout.

The catalogue currently contains one voice, LJ Speech high (English). Source/model card: https://huggingface.co/rhasspy/piper-voices/blob/e9ab671f2a0e50a701b34361136ba3e6a3a9eb0c/en/en_US/ljspeech/high/MODEL_CARD
The model card describes the dataset as public domain. Read the model card for provenance and terms; do not infer blanket licensing for other models.

Local import requires a matching configuration with `audio.sample_rate` and `phoneme_id_map`. This validates packaging metadata, not ONNX execution compatibility. Piper determines model compatibility during synthesis. Piper and Python are not installed automatically. Use `python -m pip install piper-tts` in the environment used by Energy. These are **speech models, not singing models**. Singing still uses the built-in vowel guide.

The Python worker files are now included as Tauri resources. Release builds resolve the bundled `python-runtime/main.py`; development builds resolve the checkout. A Python interpreter and optional Piper runtime are still external dependencies.

## Validation

`npm run build`: TypeScript and production build.
`npm run test:ui`: browser workflow tests with mocked Tauri IPC; default executable is installed Windows Edge. Set `PLAYWRIGHT_CHROMIUM_EXECUTABLE` to use another Chromium executable.
`cargo test`: Rust tests, including real Python preset protocol and real Windows ConPTY interaction.
`cargo test pinned_catalog_config_download_verifies_hash -- --ignored`: optional real HTTPS download/hash/configuration test (configuration only, not the 109 MiB model).

Browser tests do not prove native dialogs, end-to-end Piper audio, or every third-party agent's terminal behaviour. Test those in the real desktop window before release.

### Verified in this development session
Five Chromium/Edge UI scenarios passed with mocked IPC: instrument install/selection; Python preview/apply/Undo; catalogue UI; speech generation/apply/Undo; minimum-window transport layout. The real Windows SAPI engine generated a 2.359-second, 22050 Hz WAV using installed Irina. The real ConPTY test exchanged interactive input and resized the terminal. The pinned remote configuration was downloaded and its SHA-256 verified; the full ONNX model was not downloaded or executed in this session. Python engine tests passed (27, plus one platform-specific skip).

Release verification: `tauri build --no-bundle` succeeded. The portable executable started with the Energy Voice Editor window title and closed normally with exit code 0. The relocated bundled Python worker initialized and discovered three Windows speech voices. This is a startup smoke test, not exhaustive native UI or third-party CLI testing.
