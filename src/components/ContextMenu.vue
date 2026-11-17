<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { contextMenu, hideContextMenu } from "../contextmenu";
import { t } from "../i18n";
import type { MenuItem } from "./MenuBar.vue";

const root = ref<HTMLElement | null>(null);
const colorInput = ref<HTMLInputElement | null>(null);
const pos = ref({ x: 0, y: 0 });
const flipSub = ref(false);
const focus = ref(-1);
const subOpen = ref(-1);
const subFocus = ref(-1);
let customPick: ((color: string) => void) | null = null;

const MARGIN = 6;
const SUB_WIDTH = 230;

function selectable(item: MenuItem): boolean {
  return !item.separator && !item.disabled && !item.palette;
}

function nextIndex(items: MenuItem[], from: number, step: 1 | -1): number {
  const count = items.length;
  for (let i = 1; i <= count; i++) {
    const index = (from + step * i + count * 2) % count;
    if (selectable(items[index]) || items[index].palette) return index;
  }
  return -1;
}

async function place() {
  pos.value = { x: contextMenu.x, y: contextMenu.y };
  focus.value = -1;
  subOpen.value = -1;
  subFocus.value = -1;
  await nextTick();
  const el = root.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  const x = Math.max(MARGIN, Math.min(contextMenu.x, window.innerWidth - rect.width - MARGIN));
  const y = Math.max(MARGIN, Math.min(contextMenu.y, window.innerHeight - rect.height - MARGIN));
  pos.value = { x, y };
  flipSub.value = x + rect.width + SUB_WIDTH > window.innerWidth;
}

watch(() => contextMenu.token, place);

function run(item: MenuItem) {
  if (!selectable(item)) return;
  hideContextMenu();
  item.action?.();
}

function hover(index: number) {
  const item = contextMenu.items[index];
  focus.value = index;
  subFocus.value = -1;
  subOpen.value = item.submenu && !item.disabled ? index : -1;
}

function clickItem(index: number) {
  const item = contextMenu.items[index];
  if (item.submenu) {
    if (!item.disabled) subOpen.value = index;
    return;
  }
  run(item);
}

function pickColor(item: MenuItem, color: string) {
  hideContextMenu();
  item.palette?.pick(color);
}

function openCustom(item: MenuItem) {
  customPick = item.palette?.pick ?? null;
  const input = colorInput.value;
  if (!input) return;
  input.value = item.palette?.current ?? "#ffffff";
  input.click();
}

function onCustomInput(event: Event) {
  customPick?.((event.target as HTMLInputElement).value);
}

function onCustomChange() {
  hideContextMenu();
}

function onKeydown(event: KeyboardEvent) {
  if (!contextMenu.open) return;
  const items = contextMenu.items;
  const subItems = subOpen.value >= 0 ? (items[subOpen.value].submenu ?? []) : [];
  const inSub = subOpen.value >= 0 && subFocus.value >= 0;
  switch (event.key) {
    case "Escape":
      if (subOpen.value >= 0) {
        subOpen.value = -1;
        subFocus.value = -1;
      } else {
        hideContextMenu();
      }
      break;
    case "ArrowDown":
      if (subOpen.value >= 0) subFocus.value = nextIndex(subItems, subFocus.value, 1);
      else focus.value = nextIndex(items, focus.value, 1);
      break;
    case "ArrowUp":
      if (subOpen.value >= 0) subFocus.value = nextIndex(subItems, subFocus.value, -1);
      else focus.value = nextIndex(items, focus.value, -1);
      break;
    case "ArrowRight":
      if (focus.value >= 0 && items[focus.value].submenu && !items[focus.value].disabled) {
        subOpen.value = focus.value;
        subFocus.value = nextIndex(items[focus.value].submenu ?? [], -1, 1);
      }
      break;
    case "ArrowLeft":
      subOpen.value = -1;
      subFocus.value = -1;
      break;
    case "Enter":
      if (inSub) run(subItems[subFocus.value]);
      else if (focus.value >= 0) clickItem(focus.value);
      break;
    default:
      return;
  }
  event.preventDefault();
  event.stopPropagation();
}

function onPointerDown(event: PointerEvent) {
  if (!contextMenu.open) return;
  if (root.value && root.value.contains(event.target as Node)) return;
  hideContextMenu();
}

const close = () => hideContextMenu();

onMounted(() => {
  window.addEventListener("keydown", onKeydown, true);
  window.addEventListener("pointerdown", onPointerDown, true);
  window.addEventListener("wheel", close, { passive: true });
  window.addEventListener("resize", close);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown, true);
  window.removeEventListener("pointerdown", onPointerDown, true);
  window.removeEventListener("wheel", close);
  window.removeEventListener("resize", close);
});
</script>

<template>
  <div
    v-if="contextMenu.open"
    ref="root"
    class="ctx"
    role="menu"
    :style="{ left: `${pos.x}px`, top: `${pos.y}px` }"
    @contextmenu.prevent
  >
    <template v-for="(item, index) in contextMenu.items" :key="index">
      <div v-if="item.separator" class="sep"></div>
      <div
        v-else
        class="item"
        :class="{ disabled: item.disabled, focused: focus === index, danger: item.danger }"
        role="menuitem"
        :aria-disabled="item.disabled"
        @mouseenter="hover(index)"
        @click.stop="clickItem(index)"
      >
        <span class="check">{{ item.checked ? "✓" : "" }}</span>
        <span class="label">{{ item.label }}</span>
        <span class="shortcut">{{ item.submenu ? "▸" : (item.shortcut ?? "") }}</span>

        <div
          v-if="item.submenu && subOpen === index"
          class="ctx sub"
          :class="{ flip: flipSub }"
          role="menu"
          @click.stop
        >
          <template v-for="(sub, subIndex) in item.submenu" :key="subIndex">
            <div v-if="sub.separator" class="sep"></div>
            <div v-else-if="sub.palette" class="palette">
              <button
                v-for="color in sub.palette.colors"
                :key="color"
                class="swatch"
                :class="{ current: sub.palette.current?.toLowerCase() === color.toLowerCase() }"
                :style="{ background: color }"
                :title="color"
                @click="pickColor(sub, color)"
              ></button>
              <button class="custom" @click="openCustom(sub)">{{ t("customColor") }}</button>
            </div>
            <div
              v-else
              class="item"
              :class="{ disabled: sub.disabled, focused: subFocus === subIndex, danger: sub.danger }"
              role="menuitem"
              :aria-disabled="sub.disabled"
              @mouseenter="subFocus = subIndex"
              @click.stop="run(sub)"
            >
              <span class="check">{{ sub.checked ? "✓" : "" }}</span>
              <span class="label">{{ sub.label }}</span>
              <span class="shortcut">{{ sub.shortcut ?? "" }}</span>
            </div>
          </template>
        </div>
      </div>
    </template>
    <input ref="colorInput" class="color-input" type="color" tabindex="-1" @input="onCustomInput" @change="onCustomChange" />
  </div>
</template>

<style scoped>
.ctx {
  position: fixed;
  z-index: 80;
  min-width: 220px;
  padding: 5px;
  background: color-mix(in srgb, var(--panel) 94%, var(--text));
  color: var(--text);
  border: 1px solid color-mix(in srgb, var(--text) 22%, transparent);
  border-radius: 8px;
  box-shadow:
    0 10px 28px rgba(0, 0, 0, 0.5),
    0 1px 0 color-mix(in srgb, var(--text) 8%, transparent) inset;
  font-size: 12.5px;
  user-select: none;
  animation: ctx-in 0.1s ease-out;
}

.ctx.sub {
  position: absolute;
  top: -6px;
  left: calc(100% + 2px);
  min-width: 190px;
}

.ctx.sub.flip {
  left: auto;
  right: calc(100% + 2px);
}

.item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 5px;
  white-space: nowrap;
  cursor: pointer;
}

.item .check {
  width: 12px;
  color: var(--accent);
  font-size: 11px;
}

.item .label {
  flex: 1;
}

.item .shortcut {
  margin-left: 22px;
  opacity: 0.55;
  font-size: 11.5px;
}

.item.focused:not(.disabled),
.item:hover:not(.disabled) {
  background: color-mix(in srgb, var(--accent) 24%, transparent);
}

.item.disabled {
  opacity: 0.38;
  cursor: default;
}

.item.danger {
  color: #ff6b6b;
}

.sep {
  height: 1px;
  margin: 5px 6px;
  background: color-mix(in srgb, var(--text) 16%, transparent);
}

.palette {
  display: grid;
  grid-template-columns: repeat(6, 24px);
  gap: 6px;
  padding: 8px;
}

.swatch {
  width: 24px;
  height: 24px;
  padding: 0;
  border: 2px solid transparent;
  border-radius: 6px;
  cursor: pointer;
  box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.35) inset;
  transition: transform 0.08s;
}

.swatch:hover {
  transform: scale(1.12);
}

.swatch.current {
  border-color: var(--text);
}

.custom {
  grid-column: 1 / -1;
  height: 26px;
  border: 1px solid color-mix(in srgb, var(--text) 25%, transparent);
  border-radius: 5px;
  background: transparent;
  color: var(--text);
  font: inherit;
  cursor: pointer;
}

.custom:hover {
  background: color-mix(in srgb, var(--accent) 20%, transparent);
}

.color-input {
  position: absolute;
  width: 0;
  height: 0;
  padding: 0;
  border: none;
  opacity: 0;
  pointer-events: none;
}

@keyframes ctx-in {
  from {
    opacity: 0;
    transform: scale(0.98);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}
</style>
