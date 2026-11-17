# Documentation

Written against the code as it is. Every document starts with a status line; "design only" means nothing of it exists yet.

| Document | What | State |
|---|---|---|
| CODEBASE_AUDIT.md | what exists, gaps, debt | done |
| SYSTEM_ARCHITECTURE.md | layers, decisions (ADR) | done |
| TECHNOLOGY_STACK.md | stack and candidates | done |
| IMPLEMENTATION_ROADMAP.md | phases, next steps | done |
| EVBANK_SCHEMA.md, schemas/ | voice bank format | implemented |
| VOICEBANK_SPECIFICATION.md | bank types, manager, loader | implemented (use by synthesis: no) |
| SECURITY.md | threats and tests | bank handling implemented |
| IPC_API.md | commands and worker protocol | implemented |
| TESTING.md | how to test, state | current |
| SPEECH_SYNTHESIS.md | Windows/Piper engines | implemented, unverified on real systems |
| AUDIO_ENGINE.md, AUDIO_RENDERING.md | offline mix; realtime plan | partial |
| PROJECT_FORMAT.md | v1; v2 plan | v1 implemented |
| DSP_ENGINE.md, SINGING_SYNTHESIS.md, PHONEMIZER_SYSTEM.md, SYNTHESIS_BACKEND_API.md, PYTHON_PLUGIN_API.md | design | design only |
| CleanRoom/ | clean-room method and specifications | started |
| examples/ExampleTone.evbank | test voice bank (tones, not a voice) | built by `tools/make_example_evbank.py` |
