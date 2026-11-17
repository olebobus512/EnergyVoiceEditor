# Import and export compatibility

| Format | Direction | State | Note |
|---|---|---|---|
| WAV (PCM 16/24/32, float) | import, export (16/24) | implemented | |
| `.eveproj` v1 | read/write | implemented | |
| `.evbank` | read/write | implemented | our own format |
| MIDI 1.0 | import/export | planned | public specification |
| Piper voices (`.onnx` + `.onnx.json`) | use as speech voices | implemented (needs Piper) | not EVBANK yet |
| Other editors' project/bank formats | – | **not planned** | closed formats are not reverse engineered; open formats (for example OpenUtau projects, UTAU voicebanks under their own licences) may be considered later, each from its public documentation |
