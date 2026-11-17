// Audio and synthesis engine for Energy Voice Editor.
// Uses Web Audio API for interactive piano, vocal formant synthesis, metronome and sequencer.

export type InstrumentId = "piano" | "vocal" | "guitar";

export const INSTRUMENTS: InstrumentId[] = ["piano", "vocal", "guitar"];

// MIDI note range shown on the piano keyboard (C3=48 to B5=83, 36 notes)
export const PIANO_NOTES = Array.from({ length: 37 }, (_, index) => 48 + index);

export const GUITAR_STRINGS = [64, 59, 55, 50, 45, 40];
export const GUITAR_FRETS = 12;

const NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

export function noteName(midi: number): string {
  return `${NOTE_NAMES[midi % 12]}${Math.floor(midi / 12) - 1}`;
}

export function isBlackKey(midi: number): boolean {
  return [1, 3, 6, 8, 10].includes(midi % 12);
}

export function frequency(midi: number): number {
  return 440 * 2 ** ((midi - 69) / 12);
}

let context: AudioContext | null = null;
let masterGain: GainNode | null = null;
let noise: AudioBuffer | null = null;

export function getAudioContext(): AudioContext {
  if (!context) {
    const AudioCtx = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
    context = new AudioCtx();
    masterGain = context.createGain();
    masterGain.gain.value = 0.8;
    masterGain.connect(context.destination);

    const length = Math.floor(context.sampleRate * 0.02);
    noise = context.createBuffer(1, length, context.sampleRate);
    const data = noise.getChannelData(0);
    for (let i = 0; i < length; i++) data[i] = Math.random() * 2 - 1;
  }
  if (context.state === "suspended") {
    void context.resume();
  }
  return context;
}

export function setMasterVolume(level: number) {
  const ctx = getAudioContext();
  if (masterGain) {
    masterGain.gain.setValueAtTime(Math.max(0, Math.min(1, level)), ctx.currentTime);
  }
}

// Formants [F1, F2] for vowels
const FORMANT_MAP: Record<string, [number, number]> = {
  a: [800, 1200],
  la: [800, 1200],
  na: [800, 1200],
  ra: [800, 1200],
  o: [500, 900],
  so: [500, 900],
  u: [350, 800],
  ru: [350, 800],
  e: [500, 2000],
  i: [300, 2500],
  ki: [300, 2500],
  mi: [300, 2500],
};

function getFormants(lyric?: string): [number, number] {
  if (!lyric) return [800, 1200];
  const cleaned = lyric.trim().toLowerCase();
  if (FORMANT_MAP[cleaned]) return FORMANT_MAP[cleaned];
  // Match vowel ending
  for (const char of cleaned.split("").reverse()) {
    if (FORMANT_MAP[char]) return FORMANT_MAP[char];
  }
  return [800, 1200];
}

/** Synthesize a Vocaloid-style vocal tone using formant filtering */
export function playVoiceNote(
  midi: number,
  lyric = "la",
  durationSec = 0.5,
  volume = 0.8,
  pan = 0,
  startTime?: number,
) {
  const ctx = getAudioContext();
  const freq = frequency(midi);
  const now = startTime ?? ctx.currentTime;

  const voiceOut = ctx.createGain();
  voiceOut.gain.setValueAtTime(0.0001, now);
  voiceOut.gain.linearRampToValueAtTime(0.3 * volume, now + 0.04);
  voiceOut.gain.setValueAtTime(0.3 * volume, Math.max(now + 0.04, now + durationSec - 0.05));
  voiceOut.gain.linearRampToValueAtTime(0.0001, now + durationSec);

  // Panner
  let nodeToConnect: AudioNode = voiceOut;
  if (typeof ctx.createStereoPanner === "function") {
    const panner = ctx.createStereoPanner();
    panner.pan.setValueAtTime(Math.max(-1, Math.min(1, pan)), now);
    voiceOut.connect(panner);
    nodeToConnect = panner;
  }
  nodeToConnect.connect(masterGain ?? ctx.destination);

  // Glottal pulse source (sawtooth + square blend)
  const osc1 = ctx.createOscillator();
  osc1.type = "sawtooth";
  osc1.frequency.setValueAtTime(freq, now);

  const osc2 = ctx.createOscillator();
  osc2.type = "square";
  osc2.frequency.setValueAtTime(freq * 1.002, now);

  // Vibrato LFO
  const lfo = ctx.createOscillator();
  const lfoGain = ctx.createGain();
  lfo.frequency.setValueAtTime(5.5, now); // ~5.5 Hz natural vibrato
  lfoGain.gain.setValueAtTime(0, now);
  lfoGain.gain.linearRampToValueAtTime(freq * 0.02, now + Math.min(0.3, durationSec * 0.6));
  lfo.connect(lfoGain);
  lfoGain.connect(osc1.frequency);
  lfoGain.connect(osc2.frequency);
  lfo.start(now);
  lfo.stop(now + durationSec + 0.1);

  // Formants F1 & F2
  const [f1Freq, f2Freq] = getFormants(lyric);

  const f1 = ctx.createBiquadFilter();
  f1.type = "bandpass";
  f1.frequency.setValueAtTime(f1Freq, now);
  f1.Q.setValueAtTime(4.5, now);

  const f2 = ctx.createBiquadFilter();
  f2.type = "bandpass";
  f2.frequency.setValueAtTime(f2Freq, now);
  f2.Q.setValueAtTime(5.5, now);

  osc1.connect(f1);
  osc1.connect(f2);
  osc2.connect(f1);
  osc2.connect(f2);

  f1.connect(voiceOut);
  f2.connect(voiceOut);

  osc1.start(now);
  osc2.start(now);
  osc1.stop(now + durationSec + 0.05);
  osc2.stop(now + durationSec + 0.05);

  setTimeout(() => {
    f1.disconnect();
    f2.disconnect();
    voiceOut.disconnect();
  }, (durationSec + 0.5) * 1000);
}

export function playNote(instrument: InstrumentId, midi: number, durationSec = 0.5, volume = 0.8) {
  if (instrument === "vocal") {
    playVoiceNote(midi, "la", durationSec, volume);
    return;
  }

  const ctx = getAudioContext();
  const freq = frequency(midi);
  const now = ctx.currentTime;
  const out = ctx.createGain();
  out.gain.value = 0.35 * volume;
  out.connect(masterGain ?? ctx.destination);

  if (instrument === "piano") {
    for (const [harmonic, level] of [[1, 0.5], [2, 0.25], [3, 0.1], [4, 0.05]]) {
      const osc = ctx.createOscillator();
      const env = ctx.createGain();
      osc.frequency.value = freq * harmonic;
      env.gain.setValueAtTime(level, now);
      env.gain.exponentialRampToValueAtTime(0.0001, now + Math.max(0.4, durationSec * 2) / harmonic);
      osc.connect(env).connect(out);
      osc.start(now);
      osc.stop(now + Math.max(0.5, durationSec * 2));
    }
    return;
  }

  // Guitar (Karplus-Strong)
  const delay = ctx.createDelay(1);
  delay.delayTime.value = 1 / freq;
  const feedback = ctx.createGain();
  feedback.gain.value = 0.994;
  const tone = ctx.createBiquadFilter();
  tone.type = "lowpass";
  tone.frequency.value = 5000;
  delay.connect(feedback).connect(delay);
  delay.connect(tone).connect(out);
  const burst = ctx.createBufferSource();
  burst.buffer = noise;
  burst.connect(delay);
  burst.start(now);
  burst.stop(now + 0.02);
  out.gain.setValueAtTime(0.35 * volume, now + 1.5);
  out.gain.linearRampToValueAtTime(0, now + 2);

  setTimeout(() => {
    feedback.disconnect();
    delay.disconnect();
    tone.disconnect();
    out.disconnect();
  }, 2200);
}

/** Metronome click sound */
export function playMetronomeClick(accent = false) {
  const ctx = getAudioContext();
  const now = ctx.currentTime;
  const osc = ctx.createOscillator();
  const gain = ctx.createGain();
  osc.frequency.setValueAtTime(accent ? 1600 : 900, now);
  gain.gain.setValueAtTime(accent ? 0.35 : 0.2, now);
  gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.04);
  osc.connect(gain).connect(masterGain ?? ctx.destination);
  osc.start(now);
  osc.stop(now + 0.05);
}

/** Helper to encode AudioBuffer to WAV format */
export function audioBufferToWav(buffer: AudioBuffer): Uint8Array {
  const numOfChan = buffer.numberOfChannels;
  const length = buffer.length * numOfChan * 2 + 44;
  const out = new DataView(new ArrayBuffer(length));
  let offset = 0;

  function writeString(str: string) {
    for (let i = 0; i < str.length; i++) {
      out.setUint8(offset++, str.charCodeAt(i));
    }
  }

  writeString("RIFF");
  out.setUint32(offset, length - 8, true);
  offset += 4;
  writeString("WAVE");
  writeString("fmt ");
  out.setUint32(offset, 16, true);
  offset += 4;
  out.setUint16(offset, 1, true); // PCM
  offset += 2;
  out.setUint16(offset, numOfChan, true);
  offset += 2;
  out.setUint32(offset, buffer.sampleRate, true);
  offset += 4;
  out.setUint32(offset, buffer.sampleRate * 2 * numOfChan, true);
  offset += 4;
  out.setUint16(offset, numOfChan * 2, true);
  offset += 2;
  out.setUint16(offset, 16, true); // 16-bit
  offset += 2;
  writeString("data");
  out.setUint32(offset, length - offset - 4, true);
  offset += 4;

  const channels: Float32Array[] = [];
  for (let i = 0; i < numOfChan; i++) {
    channels.push(buffer.getChannelData(i));
  }

  for (let i = 0; i < buffer.length; i++) {
    for (let channel = 0; channel < numOfChan; channel++) {
      let sample = Math.max(-1, Math.min(1, channels[channel][i]));
      sample = sample < 0 ? sample * 0x8000 : sample * 0x7fff;
      out.setInt16(offset, sample, true);
      offset += 2;
    }
  }

  return new Uint8Array(out.buffer);
}
