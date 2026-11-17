<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import Splitter from "./Splitter.vue";
import {
  acceptsClips,
  beginChange,
  doc,
  moveTrackTo,
  mutate,
  timelineEnd,
  toggleTrackFlag,
  trackRowHeight,
  type Clip,
  type Track,
} from "../project";
import type { ArrangementContext } from "../contextmenu";
import { t } from "../i18n";

const props = defineProps<{
  pxPerSec: number;
  trackPanel: boolean;
  trackPanelWidth: number;
  tool: "select" | "draw" | "erase";
  snap: boolean;
  gridSec: number;
}>();

const emit = defineEmits<{
  trackDblclick: [id: string];
  headResize: [delta: number];
  headResizeEnd: [];
  context: [request: ArrangementContext & { x: number; y: number }];
  addTrackMenu: [x: number, y: number];
  zoom: [pxPerSec: number];
}>();

const RULER_HEIGHT = 26;
const TAIL_HEIGHT = 56;

const timelineEl = ref<HTMLElement | null>(null);
const headEl = ref<HTMLElement | null>(null);
const rowsEl = ref<HTMLElement | null>(null);
const lanesEl = ref<HTMLElement | null>(null);
const rulerEl = ref<HTMLElement | null>(null);

const tracks = computed(() => doc.project.tracks);
const heights = computed(() => tracks.value.map((track) => trackRowHeight(track)));
const tops = computed(() => {
  let y = 0;
  return heights.value.map((height) => {
    const top = y;
    y += height;
    return top;
  });
});
const totalHeight = computed(() => heights.value.reduce((sum, height) => sum + height, 0));

const contentWidth = computed(() => Math.max(1200, (timelineEnd() + 20) * props.pxPerSec + 200));
const bpm = computed(() => doc.project.tempo || 120);
const barDuration = computed(() => (60 / bpm.value) * 4); // 4 beats per bar
const barCount = computed(() => Math.max(16, Math.ceil(contentWidth.value / (barDuration.value * props.pxPerSec))));
const barTicks = computed(() =>
  Array.from({ length: barCount.value }, (_, index) => ({
    bar: index + 1,
    time: index * barDuration.value,
  })),
);

const gridStyle = computed(() => ({
  "--major": `${barDuration.value * props.pxPerSec}px`,
  "--minor": `${(barDuration.value * props.pxPerSec) / 4}px`,
}));

function snapped(seconds: number): number {
  return props.snap ? Math.round(seconds / props.gridSec) * props.gridSec : seconds;
}

/** Timeline time under a mouse position. The lanes element scrolls with the timeline, so its rect already includes the scroll offset. */
function timeAt(clientX: number): number {
  const rect = (lanesEl.value ?? rulerEl.value)?.getBoundingClientRect();
  if (!rect) return 0;
  return Math.max(0, snapped((clientX - rect.left) / props.pxPerSec));
}

function emitContext(event: MouseEvent, request: ArrangementContext) {
  emit("context", { ...request, x: event.clientX, y: event.clientY });
}

/* ---------- ruler and playhead ---------- */

let seeking = false;

function seekFromRuler(event: PointerEvent) {
  if (event.button !== 0) return;
  seeking = true;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  doc.playhead = timeAt(event.clientX);
}

function seekMove(event: PointerEvent) {
  if (seeking) doc.playhead = timeAt(event.clientX);
}

function seekEnd() {
  seeking = false;
}

/** Keeps the playhead visible while it moves during playback. */
watch(
  () => doc.playhead,
  (time) => {
    const el = timelineEl.value;
    if (!el || seeking || drag) return;
    const x = time * props.pxPerSec;
    if (x > el.scrollLeft + el.clientWidth - 24 || x < el.scrollLeft) {
      el.scrollLeft = Math.max(0, x - 80);
    }
  },
);

/* ---------- scrolling and zoom ---------- */

function syncScroll() {
  if (headEl.value && timelineEl.value) headEl.value.scrollTop = timelineEl.value.scrollTop;
}

function forwardWheel(event: WheelEvent) {
  if (timelineEl.value) timelineEl.value.scrollTop += event.deltaY;
}

let zoomAnchor: { time: number; offset: number } | null = null;

/** Ctrl + wheel zooms around the mouse position. */
function onTimelineWheel(event: WheelEvent) {
  if (!event.ctrlKey || !timelineEl.value) return;
  event.preventDefault();
  const rect = timelineEl.value.getBoundingClientRect();
  const offset = event.clientX - rect.left;
  zoomAnchor = { time: (timelineEl.value.scrollLeft + offset) / props.pxPerSec, offset };
  emit("zoom", props.pxPerSec * (event.deltaY < 0 ? 1.15 : 1 / 1.15));
}

watch(
  () => props.pxPerSec,
  (next) => {
    if (zoomAnchor && timelineEl.value) {
      timelineEl.value.scrollLeft = Math.max(0, zoomAnchor.time * next - zoomAnchor.offset);
    }
    zoomAnchor = null;
  },
  { flush: "post" },
);

/* ---------- track headers ---------- */

function selectTrack(track: Track) {
  doc.selectedTrack = track.id;
  doc.selectedClips = [];
}

function setLevel(track: Track, key: "volume" | "pan", value: number) {
  track[key] = value;
}

function resetLevel(track: Track, key: "volume" | "pan") {
  mutate(() => {
    track[key] = key === "volume" ? 1 : 0;
  });
}

function panText(pan: number): string {
  if (Math.abs(pan) < 0.005) return "C";
  return `${pan < 0 ? "L" : "R"}${Math.round(Math.abs(pan) * 100)}`;
}

/* ---------- reordering tracks by dragging the grip ---------- */

const reorder = reactive({ id: "", dropSlot: -1 });

function onGripDown(event: PointerEvent, track: Track) {
  if (event.button !== 0) return;
  event.preventDefault();
  selectTrack(track);
  reorder.id = track.id;
  reorder.dropSlot = tracks.value.findIndex((item) => item.id === track.id);
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function onGripMove(event: PointerEvent) {
  if (!reorder.id || !rowsEl.value) return;
  const y = event.clientY - rowsEl.value.getBoundingClientRect().top;
  let slot = tracks.value.length;
  for (let index = 0; index < tracks.value.length; index++) {
    if (y < tops.value[index] + heights.value[index] / 2) {
      slot = index;
      break;
    }
  }
  reorder.dropSlot = slot;
}

function onGripUp() {
  if (!reorder.id) return;
  const from = tracks.value.findIndex((item) => item.id === reorder.id);
  const to = reorder.dropSlot > from ? reorder.dropSlot - 1 : reorder.dropSlot;
  const id = reorder.id;
  reorder.id = "";
  reorder.dropSlot = -1;
  if (from >= 0 && to !== from) moveTrackTo(id, to);
}

function slotTop(slot: number): number {
  return slot >= tracks.value.length ? totalHeight.value : tops.value[slot];
}

/* ---------- clips ---------- */

function selectClip(track: Track, clip: Clip, additive: boolean) {
  doc.selectedTrack = track.id;
  if (additive) {
    doc.selectedClips = doc.selectedClips.includes(clip.id)
      ? doc.selectedClips.filter((id) => id !== clip.id)
      : [...doc.selectedClips, clip.id];
  } else {
    doc.selectedClips = [clip.id];
  }
}

function eraseClip(clip: Clip) {
  mutate(() => {
    for (const track of doc.project.tracks) {
      track.clips = track.clips.filter((item) => item.id !== clip.id);
    }
  });
}

function findClip(id: string): Clip | undefined {
  for (const track of doc.project.tracks) {
    const clip = track.clips.find((item) => item.id === id);
    if (clip) return clip;
  }
  return undefined;
}

function trackIndexOfClip(id: string): number {
  return doc.project.tracks.findIndex((track) => track.clips.some((clip) => clip.id === id));
}

interface DragState {
  leadId: string;
  ids: string[];
  startX: number;
  startY: number;
  origins: Map<string, number>;
  leadIndex: number;
  moved: boolean;
  collapseTo: string | null;
}

let drag: DragState | null = null;
const dragUi = reactive({ active: false, ids: [] as string[], dy: 0, targetIndex: -1 });

function trackIndexAt(clientY: number): number {
  const rect = lanesEl.value?.getBoundingClientRect();
  if (!rect || tracks.value.length === 0) return -1;
  const y = clientY - rect.top;
  for (let index = 0; index < tracks.value.length; index++) {
    if (y < tops.value[index] + heights.value[index]) return index;
  }
  return tracks.value.length - 1;
}

function onClipDown(event: PointerEvent, track: Track, clip: Clip) {
  if (event.button !== 0) return;
  if (props.tool === "erase") {
    eraseClip(clip);
    return;
  }
  const additive = event.ctrlKey || event.metaKey;
  const alreadySelected = doc.selectedClips.includes(clip.id);
  let collapseTo: string | null = null;
  if (!additive && alreadySelected && doc.selectedClips.length > 1) {
    // Keep the group so it can be dragged together. A plain click without a drag narrows it to this clip.
    doc.selectedTrack = track.id;
    collapseTo = clip.id;
  } else {
    selectClip(track, clip, additive);
  }
  const ids = doc.selectedClips.includes(clip.id) ? [...doc.selectedClips] : [clip.id];
  const origins = new Map<string, number>();
  for (const id of ids) {
    const item = findClip(id);
    if (item) origins.set(id, item.start);
  }
  drag = {
    leadId: clip.id,
    ids,
    startX: event.clientX,
    startY: event.clientY,
    origins,
    leadIndex: trackIndexOfClip(clip.id),
    moved: false,
    collapseTo,
  };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

/** The track-index offset for the drag, or 0 when any dragged clip would land on a track that cannot hold clips. */
function validDelta(state: DragState, targetIndex: number): number {
  const delta = targetIndex - state.leadIndex;
  if (delta === 0) return 0;
  for (const id of state.ids) {
    const target = doc.project.tracks[trackIndexOfClip(id) + delta];
    if (!target || !acceptsClips(target.type)) return 0;
  }
  return delta;
}

function onClipMove(event: PointerEvent) {
  if (!drag) return;
  const dx = event.clientX - drag.startX;
  if (!drag.moved && Math.abs(dx) < 3 && Math.abs(event.clientY - drag.startY) < 3) return;
  if (!drag.moved) {
    beginChange();
    drag.moved = true;
    dragUi.active = true;
    dragUi.ids = drag.ids;
  }
  const lead = findClip(drag.leadId);
  const leadOrigin = drag.origins.get(drag.leadId);
  if (lead && leadOrigin !== undefined) {
    const lowest = Math.min(...drag.origins.values());
    const offset = Math.max(-lowest, snapped(leadOrigin + dx / props.pxPerSec) - leadOrigin);
    for (const [id, origin] of drag.origins) {
      const clip = findClip(id);
      if (clip) clip.start = Math.max(0, origin + offset);
    }
  }
  const delta = validDelta(drag, trackIndexAt(event.clientY));
  dragUi.targetIndex = drag.leadIndex + delta;
  dragUi.dy = tops.value[dragUi.targetIndex] - tops.value[drag.leadIndex];
}

function onClipUp() {
  if (!drag) return;
  const state = drag;
  drag = null;
  const delta = dragUi.active ? dragUi.targetIndex - state.leadIndex : 0;
  dragUi.active = false;
  dragUi.ids = [];
  dragUi.dy = 0;
  dragUi.targetIndex = -1;
  if (!state.moved) {
    if (state.collapseTo) doc.selectedClips = [state.collapseTo];
    return;
  }
  if (delta !== 0) {
    const moves = state.ids.map((id) => ({ id, from: trackIndexOfClip(id) }));
    for (const move of moves) {
      const source = doc.project.tracks[move.from];
      const target = doc.project.tracks[move.from + delta];
      const clip = source?.clips.find((item) => item.id === move.id);
      if (!source || !target || !clip) continue;
      source.clips = source.clips.filter((item) => item.id !== move.id);
      target.clips.push(clip);
    }
    const leadTrack = doc.project.tracks[state.leadIndex + delta];
    if (leadTrack) doc.selectedTrack = leadTrack.id;
  }
}

function laneClick(track: Track, event: MouseEvent) {
  if (event.target === event.currentTarget) selectTrack(track);
}

function onClipContext(event: MouseEvent, track: Track, clip: Clip) {
  if (!doc.selectedClips.includes(clip.id)) selectClip(track, clip, false);
  else doc.selectedTrack = track.id;
  emitContext(event, { kind: "clip", trackId: track.id, clipId: clip.id });
}

function onLaneContext(event: MouseEvent, track: Track) {
  doc.selectedTrack = track.id;
  emitContext(event, { kind: "lane", trackId: track.id, time: timeAt(event.clientX) });
}

function onHeadContext(event: MouseEvent, track: Track) {
  selectTrack(track);
  emitContext(event, { kind: "track", trackId: track.id });
}

function onClipDblClick(clip: Clip) {
  doc.activeClipId = clip.id;
  doc.selectedClips = [clip.id];
}

function clipStyle(track: Track, clip: Clip) {
  const moving = dragUi.active && dragUi.ids.includes(clip.id);
  return {
    left: `${clip.start * props.pxPerSec}px`,
    width: `${Math.max(32, clip.duration * props.pxPerSec)}px`,
    backgroundColor: `${track.color}2e`,
    border: `1px solid ${track.color}`,
    color: "#ffffff",
    transform: moving ? `translateY(${dragUi.dy}px)` : undefined,
    zIndex: moving ? 3 : undefined,
  };
}
</script>

<template>
  <div class="arrangement">
    <div
      v-if="trackPanel"
      ref="headEl"
      class="heads"
      :style="{ width: `${trackPanelWidth}px` }"
      @wheel.prevent="forwardWheel"
      @contextmenu.prevent="emitContext($event, { kind: 'empty', time: doc.playhead })"
    >
      <div class="spacer" :style="{ height: `${RULER_HEIGHT}px` }">
        <span class="spacer-title">TRACKS</span>
        <button class="add-voice-btn" :title="t('addTrack')" @click.stop="emit('addTrackMenu', $event.clientX, $event.clientY)">+ Voice</button>
      </div>

      <div ref="rowsEl" class="rows">
        <div
          v-for="(track, index) in tracks"
          :key="track.id"
          class="head"
          :class="{
            selected: doc.selectedTrack === track.id,
            compact: track.height === 'compact',
            large: track.height === 'large',
            dragging: reorder.id === track.id,
            muted: track.muted,
          }"
          :style="{ height: `${heights[index]}px`, '--tc': track.color }"
          @click="selectTrack(track)"
          @dblclick="emit('trackDblclick', track.id)"
          @contextmenu.prevent.stop="onHeadContext($event, track)"
        >
          <div class="line-top">
            <span
              class="grip"
              :title="t('dragToReorder')"
              @pointerdown.stop="onGripDown($event, track)"
              @pointermove="onGripMove"
              @pointerup="onGripUp"
              @pointercancel="onGripUp"
              @click.stop
              @dblclick.stop
            >⋮⋮</span>
            <span class="num" :style="{ color: track.color }">{{ index + 1 }}</span>
            <span class="name" :title="track.name">{{ track.name }}</span>
            <span class="voice-chip">{{ track.voiceId || 'Aria' }}</span>
            <button
              class="kbtn"
              :class="{ on: doc.selectedTrack === track.id }"
              :title="t('trackProperties')"
              @click.stop="selectTrack(track)"
              @dblclick.stop
            >E</button>
            <button class="kbtn" disabled :title="t('record')">R</button>
            <button
              class="kbtn solo"
              :class="{ on: track.solo }"
              :title="t('solo')"
              @click.stop="toggleTrackFlag(track.id, 'solo')"
              @dblclick.stop
            >S</button>
            <button
              class="kbtn mute"
              :class="{ on: track.muted }"
              :title="t('mute')"
              @click.stop="toggleTrackFlag(track.id, 'muted')"
              @dblclick.stop
            >M</button>
          </div>
          <template v-if="track.height !== 'compact'">
            <label class="level vol" @dblclick.stop>
              <span>VOL</span>
              <input
                type="range"
                min="0"
                max="2"
                step="0.01"
                :value="track.volume"
                :title="t('doubleClickReset')"
                @pointerdown.stop="beginChange()"
                @input="setLevel(track, 'volume', Number(($event.target as HTMLInputElement).value))"
                @dblclick.stop="resetLevel(track, 'volume')"
              />
              <em>{{ Math.round(track.volume * 100) }}%</em>
            </label>
            <label class="level pan" @dblclick.stop>
              <span>PAN</span>
              <input
                type="range"
                min="-1"
                max="1"
                step="0.01"
                :value="track.pan"
                :title="t('doubleClickReset')"
                @pointerdown.stop="beginChange()"
                @input="setLevel(track, 'pan', Number(($event.target as HTMLInputElement).value))"
                @dblclick.stop="resetLevel(track, 'pan')"
              />
              <em>{{ panText(track.pan) }}</em>
            </label>
          </template>
        </div>
        <div v-if="reorder.id && reorder.dropSlot >= 0" class="drop-line" :style="{ top: `${slotTop(reorder.dropSlot)}px` }"></div>
      </div>
      <div class="tail" :style="{ height: `${TAIL_HEIGHT}px` }"></div>
    </div>

    <Splitter
      v-if="trackPanel"
      direction="x"
      @move="(delta: number) => emit('headResize', delta)"
      @end="emit('headResizeEnd')"
    />

    <div ref="timelineEl" class="timeline" :style="gridStyle" @scroll="syncScroll" @wheel="onTimelineWheel">
      <div
        ref="rulerEl"
        class="ruler"
        :style="{ width: `${contentWidth}px`, height: `${RULER_HEIGHT}px` }"
        @pointerdown="seekFromRuler"
        @pointermove="seekMove"
        @pointerup="seekEnd"
        @pointercancel="seekEnd"
        @contextmenu.prevent.stop="emitContext($event, { kind: 'ruler', time: timeAt($event.clientX) })"
      >
        <span v-for="tick in barTicks" :key="tick.bar" class="tick" :style="{ left: `${tick.time * pxPerSec}px` }">
          Bar {{ tick.bar }}
        </span>
        <div class="playhead-flag" :style="{ left: `${doc.playhead * pxPerSec}px` }"></div>
      </div>

      <div
        ref="lanesEl"
        class="lanes"
        :style="{ width: `${contentWidth}px` }"
        @contextmenu.prevent="emitContext($event, { kind: 'empty', time: timeAt($event.clientX) })"
      >
        <div
          v-for="(track, index) in tracks"
          :key="track.id"
          class="lane"
          :class="{
            muted: track.muted,
            selected: doc.selectedTrack === track.id,
            'drop-target': dragUi.active && dragUi.targetIndex === index,
            inert: !acceptsClips(track.type),
          }"
          :style="{ height: `${heights[index]}px`, '--tc': track.color }"
          @click="laneClick(track, $event)"
          @contextmenu.prevent.stop="onLaneContext($event, track)"
        >
          <div
            v-for="clip in track.clips"
            :key="clip.id"
            class="clip"
            :class="{
              selected: doc.selectedClips.includes(clip.id),
              erase: tool === 'erase',
              moving: dragUi.active && dragUi.ids.includes(clip.id),
            }"
            :style="clipStyle(track, clip)"
            :title="clip.text || clip.name"
            @pointerdown.stop="onClipDown($event, track, clip)"
            @pointermove="onClipMove"
            @pointerup="onClipUp"
            @pointercancel="onClipUp"
            @dblclick.stop="onClipDblClick(clip)"
            @contextmenu.prevent.stop="onClipContext($event, track, clip)"
          >
            <div class="clip-inner">
              <span class="clip-label" :style="{ color: track.color }">{{ clip.name || clip.text }}</span>
              <div v-if="clip.notes && clip.notes.length > 0" class="mini-notes-view">
                <div
                  v-for="mn in clip.notes"
                  :key="mn.id"
                  class="mini-note-bar"
                  :style="{
                    left: `${(mn.start / Math.max(1, clip.duration)) * 100}%`,
                    width: `${Math.max(4, (mn.duration / Math.max(1, clip.duration)) * 100)}%`,
                    top: `${Math.max(15, Math.min(75, 100 - ((mn.midi - 48) / 36) * 100))}%`,
                    backgroundColor: track.color
                  }"
                ></div>
              </div>
            </div>
          </div>
        </div>
        <div v-if="reorder.id && reorder.dropSlot >= 0" class="drop-line" :style="{ top: `${slotTop(reorder.dropSlot)}px` }"></div>
        <div class="tail" :style="{ height: `${TAIL_HEIGHT}px` }"></div>
        <div class="playhead" :style="{ left: `${doc.playhead * pxPerSec}px` }"></div>
      </div>

      <div v-if="tracks.length === 0" class="empty">{{ t("emptyTimelineHint") }}</div>
    </div>
  </div>
</template>

<style scoped>
.arrangement {
  display: flex;
  height: 100%;
  min-height: 0;
  background: var(--background);
  color: var(--text);
  font-size: 12px;
}

.heads {
  flex: none;
  position: relative;
  overflow: hidden;
  background: var(--panel);
  min-width: 0;
}

.spacer {
  position: sticky;
  top: 0;
  z-index: 5;
  box-sizing: border-box;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px 0 12px;
  background: var(--panel);
  border-bottom: 1px solid color-mix(in srgb, var(--text) 16%, transparent);
}

.spacer-title {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.8px;
  text-transform: uppercase;
  color: color-mix(in srgb, var(--text) 60%, transparent);
}

.add-voice-btn {
  height: 24px;
  padding: 0 10px;
  border: none;
  border-radius: 6px;
  background: var(--button);
  color: var(--text);
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.15s ease;
}

.add-voice-btn:hover {
  background: var(--accent);
  color: #1a1405;
}

.voice-chip {
  padding: 2px 6px;
  border-radius: 4px;
  background: var(--button);
  color: color-mix(in srgb, var(--text) 60%, transparent);
  font-size: 10px;
  font-family: "JetBrains Mono", monospace;
  font-weight: 600;
}

.level.vol input::-webkit-slider-runnable-track {
  background: #2b3145;
}

.level.vol input::-webkit-slider-thumb {
  background: var(--accent);
}

.level.pan input::-webkit-slider-thumb {
  background: var(--accent);
}

.clip-inner {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  position: relative;
  justify-content: space-between;
  padding: 2px 0;
  box-sizing: border-box;
}

.clip-label {
  font-size: 11px;
  font-weight: 700;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.8);
}

.mini-notes-view {
  position: relative;
  width: 100%;
  height: 16px;
  background: rgba(0, 0, 0, 0.2);
  border-radius: 2px;
  overflow: hidden;
}

.mini-note-bar {
  position: absolute;
  height: 3px;
  border-radius: 1px;
  box-shadow: 0 0 2px rgba(255, 255, 255, 0.3);
}

.rows {
  position: relative;
}

.head {
  position: relative;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 3px;
  padding: 3px 8px 3px 12px;
  background: color-mix(in srgb, var(--panel) 90%, var(--text));
  border-bottom: 1px solid color-mix(in srgb, var(--text) 10%, transparent);
  box-shadow: inset 4px 0 0 var(--tc);
  cursor: pointer;
  min-width: 0;
  overflow: hidden;
  transition: background 0.12s;
}

.head:hover {
  background: color-mix(in srgb, var(--panel) 84%, var(--text));
}

.head.selected {
  background: color-mix(in srgb, var(--tc) 16%, var(--panel));
}

.head.dragging {
  opacity: 0.45;
}

.head.muted .name {
  opacity: 0.5;
  text-decoration: line-through;
}

.line-top {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
  height: 20px;
}

.grip {
  flex: none;
  width: 10px;
  font-size: 11px;
  line-height: 1;
  letter-spacing: -2px;
  text-align: center;
  opacity: 0.35;
  cursor: grab;
  touch-action: none;
}

.grip:hover {
  opacity: 0.9;
}

.num {
  flex: none;
  width: 14px;
  opacity: 0.55;
  font-size: 10px;
  text-align: right;
}

.chip {
  flex: none;
  min-width: 15px;
  height: 15px;
  padding: 0 3px;
  border-radius: 4px;
  box-sizing: border-box;
  background: var(--tc);
  color: #000;
  font-size: 9px;
  font-weight: 800;
  line-height: 15px;
  text-align: center;
}

.name {
  flex: 1;
  min-width: 0;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.subline {
  padding-left: 52px;
  margin-top: -2px;
  font-size: 10px;
  opacity: 0.55;
}

.kbtn {
  flex: none;
  width: 19px;
  height: 19px;
  padding: 0;
  border: 1px solid color-mix(in srgb, var(--text) 24%, transparent);
  border-radius: 4px;
  background: transparent;
  color: var(--text);
  font-size: 10px;
  font-weight: 700;
  cursor: pointer;
  transition: background 0.1s, border-color 0.1s;
}

.kbtn:hover:not(:disabled) {
  border-color: var(--accent);
}

.kbtn.on {
  border-color: transparent;
  background: var(--accent);
  color: var(--panel);
}

.kbtn.mute.on {
  background: #d9822b;
  color: #000;
}

.kbtn:disabled {
  opacity: 0.3;
  cursor: default;
}

.level {
  display: grid;
  grid-template-columns: 44px minmax(0, 1fr) 34px;
  align-items: center;
  gap: 5px;
  min-width: 0;
  font-size: 10px;
}

.level span {
  opacity: 0.65;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.level em {
  font-style: normal;
  text-align: right;
  opacity: 0.8;
  font-variant-numeric: tabular-nums;
}

.level input {
  width: 100%;
  min-width: 0;
  height: 12px;
  margin: 0;
}

.drop-line {
  position: absolute;
  left: 0;
  right: 0;
  height: 2px;
  margin-top: -1px;
  background: var(--accent);
  box-shadow: 0 0 6px var(--accent);
  pointer-events: none;
  z-index: 4;
}

.timeline {
  position: relative;
  flex: 1;
  min-width: 0;
  overflow: auto;
}

.ruler {
  position: sticky;
  top: 0;
  z-index: 6;
  box-sizing: border-box;
  background-color: var(--panel);
  background-image: repeating-linear-gradient(
    to right,
    color-mix(in srgb, var(--text) 28%, transparent) 0 1px,
    transparent 1px var(--minor)
  );
  background-size: 100% 6px;
  background-repeat: no-repeat;
  background-position: left bottom;
  border-bottom: 1px solid color-mix(in srgb, var(--text) 16%, transparent);
  cursor: col-resize;
  user-select: none;
}

.tick {
  position: absolute;
  top: 3px;
  height: 14px;
  padding-left: 4px;
  border-left: 1px solid color-mix(in srgb, var(--text) 40%, transparent);
  font-size: 10px;
  line-height: 14px;
  opacity: 0.75;
  white-space: nowrap;
  pointer-events: none;
}

.playhead-flag {
  position: absolute;
  bottom: 0;
  width: 0;
  height: 0;
  margin-left: -5px;
  border-left: 5px solid transparent;
  border-right: 5px solid transparent;
  border-top: 8px solid var(--accent);
  pointer-events: none;
}

.lanes {
  position: relative;
  background-image: repeating-linear-gradient(
    to right,
    color-mix(in srgb, var(--text) 9%, transparent) 0 1px,
    transparent 1px var(--major)
  );
}

.lane {
  position: relative;
  box-sizing: border-box;
  border-bottom: 1px solid color-mix(in srgb, var(--text) 10%, transparent);
  background: color-mix(in srgb, var(--tc) 5%, transparent);
  transition: background 0.12s;
}

.lane.selected {
  background: color-mix(in srgb, var(--tc) 11%, transparent);
}

.lane.drop-target {
  background: color-mix(in srgb, var(--tc) 22%, transparent);
  box-shadow: inset 0 0 0 1px var(--tc);
}

.lane.inert {
  background-image: repeating-linear-gradient(
    135deg,
    color-mix(in srgb, var(--text) 5%, transparent) 0 6px,
    transparent 6px 12px
  );
}

.lane.muted .clip {
  opacity: 0.4;
}

.clip {
  position: absolute;
  top: 5px;
  bottom: 5px;
  box-sizing: border-box;
  display: flex;
  align-items: center;
  padding: 0 8px;
  border-radius: 5px;
  background-image: linear-gradient(to bottom, rgba(255, 255, 255, 0.2), rgba(0, 0, 0, 0.14));
  box-shadow:
    0 1px 3px rgba(0, 0, 0, 0.45),
    inset 0 0 0 1px rgba(255, 255, 255, 0.18);
  font-weight: 500;
  overflow: hidden;
  white-space: nowrap;
  cursor: grab;
  touch-action: none;
  user-select: none;
}

.clip span {
  overflow: hidden;
  text-overflow: ellipsis;
}

.clip.selected {
  outline: 2px solid var(--accent);
  outline-offset: -1px;
}

.clip.moving {
  cursor: grabbing;
  box-shadow: 0 6px 16px rgba(0, 0, 0, 0.55);
}

.clip.erase {
  cursor: not-allowed;
}

.tail {
  pointer-events: none;
}

.playhead {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 1px;
  background: var(--accent);
  box-shadow: 0 0 4px var(--accent);
  pointer-events: none;
  z-index: 2;
}

.empty {
  position: sticky;
  left: 16px;
  top: 44px;
  width: max-content;
  max-width: calc(100% - 32px);
  padding: 0 16px;
  opacity: 0.6;
}
</style>
