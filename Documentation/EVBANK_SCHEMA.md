# EVBANK: Energy Voice Bank, format version 1

Status: **implemented** (`src-tauri/src/evbank/`). Machine-readable rules: `schemas/evbank-manifest.schema.json`. Where the two disagree, the Rust code is the truth, and the disagreement is a bug to report.

A `.evbank` is a **zip archive**. It is a container of data and is never executed.

## 1. Layout

```
manifest.json            required, at the top of the archive
<any files>              audio, models, dictionaries, configs, resources
```

The suggested folders (`Audio/Samples`, `Audio/Phonemes`, `Audio/Transitions`, `Dictionaries/`, `Models/`, `Config/`, `Resources/`) are a convention, not a requirement. A single wrapper folder (`ExampleVoice/manifest.json`) is accepted. Entries under `__MACOSX/` and `.DS_Store` are ignored.

Missing optional parts never make a bank unloadable: a neural-only bank has no audio samples, a sample bank has no model.

## 2. manifest.json

Required fields

| Field | Rule |
|---|---|
| `format` | exactly `"evbank"` |
| `formatVersion` | integer ≥ 1. Larger than the app supports → "unsupported version" (bank is refused, or shown as Incompatible if already installed) |
| `voiceId` | 3–64 chars: `a-z 0-9 . _ -`, starting with a letter or digit, no `..`. Used as the install folder name |
| `voiceName` | non-empty, ≤ 100 chars |
| `author` | non-empty |
| `description` | text (may be empty) |
| `languages` | non-empty list of language codes (`ja`, `en`, `ru`, `zh-CN`) |
| `synthesisTypes` | non-empty subset of `sample`, `neural`, `speech`, `singing`. Hybrid = `sample` + `neural` |
| `supportedBackends` | non-empty list of backend ids (`eve-classic`, `eve-neural`, `eve-speech`, `eve-external`, or others) |
| `phonemizers` | list of phonemizer ids (may be empty) |
| `audioConfiguration` | `sampleRate` 8000–192000, `channels` 1 or 2, `bitDepth` 16/24/32 |
| `pitchRange` | `minMidi` < `maxMidi`, both 0–127 |
| `license` | object with non-empty `name`; optional `url`, `commercialUse` (bool) |
| `files` | every file except `manifest.json`: relative `/` paths, unique, safe (§5) |
| `checksums` | lowercase SHA-256 for every path in `files`, and for no other path |

Optional fields: `version` (dotted numbers; missing = `0.0.0`), `icon`, `preview` (must be listed in `files`), `supportedExpressions`, `modelRequirements`, `dependencies` (`[{voiceId, minVersion?}]`), `voiceVariants` (`[{id, name, description?}]`), `metadata`, `tags`.

Unknown extra fields are allowed and ignored (forward compatibility).

## 3. File-set rules

- Every file in the archive must be listed in `files`. An unlisted file rejects the bank, so nothing can be smuggled in next to a signed-looking manifest.
- Every listed file must exist and match its checksum.
- Two paths that differ only in letter case are rejected (they collide on Windows and macOS).
- `.wav` files must have a valid header whose data fits in the file. Under `Audio/` the sample rate must equal `audioConfiguration.sampleRate`.
- `.onnx` files must be non-empty. **The loader does not load or run models**; it can only say that the file is intact.

## 4. Status of an installed bank

| Status | Meaning |
|---|---|
| Ready | manifest valid, all files present, dependencies satisfied |
| Missing dependencies | a `dependencies` entry is not installed or too old |
| Incompatible | written for a newer format version |
| Corrupted | manifest unreadable or files missing (a full checksum check is `Verify`) |

"Ready" means the package is sound. Whether something can *use* it is separate: `usableBackends` lists the manifest's backends that exist in this build. Today it is always empty, and the UI says so.

Not implemented: "Update available" (needs a bank repository), "Loading" (a transient UI state that will appear with the first backend).

## 5. Path rules

Names are checked for every archive entry, folder item and manifest path: no empty names, `.`, `..`, absolute paths, drive letters, `:`, `\`, control characters, `* ? " < > |`, trailing dot or space, Windows device names (`CON`, `NUL`, `COM1`…), names over 120 chars, paths over 240 chars. Backslashes in archive entries are converted to `/` before checking.

## 6. Install

```
archive/folder ──extract/copy──▶ voicebanks/.staging/<unique>/   (limits enforced while writing)
                 ──verify (manifest, file set, checksums, WAV, models)──▶
                 existing bank? ── no replace flag ──▶ error "already installed", nothing touched
                                └─ replace ──▶ old bank → .trash/, new → voicebanks/<voiceId>/, old deleted
any failure ──▶ staging removed, old bank restored
```
Leftovers of a crash (`.staging`, `.trash`) are removed when the store opens.

Location: `<app data>/voicebanks/<voiceId>/`. Export writes a verified bank back to a `.evbank`.

## 7. Authoring

`Pack Folder as .evbank...` in the manager (command `evbank_pack`): put a hand-written `manifest.json` (without `files` and `checksums`) and the bank's files in a folder. Packing lists the files, computes checksums, validates the result and writes the archive atomically. `tools/make_example_evbank.py` builds `examples/ExampleTone.evbank` independently of the Rust packer.

## 8. Versioning

`formatVersion` changes only for incompatible changes. New optional fields do not change it. A reader refuses higher versions instead of guessing.
