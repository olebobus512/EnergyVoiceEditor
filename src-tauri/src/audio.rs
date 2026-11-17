use std::f32::consts::FRAC_PI_4;
use std::path::Path;

/// Interleaved PCM audio in f32 (-1.0..1.0).
#[derive(Debug, Clone, PartialEq)]
pub struct Audio {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl Audio {
    pub fn duration_sec(&self) -> f64 {
        let frames = self.samples.len() / self.channels.max(1) as usize;
        frames as f64 / self.sample_rate.max(1) as f64
    }
}

pub struct MixClip {
    pub audio: Audio,
    /// Start on the timeline in seconds (may be before the render start).
    pub start: f64,
}

pub struct MixTrack {
    pub volume: f32,
    pub pan: f32,
    pub clips: Vec<MixClip>,
}

/// Electronic guide voice based on vowel formants; not a voicebank.
pub fn vowel_note(midi: u8, lyric: &str, duration: f64, sample_rate: u32) -> Audio {
    let vowel = lyric.to_lowercase().chars().rev().find(|c| "aeiouаеёиоуыэюя".contains(*c)).unwrap_or('a');
    let (f1, f2): (f64, f64) = match vowel {
        'i' | 'и' | 'ы' => (300.0, 2300.0),
        'e' | 'э' | 'е' => (500.0, 1900.0),
        'o' | 'о' | 'ё' => (500.0, 900.0),
        'u' | 'у' | 'ю' => (350.0, 800.0),
        _ => (800.0, 1200.0),
    };
    let freq = 440.0 * 2.0_f64.powf((midi as f64 - 69.0) / 12.0);
    let count = (duration * sample_rate as f64).ceil() as usize;
    let mut samples = Vec::with_capacity(count);
    for i in 0..count {
        let t = i as f64 / sample_rate as f64;
        let envelope = (t / 0.03).min(1.0).min(((duration - t) / 0.05).max(0.0));
        let mut value = 0.0;
        for harmonic in 1..=32 {
            let hz = freq * harmonic as f64;
            if hz >= sample_rate as f64 / 2.0 { break; }
            let resonance = |center: f64| 1.0 / (1.0 + ((hz - center) / 180.0).powi(2));
            let amplitude = (0.08 + resonance(f1) + 0.7 * resonance(f2)) / harmonic as f64;
            value += (std::f64::consts::TAU * hz * t).sin() * amplitude;
        }
        samples.push((value * envelope * 0.24) as f32);
    }
    Audio { sample_rate, channels: 1, samples }
}

pub fn instrument_note(kind: &str, midi: u8, lyric: &str, duration: f64, rate: u32) -> Audio {
    if kind == "vocal" || kind.is_empty() { return vowel_note(midi, lyric, duration, rate); }
    let frequency = 440.0 * 2.0_f64.powf((midi as f64 - 69.0) / 12.0);
    let samples = (0..(duration * rate as f64).ceil() as usize).map(|i| {
        let t = i as f64 / rate as f64;
        let phase = std::f64::consts::TAU * frequency * t;
        let attack = if kind == "synth" { 0.08 } else { 0.005 };
        let envelope = (t / attack).min(1.0) * ((duration - t) / 0.03).clamp(0.0, 1.0);
        let mut signal = 0.0;
        for h in 1..=16 {
            if frequency * h as f64 >= rate as f64 * 0.45 { break; }
            let (weight, decay) = match kind {
                "piano" => (1.0 / (h as f64).powf(1.8), 2.5 + h as f64 * 0.5),
                "electric" => (if h % 2 == 1 { 1.0 / h as f64 } else { 0.05 }, 1.8),
                "bass" => (1.0 / (h as f64).powf(3.0), 2.0),
                "organ" => (if [1, 2, 4, 8].contains(&h) { 0.3 } else { 0.0 }, 0.0),
                _ => (1.0 / (h as f64).powf(1.5), 0.2),
            };
            signal += (phase * h as f64).sin() * weight * (-t * decay).exp();
        }
        (signal * envelope * 0.25) as f32
    }).collect();
    Audio { sample_rate: rate, channels: 1, samples }
}

pub fn read_wav(path: &Path) -> Result<Audio, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    parse_wav(&bytes)
}

/// Parses PCM 16/24/32-bit and 32-bit float WAV files, mono or stereo.
pub fn parse_wav(bytes: &[u8]) -> Result<Audio, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("Not a WAV file".into());
    }
    let mut pos = 12;
    let mut format: Option<(u16, u16, u32, u16)> = None;
    let mut data: Option<&[u8]> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes([bytes[pos + 4], bytes[pos + 5], bytes[pos + 6], bytes[pos + 7]]) as usize;
        let start = pos + 8;
        let end = start.saturating_add(size).min(bytes.len());
        match id {
            b"fmt " => {
                if end - start < 16 {
                    return Err("Corrupt WAV fmt chunk".into());
                }
                let mut tag = u16::from_le_bytes([bytes[start], bytes[start + 1]]);
                let channels = u16::from_le_bytes([bytes[start + 2], bytes[start + 3]]);
                let rate = u32::from_le_bytes([bytes[start + 4], bytes[start + 5], bytes[start + 6], bytes[start + 7]]);
                let bits = u16::from_le_bytes([bytes[start + 14], bytes[start + 15]]);
                if tag == 0xFFFE && end - start >= 26 {
                    tag = u16::from_le_bytes([bytes[start + 24], bytes[start + 25]]);
                }
                format = Some((tag, channels, rate, bits));
            }
            b"data" => data = Some(&bytes[start..end]),
            _ => {}
        }
        pos = start.saturating_add(size).saturating_add(size & 1);
    }
    let (tag, channels, rate, bits) = format.ok_or("WAV has no fmt chunk")?;
    let raw = data.ok_or("WAV has no data chunk")?;
    if channels == 0 || channels > 2 {
        return Err(format!("Unsupported channel count: {channels} (mono or stereo only)"));
    }
    if rate == 0 {
        return Err("WAV sample rate is zero".into());
    }
    let samples: Vec<f32> = match (tag, bits) {
        (1, 16) => raw
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
            .collect(),
        (1, 24) => raw
            .chunks_exact(3)
            .map(|c| (i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8) as f32 / 8_388_608.0)
            .collect(),
        (1, 32) => raw
            .chunks_exact(4)
            .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f32 / 2_147_483_648.0)
            .collect(),
        (3, 32) => raw
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect(),
        _ => {
            return Err(format!(
                "Unsupported WAV format (tag {tag}, {bits} bit). Use PCM 16/24/32 or 32-bit float"
            ))
        }
    };
    Ok(Audio { sample_rate: rate, channels, samples })
}

pub fn write_wav(path: &Path, audio: &Audio, bits: u16) -> Result<(), String> {
    if bits != 16 && bits != 24 {
        return Err("Unsupported output bit depth (16 or 24)".into());
    }
    let bytes_per_sample = (bits / 8) as u32;
    let data_len = audio.samples.len() as u32 * bytes_per_sample;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&audio.channels.to_le_bytes());
    out.extend_from_slice(&audio.sample_rate.to_le_bytes());
    out.extend_from_slice(&(audio.sample_rate * audio.channels as u32 * bytes_per_sample).to_le_bytes());
    out.extend_from_slice(&(audio.channels * bits / 8).to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &sample in &audio.samples {
        let v = sample.clamp(-1.0, 1.0);
        if bits == 16 {
            out.extend_from_slice(&((v * 32767.0).round() as i16).to_le_bytes());
        } else {
            let i = (v * 8_388_607.0).round() as i32;
            out.extend_from_slice(&i.to_le_bytes()[0..3]);
        }
    }
    std::fs::write(path, out).map_err(|e| format!("Cannot write {}: {e}", path.display()))
}

fn to_stereo(audio: &Audio) -> Vec<[f32; 2]> {
    if audio.channels == 1 {
        audio.samples.iter().map(|&s| [s, s]).collect()
    } else {
        audio.samples.chunks_exact(2).map(|c| [c[0], c[1]]).collect()
    }
}

/// Linear-interpolation resampler. Good enough for previews and exports; not a mastering-grade SRC.
fn resample(frames: &[[f32; 2]], from: u32, to: u32) -> Vec<[f32; 2]> {
    if from == to || frames.is_empty() {
        return frames.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let out_len = (frames.len() as f64 / ratio).floor() as usize;
    (0..out_len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = pos.floor() as usize;
            let frac = (pos - idx as f64) as f32;
            let a = frames[idx];
            let b = frames[(idx + 1).min(frames.len() - 1)];
            [a[0] + (b[0] - a[0]) * frac, a[1] + (b[1] - a[1]) * frac]
        })
        .collect()
}

/// Equal-power pan law: pan -1 is hard left, 0 is centre, +1 is hard right.
fn pan_gains(volume: f32, pan: f32) -> (f32, f32) {
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
    (angle.cos() * volume, angle.sin() * volume)
}

/// Mixes tracks into one buffer starting at `from` seconds of the timeline.
pub fn mix(tracks: &[MixTrack], sample_rate: u32, channels: u16, from: f64) -> Audio {
    let mut master: Vec<[f32; 2]> = Vec::new();
    for track in tracks {
        let (gain_l, gain_r) = pan_gains(track.volume, track.pan);
        for clip in &track.clips {
            let frames = resample(&to_stereo(&clip.audio), clip.audio.sample_rate, sample_rate);
            let mut offset = ((clip.start - from) * sample_rate as f64).round() as i64;
            let mut skip = 0usize;
            if offset < 0 {
                skip = (-offset) as usize;
                offset = 0;
            }
            if skip >= frames.len() {
                continue;
            }
            let needed = offset as usize + frames.len() - skip;
            if master.len() < needed {
                master.resize(needed, [0.0; 2]);
            }
            for (i, frame) in frames[skip..].iter().enumerate() {
                let dst = &mut master[offset as usize + i];
                dst[0] += frame[0] * gain_l;
                dst[1] += frame[1] * gain_r;
            }
        }
    }
    let samples = if channels == 1 {
        master.iter().map(|f| (f[0] + f[1]) * 0.5).collect()
    } else {
        master.iter().flat_map(|f| [f[0], f[1]]).collect()
    };
    Audio { sample_rate, channels, samples }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(sample_rate: u32, channels: u16, frames: usize, value: f32) -> Audio {
        Audio { sample_rate, channels, samples: vec![value; frames * channels as usize] }
    }

    fn temp_wav(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("eve-audio-{}-{name}.wav", std::process::id()))
    }

    #[test]
    fn wav_round_trip_16_and_24_bit() {
        let audio = tone(22050, 2, 100, 0.5);
        for bits in [16u16, 24] {
            let path = temp_wav(&format!("rt{bits}"));
            write_wav(&path, &audio, bits).unwrap();
            let back = read_wav(&path).unwrap();
            assert_eq!(back.sample_rate, 22050);
            assert_eq!(back.channels, 2);
            assert_eq!(back.samples.len(), audio.samples.len());
            assert!((back.samples[0] - 0.5).abs() < 0.001);
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn rejects_non_wav_and_unsupported_bit_depth() {
        assert!(parse_wav(b"not a wav file at all").is_err());
        let path = temp_wav("bits");
        assert!(write_wav(&path, &tone(22050, 1, 4, 0.0), 8).is_err());
    }

    #[test]
    fn mix_places_clip_at_timeline_start() {
        let track = MixTrack {
            volume: 1.0,
            pan: -1.0,
            clips: vec![MixClip { audio: tone(1000, 1, 10, 0.8), start: 0.5 }],
        };
        let out = mix(&[track], 1000, 2, 0.0);
        // Start at 0.5 s is frame 500 of a stereo buffer; hard-left pan puts the signal in the left channel.
        assert_eq!(out.samples.len() / 2, 510);
        assert!((out.samples[1000] - 0.8).abs() < 1e-6);
        assert!(out.samples[1001].abs() < 1e-6);
    }

    #[test]
    fn mix_skips_audio_before_render_start() {
        let track = MixTrack {
            volume: 1.0,
            pan: 0.0,
            clips: vec![MixClip { audio: tone(1000, 1, 10, 0.5), start: 0.0 }],
        };
        let out = mix(&[track], 1000, 1, 0.005);
        assert_eq!(out.samples.len(), 5);
    }

    #[test]
    fn resample_halves_length_when_rate_halves() {
        let frames = vec![[0.0f32, 0.0f32]; 100];
        assert_eq!(resample(&frames, 44100, 22050).len(), 50);
    }

    #[test]
    fn vowel_note_has_expected_duration_and_changes_with_vowel() {
        let a = vowel_note(69, "a", 0.1, 22050);
        let i = vowel_note(69, "i", 0.1, 22050);
        assert_eq!(a.samples.len(), 2205);
        assert!(a.samples.iter().any(|s| s.abs() > 0.001));
        assert_ne!(a.samples, i.samples);
        assert_eq!(a.samples[0], 0.0);
    }

    #[test]
    fn instruments_are_distinct_finite_and_timed() {
        let piano = instrument_note("piano", 60, "", 0.2, 22050);
        for kind in ["electric", "bass", "organ", "synth"] {
            let audio = instrument_note(kind, 60, "", 0.2, 22050);
            assert_eq!(audio.samples.len(), 4410);
            assert!(audio.samples.iter().all(|s| s.is_finite() && s.abs() < 1.0));
            assert!(audio.samples.iter().any(|s| s.abs() > 0.01));
            assert_ne!(audio.samples, piano.samples);
        }
    }
}
