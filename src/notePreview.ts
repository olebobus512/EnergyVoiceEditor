import { invoke } from "@tauri-apps/api/core";
import { getAudioContext } from "./instruments";
import { doc, type Track } from "./project";
import { logError } from "./logger";
let sequence = 0;
let current: AudioBufferSourceNode | undefined;
export async function previewTrackNote(track: Track | undefined, midi: number, lyric = "la") {
  const ticket = ++sequence;
  const context = getAudioContext();
  current?.stop(); current = undefined;
  try {
    const samples = await invoke<number[]>("preview_note", { instrument: track?.instrument || (track?.type === "voice" ? "vocal" : "piano"), midi, lyric });
    if (ticket !== sequence) return;
    const buffer = context.createBuffer(1, samples.length, 22050);
    buffer.copyToChannel(new Float32Array(samples), 0);
    const source = context.createBufferSource();
    const gain = context.createGain();
    const pan = context.createStereoPanner();
    gain.gain.value = (track?.volume ?? 0.8) * doc.masterVolume;
    pan.pan.value = track?.pan ?? 0;
    source.buffer = buffer; source.connect(gain).connect(pan).connect(context.destination);
    current = source;
    source.onended = () => { source.disconnect(); gain.disconnect(); pan.disconnect(); if (current === source) current = undefined; };
    source.start();
  } catch (e) { logError(String(e)); }
}
