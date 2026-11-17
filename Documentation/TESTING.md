# Testing

Run all tests from `EnergyVoiceEditor/`:

```
cd src-tauri && cargo test                       # Rust
cd engines/python-runtime && python -m unittest discover -s tests     # Python
npm run build                                     # type-checks the frontend (vue-tsc) and bundles it
```

## State at the time of writing
| Suite | Tests | Result when last run |
|---|---|---|
| Rust (`cargo test`) | 48 | all passed, no compiler warnings |
| Python (`unittest`) | 28 | all passed |
| Frontend | no unit tests; `vue-tsc` type check passes | – |
| Integration across layers | none automated | – |
| Manual UI check | Linux WebKit render with a mocked Tauri API: layout, menus, drag, voicebank manager. **Not** done in the real Windows window | – |

(The counts are written by hand; trust `cargo test` over this table.)

## What the Rust tests cover
- `evbank::manifest`: every required field, bad values, files/checksums consistency, versions.
- `evbank::safety`: hostile and normal paths.
- `evbank::tests`: pack → install → list → verify → export round trip; install from folder; wrapper folder; path traversal (entries and manifest); symlinks; zip bombs and limits; damaged files; tampering, undeclared files, bad WAV; bit-rot of an installed bank; replace and failed replace; dependencies; newer format; uninstall and hostile ids; stale staging; installing the example bank built by independent Python code.
- `worker`: shutdown kills a hung worker, and is quick for a polite one.
- `settings`, `audio`, `project_io`: from earlier work.

The security tests were checked by mutation: removing the path check, the ratio check or the checksum comparison made the matching tests fail.

## Not tested yet
Audio engine, DSP, synthesis, phonemizers (they do not exist), the Windows and Piper speech engines against real Windows/Piper (only against fakes), plugins, UI components, cross-layer flows.

## Rules
A feature is not "done" without a test that fails when it breaks. Test counts are never copied into docs without running the suites first.
