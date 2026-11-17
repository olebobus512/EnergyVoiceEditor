# Security

Status: voice bank handling **implemented and tested**; plugin isolation **not implemented** (plugins do not exist yet).

## Voice banks (untrusted input)
| Threat | Defence | Test |
|---|---|---|
| Path traversal in archive entries (`../x`, `/abs`, `C:/x`, `a\..\b`) | every name normalised and checked; the bank is first written to a staging folder | `path_traversal_is_rejected_and_nothing_escapes` |
| Path traversal through the manifest | the same check on every `files` entry | `manifest_path_traversal_is_rejected` |
| Symbolic links (zip entries, folders) | refused | `symlink_entries_and_folders_are_rejected` (Unix; Windows junctions are not detected separately) |
| Zip bomb | per-file limit (2 GiB), total (4 GiB), entry count (50 000), compression-ratio limit (200× for entries over 1 MiB); sizes also enforced **while writing**, because headers can lie | `zip_bombs_are_rejected` |
| Damaged or foreign file | zip errors become "not a voice bank"; truncated files fail | `damaged_and_foreign_files_are_rejected` |
| Changed content | SHA-256 per file | `tampered_files_undeclared_files_and_bad_audio_are_caught` |
| Extra files next to the manifest | any unlisted file rejects the bank | same test |
| Bad audio / model files | WAV header check, empty model check | same test |
| Hostile `voiceId` used as a folder name | strict pattern; install/uninstall/verify refuse anything else | `uninstall_and_hostile_ids` |
| Half-installed bank | staging + verification + atomic rename | `reinstalling_needs_confirmation_and_failure_keeps_the_old_bank` |
| Executing bank content | never. A bank is data. Nothing in it is run, imported or `eval`-ed | by design |

Known limits: checksums prove the bank was not damaged in transit, **not who made it** (no signatures yet). Encrypted zip entries are refused. A model file is only checked for being non-empty; a malformed model must be handled safely by the inference runtime when one exists.

## Python worker
Started by the app with a fixed script path; talks over stdin/stdout only. It does not load voice-bank code. Engines run `powershell` or `piper` with arguments built by the engine, never with a shell string; text is passed through files or stdin.

## Plugins (planned)
Python plugins are executable code. Required before they exist: separate process, no inherited credentials, time limit, IPC validated against a schema, explicit user consent per plugin, install from a local file only. Until then no third-party Python is run.

## Webview
The right-click menu and file access go through Tauri commands. `read_audio` only reads inside the app's generated-audio folder. Capabilities are listed in `src-tauri/capabilities/default.json`.
