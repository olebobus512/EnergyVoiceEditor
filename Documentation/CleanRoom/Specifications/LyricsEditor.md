# Lyrics Editor

**Purpose.** Attach text to notes.

**Features (planned).** Type lyrics and split them into syllables onto consecutive notes; edit a word; move text between notes; merge/split syllables; per-note language; run phonemisation; fix pronunciation; show problems.

**Inputs.** Text, selected notes, language. **Outputs.** Per-note lyric and phonemes.

**Scenario.** Select ten notes, paste a line, the editor distributes syllables; the user corrects one word's pronunciation.

**Interaction.** Always in sync with the Piano Roll (same note objects) and the Phoneme Editor. Uses the Phonemizer API.

**Approach.** Syllable splitting by the language module; a continuation marker for notes that hold the previous syllable.

**Limits.** Quality depends on the language module; unsupported characters are reported, never silently dropped.

**Tests.** Distribution across notes, move/merge/split, continuation notes, unsupported characters, undo.
