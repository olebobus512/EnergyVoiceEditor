# Python plugin API

Status: **design only.** No plugin loading exists. Third-party Python is not run by the app.

## Package
```
ExamplePlugin/
├── plugin.json       id, name, version, author, description, entryPoint, requiredEveVersion, apiVersion, dependencies, type
├── main.py
├── requirements.txt
└── resources/
```
Types: phonemizer, audio processing, synthesis extension, voicebank tools, import/export, project tools, analysis. (UI extensions are out of scope.)

## Execution model (required before the first plugin runs)
- One separate process per plugin, started by the Python backend manager, not inside the speech worker.
- Talks only JSON lines over stdio, validated against the API version; large audio via files in a per-plugin temp folder.
- Time limit per call, memory limit where the OS allows, cancellation, and the process is killed on disable or exit.
- The user must enable each plugin explicitly after seeing what it declares. Installing from a local folder only.
- Restricting filesystem and network for Python processes is limited without an OS sandbox. Until a sandbox exists the honest description is "runs with the user's privileges, only after explicit consent", and this must be shown to the user.

## Manager (planned)
Install, remove, enable, disable, version and dependency check, log view, diagnostics, error handling.
