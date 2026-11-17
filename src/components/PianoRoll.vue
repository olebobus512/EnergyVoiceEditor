<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from "vue";
import {
  addNoteToActiveClip,
  deleteNoteFromActiveClip,
  doc,
  mutate,
  getActiveClip,
  selectedTrackObject,
  trackOf,
  updateNoteInActiveClip,
  type Note,
} from "../project";
import { isBlackKey, noteName } from "../instruments";
import { instrumentPlugins } from "../extensions";
import { previewTrackNote } from "../notePreview";
import { t } from "../i18n";

const props = defineProps<{
  tempo?: number;
}>();

const emit = defineEmits<{
  selectNote: [id: string];
}>();

const instrumentKinds = ["vocal", "piano", "electric", "bass", "organ", "synth"] as const;
function setInstrument(event: Event) {
  const track = activeTrack.value;
  if (track) mutate(() => { track.instrument = (event.target as HTMLSelectElement).value as typeof instrumentKinds[number]; });
}
// Tool state
const tool = ref<"select" | "draw" | "erase">("select");
const snap = ref(true);
const snapDivision = ref<number>(16); // 4 = 1/4, 8 = 1/8, 16 = 1/16, 32 = 1/32
const snapOptions = [4, 8, 16, 32];

// Piano keys range: C3 (48) to B5 (83) -> 36 keys
const MIN_MIDI = 48;
const MAX_MIDI = 83;
const totalKeys = MAX_MIDI - MIN_MIDI + 1;
// Piano keys in descending order (highest pitch at the top)
const pianoMidis = computed(() =>
  Array.from({ length: totalKeys }, (_, index) => MAX_MIDI - index),
);

// Dimensions
const ROW_HEIGHT = 20;
const PX_PER_BEAT = 60; // 1 beat = 60px. A 1/16 note = 0.25 beat = 15px.
const TOTAL_BARS = 32;
const BEATS_PER_BAR = 4;
const totalBeats = TOTAL_BARS * BEATS_PER_BAR;
const gridWidth = computed(() => totalBeats * PX_PER_BEAT);
const gridHeight = computed(() => totalKeys * ROW_HEIGHT);

const gridScrollEl = ref<HTMLElement | null>(null);
const keysEl = ref<HTMLElement | null>(null);

// Active clip & notes
const activeClip = computed(() => getActiveClip());
const activeTrack = computed(() => (activeClip.value ? trackOf(activeClip.value.id) : undefined) ?? selectedTrackObject() ?? doc.project.tracks[0]);
const trackColor = computed(() => activeTrack.value?.color || "#ff5c7a");
const notes = computed<Note[]>(() => activeClip.value?.notes ?? []);

// Note editing / dragging state
const editingNoteId = ref<string | null>(null);
const editingLyric = ref("");
const lyricInputEl = ref<HTMLInputElement | null>(null);

let dragging: {
  noteId: string;
  type: "move" | "resize";
  origStart: number;
  origDuration: number;
  origMidi: number;
  startX: number;
  startY: number;
} | null = null;

// Expressions bar
const showCurves = ref(false);
const activeCurve = ref("PIT");
const curveParams = ["PIT", "PBS", "DYN", "BRE", "BRI", "CLE", "GEN", "OPE"] as const;

function playKey(midi: number) {
  void previewTrackNote(activeTrack.value, midi);
}

// Convert beat to pixel
function beatToX(beat: number): number {
  return beat * PX_PER_BEAT;
}

// Convert pixel to beat with snap
function xToBeat(x: number, useSnap = true): number {
  const rawBeat = Math.max(0, x / PX_PER_BEAT);
  if (!useSnap || !snap.value) return rawBeat;
  const step = 4 / snapDivision.value; // e.g. for 1/16: 4 / 16 = 0.25 beat
  return Math.round(rawBeat / step) * step;
}

// Convert midi to Y
function midiToY(midi: number): number {
  const index = MAX_MIDI - midi;
  return index * ROW_HEIGHT;
}

// Convert Y to midi
function yToMidi(y: number): number {
  const index = Math.floor(y / ROW_HEIGHT);
  const midi = MAX_MIDI - index;
  return Math.max(MIN_MIDI, Math.min(MAX_MIDI, midi));
}

// Handle scroll sync between keys and grid
function onGridScroll(event: Event) {
  const target = event.target as HTMLElement;
  if (keysEl.value) {
    keysEl.value.scrollTop = target.scrollTop;
  }
}

// Piano Playhead X position (based on doc.playhead in seconds)
const bpm = computed(() => doc.project.tempo || 120);
const playheadBeat = computed(() => {
  // seconds * (bpm / 60) = beats
  const beatsPerSec = bpm.value / 60;
  // If activeClip has start, relative to clip
  const clipStartSec = activeClip.value?.start ?? 0;
  const relSec = Math.max(0, doc.playhead - clipStartSec);
  return relSec * beatsPerSec;
});
const playheadX = computed(() => beatToX(playheadBeat.value));

// Grid interaction
function onGridPointerDown(event: PointerEvent) {
  if (editingNoteId.value) {
    finishEditLyric();
  }
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  const clickX = event.clientX - rect.left;
  const clickY = event.clientY - rect.top;
  const midi = yToMidi(clickY);
  const startBeat = xToBeat(clickX, true);

  if (tool.value === "draw") {
    const defaultDuration = 4 / snapDivision.value; // default 1 snap unit
    const newNote = addNoteToActiveClip({
      midi,
      start: startBeat,
      duration: defaultDuration,
      lyric: "la",
    });
    void previewTrackNote(activeTrack.value, midi);
    if (newNote) {
      doc.selectedNote = newNote.id;
    }
  } else {
    // If clicked empty grid in select mode, deselect note
    doc.selectedNote = null;
  }
}

function onNotePointerDown(event: PointerEvent, note: Note, isResize = false) {
  event.stopPropagation();
  doc.selectedNote = note.id;
  emit("selectNote", note.id);

  if (tool.value === "erase") {
    deleteNoteFromActiveClip(note.id);
    return;
  }

  // Play preview
  void previewTrackNote(activeTrack.value, note.midi, note.lyric);

  // Setup dragging
  dragging = {
    noteId: note.id,
    type: isResize ? "resize" : "move",
    origStart: note.start,
    origDuration: note.duration,
    origMidi: note.midi,
    startX: event.clientX,
    startY: event.clientY,
  };

  const onPointerMove = (e: PointerEvent) => {
    if (!dragging) return;
    const deltaX = e.clientX - dragging.startX;
    const deltaY = e.clientY - dragging.startY;
    const deltaBeats = deltaX / PX_PER_BEAT;

    if (dragging.type === "resize") {
      const step = snap.value ? 4 / snapDivision.value : 0.05;
      const rawDuration = Math.max(step, dragging.origDuration + deltaBeats);
      const newDuration = snap.value ? Math.round(rawDuration / step) * step : rawDuration;
      updateNoteInActiveClip(dragging.noteId, { duration: Math.max(step, newDuration) });
    } else {
      const step = snap.value ? 4 / snapDivision.value : 0.05;
      const rawStart = Math.max(0, dragging.origStart + deltaBeats);
      const newStart = snap.value ? Math.round(rawStart / step) * step : rawStart;

      const deltaMidi = -Math.round(deltaY / ROW_HEIGHT);
      const newMidi = Math.max(MIN_MIDI, Math.min(MAX_MIDI, dragging.origMidi + deltaMidi));

      updateNoteInActiveClip(dragging.noteId, { start: newStart, midi: newMidi });
    }
  };

  const onPointerUp = () => {
    dragging = null;
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
  };

  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp);
}

function startEditLyric(note: Note) {
  editingNoteId.value = note.id;
  editingLyric.value = note.lyric || "la";
  nextTick(() => {
    lyricInputEl.value?.focus();
    lyricInputEl.value?.select();
  });
}

function finishEditLyric() {
  if (editingNoteId.value) {
    updateNoteInActiveClip(editingNoteId.value, { lyric: editingLyric.value.trim() });
    editingNoteId.value = null;
  }
}

// Center view around middle octave (C4/C5) on mounted
onMounted(() => {
  if (gridScrollEl.value) {
    gridScrollEl.value.scrollTop = ROW_HEIGHT * 12; // C5 around top/center
  }
});
</script>

<template>
  <div class="piano-roll-container">
    <!-- Top toolbar -->
    <header class="roll-toolbar">
      <div class="tabs">
        <select class="snap-select" :value="activeTrack?.instrument || (activeTrack?.type === 'voice' ? 'vocal' : 'piano')" @change="setInstrument">
          <option v-for="kind in instrumentKinds" :key="kind" :value="kind">{{ t(kind) }}</option>
          <option v-for="plugin in instrumentPlugins" :key="plugin.id" :value="`plugin:${plugin.id}`">{{ plugin.name }}</option>
        </select>
      </div>

      <div class="divider"></div>

      <!-- Tools -->
      <div class="tool-group">
        <button
          class="btn-tool"
          :class="{ active: tool === 'select' }"
          title="Select (V)"
          @click="tool = 'select'"
        >Select</button>
        <button
          class="btn-tool"
          :class="{ active: tool === 'draw' }"
          title="Draw (D)"
          @click="tool = 'draw'"
        >Draw</button>
        <button
          class="btn-tool"
          :class="{ active: tool === 'erase' }"
          title="Erase (E)"
          @click="tool = 'erase'"
        >Erase</button>
      </div>

      <div class="divider"></div>

      <!-- Snap & Grid -->
      <div class="snap-group">
        <button
          class="btn-snap"
          :class="{ active: snap }"
          title="Toggle Snap (S)"
          @click="snap = !snap"
        >Snap</button>
        <select v-model.number="snapDivision" class="snap-select">
          <option v-for="opt in snapOptions" :key="opt" :value="opt">1/{{ opt }}</option>
        </select>
      </div>

      <div class="divider"></div>

      <!-- Active clip info -->
      <div class="clip-info">
        <span class="info-label">PART:</span>
        <span class="info-val">{{ activeClip?.name ?? "No Part Selected" }}</span>
      </div>

      <span v-if="(activeTrack?.instrument || (activeTrack?.type === 'voice' ? 'vocal' : 'piano')) === 'vocal'" class="synth-hint" :title="t('vowelSynthHint')">{{ t('vowelSynthLabel') }}</span>

      <div class="spacer"></div>

      <!-- Toggle Expression Curves -->
      <button
        class="btn-curves-toggle"
        :class="{ active: showCurves }"
        @click="showCurves = !showCurves"
      >
        Parameters {{ showCurves ? "▾" : "▴" }}
      </button>
    </header>

    <!-- Main Piano Roll Body -->
    <div class="roll-body">
      <!-- Piano Keys Column -->
      <div ref="keysEl" class="piano-keys-col">
        <div class="keys-ruler-space"></div>
        <div class="keys-list">
          <div
            v-for="midi in pianoMidis"
            :key="midi"
            class="piano-key"
            :class="{ black: isBlackKey(midi), white: !isBlackKey(midi), ckey: midi % 12 === 0 }"
            @pointerdown.prevent="playKey(midi)"
          >
            <span v-if="midi % 12 === 0" class="key-label">{{ noteName(midi) }}</span>
          </div>
        </div>
      </div>

      <!-- Interactive Grid -->
      <div ref="gridScrollEl" class="grid-scroll-area" @scroll="onGridScroll">
        <!-- Bars Ruler -->
        <div class="grid-ruler" :style="{ width: `${gridWidth}px` }">
          <div
            v-for="bar in TOTAL_BARS"
            :key="bar"
            class="ruler-bar-label"
            :style="{ left: `${(bar - 1) * BEATS_PER_BAR * PX_PER_BEAT}px` }"
          >
            Bar {{ bar }}
          </div>
        </div>

        <!-- Note Canvas & Rows -->
        <div
          class="grid-canvas"
          :style="{ width: `${gridWidth}px`, height: `${gridHeight}px` }"
          @pointerdown="onGridPointerDown"
        >
          <!-- Alternating pitch rows -->
          <div
            v-for="midi in pianoMidis"
            :key="midi"
            class="grid-row"
            :class="{ black: isBlackKey(midi) }"
            :style="{ height: `${ROW_HEIGHT}px` }"
          ></div>

          <!-- Vertical beat & bar lines -->
          <div
            v-for="beat in totalBeats"
            :key="`beat-${beat}`"
            class="grid-vert-line"
            :class="{ barline: (beat - 1) % BEATS_PER_BAR === 0 }"
            :style="{ left: `${(beat - 1) * PX_PER_BEAT}px` }"
          ></div>

          <!-- Notes -->
          <div
            v-for="note in notes"
            :key="note.id"
            class="note-rect"
            :class="{ selected: doc.selectedNote === note.id }"
            :style="{
              left: `${beatToX(note.start)}px`,
              top: `${midiToY(note.midi)}px`,
              width: `${Math.max(14, beatToX(note.duration))}px`,
              height: `${ROW_HEIGHT - 1}px`,
              backgroundColor: trackColor,
            }"
            @pointerdown="onNotePointerDown($event, note, false)"
            @dblclick.stop="startEditLyric(note)"
          >
            <!-- Lyric text inside note -->
            <span v-if="editingNoteId !== note.id" class="note-lyric">{{ note.lyric || "la" }}</span>
            <input
              v-else
              ref="lyricInputEl"
              v-model="editingLyric"
              class="lyric-input"
              @keydown.enter="finishEditLyric"
              @blur="finishEditLyric"
              @pointerdown.stop
            />

            <!-- Resize handle on right edge -->
            <div
              class="resize-handle"
              @pointerdown.stop="onNotePointerDown($event, note, true)"
            ></div>
          </div>

          <!-- Piano Roll Playhead -->
          <div
            class="roll-playhead"
            :style="{ left: `${playheadX}px` }"
          ></div>
        </div>
      </div>
    </div>

    <!-- Expression parameters drawer (PIT, PBS, DYN, etc.) -->
    <div v-if="showCurves" class="curves-drawer">
      <div class="curves-header">
        <span class="curves-title">PARAMETERS</span>
        <div class="curve-tabs">
          <button
            v-for="param in curveParams"
            :key="param"
            class="curve-tab"
            :class="{ active: activeCurve === param }"
            @click="activeCurve = param"
          >{{ param }}</button>
        </div>
      </div>
      <div class="curves-canvas">
        <div class="curve-lane-hint">Curve editor for {{ activeCurve }}: Draw pitch bends and vocal dynamics</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.piano-roll-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--panel);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  overflow: hidden;
  box-sizing: border-box;
}

/* Toolbar */
.roll-toolbar {
  display: flex;
  align-items: center;
  height: 42px;
  padding: 0 12px;
  gap: 10px;
  background: var(--background);
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  flex: none;
}

.tabs {
  display: flex;
  gap: 4px;
}

.tab-item {
  height: 28px;
  padding: 0 14px;
  border: none;
  border-radius: 6px;
  background: var(--button);
  color: var(--accent);
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.5px;
  cursor: pointer;
}

.divider {
  width: 1px;
  height: 20px;
  background: rgba(255, 255, 255, 0.1);
}

.tool-group {
  display: flex;
  gap: 4px;
}

.btn-tool {
  height: 28px;
  padding: 0 12px;
  border: none;
  border-radius: 6px;
  background: var(--button);
  color: color-mix(in srgb, var(--text) 60%, transparent);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.15s ease;
}

.btn-tool:hover {
  color: var(--text);
  background: var(--bar);
}

.btn-tool.active {
  background: var(--accent);
  color: #1a1405;
}

.snap-group {
  display: flex;
  align-items: center;
  gap: 6px;
}

.btn-snap {
  height: 28px;
  padding: 0 10px;
  border: none;
  border-radius: 6px;
  background: var(--button);
  color: color-mix(in srgb, var(--text) 60%, transparent);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}

.btn-snap.active {
  background: rgba(255, 209, 102, 0.18);
  color: var(--accent);
}

.snap-select {
  height: 28px;
  padding: 0 8px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  background: var(--button);
  color: var(--text);
  font-size: 12px;
  outline: none;
  cursor: pointer;
}

.clip-info {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  font-family: "JetBrains Mono", monospace;
}

.info-label {
  color: color-mix(in srgb, var(--text) 60%, transparent);
}

.info-val {
  color: var(--accent);
  font-weight: bold;
}

.spacer {
  flex: 1;
}

.synth-hint {
  color: color-mix(in srgb, var(--text) 60%, transparent);
  font-size: 10px;
  white-space: nowrap;
}

.btn-curves-toggle {
  height: 28px;
  padding: 0 12px;
  border: none;
  border-radius: 6px;
  background: var(--button);
  color: color-mix(in srgb, var(--text) 60%, transparent);
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
}

.btn-curves-toggle.active {
  color: var(--accent);
}

/* Body */
.roll-body {
  display: flex;
  flex: 1;
  min-height: 0;
  position: relative;
  overflow: hidden;
}

/* Left Piano Keys */
.piano-keys-col {
  width: 76px;
  flex: none;
  background: #0b0d12;
  border-right: 1px solid rgba(255, 255, 255, 0.1);
  overflow: hidden;
  user-select: none;
  display: flex;
  flex-direction: column;
}

.keys-ruler-space {
  height: 26px;
  background: var(--background);
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  flex: none;
}

.keys-list {
  display: flex;
  flex-direction: column;
}

.piano-key {
  height: 20px;
  box-sizing: border-box;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding-right: 6px;
  border-bottom: 1px solid rgba(0, 0, 0, 0.3);
  cursor: pointer;
  transition: opacity 0.1s ease;
}

.piano-key.white {
  background: #eef0f6;
  color: #2b3145;
}

.piano-key.black {
  background: #12141a;
  color: var(--text);
  width: 65%;
  align-self: flex-start;
  border-right: 2px solid #000;
  border-radius: 0 3px 3px 0;
  z-index: 2;
}

.piano-key:active {
  background: var(--accent) !important;
  color: #1a1405 !important;
}

.key-label {
  font-size: 9px;
  font-weight: 800;
  font-family: "JetBrains Mono", monospace;
}

/* Grid Scroll Area */
.grid-scroll-area {
  flex: 1;
  min-width: 0;
  overflow: auto;
  position: relative;
  background: var(--background);
}

.grid-ruler {
  position: sticky;
  top: 0;
  z-index: 10;
  height: 26px;
  background: var(--bar);
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.ruler-bar-label {
  position: absolute;
  top: 4px;
  font-size: 11px;
  font-family: "JetBrains Mono", monospace;
  font-weight: 600;
  color: color-mix(in srgb, var(--text) 60%, transparent);
  padding-left: 6px;
  border-left: 1px solid rgba(255, 255, 255, 0.15);
}

.grid-canvas {
  position: relative;
  user-select: none;
}

.grid-row {
  box-sizing: border-box;
  border-bottom: 1px solid rgba(255, 255, 255, 0.03);
  background: rgba(255, 255, 255, 0.015);
}

.grid-row.black {
  background: rgba(0, 0, 0, 0.25);
}

.grid-vert-line {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 1px;
  background: rgba(255, 255, 255, 0.04);
  pointer-events: none;
}

.grid-vert-line.barline {
  background: rgba(255, 255, 255, 0.12);
}

/* Notes */
.note-rect {
  position: absolute;
  border-radius: 4px;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  padding: 0 4px;
  box-sizing: border-box;
  cursor: grab;
  z-index: 5;
  transition: box-shadow 0.12s ease;
}

.note-rect:active {
  cursor: grabbing;
}

.note-rect.selected {
  outline: 2px solid #ffffff;
  box-shadow: 0 0 10px rgba(255, 255, 255, 0.6);
  z-index: 8;
}

.note-lyric {
  font-family: Inter, sans-serif;
  font-size: 11px;
  font-weight: 700;
  color: #1a1405;
  pointer-events: none;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.lyric-input {
  width: 100%;
  height: 16px;
  border: none;
  border-radius: 2px;
  background: #ffffff;
  color: #000000;
  font-size: 11px;
  font-weight: bold;
  padding: 0 2px;
  outline: none;
}

.resize-handle {
  position: absolute;
  right: 0;
  top: 0;
  bottom: 0;
  width: 6px;
  cursor: ew-resize;
}

.resize-handle:hover {
  background: rgba(255, 255, 255, 0.4);
  border-radius: 0 4px 4px 0;
}

/* Playhead */
.roll-playhead {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 2px;
  background: var(--accent);
  box-shadow: 0 0 8px var(--accent);
  z-index: 9;
  pointer-events: none;
}

/* Curves drawer */
.curves-drawer {
  height: 120px;
  background: var(--background);
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  display: flex;
  flex-direction: column;
}

.curves-header {
  display: flex;
  align-items: center;
  height: 30px;
  padding: 0 12px;
  gap: 12px;
  background: var(--bar);
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.curves-title {
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.8px;
  color: color-mix(in srgb, var(--text) 60%, transparent);
}

.curve-tabs {
  display: flex;
  gap: 4px;
}

.curve-tab {
  height: 22px;
  padding: 0 8px;
  border: none;
  border-radius: 4px;
  background: var(--button);
  color: color-mix(in srgb, var(--text) 60%, transparent);
  font-size: 10px;
  font-weight: 700;
  cursor: pointer;
}

.curve-tab.active {
  background: var(--accent);
  color: var(--background);
}

.curves-canvas {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.3);
}

.curve-lane-hint {
  font-size: 11px;
  color: #555e75;
  font-family: "JetBrains Mono", monospace;
}
</style>
