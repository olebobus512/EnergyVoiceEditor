# Piano Roll

**Purpose.** Edit notes on a pitch × time grid.

**Features (planned).** Create, delete, move, resize, change pitch, multi-select, copy/paste/duplicate, snap and quantise, tempo and time signature, loop, playhead, MIDI import/export. A note holds id, pitch, start, duration, lyric, phonemes, expression data.

**Inputs.** Pointer and keyboard, project notes, tempo map. **Outputs.** Note list changes (undoable commands), preview requests.

**Scenario.** The user draws a note, types a lyric, plays it back, drags the note up and hears the pitch change.

**Interaction.** Lyrics Editor and Phoneme Editor edit the same note objects; the synthesis pipeline re-renders only notes whose data or neighbours changed (incremental recomputation).

**Approach.** Canvas rendering, notes stored in ticks (integer, e.g. 480 per beat) so tempo changes do not move notes; the existing undo/redo snapshot approach of `project.ts` extended to notes.

**Independent implementation.** Grid, note and selection logic are generic UI code; no external design is needed. MIDI via a small, well-documented file format (MIDI 1.0 specification is public).

**Limits.** Monophonic per track at first (vocal lines); overlapping notes on one track are rejected or shortened with a visible rule.

**Performance.** Smooth with 5,000 notes: only visible notes drawn; no full re-render on drag.

**Tests.** Note CRUD, snap/quantise arithmetic, tick/second conversion with tempo changes, undo/redo, MIDI round-trip.
