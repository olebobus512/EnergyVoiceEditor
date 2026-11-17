# Audio engine

Status: **offline mixing implemented; realtime engine not started.**

## Today
- `audio.rs`: reads WAV (PCM 16/24/32, float), writes WAV (16/24), converts to stereo, applies volume and constant-power pan, resamples by linear interpolation, sums tracks (mute/solo respected by the caller).
- `render_mix` renders the whole mix from a start time to a WAV file. The webview plays it with an `<audio>` element. Seeking or editing during playback means rendering again.
- No device access, no recording, no routing, no metering.

## Target (Phase 3b)
Threads: **audio callback** (no locks, no allocation, no I/O; reads from a lock-free ring buffer), **render worker** (builds buffers ahead of the playhead), **loader** (decodes files), **synthesis worker**, **UI**. `cpal` for the device, `rubato` for resampling. Commands: play, stop, seek, loop, meter events. The renderer used for export and for preview is the same code, so they cannot sound different.

Requirements (to be measured, not promised): no underruns while a voice bank loads; output latency reported; cancel in under 100 ms.

## Tests to write first
Mixer sum and pan law on generated signals; resampler error against a reference sine; ring buffer under concurrent producer/consumer.
