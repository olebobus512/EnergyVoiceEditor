<script setup lang="ts">
import { ref } from "vue";

const props = defineProps<{ direction: "x" | "y" }>();
const emit = defineEmits<{ move: [delta: number]; end: [] }>();

const dragging = ref(false);
let last = 0;

function down(event: PointerEvent) {
  dragging.value = true;
  last = props.direction === "x" ? event.clientX : event.clientY;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function move(event: PointerEvent) {
  if (!dragging.value) return;
  const current = props.direction === "x" ? event.clientX : event.clientY;
  const delta = current - last;
  last = current;
  if (delta !== 0) emit("move", delta);
}

function up() {
  if (!dragging.value) return;
  dragging.value = false;
  emit("end");
}
</script>

<template>
  <div
    class="splitter"
    :class="[direction === 'x' ? 'vertical' : 'horizontal', { active: dragging }]"
    role="separator"
    @pointerdown.prevent="down"
    @pointermove="move"
    @pointerup="up"
    @pointercancel="up"
  ></div>
</template>

<style scoped>
.splitter {
  flex: none;
  background: color-mix(in srgb, var(--text) 10%, transparent);
  touch-action: none;
  transition: background 0.12s;
}

.splitter.vertical {
  width: 5px;
  cursor: col-resize;
}

.splitter.horizontal {
  height: 5px;
  cursor: row-resize;
}

.splitter:hover,
.splitter.active {
  background: var(--accent);
}
</style>
