# DSP engine

Status: **design only.** No pitch or time processing exists. Based on public descriptions of the algorithms, not on any product's internals.

## Algorithms considered
| Method | Good at | Weak at | Needs |
|---|---|---|---|
| Crossfade (equal-power) | joining segments, hiding clicks | cannot change pitch or length | nothing |
| TD-PSOLA | pitch and duration change of voiced sounds (vowels, sung notes) with natural timbre | needs pitch marks; unvoiced sounds and noisy material; large shifts make formants unnatural | pitch marks (F0 tracking) |
| WSOLA | time stretching of speech and consonants without pitch marks | echo/stutter on strongly periodic sounds when stretched a lot | nothing; pitch change = stretch + resample |
| Phase vocoder | polyphonic or noisy material, large stretch factors | phasiness, smeared transients | FFT |
| Parametric vocoder (F0 + spectral envelope + aperiodicity, e.g. the open-source WORLD) | independent control of pitch and formants, smooth morphing between units | more compute, analysis errors | analysis step per sample |

## Proposed use
- Voiced units (vowels, nasals, sustained notes): TD-PSOLA, pitch marks from a YIN-style tracker computed once per sample and stored in the bank cache.
- Consonants and short units: WSOLA or no stretching (play as recorded).
- Joins: equal-power crossfade, placed at pitch-synchronous points inside voiced parts.
- Later: a vocoder backend for formant control and better morphing.

All behind one trait (`PitchTimeScaler`) so an algorithm can be replaced without touching the engine. The resampler (`rubato`) is separate.

## Tests (before any UI)
Generated sines and harmonic tones: output pitch within 5 cents of target, duration within one period of target, no discontinuity above a threshold at joins, silence in → silence out.
