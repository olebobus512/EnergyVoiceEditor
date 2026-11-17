# Phoneme Editor

**Purpose.** Inspect and correct the phonemes of notes and their timing.

**Features (planned).** View, add, delete, replace, reorder, change duration, move boundaries, preview, restore automatic phonemisation.

**Inputs.** Notes with phonemes, the bank's phoneme inventory. **Outputs.** Edited phoneme lists with explicit timing flagged as manual.

**Scenario.** A word sounds wrong; the user swaps one phoneme for another from the bank's inventory and previews just that note.

**Interaction.** Restricts choices to phonemes the bank supports (from the bank's dictionary/config); changes are stored in the project and kept when other notes are re-phonemised.

**Approach.** A lane under the note showing phoneme boxes; manual edits carry a flag so automatic re-runs do not overwrite them.

**Limits.** Needs a bank with a phoneme inventory; neural banks may expose a different unit set.

**Tests.** Edit, reorder and restore; manual flag survives re-phonemisation; invalid phoneme rejected with a message.
