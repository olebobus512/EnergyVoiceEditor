# Voicebank specification

Status: format and manager **implemented**; use of banks by synthesis **not implemented**.

## Bank types
| Type | `synthesisTypes` | Content | Used by |
|---|---|---|---|
| Sample voicebank | `sample` | recorded phonemes, syllables, transitions | `eve-classic` (planned) |
| Neural voicebank | `neural` | trained model + config | `eve-neural` (planned) |
| Hybrid | `sample` + `neural` | both | either |
| Speech voicebank | `speech` | for ordinary speech | `eve-speech` (planned) |
| Singing voicebank | `singing` | for vocals | classic/neural |

A bank may combine purposes. Which backend uses which part is decided by `supportedBackends`.

## Voicebank Manager (UI: Settings → Voicebank Manager)
Implemented: import archive, import folder, pack folder, list with search and filter (all / ready / needs attention), details (author, languages, types, backends, licence, size, location, problems), verify, preview (plays `preview` WAV), export, uninstall, replace confirmation, open folder.

Not implemented: update from a repository, dependency auto-install, using a bank in the voice list.

## Loader pipeline (master prompt §6.4)
| Step | Done where |
|---|---|
| open file, check archive | `archive::extract_archive` |
| read manifest, validate fields, version | `manifest::parse_manifest` |
| required files, checksums | `verify::verify_dir` |
| audio materials | WAV header and sample-rate check |
| models | non-empty check only (cannot be loaded without a runtime) |
| backend compatibility | `usableBackends` from `IMPLEMENTED_BACKENDS` |
| language modules (phonemizers) | recorded in the manifest; no phonemizer exists to check against yet |
| hand over to an engine | **not implemented** (no engine) |

## Licensing
The manifest carries the licence name and `commercialUse`. The app shows them but does not enforce them. Bank authors and users remain responsible for the rights to the recordings and models inside a bank.
