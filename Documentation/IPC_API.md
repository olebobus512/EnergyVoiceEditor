# IPC and component API

Status: the commands and the Python protocol below are **implemented as written**. Parts marked *planned* are not.

## 1. Frontend ⇄ Rust (Tauri commands, `invoke`)

Errors are strings, except the `evbank_*` commands which return `{ code, message, installed?, incoming? }`.

| Command | Purpose |
|---|---|
| `get_app_info` | version, whether the Python worker runs |
| `get_settings`, `save_settings` | settings file |
| `save_project_file`, `open_project_file` | project JSON |
| `list_voices(refresh?)` | voices of all speech engines |
| `get_voices_dir` | folder for Piper models |
| `synthesize` | speech to a WAV in the generated-audio folder; progress via event `synthesis-progress` |
| `import_audio` | read an audio file's info |
| `render_mix(request)` | mix the timeline to a WAV (preview, or export when `output` is set) |
| `read_audio(path)`, `export_audio(path)` | read from / copy out of the generated-audio folder only |
| `shell_run`, `shell_restart` | console PowerShell session; events `shell-output`, `shell-done` |
| `exit_app` | quit: stops the worker and the shell |
| `evbank_list` | installed banks with status |
| `evbank_install(path, kind, replace)` | `kind` = `archive` or `folder`; error code `already_installed` carries both versions |
| `evbank_verify(id)` | full checksum check |
| `evbank_uninstall(id)`, `evbank_export(id, destination)` | |
| `evbank_pack(folder, destination)` | authoring tool |
| `evbank_dir` | store folder |
| `evbank_preview(id)` | bytes of the bank's preview WAV |

Events: `worker-log`, `synthesis-progress`, `shell-output`, `shell-done`.

Heavy commands run in `spawn_blocking`. Audio never travels as JSON: it is a file, or a binary `ipc::Response`.

## 2. Rust ⇄ Python worker

Transport: one JSON object per line on the worker's stdin/stdout. stderr carries logs (forwarded as `worker-log`). Large data is exchanged as file paths.

```
→ {"id":1,"method":"init","params":{"engines":["sapi","piper","debug"],"voices_dir":"<folder>"}}
← {"id":1,"result":{"engines":["sapi"],"voices":[{"id","name","languages","engine","gender","supportsPitch"}]}}
→ {"id":2,"method":"synthesize","params":{"text","voice","output_path","speed","pitch"}}
← {"event":"progress","id":2,"progress":0.1}
← {"id":2,"result":{"path","sample_rate","duration_sec"}}
← {"id":2,"error":{"code":"invalid_params|unknown_method|invalid_json|internal","message":"..."}}
→ {"id":3,"method":"shutdown"}
```
Other methods: `list_voices {refresh}`. The docstring of `engines/python-runtime/main.py` is the authoritative description.

Behaviour: one request at a time; the Rust side holds a lock while a request runs; shutdown waits 2 s then kills the worker. **Cancellation of a running synthesis is not implemented.** Planned: a `cancel` message handled by a reader thread in the worker.

## 3. Planned interfaces
- Synthesis backend interface: SYNTHESIS_BACKEND_API.md.
- Plugin protocol: PYTHON_PLUGIN_API.md.
- Rust-side audio engine commands (play, stop, seek, meter events): with AUDIO_ENGINE.md.
