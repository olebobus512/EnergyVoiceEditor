# Phonemizer system

Status: **design only.** Nothing exists in code.

## Interface (planned)
Input: text, language hint. Output: a list of words → syllables → phonemes, each with the source text span, plus diagnostics (unsupported characters, unknown words). Phonemes use one internal symbol set (an ASCII subset of IPA-like symbols) that banks map to their own units through the bank's dictionary files.

Process: detect language → tokenise → syllabify → apply dictionary and rules → exceptions and user dictionary → phoneme sequence → align with notes.

## Modules
```
Phonemizers/  Core/  Japanese/  English/  Russian/  Chinese/  Custom/
```
- Core: the interface, the user dictionary, the manual-fix layer.
- Japanese: kana (hiragana, katakana) → morae; romaji input. The smallest honest first module because kana map directly to units.
- English: dictionary-first (an open pronunciation dictionary with a compatible licence, to be chosen) with rule fallback.
- Russian: rule-based with stress marking from a dictionary; reduction of unstressed vowels.
- Chinese: pinyin → initials/finals; character → pinyin needs a dictionary.
- Custom: Python plugins (PYTHON_PLUGIN_API.md).

## Phoneme editor (planned)
View, add, delete, replace, reorder, change duration, move boundaries, preview, restore the automatic result. Edits are stored in the project per note and survive re-phonemisation of other notes.

## Tests (to write with the first module)
Known words/kana produce expected sequences; unknown input yields a diagnostic, not a crash; user dictionary overrides rules; manual edits survive.
