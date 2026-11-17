<script setup lang="ts">
import { computed, ref } from "vue";
import { t } from "../i18n";
import {
  GUITAR_FRETS,
  GUITAR_STRINGS,
  PIANO_NOTES,
  isBlackKey,
  noteName,
  playNote,
  type InstrumentId,
} from "../instruments";

const props = defineProps<{ instrument: InstrumentId }>();

const recent = ref<string[]>([]);
const pianoKeys = computed(() => [...PIANO_NOTES].reverse());
const frets = Array.from({ length: GUITAR_FRETS + 1 }, (_, fret) => fret);

function play(midi: number) {
  playNote(props.instrument, midi);
  recent.value = [noteName(midi), ...recent.value].slice(0, 12);
}
</script>

<template>
  <div class="instrument-panel">
    <div v-if="instrument === 'piano'" class="piano-keys">
      <button
        v-for="midi in pianoKeys"
        :key="midi"
        class="key"
        :class="isBlackKey(midi) ? 'black' : 'white'"
        :title="noteName(midi)"
        @pointerdown.prevent="play(midi)"
      >
        <span v-if="midi % 12 === 0">{{ noteName(midi) }}</span>
      </button>
    </div>

    <div class="music-area">
      <div class="hint">{{ instrument === "piano" ? t("pianoHint") : t("guitarHint") }}</div>
      <div class="recent">{{ t("recentNotes") }}: {{ recent.join("  ") || t("noNotes") }}</div>

      <div v-if="instrument === 'guitar'" class="fretboard">
        <div v-for="(open, string) in GUITAR_STRINGS" :key="string" class="string-row">
          <span class="string-name">{{ noteName(open) }}</span>
          <button
            v-for="fret in frets"
            :key="fret"
            class="fret"
            :title="noteName(open + fret)"
            @pointerdown.prevent="play(open + fret)"
          ></button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.instrument-panel {
  position: absolute;
  inset: 0;
  color: var(--text);
}

.piano-keys {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 97px;
  display: flex;
  flex-direction: column;
  background: var(--panel);
}

.key {
  flex: 1;
  min-height: 0;
  padding: 0 4px;
  border: none;
  border-bottom: 1px solid #888;
  font-size: 10px;
  text-align: left;
  cursor: pointer;
}

.key.white {
  width: 100%;
  background: #f2f2f2;
  color: #000;
}

.key.black {
  width: 60%;
  background: #1a1a1a;
  color: #fff;
  border-bottom-color: #000;
}

.key:active {
  background: #9ecbff;
}

.music-area {
  position: absolute;
  left: 105px;
  top: 0;
  right: 0;
  bottom: 0;
  padding: 8px 12px;
  box-sizing: border-box;
  font-size: 12px;
}

.hint {
  color: var(--text);
  opacity: 0.7;
}

.recent {
  margin-top: 4px;
  font-family: Consolas, "Courier New", monospace;
}

.fretboard {
  position: absolute;
  left: 12px;
  right: 12px;
  top: 56px;
  bottom: 12px;
  display: flex;
  flex-direction: column;
}

.string-row {
  position: relative;
  flex: 1;
  display: flex;
  align-items: stretch;
}

.string-row::before {
  content: "";
  position: absolute;
  left: 44px;
  right: 0;
  top: 50%;
  height: 1px;
  background: var(--text);
  opacity: 0.6;
  pointer-events: none;
}

.string-name {
  width: 44px;
  display: flex;
  align-items: center;
  font-family: Consolas, "Courier New", monospace;
}

.fret {
  flex: 1;
  border: none;
  border-right: 2px solid #666;
  background: transparent;
  cursor: pointer;
}

.fret:hover,
.fret:active {
  background: color-mix(in srgb, var(--accent) 25%, transparent);
}
</style>
