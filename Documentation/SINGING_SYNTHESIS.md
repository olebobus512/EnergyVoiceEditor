# Singing synthesis

Status: **design only.** No singing synthesis exists: no notes, no phonemizer, no concatenative engine, no neural model. Nothing in the app produces sung audio.

## Pipeline (master prompt §4.2, as designed)
1. Project data: notes (MIDI pitch, start, duration, lyric, phonemes), pitch and expression curves, voice bank, backend, settings.
2. Validate: note overlap rules, pitch inside the bank's `pitchRange`, bank usable by the backend.
3. Phonemise lyrics (PHONEMIZER_SYSTEM.md), map phonemes to notes: vowel on the note start by default, consonants before the beat; the user can edit boundaries.
4. Timing: phoneme boundaries from note timing and unit lengths.
5. Pitch: base note + portamento + vibrato + hand-drawn curve → F0 per frame.
6. Expression: dynamics, breathiness, etc. as the backend supports them.
7. Backend synthesis: classic = select units, scale pitch/duration, join; neural = model inference in the Python process.
8. Assemble audio, hand to the renderer, return a file path.

The pipeline is backend-independent: steps 1–6 produce a **render plan** (a plain data structure); step 7 consumes it.

## Backends
- `eve-classic` (Rust): unit selection + DSP_ENGINE.md. First target: sustained vowels from `ExampleTone.evbank`, then real banks.
- `eve-neural` (Python, ONNX Runtime): only after a model with a suitable licence is chosen. If no model is available the backend interface and tests may exist, and the UI must say that neural synthesis is not available.

## Definition of done (from the prompt)
Create a note, type lyrics, get phonemes, audio is generated, played and exported, and **changing the note changes the result**. None of this is met yet.
