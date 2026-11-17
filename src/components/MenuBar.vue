<script lang="ts">
export interface MenuItem {
  label?: string;
  shortcut?: string;
  disabled?: boolean;
  separator?: boolean;
  checked?: boolean;
  /** Red text for destructive actions (context menus). */
  danger?: boolean;
  action?: () => void;
  submenu?: MenuItem[];
  /** A grid of colour swatches instead of a label (context menus). `pick` receives #RRGGBB. */
  palette?: { colors: string[]; current?: string; pick: (color: string) => void };
}

export interface MenuDef {
  label: string;
  items: MenuItem[];
}
</script>

<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";

const props = withDefaults(
  defineProps<{
    menus: MenuDef[];
    statusText?: string;
  }>(),
  {
    statusText: "Ready",
  },
);

const root = ref<HTMLElement | null>(null);
const openIndex = ref(-1);
const focusIndex = ref(-1);
/** Index of the item whose submenu is open, or -1. */
const subIndex = ref(-1);
const subFocus = ref(-1);

function isSelectable(item: MenuItem): boolean {
  return !item.separator && !item.disabled;
}

function close() {
  openIndex.value = -1;
  focusIndex.value = -1;
  subIndex.value = -1;
  subFocus.value = -1;
}

function toggleMenu(index: number) {
  if (openIndex.value === index) return close();
  openIndex.value = index;
  focusIndex.value = -1;
  subIndex.value = -1;
  subFocus.value = -1;
}

function hoverMenu(index: number) {
  if (openIndex.value >= 0 && openIndex.value !== index) toggleMenu(index);
}

function hoverItem(index: number) {
  const item = props.menus[openIndex.value].items[index];
  focusIndex.value = index;
  subFocus.value = -1;
  subIndex.value = item.submenu ? index : -1;
}

function activate(index: number) {
  const item = props.menus[openIndex.value].items[index];
  if (item.submenu) {
    subIndex.value = index;
    subFocus.value = -1;
    return;
  }
  if (!isSelectable(item)) return;
  close();
  item.action?.();
}

function activateSub(item: MenuItem) {
  if (!isSelectable(item)) return;
  close();
  item.action?.();
}

function nextIndex(items: MenuItem[], from: number, step: 1 | -1): number {
  const count = items.length;
  for (let i = 1; i <= count; i++) {
    const index = (from + step * i + count * 2) % count;
    if (isSelectable(items[index])) return index;
  }
  return -1;
}

function onKeydown(event: KeyboardEvent) {
  if (openIndex.value < 0) return;
  const menu = props.menus[openIndex.value];
  const inSub = subIndex.value >= 0 && subFocus.value >= 0;
  const subItems = subIndex.value >= 0 ? menu.items[subIndex.value].submenu ?? [] : [];

  switch (event.key) {
    case "Escape":
      if (subIndex.value >= 0) {
        subIndex.value = -1;
        subFocus.value = -1;
      } else {
        close();
      }
      break;
    case "ArrowDown":
      if (subIndex.value >= 0 && (subFocus.value >= 0 || focusIndex.value === subIndex.value)) {
        subFocus.value = nextIndex(subItems, subFocus.value, 1);
      } else {
        focusIndex.value = nextIndex(menu.items, focusIndex.value, 1);
      }
      break;
    case "ArrowUp":
      if (subIndex.value >= 0 && subFocus.value >= 0) {
        subFocus.value = nextIndex(subItems, subFocus.value, -1);
      } else {
        focusIndex.value = nextIndex(menu.items, focusIndex.value, -1);
      }
      break;
    case "ArrowRight":
      if (focusIndex.value >= 0 && menu.items[focusIndex.value].submenu) {
        const list = menu.items[focusIndex.value].submenu ?? [];
        subIndex.value = focusIndex.value;
        subFocus.value = nextIndex(list, -1, 1);
      } else {
        openIndex.value = (openIndex.value + 1) % props.menus.length;
        focusIndex.value = -1;
        subIndex.value = -1;
      }
      break;
    case "ArrowLeft":
      if (subIndex.value >= 0) {
        subIndex.value = -1;
        subFocus.value = -1;
      } else {
        openIndex.value = (openIndex.value - 1 + props.menus.length) % props.menus.length;
        focusIndex.value = -1;
      }
      break;
    case "Enter":
      if (inSub) activateSub(subItems[subFocus.value]);
      else if (focusIndex.value >= 0) activate(focusIndex.value);
      break;
    default:
      return;
  }
  event.preventDefault();
  event.stopPropagation();
}

function onPointerDown(event: PointerEvent) {
  if (openIndex.value >= 0 && root.value && !root.value.contains(event.target as Node)) close();
}

onMounted(() => {
  window.addEventListener("keydown", onKeydown, true);
  window.addEventListener("pointerdown", onPointerDown);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown, true);
  window.removeEventListener("pointerdown", onPointerDown);
});
</script>

<template>
  <nav ref="root" class="menubar" role="menubar">
    <div class="brand">
      <img class="logo-badge" src="/energy.png" alt="Energy Voice Editor" width="28" height="28" />
      <span class="app-title">Energy Voice Editor</span>
    </div>

    <div class="menus-container">
      <div v-for="(menu, menuIndex) in menus" :key="menu.label" class="menu-anchor">
        <button
          class="menu-title"
          :class="{ active: openIndex === menuIndex }"
          :aria-expanded="openIndex === menuIndex"
          @click="toggleMenu(menuIndex)"
          @mouseenter="hoverMenu(menuIndex)"
        >{{ menu.label }}</button>

        <div v-if="openIndex === menuIndex" class="dropdown" role="menu">
          <template v-for="(item, itemIndex) in menu.items" :key="itemIndex">
            <div v-if="item.separator" class="separator"></div>
            <div
              v-else
              class="item"
              :class="{ disabled: item.disabled, focused: focusIndex === itemIndex }"
              role="menuitem"
              :aria-disabled="item.disabled"
              @mouseenter="hoverItem(itemIndex)"
              @click.stop="activate(itemIndex)"
            >
              <span class="check">{{ item.checked ? "✓" : "" }}</span>
              <span class="label">{{ item.label }}</span>
              <span class="shortcut">{{ item.submenu ? "▸" : item.shortcut ?? "" }}</span>

              <div v-if="item.submenu && subIndex === itemIndex" class="dropdown submenu" role="menu">
                <template v-for="(sub, subItemIndex) in item.submenu" :key="subItemIndex">
                  <div v-if="sub.separator" class="separator"></div>
                  <div
                    v-else
                    class="item"
                    :class="{ disabled: sub.disabled, focused: subFocus === subItemIndex }"
                    role="menuitem"
                    :aria-disabled="sub.disabled"
                    @mouseenter="subFocus = subItemIndex"
                    @click.stop="activateSub(sub)"
                  >
                    <span class="label">{{ sub.label }}</span>
                    <span class="shortcut">{{ sub.shortcut ?? "" }}</span>
                  </div>
                </template>
              </div>
            </div>
          </template>
        </div>
      </div>
    </div>

    <div class="status">
      <span class="status-dot"></span>
      <span class="status-text">{{ statusText }}</span>
    </div>
  </nav>
</template>

<style scoped>
.menubar {
  display: flex;
  align-items: center;
  height: 44px;
  background: var(--panel);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  padding: 0 12px;
  color: var(--text);
  font-size: 13px;
  user-select: none;
  box-sizing: border-box;
}

.brand {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-right: 20px;
}

.logo-badge {
  width: 26px;
  height: 26px;
  background: var(--accent);
  color: #1a1405;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 800;
  font-size: 14px;
}

.app-title {
  font-weight: 700;
  font-size: 14px;
  letter-spacing: -0.2px;
  color: var(--text);
}

.menus-container {
  display: flex;
  align-items: center;
  gap: 4px;
  flex: 1;
}

.menu-anchor {
  position: relative;
}

.menu-title {
  height: 30px;
  padding: 0 10px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: color-mix(in srgb, var(--text) 60%, transparent);
  font: inherit;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;
}

.menu-title:hover,
.menu-title.active {
  background: var(--button);
  color: var(--text);
}

.status {
  display: flex;
  align-items: center;
  gap: 8px;
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: color-mix(in srgb, var(--text) 60%, transparent);
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--accent);
  box-shadow: 0 0 8px rgba(45, 226, 196, 0.6);
}

.dropdown {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  z-index: 50;
  min-width: 240px;
  max-height: calc(100vh - 80px);
  overflow-y: auto;
  padding: 6px;
  background: var(--panel);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  animation: menu-in 0.12s ease-out;
}

.dropdown.submenu {
  top: -4px;
  left: calc(100% + 4px);
  min-width: 200px;
}

.item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 6px;
  white-space: nowrap;
  cursor: pointer;
  color: var(--text);
  transition: background 0.12s ease;
}

.item .label {
  flex: 1;
}

.item .check {
  width: 14px;
  color: var(--accent);
  font-weight: bold;
}

.item .shortcut {
  margin-left: 20px;
  color: color-mix(in srgb, var(--text) 60%, transparent);
  font-size: 11px;
  font-family: "JetBrains Mono", monospace;
}

.item.focused,
.item:hover:not(.disabled) {
  background: var(--button);
  color: var(--accent);
}

.item.disabled {
  opacity: 0.4;
  cursor: default;
}

.separator {
  height: 1px;
  margin: 4px 0;
  background: rgba(255, 255, 255, 0.08);
}

@keyframes menu-in {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
